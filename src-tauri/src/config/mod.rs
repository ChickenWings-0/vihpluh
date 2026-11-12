//! Configuration: shipped defaults ⊕ the user's `config.toml` (KEYBINDS_AND_CONFIG §1, §5).
//!
//! The user file holds only overrides. Unknown keys are left alone and reported once; invalid
//! values fall back to the default and are reported by key.

pub mod schema;

use std::path::Path;

use toml::{Table, Value};

pub use schema::Config;
use schema::{COLOR_KEYS, FREE_FORM_TABLES, RANGES};

const DEFAULT_TOML: &str = include_str!("../../resources/default.toml");

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigWarning {
    /// The file could not be read or is not valid TOML; defaults are in effect.
    Unreadable {
        message: String,
    },
    UnknownKey {
        path: String,
    },
    InvalidValue {
        path: String,
        reason: String,
    },
}

impl std::fmt::Display for ConfigWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable { message } => write!(f, "config.toml could not be read: {message}"),
            Self::UnknownKey { path } => write!(f, "unknown setting `{path}` (left untouched)"),
            Self::InvalidValue { path, reason } => {
                write!(f, "invalid value for `{path}` ({reason}); using the default")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Loaded {
    pub config: Config,
    pub warnings: Vec<ConfigWarning>,
}

/// The shipped defaults as a TOML table.
pub fn defaults_table() -> Table {
    // Startup invariant: default.toml is compiled in and covered by the `defaults_parse` test.
    #[allow(clippy::expect_used)]
    DEFAULT_TOML.parse::<Table>().expect("resources/default.toml is valid TOML")
}

pub fn defaults() -> Config {
    #[allow(clippy::expect_used)]
    to_config(&defaults_table()).expect("resources/default.toml matches the Config schema")
}

/// Loads the effective configuration. A missing file is not an error.
pub fn load(path: &Path) -> Loaded {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Loaded { config: defaults(), warnings: Vec::new() };
        }
        Err(err) => {
            return Loaded {
                config: defaults(),
                warnings: vec![ConfigWarning::Unreadable { message: err.to_string() }],
            };
        }
    };
    from_user_text(&text)
}

pub fn from_user_text(text: &str) -> Loaded {
    match text.parse::<Table>() {
        Ok(user) => {
            let (merged, warnings) = overlay(&defaults_table(), &user);
            let config = to_config(&merged).unwrap_or_else(|_| defaults());
            Loaded { config, warnings }
        }
        Err(err) => Loaded {
            config: defaults(),
            warnings: vec![ConfigWarning::Unreadable { message: err.message().to_string() }],
        },
    }
}

fn to_config(table: &Table) -> Result<Config, toml::de::Error> {
    Value::Table(table.clone()).try_into()
}

/// Layers `user` over `defaults`, leaf by leaf, dropping anything unknown or invalid.
pub fn overlay(defaults: &Table, user: &Table) -> (Table, Vec<ConfigWarning>) {
    let mut merged = defaults.clone();
    let mut warnings = Vec::new();
    let mut leaves = Vec::new();
    collect_leaves(defaults, user, "", &mut leaves, &mut warnings);

    for (path, value) in leaves {
        if let Err(reason) = check_value(&path, &value) {
            warnings.push(ConfigWarning::InvalidValue { path, reason });
            continue;
        }
        let mut candidate = merged.clone();
        set_path(&mut candidate, &path, value);
        match to_config(&candidate) {
            Ok(_) => merged = candidate,
            Err(err) => warnings
                .push(ConfigWarning::InvalidValue { path, reason: err.message().to_string() }),
        }
    }
    (merged, warnings)
}

fn collect_leaves(
    defaults: &Table,
    user: &Table,
    prefix: &str,
    leaves: &mut Vec<(String, Value)>,
    warnings: &mut Vec<ConfigWarning>,
) {
    for (key, value) in user {
        let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
        if FREE_FORM_TABLES.contains(&path.as_str()) {
            if let Value::Table(table) = value {
                for (k, v) in table {
                    leaves.push((format!("{path}.{k}"), v.clone()));
                }
            } else {
                warnings.push(ConfigWarning::InvalidValue {
                    path,
                    reason: "expected a table".to_string(),
                });
            }
            continue;
        }
        match (defaults.get(key), value) {
            (None, _) => warnings.push(ConfigWarning::UnknownKey { path }),
            (Some(Value::Table(d)), Value::Table(u)) => {
                collect_leaves(d, u, &path, leaves, warnings);
            }
            (Some(Value::Table(_)), _) => warnings
                .push(ConfigWarning::InvalidValue { path, reason: "expected a table".to_string() }),
            (Some(_), _) => leaves.push((path, value.clone())),
        }
    }
}

