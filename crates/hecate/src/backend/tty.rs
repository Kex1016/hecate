//! Real session backend: DRM/KMS outputs, libinput, libseat.
//!
//! Structure follows Smithay's anvil `udev.rs` (MIT). hecate renders with a single
//! `GlesRenderer` on the primary GPU; outputs that can only be driven by a different GPU are
//! skipped for now.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::format::FormatSet;
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::drm::compositor::FrameFlags;
use smithay::backend::drm::exporter::gbm::GbmFramebufferExporter;
use smithay::backend::drm::output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements};
use smithay::backend::drm::{
    DrmDevice, DrmDeviceFd, DrmEvent, DrmEventMetadata, DrmEventTime, DrmNode, NodeType,
};
use smithay::backend::egl::context::ContextPriority;
use smithay::backend::egl::{EGLContext, EGLDevice, EGLDisplay};
use smithay::backend::input::InputEvent;
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::{ImportDma, ImportEgl, ImportMemWl};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::backend::udev::{UdevBackend, UdevEvent, all_gpus, primary_gpu};
use smithay::desktop::utils::OutputPresentationFeedback;
use smithay::input::keyboard::LedState;
use smithay::output::{Mode as WlMode, Output, PhysicalProperties};
use smithay::reexports::calloop::{LoopHandle, RegistrationToken};
use smithay::reexports::drm::Device as _;
use smithay::reexports::drm::control::{Device as _, ModeTypeFlags, connector, crtc};
use smithay::reexports::input::{DeviceCapability, Libinput};
use smithay::reexports::rustix::fs::OFlags;
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::DeviceFd;
use smithay::wayland::dmabuf::{DmabufFeedbackBuilder, DmabufGlobal};
use smithay::wayland::drm_lease::{DrmLease, DrmLeaseBuilder, DrmLeaseRequest, DrmLeaseState, LeaseRejected};
use smithay::wayland::presentation::Refresh;
use smithay_drm_extras::display_info;
use smithay_drm_extras::drm_scanner::{DrmScanEvent, DrmScanner};

use super::gamma::GammaProps;
use crate::config::Config;
use crate::render::HecateElement;
use crate::state::{Hecate, State};

const SUPPORTED_FORMATS: &[Fourcc] = &[
    Fourcc::Abgr2101010,
    Fourcc::Argb2101010,
    Fourcc::Abgr8888,
    Fourcc::Argb8888,
];

type HecateDrmOutput = DrmOutput<
    GbmAllocator<DrmDeviceFd>,
    GbmFramebufferExporter<DrmDeviceFd>,
    Option<OutputPresentationFeedback>,
    DrmDeviceFd,
>;
type HecateDrmOutputManager = DrmOutputManager<
    GbmAllocator<DrmDeviceFd>,
    GbmFramebufferExporter<DrmDeviceFd>,
    Option<OutputPresentationFeedback>,
    DrmDeviceFd,
>;

/// Which DRM device and CRTC an output is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TtyOutputId {
    node: DrmNode,
    crtc: crtc::Handle,
}

struct Surface {
    output: Output,
    drm_output: HecateDrmOutput,
    connector: connector::Handle,
    vrr_supported: bool,
    gamma: Option<GammaProps>,
    /// Gamma set while the session was inactive, applied on resume.
    pending_gamma: Option<Option<Vec<u16>>>,
}

struct Device {
    token: RegistrationToken,
    manager: HecateDrmOutputManager,
    scanner: DrmScanner,
    surfaces: HashMap<crtc::Handle, Surface>,
    gbm: GbmDevice<DrmDeviceFd>,
    /// drm-lease for non-desktop connectors (VR headsets).
    lease_state: Option<DrmLeaseState>,
    non_desktop: Vec<(connector::Handle, crtc::Handle)>,
    active_leases: Vec<DrmLease>,
}

pub struct Tty {
    /// Explicit sync, when the primary GPU supports syncobj eventfds.
    pub syncobj_state: Option<smithay::wayland::drm_syncobj::DrmSyncobjState>,
    session: LibSeatSession,
    libinput: Libinput,
    primary_node: DrmNode,
    primary_render_node: DrmNode,
    renderer: Option<GlesRenderer>,
    devices: HashMap<DrmNode, Device>,
    dmabuf_global: Option<DmabufGlobal>,
    keyboards: Vec<smithay::reexports::input::Device>,
    input_devices: Vec<smithay::reexports::input::Device>,
    event_loop: LoopHandle<'static, State>,
}

