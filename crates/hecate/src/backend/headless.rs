//! Headless backend: virtual outputs, optional software rendering. Used by tests and
//! `--backend headless`.

use std::collections::HashMap;

use anyhow::Context as _;
use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::egl::native::EGLSurfacelessDisplay;
use smithay::backend::egl::{EGLContext, EGLDisplay};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{Bind, ExportMem, ImportDma, Offscreen, Renderer};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::utils::{Buffer, Rectangle, Size};

use crate::state::Hecate;

pub struct Headless {
    renderer: Option<GlesRenderer>,
    targets: HashMap<String, (GlesTexture, OutputDamageTracker)>,
    /// Last rendered frame per output: RGBA8, width, height.
    pub frames: HashMap<String, (Vec<u8>, i32, i32)>,
    /// Read back every frame into `frames` (tests).
    pub capture: bool,
    /// Gamma ramp set per output (tests).
    pub gamma: HashMap<String, Vec<u16>>,
    /// Every output ever added (name, native size), enabled or not, in order.
    known: Vec<(String, (i32, i32))>,
}

impl Default for Headless {
    fn default() -> Self {
        Self::new()
    }
}

impl Headless {
    pub fn new() -> Self {
        Headless {
            renderer: None,
            targets: HashMap::new(),
            frames: HashMap::new(),
            capture: false,
            gamma: HashMap::new(),
            known: Vec::new(),
        }
    }

    pub fn init(&mut self, _hecate: &mut Hecate) {}

    /// Create a surfaceless EGL renderer (Mesa llvmpipe works).
    pub fn add_renderer(&mut self) -> anyhow::Result<()> {
        // SAFETY: surfaceless EGL display creation.
        let mut renderer = unsafe {
            let display = EGLDisplay::new(EGLSurfacelessDisplay).context("creating EGL display")?;
            let context = EGLContext::new(&display).context("creating EGL context")?;
            GlesRenderer::new(context).context("creating renderer")?
        };
        crate::render::border::Shaders::init(&mut renderer);
        self.renderer = Some(renderer);
        Ok(())
    }

    pub fn has_renderer(&self) -> bool {
        self.renderer.is_some()
    }

    /// Modes a headless output offers: its native size at 60 Hz (preferred) and 144 Hz, and
    /// 1280x720 at 60 Hz.
    fn modes_for(size: (i32, i32)) -> Vec<Mode> {
        let mut modes = vec![
            Mode {
                size: size.into(),
                refresh: 60_000,
            },
            Mode {
                size: size.into(),
                refresh: 144_000,
            },
        ];
        if size != (1280, 720) {
            modes.push(Mode {
                size: (1280, 720).into(),
                refresh: 60_000,
            });
        }
        modes
    }

    fn make_output(name: &str, size: (i32, i32)) -> Output {
        let output = Output::new(
            name.to_string(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "hecate".into(),
                model: "headless".into(),
                serial_number: name.into(),
            },
        );
        let modes = Self::modes_for(size);
        for m in &modes {
            output.add_mode(*m);
        }
        output.set_preferred(modes[0]);
        output
    }

    fn pick(size: (i32, i32), wanted: Option<&str>) -> Mode {
        let modes = Self::modes_for(size);
        let triples: Vec<(i32, i32, i32)> = modes.iter().map(|m| (m.size.w, m.size.h, m.refresh)).collect();
        modes[super::pick_mode_index(&triples, 0, wanted)]
    }

    pub fn add_output(&mut self, hecate: &mut Hecate, name: &str, size: (i32, i32)) -> Output {
        if !self.known.iter().any(|(n, _)| n == name) {
            self.known.push((name.to_string(), size));
        }
        let output = Self::make_output(name, size);
        let wanted = hecate.config.outputs.get(name).and_then(|c| c.mode.clone());
        output.change_current_state(Some(Self::pick(size, wanted.as_deref())), None, None, None);
        if hecate.config.outputs.get(name).and_then(|c| c.enable) == Some(false) {
            return output;
        }
        hecate.add_output(output.clone());
        output
    }

    pub fn heads(&self, hecate: &Hecate) -> Vec<super::HeadInfo> {
        self.known
            .iter()
            .map(|(name, size)| match hecate.wm.output_by_name(name) {
                Some(o) => super::HeadInfo::from_output(o, &hecate.config, true),
                None => {
                    let mut head =
                        super::HeadInfo::from_output(&Self::make_output(name, *size), &hecate.config, true);
                    head.current = None;
                    head
                }
            })
            .collect()
    }

