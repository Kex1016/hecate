//! Runtime config overrides (`hecate msg set/get/unset`), used by e.g. the shell's Game Mode.
//!
//! Overrides are dotted paths into the config (`animations.enable`) layered on top of the
//! config file. They survive reloads and are never written to disk.

use hecate_ipc as ipc;
use serde_json::Value;

use crate::state::State;

fn set_path(root: &mut Value, path: &str, value: Value) -> Result<(), String> {
    let mut cur = root;
    let parts: Vec<&str> = path.split('.').collect();
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            return Err(format!("invalid path {path:?}"));
        }
        let obj = cur
            .as_object_mut()
            .ok_or_else(|| format!("{} is not a table", parts[..i].join(".")))?;
        if i + 1 == parts.len() {
            obj.insert(part.to_string(), value);
            return Ok(());
        }
        cur = obj
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Default::default()));
    }
    Ok(())
}

pub fn get_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(root, |cur, part| cur.get(part))
}

impl State {
    /// The config with overrides applied, or an error if it doesn't validate.
    fn config_with_overrides(&self, overrides: &[(String, Value)]) -> Result<crate::config::Config, String> {
        let mut value = serde_json::to_value(&self.hecate.file_config).map_err(|e| e.to_string())?;
        for (path, v) in overrides {
            set_path(&mut value, path, v.clone())?;
        }
        let config: crate::config::Config = serde_json::from_value(value).map_err(|e| e.to_string())?;
        crate::config::load::validate(&config).map_err(|e| e.to_string())?;
        Ok(config)
    }

    pub fn set_override(&mut self, path: &str, value: Value) -> Result<(), String> {
        let mut overrides = self.hecate.overrides.clone();
        overrides.retain(|(p, _)| p != path);
        overrides.push((path.to_string(), value));
        let config = self.config_with_overrides(&overrides)?;
        self.hecate.overrides = overrides;
        self.apply_config(config);
        Ok(())
    }

    pub fn unset_override(&mut self, which: ipc::Unset) -> Result<(), String> {
        let mut overrides = self.hecate.overrides.clone();
        match which {
            ipc::Unset::All => overrides.clear(),
            ipc::Unset::Path(p) => {
                let before = overrides.len();
                overrides.retain(|(x, _)| *x != p);
                if overrides.len() == before {
                    return Err(format!("no override at {p:?}"));
                }
            }
        }
        let config = self.config_with_overrides(&overrides)?;
        self.hecate.overrides = overrides;
        self.apply_config(config);
        Ok(())
    }

    pub fn get_config_value(&self, path: &str) -> Option<Value> {
        let value = serde_json::to_value(&self.hecate.config).ok()?;
        get_path(&value, path).cloned()
    }

    /// Re-apply overrides on top of a freshly loaded file config.
    pub fn effective_config(&self, file: &crate::config::Config) -> crate::config::Config {
        if self.hecate.overrides.is_empty() {
            return file.clone();
        }
        let mut value = match serde_json::to_value(file) {
            Ok(v) => v,
            Err(_) => return file.clone(),
        };
        for (path, v) in &self.hecate.overrides {
            if set_path(&mut value, path, v.clone()).is_err() {
                tracing::warn!("override {path} no longer applies");
            }
        }
        serde_json::from_value(value)
            .ok()
            .filter(|c| crate::config::load::validate(c).is_ok())
            .unwrap_or_else(|| file.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        let mut v = serde_json::json!({"animations": {"enable": true}});
        set_path(&mut v, "animations.enable", Value::Bool(false)).unwrap();
        set_path(&mut v, "border.width", serde_json::json!(1)).unwrap();
        assert_eq!(get_path(&v, "animations.enable"), Some(&Value::Bool(false)));
        assert_eq!(get_path(&v, "border.width"), Some(&serde_json::json!(1)));
        assert!(set_path(&mut v, "animations.enable.x", Value::Null).is_err());
        assert!(set_path(&mut v, "a..b", Value::Null).is_err());
    }
}
