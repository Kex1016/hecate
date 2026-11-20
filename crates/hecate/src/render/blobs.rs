//! The blob background (`background.blobs`): pixelated blobs that drift, bridge and merge
//! under every output's layers, in three dithered tones.
//!
//! The simulation is a few dozen blobs per output and runs on the CPU, in cells of `cell`
//! logical pixels. A shader sums their fields and dithers the sum, into a texture of one
//! texel per cell; a second shader draws that texture over the output in sharp squares.
//! The field is only drawn again when the simulation has moved, `fps` times a second at
//! most: a timer steps it and asks for a frame, and stops when nothing moves (blobs off or
//! still, animations off, a fullscreen window, the output powered off) or nothing is drawn
//! (the session is on another VT).
//!
//! A simulation can be exported and carried on from by another compositor (`save` and
//! `resume`, or IPC): hecate-greetd's login screen hands its blobs to the session, whose
//! first frame is then the next one of the same picture.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use hecate_ipc as ipc;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::{Element, Id, Kind, RenderElement, UnderlyingStorage};
use smithay::backend::renderer::gles::{
    GlesError, GlesFrame, GlesPixelProgram, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformName,
    UniformType,
};
use smithay::backend::renderer::utils::{CommitCounter, DamageSet, OpaqueRegions};
use smithay::backend::renderer::{Bind, ContextId, Frame, Offscreen, Renderer, Texture};
use smithay::output::Output;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::utils::user_data::UserDataMap;
use smithay::utils::{Buffer, Physical, Rectangle, Scale, Size, Transform};

use crate::config::{Blobs as BlobsConfig, Color};
use crate::state::{Hecate, State};
use crate::wm::{WindowExt, Wm};

/// As many blobs as the field shader takes.
pub const MAX_BLOBS: usize = 64;
/// The smallest blob that drifts in, in cells.
pub const RADIUS_MIN: f32 = 2.;
/// The biggest one. Merging makes bigger ones, up to `radius_max`.
const RADIUS_START: f32 = 9.;
/// A blob's field counts this many radii around it.
const REACH: f32 = 3.;
/// The longest step, in seconds: after a stall the blobs carry on from where they were.
const MAX_STEP: f32 = 0.1;
/// New blobs drift in while there are fewer than this share of the wanted number.
const REFILL: f32 = 0.85;
/// A still frame is taken after this many steps of `MAX_STEP`: blobs that started on top of
/// each other have merged by then.
const SETTLE_STEPS: u32 = 300;
/// Steps a simulation may be ahead of what was last drawn of it. Past that nobody is
/// looking (the session is on another VT) and it rests until the next frame.
const UNSEEN_STEPS: u8 = 8;
/// Where a tone starts in the field, and how much field its dither ramp spans.
const HALO: (f32, f32) = (0.3, 0.75);
const CORE: (f32, f32) = (0.9, 0.5);
/// The ordered dither's thresholds, row by row. The shader computes the same numbers.
#[cfg(test)]
const BAYER: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];

/// SplitMix64: small, and its whole state is one number to hand over.
#[derive(Debug, Clone, PartialEq)]
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// In `from..to`.
    fn range(&mut self, from: f32, to: f32) -> f32 {
        let unit = (self.next() >> 40) as f32 / (1u64 << 24) as f32;
        from + unit * (to - from)
    }
}

/// A seed nobody chose.
fn entropy() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
}

/// The blobs of one output, on a grid of `w` by `h` cells.
#[derive(Debug, Clone, PartialEq)]
pub struct Sim {
    w: i32,
    h: i32,
    blobs: Vec<ipc::Blob>,
    rng: Rng,
    /// Seconds simulated.
    time: f64,
}

impl Sim {
    /// As many blobs as the grid wants, anywhere on it and headed anywhere.
    pub fn new(w: i32, h: i32, cfg: &BlobsConfig, seed: u64) -> Sim {
        let mut sim = Sim {
            w,
            h,
            blobs: Vec::new(),
            rng: Rng(seed),
            time: 0.,
        };
        for _ in 0..sim.target(cfg) {
            let (x, y) = (sim.rng.range(0., w as f32), sim.rng.range(0., h as f32));
            let heading = sim.rng.range(0., TAU);
            sim.add(x, y, heading, cfg);
        }
        sim
    }

    /// Carry on from an exported simulation, on a grid of `w` by `h`.
    pub fn restore(saved: &ipc::BlobsOutput, w: i32, h: i32) -> Sim {
        let sane = |b: &&ipc::Blob| {
            [b.x, b.y, b.r, b.vx, b.vy].iter().all(|v| v.is_finite()) && b.r > 0. && b.r <= 1024.
        };
        let mut sim = Sim {
            w: (saved.width as i32).max(1),
            h: (saved.height as i32).max(1),
            blobs: saved.blobs.iter().filter(sane).take(MAX_BLOBS).copied().collect(),
            rng: Rng(saved.seed),
            time: if saved.time.is_finite() { saved.time } else { 0. },
        };
        sim.resize(w, h);
        sim
    }

    pub fn export(&self, output: &str) -> ipc::BlobsOutput {
        ipc::BlobsOutput {
            output: output.to_owned(),
            width: self.w as u32,
            height: self.h as u32,
            time: self.time,
            seed: self.rng.0,
            blobs: self.blobs.clone(),
        }
    }

