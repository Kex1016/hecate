//! Pixelate: something drawn as coarse blocks, each the average of what it covers. An
//! animation shrinks the blocks until the picture is sharp (or grows them until it is cut).
//!
//! The blocks are cut out of one texture, so what pixelates is drawn into one first: a
//! window with its frame, a layer surface, a close snapshot or the splash.

use std::cell::{Cell, RefCell};

use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::element::{Element, Id, Kind, RenderElement, UnderlyingStorage};
use smithay::backend::renderer::gles::{
    GlesError, GlesFrame, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformName, UniformType,
};
use smithay::backend::renderer::utils::{CommitCounter, DamageSet, OpaqueRegions};
use smithay::backend::renderer::{ContextId, Renderer, Texture};
use smithay::utils::user_data::UserDataMap;
use smithay::utils::{Buffer, Logical, Physical, Point, Rectangle, Scale, Size, Transform};

use super::border::Shaders;

/// Sixteen taps spread over the block. With linear filtering each takes in four texels, so
/// a 16 px block is averaged from half of what it covers and smaller ones from all of it.
/// The taps stay half a texel inside their block, so no block takes in its neighbours.
const PIXELATE_FRAG: &str = r#"
//_DEFINES_
#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif
precision highp float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif
uniform float alpha;
varying vec2 v_coords;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

// One block and one texel, in texture coordinates.
uniform vec2 pix_cell;
uniform vec2 pix_texel;

void main() {
    // Blocks are counted from the middle: both edges get the same partial block, whichever
    // way up the texture is, and a block of half the size splits each one in four.
    vec2 corner = floor((v_coords - 0.5) / pix_cell) * pix_cell + 0.5;
    // The part of the block that is on the texture, and where its taps may fall.
    vec2 from = max(corner, vec2(0.0));
    vec2 to = min(corner + pix_cell, vec2(1.0));
    vec2 low = from + 0.5 * pix_texel;
    vec2 high = max(to - 0.5 * pix_texel, low);
    vec4 color = vec4(0.0);
    for (int i = 0; i < 4; i++) {
        for (int j = 0; j < 4; j++) {
            vec2 at = mix(from, to, (vec2(float(i), float(j)) + 0.5) * 0.25);
            color += texture2D(tex, clamp(at, low, high));
        }
    }
    color *= 0.0625;
#if defined(NO_ALPHA)
    color = vec4(color.rgb, 1.0);
#endif
    color = color * alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    gl_FragColor = color;
}
"#;

pub(super) fn compile(renderer: &mut GlesRenderer) -> Option<GlesTexProgram> {
    renderer
        .compile_custom_texture_shader(
            PIXELATE_FRAG,
            &[
                UniformName::new("pix_cell", UniformType::_2f),
                UniformName::new("pix_texel", UniformType::_2f),
            ],
        )
        .map_err(|e| tracing::warn!("error compiling pixelate shader: {e:?}"))
        .ok()
}

/// Block size in pixels for an animation's block size in logical pixels: whole pixels, and
/// 1 (sharp) rather than something a hair coarser.
pub fn cell_px(logical: f64, scale: Scale<f64>) -> i32 {
    (logical * scale.x).round().max(1.) as i32
}

/// A texture element drawn in blocks of `cell` texture pixels (as it is, when 1).
#[derive(Debug)]
pub struct PixelateElement<E> {
    inner: E,
    id: Id,
    commit: CommitCounter,
    program: Option<GlesTexProgram>,
    cell: i32,
    texture_size: Size<i32, Buffer>,
}

impl<E> PixelateElement<E> {
    /// `id` and `commit` are this element's own, not `inner`'s: the picture changes with
    /// the block size, which `inner` knows nothing of. Give every block size (and every new
    /// texture) another `commit`.
    pub fn new(
        renderer: &GlesRenderer,
        inner: E,
        id: Id,
        commit: CommitCounter,
        cell: i32,
        texture_size: Size<i32, Buffer>,
    ) -> Self {
        PixelateElement {
            inner,
            id,
            commit,
            program: Shaders::pixelate(renderer),
            cell,
            texture_size,
        }
    }
}

impl<E: Element> Element for PixelateElement<E> {
    fn id(&self) -> &Id {
        &self.id
    }

    fn current_commit(&self) -> CommitCounter {
        self.commit
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.inner.geometry(scale)
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        self.inner.src()
    }

    fn transform(&self) -> Transform {
        self.inner.transform()
    }

    fn damage_since(&self, scale: Scale<f64>, commit: Option<CommitCounter>) -> DamageSet<i32, Physical> {
        if commit == Some(self.commit) {
            DamageSet::default()
        } else {
            DamageSet::from_slice(&[Rectangle::from_size(self.geometry(scale).size)])
        }
    }

    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        OpaqueRegions::default()
    }

    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }

    fn kind(&self) -> Kind {
        self.inner.kind()
    }
}

