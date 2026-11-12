//! The live configuration (KEYBINDS_AND_CONFIG §1, §6 rule 8).
//!
//! - `config.toml` holds only what differs from the shipped defaults. Writes go through
//!   `toml_edit`, so the user's comments and ordering survive.
//! - The file is watched: saving it in a text editor applies the change live.
//! - Every change, whatever its source, is diffed against the previous effective config and
//!   reported as one `(dotted path, new value)` pair per changed key.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use toml_edit::DocumentMut;

use super::schema::{FREE_FORM_TABLES, SCHEMA_VERSION};
use super::{Config, ConfigWarning, defaults_table, from_user_text, get_path};
use crate::error::{AppError, AppResult};

/// Editors often save in several steps (truncate, write, rename); wait for quiet.
const WATCH_DEBOUNCE: Duration = Duration::from_millis(150);

/// What changed, handed to the change callback.
#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub config: Arc<Config>,
    /// Dotted path → new value, for every leaf that changed.
    pub changed: Vec<(String, serde_json::Value)>,
    /// Problems in the file that were not reported before.
    pub new_warnings: Vec<ConfigWarning>,
}

type ChangeCallback = Box<dyn Fn(&ConfigChange) + Send + Sync>;

struct Inner {
    current: Arc<Config>,
    warnings: Vec<ConfigWarning>,
}

pub struct ConfigStore {
    path: PathBuf,
    inner: Mutex<Inner>,
    on_change: OnceLock<ChangeCallback>,
    watcher: Mutex<Option<RecommendedWatcher>>,
}

impl ConfigStore {
    /// Loads `path` (a missing file means "all defaults").
    pub fn open(path: PathBuf) -> (Arc<Self>, Vec<ConfigWarning>) {
        let loaded = super::load(&path);
        let store = Arc::new(Self {
            path,
            inner: Mutex::new(Inner {
                current: Arc::new(loaded.config),
                warnings: loaded.warnings.clone(),
            }),
            on_change: OnceLock::new(),
            watcher: Mutex::new(None),
        });
        (store, loaded.warnings)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The effective configuration.
    pub fn get(&self) -> Arc<Config> {
        match self.inner.lock() {
            Ok(inner) => inner.current.clone(),
            Err(poisoned) => poisoned.into_inner().current.clone(),
        }
    }

    /// Registers the single change callback. It runs on whichever thread caused the change and
    /// must not block.
    pub fn on_change(&self, callback: impl Fn(&ConfigChange) + Send + Sync + 'static) {
        let _ = self.on_change.set(Box::new(callback));
    }

    /// Sets one key. A value equal to the default removes the key from the file instead.
    pub fn set(&self, path: &str, value: &serde_json::Value) -> AppResult<()> {
        let new_value = json_to_edit(value)
            .ok_or_else(|| AppError::invalid(format!("unsupported value for `{path}`")))?;
        let segments = segments(path)?;
        let is_default = is_default_value(path, value);

        let mut doc = self.read_document()?;
        if is_default {
            remove_key(&mut doc, &segments);
        } else {
            insert_key(&mut doc, &segments, new_value);
        }
        let text = doc.to_string();

        // Validate the result exactly as a reload would see it.
        let loaded = from_user_text(&text);
        if let Some(problem) = loaded.warnings.iter().find_map(|w| match w {
            ConfigWarning::InvalidValue { path: p, reason } if p == path => Some(reason.clone()),
            ConfigWarning::UnknownKey { path: p } if p == path => {
                Some("not a known setting".to_string())
            }
            ConfigWarning::Unreadable { message } => Some(message.clone()),
            _ => None,
        }) {
            return Err(AppError::invalid(format!("`{path}`: {problem}")));
        }

        self.write_text(&text)?;
        self.reload();
        Ok(())
    }

    /// Resets one key, or every setting when `path` is `None`. A full reset keeps the
    /// file-only advanced tables (`[mpv]`, `[env]`) and the user's comments at the top.
    pub fn reset(&self, path: Option<&str>) -> AppResult<()> {
        let mut doc = self.read_document()?;
        match path {
            Some(path) => remove_key(&mut doc, &segments(path)?),
            None => {
                let sections: Vec<String> = doc
                    .iter()
                    .map(|(key, _)| key.to_string())
                    .filter(|key| !matches!(key.as_str(), "schema" | "mpv" | "env"))
                    .collect();
                for section in sections {
                    doc.remove(&section);
                }
            }
        }
        self.write_text(&doc.to_string())?;
        self.reload();
        Ok(())
    }

    /// Re-reads the file and reports what changed. Safe to call at any time.
    pub fn reload(&self) {
        let loaded = super::load(&self.path);
        let change = {
            let mut inner = match self.inner.lock() {
                Ok(inner) => inner,
                Err(poisoned) => poisoned.into_inner(),
            };
            let changed = diff(&inner.current, &loaded.config);
            let new_warnings: Vec<ConfigWarning> =
                loaded.warnings.iter().filter(|w| !inner.warnings.contains(w)).cloned().collect();
            inner.warnings = loaded.warnings;
            if changed.is_empty() && new_warnings.is_empty() {
                return;
            }
            inner.current = Arc::new(loaded.config);
            ConfigChange { config: inner.current.clone(), changed, new_warnings }
        };
        for (path, value) in &change.changed {
            tracing::info!("setting changed: {path} = {value}");
        }
        for warning in &change.new_warnings {
            tracing::warn!("{warning}");
        }
        if let Some(callback) = self.on_change.get() {
            callback(&change);
        }
    }

    /// Starts watching the config file for external edits. Creates the (empty) config
    /// directory if needed so there is something to watch.
    pub fn watch(self: &Arc<Self>) -> AppResult<()> {
        let dir = self.path.parent().map(Path::to_path_buf).unwrap_or_default();
        std::fs::create_dir_all(&dir)?;
        let file_name = self.path.file_name().map(ToOwned::to_owned);

        let (tx, rx) = mpsc::channel::<()>();
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                let Ok(event) = result else { return };
                let touches_file =
                    event.paths.iter().any(|p| p.file_name() == file_name.as_deref());
                if touches_file && !event.kind.is_access() {
                    let _ = tx.send(());
                }
            })
            .map_err(|e| AppError::internal(format!("cannot watch the config file: {e}")))?;
        watcher
            .watch(&dir, RecursiveMode::NonRecursive)
            .map_err(|e| AppError::internal(format!("cannot watch {}: {e}", dir.display())))?;