impl Tty {
    pub fn new(event_loop: LoopHandle<'static, State>) -> anyhow::Result<Self> {
        let (session, notifier) = LibSeatSession::new()
            .map_err(|e| anyhow::anyhow!("starting a libseat session (is seatd or logind running?): {e}"))?;
        let seat = session.seat();

        let primary_render_node = match std::env::var("HECATE_DRM_DEVICE") {
            Ok(path) => DrmNode::from_path(path)?,
            Err(_) => primary_gpu(&seat)?
                .and_then(|p| DrmNode::from_path(p).ok()?.node_with_type(NodeType::Render)?.ok())
                .or_else(|| {
                    all_gpus(&seat)
                        .ok()?
                        .into_iter()
                        .find_map(|p| DrmNode::from_path(p).ok())
                })
                .ok_or_else(|| anyhow::anyhow!("no GPU found"))?,
        };
        let primary_node = primary_render_node
            .node_with_type(NodeType::Primary)
            .and_then(Result::ok)
            .unwrap_or(primary_render_node);
        tracing::info!("using {primary_render_node} as the primary GPU");

        let mut libinput =
            Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
        libinput
            .udev_assign_seat(&seat)
            .map_err(|_| anyhow::anyhow!("assigning libinput seat {seat}"))?;
        let input_backend = LibinputInputBackend::new(libinput.clone());
        event_loop
            .insert_source(input_backend, |mut event, _, state| {
                if let crate::backend::Backend::Tty(tty) = &mut state.backend {
                    match &mut event {
                        InputEvent::DeviceAdded { device } => {
                            if device.has_capability(DeviceCapability::Keyboard) {
                                if let Some(kb) = state.hecate.seat.get_keyboard() {
                                    device.led_update(kb.led_state().into());
                                }
                                tty.keyboards.push(device.clone());
                            }
                            apply_libinput_config(device, &state.hecate.config);
                            tty.input_devices.push(device.clone());
                        }
                        InputEvent::DeviceRemoved { device } => {
                            tty.keyboards.retain(|d| d != device);
                            tty.input_devices.retain(|d| d != device);
                        }
                        _ => {}
                    }
                }
                state.process_input_event(event);
            })
            .map_err(|e| anyhow::anyhow!("inserting libinput source: {e}"))?;

        let mut libinput_for_session = libinput.clone();
        event_loop
            .insert_source(notifier, move |event, _, state| match event {
                SessionEvent::PauseSession => {
                    tracing::info!("session paused");
                    libinput_for_session.suspend();
                    if let crate::backend::Backend::Tty(tty) = &mut state.backend {
                        for device in tty.devices.values_mut() {
                            device.manager.pause();
                            if let Some(lease_state) = &mut device.lease_state {
                                lease_state.suspend();
                            }
                        }
                    }
                }
                SessionEvent::ActivateSession => {
                    tracing::info!("session resumed");
                    if let Err(err) = libinput_for_session.resume() {
                        tracing::warn!("error resuming libinput: {err:?}");
                    }
                    if let crate::backend::Backend::Tty(tty) = &mut state.backend {
                        for device in tty.devices.values_mut() {
                            if let Err(err) = device.manager.lock().activate(false) {
                                tracing::warn!("error re-activating DRM device: {err:?}");
                            }
                            if let Some(lease_state) = &mut device.lease_state {
                                lease_state.resume::<State>();
                            }
                            // Another session may have changed gamma.
                            let drm = device.manager.device();
                            for surface in device.surfaces.values_mut() {
                                let Some(gamma) = &mut surface.gamma else {
                                    continue;
                                };
                                let res = match surface.pending_gamma.take() {
                                    Some(ramp) => gamma.set_gamma(drm, ramp),
                                    None => gamma.restore_gamma(drm),
                                };
                                if let Err(err) = res {
                                    tracing::warn!("error restoring gamma: {err:?}");
                                }
                            }
                        }
                    }
                    let outputs: Vec<Output> = state.hecate.output_state.keys().cloned().collect();
                    for o in outputs {
                        if let Some(s) = state.hecate.output_state.get_mut(&o) {
                            s.waiting_for_vblank = false;
                        }
                    }
                    state.hecate.queue_redraw_all();
                }
            })
            .map_err(|e| anyhow::anyhow!("inserting session source: {e}"))?;

        Ok(Tty {
            syncobj_state: None,
            session,
            libinput,
            primary_node,
            primary_render_node,
            renderer: None,
            devices: HashMap::new(),
            dmabuf_global: None,
            keyboards: Vec::new(),
            input_devices: Vec::new(),
            event_loop,
        })
    }

