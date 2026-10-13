//! Parsing of key combos like `Mod+Shift+Return`, `Super` (tap) or `Mod+BTN_LEFT`.

use std::fmt;
use std::str::FromStr;

use bitflags::bitflags;
use smithay::input::keyboard::{Keysym, keysyms, xkb};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Modifiers: u8 {
        const CTRL = 1;
        const SHIFT = 1 << 1;
        const ALT = 1 << 2;
        const SUPER = 1 << 3;
        /// Resolved to `SUPER` on a real session and `ALT` when nested.
        const MOD = 1 << 4;
    }
}

impl Modifiers {
    /// Replace `MOD` with the concrete modifier.
    pub fn resolve(self, mod_key: Modifiers) -> Modifiers {
        if self.contains(Modifiers::MOD) {
            (self - Modifiers::MOD) | mod_key
        } else {
            self
        }
    }

    pub fn from_xkb(state: &smithay::input::keyboard::ModifiersState) -> Modifiers {
        let mut m = Modifiers::empty();
        if state.ctrl {
            m |= Modifiers::CTRL;
        }
        if state.shift {
            m |= Modifiers::SHIFT;
        }
        if state.alt {
            m |= Modifiers::ALT;
        }
        if state.logo {
            m |= Modifiers::SUPER;
        }
        m
    }

