//! Cursor theme loading and cursor textures.
//!
//! Adapted from niri (https://github.com/YaLTeR/niri), GPL-3.0-or-later. The XCursor frame
//! handling is inspired by wayland-rs (MIT).

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::rc::Rc;

use anyhow::{Context, anyhow};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::input::pointer::CursorIcon;
use smithay::utils::{Physical, Point, Transform};
use xcursor::CursorTheme;
use xcursor::parser::{Image, parse_xcursor};

/// A default-looking `left_ptr` used when the theme has no cursor.
static FALLBACK_CURSOR_DATA: &[u8] = include_bytes!("../resources/cursor.rgba");

type XCursorCache = HashMap<(CursorIcon, i32), Option<Rc<XCursor>>>;
type TextureCache = HashMap<(CursorIcon, i32), Vec<MemoryRenderBuffer>>;

pub struct CursorManager {
    theme: CursorTheme,
    size: u32,
    named_cache: RefCell<XCursorCache>,
    textures: RefCell<TextureCache>,
}

impl CursorManager {
    pub fn new(theme: &str, size: u32) -> Self {
        crate::utils::set_child_env("XCURSOR_THEME", Some(theme));
        crate::utils::set_child_env("XCURSOR_SIZE", Some(&size.to_string()));
        CursorManager {
            theme: CursorTheme::load(theme),
            size,
            named_cache: Default::default(),
            textures: Default::default(),
        }
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn get(&self, icon: CursorIcon, scale: i32) -> Rc<XCursor> {
        self.get_named(icon, scale)
            .or_else(|| self.get_named(CursorIcon::Default, scale))
            .unwrap_or_else(|| Rc::new(fallback_cursor()))
    }

    fn get_named(&self, icon: CursorIcon, scale: i32) -> Option<Rc<XCursor>> {
        self.named_cache
            .borrow_mut()
            .entry((icon, scale))
            .or_insert_with(|| {
                let size = self.size as i32 * scale;
                let mut cursor = load_xcursor(&self.theme, icon.name(), size);
                if cursor.is_err() {
                    for name in icon.alt_names() {
                        cursor = load_xcursor(&self.theme, name, size);
                        if cursor.is_ok() {
                            break;
                        }
                    }
                }
                if icon == CursorIcon::Default && cursor.is_err() {
                    cursor = Ok(fallback_cursor());
                }
                if let Err(err) = &cursor {
                    tracing::debug!("error loading xcursor {}@{size}: {err:?}", icon.name());
                }
                cursor.ok().map(Rc::new)
            })
            .clone()
    }

    /// Texture for a frame of a named cursor.
    pub fn texture(&self, icon: CursorIcon, scale: i32, cursor: &XCursor, idx: usize) -> MemoryRenderBuffer {
        self.textures
            .borrow_mut()
            .entry((icon, scale))
            .or_insert_with(|| {
                cursor
                    .frames()
                    .iter()
                    .map(|frame| {
                        MemoryRenderBuffer::from_slice(
                            &frame.pixels_rgba,
                            Fourcc::Argb8888,
                            (frame.width as i32, frame.height as i32),
                            scale,
                            Transform::Normal,
                            None,
                        )
                    })
                    .collect()
            })[idx]
            .clone()
    }
}

fn load_xcursor(theme: &CursorTheme, name: &str, size: i32) -> anyhow::Result<XCursor> {
    let path = theme.load_icon(name).ok_or_else(|| anyhow!("no icon {name}"))?;
    let mut file = File::open(path).context("opening cursor file")?;
    let mut buf = vec![];
    file.read_to_end(&mut buf).context("reading cursor file")?;
    let mut images = parse_xcursor(&buf).context("parsing cursor file")?;
    let (width, height) = images
        .iter()
        .min_by_key(|image| (size - image.size as i32).abs())
        .map(|image| (image.width, image.height))
        .ok_or_else(|| anyhow!("empty cursor file"))?;
    images.retain(move |image| image.width == width && image.height == height);
    let animation_duration = images.iter().fold(0, |acc, image| acc + image.delay);
    Ok(XCursor {
        images,
        animation_duration,
    })
}

fn fallback_cursor() -> XCursor {
    XCursor {
        images: vec![Image {
            size: 32,
            width: 64,
            height: 64,
            xhot: 1,
            yhot: 1,
            delay: 0,
            pixels_rgba: Vec::from(FALLBACK_CURSOR_DATA),
            pixels_argb: vec![],
        }],
        animation_duration: 0,
    }
}

pub struct XCursor {
    images: Vec<Image>,
    animation_duration: u32,
}

impl XCursor {
    /// Frame to show at `millis` (wrapping).
    pub fn frame(&self, mut millis: u32) -> (usize, &Image) {
        if self.animation_duration == 0 {
            return (0, &self.images[0]);
        }
        millis %= self.animation_duration;
        let mut res = 0;
        for (i, img) in self.images.iter().enumerate() {
            if millis < img.delay {
                res = i;
                break;
            }
            millis -= img.delay;
        }
        (res, &self.images[res])
    }

    pub fn frames(&self) -> &[Image] {
        &self.images
    }

    pub fn is_animated(&self) -> bool {
        self.images.len() > 1
    }

    pub fn hotspot(image: &Image) -> Point<i32, Physical> {
        (image.xhot as i32, image.yhot as i32).into()
    }
}
