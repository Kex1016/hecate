//! Window borders with per-corner radii, and rounded clipping of window contents.
//!
//! The clipping element is adapted from niri's `ClippedSurfaceRenderElement`
//! (https://github.com/YaLTeR/niri, GPL-3.0-or-later). The border is a pixel shader that
//! draws the ring between an outer rounded rectangle and an inner one.

use std::cell::RefCell;

use glam::{Mat3, Vec2};
use smithay::backend::renderer::buffer_y_inverted;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::{Element, Id, Kind, RenderElement, UnderlyingStorage};
use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::backend::renderer::gles::{
    GlesError, GlesFrame, GlesPixelProgram, GlesRenderer, GlesTexProgram, Uniform, UniformName, UniformType,
    UniformValue,
};
use smithay::backend::renderer::utils::{CommitCounter, DamageSet, OpaqueRegions};
use smithay::desktop::{PopupManager, Window};
use smithay::utils::user_data::UserDataMap;
use smithay::utils::{Buffer, Logical, Physical, Point, Rectangle, Scale, Size, Transform};

use super::HecateElement;
use crate::config::{Color, Config, Corners};
use crate::wm::{Maximized, WindowExt};

pub type BorderElement = PixelShaderElement;

const ROUNDING_ALPHA: &str = r#"
float hecate_rounding_alpha(vec2 coords, vec2 size, vec4 r, float scale) {
    vec2 center;
    float radius;
    if (coords.x < r.x && coords.y < r.x) {
        radius = r.x; center = vec2(radius, radius);
    } else if (size.x - r.y < coords.x && coords.y < r.y) {
        radius = r.y; center = vec2(size.x - radius, radius);
    } else if (size.x - r.z < coords.x && size.y - r.z < coords.y) {
        radius = r.z; center = vec2(size.x - radius, size.y - radius);
    } else if (coords.x < r.w && size.y - r.w < coords.y) {
        radius = r.w; center = vec2(radius, size.y - radius);
    } else {
        return 1.0;
    }
    float dist = distance(coords, center);
    float t = clamp((dist - radius) * scale + 0.5, 0.0, 1.0);
    return 1.0 - t * t * (3.0 - 2.0 * t);
}
"#;

const BORDER_FRAG: &str = r#"
precision highp float;
varying vec2 v_coords;
uniform vec2 size;
uniform float alpha;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

uniform vec4 color;
uniform vec4 outer_radius;
uniform vec4 inner_radius;
uniform float border_width;
uniform float hecate_scale;

float hecate_rounding_alpha(vec2 coords, vec2 size, vec4 r, float scale);

void main() {
    vec2 coords = v_coords * size;
    float outer = hecate_rounding_alpha(coords, size, outer_radius, hecate_scale);

    vec2 inner_size = size - vec2(2.0 * border_width);
    vec2 inner_coords = coords - vec2(border_width);
    float inner = 0.0;
    if (inner_coords.x >= 0.0 && inner_coords.y >= 0.0
        && inner_coords.x <= inner_size.x && inner_coords.y <= inner_size.y) {
        inner = hecate_rounding_alpha(inner_coords, inner_size, inner_radius, hecate_scale);
    }

    gl_FragColor = color * (outer * (1.0 - inner)) * alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        gl_FragColor = vec4(0.0, 0.3, 0.0, 0.2) + gl_FragColor * 0.8;
#endif
}
"#;

const CLIP_FRAG: &str = r#"
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

uniform float hecate_scale;
uniform vec2 geo_size;
uniform vec4 corner_radius;
uniform mat3 input_to_geo;

float hecate_rounding_alpha(vec2 coords, vec2 size, vec4 r, float scale);

void main() {
    vec3 coords_geo = input_to_geo * vec3(v_coords, 1.0);
    vec4 color = texture2D(tex, v_coords);
#if defined(NO_ALPHA)
    color = vec4(color.rgb, 1.0);
#endif
    if (coords_geo.x < 0.0 || 1.0 < coords_geo.x || coords_geo.y < 0.0 || 1.0 < coords_geo.y) {
        color = vec4(0.0);
    } else {
        color = color * hecate_rounding_alpha(coords_geo.xy * geo_size, geo_size, corner_radius, hecate_scale);
    }
    color = color * alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    gl_FragColor = color;
}
"#;

pub struct Shaders {
    pub border: Option<GlesPixelProgram>,
    pub clip: Option<GlesTexProgram>,
}

