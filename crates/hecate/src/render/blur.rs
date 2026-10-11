//! Background blur behind layer surfaces: dual Kawase over what's below the surface (or, with
//! `xray`, only the background and bottom layers), cached until that changes.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};

use anyhow::Context as _;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::{Element, Id, Kind, RenderElement, UnderlyingStorage};
use smithay::backend::renderer::gles::{
    GlesError, GlesFrame, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformName, UniformType, ffi,
};
use smithay::backend::renderer::utils::{CommitCounter, DamageSet, OpaqueRegions};
use smithay::backend::renderer::{Bind, Color32F, Frame, Offscreen, Renderer, Texture};
use smithay::utils::user_data::UserDataMap;
use smithay::utils::{Buffer, Physical, Rectangle, Scale, Size, Transform};

use crate::config::Blur;

const HEADER: &str = r#"
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
"#;

const DOWN: &str = r#"
uniform vec2 halfpixel;
uniform float offset;
void main() {
    vec2 uv = v_coords;
    vec4 sum = texture2D(tex, uv) * 4.0;
    sum += texture2D(tex, uv - halfpixel * offset);
    sum += texture2D(tex, uv + halfpixel * offset);
    sum += texture2D(tex, uv + vec2(halfpixel.x, -halfpixel.y) * offset);
    sum += texture2D(tex, uv - vec2(halfpixel.x, -halfpixel.y) * offset);
    gl_FragColor = sum / 8.0;
}
"#;

const UP: &str = r#"
uniform vec2 halfpixel;
uniform float offset;
void main() {
    vec2 uv = v_coords;
    vec2 h = halfpixel * offset;
    vec4 sum = texture2D(tex, uv + vec2(-h.x * 2.0, 0.0));
    sum += texture2D(tex, uv + vec2(-h.x, h.y)) * 2.0;
    sum += texture2D(tex, uv + vec2(0.0, h.y * 2.0));
    sum += texture2D(tex, uv + vec2(h.x, h.y)) * 2.0;
    sum += texture2D(tex, uv + vec2(h.x * 2.0, 0.0));
    sum += texture2D(tex, uv + vec2(h.x, -h.y)) * 2.0;
    sum += texture2D(tex, uv + vec2(0.0, -h.y * 2.0));
    sum += texture2D(tex, uv + vec2(-h.x, -h.y)) * 2.0;
    gl_FragColor = sum / 12.0;
}
"#;

/// Blurred texture, cropped by the vertex shader, with noise against banding.
const FINISH: &str = r#"
uniform float noise;
float hecate_hash(vec2 p) {
    return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}
void main() {
    vec4 color = texture2D(tex, v_coords);
    color.rgb += (hecate_hash(gl_FragCoord.xy) - 0.5) * noise;
    gl_FragColor = vec4(color.rgb, 1.0) * alpha;
}
"#;

/// `tex` is the layer surface's buffer: blur shows only where it's more opaque than
/// `threshold`. The blurred texture is on unit 1, `blur_rect` maps the buffer onto it.
const MASK: &str = r#"
uniform sampler2D blurred;
uniform vec4 blur_rect;
uniform float threshold;
uniform float noise;
float hecate_hash(vec2 p) {
    return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}
void main() {
    float a = texture2D(tex, v_coords).a;
#if defined(NO_ALPHA)
    a = 1.0;
#endif
    if (a <= threshold) {
        gl_FragColor = vec4(0.0);
        return;
    }
    vec4 color = texture2D(blurred, blur_rect.xy + v_coords * blur_rect.zw);
    color.rgb += (hecate_hash(gl_FragCoord.xy) - 0.5) * noise;
    gl_FragColor = vec4(color.rgb, 1.0) * alpha;
}
"#;

pub struct BlurShaders {
    down: GlesTexProgram,
    up: GlesTexProgram,
    finish: GlesTexProgram,
    mask: GlesTexProgram,
}

impl BlurShaders {
    pub fn init(renderer: &mut GlesRenderer) {
        let compile = |r: &mut GlesRenderer, body: &str, uniforms: &[UniformName<'_>]| {
            r.compile_custom_texture_shader(format!("{HEADER}\n{body}"), uniforms)
                .map_err(|e| tracing::warn!("error compiling blur shader: {e:?}"))
                .ok()
        };
        let pass = [
            UniformName::new("halfpixel", UniformType::_2f),
            UniformName::new("offset", UniformType::_1f),
        ];
        let shaders = (|| {
            Some(BlurShaders {
                down: compile(renderer, DOWN, &pass)?,
                up: compile(renderer, UP, &pass)?,
                finish: compile(renderer, FINISH, &[UniformName::new("noise", UniformType::_1f)])?,
                mask: compile(
                    renderer,
                    MASK,
                    &[
                        UniformName::new("blurred", UniformType::_1i),
                        UniformName::new("blur_rect", UniformType::_4f),
                        UniformName::new("threshold", UniformType::_1f),
                        UniformName::new("noise", UniformType::_1f),
                    ],
                )?,
            })
        })();
        let data = renderer.egl_context().user_data();
        data.insert_if_missing(|| RefCell::new(None::<BlurShaders>));
        *data.get::<RefCell<Option<BlurShaders>>>().unwrap().borrow_mut() = shaders;
    }