    pub fn init(&mut self, hecate: &mut Hecate) {
        let udev = match UdevBackend::new(self.session.seat()) {
            Ok(u) => u,
            Err(err) => {
                tracing::error!("error starting udev: {err}");
                return;
            }
        };
        // Primary device first so its renderer exists for the others.
        let devices: Vec<(libc::dev_t, std::path::PathBuf)> =
            udev.device_list().map(|(id, p)| (id, p.to_path_buf())).collect();
        let (primary, rest): (Vec<_>, Vec<_>) = devices.into_iter().partition(|(id, _)| {
            *id == self.primary_node.dev_id() || *id == self.primary_render_node.dev_id()
        });
        for (id, path) in primary.into_iter().chain(rest) {
            if let Ok(node) = DrmNode::from_dev_id(id)
                && let Err(err) = self.device_added(hecate, node, &path)
            {
                tracing::warn!("skipping DRM device {path:?}: {err:?}");
            }
        }

        if let Some(renderer) = self.renderer.as_mut() {
            if renderer.bind_wl_display(&hecate.display_handle).is_ok() {
                tracing::info!("EGL hardware acceleration enabled for clients");
            }
            hecate.shm_state.update_formats(renderer.shm_formats());
            let formats = renderer.dmabuf_formats();
            match DmabufFeedbackBuilder::new(self.primary_render_node.dev_id(), formats).build() {
                Ok(feedback) => {
                    self.dmabuf_global = Some(
                        hecate
                            .dmabuf_state
                            .create_global_with_default_feedback::<State>(&hecate.display_handle, &feedback),
                    );
                }
                Err(err) => tracing::warn!("error building dmabuf feedback: {err:?}"),
            }
        }

        // Explicit sync (linux-drm-syncobj-v1) if the primary device can do it.
        if let Some(device) = self.devices.get(&self.primary_node) {
            let fd = device.manager.device().device_fd().clone();
            if smithay::wayland::drm_syncobj::supports_syncobj_eventfd(&fd) {
                self.syncobj_state = Some(smithay::wayland::drm_syncobj::DrmSyncobjState::new::<State>(
                    &hecate.display_handle,
                    fd,
                ));
                tracing::info!("explicit sync enabled");
            }
        }

        let res = self.event_loop.insert_source(udev, |event, _, state| {
            let crate::backend::Backend::Tty(tty) = &mut state.backend else {
                return;
            };
            match event {
                UdevEvent::Added { device_id, path } => {
                    if let Ok(node) = DrmNode::from_dev_id(device_id)
                        && let Err(err) = tty.device_added(&mut state.hecate, node, &path)
                    {
                        tracing::warn!("skipping DRM device {path:?}: {err:?}");
                    }
                }
                UdevEvent::Changed { device_id } => {
                    if let Ok(node) = DrmNode::from_dev_id(device_id) {
                        tty.device_changed(&mut state.hecate, node);
                    }
                }
                UdevEvent::Removed { device_id } => {
                    if let Ok(node) = DrmNode::from_dev_id(device_id) {
                        tty.device_removed(&mut state.hecate, node);
                    }
                }
            }
            let outputs: Vec<Output> = state.hecate.wm.outputs().cloned().collect();
            for o in outputs {
                state.on_usable_area_changed(&o);
            }
        });
        if let Err(err) = res {
            tracing::error!("error inserting udev source: {err}");
        }
    }

    pub fn seat_name(&self) -> String {
        self.session.seat()
    }

    fn device_added(&mut self, hecate: &mut Hecate, node: DrmNode, path: &Path) -> anyhow::Result<()> {
        let fd = self.session.open(
            path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        )?;
        let fd = DrmDeviceFd::new(DeviceFd::from(fd));
        let (drm, notifier) = DrmDevice::new(fd.clone(), true)?;
        let gbm = GbmDevice::new(fd)?;

        // SAFETY: the GBM device outlives the display (both live in this backend).
        let display = unsafe { EGLDisplay::new(gbm.clone())? };
        let egl_device = EGLDevice::device_for_display(&display)?;
        let render_node = egl_device.try_get_render_node().ok().flatten().unwrap_or(node);
        if render_node != self.primary_render_node && self.renderer.is_some() {
            anyhow::bail!(
                "outputs on secondary GPU {render_node} are not supported yet (primary is {})",
                self.primary_render_node
            );
        }
        if self.renderer.is_none() {
            let context = EGLContext::new_with_priority(&display, ContextPriority::High)?;
            // SAFETY: the context is current-able on this thread.
            let mut renderer = unsafe { GlesRenderer::new(context)? };
            crate::render::border::Shaders::init(&mut renderer);
            self.renderer = Some(renderer);
            self.primary_render_node = render_node;
        }

        let token = self
            .event_loop
            .insert_source(notifier, move |event, meta, state| match event {
                DrmEvent::VBlank(crtc) => {
                    if let crate::backend::Backend::Tty(tty) = &mut state.backend {
                        tty.on_vblank(&mut state.hecate, node, crtc, meta);
                    }
                }
                DrmEvent::Error(err) => tracing::warn!("DRM error: {err:?}"),
            })
            .map_err(|e| anyhow::anyhow!("inserting DRM source: {e}"))?;

        let allocator = GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
        let exporter = GbmFramebufferExporter::new(gbm.clone(), Some(render_node).into());
        let renderer = self.renderer.as_mut().unwrap();
        let render_formats: FormatSet = renderer
            .egl_context()
            .dmabuf_render_formats()
            .iter()
            .copied()
            .collect();
        let manager = DrmOutputManager::new(
            drm,
            allocator,
            exporter,
            Some(gbm.clone()),
            SUPPORTED_FORMATS.iter().copied(),
            render_formats,
        );
        let lease_state = DrmLeaseState::new::<State>(&hecate.display_handle, &node)
            .inspect_err(|err| tracing::debug!("no DRM leasing on {node}: {err:?}"))
            .ok();
        self.devices.insert(
            node,
            Device {
                token,
                manager,
                scanner: DrmScanner::new(),
                surfaces: HashMap::new(),
                gbm,
                lease_state,
                non_desktop: Vec::new(),
                active_leases: Vec::new(),
            },
        );
        self.device_changed(hecate, node);
        Ok(())
    }

