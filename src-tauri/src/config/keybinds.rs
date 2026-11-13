//! The keymap: a shipped base (`resources/keymaps/<base>.toml`) with the user's
//! `keybinds.toml` layered on top (KEYBINDS_AND_CONFIG §3). The frontend receives the
//! resolved result and does all key matching; the core only owns the files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use notify::RecommendedWatcher;
use serde::Serialize;
use toml_edit::{DocumentMut, Item};
use ts_rs::TS;

use crate::error::AppResult;

const VIHPLUH_KEYMAP: &str = include_str!("../../resources/keymaps/vihpluh.toml");

/// The modifier tokens, in the order they are always written (§3.1).
const MODIFIERS: [&str; 4] = ["Ctrl", "Alt", "Shift", "Meta"];

/// Key tokens with a fixed spelling (letters, digits, and F-keys are handled separately).
const NAMED_KEYS: &[&str] = &[
    "Space",
    "Enter",
    "Escape",
    "Tab",
    "Backspace",
    "Delete",
    "Insert",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Up",
    "Down",
    "Left",
    "Right",
    "ContextMenu",
    "`",
    "-",
    "=",
    "[",
    "]",
    "\\",
    ";",
    "'",
    ",",
    ".",
    "/",
    "NumAdd",
    "NumSubtract",
    "NumMultiply",
    "NumDivide",
    "NumEnter",
    "NumDecimal",
    "MouseLeft",
    "MouseLeftDouble",
    "MouseMiddle",
    "MouseRight",
    "MouseBack",
    "MouseForward",
    "WheelUp",
    "WheelDown",
    "WheelLeft",
    "WheelRight",
    "MediaPlayPause",
    "MediaStop",
    "MediaTrackNext",
    "MediaTrackPrevious",
];

/// Actions that only `[[custom]]` bindings may use, because they take arguments (§3.4).
pub const PARAMETERISED_ACTIONS: &[&str] = &[
    "player.seek_relative",
    "player.seek_absolute",
    "player.set_speed",
    "player.set_volume",
    "audio.set_delay",
    "subtitles.set_delay",
    "audio.set_equalizer_preset",
    "video.set_aspect",
    "presets.apply",
    "player.set_sleep_timer",
    "mpv.command",
];

/// One `[[custom]]` entry, validated.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CustomBinding {
    pub keys: Vec<String>,
    pub action: String,
    #[ts(type = "Record<string, unknown>")]
    pub args: serde_json::Value,
    pub label: Option<String>,
}

/// What the frontend matches keys against.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResolvedKeymap {
    /// `"vihpluh" | "vlc" | "mpv" | "none"`
    pub base: String,
    /// Context (`"global"`, `"queue"`, …) → action id → keys.
    pub contexts: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// Evaluated before the context bindings.
    pub custom: Vec<CustomBinding>,
    /// Problems in `keybinds.toml`, readable as-is.
    pub warnings: Vec<String>,
}

/// Normalises a binding string: modifiers in canonical order, canonical key spelling.
/// Returns `None` for anything that is not a valid binding.
pub fn normalize_binding(binding: &str) -> Option<String> {
    let binding = binding.trim();
    if binding.is_empty() {
        return None;
    }
    // "+" itself is never a key on a US layout ("=" with Shift), so splitting on it is safe;
    // a trailing "+" would be an empty key and is rejected.
    let parts: Vec<&str> = binding.split('+').collect();
    let (key, modifiers) = parts.split_last()?;
    let mut present = [false; 4];
    for modifier in modifiers {
        let index = MODIFIERS.iter().position(|m| m.eq_ignore_ascii_case(modifier.trim()))?;
        if present[index] {
            return None;
        }
        present[index] = true;
    }
    let key = canonical_key(key.trim())?;
    let mut out: Vec<&str> =
        MODIFIERS.iter().zip(present).filter(|(_, on)| *on).map(|(m, _)| *m).collect();
    out.push(&key);
    Some(out.join("+"))
}