    pub fn blobs(&self) -> &[ipc::Blob] {
        &self.blobs
    }

    pub fn size(&self) -> (i32, i32) {
        (self.w, self.h)
    }

    pub fn time(&self) -> f64 {
        self.time
    }

    /// How many blobs the grid wants.
    fn target(&self, cfg: &BlobsConfig) -> usize {
        let wanted = ((self.w * self.h) as f32 / cfg.density).round();
        (wanted.max(cfg.count_min as f32).min(cfg.count_max as f32) as usize).min(MAX_BLOBS)
    }

    fn add(&mut self, x: f32, y: f32, heading: f32, cfg: &BlobsConfig) {
        // Mostly small ones.
        let u = self.rng.range(0., 1.);
        let r = (RADIUS_MIN + u * u * (RADIUS_START - RADIUS_MIN)).min(cfg.radius_max);
        let speed = self.rng.range(cfg.speed_min, cfg.speed_max);
        self.blobs.push(ipc::Blob {
            x,
            y,
            r,
            vx: heading.cos() * speed,
            vy: heading.sin() * speed,
        });
    }

    /// A blob just off a random edge, headed roughly for the middle.
    fn spawn(&mut self, cfg: &BlobsConfig) {
        let (w, h) = (self.w as f32, self.h as f32);
        let before = self.blobs.len();
        self.add(0., 0., 0., cfg);
        let m = REACH * self.blobs[before].r;
        let (x, y) = match self.rng.next() % 4 {
            0 => (-m, self.rng.range(0., h)),
            1 => (w + m, self.rng.range(0., h)),
            2 => (self.rng.range(0., w), -m),
            _ => (self.rng.range(0., w), h + m),
        };
        let heading = (h / 2. - y).atan2(w / 2. - x) + self.rng.range(-0.6, 0.6);
        let b = &mut self.blobs[before];
        let speed = b.vx.hypot(b.vy);
        (b.x, b.y) = (x, y);
        (b.vx, b.vy) = (heading.cos() * speed, heading.sin() * speed);
    }

    /// The grid changed: the blobs keep their place on the output.
    pub fn resize(&mut self, w: i32, h: i32) {
        if (w, h) == (self.w, self.h) {
            return;
        }
        let (kx, ky) = (w as f32 / self.w as f32, h as f32 / self.h as f32);
        for b in &mut self.blobs {
            b.x *= kx;
            b.y *= ky;
        }
        (self.w, self.h) = (w, h);
    }

    /// Move on by `dt` seconds.
    pub fn step(&mut self, dt: f32, cfg: &BlobsConfig) {
        self.time += dt as f64;
        for b in &mut self.blobs {
            b.x += b.vx * dt;
            b.y += b.vy * dt;
        }

        // Two blobs merge once the centre of one is inside the other: the new one has
        // their area and their momentum. Too big a pair passes through each other.
        let mut i = 0;
        while i < self.blobs.len() {
            let mut j = i + 1;
            while j < self.blobs.len() {
                let (a, c) = (self.blobs[i], self.blobs[j]);
                let apart = (a.x - c.x).hypot(a.y - c.y);
                let (wa, wc) = (a.r * a.r, c.r * c.r);
                let r = (wa + wc).sqrt();
                if apart > a.r.max(c.r) || r > cfg.radius_max {
                    j += 1;
                    continue;
                }
                let w = wa + wc;
                let (mut vx, mut vy) = ((a.vx * wa + c.vx * wc) / w, (a.vy * wa + c.vy * wc) / w);
                let mut speed = vx.hypot(vy);
                if speed < 1e-3 {
                    // Head on: the momentum is gone, and nothing stands still.
                    let heading = self.rng.range(0., TAU);
                    (vx, vy, speed) = (heading.cos(), heading.sin(), 1.);
                }
                if speed < cfg.speed_min {
                    vx *= cfg.speed_min / speed;
                    vy *= cfg.speed_min / speed;
                }
                self.blobs[i] = ipc::Blob {
                    x: (a.x * wa + c.x * wc) / w,
                    y: (a.y * wa + c.y * wc) / w,
                    r,
                    vx,
                    vy,
                };
                self.blobs.remove(j);
            }
            i += 1;
        }

        // Gone once it is out of sight and leaving; others drift in for the ones lost.
        let (w, h) = (self.w as f32, self.h as f32);
        self.blobs.retain(|b| {
            let m = REACH * b.r + 2.;
            !((b.x < -m && b.vx <= 0.)
                || (b.x > w + m && b.vx >= 0.)
                || (b.y < -m && b.vy <= 0.)
                || (b.y > h + m && b.vy >= 0.))
        });
        self.blobs.truncate(MAX_BLOBS);
        while (self.blobs.len() as f32) < self.target(cfg) as f32 * REFILL {
            self.spawn(cfg);
        }
    }

    /// Run until it looks like it has always been running: what a still frame shows.
    pub fn settle(&mut self, cfg: &BlobsConfig) {
        for _ in 0..SETTLE_STEPS {
            self.step(MAX_STEP, cfg);
        }
    }