    fn parse_one(s: &str) -> Option<Modifiers> {
        Some(match s.to_ascii_lowercase().as_str() {
            "mod" => Modifiers::MOD,
            "super" | "logo" | "win" | "meta" => Modifiers::SUPER,
            "alt" => Modifiers::ALT,
            "ctrl" | "control" => Modifiers::CTRL,
            "shift" => Modifiers::SHIFT,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Trigger {
    Keysym(Keysym),
    /// evdev button code (BTN_LEFT = 0x110, ...).
    Button(u32),
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
    /// A modifier pressed and released on its own.
    ModTap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub modifiers: Modifiers,
    pub trigger: Trigger,
}

pub const BTN_LEFT: u32 = 0x110;
pub const BTN_RIGHT: u32 = 0x111;
pub const BTN_MIDDLE: u32 = 0x112;
pub const BTN_SIDE: u32 = 0x113;
pub const BTN_EXTRA: u32 = 0x114;
pub const BTN_FORWARD: u32 = 0x115;
pub const BTN_BACK: u32 = 0x116;

fn parse_button(s: &str) -> Option<u32> {
    Some(match s.to_ascii_uppercase().as_str() {
        "BTN_LEFT" | "MOUSELEFT" => BTN_LEFT,
        "BTN_RIGHT" | "MOUSERIGHT" => BTN_RIGHT,
        "BTN_MIDDLE" | "MOUSEMIDDLE" => BTN_MIDDLE,
        "BTN_SIDE" | "MOUSEBACK" => BTN_SIDE,
        "BTN_EXTRA" | "MOUSEFORWARD" => BTN_EXTRA,
        "BTN_FORWARD" => BTN_FORWARD,
        "BTN_BACK" => BTN_BACK,
        _ => return None,
    })
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum KeyParseError {
    #[error("empty key combo")]
    Empty,
    #[error("unknown modifier {0:?} in key combo")]
    UnknownModifier(String),
    #[error("unknown key {0:?} (use an xkb keysym name like \"Return\", \"a\", \"XF86AudioMute\")")]
    UnknownKey(String),
}

impl FromStr for KeyCombo {
    type Err = KeyParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('+').map(str::trim).collect();
        if parts.iter().any(|p| p.is_empty()) {
            return Err(KeyParseError::Empty);
        }
        let (key, mods) = parts.split_last().ok_or(KeyParseError::Empty)?;

        let mut modifiers = Modifiers::empty();
        for m in mods {
            modifiers |=
                Modifiers::parse_one(m).ok_or_else(|| KeyParseError::UnknownModifier(m.to_string()))?;
        }

        // A lone modifier name means a tap bind.
        if let Some(m) = Modifiers::parse_one(key) {
            return Ok(KeyCombo {
                modifiers: modifiers | m,
                trigger: Trigger::ModTap,
            });
        }

        let trigger = match key.to_ascii_lowercase().as_str() {
            "wheelscrollup" => Trigger::WheelUp,
            "wheelscrolldown" => Trigger::WheelDown,
            "wheelscrollleft" => Trigger::WheelLeft,
            "wheelscrollright" => Trigger::WheelRight,
            _ => {
                if let Some(b) = parse_button(key) {
                    Trigger::Button(b)
                } else {
                    // Letters match case-insensitively: binds use the unshifted keysym.
                    let lowered;
                    let key = if key.len() == 1 && key.as_bytes()[0].is_ascii_alphabetic() {
                        lowered = key.to_ascii_lowercase();
                        lowered.as_str()
                    } else {
                        key
                    };
                    let sym = xkb::keysym_from_name(key, xkb::KEYSYM_NO_FLAGS);
                    let sym = if sym.raw() == keysyms::KEY_NoSymbol {
                        xkb::keysym_from_name(key, xkb::KEYSYM_CASE_INSENSITIVE)
                    } else {
                        sym
                    };
                    if sym.raw() == keysyms::KEY_NoSymbol {
                        return Err(KeyParseError::UnknownKey(key.to_string()));
                    }
                    Trigger::Keysym(sym)
                }
            }
        };

        Ok(KeyCombo { modifiers, trigger })
    }
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = [
            (Modifiers::MOD, "Mod"),
            (Modifiers::SUPER, "Super"),
            (Modifiers::CTRL, "Ctrl"),
            (Modifiers::ALT, "Alt"),
            (Modifiers::SHIFT, "Shift"),
        ];
        let mut first = true;
        let mut sep = |f: &mut fmt::Formatter<'_>| {
            if !first {
                f.write_str("+")?;
            }
            first = false;
            Ok::<_, fmt::Error>(())
        };
        let tap = self.trigger == Trigger::ModTap;
        for (m, n) in names {
            if self.modifiers.contains(m) {
                sep(f)?;
                f.write_str(n)?;
            }
        }
        if tap {
            return Ok(());
        }
        sep(f)?;
        match self.trigger {
            Trigger::Keysym(sym) => f.write_str(&xkb::keysym_get_name(sym)),
            Trigger::Button(BTN_LEFT) => f.write_str("BTN_LEFT"),
            Trigger::Button(BTN_RIGHT) => f.write_str("BTN_RIGHT"),
            Trigger::Button(BTN_MIDDLE) => f.write_str("BTN_MIDDLE"),
            Trigger::Button(b) => write!(f, "BTN_{b:#x}"),
            Trigger::WheelUp => f.write_str("WheelScrollUp"),
            Trigger::WheelDown => f.write_str("WheelScrollDown"),
            Trigger::WheelLeft => f.write_str("WheelScrollLeft"),
            Trigger::WheelRight => f.write_str("WheelScrollRight"),
            Trigger::ModTap => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_combos() {
        let c: KeyCombo = "Mod+Shift+Return".parse().unwrap();
        assert_eq!(c.modifiers, Modifiers::MOD | Modifiers::SHIFT);
        assert_eq!(c.trigger, Trigger::Keysym(Keysym::Return));

        let c: KeyCombo = "Super".parse().unwrap();
        assert_eq!(c.trigger, Trigger::ModTap);
        assert_eq!(c.modifiers, Modifiers::SUPER);

        let c: KeyCombo = "Super_L".parse().unwrap();
        assert_eq!(c.trigger, Trigger::Keysym(Keysym::Super_L));

        let c: KeyCombo = "Mod+BTN_LEFT".parse().unwrap();
        assert_eq!(c.trigger, Trigger::Button(BTN_LEFT));

        let c: KeyCombo = "XF86AudioRaiseVolume".parse().unwrap();
        assert_eq!(c.modifiers, Modifiers::empty());

        assert!(matches!(
            "Mod+Nope".parse::<KeyCombo>(),
            Err(KeyParseError::UnknownKey(_))
        ));
        assert!(matches!(
            "Hyper+a".parse::<KeyCombo>(),
            Err(KeyParseError::UnknownModifier(_))
        ));
    }

    #[test]
    fn displays_combos() {
        let c: KeyCombo = "Mod+Ctrl+Left".parse().unwrap();
        assert_eq!(c.to_string(), "Mod+Ctrl+Left");
        let c: KeyCombo = "Super".parse().unwrap();
        assert_eq!(c.to_string(), "Super");
    }

    #[test]
    fn resolves_mod() {
        let m = (Modifiers::MOD | Modifiers::SHIFT).resolve(Modifiers::ALT);
        assert_eq!(m, Modifiers::ALT | Modifiers::SHIFT);
    }
}