        let store = Arc::downgrade(self);
        std::thread::Builder::new()
            .name("config-watch".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    // Swallow the burst, then reload once.
                    loop {
                        match rx.recv_timeout(WATCH_DEBOUNCE) {
                            Ok(()) => {}
                            Err(RecvTimeoutError::Timeout) => break,
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
                    }
                    match store.upgrade() {
                        Some(store) => store.reload(),
                        None => return,
                    }
                }
            })
            .map_err(|e| AppError::internal(format!("cannot start the config watcher: {e}")))?;

        if let Ok(mut slot) = self.watcher.lock() {
            *slot = Some(watcher);
        }
        Ok(())
    }

    fn read_document(&self) -> AppResult<DocumentMut> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                format!("schema = {SCHEMA_VERSION}\n")
            }
            Err(err) => return Err(err.into()),
        };
        text.parse::<DocumentMut>().map_err(|e| {
            AppError::invalid(format!(
                "config.toml has a syntax error; fix it before changing settings here ({})",
                e.message()
            ))
        })
    }

    /// Writes via a temporary file and a rename, so a crash never leaves half a file.
    fn write_text(&self, text: &str) -> AppResult<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temp = self.path.with_extension("toml.tmp");
        std::fs::write(&temp, text)?;
        std::fs::rename(&temp, &self.path)?;
        Ok(())
    }
}

/// Splits a dotted path into table segments; keys under a free-form table keep any dots.
fn segments(path: &str) -> AppResult<Vec<String>> {
    if let Some(prefix) = FREE_FORM_TABLES.iter().find(|t| path.starts_with(&format!("{t}."))) {
        let key = &path[prefix.len() + 1..];
        if key.is_empty() {
            return Err(AppError::invalid("missing key name"));
        }
        let mut parts: Vec<String> = prefix.split('.').map(str::to_string).collect();
        parts.push(key.to_string());
        return Ok(parts);
    }
    match get_path(&defaults_table(), path) {
        Some(toml::Value::Table(_)) => {
            Err(AppError::invalid(format!("`{path}` is a section, not a setting")))
        }
        Some(_) if path != "schema" => Ok(path.split('.').map(str::to_string).collect()),
        _ => Err(AppError::not_found(format!("there is no setting named `{path}`"))),
    }
}

fn is_default_value(path: &str, value: &serde_json::Value) -> bool {
    let defaults = defaults_table();
    let Some(default) = get_path(&defaults, path) else { return false };
    let Ok(default) = serde_json::to_value(default) else { return false };
    json_equal(&default, value)
}

/// Equality that treats `42` and `42.0` as the same number.
fn json_equal(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value as J;
    match (a, b) {
        (J::Number(x), J::Number(y)) => x.as_f64() == y.as_f64(),
        (J::Array(x), J::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| json_equal(x, y))
        }
        _ => a == b,
    }
}

