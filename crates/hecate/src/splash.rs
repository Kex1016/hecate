//! The login splash (`splash.enable`). When the session locks or ends, each output's desktop is
//! saved as a picture: the wallpaper and the shell's layers, no windows, overlays or cursor.
//! The next session shows it from its first frame, so there's no bare screen while the shell
//! starts, and takes it away once the shell says it's ready (action `splash_done`), or after
//! `TIMEOUT`: with a fade, a cut or a pixelate (`splash.handover`).
//!
//! With the blob background (`background.blobs.enable`) there is no splash, whatever
//! `splash.enable` says. The blobs are on screen from the first frame and carry on from where the
//! login screen left them (`background.blobs.resume`), so no picture is loaded, saved or waited
//! behind, and the first frame never holds for the shell.

use std::collections::HashMap;
use std::path::PathBuf;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::Context as _;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement};
use smithay::backend::renderer::element::{Id, Kind};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::utils::CommitCounter;
use smithay::output::Output;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::utils::{Buffer, Physical, Scale, Size, Transform};

use crate::config::SplashHandover;
use crate::render::HecateElement;
use crate::render::pixelate::{self, PixelateElement};
use crate::state::Hecate;
use crate::wm::Wm;

/// How long the splash waits for the shell before fading anyway.
const TIMEOUT: Duration = Duration::from_secs(20);
/// How long `splash.hold` keeps the first frame off screen waiting for the shell.
const HOLD: Duration = Duration::from_secs(5);
const FADE: Duration = Duration::from_millis(400);
/// The pixelate handover: this long, in this many steps, each with blocks twice the size of
/// the step before, up to `PIXELATE_CELL` logical pixels.
const PIXELATE: Duration = Duration::from_millis(500);
const PIXELATE_STEPS: u32 = 4;
const PIXELATE_CELL: u32 = 16;

/// One output's splash, and when it started going away.
pub struct Splash {
    buffer: MemoryRenderBuffer,
    /// The picture's size in pixels.
    size: Size<i32, Buffer>,
    /// The element's id while it pixelates.
    id: Id,
    fading_since: Option<Duration>,
}

/// Block size, in logical pixels, of a pixelate handover `elapsed` into it: 2, 4, 8, 16.
fn pixelate_cell(elapsed: Duration) -> u32 {
    let step = (elapsed.as_nanos() * PIXELATE_STEPS as u128 / PIXELATE.as_nanos()) as u32;
    let step = step.min(PIXELATE_STEPS - 1);
    PIXELATE_CELL >> (PIXELATE_STEPS - 1 - step)
}

type Decoded = anyhow::Result<(Vec<u8>, u32, u32)>;

/// Pictures decoded on threads from startup, by output name. Decoding a monitor's PNG takes
/// a good fraction of a second, which on the main thread would sit between the backend coming
/// up and the first frame: that is time the screen shows whatever the previous compositor
/// left, or nothing. Started before the backend, the decoding overlaps with it.
#[derive(Default)]
pub struct Decoding(pub(crate) HashMap<String, JoinHandle<Decoded>>);

/// `$XDG_STATE_HOME/hecate/snapshots`, where `<output>.png` are kept.
pub fn dir() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(state.join("hecate/snapshots"))
}

/// The output's size in pixels, as its snapshot is taken.
fn physical_size(output: &Output) -> Size<i32, Physical> {
    let scale = Scale::from(output.current_scale().fractional_scale());
    Wm::output_geometry(output).size.to_physical_precise_round(scale)
}

pub fn decode(path: &std::path::Path) -> anyhow::Result<(Vec<u8>, u32, u32)> {
    let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?)).read_info()?;
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels)?;
    anyhow::ensure!(
        info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight,
        "not 8-bit RGBA"
    );
    pixels.truncate(info.buffer_size());
    Ok((pixels, info.width, info.height))
}