impl Shaders {
    /// Compile the shaders once per renderer.
    pub fn init(renderer: &mut GlesRenderer) {
        let border = renderer
            .compile_custom_pixel_shader(
                format!("{BORDER_FRAG}\n{ROUNDING_ALPHA}"),
                &[
                    UniformName::new("color", UniformType::_4f),
                    UniformName::new("outer_radius", UniformType::_4f),
                    UniformName::new("inner_radius", UniformType::_4f),
                    UniformName::new("border_width", UniformType::_1f),
                    UniformName::new("hecate_scale", UniformType::_1f),
                ],
            )
            .map_err(|e| tracing::warn!("error compiling border shader: {e:?}"))
            .ok();
        let clip = renderer
            .compile_custom_texture_shader(
                format!("{CLIP_FRAG}\n{ROUNDING_ALPHA}"),
                &[
                    UniformName::new("hecate_scale", UniformType::_1f),
                    UniformName::new("geo_size", UniformType::_2f),
                    UniformName::new("corner_radius", UniformType::_4f),
                    UniformName::new("input_to_geo", UniformType::Matrix3x3),
                ],
            )
            .map_err(|e| tracing::warn!("error compiling clip shader: {e:?}"))
            .ok();
        let data = renderer.egl_context().user_data();
        data.insert_if_missing(|| {
            RefCell::new(Shaders {
                border: None,
                clip: None,
            })
        });
        let cell = data.get::<RefCell<Shaders>>().unwrap();
        *cell.borrow_mut() = Shaders { border, clip };
    }

    pub fn border(renderer: &GlesRenderer) -> Option<GlesPixelProgram> {
        renderer
            .egl_context()
            .user_data()
            .get::<RefCell<Shaders>>()
            .and_then(|s| s.borrow().border.clone())
    }

    pub fn clip(renderer: &GlesRenderer) -> Option<GlesTexProgram> {
        renderer
            .egl_context()
            .user_data()
            .get::<RefCell<Shaders>>()
            .and_then(|s| s.borrow().clip.clone())
    }
}

/// Resolved border look for one window.
#[derive(Debug, Clone, PartialEq)]
pub struct BorderStyle {
    pub width: f32,
    pub radius: Corners,
    pub color: Color,
    pub clip: bool,
    pub draw_border: bool,
}

pub fn style_for(config: &Config, window: &Window, focused: bool) -> BorderStyle {
    style_for_inner(config, window, focused, false)
}

/// `identify`: thick urgent-coloured borders on every window for a moment.
pub fn style_for_identify(config: &Config, window: &Window, focused: bool) -> BorderStyle {
    style_for_inner(config, window, focused, true)
}

fn style_for_inner(config: &Config, window: &Window, focused: bool, identify: bool) -> BorderStyle {
    let d = window.data();
    let o = d.rules.border.clone().unwrap_or_default();
    let width = o.width.unwrap_or(config.border.width).max(0.);
    let radius = o.radius.unwrap_or(config.border.radius).corners();
    let colors = config.border.color;
    let oc = o.color.unwrap_or_default();
    let color = if d.urgent {
        oc.urgent.unwrap_or(colors.urgent)
    } else if focused {
        oc.active.unwrap_or(colors.active)
    } else {
        oc.inactive.unwrap_or(colors.inactive)
    };
    let clip = o.clip_content.unwrap_or(config.border.clip_content);
    let maximized_hides = d.maximized != Maximized::No && !config.border.keep_on_maximize;
    let fullscreen = d.fullscreen;
    let draw_border = width > 0. && !d.border_hidden && !maximized_hides && !fullscreen;
    if identify {
        return BorderStyle {
            width: width.max(6.),
            radius,
            color: colors.urgent,
            clip,
            draw_border: true,
        };
    }
    BorderStyle {
        width: if draw_border { width } else { 0. },
        // Fullscreen and maximized windows get square corners.
        radius: if fullscreen || maximized_hides {
            Corners::default()
        } else {
            radius
        },
        color,
        clip: clip && !fullscreen,
        draw_border,
    }
}

/// Per-window persistent border element (so its damage tracking stays stable).
struct BorderSlot(RefCell<Option<PixelShaderElement>>);

