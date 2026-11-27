//! Window frames: borders with per-corner radii, solid or double, the notch label on the
//! top edge, and rounded clipping of window contents.
//!
//! The clipping element is adapted from niri's `ClippedSurfaceRenderElement`
//! (https://github.com/YaLTeR/niri, GPL-3.0-or-later). The border is a pixel shader that
//! draws the rings between nested rounded rectangles.

use std::cell::{Cell, RefCell};

use glam::{Mat3, Vec2};
use smithay::backend::renderer::buffer_y_inverted;
use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
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

use super::WindowElement;
use super::label::{NOTCH_HEIGHT, NotchCache};
use crate::config::{BorderLine, Color, Config, Corners};
use crate::wm::{Maximized, WindowExt};

pub type BorderElement = PixelShaderElement;

pub(super) const ROUNDING_ALPHA: &str = r#"
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
uniform vec4 background;
uniform vec4 outer_radius;
uniform vec4 inner_radius;
// A line of the border, the gap between two of them (0 for a solid border), the whole
// width, and the band under the top edge.
uniform vec4 widths;
uniform float hecate_scale;

float hecate_rounding_alpha(vec2 coords, vec2 size, vec4 r, float scale);

// How much of the pixel is inside the frame's rectangle shrunk by `inset` on every side
// and by `top` more at the top.
float within(vec2 coords, float inset, float top, vec4 radius) {
    vec2 extent = size - vec2(2.0 * inset, 2.0 * inset + top);
    vec2 c = coords - vec2(inset, inset + top);
    if (c.x < 0.0 || c.y < 0.0 || c.x > extent.x || c.y > extent.y)
        return 0.0;
    return hecate_rounding_alpha(c, extent, radius, hecate_scale);
}

void main() {
    vec2 coords = v_coords * size;
    float line = widths.x;
    float gap = widths.y;
    // From the outside in: a line, the gap, the second line, the band, the window.
    float a0 = within(coords, 0.0, 0.0, outer_radius);
    float a1 = within(coords, line, 0.0, max(outer_radius - line, 0.0));
    float a2 = within(coords, line + gap, 0.0, max(outer_radius - line - gap, 0.0));
    float a3 = within(coords, widths.z, 0.0, inner_radius);
    float a4 = within(coords, widths.z, widths.w, inner_radius);

    gl_FragColor = (color * (a0 - a1 + a2 - a3) + background * (a1 - a2 + a3 - a4)) * alpha;
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
    pub pixelate: Option<GlesTexProgram>,
}