    fn device_changed(&mut self, hecate: &mut Hecate, node: DrmNode) {
        let Some(device) = self.devices.get_mut(&node) else {
            return;
        };
        let events = match device.scanner.scan_connectors(device.manager.device()) {
            Ok(r) => r,
            Err(err) => {
                tracing::warn!("error scanning connectors: {err:?}");
                return;
            }
        };
        for event in events {
            match event {
                DrmScanEvent::Connected {
                    connector,
                    crtc: Some(crtc),
                } => self.connector_connected(hecate, node, connector, crtc),
                DrmScanEvent::Disconnected {
                    connector,
                    crtc: Some(crtc),
                } => self.connector_disconnected(hecate, node, connector, crtc),
                _ => {}
            }
        }
    }

    fn device_removed(&mut self, hecate: &mut Hecate, node: DrmNode) {
        let Some(device) = self.devices.get_mut(&node) else {
            return;
        };
        let crtcs: Vec<(connector::Info, crtc::Handle)> = device
            .scanner
            .crtcs()
            .map(|(info, crtc)| (info.clone(), crtc))
            .collect();
        for (connector, crtc) in crtcs {
            self.connector_disconnected(hecate, node, connector, crtc);
        }
        if let Some(mut device) = self.devices.remove(&node) {
            if let Some(lease_state) = &mut device.lease_state {
                lease_state.disable_global::<State>();
            }
            self.event_loop.remove(device.token);
        }
    }