    /// Enable/disable outputs and switch modes per the config.
    pub fn apply_output_config(&mut self, hecate: &mut Hecate) {
        for (name, size) in self.known.clone() {
            let cfg = hecate.config.outputs.get(&name).cloned().unwrap_or_default();
            let existing = hecate.wm.output_by_name(&name).cloned();
            match (existing, cfg.enable != Some(false)) {
                (Some(o), false) => {
                    hecate.remove_output(&o);
                    self.targets.remove(&name);
                    self.frames.remove(&name);
                }
                (None, true) => {
                    self.add_output(hecate, &name, size);
                }
                (Some(o), true) => {
                    let mode = Self::pick(size, cfg.mode.as_deref());
                    if o.current_mode() != Some(mode) {
                        o.change_current_state(Some(mode), None, None, None);
                        hecate.update_output_buffers(&o);
                        smithay::desktop::layer_map_for_output(&o).arrange();
                    }
                }
                (None, false) => (),
            }
        }
        let outputs: Vec<Output> = hecate.wm.outputs().cloned().collect();
        for o in outputs {
            hecate.apply_output_props(&o);
        }
        hecate.queue_redraw_all();
    }

    pub fn render(&mut self, hecate: &mut Hecate, output: &Output) -> bool {
        if hecate.output_state.get(output).is_some_and(|s| s.powered_off) {
            return false;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return true;
        };
        let size = output
            .current_mode()
            .map(|m| m.size)
            .unwrap_or_else(|| Size::from((1, 1)));
        let name = output.name();
        let needs_new = self.targets.get(&name).is_none_or(|(tex, _)| {
            smithay::backend::renderer::Texture::size(tex)
                != size.to_logical(1).to_buffer(1, smithay::utils::Transform::Normal)
        });
        if needs_new {
            let buffer_size: Size<i32, Buffer> = (size.w, size.h).into();
            let tex: GlesTexture = match renderer.create_buffer(Fourcc::Abgr8888, buffer_size) {
                Ok(t) => t,
                Err(err) => {
                    tracing::warn!("error creating offscreen buffer: {err:?}");
                    return false;
                }
            };
            self.targets
                .insert(name.clone(), (tex, OutputDamageTracker::from_output(output)));
        }
        let elements = hecate.render_elements(renderer, output);
        let (tex, tracker) = self.targets.get_mut(&name).unwrap();
        let mut fb = match renderer.bind(tex) {
            Ok(fb) => fb,
            Err(err) => {
                tracing::warn!("error binding offscreen buffer: {err:?}");
                return false;
            }
        };
        let res = tracker.render_output(renderer, &mut fb, 0, &elements, [0., 0., 0., 1.]);
        if let Err(err) = res {
            tracing::warn!("headless render error: {err:?}");
            return false;
        }
        if self.capture {
            let region = Rectangle::from_size((size.w, size.h).into());
            match renderer.copy_framebuffer(&fb, region, Fourcc::Abgr8888) {
                Ok(mapping) => match renderer.map_texture(&mapping) {
                    Ok(bytes) => {
                        self.frames.insert(name, (bytes.to_vec(), size.w, size.h));
                    }
                    Err(err) => tracing::warn!("error mapping frame: {err:?}"),
                },
                Err(err) => tracing::warn!("error copying frame: {err:?}"),
            }
        }
        let _ = renderer.wait(&smithay::backend::renderer::sync::SyncPoint::signaled());
        true
    }

    pub fn import_dmabuf(&mut self, dmabuf: &Dmabuf) -> bool {
        self.renderer
            .as_mut()
            .is_some_and(|r| r.import_dmabuf(dmabuf, None).is_ok())
    }

    pub fn gamma_size(&self, _output: &Output) -> u32 {
        256
    }

    pub fn set_gamma(&mut self, output: &Output, ramp: Option<Vec<u16>>) -> anyhow::Result<()> {
        match ramp {
            Some(r) => {
                anyhow::ensure!(r.len() == 256 * 3, "wrong gamma length");
                self.gamma.insert(output.name(), r);
            }
            None => {
                self.gamma.remove(&output.name());
            }
        }
        Ok(())
    }

    pub fn renderer(&mut self) -> Option<&mut GlesRenderer> {
        self.renderer.as_mut()
    }
}