fn json_to_edit(value: &serde_json::Value) -> Option<toml_edit::Value> {
    use serde_json::Value as J;
    Some(match value {
        J::Bool(b) => toml_edit::Value::from(*b),
        J::Number(n) => match n.as_i64() {
            Some(i) => toml_edit::Value::from(i),
            None => toml_edit::Value::from(n.as_f64()?),
        },
        J::String(s) => toml_edit::Value::from(s.as_str()),
        J::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(json_to_edit(item)?);
            }
            toml_edit::Value::Array(array)
        }
        J::Null | J::Object(_) => return None,
    })
}

fn insert_key(doc: &mut DocumentMut, segments: &[String], value: toml_edit::Value) {
    let Some((leaf, tables)) = segments.split_last() else { return };
    let mut table = doc.as_table_mut();
    for name in tables {
        let entry = table.entry(name).or_insert_with(toml_edit::table);
        if !entry.is_table() {
            *entry = toml_edit::table();
        }
        let Some(next) = entry.as_table_mut() else { return };
        table = next;
    }
    match table.get_mut(leaf).and_then(toml_edit::Item::as_value_mut) {
        // Keep the decoration (trailing comment) of an existing line.
        Some(existing) => {
            let decor = existing.decor().clone();
            *existing = value;
            *existing.decor_mut() = decor;
        }
        None => {
            table.insert(leaf, toml_edit::Item::Value(value));
        }
    }
}

/// Removes a key, then any table it leaves empty (so a reset does not leave a bare
/// `[section]` header behind).
fn remove_key(doc: &mut DocumentMut, segments: &[String]) {
    fn remove_in(table: &mut toml_edit::Table, segments: &[String]) {
        match segments {
            [] => {}
            [leaf] => {
                table.remove(leaf);
            }
            [name, rest @ ..] => {
                let Some(child) = table.get_mut(name).and_then(toml_edit::Item::as_table_mut)
                else {
                    return;
                };
                remove_in(child, rest);
                if child.is_empty() {
                    table.remove(name);
                }
            }
        }
    }
    remove_in(doc.as_table_mut(), segments);
}

/// Every leaf whose value differs between two configs, as dotted paths.
fn diff(old: &Config, new: &Config) -> Vec<(String, serde_json::Value)> {
    let flatten = |config: &Config| {
        let mut out = BTreeMap::new();
        if let Ok(value) = serde_json::to_value(config) {
            flatten_into(&value, String::new(), &mut out);
        }
        out
    };
    let (old, new) = (flatten(old), flatten(new));
    let mut changed: Vec<(String, serde_json::Value)> = new
        .iter()
        .filter(|(path, value)| old.get(*path) != Some(value))
        .map(|(path, value)| (path.clone(), value.clone()))
        .collect();
    // A free-form key that disappeared is reported as null.
    changed.extend(
        old.keys()
            .filter(|path| !new.contains_key(*path))
            .map(|p| (p.clone(), serde_json::Value::Null)),
    );
    changed
}