    fn connector_connected(
        &mut self,
        hecate: &mut Hecate,
        node: DrmNode,
        connector: connector::Info,
        crtc: crtc::Handle,
    ) {
        let Some(device) = self.devices.get_mut(&node) else {
            return;
        };
        if is_non_desktop(device.manager.device(), connector.handle()) {
            // VR headsets and the like: offered through drm-lease, never part of the desktop.
            let name = connector_name(&connector);
            if !device.non_desktop.contains(&(connector.handle(), crtc)) {
                tracing::info!("non-desktop connector {name} available for leasing");
                if let Some(lease_state) = &mut device.lease_state {
                    lease_state.add_connector::<State>(connector.handle(), name.clone(), name);
                }
                device.non_desktop.push((connector.handle(), crtc));
            }
            return;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let name = connector_name(&connector);
        let config = hecate.config.outputs.get(&name).cloned().unwrap_or_default();
        if config.enable == Some(false) {
            tracing::info!("output {name} disabled in config");
            return;
        }
        let drm = device.manager.device();
        let info = display_info::for_connector(drm, connector.handle());
        let make = info
            .as_ref()
            .and_then(|i| i.make())
            .unwrap_or_else(|| "Unknown".into());
        let model = info
            .as_ref()
            .and_then(|i| i.model())
            .unwrap_or_else(|| "Unknown".into());
        let serial = info
            .as_ref()
            .and_then(|i| i.serial())
            .unwrap_or_else(|| "Unknown".into());

        let modes = connector.modes();
        if modes.is_empty() {
            tracing::warn!("output {name} has no modes");
            return;
        }
        let mode = pick_mode(modes, config.mode.as_deref());
        let wl_mode = WlMode::from(mode);
        let (pw, ph) = connector.size().unwrap_or((0, 0));
        let output = Output::new(
            name.clone(),
            PhysicalProperties {
                size: (pw as i32, ph as i32).into(),
                subpixel: connector.subpixel().into(),
                make,
                model,
                serial_number: serial,
            },
        );
        for m in modes {
            output.add_mode(WlMode::from(*m));
        }
        output.set_preferred(WlMode::from(
            *modes
                .iter()
                .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
                .unwrap_or(&modes[0]),
        ));
        output.change_current_state(Some(wl_mode), None, None, None);
        output
            .user_data()
            .insert_if_missing(|| TtyOutputId { node, crtc });

        let planes = match drm.planes(&crtc) {
            Ok(mut p) => {
                // Overlay planes are unreliable on NVIDIA.
                if drm
                    .get_driver()
                    .is_ok_and(|d| d.name().to_string_lossy().to_lowercase().contains("nvidia"))
                {
                    p.overlay.clear();
                }
                Some(p)
            }
            Err(_) => None,
        };
        let drm_output = match device.manager.lock().initialize_output::<_, HecateElement>(
            crtc,
            mode,
            &[connector.handle()],
            &output,
            planes,
            renderer,
            &DrmOutputRenderElements::default(),
        ) {
            Ok(o) => o,
            Err(err) => {
                tracing::warn!("error initializing output {name}: {err:?}");
                return;
            }
        };

        let vrr_supported = drm_output
            .with_compositor(|c| c.vrr_supported(connector.handle()))
            .map(|s| s != smithay::backend::drm::VrrSupport::NotSupported)
            .unwrap_or(false);
        tracing::info!(
            "output {name} connected ({}x{}@{})",
            wl_mode.size.w,
            wl_mode.size.h,
            wl_mode.refresh
        );
        let drm = device.manager.device();
        let mut gamma = GammaProps::new(drm, crtc)
            .inspect_err(|err| tracing::debug!("no gamma control for {name}: {err:?}"))
            .ok();
        // Reset gamma left over from a previous session.
        if let Some(g) = &mut gamma
            && let Err(err) = g.set_gamma(drm, None)
        {
            tracing::debug!("couldn't reset gamma on {name}: {err:?}");
        }
        device.surfaces.insert(
            crtc,
            Surface {
                output: output.clone(),
                drm_output,
                connector: connector.handle(),
                vrr_supported,
                gamma,
                pending_gamma: None,
            },
        );
        hecate.add_output(output.clone());
    }

    fn connector_disconnected(
        &mut self,
        hecate: &mut Hecate,
        node: DrmNode,
        _connector: connector::Info,
        crtc: crtc::Handle,
    ) {
        let Some(device) = self.devices.get_mut(&node) else {
            return;
        };
        if let Some(surface) = device.surfaces.remove(&crtc) {
            tracing::info!("output {} disconnected", surface.output.name());
            hecate.remove_output(&surface.output);
        } else if let Some(idx) = device.non_desktop.iter().position(|(_, c)| *c == crtc) {
            let (conn, _) = device.non_desktop.remove(idx);
            if let Some(lease_state) = &mut device.lease_state {
                lease_state.withdraw_connector(conn);
            }
        }
    }

    /// Build a lease for a client's requested non-desktop connectors.
    pub fn lease_request(
        &self,
        node: DrmNode,
        request: DrmLeaseRequest,
    ) -> Result<DrmLeaseBuilder, LeaseRejected> {
        let device = self.devices.get(&node).ok_or_else(LeaseRejected::default)?;
        let drm = device.manager.device();
        let mut builder = DrmLeaseBuilder::new(drm);
        for connector in request.connectors {
            let (_, crtc) = device
                .non_desktop
                .iter()
                .find(|(c, _)| *c == connector)
                .copied()
                .ok_or_else(|| {
                    tracing::warn!("lease request for a desktop connector");
                    LeaseRejected::default()
                })?;
            builder.add_connector(connector);
            builder.add_crtc(crtc);
            let planes = drm.planes(&crtc).map_err(LeaseRejected::with_cause)?;
            let (plane, claim) = planes
                .primary
                .iter()
                .find_map(|p| drm.claim_plane(p.handle, crtc).map(|claim| (p.handle, claim)))
                .ok_or_else(LeaseRejected::default)?;
            builder.add_plane(plane, claim);
        }
        Ok(builder)
    }

    pub fn lease_state(&mut self, node: DrmNode) -> Option<&mut DrmLeaseState> {
        self.devices.get_mut(&node)?.lease_state.as_mut()
    }

    pub fn add_lease(&mut self, node: DrmNode, lease: DrmLease) {
        if let Some(d) = self.devices.get_mut(&node) {
            d.active_leases.push(lease);
        }
    }

    pub fn remove_lease(&mut self, node: DrmNode, id: u32) {
        if let Some(d) = self.devices.get_mut(&node) {
            d.active_leases.retain(|l| l.id() != id);
        }
    }

    fn surface_for(&mut self, output: &Output) -> Option<&mut Surface> {
        let id = *output.user_data().get::<TtyOutputId>()?;
        self.devices.get_mut(&id.node)?.surfaces.get_mut(&id.crtc)
    }

    pub fn render_node(&self) -> DrmNode {
        self.primary_render_node
    }

    pub fn gamma_size(&self, output: &Output) -> Option<u32> {
        let id = *output.user_data().get::<TtyOutputId>()?;
        let surface = self.devices.get(&id.node)?.surfaces.get(&id.crtc)?;
        surface.gamma.as_ref().map(GammaProps::gamma_size)
    }

    pub fn set_gamma(&mut self, output: &Output, ramp: Option<Vec<u16>>) -> anyhow::Result<()> {
        let id = *output
            .user_data()
            .get::<TtyOutputId>()
            .ok_or_else(|| anyhow::anyhow!("not a tty output"))?;
        let active = self.session.is_active();
        let device = self
            .devices
            .get_mut(&id.node)
            .ok_or_else(|| anyhow::anyhow!("missing device"))?;
        let surface = device
            .surfaces
            .get_mut(&id.crtc)
            .ok_or_else(|| anyhow::anyhow!("missing surface"))?;
        // Properties can't change while another session owns the device.
        if !active {
            surface.pending_gamma = Some(ramp);
            return Ok(());
        }
        let gamma = surface
            .gamma
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("setting gamma is not supported"))?;
        gamma.set_gamma(device.manager.device(), ramp)
    }