/// Range and format checks that the type system does not express.
pub fn check_value(path: &str, value: &Value) -> Result<(), String> {
    if let Some((_, min, max)) = RANGES.iter().find(|(p, _, _)| *p == path) {
        let number = match value {
            Value::Integer(i) => *i as f64,
            Value::Float(f) => *f,
            _ => return Err("expected a number".to_string()),
        };
        if !number.is_finite() || number < *min || number > *max {
            return Err(format!("must be between {min} and {max}"));
        }
    }
    if COLOR_KEYS.contains(&path) {
        let ok = value.as_str().is_some_and(is_hex_color);
        if !ok {
            return Err("expected a colour like #RRGGBB".to_string());
        }
    }
    Ok(())
}

pub fn is_hex_color(text: &str) -> bool {
    text.len() == 7 && text.starts_with('#') && text[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Sets a dotted path, creating intermediate tables. Paths under a free-form table keep any
/// dots in the final key (mpv option names never contain dots, env names do not either).
pub fn set_path(table: &mut Table, path: &str, value: Value) {
    let free_form = FREE_FORM_TABLES.iter().find(|t| path.starts_with(&format!("{t}.")));
    let (parents, leaf): (Vec<&str>, &str) = match free_form {
        Some(prefix) => (prefix.split('.').collect(), &path[prefix.len() + 1..]),
        None => {
            let mut parts: Vec<&str> = path.split('.').collect();
            let leaf = parts.pop().unwrap_or(path);
            (parts, leaf)
        }
    };
    let mut current = table;
    for part in parents {
        let entry = current.entry(part.to_string()).or_insert_with(|| Value::Table(Table::new()));
        if !entry.is_table() {
            *entry = Value::Table(Table::new());
        }
        current = match entry {
            Value::Table(t) => t,
            _ => return,
        };
    }
    current.insert(leaf.to_string(), value);
}

/// Reads a dotted path out of a table.
pub fn get_path<'a>(table: &'a Table, path: &str) -> Option<&'a Value> {
    let free_form = FREE_FORM_TABLES.iter().find(|t| path.starts_with(&format!("{t}.")));
    let parts: Vec<&str> = match free_form {
        Some(prefix) => {
            let mut parts: Vec<&str> = prefix.split('.').collect();
            parts.push(&path[prefix.len() + 1..]);
            parts
        }
        None => path.split('.').collect(),
    };
    let (leaf, parents) = parts.split_last()?;
    let mut current = table;
    for part in parents {
        current = current.get(*part)?.as_table()?;
    }
    current.get(*leaf)
}

#[cfg(test)]
mod tests {
    use super::schema::*;
    use super::*;

    #[test]
    fn defaults_parse() {
        let config = defaults();
        assert_eq!(config.schema, SCHEMA_VERSION);
        assert_eq!(config.interface.accent, "#FF6B00");
        assert_eq!(config.interface.osd_position, OsdPosition::TopCenter);
        assert_eq!(config.video.hwdec, "auto");
        assert_eq!(config.video.target_peak, IntOrKeyword::Keyword("auto".into()));
        assert_eq!(config.audio.channels, AudioChannels::Auto);
        assert_eq!(config.audio.max_volume, 130);
        assert_eq!(config.performance.demuxer_cache_mb, 512);
        assert!(!config.mpv.load_user_config);
        assert!(config.mpv.options.is_empty());
    }

    #[test]
    fn every_ranged_and_colour_key_exists_with_a_valid_default() {
        let table = defaults_table();
        for (path, _, _) in RANGES {
            let value = get_path(&table, path).unwrap_or_else(|| panic!("{path} missing"));
            assert_eq!(check_value(path, value), Ok(()), "{path}");
        }
        for path in COLOR_KEYS {
            let value = get_path(&table, path).unwrap_or_else(|| panic!("{path} missing"));
            assert_eq!(check_value(path, value), Ok(()), "{path}");
        }
    }

