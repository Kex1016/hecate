//! Loading, include merging and validation.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::binds::{Bind, BindValue, MouseBind};
use super::defaults::{base_binds, base_mouse_binds};
use super::keys::KeyCombo;
use super::{Config, MAX_WORKSPACES, WorkspaceKind};

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("reading {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("parsing {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("include cycle through {0}")]
    IncludeCycle(PathBuf),
    #[error("{0}")]
    Invalid(String),
}

/// Where the config lives when no path is given.
pub fn default_path() -> Option<PathBuf> {
    let dirs = directories::BaseDirs::new()?;
    let dir = dirs.config_dir().join("hecate");
    let toml = dir.join("config.toml");
    let json = dir.join("config.json");
    if toml.exists() {
        Some(toml)
    } else if json.exists() {
        Some(json)
    } else {
        Some(toml)
    }
}

/// Load the config from `path`, or from the default location, or built-in defaults if
/// nothing exists.
pub fn load(path: Option<&Path>) -> Result<(Config, Option<PathBuf>), LoadError> {
    let path = match path {
        Some(p) => Some(p.to_path_buf()),
        None => std::env::var_os("HECATE_CONFIG")
            .map(PathBuf::from)
            .or_else(default_path),
    };
    match path {
        Some(p) if p.exists() => Ok((load_file(&p)?, Some(p))),
        Some(p) => {
            tracing::info!("no config at {}, using defaults", p.display());
            Ok((Config::default(), Some(p)))
        }
        None => Ok((Config::default(), None)),
    }
}

pub fn load_file(path: &Path) -> Result<Config, LoadError> {
    let mut stack = Vec::new();
    let value = read_merged(path, &mut stack)?;
    let config: Config = serde_json::from_value(value).map_err(|e| LoadError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    validate(&config)?;
    Ok(config)
}

/// Parse a config from a string (TOML), without includes.
pub fn from_toml_str(s: &str) -> Result<Config, LoadError> {
    let config: Config = toml::from_str(s).map_err(|e| LoadError::Parse {
        path: PathBuf::from("<string>"),
        message: e.to_string(),
    })?;
    validate(&config)?;
    Ok(config)
}

fn parse_file(path: &Path) -> Result<serde_json::Value, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let parse_err = |message: String| LoadError::Parse {
        path: path.to_path_buf(),
        message,
    };
    if path.extension().is_some_and(|e| e == "json") {
        serde_json::from_str(&text).map_err(|e| parse_err(e.to_string()))
    } else {
        let v: toml::Value = toml::from_str(&text).map_err(|e| parse_err(e.to_string()))?;
        serde_json::to_value(v).map_err(|e| parse_err(e.to_string()))
    }
}

fn read_merged(path: &Path, stack: &mut Vec<PathBuf>) -> Result<serde_json::Value, LoadError> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if stack.contains(&canonical) {
        return Err(LoadError::IncludeCycle(canonical));
    }
    stack.push(canonical);

    let mut value = parse_file(path)?;
    let includes: Vec<String> = value
        .get("include")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let base_dir = path.parent().unwrap_or(Path::new("."));
    for inc in includes {
        let inc_path = expand_tilde(&inc);
        let inc_path = if inc_path.is_absolute() {
            inc_path
        } else {
            base_dir.join(inc_path)
        };
        if !inc_path.exists() {
            tracing::warn!("included config {} does not exist, skipping", inc_path.display());
            continue;
        }
        let included = read_merged(&inc_path, stack)?;
        merge(&mut value, included);
    }

    stack.pop();
    Ok(value)
}

fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(p)
}

/// Tables merge recursively, everything else (arrays included) is replaced by `other`.
pub fn merge(base: &mut serde_json::Value, other: serde_json::Value) {
    match (base, other) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for (k, v) in b {
                if k == "include" {
                    continue;
                }
                match a.get_mut(&k) {
                    Some(existing) => merge(existing, v),
                    None => {
                        a.insert(k, v);
                    }
                }
            }
        }
        (a, b) => *a = b,
    }
}