fn flatten_into(
    value: &serde_json::Value,
    prefix: String,
    out: &mut BTreeMap<String, serde_json::Value>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                flatten_into(child, path, out);
            }
        }
        leaf => {
            out.insert(prefix, leaf.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_store(name: &str, initial: Option<&str>) -> (Arc<ConfigStore>, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("vihpluh-config-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        if let Some(text) = initial {
            std::fs::write(&path, text).unwrap();
        }
        (ConfigStore::open(path.clone()).0, dir)
    }

    fn recorder(store: &ConfigStore) -> Arc<Mutex<Vec<ConfigChange>>> {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        store.on_change(move |change| sink.lock().unwrap().push(change.clone()));
        seen
    }

    #[test]
    fn set_writes_only_the_override_and_reports_it() {
        let (store, dir) = temp_store("set", None);
        let seen = recorder(&store);
        store.set("subtitles.font_size", &json!(46)).unwrap();

        let text = std::fs::read_to_string(store.path()).unwrap();
        assert!(text.contains("schema = 1"));
        assert!(text.contains("[subtitles]"));
        assert!(text.contains("font_size = 46"));
        assert!(!text.contains("[interface]"), "untouched sections are not written");
        assert_eq!(store.get().subtitles.font_size, 46.0);

        let changes = seen.lock().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].changed, vec![("subtitles.font_size".to_string(), json!(46.0))]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn setting_the_default_removes_the_key() {
        let (store, dir) = temp_store("default", Some("schema = 1\n[audio]\nmax_volume = 150\n"));
        assert_eq!(store.get().audio.max_volume, 150);
        store.set("audio.max_volume", &json!(130)).unwrap();
        let text = std::fs::read_to_string(store.path()).unwrap();
        assert!(!text.contains("max_volume"), "{text}");
        assert!(!text.contains("[audio]"), "an emptied section goes too: {text}");
        assert_eq!(store.get().audio.max_volume, 130);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn comments_and_order_survive_a_write() {
        let original = "# my settings\nschema = 1\n\n[interface]\n# I like blue\naccent = \"#3E8BFF\" # keep\nseek_waveform = true\n";
        let (store, dir) = temp_store("comments", Some(original));
        store.set("interface.accent", &json!("#E5484D")).unwrap();
        let text = std::fs::read_to_string(store.path()).unwrap();
        assert!(text.starts_with("# my settings\n"));
        assert!(text.contains("# I like blue\naccent = \"#E5484D\" # keep\n"), "{text}");
        assert!(text.contains("seek_waveform = true"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_values_are_rejected_and_the_file_is_untouched() {
        let (store, dir) = temp_store("invalid", Some("schema = 1\n"));
        let seen = recorder(&store);
        for (path, value) in [
            ("interface.ui_scale", json!(9.0)),
            ("interface.theme", json!("neon")),
            ("interface.accent", json!("orange")),
            ("audio.max_volume", json!("loud")),
        ] {
            let err = store.set(path, &value).unwrap_err();
            assert_eq!(err.code, crate::error::ErrorCode::InvalidArgument, "{path}");
        }
        assert_eq!(std::fs::read_to_string(store.path()).unwrap(), "schema = 1\n");
        assert!(seen.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unknown_paths_and_sections_are_refused() {
        let (store, dir) = temp_store("unknown", None);
        assert_eq!(
            store.set("interface.sparkles", &json!(true)).unwrap_err().code,
            crate::error::ErrorCode::NotFound
        );
        assert_eq!(
            store.set("interface", &json!(1)).unwrap_err().code,
            crate::error::ErrorCode::InvalidArgument
        );
        assert!(store.set("schema", &json!(2)).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn free_form_mpv_options_can_be_set_and_removed() {
        let (store, dir) = temp_store("mpv", None);
        store.set("mpv.options.dither-depth", &json!("auto")).unwrap();
        assert_eq!(
            store.get().mpv.options.get("dither-depth"),
            Some(&toml::Value::String("auto".into()))
        );
        let seen = recorder(&store);
        store.reset(Some("mpv.options.dither-depth")).unwrap();
        assert!(store.get().mpv.options.is_empty());
        assert_eq!(
            seen.lock().unwrap()[0].changed,
            vec![("mpv.options.dither-depth".to_string(), serde_json::Value::Null)]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reset_all_keeps_advanced_tables() {
        let original = "schema = 1\n[interface]\ntheme = \"light\"\n[audio]\nmax_volume = 150\n[mpv.options]\nscale = \"ewa_lanczos\"\n";
        let (store, dir) = temp_store("reset", Some(original));
        store.reset(None).unwrap();
        let config = store.get();
        assert_eq!(config.interface.theme, super::super::schema::Theme::Dark);
        assert_eq!(config.audio.max_volume, 130);
        assert_eq!(config.mpv.options.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn external_edits_are_picked_up_by_reload() {
        let (store, dir) = temp_store("reload", None);
        let seen = recorder(&store);
        std::fs::write(store.path(), "schema = 1\n[audio]\nmax_volume = 110\n").unwrap();
        store.reload();
        store.reload();
        let changes = seen.lock().unwrap();
        assert_eq!(changes.len(), 1, "a reload without differences reports nothing");
        assert_eq!(changes[0].changed, vec![("audio.max_volume".to_string(), json!(110))]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn new_warnings_are_reported_once() {
        let (store, dir) = temp_store("warn", None);
        let seen = recorder(&store);
        std::fs::write(store.path(), "schema = 1\n[interface]\ntheme = \"neon\"\n").unwrap();
        store.reload();
        store.reload();
        let changes = seen.lock().unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(
            changes[0].new_warnings.as_slice(),
            [ConfigWarning::InvalidValue { path, .. }] if path == "interface.theme"
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_watcher_applies_an_edit_made_in_another_program() {
        let (store, dir) = temp_store("watch", None);
        let (tx, rx) = mpsc::channel();
        store.on_change(move |change| {
            let _ = tx.send(change.changed.clone());
        });
        store.watch().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        std::fs::write(store.path(), "schema = 1\n[controls]\nvolume_step = 2\n").unwrap();
        let changed = rx.recv_timeout(Duration::from_secs(5)).expect("a change notification");
        assert_eq!(changed, vec![("controls.volume_step".to_string(), json!(2.0))]);
        assert_eq!(store.get().controls.volume_step, 2.0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