fn canonical_key(key: &str) -> Option<String> {
    if key.len() == 1 {
        let c = key.chars().next()?;
        if c.is_ascii_alphanumeric() {
            return Some(c.to_ascii_uppercase().to_string());
        }
    }
    if let Some(named) = NAMED_KEYS.iter().find(|k| k.eq_ignore_ascii_case(key)) {
        return Some((*named).to_string());
    }
    let upper = key.to_ascii_uppercase();
    if let Some(n) = upper.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
        if (1..=24).contains(&n) {
            return Some(format!("F{n}"));
        }
    }
    if let Some(n) = key.strip_prefix("Num").and_then(|n| n.parse::<u8>().ok()) {
        if n <= 9 {
            return Some(format!("Num{n}"));
        }
    }
    None
}

/// `Tab` and `Shift+Tab` belong to focus traversal and cannot be bound (§3.1).
fn is_reserved(binding: &str) -> bool {
    matches!(binding, "Tab" | "Shift+Tab")
}

/// Resolves a base keymap plus an optional user file.
pub fn resolve(base_text: &str, base_name: &str, user_text: Option<&str>) -> ResolvedKeymap {
    let mut warnings = Vec::new();
    let mut contexts = parse_contexts(base_text, &mut Vec::new());

    let mut custom = Vec::new();
    if let Some(text) = user_text {
        match text.parse::<DocumentMut>() {
            Ok(doc) => {
                let user = read_contexts(&doc, &mut warnings);
                for (context, actions) in user {
                    let target = contexts.entry(context.clone()).or_default();
                    // Track which user line claimed each key, to report user-vs-user clashes.
                    let mut claimed: BTreeMap<String, String> = BTreeMap::new();
                    for (action, keys) in actions {
                        for key in &keys {
                            if let Some(previous) = claimed.insert(key.clone(), action.clone()) {
                                if previous != action {
                                    warnings.push(format!(
                                        "`{key}` is bound to both {previous} and {action} in \
                                         [{context}]; {action} (the later line) wins"
                                    ));
                                }
                            }
                            // The user file wins over the base: take the key from any other
                            // action in this context.
                            for (other, other_keys) in target.iter_mut() {
                                if *other != action {
                                    other_keys.retain(|k| k != key);
                                }
                            }
                        }
                        target.insert(action, keys);
                    }
                }
                custom = read_custom(&doc, &mut warnings);
            }
            Err(err) => {
                warnings.push(format!("keybinds.toml has a syntax error: {}", err.message()))
            }
        }
    }

    ResolvedKeymap { base: base_name.to_string(), contexts, custom, warnings }
}