impl Shaders {
    /// Compile the shaders once per renderer.
    pub fn init(renderer: &mut GlesRenderer) {
        let border = renderer
            .compile_custom_pixel_shader(
                format!("{BORDER_FRAG}\n{ROUNDING_ALPHA}"),
                &[
                    UniformName::new("color", UniformType::_4f),
                    UniformName::new("background", UniformType::_4f),
                    UniformName::new("outer_radius", UniformType::_4f),
                    UniformName::new("inner_radius", UniformType::_4f),
                    UniformName::new("widths", UniformType::_4f),
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
        let pixelate = super::pixelate::compile(renderer);
        let data = renderer.egl_context().user_data();
        data.insert_if_missing(|| {
            RefCell::new(Shaders {
                border: None,
                clip: None,
                pixelate: None,
            })
        });
        let cell = data.get::<RefCell<Shaders>>().unwrap();
        *cell.borrow_mut() = Shaders {
            border,
            clip,
            pixelate,
        };
        super::blur::BlurShaders::init(renderer);
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

    pub fn pixelate(renderer: &GlesRenderer) -> Option<GlesTexProgram> {
        renderer
            .egl_context()
            .user_data()
            .get::<RefCell<Shaders>>()
            .and_then(|s| s.borrow().pixelate.clone())
    }
}

/// How far a window's frame reaches beyond its content on each side, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Extents {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Extents {
    /// The frame around a window's rectangle.
    pub fn around(&self, rect: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
        Rectangle::new(
            (rect.loc.x - self.left, rect.loc.y - self.top).into(),
            (
                rect.size.w + self.left + self.right,
                rect.size.h + self.top + self.bottom,
            )
                .into(),
        )
    }

    /// The window's rectangle that fills `rect` with its frame.
    pub fn inside(&self, rect: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
        Rectangle::new(
            (rect.loc.x + self.left, rect.loc.y + self.top).into(),
            (
                (rect.size.w - self.left - self.right).max(1),
                (rect.size.h - self.top - self.bottom).max(1),
            )
                .into(),
        )
    }
}

/// An explicit border colour for one window (`border_tint`), in its user data. It wins
/// over urgency and focus until it is cleared.
#[derive(Default)]
struct Tint {
    color: Cell<Option<Color>>,
    /// Counts every change, so a timer only clears the tint it was set for.
    generation: Cell<u64>,
}

/// Tint a window's border, or give it back its usual colours. Returns a token for
/// `clear_tint_if`.
pub fn set_tint(window: &Window, color: Option<Color>) -> u64 {
    let tint = window.user_data().get_or_insert(Tint::default);
    tint.color.set(color);
    tint.generation.set(tint.generation.get() + 1);
    tint.generation.get()
}

pub fn tint(window: &Window) -> Option<Color> {
    window.user_data().get::<Tint>().and_then(|t| t.color.get())
}

/// Clear the tint that `set_tint` returned `token` for, unless it changed since. Returns
/// whether it did clear it.
pub fn clear_tint_if(window: &Window, token: u64) -> bool {
    match window.user_data().get::<Tint>() {
        Some(t) if t.generation.get() == token && t.color.get().is_some() => {
            t.color.set(None);
            true
        }
        _ => false,
    }
}

/// Resolved border look for one window.
#[derive(Debug, Clone, PartialEq)]
pub struct BorderStyle {
    pub width: f32,
    pub line: BorderLine,
    pub radius: Corners,
    pub color: Color,
    /// Fills a double border's gap and the label's band.
    pub background: Color,
    pub clip: bool,
    pub draw_border: bool,
    /// The notch label: its text colour and its offset from the border's inner left edge.
    /// None for a frame without label and band.
    pub label: Option<(Color, i32)>,
}

impl BorderStyle {
    /// The border's width in whole pixels (0 when it isn't drawn).
    fn width_px(&self) -> i32 {
        if self.draw_border {
            self.width.ceil() as i32
        } else {
            0
        }
    }

    /// Height of the band between the top line and the window. The label is centred on the
    /// line's inner edge, as in the mockups, so its lower half needs this much room to stay
    /// off the window.
    fn band(&self) -> i32 {
        if self.draw_border && self.label.is_some() {
            NOTCH_HEIGHT / 2
        } else {
            0
        }
    }

    /// Widths of one line and of the gap between two (0 when there is one line).
    fn lines(&self) -> (i32, i32) {
        let w = self.width_px();
        let line = ((w + 1) / 3).max(1);
        match self.line {
            BorderLine::Double if w - 2 * line > 0 => (line, w - 2 * line),
            _ => (w, 0),
        }
    }

    /// The border with its band: what the shader fills.
    fn border_extents(&self) -> Extents {
        let w = self.width_px();
        Extents {
            top: w + self.band(),
            right: w,
            bottom: w,
            left: w,
        }
    }

    /// Everything drawn around the window: the border, the band, and the upper half of the
    /// label where it stands above a thin top line.
    pub fn extents(&self) -> Extents {
        let w = self.width_px();
        let band = self.band();
        Extents {
            top: band + w.max(band),
            ..self.border_extents()
        }
    }
}

/// The frame around `window` as layout sees it (snapping, maximizing, hit-testing).
pub fn extents_for(config: &Config, window: &Window) -> Extents {
    style_for(config, window, false).extents()
}

/// The frame around a window that is neither maximized nor fullscreen and has no rules.
pub fn default_extents(config: &Config) -> Extents {
    let b = &config.border;
    BorderStyle {
        width: b.width.max(0.),
        line: b.style,
        radius: b.radius.corners(),
        color: b.color.inactive,
        background: b.background,
        clip: b.clip_content,
        draw_border: b.width > 0.,
        label: b.label.enable.then_some((b.label.color, b.label.offset)),
    }
    .extents()
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
    let color = if let Some(tint) = tint(window) {
        tint
    } else if d.urgent {
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
    let line = o.style.unwrap_or(config.border.style);
    let background = config.border.background;
    let label = o
        .label
        .unwrap_or(config.border.label.enable)
        .then_some((config.border.label.color, config.border.label.offset));
    if identify {
        return BorderStyle {
            width: width.max(6.),
            line,
            radius,
            color: colors.urgent,
            background,
            clip,
            draw_border: true,
            label,
        };
    }
    BorderStyle {
        width: if draw_border { width } else { 0. },
        line,
        // Fullscreen and maximized windows get square corners.
        radius: if fullscreen || maximized_hides {
            Corners::default()
        } else {
            radius
        },
        color,
        background,
        clip: clip && !fullscreen,
        draw_border,
        label: label.filter(|_| draw_border),
    }
}

/// Width of the line the overview draws around a selected window that has no border.
pub const OVERVIEW_OUTLINE: f32 = 2.;

/// The frame of a window's slot in the overview.
#[derive(Debug, Clone, PartialEq)]
pub struct OverviewStyle {
    pub style: BorderStyle,
    /// Drawn around the selected slot only (a window whose border has no width).
    pub selected_only: bool,
}

/// The frame the overview draws around `window`. Every window gets one there, since the
/// frame's colour is what shows the selection: a maximized, fullscreen or border-hidden
/// window gets the configured frame back, square, and one whose border has no width gets a
/// thin line while it is selected.
pub fn style_for_overview(config: &Config, window: &Window) -> OverviewStyle {
    let style = style_for(config, window, false);
    if style.draw_border {
        return OverviewStyle {
            style,
            selected_only: false,
        };
    }
    let o = window.data().rules.border.clone().unwrap_or_default();
    let width = o.width.unwrap_or(config.border.width).max(0.);
    let hidden = BorderStyle {
        radius: Corners::default(),
        clip: false,
        draw_border: true,
        ..style
    };
    if width > 0. {
        let label = o
            .label
            .unwrap_or(config.border.label.enable)
            .then_some((config.border.label.color, config.border.label.offset));
        OverviewStyle {
            style: BorderStyle {
                width,
                label,
                ..hidden
            },
            selected_only: false,
        }
    } else {
        OverviewStyle {
            style: BorderStyle {
                width: OVERVIEW_OUTLINE,
                line: BorderLine::Solid,
                label: None,
                ..hidden
            },
            selected_only: true,
        }
    }
}

/// Per-window persistent border element (so its damage tracking stays stable), and the
/// uniform values it was last given: they are only sent again when one changes, since
/// every update damages the whole border.
struct BorderSlot(RefCell<Option<(PixelShaderElement, Vec<f32>)>>);

/// Push a window's elements (popups, clipped content, label, border), topmost first.
#[allow(clippy::too_many_arguments)]
pub fn push_window(
    renderer: &mut GlesRenderer,
    out: &mut Vec<WindowElement>,
    window: &Window,
    geo: Rectangle<i32, Logical>,
    output_loc: Point<i32, Logical>,
    scale: Scale<f64>,
    alpha: f32,
    style: &BorderStyle,
) {
    push_popups(renderer, out, window, geo, output_loc, scale, alpha);
    push_body(renderer, out, window, geo, output_loc, scale, alpha, style);
}

/// A window's popups. They are never clipped and go on top.
pub fn push_popups(
    renderer: &mut GlesRenderer,
    out: &mut Vec<WindowElement>,
    window: &Window,
    geo: Rectangle<i32, Logical>,
    output_loc: Point<i32, Logical>,
    scale: Scale<f64>,
    alpha: f32,
) {
    let toplevel = window.toplevel_surface();
    let surface = toplevel.wl_surface();
    let render_loc = geo.loc - window.geometry().loc;
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
        out.extend(elems.into_iter().map(WindowElement::Surface));
    }
}

/// A window without its popups: the (clipped) content and the frame around it, all
/// within `style.extents()` of `geo`.
#[allow(clippy::too_many_arguments)]
pub fn push_body(
    renderer: &mut GlesRenderer,
    out: &mut Vec<WindowElement>,
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
                out.push(WindowElement::Clipped(ClippedSurfaceElement::new(
                    e,
                    scale,
                    local_geo.to_f64(),
                    program.clone(),
                    inner_radius,
                )));
            }
            _ => out.push(WindowElement::Surface(e)),
        }
    }

    push_frame(renderer, out, window, geo, output_loc, scale, alpha, style);
}