fn encode(path: &std::path::Path, pixels: &[u8], size: Size<i32, Physical>) -> anyhow::Result<()> {
    std::fs::create_dir_all(path.parent().context("no parent")?)?;
    let tmp = path.with_extension("png.tmp");
    {
        let mut encoder = png::Encoder::new(
            std::io::BufWriter::new(std::fs::File::create(&tmp)?),
            size.w as u32,
            size.h as u32,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        encoder.write_header()?.write_image_data(pixels)?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

impl Hecate {
    /// Whether the splash is in use: `splash.enable`, and no blob background. What the config
    /// file says about the blobs counts, not `hecate msg blobs off`: a shell showing a wallpaper
    /// turns them off once it is up, and the next start has them again.
    fn splash_in_use(&self) -> bool {
        self.config.splash.enable && !self.file_config.background.blobs.enable
    }

    /// `splash.dir`, else the default (`dir()`).
    fn splash_dir(&self) -> Option<PathBuf> {
        self.config
            .splash
            .dir
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| self.snapshot_dir.clone())
    }

    /// `<splash.dir or the default>/<output>.png`
    fn splash_path(&self, output: &Output) -> Option<PathBuf> {
        Some(self.splash_dir()?.join(format!("{}.png", output.name())))
    }

    /// Start decoding every picture in the splash directory, one thread each, so they are
    /// ready (or nearly) by the time the outputs are. `load_splash` picks them up.
    pub fn start_splash_decoding(&mut self) {
        if self.config.splash.enable && !self.splash_in_use() {
            tracing::info!("no splash: the blob background is on screen from the first frame");
        }
        if !self.splash_in_use() {
            return;
        }
        let Some(dir) = self.splash_dir() else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".png"))
            else {
                continue;
            };
            let name = name.to_string();
            let thread = std::thread::Builder::new().name(format!("splash {name}")).spawn({
                let path = path.clone();
                move || decode(&path)
            });
            if let Ok(handle) = thread {
                self.splash_decoding.0.insert(name, handle);
            }
        }
    }

    /// Show the output's snapshot until the shell is ready, if there's one that fits it.
    pub fn load_splash(&mut self, output: &Output) {
        if !self.splash_in_use() || self.splash_done {
            return;
        }
        let Some(path) = self.splash_path(output) else {
            return;
        };
        let decoded = match self.splash_decoding.0.remove(&output.name()) {
            Some(handle) => handle
                .join()
                .unwrap_or_else(|_| Err(anyhow::anyhow!("the decoding thread panicked"))),
            None => decode(&path),
        };
        let (pixels, w, h) = match decoded {
            Ok(d) => d,
            Err(e) => {
                tracing::info!("no splash for {} ({}): {e:#}", output.name(), path.display());
                return;
            }
        };
        // A picture of a different mode or scale would be stretched: show nothing instead.
        let size = physical_size(output);
        if size != Size::from((w as i32, h as i32)) {
            tracing::info!(
                "no splash for {}: {} is {w}x{h}, the output {}x{}",
                output.name(),
                path.display(),
                size.w,
                size.h
            );
            return;
        }
        tracing::info!("splash for {} from {}", output.name(), path.display());
        let buffer = MemoryRenderBuffer::from_slice(
            &pixels,
            Fourcc::Abgr8888,
            (w as i32, h as i32),
            1,
            Transform::Normal,
            None,
        );
        self.splash.insert(
            output.clone(),
            Splash {
                buffer,
                size: (w as i32, h as i32).into(),
                id: Id::new(),
                fading_since: None,
            },
        );
    }

    /// Fade out at the latest after `TIMEOUT`, in case the shell never says it's ready, and
    /// stop holding the first frame (`splash.hold`) after `HOLD`.
    pub fn start_splash_timeout(&mut self) {
        if !self.splash_in_use() {
            return;
        }
        let _ = self
            .event_loop
            .insert_source(Timer::from_duration(TIMEOUT), |_, _, state| {
                state.hecate.finish_splash();
                TimeoutAction::Drop
            });
        if self.config.splash.hold {
            let _ = self
                .event_loop
                .insert_source(Timer::from_duration(HOLD), |_, _, state| {
                    if !state.hecate.splash_done {
                        tracing::info!(
                            "the shell took more than {HOLD:?}: putting the first frame on screen"
                        );
                    }
                    state.hecate.splash_hold_over = true;
                    state.hecate.queue_redraw_all();
                    TimeoutAction::Drop
                });
        }
    }

    /// Whether the output's first frame should stay off screen for now (`splash.hold`): it
    /// would only be the splash, which is what the previous compositor left there. Until the
    /// shell is ready or `HOLD` runs out; a splash that's fading is drawn.
    pub fn splash_holds(&self, output: &Output) -> bool {
        self.splash_in_use()
            && self.config.splash.hold
            && !self.splash_done
            && !self.splash_hold_over
            && self.splash.get(output).is_some_and(|s| s.fading_since.is_none())
    }

    /// How long the handover takes: nothing for a cut, and for any of them when animations
    /// are off (Game Mode, `animations.enable = false`).
    fn splash_handover(&self) -> Option<Duration> {
        if self.wm.clock.instant || !self.wm.anim.enable {
            return None;
        }
        match self.config.splash.handover {
            SplashHandover::Fade => Some(FADE),
            SplashHandover::Pixelate => Some(PIXELATE),
            SplashHandover::Cut => None,
        }
    }

    /// The shell is ready (or it's too late): take every splash away.
    pub fn finish_splash(&mut self) {
        if self.splash_done {
            return;
        }
        self.splash_done = true;
        if self.splash_handover().is_none() {
            self.splash.clear();
        }
        let now = self.wm.clock.now();
        for splash in self.splash.values_mut() {
            splash.fading_since = Some(now);
        }
        self.queue_redraw_all();
    }

    /// Keeps frames coming while the output's splash goes away; drops it when done.
    pub fn splash_animating(&mut self, output: &Output) -> bool {
        let now = self.wm.clock.now();
        let Some(since) = self.splash.get(output).and_then(|s| s.fading_since) else {
            return false;
        };
        if self.splash_handover().is_none_or(|d| now >= since + d) {
            self.splash.remove(output);
            return false;
        }
        true
    }

    /// The splash element, on top of everything else.
    pub fn splash_element(&self, renderer: &mut GlesRenderer, output: &Output) -> Option<HecateElement> {
        let splash = self.splash.get(output)?;
        let elapsed = splash
            .fading_since
            .map(|since| self.wm.clock.now().saturating_sub(since));
        let pixelate = self.config.splash.handover == SplashHandover::Pixelate;
        let alpha = match elapsed {
            Some(elapsed) if !pixelate => 1. - (elapsed.as_secs_f64() / FADE.as_secs_f64()).min(1.),
            _ => 1.,
        };
        let element = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            (0., 0.),
            &splash.buffer,
            Some(alpha as f32),
            None,
            Some(Wm::output_geometry(output).size),
            Kind::Unspecified,
        )
        .ok()?;
        match elapsed {
            Some(elapsed) if pixelate => {
                let scale = Scale::from(output.current_scale().fractional_scale());
                let cell = pixelate::cell_px(pixelate_cell(elapsed) as f64, scale);
                Some(HecateElement::PixelatedMemory(PixelateElement::new(
                    renderer,
                    element,
                    splash.id.clone(),
                    CommitCounter::from(cell as usize),
                    cell,
                    splash.size,
                )))
            }
            _ => Some(HecateElement::Memory(element)),
        }
    }

    /// Save every output's desktop for the next login's splash. Encoding runs on threads; join
    /// the handles when the process is about to exit.
    pub fn save_snapshots(&self, renderer: &mut GlesRenderer) -> Vec<JoinHandle<()>> {
        if !self.splash_in_use() || !self.config.splash.save {
            return Vec::new();
        }
        let mut handles = Vec::new();
        for output in self.wm.outputs() {
            let Some(path) = self.splash_path(output) else {
                continue;
            };
            let size = physical_size(output);
            let scale = Scale::from(output.current_scale().fractional_scale());
            let elements = self.render_elements_for(renderer, output, false, true);
            let pixels = match crate::render::capture::render_to_rgba(renderer, size, scale, &elements) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("couldn't capture {} for the splash: {e:#}", output.name());
                    continue;
                }
            };
            handles.push(std::thread::spawn(move || {
                if let Err(e) = encode(&path, &pixels, size) {
                    tracing::warn!("couldn't save {}: {e:#}", path.display());
                }
            }));
        }
        handles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixelate_handover_doubles_its_blocks() {
        let cells: Vec<u32> = [0, 124, 125, 250, 375, 499, 500, 900]
            .into_iter()
            .map(|ms| pixelate_cell(Duration::from_millis(ms)))
            .collect();
        assert_eq!(cells, [2, 2, 4, 8, 16, 16, 16, 16]);
    }

    #[test]
    fn png_round_trip() {
        let dir = std::env::temp_dir().join(format!("hecate-splash-{}", std::process::id()));
        let path = dir.join("out.png");
        let pixels: Vec<u8> = (0..4 * 3 * 2).map(|i| i as u8).collect();
        encode(&path, &pixels, (3, 2).into()).unwrap();
        assert_eq!(decode(&path).unwrap(), (pixels, 3, 2));
        let _ = std::fs::remove_dir_all(dir);
    }
}