impl<E: RenderElement<GlesRenderer>> RenderElement<GlesRenderer> for PixelateElement<E> {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        let (w, h) = (self.texture_size.w as f32, self.texture_size.h as f32);
        let program = match &self.program {
            Some(program) if self.cell > 1 && w > 0. && h > 0. => program.clone(),
            _ => return self.inner.draw(frame, src, dst, damage, opaque_regions, cache),
        };
        let cell = self.cell as f32;
        frame.override_default_tex_program(
            program,
            vec![
                Uniform::new("pix_cell", (cell / w, cell / h)),
                Uniform::new("pix_texel", (1. / w, 1. / h)),
            ],
        );
        let drawn = self.inner.draw(frame, src, dst, damage, opaque_regions, cache);
        frame.clear_tex_program_override();
        drawn
    }

    fn underlying_storage(&self, _renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        None
    }
}

/// A pixelated texture on screen.
pub type PixelatedTexture = PixelateElement<TextureRenderElement<GlesTexture>>;

/// `texture` (physical pixels, one to one) over `size` at `loc`, in blocks of `cell` pixels.
#[allow(clippy::too_many_arguments)]
pub fn texture_element(
    renderer: &GlesRenderer,
    id: Id,
    commit: CommitCounter,
    context: ContextId<GlesTexture>,
    texture: GlesTexture,
    loc: Point<i32, Physical>,
    size: Size<i32, Logical>,
    alpha: f32,
    cell: i32,
) -> PixelatedTexture {
    let texture_size = texture.size();
    let src = Rectangle::from_size((texture_size.w as f64, texture_size.h as f64).into());
    let inner = TextureRenderElement::from_static_texture(
        // Never seen by the damage tracker, which goes by the wrapper's id.
        id.clone(),
        context,
        loc.to_f64(),
        texture,
        1,
        Transform::Normal,
        Some(alpha),
        Some(src),
        Some(size),
        None,
        Kind::Unspecified,
    );
    PixelateElement::new(renderer, inner, id, commit, cell, texture_size)
}

/// What a snapshot was drawn for: the block size, its size in pixels and the scale.
type Key = (i32, Size<i32, Physical>, u64);

/// The picture an opening window or layer surface pixelates from, in its user data. Drawn
/// again each time the block size changes, so the picture keeps up with the client in
/// steps, like everything else about the animation.
pub struct PixelCache {
    id: Id,
    version: Cell<usize>,
    snapshot: RefCell<Option<(Key, GlesTexture, ContextId<GlesTexture>)>>,
}

impl Default for PixelCache {
    fn default() -> Self {
        PixelCache {
            id: Id::new(),
            version: Cell::new(0),
            snapshot: RefCell::new(None),
        }
    }
}

impl PixelCache {
    /// This frame's element: `size` at `loc` in blocks of `cell` pixels, cut from the kept
    /// picture, or from a new one that `draw` renders at `size`.
    #[allow(clippy::too_many_arguments)]
    pub fn element(
        &self,
        renderer: &mut GlesRenderer,
        cell: i32,
        loc: Point<i32, Physical>,
        size: Size<i32, Logical>,
        scale: Scale<f64>,
        alpha: f32,
        draw: impl FnOnce(&mut GlesRenderer, Size<i32, Physical>) -> anyhow::Result<GlesTexture>,
    ) -> Option<PixelatedTexture> {
        let pixels: Size<i32, Physical> = size.to_f64().to_physical(scale).to_i32_round();
        if pixels.w <= 0 || pixels.h <= 0 {
            return None;
        }
        let key = (cell, pixels, scale.x.to_bits());
        let mut snapshot = self.snapshot.borrow_mut();
        if snapshot.as_ref().is_none_or(|(k, ..)| *k != key) {
            let texture = draw(renderer, pixels)
                .map_err(|e| tracing::warn!("error drawing a pixelate snapshot: {e:#}"))
                .ok()?;
            self.version.set(self.version.get().wrapping_add(1));
            *snapshot = Some((key, texture, renderer.context_id()));
        }
        let (_, texture, context) = snapshot.as_ref()?;
        Some(texture_element(
            renderer,
            self.id.clone(),
            CommitCounter::from(self.version.get()),
            context.clone(),
            texture.clone(),
            loc,
            size,
            alpha,
            cell,
        ))
    }

    /// The animation is over: let the texture go.
    pub fn clear(&self) {
        self.snapshot.borrow_mut().take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_sizes_in_pixels() {
        let one = Scale::from(1.0);
        assert_eq!(cell_px(16., one), 16);
        assert_eq!(cell_px(1.4, one), 1, "all but sharp is sharp");
        assert_eq!(cell_px(0., one), 1);
        assert_eq!(cell_px(16., Scale::from(1.5)), 24);
        assert_eq!(cell_px(2., Scale::from(2.0)), 4);
    }
}