    /// The tone of every cell, row by row: 0 the ground, 1 the halo, 2 the core. What the
    /// field shader draws, for the tests to hold it to.
    #[cfg(test)]
    pub fn tones(&self) -> Vec<u8> {
        let (w, h) = (self.w as usize, self.h as usize);
        let mut field = vec![0f32; w * h];
        for b in &self.blobs {
            let reach = REACH * b.r;
            let (x0, x1) = ((b.x - reach).floor() as i32, (b.x + reach).ceil() as i32);
            let (y0, y1) = ((b.y - reach).floor() as i32, (b.y + reach).ceil() as i32);
            for y in y0.max(0)..=y1.min(self.h - 1) {
                for x in x0.max(0)..=x1.min(self.w - 1) {
                    let (dx, dy) = (x as f32 + 0.5 - b.x, y as f32 + 0.5 - b.y);
                    field[y as usize * w + x as usize] += b.r * b.r / (dx * dx + dy * dy + 0.01);
                }
            }
        }
        let mut tones = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                let f = field[y * w + x];
                let t = (BAYER[(y & 3) * 4 + (x & 3)] as f32 + 0.5) / 16.;
                tones[y * w + x] = if (f - CORE.0) / CORE.1 > t {
                    2
                } else if (f - HALO.0) / HALO.1 > t {
                    1
                } else {
                    0
                };
            }
        }
        tones
    }
}

/// Sums the fields of the blobs around a cell and dithers the sum into a tone. Drawn into
/// a texture of one texel per cell.
const FIELD_FRAG: &str = r#"
precision highp float;
varying vec2 v_coords;
uniform vec2 size;
uniform float alpha;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

// A blob: its centre, how far its field counts and its radius squared, in cells.
uniform vec4 blobs[MAX_BLOBS];
uniform float count;
uniform vec3 ground;
uniform vec3 halo;
uniform vec3 core;
// Where the halo and the core start in the field, and the field their ramps span.
uniform vec4 ramps;

// The 2x2 Bayer matrix (0 2 / 3 1); the 4x4 one is two rounds of it.
float bayer2(vec2 at) {
    return 2.0 * at.x + 3.0 * at.y - 4.0 * at.x * at.y;
}

void main() {
    vec2 cell = floor(v_coords * size);
    vec2 middle = cell + 0.5;
    float field = 0.0;
    for (int i = 0; i < MAX_BLOBS; i++) {
        if (float(i) >= count)
            break;
        vec4 blob = blobs[i];
        // Whole cells, like the box the reference walks.
        vec2 from = floor(blob.xy - blob.z);
        vec2 to = ceil(blob.xy + blob.z);
        if (cell.x < from.x || cell.y < from.y || cell.x > to.x || cell.y > to.y)
            continue;
        vec2 d = middle - blob.xy;
        field += blob.w / (d.x * d.x + d.y * d.y + 0.01);
    }

    vec2 in_tile = mod(cell, 4.0);
    float threshold = (4.0 * bayer2(mod(in_tile, 2.0)) + bayer2(floor(in_tile * 0.5)) + 0.5) / 16.0;
    vec3 color = ground;
    if ((field - ramps.z) / ramps.w > threshold)
        color = core;
    else if ((field - ramps.x) / ramps.y > threshold)
        color = halo;

    gl_FragColor = vec4(color, 1.0) * alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        gl_FragColor = vec4(0.0, 0.2, 0.0, 0.2) + gl_FragColor * 0.8;
#endif
}
"#;

/// Draws the cell texture in squares: every pixel of a cell samples the middle of its
/// texel, so the filter has nothing to blend.
const CELLS_FRAG: &str = r#"
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

// The texture's size: the grid.
uniform vec2 cells;