    pub fn render(&mut self, hecate: &mut Hecate, output: &Output) -> bool {
        if !self.session.is_active() || hecate.output_state.get(output).is_some_and(|s| s.powered_off) {
            return false;
        }
        let Some(mut renderer) = self.renderer.take() else {
            return false;
        };
        let elements = hecate.render_elements(&mut renderer, output);
        let direct_scanout = hecate.config.gaming.direct_scanout;
        let submitted = match self.surface_for(output) {
            Some(surface) => {
                let flags = if direct_scanout {
                    FrameFlags::DEFAULT
                } else {
                    FrameFlags::empty()
                };
                match surface
                    .drm_output
                    .render_frame(&mut renderer, &elements, [0., 0., 0., 1.], flags)
                {
                    Ok(res) => {
                        hecate.update_primary_scanout_output(output, &res.states);
                        if res.is_empty {
                            false
                        } else {
                            let feedback = hecate.take_presentation_feedback(output, &res.states);
                            match surface.drm_output.queue_frame(Some(feedback)) {
                                Ok(()) => true,
                                Err(err) => {
                                    tracing::warn!("error queueing frame: {err:?}");
                                    false
                                }
                            }
                        }
                    }
                    Err(err) => {
                        tracing::warn!("error rendering frame: {err:?}");
                        false
                    }
                }
            }
            None => false,
        };
        self.renderer = Some(renderer);
        submitted
    }

    fn on_vblank(
        &mut self,
        hecate: &mut Hecate,
        node: DrmNode,
        crtc: crtc::Handle,
        meta: &mut Option<DrmEventMetadata>,
    ) {
        let Some(surface) = self
            .devices
            .get_mut(&node)
            .and_then(|d| d.surfaces.get_mut(&crtc))
        else {
            return;
        };
        let output = surface.output.clone();
        let refresh = output.current_mode().map(|m| m.refresh).unwrap_or(60_000).max(1);
        let frame_duration = Duration::from_secs_f64(1000.0 / refresh as f64);
        let (time, flags) = match meta.as_ref().map(|m| m.time) {
            Some(DrmEventTime::Monotonic(tp)) if !tp.is_zero() => (
                tp,
                wp_presentation_feedback::Kind::Vsync
                    | wp_presentation_feedback::Kind::HwClock
                    | wp_presentation_feedback::Kind::HwCompletion,
            ),
            _ => (
                crate::utils::monotonic_time(),
                wp_presentation_feedback::Kind::Vsync,
            ),
        };
        let seq = meta.as_ref().map(|m| m.sequence).unwrap_or(0);
        match surface.drm_output.frame_submitted() {
            Ok(Some(Some(mut feedback))) => {
                feedback.presented::<_, smithay::utils::Monotonic>(
                    time,
                    Refresh::fixed(frame_duration),
                    seq as u64,
                    flags,
                );
            }
            Ok(_) => {}
            Err(err) => tracing::warn!("frame_submitted error: {err:?}"),
        }
        hecate.on_vblank(&output, time, frame_duration);
    }

    pub fn renderer_mut(&mut self) -> Option<&mut GlesRenderer> {
        self.renderer.as_mut()
    }

    pub fn import_dmabuf(&mut self, dmabuf: &Dmabuf) -> bool {
        let ok = self
            .renderer
            .as_mut()
            .is_some_and(|r| r.import_dmabuf(dmabuf, None).is_ok());
        if ok && dmabuf.node().is_none() {
            dmabuf.set_node(self.primary_render_node);
        }
        ok
    }

    pub fn early_import(&mut self, surface: &WlSurface) {
        if let Some(renderer) = self.renderer.as_mut() {
            let _ = smithay::backend::renderer::utils::import_surface_tree(renderer, surface);
        }
    }

    pub fn update_led_state(&mut self, led: LedState) {
        for kb in &mut self.keyboards {
            kb.led_update(led.into());
        }
    }

    pub fn change_vt(&mut self, vt: i32) {
        if let Err(err) = self.session.change_vt(vt) {
            tracing::warn!("error switching to VT {vt}: {err}");
        }
    }

    /// Turn one output's CRTC off (DPMS) or back on.
    pub fn set_output_powered(&mut self, hecate: &mut Hecate, output: &Output, on: bool) {
        let Some(surface) = self.surface_for(output) else {
            return;
        };
        let res = if on {
            surface.drm_output.with_compositor(|c| c.reset_state())
        } else {
            surface.drm_output.with_compositor(|c| c.clear())
        };
        if let Err(err) = res {
            tracing::warn!(
                "error powering {} {}: {err:?}",
                output.name(),
                if on { "on" } else { "off" }
            );
        }
        if on && let Some(s) = hecate.output_state.get_mut(output) {
            s.waiting_for_vblank = false;
        }
    }

