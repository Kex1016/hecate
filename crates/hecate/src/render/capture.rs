//! Rendering into client buffers (screencopy).
//!
//! Adapted from niri (src/render_helpers/mod.rs), GPL-3.0-or-later.

use std::ptr;

use anyhow::{Context, bail, ensure};
use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::{RenderElement, RenderElementStates};
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::sync::SyncPoint;
use smithay::backend::renderer::{Bind, Color32F, ExportMem, Offscreen};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::utils::{Buffer, Rectangle, Size};
use smithay::wayland::shm;

pub fn render_to_dmabuf(
    renderer: &mut GlesRenderer,
    damage_tracker: &mut OutputDamageTracker,
    mut dmabuf: Dmabuf,
    elements: &[impl RenderElement<GlesRenderer>],
    states: RenderElementStates,
) -> anyhow::Result<SyncPoint> {
    use smithay::backend::allocator::Buffer as _;
    let (size, _scale, _transform) = damage_tracker.mode().try_into().unwrap();
    ensure!(
        dmabuf.width() == size.w as u32 && dmabuf.height() == size.h as u32,
        "invalid buffer size"
    );
    let mut target = renderer.bind(&mut dmabuf).context("error binding dmabuf")?;
    let res = damage_tracker
        .render_output_with_states(renderer, &mut target, 0, elements, Color32F::TRANSPARENT, states)
        .context("error rendering to dmabuf")?;
    Ok(res.sync)
}

pub fn render_to_shm(
    renderer: &mut GlesRenderer,
    damage_tracker: &mut OutputDamageTracker,
    buffer: &WlBuffer,
    format: wl_shm::Format,
    elements: &[impl RenderElement<GlesRenderer>],
    states: RenderElementStates,
) -> anyhow::Result<()> {
    shm::with_buffer_contents_mut(buffer, |pool, pool_len, buffer_data| {
        let (size, _scale, _transform) = damage_tracker.mode().try_into().unwrap();
        let fourcc = match format {
            wl_shm::Format::Xrgb8888 => Fourcc::Xrgb8888,
            wl_shm::Format::Argb8888 => Fourcc::Argb8888,
            _ => bail!("unsupported shm format: {format:?}"),
        };
        ensure!(
            buffer_data.format == format && buffer_data.width == size.w && buffer_data.height == size.h,
            "invalid buffer format or size"
        );

        // The client picks the stride and may pad rows; only row_len bytes per row are ours.
        let row_len = size.w as usize * 4;
        let height = size.h as usize;
        let offset = usize::try_from(buffer_data.offset).context("negative buffer offset")?;
        let stride = usize::try_from(buffer_data.stride).context("negative buffer stride")?;
        let end = stride
            .checked_mul(height.saturating_sub(1))
            .and_then(|len| len.checked_add(row_len))
            .and_then(|len| len.checked_add(offset));
        ensure!(
            stride >= row_len && end.is_some_and(|end| end <= pool_len),
            "buffer does not fit in its shm pool"
        );

        let buffer_size: Size<i32, Buffer> = (size.w, size.h).into();
        let mut texture: GlesTexture = renderer
            .create_buffer(fourcc, buffer_size)
            .context("error creating texture")?;
        let target = {
            let mut target = renderer.bind(&mut texture).context("error binding texture")?;
            damage_tracker
                .render_output_with_states(renderer, &mut target, 0, elements, Color32F::TRANSPARENT, states)
                .context("error rendering")?;
            renderer
                .copy_framebuffer(&target, Rectangle::from_size(buffer_size), fourcc)
                .context("error copying framebuffer")?
        };
        let bytes = renderer.map_texture(&target).context("error mapping texture")?;
        ensure!(bytes.len() >= row_len * height, "short texture mapping");

        // SAFETY: bounds checked above against the pool length.
        unsafe {
            let dst = pool.add(offset);
            if stride == row_len {
                ptr::copy_nonoverlapping(bytes.as_ptr(), dst, row_len * height);
            } else {
                for y in 0..height {
                    ptr::copy_nonoverlapping(bytes.as_ptr().add(y * row_len), dst.add(y * stride), row_len);
                }
            }
        }
        Ok(())
    })
    .context("expected an shm buffer")?
}