void main() {
    vec4 color = texture2D(tex, (floor(v_coords * cells) + 0.5) / cells);
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

/// `blobs[0]`, `blobs[1]`, …: the field shader's array, one uniform per element.
static BLOB_UNIFORMS: LazyLock<Vec<String>> =
    LazyLock::new(|| (0..MAX_BLOBS).map(|i| format!("blobs[{i}]")).collect());

#[derive(Debug, Clone)]
struct Programs {
    field: GlesPixelProgram,
    cells: GlesTexProgram,
}

impl Programs {
    /// The shaders, compiled the first time they are asked for. `None` if they don't
    /// compile: the background is the solid colour then.
    fn get(renderer: &mut GlesRenderer) -> Option<Programs> {
        if let Some(programs) = renderer.egl_context().user_data().get::<Option<Programs>>() {
            return programs.clone();
        }
        let programs = Self::compile(renderer)
            .map_err(|e| tracing::warn!("error compiling the blob shaders, no blobs: {e:?}"))
            .ok();
        renderer
            .egl_context()
            .user_data()
            .insert_if_missing(|| programs.clone());
        programs
    }

    fn compile(renderer: &mut GlesRenderer) -> Result<Programs, GlesError> {
        let mut uniforms: Vec<UniformName<'_>> = BLOB_UNIFORMS
            .iter()
            .map(|name| UniformName::new(name.as_str(), UniformType::_4f))
            .collect();
        uniforms.extend([
            UniformName::new("count", UniformType::_1f),
            UniformName::new("ground", UniformType::_3f),
            UniformName::new("halo", UniformType::_3f),
            UniformName::new("core", UniformType::_3f),
            UniformName::new("ramps", UniformType::_4f),
        ]);
        let field = renderer.compile_custom_pixel_shader(
            FIELD_FRAG.replace("MAX_BLOBS", &MAX_BLOBS.to_string()),
            &uniforms,
        )?;
        let cells = renderer
            .compile_custom_texture_shader(CELLS_FRAG, &[UniformName::new("cells", UniformType::_2f)])?;
        Ok(Programs { field, cells })
    }
}

/// The cell texture drawn over an output.
#[derive(Debug)]
pub struct BlobsElement {
    id: Id,
    commit: CommitCounter,
    program: GlesTexProgram,
    texture: GlesTexture,
    /// The output, all of it.
    geometry: Rectangle<i32, Physical>,
    /// The cells that cover it: the last column and row may stick out.
    src: Rectangle<f64, Buffer>,
}

impl Element for BlobsElement {
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
        self.src
    }

    fn transform(&self) -> Transform {
        Transform::Normal
    }

    fn damage_since(&self, _scale: Scale<f64>, commit: Option<CommitCounter>) -> DamageSet<i32, Physical> {
        if commit == Some(self.commit) {
            DamageSet::default()
        } else {
            DamageSet::from_slice(&[Rectangle::from_size(self.geometry.size)])
        }
    }

    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        OpaqueRegions::from_slice(&[Rectangle::from_size(self.geometry.size)])
    }

    fn alpha(&self) -> f32 {
        1.
    }

    fn kind(&self) -> Kind {
        Kind::Unspecified
    }
}

impl RenderElement<GlesRenderer> for BlobsElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
        _cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        let grid = self.texture.size();
        frame.render_texture_from_to(
            &self.texture,
            src,
            dst,
            damage,
            opaque_regions,
            Transform::Normal,
            1.,
            Some(&self.program),
            &[Uniform::new("cells", (grid.w as f32, grid.h as f32))],
        )
    }

    fn underlying_storage(&self, _renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        None
    }
}

/// The field of `sim`, drawn into `texture` (or a new one, when that is the wrong size).
fn draw_field(
    renderer: &mut GlesRenderer,
    program: &GlesPixelProgram,
    sim: &Sim,
    tones: [Color; 3],
    texture: Option<GlesTexture>,
) -> anyhow::Result<GlesTexture> {
    let grid: Size<i32, Buffer> = sim.size().into();
    let mut texture = match texture {
        Some(texture) if texture.size() == grid => texture,
        _ => renderer
            .create_buffer(Fourcc::Abgr8888, grid)
            .context("error creating the cell texture")?,
    };
    let rgb = |c: Color| (c.r, c.g, c.b);
    let mut uniforms: Vec<Uniform<'_>> = sim
        .blobs()
        .iter()
        .take(MAX_BLOBS)
        .zip(BLOB_UNIFORMS.iter())
        .map(|(b, name)| Uniform::new(name.as_str(), (b.x, b.y, REACH * b.r, b.r * b.r)))
        .collect();
    uniforms.extend([
        Uniform::new("count", sim.blobs().len().min(MAX_BLOBS) as f32),
        Uniform::new("ground", rgb(tones[0])),
        Uniform::new("halo", rgb(tones[1])),
        Uniform::new("core", rgb(tones[2])),
        Uniform::new("ramps", (HALO.0, HALO.1, CORE.0, CORE.1)),
    ]);
    {
        let mut target = renderer
            .bind(&mut texture)
            .context("error binding the cell texture")?;
        let size: Size<i32, Physical> = (grid.w, grid.h).into();
        let mut frame = renderer
            .render(&mut target, size, Transform::Normal)
            .context("error starting the blob field")?;
        let all = Rectangle::from_size(size);
        frame
            .render_pixel_shader_to(
                program,
                Rectangle::from_size(grid.to_f64()),
                all,
                grid,
                Some(&[all]),
                1.,
                &uniforms,
            )
            .context("error drawing the blob field")?;
        let _ = frame.finish().context("error finishing the blob field")?;
    }
    Ok(texture)
}

/// An output's blobs and the picture of them.
struct OutputBlobs {
    id: Id,
    sim: Sim,
    /// Counts the pictures: one more for every step, new grid or new set of tones.
    version: usize,
    /// A cell's side in physical pixels.
    cell: i32,
    tones: [Color; 3],
    /// The field as last drawn, by which renderer and of which picture.
    texture: Option<(GlesTexture, ContextId<GlesTexture>, usize)>,
    /// When the simulation last moved, by the animation clock. `None` while it rests.
    stepped: Option<Duration>,
    /// Steps since a frame last showed the simulation as it was.
    unseen: u8,
}

/// Every output's blobs (by output name), and what goes on around them.
#[derive(Default)]
pub struct Blobs {
    outputs: RefCell<HashMap<String, OutputBlobs>>,
    /// Simulations for outputs to carry on from when they are first drawn.
    waiting: RefCell<HashMap<String, ipc::BlobsOutput>>,
    /// `background.blobs.resume` has been read.
    resumed: Cell<bool>,
    /// The timer that steps the simulations is running.
    ticking: Cell<bool>,
}