/// The frame around a window's content at `geo`: the notch label, the band and the lines.
/// Nothing when the style draws no border.
#[allow(clippy::too_many_arguments)]
pub fn push_frame(
    renderer: &mut GlesRenderer,
    out: &mut Vec<WindowElement>,
    window: &Window,
    geo: Rectangle<i32, Logical>,
    output_loc: Point<i32, Logical>,
    scale: Scale<f64>,
    alpha: f32,
    style: &BorderStyle,
) {
    if !style.draw_border {
        return;
    }
    let local_geo = Rectangle::new(geo.loc - output_loc, geo.size);
    let band = style.band();

    // The label straddles the top line's inner edge, over the border.
    if let Some((color, offset)) = style.label {
        let zoom = scale.x.ceil().max(1.) as usize;
        let cache = window.user_data().get_or_insert(NotchCache::default);
        let room = local_geo.size.w - 2 * offset;
        let label = cache.get(
            &window.app_id(),
            &window.title(),
            room,
            color,
            style.background,
            zoom,
        );
        if let Some((buffer, _)) = label {
            let loc = Point::<i32, Logical>::from((local_geo.loc.x + offset, local_geo.loc.y - 2 * band));
            match MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                loc.to_physical_precise_round(scale),
                &buffer,
                Some(alpha),
                None,
                None,
                Kind::Unspecified,
            ) {
                Ok(e) => out.push(WindowElement::Label(e)),
                Err(err) => tracing::warn!("error importing window label: {err:?}"),
            }
        }
    }

    if let Some(program) = Shaders::border(renderer) {
        let w = style.width;
        let wi = style.width_px();
        let (line, gap) = style.lines();
        let area = style.border_extents().around(local_geo);
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
        // The element's own alpha is fixed when it's created; fade through the colours.
        let color = style.color.premultiplied().map(|c| c * alpha);
        let background = style.background.premultiplied().map(|c| c * alpha);
        let widths = [line as f32, gap as f32, wi as f32, band as f32];
        let values: Vec<f32> = [
            &color[..],
            &background[..],
            &outer.as_array()[..],
            &style.radius.as_array()[..],
            &widths[..],
            &[scale.x as f32][..],
        ]
        .concat();
        let uniforms = || {
            vec![
                Uniform::new("color", color),
                Uniform::new("background", background),
                Uniform::new("outer_radius", outer.as_array()),
                Uniform::new("inner_radius", style.radius.as_array()),
                Uniform::new("widths", widths),
                Uniform::new("hecate_scale", scale.x as f32),
            ]
        };
        let slot = window
            .user_data()
            .get_or_insert(|| BorderSlot(RefCell::new(None)));
        let mut slot = slot.0.borrow_mut();
        let elem = match slot.as_mut() {
            Some((e, last)) => {
                e.resize(area, None);
                if *last != values {
                    e.update_uniforms(uniforms());
                    *last = values;
                }
                e.clone()
            }
            None => {
                let e = PixelShaderElement::new(program, area, None, 1.0, uniforms(), Kind::Unspecified);
                *slot = Some((e.clone(), values));
                e
            }
        };
        out.push(WindowElement::Border(elem));
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

    fn style(width: f32, line: BorderLine, label: bool) -> BorderStyle {
        BorderStyle {
            width,
            line,
            radius: Corners::default(),
            color: Color::rgb(0x31, 0x74, 0x8f),
            background: Color::rgb(0x1f, 0x1d, 0x2e),
            clip: true,
            draw_border: width > 0.,
            label: label.then_some((Color::rgb(0xe0, 0xde, 0xf4), 15)),
        }
    }

    #[test]
    fn double_lines_split_the_width_in_thirds() {
        let lines = |w: f32, line| style(w, line, false).lines();
        assert_eq!(lines(5., BorderLine::Double), (2, 1), "the haiiro frame: 2, 1, 2");
        assert_eq!(lines(3., BorderLine::Double), (1, 1));
        assert_eq!(lines(6., BorderLine::Double), (2, 2));
        assert_eq!(lines(9., BorderLine::Double), (3, 3));
        assert_eq!(lines(2., BorderLine::Double), (2, 0), "too thin for two lines");
        assert_eq!(lines(5., BorderLine::Solid), (5, 0));
    }

    #[test]
    fn frame_extents() {
        let plain = Extents {
            top: 5,
            right: 5,
            bottom: 5,
            left: 5,
        };
        assert_eq!(style(5., BorderLine::Double, false).extents(), plain);
        // With a label the top grows by a band of half the label's height, and the label's
        // upper half stands 4 px above the 5 px line.
        let labelled = style(5., BorderLine::Double, true);
        assert_eq!(labelled.border_extents().top, 5 + 9);
        assert_eq!(labelled.extents(), Extents { top: 18, ..plain });
        // A line thicker than half the label holds all of it.
        assert_eq!(style(12., BorderLine::Solid, true).extents().top, 12 + 9);
        // No border, no band.
        assert_eq!(style(0., BorderLine::Solid, true).extents(), Extents::default());

        let rect = Rectangle::new((100, 100).into(), (300, 200).into());
        let frame = labelled.extents().around(rect);
        assert_eq!(frame, Rectangle::new((95, 82).into(), (310, 223).into()));
        assert_eq!(labelled.extents().inside(frame), rect);
    }

    #[test]
    fn default_extents_follow_the_config() {
        let mut config = Config::default();
        assert_eq!(default_extents(&config).top, 2);
        config.border.width = 5.;
        config.border.label.enable = true;
        assert_eq!(default_extents(&config).top, 18);
        assert_eq!(default_extents(&config).left, 5);
        config.border.width = 0.;
        assert_eq!(default_extents(&config), Extents::default());
    }
}