    /// The programs drawing a blur element: plain and masked.
    pub fn composite(renderer: &GlesRenderer) -> Option<(GlesTexProgram, GlesTexProgram)> {
        Self::get(renderer, |s| (s.finish.clone(), s.mask.clone()))
    }

    fn get<T>(renderer: &GlesRenderer, f: impl FnOnce(&BlurShaders) -> T) -> Option<T> {
        let cell = renderer
            .egl_context()
            .user_data()
            .get::<RefCell<Option<BlurShaders>>>()?;
        cell.borrow().as_ref().map(f)
    }
}

/// Render elements into a new texture of `size`, opaque black where nothing is drawn.
fn render_scene(
    renderer: &mut GlesRenderer,
    size: Size<i32, Physical>,
    scale: Scale<f64>,
    elements: &[impl RenderElement<GlesRenderer>],
) -> anyhow::Result<GlesTexture> {
    let mut texture: GlesTexture = renderer
        .create_buffer(Fourcc::Abgr8888, (size.w, size.h).into())
        .context("error creating texture")?;
    {
        let mut target = renderer.bind(&mut texture).context("error binding texture")?;
        let mut tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
        tracker
            .render_output(renderer, &mut target, 0, elements, Color32F::BLACK)
            .context("error rendering the scene to blur")?;
    }
    Ok(texture)
}

/// One Kawase pass from `src` into a new texture of `size`.
fn pass(
    renderer: &mut GlesRenderer,
    src: &GlesTexture,
    size: Size<i32, Buffer>,
    program: &GlesTexProgram,
    offset: f32,
) -> anyhow::Result<GlesTexture> {
    let mut out: GlesTexture = renderer
        .create_buffer(Fourcc::Abgr8888, size)
        .context("error creating texture")?;
    let src_size = src.size();
    let halfpixel = (0.5 / src_size.w as f32, 0.5 / src_size.h as f32);
    {
        let mut target = renderer.bind(&mut out).context("error binding texture")?;
        let phys: Size<i32, Physical> = (size.w, size.h).into();
        let mut frame = renderer
            .render(&mut target, phys, Transform::Normal)
            .context("error starting a blur pass")?;
        let dst = Rectangle::from_size(phys);
        frame
            .render_texture_from_to(
                src,
                Rectangle::from_size(src_size.to_f64()),
                dst,
                &[dst],
                &[],
                Transform::Normal,
                1.0,
                Some(program),
                &[
                    Uniform::new("halfpixel", halfpixel),
                    Uniform::new("offset", offset),
                ],
            )
            .context("error in a blur pass")?;
        let _ = frame.finish().context("error finishing a blur pass")?;
    }
    Ok(out)
}

/// Dual Kawase: halve the size `passes` times, then double back up.
fn blur(renderer: &mut GlesRenderer, scene: GlesTexture, config: &Blur) -> anyhow::Result<GlesTexture> {
    let (down, up) =
        BlurShaders::get(renderer, |s| (s.down.clone(), s.up.clone())).context("no blur shaders")?;
    let full = scene.size();
    let passes = config.passes.clamp(1, 6);
    let offset = config.radius.max(0.);
    let mut sizes = vec![full];
    let mut tex = scene;
    for _ in 0..passes {
        let last = *sizes.last().unwrap();
        let next: Size<i32, Buffer> = ((last.w / 2).max(1), (last.h / 2).max(1)).into();
        tex = pass(renderer, &tex, next, &down, offset)?;
        sizes.push(next);
    }
    sizes.pop();
    while let Some(size) = sizes.pop() {
        tex = pass(renderer, &tex, size, &up, offset)?;
    }
    Ok(tex)
}

/// Blurred scenes by what they were made from, kept per output while in use.
#[derive(Default)]
pub struct BlurCache {
    entries: Vec<CacheEntry>,
}

struct CacheEntry {
    key: u64,
    texture: GlesTexture,
    commit: CommitCounter,
    used: bool,
}

/// What a blurred scene depends on: every element below, where it is and its contents.
pub fn scene_key<E: Element>(
    elements: &[E],
    scale: Scale<f64>,
    size: Size<i32, Physical>,
    config: &Blur,
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    size.w.hash(&mut h);
    size.h.hash(&mut h);
    config.passes.hash(&mut h);
    config.radius.to_bits().hash(&mut h);
    for e in elements {
        e.id().hash(&mut h);
        format!("{:?}", e.current_commit()).hash(&mut h);
        let g = e.geometry(scale);
        (g.loc.x, g.loc.y, g.size.w, g.size.h).hash(&mut h);
        e.alpha().to_bits().hash(&mut h);
    }
    h.finish()
}