/// The grid for an output of `size`: whole cells of `cell` pixels, enough to cover it.
fn grid_for(size: Size<i32, Physical>, cell: i32) -> (i32, i32) {
    ((size.w + cell - 1) / cell, (size.h + cell - 1) / cell)
}

impl Hecate {
    /// Why `output`'s blobs don't move, if they don't.
    fn blobs_paused(&self, output: &Output) -> Option<&'static str> {
        let cfg = &self.config.background.blobs;
        if !cfg.enable {
            Some("off")
        } else if cfg.still {
            Some("still")
        } else if !self.config.animations.enable {
            // Game Mode.
            Some("animations_off")
        } else if self.output_state.get(output).is_none_or(|s| s.powered_off) {
            Some("output_off")
        } else if !self.wm.is_locked()
            && self
                .wm
                .windows_on_output(output)
                .iter()
                .any(|w| w.data().fullscreen)
        {
            Some("fullscreen")
        } else {
            None
        }
    }

    /// The blob background of `output`, drawn again if the blobs have moved. `None` when
    /// it is off or can't be drawn: the solid colour is the background then.
    pub(super) fn blobs_element(&self, renderer: &mut GlesRenderer, output: &Output) -> Option<BlobsElement> {
        let cfg = &self.config.background.blobs;
        let name = output.name();
        let mut outputs = self.blobs.outputs.borrow_mut();
        if !cfg.enable {
            // The blobs stay where they are for when it is back on; the picture can go.
            if let Some(blobs) = outputs.get_mut(&name) {
                blobs.texture = None;
            }
            return None;
        }
        let programs = Programs::get(renderer)?;
        let scale = output.current_scale().fractional_scale();
        let size: Size<i32, Physical> = Wm::output_geometry(output).size.to_physical_precise_round(scale);
        if size.w <= 0 || size.h <= 0 {
            return None;
        }
        let cell = ((cfg.cell as f64 * scale).round() as i32).max(1);
        let (w, h) = grid_for(size, cell);
        let moving = self.blobs_paused(output).is_none();

        let blobs = outputs.entry(name.clone()).or_insert_with(|| {
            self.read_blobs_to_resume();
            let sim = match self.blobs.waiting.borrow_mut().remove(&name) {
                Some(saved) => Sim::restore(&saved, w, h),
                None => {
                    let mut sim = Sim::new(w, h, cfg, entropy());
                    if !moving {
                        sim.settle(cfg);
                    }
                    sim
                }
            };
            OutputBlobs {
                id: Id::new(),
                sim,
                version: 0,
                cell,
                tones: [cfg.ground, cfg.halo, cfg.core],
                texture: None,
                stepped: None,
                unseen: 0,
            }
        });
        let tones = [cfg.ground, cfg.halo, cfg.core];
        if blobs.sim.size() != (w, h) || blobs.cell != cell || blobs.tones != tones {
            blobs.sim.resize(w, h);
            blobs.cell = cell;
            blobs.tones = tones;
            blobs.version = blobs.version.wrapping_add(1);
        }

        let context = renderer.context_id();
        let texture = match blobs.texture.take() {
            Some((texture, drawn_by, version)) if drawn_by == context && version == blobs.version => texture,
            stale => {
                let reuse = stale.filter(|(_, drawn_by, _)| *drawn_by == context).map(|s| s.0);
                draw_field(renderer, &programs.field, &blobs.sim, tones, reuse)
                    .map_err(|e| tracing::warn!("error drawing the blobs: {e:#}"))
                    .ok()?
            }
        };
        blobs.texture = Some((texture.clone(), context, blobs.version));

        if moving {
            self.start_blobs_timer();
        }
        Some(BlobsElement {
            id: blobs.id.clone(),
            commit: CommitCounter::from(blobs.version),
            program: programs.cells,
            texture,
            geometry: Rectangle::from_size(size),
            src: Rectangle::from_size((size.w as f64 / cell as f64, size.h as f64 / cell as f64).into()),
        })
    }

    /// The time between two steps: `fps` of them a second.
    fn blobs_period(&self) -> Duration {
        Duration::from_secs_f64(1. / self.config.background.blobs.fps.max(1) as f64)
    }

    /// Step the simulations for as long as one of them moves.
    fn start_blobs_timer(&self) {
        if self.blobs.ticking.replace(true) {
            return;
        }
        let timer = Timer::from_duration(self.blobs_period());
        let started = self.event_loop.insert_source(timer, |due, _, state| {
            let hecate = &mut state.hecate;
            if !hecate.advance_blobs() {
                hecate.blobs.ticking.set(false);
                return TimeoutAction::Drop;
            }
            // A period after this one was due, so the pace doesn't drift; after now when
            // that has passed already, so no two steps come closer than a period.
            let (period, now) = (hecate.blobs_period(), Instant::now());
            TimeoutAction::ToInstant(if due + period > now {
                due + period
            } else {
                now + period
            })
        });
        if let Err(e) = started {
            tracing::warn!("error starting the blobs' timer: {e}");
            self.blobs.ticking.set(false);
        }
    }

    /// Move every output's blobs on by the time that has passed and ask for a frame.
    /// False when none of them moves (the timer stops until one is drawn moving again).
    pub fn advance_blobs(&mut self) -> bool {
        let now = self.wm.clock.now();
        let outputs: Vec<Output> = self.wm.outputs().cloned().collect();
        let mut moved = Vec::new();
        let mut moving = false;
        {
            let cfg = &self.config.background.blobs;
            let mut all = self.blobs.outputs.borrow_mut();
            all.retain(|name, _| outputs.iter().any(|o| o.name() == *name));
            for output in &outputs {
                let Some(blobs) = all.get_mut(&output.name()) else {
                    continue;
                };
                if self.blobs_paused(output).is_some() {
                    blobs.stepped = None;
                    continue;
                }
                let seen = (blobs.texture.as_ref()).is_some_and(|(.., version)| *version == blobs.version);
                blobs.unseen = if seen { 0 } else { blobs.unseen.saturating_add(1) };
                if blobs.unseen > UNSEEN_STEPS {
                    blobs.stepped = None;
                    continue;
                }
                moving = true;
                // The first tick after a rest only starts the clock.
                let dt = match blobs.stepped.replace(now) {
                    Some(last) => now.saturating_sub(last).as_secs_f32().min(MAX_STEP),
                    None => 0.,
                };
                if dt > 0. {
                    blobs.sim.step(dt, cfg);
                    blobs.version = blobs.version.wrapping_add(1);
                    moved.push(output.clone());
                }
            }
        }
        for output in moved {
            self.queue_redraw(&output);
        }
        moving
    }

    /// Every output's simulation as it is now.
    pub fn export_blobs(&self) -> ipc::BlobsState {
        let all = self.blobs.outputs.borrow();
        let mut outputs: Vec<_> = all.iter().map(|(name, blobs)| blobs.sim.export(name)).collect();
        outputs.sort_by(|a, b| a.output.cmp(&b.output));
        ipc::BlobsState { outputs }
    }

    /// Carry on from exported simulations: at once on the outputs that have blobs, and
    /// from their first frame on the ones still to come.
    pub fn import_blobs(&mut self, state: ipc::BlobsState) {
        {
            let mut all = self.blobs.outputs.borrow_mut();
            let mut waiting = self.blobs.waiting.borrow_mut();
            for saved in state.outputs {
                match all.get_mut(&saved.output) {
                    Some(blobs) => {
                        let (w, h) = blobs.sim.size();
                        blobs.sim = Sim::restore(&saved, w, h);
                        blobs.version = blobs.version.wrapping_add(1);
                        blobs.stepped = None;
                    }
                    None => {
                        waiting.insert(saved.output.clone(), saved);
                    }
                }
            }
        }
        // Whatever the file said has been overtaken.
        self.blobs.resumed.set(true);
        self.queue_redraw_all();
    }

    /// `background.blobs.resume`, read once, before the first blobs are made.
    fn read_blobs_to_resume(&self) {
        if self.blobs.resumed.replace(true) {
            return;
        }
        let Some(path) = &self.config.background.blobs.resume else {
            return;
        };
        let state = match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice::<ipc::BlobsState>(&bytes),
            // Nothing to carry on from: the usual start.
            Err(_) => return,
        };
        match state {
            Ok(state) => {
                let mut waiting = self.blobs.waiting.borrow_mut();
                for saved in state.outputs {
                    waiting.entry(saved.output.clone()).or_insert(saved);
                }
            }
            Err(e) => tracing::warn!("error reading the blobs to resume from {path}: {e}"),
        }
    }

    /// Write the simulations to `background.blobs.save`, for the next compositor. Called
    /// after the last frame.
    pub fn save_blobs(&self) {
        let cfg = &self.config.background.blobs;
        let Some(path) = cfg.save.as_ref().filter(|_| cfg.enable) else {
            return;
        };
        let state = self.export_blobs();
        if state.outputs.is_empty() {
            return;
        }
        let tmp = format!("{path}.tmp");
        let written = serde_json::to_vec(&state)
            .map_err(std::io::Error::other)
            .and_then(|bytes| std::fs::write(&tmp, bytes))
            .and_then(|()| std::fs::rename(&tmp, path));
        if let Err(e) = written {
            tracing::warn!("error saving the blobs to {path}: {e}");
        }
    }

    pub fn blobs_status(&self) -> ipc::BlobsStatus {
        let cfg = &self.config.background.blobs;
        let all = self.blobs.outputs.borrow();
        let mut outputs: Vec<_> = self
            .wm
            .outputs()
            .filter_map(|output| {
                let blobs = all.get(&output.name())?;
                let paused = self.blobs_paused(output);
                Some(ipc::BlobsOutputStatus {
                    output: output.name(),
                    width: blobs.sim.size().0 as u32,
                    height: blobs.sim.size().1 as u32,
                    cell: blobs.cell as u32,
                    blobs: blobs.sim.blobs().len(),
                    moving: paused.is_none(),
                    paused: paused.map(str::to_owned),
                })
            })
            .collect();
        outputs.sort_by(|a, b| a.output.cmp(&b.output));
        ipc::BlobsStatus {
            enabled: cfg.enable,
            still: cfg.still,
            outputs,
        }
    }

    /// An output's simulation (tests).
    #[cfg(test)]
    pub fn blobs_sim(&self, output: &str) -> Option<Sim> {
        self.blobs.outputs.borrow().get(output).map(|b| b.sim.clone())
    }

    /// Whether the timer that steps the simulations is running (tests).
    #[cfg(test)]
    pub fn blobs_ticking(&self) -> bool {
        self.blobs.ticking.get()
    }
}