    pub fn heads(&self, hecate: &Hecate) -> Vec<super::HeadInfo> {
        let mut heads = Vec::new();
        for device in self.devices.values() {
            for (conn, crtc) in device.scanner.crtcs() {
                if conn.state() != connector::State::Connected
                    || device.non_desktop.iter().any(|(c, _)| *c == conn.handle())
                {
                    continue;
                }
                if let Some(surface) = device.surfaces.get(&crtc) {
                    heads.push(super::HeadInfo::from_output(
                        &surface.output,
                        &hecate.config,
                        surface.vrr_supported,
                    ));
                    continue;
                }
                let info = display_info::for_connector(device.manager.device(), conn.handle());
                let field = |f: Option<String>| f.unwrap_or_else(|| "Unknown".into());
                let (pw, ph) = conn.size().unwrap_or((0, 0));
                heads.push(super::HeadInfo {
                    name: connector_name(conn),
                    make: field(info.as_ref().and_then(|i| i.make())),
                    model: field(info.as_ref().and_then(|i| i.model())),
                    serial: field(info.as_ref().and_then(|i| i.serial())),
                    physical_size: (pw as i32, ph as i32),
                    modes: conn
                        .modes()
                        .iter()
                        .map(|m| {
                            let wl = WlMode::from(*m);
                            super::HeadMode {
                                width: wl.size.w,
                                height: wl.size.h,
                                refresh: wl.refresh,
                                preferred: m.mode_type().contains(ModeTypeFlags::PREFERRED),
                            }
                        })
                        .collect(),
                    current: None,
                    vrr_supported: false,
                });
            }
        }
        heads.sort_by(|a, b| a.name.cmp(&b.name));
        heads
    }

    /// Turn outputs off and on per `enable`, then apply modes and the rest.
    fn apply_output_enablement(&mut self, hecate: &mut Hecate) {
        let mut to_disable = Vec::new();
        let mut to_enable = Vec::new();
        for (node, device) in &self.devices {
            for (conn, crtc) in device.scanner.crtcs() {
                if conn.state() != connector::State::Connected {
                    continue;
                }
                let enable = hecate
                    .config
                    .outputs
                    .get(&connector_name(conn))
                    .and_then(|c| c.enable)
                    != Some(false);
                match (device.surfaces.contains_key(&crtc), enable) {
                    (true, false) => to_disable.push((*node, crtc)),
                    (false, true) => to_enable.push((*node, conn.clone(), crtc)),
                    _ => (),
                }
            }
        }
        for (node, crtc) in to_disable {
            if let Some(surface) = self.devices.get_mut(&node).and_then(|d| d.surfaces.remove(&crtc)) {
                tracing::info!("disabling output {}", surface.output.name());
                hecate.remove_output(&surface.output);
                // Dropping the DrmOutput turns the CRTC off.
            }
        }
        for (node, conn, crtc) in to_enable {
            self.connector_connected(hecate, node, conn, crtc);
        }
    }

    /// Re-apply enablement, modes, scale, transform and position from the config.
    pub fn apply_output_config(&mut self, hecate: &mut Hecate) {
        self.apply_output_enablement(hecate);
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        for device in self.devices.values_mut() {
            for surface in device.surfaces.values_mut() {
                let name = surface.output.name();
                let config = hecate.config.outputs.get(&name).cloned().unwrap_or_default();
                let Ok(info) = device.manager.device().get_connector(surface.connector, false) else {
                    continue;
                };
                let mode = pick_mode(info.modes(), config.mode.as_deref());
                if surface.output.current_mode() != Some(WlMode::from(mode)) {
                    match surface.drm_output.use_mode::<_, HecateElement>(
                        mode,
                        renderer,
                        &DrmOutputRenderElements::default(),
                    ) {
                        Ok(()) => {
                            surface
                                .output
                                .change_current_state(Some(WlMode::from(mode)), None, None, None);
                        }
                        Err(err) => tracing::warn!("error setting mode on {name}: {err:?}"),
                    }
                }
            }
        }
        let outputs: Vec<Output> = hecate.wm.outputs().cloned().collect();
        for o in &outputs {
            hecate.apply_output_props(o);
            // refresh_vrr re-applies VRR with the new config.
            if let Some(s) = hecate.output_state.get_mut(o) {
                s.vrr_applied = None;
            }
        }
        hecate.queue_redraw_all();
    }

    pub fn vrr_supported(&self, output: &Output) -> bool {
        let Some(id) = output.user_data().get::<TtyOutputId>() else {
            return false;
        };
        self.devices
            .get(&id.node)
            .and_then(|d| d.surfaces.get(&id.crtc))
            .is_some_and(|s| s.vrr_supported)
    }

    /// Turn adaptive sync on or off for an output.
    pub fn set_vrr(&mut self, output: &Output, on: bool) {
        if let Some(surface) = self.surface_for(output)
            && surface.vrr_supported
            && let Err(err) = surface.drm_output.with_compositor(|c| c.use_vrr(on))
        {
            tracing::debug!("error setting VRR on {}: {err:?}", output.name());
        }
    }