/// Push a window's elements (popups, clipped content, border), topmost first.
#[allow(clippy::too_many_arguments)]
pub fn push_window(
    renderer: &mut GlesRenderer,
    out: &mut Vec<HecateElement>,
    window: &Window,
    geo: Rectangle<i32, Logical>,
    output_loc: Point<i32, Logical>,
    scale: Scale<f64>,
    alpha: f32,
    style: &BorderStyle,
) {
    let toplevel = window.toplevel_surface();
    let surface = toplevel.wl_surface();
    let render_loc = geo.loc - window.geometry().loc;
    let local_geo = Rectangle::new(geo.loc - output_loc, geo.size);

    // Popups are never clipped and go on top.
    for (popup, popup_offset) in PopupManager::popups_for_surface(surface) {
        let loc = render_loc + window.geometry().loc + popup_offset - popup.geometry().loc - output_loc;
        let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = render_elements_from_surface_tree(
            renderer,
            popup.wl_surface(),
            loc.to_physical_precise_round(scale),
            scale,
            alpha,
            Kind::Unspecified,
        );
        out.extend(elems.into_iter().map(HecateElement::Surface));
    }

    let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = render_elements_from_surface_tree(
        renderer,
        surface,
        (render_loc - output_loc).to_physical_precise_round(scale),
        scale,
        alpha,
        Kind::ScanoutCandidate,
    );
    let inner_radius = style.radius.shrink(0.);
    let clip_program = if style.clip && !inner_radius.is_zero() {
        Shaders::clip(renderer)
    } else {
        None
    };
    for e in elems {
        match &clip_program {
            Some(program)
                if ClippedSurfaceElement::will_clip(&e, scale, local_geo.to_f64(), inner_radius) =>
            {
                out.push(HecateElement::Clipped(ClippedSurfaceElement::new(
                    e,
                    scale,
                    local_geo.to_f64(),
                    program.clone(),
                    inner_radius,
                )));
            }
            _ => out.push(HecateElement::Surface(e)),
        }
    }

    if style.draw_border
        && let Some(program) = Shaders::border(renderer)
    {
        let w = style.width;
        let wi = w.ceil() as i32;
        let area = Rectangle::new(
            (local_geo.loc.x - wi, local_geo.loc.y - wi).into(),
            (local_geo.size.w + 2 * wi, local_geo.size.h + 2 * wi).into(),
        );
        // Outer radii grow by the border width so the inner edge follows the window's corners.
        let outer = Corners {
            top_left: style.radius.top_left + w,
            top_right: style.radius.top_right + w,
            bottom_right: style.radius.bottom_right + w,
            bottom_left: style.radius.bottom_left + w,
        };
        let outer = if style.radius.is_zero() {
            Corners::default()
        } else {
            outer
        };
        let uniforms = vec![
            Uniform::new("color", style.color.premultiplied()),
            Uniform::new("outer_radius", outer.as_array()),
            Uniform::new("inner_radius", style.radius.as_array()),
            Uniform::new("border_width", wi as f32),
            Uniform::new("hecate_scale", scale.x as f32),
        ];
        let slot = window
            .user_data()
            .get_or_insert(|| BorderSlot(RefCell::new(None)));
        let mut slot = slot.0.borrow_mut();
        let elem = match slot.as_mut() {
            Some(e) => {
                e.resize(area, None);
                e.update_uniforms(uniforms);
                e.clone()
            }
            None => {
                let e = PixelShaderElement::new(program, area, None, alpha, uniforms, Kind::Unspecified);
                *slot = Some(e.clone());
                e
            }
        };
        out.push(HecateElement::Border(elem));
    }
}

fn mat3_uniform(name: &str, mat: Mat3) -> Uniform<'_> {
    Uniform::new(
        name,
        UniformValue::Matrix3x3 {
            matrices: vec![mat.to_cols_array()],
            transpose: false,
        },
    )
}

#[derive(Debug)]
pub struct ClippedSurfaceElement {
    inner: WaylandSurfaceRenderElement<GlesRenderer>,
    program: GlesTexProgram,
    corner_radius: Corners,
    geometry: Rectangle<f64, Logical>,
    scale: f32,
}

impl ClippedSurfaceElement {
    pub fn new(
        inner: WaylandSurfaceRenderElement<GlesRenderer>,
        scale: Scale<f64>,
        geometry: Rectangle<f64, Logical>,
        program: GlesTexProgram,
        corner_radius: Corners,
    ) -> Self {
        ClippedSurfaceElement {
            inner,
            program,
            corner_radius,
            geometry,
            scale: scale.x as f32,
        }
    }