impl State {
    /// The `blobs` IPC request.
    pub fn ipc_blobs(&mut self, request: ipc::Blobs) -> ipc::Reply {
        let value = |v: Result<serde_json::Value, serde_json::Error>| match v {
            Ok(v) => ipc::Reply::ok(ipc::Payload::Value(v)),
            Err(e) => ipc::Reply::error(e.to_string()),
        };
        match request {
            ipc::Blobs::Status => value(serde_json::to_value(self.hecate.blobs_status())),
            ipc::Blobs::Enable(on) => match self.set_override("background.blobs.enable", on.into()) {
                Ok(()) => ipc::Reply::unit(),
                Err(e) => ipc::Reply::error(e),
            },
            ipc::Blobs::Export => value(serde_json::to_value(self.hecate.export_blobs())),
            ipc::Blobs::Import(state) => {
                self.hecate.import_blobs(state);
                ipc::Reply::unit()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> BlobsConfig {
        BlobsConfig::default()
    }

    fn blob(x: f32, y: f32, r: f32, vx: f32, vy: f32) -> ipc::Blob {
        ipc::Blob { x, y, r, vx, vy }
    }

    /// Nothing drifts in.
    fn no_refill() -> BlobsConfig {
        BlobsConfig {
            count_min: 0,
            count_max: 0,
            ..cfg()
        }
    }

    fn sim_of(w: i32, h: i32, blobs: Vec<ipc::Blob>) -> Sim {
        Sim {
            w,
            h,
            blobs,
            rng: Rng(7),
            time: 0.,
        }
    }

    #[test]
    fn a_grid_gets_its_share_of_blobs() {
        // The board: 1920x1080 in cells of 6.
        let sim = Sim::new(320, 180, &cfg(), 1);
        assert_eq!(sim.blobs().len(), 60, "320 * 180 / 700 is 82, cut to count_max");
        assert_eq!(Sim::new(64, 36, &cfg(), 1).blobs().len(), 6, "count_min");
        assert_eq!(Sim::new(200, 100, &cfg(), 1).blobs().len(), 29);
        for b in sim.blobs() {
            assert!((0. ..320.).contains(&b.x) && (0. ..180.).contains(&b.y));
            assert!((RADIUS_MIN..=RADIUS_START).contains(&b.r));
            let speed = b.vx.hypot(b.vy);
            assert!((1.5 - 1e-3..=6. + 1e-3).contains(&speed), "speed {speed}");
        }
    }

    #[test]
    fn the_same_seed_is_the_same_simulation() {
        let (mut a, mut b) = (Sim::new(160, 90, &cfg(), 42), Sim::new(160, 90, &cfg(), 42));
        for _ in 0..200 {
            a.step(1. / 30., &cfg());
            b.step(1. / 30., &cfg());
        }
        assert_eq!(a, b);
        assert_ne!(a, Sim::new(160, 90, &cfg(), 43));
    }

    #[test]
    fn overlapping_blobs_merge_with_their_area_and_momentum() {
        // The centre of the small one is inside the big one.
        let mut sim = sim_of(
            400,
            400,
            vec![blob(200., 200., 4., 4., 0.), blob(203., 200., 3., -1., 3.)],
        );
        sim.step(0., &no_refill());
        assert_eq!(sim.blobs().len(), 1);
        let b = sim.blobs()[0];
        assert!((b.r - 5.).abs() < 1e-5, "sqrt(16 + 9), was {}", b.r);
        assert!((b.x - (200. * 16. + 203. * 9.) / 25.).abs() < 1e-4);
        assert_eq!(b.y, 200.);
        assert!((b.vx - (4. * 16. - 9.) / 25.).abs() < 1e-5);
        assert!((b.vy - 27. / 25.).abs() < 1e-5);
    }

    #[test]
    fn blobs_apart_or_too_big_together_do_not_merge() {
        // Overlapping, but neither centre is inside the other.
        let mut sim = sim_of(
            400,
            400,
            vec![blob(200., 200., 4., 0., 2.), blob(205., 200., 4., 0., 2.)],
        );
        sim.step(0., &no_refill());
        assert_eq!(sim.blobs().len(), 2);
        // sqrt(20² + 20²) is over radius_max.
        let mut sim = sim_of(
            400,
            400,
            vec![blob(200., 200., 20., 0., 2.), blob(201., 200., 20., 0., 2.)],
        );
        sim.step(0., &no_refill());
        assert_eq!(sim.blobs().len(), 2);
    }

    #[test]
    fn a_head_on_merge_keeps_moving() {
        let mut sim = sim_of(
            400,
            400,
            vec![blob(200., 200., 3., 2., 0.), blob(201., 200., 3., -2., 0.)],
        );
        sim.step(0., &no_refill());
        assert_eq!(sim.blobs().len(), 1);
        let b = sim.blobs()[0];
        assert!((b.vx.hypot(b.vy) - 1.5).abs() < 1e-4, "speed_min");
    }

    #[test]
    fn blobs_leave_and_others_drift_in() {
        let config = cfg();
        // Out of sight and leaving, out of sight but coming back, and on the grid.
        let mut sim = sim_of(
            100,
            100,
            vec![
                blob(-20., 50., 2., -1., 0.),
                blob(-20., 50., 2., 1., 0.),
                blob(50., 50., 2., 1., 0.),
            ],
        );
        sim.step(0., &config);
        // 100 * 100 / 700 is 14 blobs; under 85 % of that new ones come: 12 of them.
        assert_eq!(sim.blobs().len(), 12);
        assert!(!sim.blobs().contains(&blob(-20., 50., 2., -1., 0.)));
        for b in &sim.blobs()[2..] {
            let outside = b.x < 0. || b.x > 100. || b.y < 0. || b.y > 100.;
            assert!(outside, "a new blob starts off the grid, not at {}, {}", b.x, b.y);
            // And comes closer to the middle.
            let (before, after) = (
                (b.x - 50.).hypot(b.y - 50.),
                (b.x + b.vx * 0.1 - 50.).hypot(b.y + b.vy * 0.1 - 50.),
            );
            assert!(after < before);
        }
        // And so it stays: what merges or leaves is made up for.
        for _ in 0..3000 {
            sim.step(1. / 30., &config);
            assert_eq!(sim.blobs().len(), 12);
        }
    }

    #[test]
    fn a_new_grid_keeps_the_blobs_in_place() {
        let mut sim = sim_of(320, 180, vec![blob(160., 90., 4., 1., 2.)]);
        sim.resize(640, 360);
        assert_eq!(sim.blobs()[0], blob(320., 180., 4., 1., 2.));
        assert_eq!(sim.size(), (640, 360));
    }

    #[test]
    fn an_exported_simulation_carries_on_the_same() {
        let config = cfg();
        let mut a = Sim::new(320, 180, &config, 9);
        for _ in 0..100 {
            a.step(1. / 30., &config);
        }
        let json = serde_json::to_string(&a.export("DP-1")).unwrap();
        let saved: ipc::BlobsOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(saved.output, "DP-1");
        let mut b = Sim::restore(&saved, 320, 180);
        assert_eq!(a, b);
        for _ in 0..500 {
            a.step(1. / 30., &config);
            b.step(1. / 30., &config);
        }
        assert_eq!(a, b, "what drifts in next is the same too");
        assert!((a.time() - 20.).abs() < 1e-3);
    }

    #[test]
    fn a_restored_simulation_drops_what_makes_no_sense() {
        let saved = ipc::BlobsOutput {
            output: "DP-1".into(),
            width: 160,
            height: 90,
            time: f64::NAN,
            seed: 1,
            blobs: vec![
                blob(80., 45., 4., 1., 1.),
                blob(f32::NAN, 0., 4., 1., 1.),
                blob(0., 0., -1., 1., 1.),
            ],
        };
        let sim = Sim::restore(&saved, 320, 180);
        assert_eq!(sim.blobs(), &[blob(160., 90., 4., 1., 1.)]);
        assert_eq!(sim.time(), 0.);
    }

    #[test]
    fn the_bayer_thresholds_are_two_rounds_of_the_2x2_matrix() {
        // What the shader computes, since GLSL ES 1.00 has no constant arrays.
        let bayer2 = |x: usize, y: usize| 2 * x + 3 * y - 4 * x * y;
        for y in 0..4 {
            for x in 0..4 {
                let computed = 4 * bayer2(x % 2, y % 2) + bayer2(x / 2, y / 2);
                assert_eq!(computed, BAYER[y * 4 + x] as usize, "at {x}, {y}");
            }
        }
    }

    #[test]
    fn one_blob_is_a_core_in_a_dithered_halo() {
        let sim = sim_of(64, 64, vec![blob(32., 32., 6., 0., 0.)]);
        let tones = sim.tones();
        let at = |x: usize, y: usize| tones[y * 64 + x];
        // The field is r² / d²: 1.4 (the core, whatever the threshold) inside 0.845 r.
        assert_eq!(at(32, 32), 2);
        assert_eq!(at(36, 32), 2);
        // Under 0.3 (no halo at all) beyond 1.83 r, and nothing outside the 3 r box.
        assert_eq!(at(32 + 12, 32), 0);
        assert_eq!(at(2, 2), 0);
        // Between the two, every tone shows up.
        let ring: Vec<u8> = (0..64 * 64)
            .filter(|i| {
                let d = ((i % 64) as f32 + 0.5 - 32.).hypot((i / 64) as f32 + 0.5 - 32.);
                (6. ..10.).contains(&d)
            })
            .map(|i| tones[i])
            .collect();
        for tone in 0..3 {
            assert!(ring.contains(&tone), "tone {tone} in the ring");
        }
    }

    #[test]
    fn a_grid_covers_the_output() {
        assert_eq!(grid_for((1920, 1080).into(), 6), (320, 180));
        assert_eq!(grid_for((3840, 2160).into(), 6), (640, 360));
        assert_eq!(grid_for((1366, 768).into(), 6), (228, 128));
        assert_eq!(grid_for((2560, 1440).into(), 9), (285, 160));
    }
}
