//! Bind values: what a key combo maps to.
//!
//! Accepted forms:
//! - `"close"` or `{ workspace = 3 }`: just an action
//! - `{ spawn = ["foo"], allow_when_locked = true }`: action plus flags
//! - `{ action = "toggle_keybinds", allow_when_inhibited = true }`
//! - `{ on_press = ..., on_release = ... }`: hold-style binds
//! - `{ tap = ... }`: modifier-only tap binds (`"Super" = { tap = ... }`)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::action::Action;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BindValue {
    pub action: Option<Action>,
    pub on_press: Option<Action>,
    pub on_release: Option<Action>,
    pub tap: Option<Action>,
    pub flags: BindFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BindFlags {
    /// Fire even while the session is locked.
    pub allow_when_locked: bool,
    /// Fire even while a client inhibits shortcuts (games, VMs).
    pub allow_when_inhibited: bool,
    /// Let the event continue to other binds and the focused client.
    pub transparent: bool,
    /// Match on the keysym of the current layout instead of the first layout.
    pub layout_dependent: bool,
    /// Key repeat. Defaults to true for non-spawn actions.
    pub repeat: Option<bool>,
}

const FLAG_KEYS: &[&str] = &[
    "allow_when_locked",
    "allow_when_inhibited",
    "transparent",
    "layout_dependent",
    "repeat",
];
const SLOT_KEYS: &[&str] = &["action", "on_press", "on_release", "tap"];

fn parse_action(v: Value) -> Result<Action, String> {
    serde_json::from_value(v.clone()).map_err(|e| format!("invalid action {v}: {e}"))
}

impl TryFrom<Value> for BindValue {
    type Error = String;

    fn try_from(v: Value) -> Result<Self, String> {
        match v {
            Value::String(_) => Ok(BindValue {
                action: Some(parse_action(v)?),
                ..Default::default()
            }),
            Value::Object(map) => {
                let mut flags = Map::new();
                let mut slots = Map::new();
                let mut rest = Map::new();
                for (k, v) in map {
                    if FLAG_KEYS.contains(&k.as_str()) {
                        flags.insert(k, v);
                    } else if SLOT_KEYS.contains(&k.as_str()) {
                        slots.insert(k, v);
                    } else {
                        rest.insert(k, v);
                    }
                }
                let flags: BindFlags = serde_json::from_value(Value::Object(flags))
                    .map_err(|e| format!("invalid bind flags: {e}"))?;

                let mut out = BindValue {
                    flags,
                    ..Default::default()
                };
                match rest.len() {
                    0 => {}
                    1 => {
                        if !slots.is_empty() {
                            return Err(format!(
                                "bind mixes an inline action ({}) with {}",
                                rest.keys().next().unwrap(),
                                slots.keys().cloned().collect::<Vec<_>>().join(", ")
                            ));
                        }
                        out.action = Some(parse_action(Value::Object(rest))?);
                    }
                    _ => {
                        return Err(format!(
                            "bind has several actions or unknown keys: {}",
                            rest.keys().cloned().collect::<Vec<_>>().join(", ")
                        ));
                    }
                }
                for (k, v) in slots {
                    let a = Some(parse_action(v)?);
                    match k.as_str() {
                        "action" => out.action = a,
                        "on_press" => out.on_press = a,
                        "on_release" => out.on_release = a,
                        "tap" => out.tap = a,
                        _ => unreachable!(),
                    }
                }
                if out.action.is_none()
                    && out.on_press.is_none()
                    && out.on_release.is_none()
                    && out.tap.is_none()
                {
                    return Err("bind has no action".into());
                }
                if out.action.is_some() && (out.on_press.is_some() || out.on_release.is_some()) {
                    return Err("use either `action` or `on_press`/`on_release`, not both".into());
                }
                Ok(out)
            }
            other => Err(format!(
                "invalid bind {other}: expected an action string or a table"
            )),
        }
    }
}

impl<'de> Deserialize<'de> for BindValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        BindValue::try_from(v).map_err(serde::de::Error::custom)
    }
}