    pub fn apply_input_config(&mut self, config: &Config) {
        for device in &mut self.input_devices {
            apply_libinput_config(device, config);
        }
    }

    pub fn on_device_added<D: smithay::backend::input::Device>(&mut self, _device: &D, _config: &Config) {}
}

/// Pick the DRM mode for a `WxH[@Hz]` string, else the preferred mode.
fn is_non_desktop(drm: &DrmDevice, connector: connector::Handle) -> bool {
    use smithay::reexports::drm::control::Device as _;
    let Ok(props) = drm.get_properties(connector) else {
        return false;
    };
    props.into_iter().any(|(handle, value)| {
        drm.get_property(handle).is_ok_and(|info| {
            info.name().to_str() == Ok("non-desktop")
                && info.value_type().convert_value(value).as_boolean() == Some(true)
        })
    })
}

fn connector_name(connector: &connector::Info) -> String {
    format!("{}-{}", connector.interface().as_str(), connector.interface_id())
}

fn pick_mode(
    modes: &[smithay::reexports::drm::control::Mode],
    wanted: Option<&str>,
) -> smithay::reexports::drm::control::Mode {
    let preferred = modes
        .iter()
        .position(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
        .unwrap_or(0);
    let triples: Vec<(i32, i32, i32)> = modes
        .iter()
        .map(|m| {
            let wl = WlMode::from(*m);
            (wl.size.w, wl.size.h, wl.refresh)
        })
        .collect();
    modes[super::pick_mode_index(&triples, preferred, wanted)]
}

/// Apply `[input]` settings to a libinput device. Order: `"*"`, `"type:..."`, then the name.
pub fn apply_libinput_config(device: &mut smithay::reexports::input::Device, config: &Config) {
    use crate::config::{AccelProfile, ClickMethod, InputDevice, ScrollMethod};
    use smithay::reexports::input as li;

    let is_touchpad = device.config_tap_finger_count() > 0;
    let is_pointer = device.has_capability(DeviceCapability::Pointer);
    let mut merged = InputDevice::default();
    let keys = [
        Some(device.name().to_string()),
        is_touchpad.then(|| "type:touchpad".to_string()),
        (is_pointer && !is_touchpad).then(|| "type:pointer".to_string()),
        Some("*".to_string()),
    ];
    for k in keys.into_iter().flatten() {
        if let Some(c) = config.input.get(&k) {
            merged.merge_from(c);
        }
    }
    let c = merged;
    if let Some(p) = c.accel_profile {
        let _ = device.config_accel_set_profile(match p {
            AccelProfile::Flat => li::AccelProfile::Flat,
            AccelProfile::Adaptive => li::AccelProfile::Adaptive,
        });
    }
    if let Some(s) = c.pointer_speed {
        let _ = device.config_accel_set_speed(s.clamp(-1., 1.));
    }
    if let Some(v) = c.natural_scroll {
        let _ = device.config_scroll_set_natural_scroll_enabled(v);
    }
    if let Some(v) = c.left_handed {
        let _ = device.config_left_handed_set(v);
    }
    if let Some(v) = c.tap {
        let _ = device.config_tap_set_enabled(v);
    }
    if let Some(v) = c.tap_and_drag {
        let _ = device.config_tap_set_drag_enabled(v);
    }
    if let Some(v) = c.drag_lock {
        let _ = device.config_tap_set_drag_lock_enabled(if v {
            li::DragLockState::EnabledTimeout
        } else {
            li::DragLockState::Disabled
        });
    }
    if let Some(v) = c.middle_emulation {
        let _ = device.config_middle_emulation_set_enabled(v);
    }
    if let Some(v) = c.disable_while_typing {
        let _ = device.config_dwt_set_enabled(v);
    }
    if let Some(m) = c.click_method {
        let _ = device.config_click_set_method(match m {
            ClickMethod::None => li::ClickMethod::ButtonAreas,
            ClickMethod::ButtonAreas => li::ClickMethod::ButtonAreas,
            ClickMethod::Clickfinger => li::ClickMethod::Clickfinger,
        });
    }
    if let Some(m) = c.scroll_method {
        let _ = device.config_scroll_set_method(match m {
            ScrollMethod::None => li::ScrollMethod::NoScroll,
            ScrollMethod::TwoFinger => li::ScrollMethod::TwoFinger,
            ScrollMethod::Edge => li::ScrollMethod::Edge,
            ScrollMethod::OnButton => li::ScrollMethod::OnButtonDown,
        });
    }
    if let Some(b) = c.scroll_button {
        let _ = device.config_scroll_set_button(b);
    }
    if let Some(m) = c.calibration_matrix {
        let _ = device.config_calibration_set_matrix(m);
    }
    if let Some(enabled) = c.enabled {
        let _ = device.config_send_events_set_mode(if enabled {
            li::SendEventsMode::ENABLED
        } else {
            li::SendEventsMode::DISABLED
        });
    }
}