impl BlurCache {
    /// The blurred scene for `key`, rendering and blurring `elements` if it isn't cached.
    pub fn get(
        &mut self,
        renderer: &mut GlesRenderer,
        key: u64,
        size: Size<i32, Physical>,
        scale: Scale<f64>,
        elements: &[impl RenderElement<GlesRenderer>],
        config: &Blur,
    ) -> Option<(GlesTexture, CommitCounter)> {
        if let Some(e) = self.entries.iter_mut().find(|e| e.key == key) {
            e.used = true;
            return Some((e.texture.clone(), e.commit));
        }
        let result =
            render_scene(renderer, size, scale, elements).and_then(|scene| blur(renderer, scene, config));
        let texture = match result {
            Ok(t) => t,
            Err(err) => {
                tracing::warn!("error blurring: {err:?}");
                return None;
            }
        };
        // Commit counters only need to differ between textures.
        let commit = CommitCounter::from(key as usize);
        self.entries.push(CacheEntry {
            key,
            texture: texture.clone(),
            commit,
            used: true,
        });
        Some((texture, commit))
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Drop entries no frame used since the last call.
    pub fn end_frame(&mut self) {
        self.entries.retain(|e| e.used);
        for e in &mut self.entries {
            e.used = false;
        }
    }
}

/// Blur behind one layer surface.
#[derive(Debug)]
pub struct BlurElement {
    id: Id,
    commit: CommitCounter,
    blurred: GlesTexture,
    /// Where the element is, in output physical coordinates (also its spot in `blurred`).
    geometry: Rectangle<i32, Physical>,
    /// The surface's buffer, to blur only where it's opaque enough.
    mask: Option<(GlesTexture, f32)>,
    noise: f32,
    alpha: f32,
    programs: (GlesTexProgram, GlesTexProgram),
}

impl BlurElement {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        programs: (GlesTexProgram, GlesTexProgram),
        id: Id,
        commit: CommitCounter,
        blurred: GlesTexture,
        geometry: Rectangle<i32, Physical>,
        mask: Option<(GlesTexture, f32)>,
        noise: f32,
        alpha: f32,
    ) -> Self {
        BlurElement {
            id,
            commit,
            blurred,
            geometry,
            mask,
            noise,
            alpha,
            programs,
        }
    }
}

impl Element for BlurElement {
    fn id(&self) -> &Id {
        &self.id
    }

    fn current_commit(&self) -> CommitCounter {
        self.commit
    }

    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.geometry
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        Rectangle::new(
            (self.geometry.loc.x as f64, self.geometry.loc.y as f64).into(),
            (self.geometry.size.w as f64, self.geometry.size.h as f64).into(),
        )
    }

    fn damage_since(&self, scale: Scale<f64>, commit: Option<CommitCounter>) -> DamageSet<i32, Physical> {
        if commit == Some(self.commit) {
            DamageSet::default()
        } else {
            DamageSet::from_slice(&[Rectangle::from_size(self.geometry(scale).size)])
        }
    }

    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        if self.mask.is_none() && self.alpha >= 1.0 {
            OpaqueRegions::from_slice(&[Rectangle::from_size(self.geometry.size)])
        } else {
            OpaqueRegions::default()
        }
    }

    fn alpha(&self) -> f32 {
        self.alpha
    }

    fn kind(&self) -> Kind {
        Kind::Unspecified
    }
}

impl RenderElement<GlesRenderer> for BlurElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        _src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        _opaque_regions: &[Rectangle<i32, Physical>],
        _cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        let (finish, mask_program) = &self.programs;
        let size = self.blurred.size();
        match &self.mask {
            None => frame.render_texture_from_to(
                &self.blurred,
                self.src(),
                dst,
                damage,
                &[],
                Transform::Normal,
                self.alpha,
                Some(finish),
                &[Uniform::new("noise", self.noise)],
            ),
            Some((mask, threshold)) => {
                let blurred = self.blurred.tex_id();
                frame.with_context(|gl| unsafe {
                    gl.ActiveTexture(ffi::TEXTURE1);
                    gl.BindTexture(ffi::TEXTURE_2D, blurred);
                    gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_MIN_FILTER, ffi::LINEAR as i32);
                    gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_MAG_FILTER, ffi::LINEAR as i32);
                    gl.ActiveTexture(ffi::TEXTURE0);
                })?;
                let g = self.geometry;
                let blur_rect = [
                    g.loc.x as f32 / size.w as f32,
                    g.loc.y as f32 / size.h as f32,
                    g.size.w as f32 / size.w as f32,
                    g.size.h as f32 / size.h as f32,
                ];
                let res = frame.render_texture_from_to(
                    mask,
                    Rectangle::from_size(mask.size().to_f64()),
                    dst,
                    damage,
                    &[],
                    Transform::Normal,
                    self.alpha,
                    Some(mask_program),
                    &[
                        Uniform::new("blurred", 1i32),
                        Uniform::new("blur_rect", blur_rect),
                        Uniform::new("threshold", *threshold),
                        Uniform::new("noise", self.noise),
                    ],
                );
                frame.with_context(|gl| unsafe {
                    gl.ActiveTexture(ffi::TEXTURE1);
                    gl.BindTexture(ffi::TEXTURE_2D, 0);
                    gl.ActiveTexture(ffi::TEXTURE0);
                })?;
                res
            }
        }
    }

    fn underlying_storage(&self, _renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        None
    }
}
