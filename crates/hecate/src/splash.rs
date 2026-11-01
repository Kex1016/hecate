//! The login splash (`splash.enable`). When the session locks or ends, each output's desktop is
//! saved as a picture: the wallpaper and the shell's layers, no windows, overlays or cursor.
//! The next session shows it from its first frame, so there's no bare screen while the shell
//! starts, and fades it out once the shell says it's ready (action `splash_done`), or after
//! `TIMEOUT`. hecate-greetd shows the same pictures, so a login fades straight into them.

use std::collections::HashMap;
use std::path::PathBuf;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::Context as _;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::output::Output;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::utils::{Physical, Scale, Size, Transform};

use crate::state::Hecate;
use crate::wm::Wm;

/// How long the splash waits for the shell before fading anyway.
const TIMEOUT: Duration = Duration::from_secs(20);
const FADE: Duration = Duration::from_millis(400);

/// One output's splash, and when it started fading.
pub struct Splash {
    buffer: MemoryRenderBuffer,
    fading_since: Option<Duration>,
}

type Decoded = anyhow::Result<(Vec<u8>, u32, u32)>;

/// Pictures decoded on threads from startup, by output name. Decoding a monitor's PNG takes
/// a good fraction of a second, which on the main thread would sit between the backend coming
/// up and the first frame: that is time the screen shows whatever the previous compositor
/// left, or nothing. Started before the backend, the decoding overlaps with it.
#[derive(Default)]
pub struct Decoding(HashMap<String, JoinHandle<Decoded>>);

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
        if !self.config.splash.enable {
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
        if !self.config.splash.enable || self.splash_done {
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
                fading_since: None,
            },
        );
    }

    /// Fade out at the latest after `TIMEOUT`, in case the shell never says it's ready.
    pub fn start_splash_timeout(&mut self) {
        if !self.config.splash.enable {
            return;
        }
        let _ = self
            .event_loop
            .insert_source(Timer::from_duration(TIMEOUT), |_, _, state| {
                state.hecate.finish_splash();
                TimeoutAction::Drop
            });
    }

    /// The shell is ready (or it's too late): fade every splash out.
    pub fn finish_splash(&mut self) {
        if self.splash_done {
            return;
        }
        self.splash_done = true;
        let now = self.wm.clock.now();
        for splash in self.splash.values_mut() {
            splash.fading_since = Some(now);
        }
        self.queue_redraw_all();
    }

    /// Keeps frames coming while the output's splash fades; drops it when done.
    pub fn splash_animating(&mut self, output: &Output) -> bool {
        let now = self.wm.clock.now();
        let Some(since) = self.splash.get(output).and_then(|s| s.fading_since) else {
            return false;
        };
        if self.wm.clock.instant || now >= since + FADE {
            self.splash.remove(output);
            return false;
        }
        true
    }

    /// The splash element, on top of everything else.
    pub fn splash_element(
        &self,
        renderer: &mut GlesRenderer,
        output: &Output,
    ) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
        let splash = self.splash.get(output)?;
        let alpha = splash.fading_since.map_or(1., |since| {
            1. - (self.wm.clock.now().saturating_sub(since).as_secs_f64() / FADE.as_secs_f64()).min(1.)
        });
        MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            (0., 0.),
            &splash.buffer,
            Some(alpha as f32),
            None,
            Some(Wm::output_geometry(output).size),
            Kind::Unspecified,
        )
        .ok()
    }

    /// Save every output's desktop for the next login's splash. Encoding runs on threads; join
    /// the handles when the process is about to exit.
    pub fn save_snapshots(&self, renderer: &mut GlesRenderer) -> Vec<JoinHandle<()>> {
        if !self.config.splash.enable || !self.config.splash.save {
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
    fn png_round_trip() {
        let dir = std::env::temp_dir().join(format!("hecate-splash-{}", std::process::id()));
        let path = dir.join("out.png");
        let pixels: Vec<u8> = (0..4 * 3 * 2).map(|i| i as u8).collect();
        encode(&path, &pixels, (3, 2).into()).unwrap();
        assert_eq!(decode(&path).unwrap(), (pixels, 3, 2));
        let _ = std::fs::remove_dir_all(dir);
    }
}