    fn uniforms(&self) -> Vec<Uniform<'static>> {
        let scale = Scale::from(f64::from(self.scale));
        let elem_geo = self.inner.geometry(scale);
        let elem_loc = Vec2::new(elem_geo.loc.x as f32, elem_geo.loc.y as f32);
        let elem_size = Vec2::new(elem_geo.size.w as f32, elem_geo.size.h as f32);
        let geo = self.geometry.to_physical_precise_round(scale);
        let geo_loc = Vec2::new(geo.loc.x, geo.loc.y);
        let geo_size = Vec2::new(geo.size.w, geo.size.h);
        let buf = self.inner.buffer_size();
        let buf_size = Vec2::new(buf.w as f32, buf.h as f32);
        let view = self.inner.view();
        let src_loc = Vec2::new(view.src.loc.x as f32, view.src.loc.y as f32);
        let src_size = Vec2::new(view.src.size.w as f32, view.src.size.h as f32);
        let transform = match self.inner.transform() {
            Transform::_90 => Transform::_270,
            Transform::_270 => Transform::_90,
            x => x,
        };
        let transform_matrix = Mat3::from_translation(Vec2::new(0.5, 0.5))
            * transform.matrix()
            * Mat3::from_translation(-Vec2::new(0.5, 0.5));
        let y_invert = if buffer_y_inverted(self.inner.buffer()).unwrap_or(false) {
            Mat3::from_scale(Vec2::new(1., -1.))
        } else {
            Mat3::IDENTITY
        };
        let input_to_geo = transform_matrix
            * Mat3::from_scale(elem_size / geo_size)
            * Mat3::from_translation((elem_loc - geo_loc) / elem_size)
            * Mat3::from_scale(buf_size / src_size)
            * Mat3::from_translation(-src_loc / buf_size)
            * y_invert;
        let logical_geo_size = (self.geometry.size.w as f32, self.geometry.size.h as f32);
        vec![
            Uniform::new("hecate_scale", self.scale),
            Uniform::new("geo_size", logical_geo_size),
            Uniform::new("corner_radius", self.corner_radius.as_array()),
            mat3_uniform("input_to_geo", input_to_geo).into_owned(),
        ]
    }

    fn rounded_corners(geo: Rectangle<f64, Logical>, r: Corners) -> [Rectangle<f64, Logical>; 4] {
        let (tl, tr, br, bl) = (
            r.top_left as f64,
            r.top_right as f64,
            r.bottom_right as f64,
            r.bottom_left as f64,
        );
        [
            Rectangle::new(geo.loc, Size::from((tl, tl))),
            Rectangle::new(
                Point::from((geo.loc.x + geo.size.w - tr, geo.loc.y)),
                Size::from((tr, tr)),
            ),
            Rectangle::new(
                Point::from((geo.loc.x + geo.size.w - br, geo.loc.y + geo.size.h - br)),
                Size::from((br, br)),
            ),
            Rectangle::new(
                Point::from((geo.loc.x, geo.loc.y + geo.size.h - bl)),
                Size::from((bl, bl)),
            ),
        ]
    }

    /// Whether clipping changes anything for this element (else render it plainly, which keeps
    /// direct scanout possible).
    pub fn will_clip(
        elem: &WaylandSurfaceRenderElement<GlesRenderer>,
        scale: Scale<f64>,
        geometry: Rectangle<f64, Logical>,
        radius: Corners,
    ) -> bool {
        let elem_geo = elem.geometry(scale);
        let geo = geometry.to_physical_precise_round(scale);
        if radius.is_zero() {
            return !geo.contains_rect(elem_geo);
        }
        let corners = Self::rounded_corners(geometry, radius)
            .into_iter()
            .map(|r| r.to_physical_precise_up(scale));
        let geo = Rectangle::subtract_rects_many([geo], corners);
        !Rectangle::subtract_rects_many([elem_geo], geo).is_empty()
    }
}

impl Element for ClippedSurfaceElement {
    fn id(&self) -> &Id {
        self.inner.id()
    }

    fn current_commit(&self) -> CommitCounter {
        self.inner.current_commit()
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
        let damage = self.inner.damage_since(scale, commit);
        let mut geo = self.geometry.to_physical_precise_round(scale);
        geo.loc -= self.geometry(scale).loc;
        damage.into_iter().filter_map(|r| r.intersection(geo)).collect()
    }

    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        let regions = self.inner.opaque_regions(scale);
        let mut geo = self.geometry.to_physical_precise_round(scale);
        geo.loc -= self.geometry(scale).loc;
        let regions = regions.into_iter().filter_map(|r| r.intersection(geo));
        let elem_loc = self.geometry(scale).loc;
        let corners = Self::rounded_corners(self.geometry, self.corner_radius)
            .into_iter()
            .map(|r| {
                let mut r = r.to_physical_precise_up(scale);
                r.loc -= elem_loc;
                r
            });
        OpaqueRegions::from_slice(&Rectangle::subtract_rects_many(regions, corners))
    }

    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }

    fn kind(&self) -> Kind {
        self.inner.kind()
    }
}

impl RenderElement<GlesRenderer> for ClippedSurfaceElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        frame.override_default_tex_program(self.program.clone(), self.uniforms());
        RenderElement::<GlesRenderer>::draw(&self.inner, frame, src, dst, damage, opaque_regions, cache)?;
        frame.clear_tex_program_override();
        Ok(())
    }

    fn underlying_storage(&self, _renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_hides_border_when_fullscreen() {
        // Pure logic: width 0 means no border drawn.
        let c = Corners {
            top_left: 10.,
            top_right: 10.,
            bottom_right: 4.,
            bottom_left: 4.,
        };
        assert_eq!(c.shrink(2.).as_array(), [8., 8., 2., 2.]);
    }
}