fn parse_contexts(
    text: &str,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, BTreeMap<String, Vec<String>>> {
    match text.parse::<DocumentMut>() {
        Ok(doc) => read_contexts(&doc, warnings)
            .into_iter()
            .map(|(context, actions)| (context, actions.into_iter().collect()))
            .collect(),
        Err(err) => {
            warnings.push(format!("keymap has a syntax error: {}", err.message()));
            BTreeMap::new()
        }
    }
}

/// Reads `[bind]` (context `global`) and `[context.<name>.bind]` in file order.
fn read_contexts(
    doc: &DocumentMut,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, Vec<(String, Vec<String>)>> {
    let mut out: BTreeMap<String, Vec<(String, Vec<String>)>> = BTreeMap::new();
    let mut read_table = |context: &str, item: &Item, warnings: &mut Vec<String>| {
        let Some(table) = item.as_table_like() else {
            warnings.push(format!("[{context}] bindings must be a table"));
            return;
        };
        let entries = out.entry(context.to_string()).or_default();
        for (action, value) in table.iter() {
            let Some(array) = value.as_array() else {
                warnings.push(format!("`{action}` needs a list of keys, like [\"Space\"]"));
                continue;
            };
            entries.push((action.to_string(), keys_from(array, action, warnings)));
        }
    };
    if let Some(bind) = doc.get("bind") {
        read_table("global", bind, warnings);
    }
    if let Some(contexts) = doc.get("context").and_then(Item::as_table_like) {
        for (name, table) in contexts.iter() {
            if let Some(bind) = table.as_table_like().and_then(|t| t.get("bind")) {
                read_table(name, bind, warnings);
            }
        }
    }
    out
}

fn keys_from(array: &toml_edit::Array, action: &str, warnings: &mut Vec<String>) -> Vec<String> {
    let mut keys = Vec::new();
    for value in array.iter() {
        let Some(raw) = value.as_str() else {
            warnings.push(format!("`{action}`: keys must be strings"));
            continue;
        };
        match normalize_binding(raw) {
            Some(key) if is_reserved(&key) => {
                warnings.push(format!("`{action}`: {key} is reserved for keyboard focus"));
            }
            Some(key) => {
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
            None => warnings.push(format!("`{action}`: \"{raw}\" is not a key")),
        }
    }
    keys
}

fn read_custom(doc: &DocumentMut, warnings: &mut Vec<String>) -> Vec<CustomBinding> {
    let Some(array) = doc.get("custom").and_then(Item::as_array_of_tables) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (index, table) in array.iter().enumerate() {
        let n = index + 1;
        let Some(action) = table.get("action").and_then(Item::as_str) else {
            warnings.push(format!("[[custom]] #{n} has no action"));
            continue;
        };
        if !PARAMETERISED_ACTIONS.contains(&action) {
            warnings.push(format!("[[custom]] #{n}: `{action}` cannot be used with arguments"));
            continue;
        }
        let keys = match table.get("keys").and_then(Item::as_array) {
            Some(array) => keys_from(array, action, warnings),
            None => {
                warnings.push(format!("[[custom]] #{n} has no keys"));
                continue;
            }
        };
        let args = table
            .get("args")
            .map(|item| item_to_json(item, warnings))
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
        let label = table.get("label").and_then(Item::as_str).map(str::to_string);
        out.push(CustomBinding { keys, action: action.to_string(), args, label });
    }
    out
}

fn item_to_json(item: &Item, warnings: &mut Vec<String>) -> serde_json::Value {
    let text = match item {
        Item::Value(value) => value.to_string(),
        other => other.to_string(),
    };
    // Re-parse through the `toml` crate, whose values serialise straight into JSON.
    match format!("v = {text}").parse::<toml::Table>() {
        Ok(table) => table
            .get("v")
            .and_then(|v| serde_json::to_value(v).ok())
            .unwrap_or(serde_json::Value::Null),
        Err(err) => {
            warnings.push(format!("[[custom]] args are not valid: {}", err.message()));
            serde_json::Value::Null
        }
    }
}

/// The text of a shipped base keymap. `vlc` and `mpv` arrive in Phase 10; until then they
/// fall back to the vihpluh layout. `none` starts empty.
fn base_text(base: &str) -> &'static str {
    match base {
        "none" => "schema = 1\n",
        _ => VIHPLUH_KEYMAP,
    }
}

type ChangeCallback = Box<dyn Fn(&ResolvedKeymap) + Send + Sync>;

/// Owns `keybinds.toml`: loads, re-resolves on external edits, reports changes.
pub struct KeybindStore {
    path: PathBuf,
    current: Mutex<Arc<ResolvedKeymap>>,
    on_change: OnceLock<ChangeCallback>,
    watcher: Mutex<Option<RecommendedWatcher>>,
}

impl KeybindStore {
    pub fn open(path: PathBuf) -> Arc<Self> {
        let resolved = load(&path);
        for warning in &resolved.warnings {
            tracing::warn!("keybinds: {warning}");
        }
        Arc::new(Self {
            path,
            current: Mutex::new(Arc::new(resolved)),
            on_change: OnceLock::new(),
            watcher: Mutex::new(None),
        })
    }

    pub fn get(&self) -> Arc<ResolvedKeymap> {
        match self.current.lock() {
            Ok(current) => current.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub fn on_change(&self, callback: impl Fn(&ResolvedKeymap) + Send + Sync + 'static) {
        let _ = self.on_change.set(Box::new(callback));
    }

    pub fn reload(&self) {
        let resolved = load(&self.path);
        let changed = {
            let mut current = match self.current.lock() {
                Ok(current) => current,
                Err(poisoned) => poisoned.into_inner(),
            };
            if **current == resolved {
                false
            } else {
                *current = Arc::new(resolved.clone());
                true
            }
        };
        if changed {
            tracing::info!("keybinds reloaded");
            for warning in &resolved.warnings {
                tracing::warn!("keybinds: {warning}");
            }
            if let Some(callback) = self.on_change.get() {
                callback(&resolved);
            }
        }
    }

    pub fn watch(self: &Arc<Self>) -> AppResult<()> {
        let store = Arc::downgrade(self);
        let watcher =
            super::watch::watch_file(&self.path, "keybinds", move || match store.upgrade() {
                Some(store) => {
                    store.reload();
                    true
                }
                None => false,
            })?;
        if let Ok(mut slot) = self.watcher.lock() {
            *slot = Some(watcher);
        }
        Ok(())
    }
}

fn load(path: &Path) -> ResolvedKeymap {
    let user = std::fs::read_to_string(path).ok();
    let base = user
        .as_deref()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|table| table.get("base").and_then(|b| b.as_str()).map(str::to_string))
        .unwrap_or_else(|| "vihpluh".to_string());
    let mut resolved = resolve(base_text(&base), &base, user.as_deref());
    if !matches!(base.as_str(), "vihpluh" | "vlc" | "mpv" | "none") {
        resolved.warnings.push(format!("unknown base keymap `{base}`; using vihpluh"));
        resolved.base = "vihpluh".to_string();
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn global(map: &ResolvedKeymap) -> &BTreeMap<String, Vec<String>> {
        &map.contexts["global"]
    }

    #[test]
    fn normalizes_bindings() {
        assert_eq!(normalize_binding("ctrl+shift+p").as_deref(), Some("Ctrl+Shift+P"));
        assert_eq!(normalize_binding("Shift+Ctrl+Left").as_deref(), Some("Ctrl+Shift+Left"));
        assert_eq!(normalize_binding("space").as_deref(), Some("Space"));
        assert_eq!(normalize_binding("f11").as_deref(), Some("F11"));
        assert_eq!(normalize_binding("Alt+=").as_deref(), Some("Alt+="));
        assert_eq!(normalize_binding("\\").as_deref(), Some("\\"));
        assert_eq!(normalize_binding("Num4").as_deref(), Some("Num4"));
        assert_eq!(normalize_binding("wheelup").as_deref(), Some("WheelUp"));
        for bad in ["", "Ctrl+", "Hyper+K", "Ctrl+Ctrl+K", "F25", "Enterr", "Ctrl+K+L"] {
            assert_eq!(normalize_binding(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_shipped_keymap_is_clean_and_complete() {
        let map = resolve(VIHPLUH_KEYMAP, "vihpluh", None);
        assert!(map.warnings.is_empty(), "{:?}", map.warnings);
        let g = global(&map);
        assert_eq!(g["player.toggle_pause"], vec!["Space", "K", "MouseLeft"]);
        assert_eq!(g["ui.toggle_sidebar"], vec!["\\"]);
        assert_eq!(g["player.toggle_reverse"], Vec::<String>::new());
        assert_eq!(map.contexts["queue"]["queue.remove_selected"], vec!["Delete"]);
        // No key is claimed twice within one context.
        for (context, actions) in &map.contexts {
            let mut seen = BTreeMap::new();
            for (action, keys) in actions {
                for key in keys {
                    if let Some(other) = seen.insert(key.clone(), action.clone()) {
                        panic!("{key} is bound to {other} and {action} in {context}");
                    }
                }
            }
        }
    }

    #[test]
    fn user_bindings_replace_an_actions_keys_and_steal_conflicting_ones() {
        let user = r#"
            schema = 1
            [bind]
            "window.toggle_zen" = ["Z"]
            "player.seek_back_medium" = []
            "ui.toggle_sidebar" = ["`"]
            "bookmarks.add" = ["M"]
        "#;
        let map = resolve(VIHPLUH_KEYMAP, "vihpluh", Some(user));
        assert!(map.warnings.is_empty(), "{:?}", map.warnings);
        let g = global(&map);
        assert_eq!(g["window.toggle_zen"], vec!["Z"], "middle-click default dropped");
        assert!(g["player.seek_back_medium"].is_empty(), "unbound");
        assert_eq!(g["ui.toggle_sidebar"], vec!["`"]);
        assert_eq!(g["bookmarks.add"], vec!["M"]);
        assert!(g["player.toggle_mute"].is_empty(), "M moved to bookmarks.add");
        assert_eq!(g["player.toggle_pause"], vec!["Space", "K", "MouseLeft"], "untouched");
    }

    #[test]
    fn a_clash_between_two_user_lines_is_reported_and_the_later_wins() {
        let user = "[bind]\n\"player.toggle_mute\" = [\"Y\"]\n\"window.toggle_zen\" = [\"Y\"]\n";
        let map = resolve(VIHPLUH_KEYMAP, "vihpluh", Some(user));
        let g = global(&map);
        assert_eq!(g["window.toggle_zen"], vec!["Y"]);
        assert!(g["player.toggle_mute"].is_empty());
        assert_eq!(map.warnings.len(), 1, "{:?}", map.warnings);
        assert!(map.warnings[0].contains("window.toggle_zen"));
    }

    #[test]
    fn context_bindings_layer_per_context() {
        let user =
            "[context.queue.bind]\n\"queue.remove_selected\" = [\"Delete\", \"Backspace\"]\n";
        let map = resolve(VIHPLUH_KEYMAP, "vihpluh", Some(user));
        assert_eq!(map.contexts["queue"]["queue.remove_selected"], vec!["Delete", "Backspace"]);
        assert_eq!(global(&map)["player.speed_reset"], vec!["Backspace"], "global untouched");
    }

    #[test]
    fn reserved_and_invalid_keys_are_refused_with_a_reason() {
        let user = "[bind]\n\"ui.toggle_sidebar\" = [\"Tab\", \"Hyper+X\", 3]\n";
        let map = resolve(VIHPLUH_KEYMAP, "vihpluh", Some(user));
        assert!(global(&map)["ui.toggle_sidebar"].is_empty());
        assert_eq!(map.warnings.len(), 3, "{:?}", map.warnings);
    }

    #[test]
    fn custom_bindings_are_validated() {
        let user = r#"
            [[custom]]
            keys = ["Ctrl+Shift+Right"]
            action = "player.seek_relative"
            args = { seconds = 85 }

            [[custom]]
            keys = ["Ctrl+D"]
            label = "Toggle deinterlace"
            action = "mpv.command"
            args = { command = ["cycle", "deinterlace"] }

            [[custom]]
            keys = ["F9"]
            action = "player.toggle_pause"
        "#;
        let map = resolve(VIHPLUH_KEYMAP, "vihpluh", Some(user));
        assert_eq!(map.custom.len(), 2);
        assert_eq!(map.custom[0].args, serde_json::json!({ "seconds": 85 }));
        assert_eq!(map.custom[1].label.as_deref(), Some("Toggle deinterlace"));
        assert_eq!(map.custom[1].args, serde_json::json!({ "command": ["cycle", "deinterlace"] }));
        assert_eq!(map.warnings.len(), 1, "a plain action in [[custom]] is refused");
    }

    #[test]
    fn a_broken_user_file_keeps_the_base() {
        let map = resolve(VIHPLUH_KEYMAP, "vihpluh", Some("[bind\n"));
        assert_eq!(map.warnings.len(), 1);
        assert_eq!(global(&map)["player.toggle_pause"], vec!["Space", "K", "MouseLeft"]);
    }

    #[test]
    fn base_none_starts_empty() {
        let dir = std::env::temp_dir().join(format!("vihpluh-keys-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("keybinds.toml");
        std::fs::write(&path, "schema = 1\nbase = \"none\"\n[bind]\n\"app.quit\" = [\"Ctrl+Q\"]\n")
            .unwrap();
        let map = load(&path);
        assert_eq!(map.base, "none");
        assert_eq!(global(&map).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