impl Serialize for BindValue {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;
        let simple = self.flags == BindFlags::default()
            && self.on_press.is_none()
            && self.on_release.is_none()
            && self.tap.is_none();
        if simple && let Some(a) = &self.action {
            return a.serialize(s);
        }
        let mut map = Map::new();
        let mut put = |k: &str, a: &Option<Action>| -> Result<(), serde_json::Error> {
            if let Some(a) = a {
                map.insert(k.into(), serde_json::to_value(a)?);
            }
            Ok(())
        };
        put("action", &self.action).map_err(S::Error::custom)?;
        put("on_press", &self.on_press).map_err(S::Error::custom)?;
        put("on_release", &self.on_release).map_err(S::Error::custom)?;
        put("tap", &self.tap).map_err(S::Error::custom)?;
        if let Value::Object(flags) = serde_json::to_value(self.flags).map_err(S::Error::custom)? {
            let defaults = serde_json::to_value(BindFlags::default()).map_err(S::Error::custom)?;
            for (k, v) in flags {
                if defaults.get(&k) != Some(&v) {
                    map.insert(k, v);
                }
            }
        }
        Value::Object(map).serialize(s)
    }
}

impl JsonSchema for BindValue {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "BindValue".into()
    }
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let action = generator.subschema_for::<Action>();
        schemars::json_schema!({
            "description": "An action, or a table with an action plus flags, or on_press/on_release/tap actions.",
            "anyOf": [
                action,
                {
                    "type": "object",
                    "properties": {
                        "action": action,
                        "on_press": action,
                        "on_release": action,
                        "tap": action,
                        "allow_when_locked": { "type": "boolean" },
                        "allow_when_inhibited": { "type": "boolean" },
                        "transparent": { "type": "boolean" },
                        "layout_dependent": { "type": "boolean" },
                        "repeat": { "type": "boolean" }
                    }
                }
            ]
        })
    }
}

/// Where a mouse bind applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum MouseContext {
    /// Anywhere over a window (with modifiers, usually).
    #[default]
    Window,
    /// Over a window's border.
    Border,
    /// Over the desktop background.
    Desktop,
    /// Anywhere.
    Any,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MouseBind {
    pub action: Action,
    #[serde(default)]
    pub context: MouseContext,
}

/// A bind after parsing its key combo.
#[derive(Debug, Clone, PartialEq)]
pub struct Bind {
    pub combo: super::keys::KeyCombo,
    pub source: String,
    pub value: BindValue,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::action::WorkspaceRef;

    fn parse(s: &str) -> Result<BindValue, String> {
        #[derive(Deserialize)]
        struct W {
            b: BindValue,
        }
        toml::from_str::<W>(s).map(|w| w.b).map_err(|e| e.to_string())
    }

    #[test]
    fn forms() {
        assert_eq!(parse(r#"b = "close""#).unwrap().action, Some(Action::Close));
        let b = parse(r#"b = { workspace = 2 }"#).unwrap();
        assert_eq!(b.action, Some(Action::Workspace(WorkspaceRef::Index(2))));

        let b = parse(r#"b = { spawn = ["wpctl"], allow_when_locked = true }"#).unwrap();
        assert!(b.flags.allow_when_locked);
        assert_eq!(b.action, Some(Action::Spawn(vec!["wpctl".into()])));

        let b = parse(r#"b = { action = "toggle_keybinds", allow_when_inhibited = true }"#).unwrap();
        assert_eq!(b.action, Some(Action::ToggleKeybinds));

        let b = parse(r#"b = { on_press = "focus", on_release = "raise", transparent = true }"#).unwrap();
        assert_eq!(b.on_press, Some(Action::Focus));
        assert_eq!(b.on_release, Some(Action::Raise));
        assert!(b.flags.transparent);

        let b = parse(r#"b = { tap = { spawn_sh = "x" } }"#).unwrap();
        assert_eq!(b.tap, Some(Action::SpawnSh("x".into())));
    }

    #[test]
    fn errors() {
        assert!(parse(r#"b = "nope""#).is_err());
        assert!(parse(r#"b = { close = 1, focus = 2 }"#).is_err());
        assert!(parse(r#"b = { allow_when_locked = true }"#).is_err());
        assert!(parse(r#"b = { action = "close", on_press = "focus" }"#).is_err());
    }

    #[test]
    fn roundtrip() {
        let b = parse(r#"b = { spawn = ["wpctl"], allow_when_locked = true }"#).unwrap();
        let v = serde_json::to_value(&b).unwrap();
        let back = BindValue::try_from(v).unwrap();
        assert_eq!(back, b);
    }
}