pub fn validate(config: &Config) -> Result<(), LoadError> {
    let mut errors = Vec::new();

    let ws = &config.workspaces;
    if ws.count == 0 || ws.count > MAX_WORKSPACES {
        errors.push(format!(
            "workspaces.count must be 1..={MAX_WORKSPACES}, got {}",
            ws.count
        ));
    }
    if ws.min > MAX_WORKSPACES {
        errors.push(format!(
            "workspaces.min must be at most {MAX_WORKSPACES}, got {}",
            ws.min
        ));
    }
    if ws.kind == WorkspaceKind::Fixed && ws.names.len() > ws.count as usize {
        errors.push(format!(
            "workspaces.names has {} entries but count is {}",
            ws.names.len(),
            ws.count
        ));
    }

    for (section, binds) in [("binds", &config.binds), ("shell_binds", &config.shell_binds)] {
        for k in binds.keys() {
            if let Err(e) = k.parse::<KeyCombo>() {
                errors.push(format!("{section}.{k:?}: {e}"));
            }
        }
    }
    for k in config.mouse_binds.keys() {
        if let Err(e) = k.parse::<KeyCombo>() {
            errors.push(format!("mouse_binds.{k:?}: {e}"));
        }
    }

    let curves = &config.animations.curves;
    for (name, anim) in [
        ("window_open", &config.animations.window_open),
        ("window_close", &config.animations.window_close),
        ("window_move", &config.animations.window_move),
        ("border", &config.animations.border),
        ("layer_open", &config.animations.layer_open),
        ("layer_close", &config.animations.layer_close),
        ("workspace_switch", &config.animations.workspace_switch),
    ] {
        if !curves.contains_key(&anim.curve) {
            errors.push(format!("animations.{name}.curve: unknown curve {:?}", anim.curve));
        }
    }
    if config.animations.slowdown <= 0. {
        errors.push("animations.slowdown must be positive".into());
    }

    const LAYER_ANIMATIONS: &[&str] = &[
        "none",
        "fade",
        "popin",
        "slide",
        "slide_top",
        "slide_bottom",
        "slide_left",
        "slide_right",
    ];
    for (i, rule) in config.layer_rules.iter().enumerate() {
        if let Some(ns) = &rule.matches.namespace
            && let Err(e) = regex::Regex::new(ns)
        {
            errors.push(format!("layer_rules[{i}].match.namespace: {e}"));
        }
        if let Some(super::LayerAnimation::Named(name)) = &rule.animation
            && !LAYER_ANIMATIONS.contains(&name.as_str())
        {
            errors.push(format!(
                "layer_rules[{i}].animation: unknown {name:?} (one of {})",
                LAYER_ANIMATIONS.join(", ")
            ));
        }
        if let Some(a) = rule.ignore_alpha
            && !(0.0..=1.0).contains(&a)
        {
            errors.push(format!("layer_rules[{i}].ignore_alpha must be 0..=1, got {a}"));
        }
    }
    for (i, rule) in config.window_rules.iter().enumerate() {
        for (field, pat) in [("app_id", &rule.matches.app_id), ("title", &rule.matches.title)] {
            if let Some(p) = pat
                && let Err(e) = glob::Pattern::new(p)
            {
                errors.push(format!("window_rules[{i}].match.{field}: {e}"));
            }
        }
    }

    for (name, out) in &config.outputs {
        if let Some(mode) = &out.mode
            && parse_mode(mode).is_none()
        {
            errors.push(format!(
                "outputs.{name:?}.mode: invalid mode {mode:?}, expected WxH or WxH@Hz"
            ));
        }
        if let Some(scale) = out.scale
            && !(0.25..=10.0).contains(&scale)
        {
            errors.push(format!("outputs.{name:?}.scale: {scale} out of range"));
        }
    }

    if config.border.width < 0. {
        errors.push("border.width must not be negative".into());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(LoadError::Invalid(errors.join("\n")))
    }
}

/// `WxH` or `WxH@refresh` (refresh in Hz, decimals allowed).
pub fn parse_mode(s: &str) -> Option<(i32, i32, Option<f64>)> {
    let (size, refresh) = match s.split_once('@') {
        Some((size, r)) => (size, Some(r.trim().trim_end_matches("Hz").parse::<f64>().ok()?)),
        None => (s, None),
    };
    let (w, h) = size.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?, refresh))
}

/// Base binds (unless disabled) overlaid with the user's, parsed.
pub fn resolved_binds(config: &Config) -> Vec<Bind> {
    let mut merged: BTreeMap<String, (String, BindValue)> = BTreeMap::new();
    let mut insert = |k: &String, v: &BindValue, source: &str| {
        // Normalise the key so "Mod+q" and "Mod+Q" collide.
        let norm = k
            .parse::<KeyCombo>()
            .map(|c| c.to_string())
            .unwrap_or_else(|_| k.clone());
        merged.insert(norm, (source.to_string(), v.clone()));
    };
    if config.default_binds {
        for (k, v) in base_binds() {
            insert(k, v, "base");
        }
    }
    for (k, v) in &config.shell_binds {
        insert(k, v, "shell");
    }
    for (k, v) in &config.binds {
        insert(k, v, "config");
    }
    merged
        .into_iter()
        .filter_map(|(k, (source, value))| {
            Some(Bind {
                combo: k.parse().ok()?,
                source,
                value,
            })
        })
        .collect()
}