    #[test]
    fn user_values_override_defaults_key_by_key() {
        let loaded = from_user_text(
            r##"
            schema = 1
            [interface]
            accent = "#3E8BFF"
            seek_waveform = true
            [audio]
            preferred_languages = ["jpn", "ja", "eng"]
            channels = "5.1"
            [video]
            hwdec = "off"
            target_peak = 203
            [mpv.options]
            dither-depth = "auto"
            deband-iterations = 2
            "##,
        );
        assert_eq!(loaded.warnings, vec![]);
        let c = loaded.config;
        assert_eq!(c.interface.accent, "#3E8BFF");
        assert!(c.interface.seek_waveform);
        assert!(c.interface.seek_preview, "untouched keys keep their defaults");
        assert_eq!(c.audio.preferred_languages, vec!["jpn", "ja", "eng"]);
        assert_eq!(c.audio.channels, AudioChannels::Surround51);
        assert_eq!(c.video.hwdec, "off");
        assert_eq!(c.video.target_peak, IntOrKeyword::Int(203));
        assert_eq!(c.mpv.options.get("dither-depth"), Some(&Value::String("auto".into())));
        assert_eq!(c.mpv.options.get("deband-iterations"), Some(&Value::Integer(2)));
    }

    #[test]
    fn integers_are_accepted_for_float_settings() {
        let loaded = from_user_text("[interface]\nui_scale = 1\n[controls]\nseek_small_s = 3\n");
        assert_eq!(loaded.warnings, vec![]);
        assert_eq!(loaded.config.controls.seek_small_s, 3.0);
    }

    #[test]
    fn unknown_keys_are_reported_and_ignored() {
        let loaded = from_user_text("[interface]\nsparkles = true\n[nonsense]\nx = 1\n");
        assert_eq!(
            loaded.warnings,
            vec![
                ConfigWarning::UnknownKey { path: "interface.sparkles".into() },
                ConfigWarning::UnknownKey { path: "nonsense".into() },
            ]
        );
        assert_eq!(loaded.config, defaults());
    }

    #[test]
    fn invalid_values_fall_back_to_the_default_and_name_the_key() {
        let loaded = from_user_text(
            r##"
            [interface]
            theme = "neon"
            ui_scale = 9.0
            accent = "orange"
            sidebar_width = 400
            [audio]
            max_volume = "loud"
            "##,
        );
        let invalid: Vec<&str> = loaded
            .warnings
            .iter()
            .filter_map(|w| match w {
                ConfigWarning::InvalidValue { path, .. } => Some(path.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            invalid,
            vec!["audio.max_volume", "interface.accent", "interface.theme", "interface.ui_scale"]
        );
        let c = loaded.config;
        assert_eq!(c.interface.theme, Theme::Dark);
        assert_eq!(c.interface.ui_scale, 1.0);
        assert_eq!(c.interface.accent, "#FF6B00");
        assert_eq!(c.interface.sidebar_width, 400, "valid neighbours still apply");
        assert_eq!(c.audio.max_volume, 130);
    }

    #[test]
    fn malformed_toml_yields_defaults_and_one_warning() {
        let loaded = from_user_text("[interface\ntheme = ");
        assert_eq!(loaded.config, defaults());
        assert!(matches!(loaded.warnings.as_slice(), [ConfigWarning::Unreadable { .. }]));
    }

    #[test]
    fn a_missing_file_is_just_defaults() {
        let loaded = load(Path::new("/definitely/not/here/config.toml"));
        assert_eq!(loaded.config, defaults());
        assert!(loaded.warnings.is_empty());
    }

    #[test]
    fn set_and_get_path_round_trip() {
        let mut table = Table::new();
        set_path(&mut table, "subtitles.font_size", Value::Float(46.0));
        set_path(&mut table, "mpv.options.dither-depth", Value::String("auto".into()));
        assert_eq!(get_path(&table, "subtitles.font_size"), Some(&Value::Float(46.0)));
        assert_eq!(
            get_path(&table, "mpv.options.dither-depth"),
            Some(&Value::String("auto".into()))
        );
        assert_eq!(get_path(&table, "subtitles.missing"), None);
    }

    #[test]
    fn hex_colours() {
        assert!(is_hex_color("#FF6B00"));
        assert!(is_hex_color("#0d0d0f"));
        assert!(!is_hex_color("FF6B00"));
        assert!(!is_hex_color("#FFF"));
        assert!(!is_hex_color("#GG0000"));
    }
}