pub fn resolved_mouse_binds(config: &Config) -> Vec<(KeyCombo, MouseBind)> {
    let mut merged: BTreeMap<String, MouseBind> = BTreeMap::new();
    if config.default_binds {
        for (k, v) in base_mouse_binds() {
            merged.insert(k.clone(), v.clone());
        }
    }
    for (k, v) in &config.mouse_binds {
        merged.insert(k.clone(), v.clone());
    }
    merged
        .into_iter()
        .filter_map(|(k, v)| Some((k.parse().ok()?, v)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../../tests/example-config.toml");

    #[test]
    fn default_config_is_valid() {
        validate(&Config::default()).unwrap();
    }

    #[test]
    fn example_parses() {
        let c = from_toml_str(EXAMPLE).unwrap();
        assert_eq!(c.workspaces.count, 9);
        assert_eq!(c.border.radius.corners().top_left, 15.);
        assert!(!c.layer_rules.is_empty());
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(from_toml_str("[border]\nwidht = 2").is_err());
        assert!(from_toml_str("bogus = 1").is_err());
    }

    #[test]
    fn workspace_limits() {
        assert!(from_toml_str("[workspaces]\ncount = 33").is_err());
        assert!(from_toml_str("[workspaces]\ncount = 0").is_err());
        assert!(from_toml_str("[workspaces]\ncount = 32").is_ok());
    }

    #[test]
    fn modes() {
        assert_eq!(parse_mode("2560x1080@200.007"), Some((2560, 1080, Some(200.007))));
        assert_eq!(parse_mode("1920x1080"), Some((1920, 1080, None)));
        assert_eq!(parse_mode("garbage"), None);
    }

    #[test]
    fn includes_merge() {
        let dir = std::env::temp_dir().join(format!("hecate-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            "include = [\"extra.toml\", \"missing.toml\"]\n[border]\nwidth = 3\n[binds]\n\"Mod+A\" = \"close\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("extra.toml"),
            "[border]\nclip_content = false\n[binds]\n\"Mod+B\" = \"close\"\n",
        )
        .unwrap();
        let c = load_file(&dir.join("config.toml")).unwrap();
        assert_eq!(c.border.width, 3.);
        assert!(!c.border.clip_content);
        assert_eq!(c.binds.len(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn user_binds_override_base() {
        let c = from_toml_str("[binds]\n\"Mod+q\" = \"kill\"").unwrap();
        let binds = resolved_binds(&c);
        let q: Vec<_> = binds
            .iter()
            .filter(|b| b.combo.to_string() == "Mod+q" || b.combo.to_string() == "Mod+Q")
            .collect();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].source, "config");
    }

    #[test]
    fn shell_binds_sit_between_base_and_config() {
        let c = from_toml_str(
            "[shell_binds]\n\"Mod+J\" = \"close\"\n\"Mod+K\" = \"close\"\n[binds]\n\"Mod+K\" = \"kill\"",
        )
        .unwrap();
        let binds = resolved_binds(&c);
        let source = |k: &str| {
            let combo: KeyCombo = k.parse().unwrap();
            binds.iter().find(|b| b.combo == combo).map(|b| b.source.clone())
        };
        assert_eq!(source("Mod+J").as_deref(), Some("shell"), "shell overrides base");
        assert_eq!(source("Mod+K").as_deref(), Some("config"), "yours override shell");
        assert_eq!(source("Mod+Q").as_deref(), Some("base"));
        assert!(from_toml_str("[shell_binds]\n\"Mod+Nope+X\" = \"close\"").is_err());
    }

    #[test]
    fn layer_rule_animation_names_are_checked() {
        let ok = "[[layer_rules]]\nmatch = { namespace = \"bar\" }\nanimation = \"slide_left\"";
        assert!(from_toml_str(ok).is_ok());
        let popin = "[[layer_rules]]\nanimation = { popin = 0.9 }";
        assert!(from_toml_str(popin).is_ok());
        let typo = "[[layer_rules]]\nanimation = \"slide-left\"";
        let err = from_toml_str(typo).unwrap_err().to_string();
        assert!(err.contains("slide-left"), "{err}");
        assert!(from_toml_str("[[layer_rules]]\nignore_alpha = 1.5").is_err());
    }
}
