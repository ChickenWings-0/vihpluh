//! Presets (ARCHITECTURE §10): sparse bundles of audio / subtitle / video / playback settings
//! plus a rule saying which files they apply to. One human-editable TOML file per preset in
//! `<config>/presets/`. This module is the schema and the file store; `matcher` decides which
//! presets hit a file, `rules` resolves track rules, and `player::presets` applies the result.

pub mod matcher;
pub mod rules;
pub mod series;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use rules::{SubTrackRule, TrackRule};

pub const SCHEMA_VERSION: u32 = 1;

fn schema_version() -> u32 {
    SCHEMA_VERSION
}

fn yes() -> bool {
    true
}

/// One preset, exactly as its file reads (§10.1). Only keys that are present are applied.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Preset {
    #[serde(default = "schema_version")]
    pub schema: u32,
    /// The file name stem: `[a-z0-9-]+`.
    pub id: String,
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Tie-breaker among equally specific matches (higher applies later).
    #[serde(default)]
    pub priority: i32,
    #[serde(rename = "match")]
    pub matcher: PresetMatch,
    #[serde(flatten)]
    pub layer: Layer,
}

/// The shape a preset is read in. The same keys as [`Preset`], but with the sections as
/// plain fields: through a flattened struct the TOML parser loses the position of an error,
/// and the message must name the line.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetFile {
    #[serde(default = "schema_version")]
    schema: u32,
    id: String,
    name: String,
    #[serde(default = "yes")]
    enabled: bool,
    #[serde(default)]
    priority: i32,
    #[serde(rename = "match")]
    matcher: PresetMatch,
    #[serde(default)]
    audio: AudioSection,
    #[serde(default)]
    subtitles: SubSection,
    #[serde(default)]
    video: VideoSection,
    #[serde(default)]
    playback: PlaybackSection,
}

impl<'de> Deserialize<'de> for Preset {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        PresetFile::deserialize(deserializer).map(Into::into)
    }
}

impl From<PresetFile> for Preset {
    fn from(file: PresetFile) -> Self {
        Self {
            schema: file.schema,
            id: file.id,
            name: file.name,
            enabled: file.enabled,
            priority: file.priority,
            matcher: file.matcher,
            layer: Layer {
                audio: file.audio,
                subtitles: file.subtitles,
                video: file.video,
                playback: file.playback,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MatchType {
    /// Listed least specific first (§10.4).
    Glob,
    Series,
    FolderRecursive,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PresetMatch {
    #[serde(rename = "type")]
    pub kind: MatchType,
    /// `type = "series"`: the normalised series key (§10.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub series: Option<String>,
    /// `type = "folder"` / `"folder_recursive"`: a directory (forward slashes, `~` allowed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub path: Option<String>,
    /// `type = "glob"`: matched against the full path or URL, case-insensitive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub glob: Option<String>,
}

/// The settings a preset carries; also the sum of several presets layered key by key.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Layer {
    #[serde(default, skip_serializing_if = "AudioSection::is_empty")]
    pub audio: AudioSection,
    #[serde(default, skip_serializing_if = "SubSection::is_empty")]
    pub subtitles: SubSection,
    #[serde(default, skip_serializing_if = "VideoSection::is_empty")]
    pub video: VideoSection,
    #[serde(default, skip_serializing_if = "PlaybackSection::is_empty")]
    pub playback: PlaybackSection,
}

macro_rules! section {
    ($(#[$meta:meta])* $name:ident { $($(#[$fmeta:meta])* $field:ident : $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
        #[serde(deny_unknown_fields)]
        #[ts(export, optional_fields)]
        pub struct $name {
            $(
                $(#[$fmeta])*
                #[serde(default, skip_serializing_if = "Option::is_none")]
                pub $field: Option<$ty>,
            )*
        }

        impl $name {
            pub fn is_empty(&self) -> bool {
                *self == Self::default()
            }
        }
    };
}

section!(AudioSection {
    track: TrackRule,
    delay_ms: i32,
    /// An EQ preset id, or `"off"`.
    equalizer: String,
    normalize: bool,
});

section!(StyleSection {
    font: String,
    size: f64,
    scale: f64,
    bold: bool,
    color: String,
    outline_color: String,
    outline_size: f64,
    shadow_offset: f64,
    background: String,
    background_color: String,
    position: f64,
    margin_y: f64,
    ass_override: String,
});

section!(VideoSection {
    brightness: f64,
    contrast: f64,
    saturation: f64,
    gamma: f64,
    hue: f64,
    rotate: u32,
    flip_h: bool,
    flip_v: bool,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    aspect: String,
    tone_mapping: String,
    deband: bool,
    interpolation: bool,
    /// File names in `<config>/shaders/`.
    shaders: Vec<String>,
});

section!(PlaybackSection { speed: f64 });

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct SubSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<SubTrackRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<i32>,
    #[serde(default, skip_serializing_if = "StyleSection::is_empty")]
    #[ts(optional = nullable)]
    pub style: StyleSection,
}

impl SubSection {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A layer as a flat map from dotted key (`subtitles.style.size`) to value: the common
/// currency for layering, comparing live values with a preset, and writing changes back.
pub type Flat = BTreeMap<String, serde_json::Value>;

fn flatten_into(prefix: &str, value: &serde_json::Value, out: &mut Flat) {
    match value {
        // A track rule is one value, not a table of settings.
        serde_json::Value::Object(map) if !prefix.ends_with(".track") => {
            for (key, child) in map {
                let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                flatten_into(&path, child, out);
            }
        }
        serde_json::Value::Null => {}
        other => {
            out.insert(prefix.to_string(), other.clone());
        }
    }
}

impl Layer {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn flat(&self) -> Flat {
        let mut out = Flat::new();
        if let Ok(value) = serde_json::to_value(self) {
            flatten_into("", &value, &mut out);
        }
        out
    }

    /// Rebuilds a layer from flat keys. Keys or values that do not fit the schema fail.
    pub fn from_flat(flat: &Flat) -> Result<Self, String> {
        let mut root = serde_json::Map::new();
        for (path, value) in flat {
            let mut node = &mut root;
            let mut parts = path.split('.').peekable();
            while let Some(part) = parts.next() {
                if parts.peek().is_none() {
                    node.insert(part.to_string(), value.clone());
                    break;
                }
                let child = node
                    .entry(part.to_string())
                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
                let serde_json::Value::Object(next) = child else {
                    return Err(format!("`{path}` is not a preset setting"));
                };
                node = next;
            }
        }
        serde_json::from_value(serde_json::Value::Object(root))
            .map_err(|e| format!("not a valid preset setting: {e}"))
    }

    /// Lays `other` over this layer: every key `other` sets replaces ours.
    pub fn overlay(&mut self, other: &Layer) {
        let mut flat = self.flat();
        flat.extend(other.flat());
        if let Ok(merged) = Self::from_flat(&flat) {
            *self = merged;
        }
    }

    /// The part of this layer made of the given keys.
    pub fn only(&self, keys: &[String]) -> Layer {
        let flat: Flat = self.flat().into_iter().filter(|(key, _)| keys.contains(key)).collect();
        Self::from_flat(&flat).unwrap_or_default()
    }

    /// The top-level sections this layer has anything in.
    pub fn sections(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .flat()
            .keys()
            .filter_map(|key| key.split('.').next().map(str::to_string))
            .collect();
        names.dedup();
        names
    }
}

/// A row of the presets list.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PresetSummary {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub match_type: MatchType,
    /// The series key, folder, or pattern the preset applies to.
    pub match_value: String,
    /// Which of audio / subtitles / video / playback it sets anything in.
    pub sections: Vec<String>,
}

impl Preset {
    pub fn summary(&self) -> PresetSummary {
        let m = &self.matcher;
        let value = match m.kind {
            MatchType::Series => m.series.clone(),
            MatchType::Folder | MatchType::FolderRecursive => m.path.clone(),
            MatchType::Glob => m.glob.clone(),
        };
        PresetSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            enabled: self.enabled,
            priority: self.priority,
            match_type: m.kind,
            match_value: value.unwrap_or_default(),
            sections: self.layer.sections(),
        }
    }

    /// Checks what the type system cannot: the id's shape, the match rule's field, ranges.
    pub fn validate(&self) -> Result<(), String> {
        if !is_valid_id(&self.id) {
            return Err(format!("id `{}` must be lowercase letters, digits, and hyphens", self.id));
        }
        if self.name.trim().is_empty() {
            return Err("name is empty".into());
        }
        let m = &self.matcher;
        let (field, value) = match m.kind {
            MatchType::Series => ("series", &m.series),
            MatchType::Folder | MatchType::FolderRecursive => ("path", &m.path),
            MatchType::Glob => ("glob", &m.glob),
        };
        if value.as_deref().is_none_or(|v| v.trim().is_empty()) {
            return Err(format!("[match] needs `{field}` for this type"));
        }
        let range = |name: &str, value: Option<f64>, low: f64, high: f64| match value {
            Some(v) if !(low..=high).contains(&v) => {
                Err(format!("{name} must be between {low} and {high}"))
            }
            _ => Ok(()),
        };
        let (audio, subs, video) = (&self.layer.audio, &self.layer.subtitles, &self.layer.video);
        range("audio.delay_ms", audio.delay_ms.map(f64::from), -600_000.0, 600_000.0)?;
        range("subtitles.delay_ms", subs.delay_ms.map(f64::from), -600_000.0, 600_000.0)?;
        range("subtitles.style.size", subs.style.size, 10.0, 120.0)?;
        range("subtitles.style.scale", subs.style.scale, 0.5, 3.0)?;
        range("subtitles.style.position", subs.style.position, 0.0, 100.0)?;
        for (name, value) in [
            ("video.brightness", video.brightness),
            ("video.contrast", video.contrast),
            ("video.saturation", video.saturation),
            ("video.gamma", video.gamma),
            ("video.hue", video.hue),
        ] {
            range(name, value, -100.0, 100.0)?;
        }
        range("playback.speed", self.layer.playback.speed, 0.25, 4.0)?;
        for (name, color) in [
            ("subtitles.style.color", &subs.style.color),
            ("subtitles.style.outline_color", &subs.style.outline_color),
            ("subtitles.style.background_color", &subs.style.background_color),
        ] {
            if color.as_deref().is_some_and(|c| !crate::config::is_hex_color(c)) {
                return Err(format!("{name} must look like #RRGGBB"));
            }
        }
        if let Some(SubTrackRule::Keyword(word)) = &subs.track {
            if word != "off" {
                return Err("subtitles.track must be a rule table or \"off\"".into());
            }
        }
        Ok(())
    }
}

pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

/// An id made from a name: lowercase, runs of anything else become one hyphen.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() { "preset".to_string() } else { out }
}

/// Parses one preset file. Errors name the line, for a toast the user can act on.
pub fn parse(text: &str, file_stem: &str) -> Result<Preset, String> {
    let preset: Preset = toml::from_str(text).map_err(|err| {
        let line =
            err.span().map(|span| text[..span.start.min(text.len())].matches('\n').count() + 1);
        match line {
            Some(line) => format!("line {line}: {}", err.message()),
            None => err.message().to_string(),
        }
    })?;
    if preset.id != file_stem {
        return Err(format!("id `{}` does not match the file name `{file_stem}.toml`", preset.id));
    }
    preset.validate()?;
    Ok(preset)
}

type Listener = Box<dyn Fn() + Send + Sync>;
type ProblemListener = Box<dyn Fn(&str) + Send + Sync>;

/// The presets folder: loaded once, kept current by `reload` (after a save or an edit made
/// in another program).
pub struct PresetStore {
    dir: PathBuf,
    presets: RwLock<Arc<Vec<Preset>>>,
    /// Files that failed to load, as `file: reason`; each is reported once.
    reported: RwLock<Vec<String>>,
    on_change: OnceLock<Listener>,
    on_problem: OnceLock<ProblemListener>,
    watcher: OnceLock<notify::RecommendedWatcher>,
}

impl PresetStore {
    pub fn open(dir: PathBuf) -> Arc<Self> {
        let store = Arc::new(Self {
            dir,
            presets: RwLock::new(Arc::new(Vec::new())),
            reported: RwLock::new(Vec::new()),
            on_change: OnceLock::new(),
            on_problem: OnceLock::new(),
            watcher: OnceLock::new(),
        });
        store.reload();
        store
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn file(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.toml"))
    }

    /// Called after the set of presets changed.
    pub fn on_change(&self, listener: impl Fn() + Send + Sync + 'static) {
        let _ = self.on_change.set(Box::new(listener));
    }

    /// Called once for each preset file that cannot be loaded, with a message naming it.
    pub fn on_problem(&self, listener: impl Fn(&str) + Send + Sync + 'static) {
        let _ = self.on_problem.set(Box::new(listener));
        // Problems found before anyone was listening (startup).
        for problem in self.problems() {
            if let Some(listener) = self.on_problem.get() {
                listener(&problem);
            }
        }
    }

    pub fn list(&self) -> Arc<Vec<Preset>> {
        self.presets.read().map(|p| p.clone()).unwrap_or_default()
    }

    pub fn get(&self, id: &str) -> Option<Preset> {
        self.list().iter().find(|p| p.id == id).cloned()
    }

    pub fn problems(&self) -> Vec<String> {
        self.reported.read().map(|r| r.clone()).unwrap_or_default()
    }

    /// Reads every `*.toml` in the folder. A file that does not load is skipped and
    /// reported; the others still load. Returns whether the set of presets changed.
    pub fn reload(&self) -> bool {
        let mut presets = Vec::new();
        let mut problems = Vec::new();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default();
        files.sort();
        for path in files {
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()) {
                Ok(text) => match parse(&text, stem) {
                    Ok(preset) => presets.push(preset),
                    Err(reason) => problems.push(format!("presets/{name}, {reason}")),
                },
                Err(reason) => problems.push(format!("presets/{name}: {reason}")),
            }
        }
        let fresh: Vec<String> = {
            let known = self.problems();
            problems.iter().filter(|p| !known.contains(p)).cloned().collect()
        };
        if let Ok(mut reported) = self.reported.write() {
            *reported = problems;
        }
        for problem in &fresh {
            tracing::warn!("preset skipped: {problem}");
            if let Some(listener) = self.on_problem.get() {
                listener(problem);
            }
        }
        let changed = *self.list() != presets;
        if changed {
            if let Ok(mut current) = self.presets.write() {
                *current = Arc::new(presets);
            }
            if let Some(listener) = self.on_change.get() {
                listener();
            }
        }
        changed
    }

    /// Writes a preset. An existing file is edited in place, so comments, ordering, and
    /// every key that did not change stay exactly as they were.
    pub fn save(&self, preset: &Preset) -> AppResult<()> {
        preset.validate().map_err(AppError::invalid)?;
        std::fs::create_dir_all(&self.dir)?;
        let path = self.file(&preset.id);
        let wanted = toml::Table::try_from(preset)
            .map_err(|e| AppError::internal(format!("preset could not be written: {e}")))?;
        let mut doc = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| text.parse::<toml_edit::DocumentMut>().ok())
            .unwrap_or_else(|| fresh_document(preset));
        // Header keys a hand-written file left out stay out while they hold their defaults.
        let mut wanted = wanted;
        for (key, default) in [
            ("schema", toml::Value::Integer(i64::from(SCHEMA_VERSION))),
            ("enabled", toml::Value::Boolean(true)),
            ("priority", toml::Value::Integer(0)),
        ] {
            if !doc.contains_key(key) && wanted.get(key) == Some(&default) {
                wanted.remove(key);
            }
        }
        sync_table(doc.as_table_mut(), &wanted);
        let text = doc.to_string();
        // What is on disk must load again as the same preset.
        match parse(&text, &preset.id) {
            Ok(written) if written == *preset => {}
            Ok(_) | Err(_) => {
                return Err(AppError::internal("the preset did not survive being written"));
            }
        }
        crate::media::thumbs::write_atomic(&path, text.as_bytes())?;
        self.reload();
        Ok(())
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        if !is_valid_id(id) {
            return Err(AppError::invalid("not a preset id"));
        }
        match std::fs::remove_file(self.file(id)) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(AppError::not_found(format!("no preset named `{id}`")));
            }
            Err(err) => return Err(err.into()),
        }
        self.reload();
        Ok(())
    }

    /// An id for a new preset called `name` that no existing preset uses.
    pub fn free_id(&self, name: &str) -> String {
        let base = slug(name);
        let taken = |id: &str| self.file(id).exists();
        if !taken(&base) {
            return base;
        }
        (2..).map(|n| format!("{base}-{n}")).find(|id| !taken(id)).unwrap_or(base)
    }

    /// Reloads when a preset file is edited, added, or removed in another program.
    pub fn watch(self: &Arc<Self>) -> AppResult<()> {
        let store = Arc::downgrade(self);
        let watcher = crate::config::watch::watch_dir(&self.dir, "presets", "toml", move || {
            match store.upgrade() {
                Some(store) => {
                    store.reload();
                    true
                }
                None => false,
            }
        })?;
        let _ = self.watcher.set(watcher);
        Ok(())
    }
}

fn to_edit(value: &toml::Value) -> toml_edit::Value {
    match value {
        toml::Value::String(s) => toml_edit::Value::from(s.as_str()),
        toml::Value::Integer(i) => toml_edit::Value::from(*i),
        toml::Value::Float(f) => toml_edit::Value::from(*f),
        toml::Value::Boolean(b) => toml_edit::Value::from(*b),
        toml::Value::Datetime(d) => toml_edit::Value::from(d.to_string()),
        toml::Value::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(to_edit(item));
            }
            toml_edit::Value::Array(array)
        }
        toml::Value::Table(table) => {
            let mut inline = toml_edit::InlineTable::new();
            for (key, item) in table {
                inline.insert(key, to_edit(item));
            }
            toml_edit::Value::InlineTable(inline)
        }
    }
}

/// The document for a preset that has no file yet: keys in the order the schema lists them
/// (§10.1), track rules inline.
fn fresh_document(preset: &Preset) -> toml_edit::DocumentMut {
    let mut doc = toml::to_string(preset)
        .ok()
        .and_then(|text| text.parse::<toml_edit::DocumentMut>().ok())
        .unwrap_or_default();
    for section in ["audio", "subtitles"] {
        let Some(table) = doc.get_mut(section).and_then(toml_edit::Item::as_table_mut) else {
            continue;
        };
        if let Some(rule) = table.remove("track") {
            let inline = match rule {
                toml_edit::Item::Table(rule) => {
                    toml_edit::Item::Value(toml_edit::Value::InlineTable(rule.into_inline_table()))
                }
                other => other,
            };
            table.insert("track", inline);
        }
    }
    doc
}

/// The plain value of a document item, for comparing with what should be there.
fn plain(item: &toml_edit::Item) -> Option<toml::Value> {
    let text = match item {
        toml_edit::Item::Value(value) => format!("v = {value}"),
        _ => return None,
    };
    text.parse::<toml::Table>().ok()?.remove("v")
}

/// Equality that does not care whether a whole number was written `46` or `46.0`.
fn same_value(a: &toml::Value, b: &toml::Value) -> bool {
    use toml::Value as V;
    match (a, b) {
        (V::Integer(i), V::Float(f)) | (V::Float(f), V::Integer(i)) => *i as f64 == *f,
        (V::Array(x), V::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same_value(x, y))
        }
        (V::Table(x), V::Table(y)) => {
            x.len() == y.len()
                && x.iter().all(|(key, x)| y.get(key).is_some_and(|y| same_value(x, y)))
        }
        _ => a == b,
    }
}

/// Makes `doc` say what `wanted` says, touching only what differs.
fn sync_table(doc: &mut toml_edit::Table, wanted: &toml::Table) {
    let stale: Vec<String> = doc
        .iter()
        .map(|(key, _)| key.to_string())
        .filter(|key| !wanted.contains_key(key))
        .collect();
    for key in stale {
        doc.remove(&key);
    }
    for (key, value) in wanted {
        // Track rules stay inline (`track = { lang = [...] }`); settings groups are tables.
        let as_table = matches!(value, toml::Value::Table(_)) && key != "track";
        if let (true, toml::Value::Table(inner)) = (as_table, value) {
            let entry = doc.entry(key).or_insert_with(toml_edit::table);
            if !entry.is_table() {
                *entry = toml_edit::table();
            }
            if let Some(table) = entry.as_table_mut() {
                sync_table(table, inner);
            }
            continue;
        }
        if doc.get(key).and_then(plain).is_some_and(|existing| same_value(&existing, value)) {
            continue;
        }
        match doc.get_mut(key).and_then(toml_edit::Item::as_value_mut) {
            Some(existing) => {
                // Keep the comment that follows the value on its line.
                let decor = existing.decor().clone();
                *existing = to_edit(value);
                *existing.decor_mut() = decor;
            }
            None => {
                doc.insert(key, toml_edit::Item::Value(to_edit(value)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRIEREN: &str = r##"schema   = 1
id       = "frieren"                 # = filename stem
name     = "Frieren"
enabled  = true
priority = 0

[match]
type   = "series"
series = "sousou no frieren"

[audio]
track     = { lang = ["jpn", "ja"] }
delay_ms  = 0
equalizer = "dialogue"
normalize = false

[subtitles]
track    = { lang = ["eng", "en"], title_contains = "full", forced = false, prefer_external = true }
visible  = true
delay_ms = -350   # they run early

[subtitles.style]
font           = "Gandhi Sans"
size           = 46
outline_size   = 3.0
position       = 96
ass_override   = "scale"

[video]
brightness   = 0
contrast     = 4
saturation   = 6
aspect       = "auto"
tone_mapping = "auto"
deband       = true
shaders      = ["Anime4K_Clamp_Highlights.glsl"]

[playback]
speed = 1.0
"##;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vihpluh-presets-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_documented_example_parses() {
        let preset = parse(FRIEREN, "frieren").unwrap();
        assert_eq!(preset.name, "Frieren");
        assert_eq!(preset.matcher.kind, MatchType::Series);
        assert_eq!(preset.matcher.series.as_deref(), Some("sousou no frieren"));
        assert_eq!(preset.layer.subtitles.delay_ms, Some(-350));
        assert_eq!(preset.layer.subtitles.style.size, Some(46.0));
        assert_eq!(preset.layer.video.contrast, Some(4.0));
        assert_eq!(preset.layer.video.gamma, None, "absent keys stay absent");
        assert_eq!(
            preset.layer.video.shaders,
            Some(vec!["Anime4K_Clamp_Highlights.glsl".to_string()])
        );
        assert_eq!(preset.summary().sections, vec!["audio", "playback", "subtitles", "video"]);
    }

    #[test]
    fn sparse_presets_and_subtitles_off() {
        let text = "id = \"quiet\"\nname = \"Quiet\"\n[match]\ntype = \"folder\"\npath = \"D:/Films\"\n[subtitles]\ntrack = \"off\"\n";
        let preset = parse(text, "quiet").unwrap();
        assert!(preset.enabled);
        assert_eq!(preset.layer.subtitles.track, Some(SubTrackRule::Keyword("off".into())));
        assert!(preset.layer.audio.is_empty() && preset.layer.video.is_empty());
        assert_eq!(preset.layer.flat().len(), 1);
    }

    #[test]
    fn errors_name_the_line_or_the_field() {
        let broken = "id = \"x\"\nname = \"X\"\n[match]\ntype = \"series\"\nseries = \n";
        assert!(parse(broken, "x").unwrap_err().starts_with("line 5"));
        let typo = "id = \"x\"\nname = \"X\"\n[match]\ntype = \"series\"\nseries = \"s\"\n[video]\ncontrst = 4\n";
        let message = parse(typo, "x").unwrap_err();
        assert!(message.contains("line 7") && message.contains("contrst"), "{message}");
        let wrong_id = "id = \"other\"\nname = \"X\"\n[match]\ntype = \"glob\"\nglob = \"*\"\n";
        assert!(parse(wrong_id, "x").unwrap_err().contains("does not match the file name"));
        let no_field = "id = \"x\"\nname = \"X\"\n[match]\ntype = \"folder\"\n";
        assert!(parse(no_field, "x").unwrap_err().contains("needs `path`"));
        let range = "id = \"x\"\nname = \"X\"\n[match]\ntype = \"glob\"\nglob = \"*\"\n[video]\ncontrast = 400\n";
        assert!(parse(range, "x").unwrap_err().contains("video.contrast"));
        let keyword = "id = \"x\"\nname = \"X\"\n[match]\ntype = \"glob\"\nglob = \"*\"\n[subtitles]\ntrack = \"none\"\n";
        assert!(parse(keyword, "x").unwrap_err().contains("\"off\""));
    }

    #[test]
    fn layers_merge_key_by_key() {
        let mut base = parse(FRIEREN, "frieren").unwrap().layer;
        let over = Layer {
            video: VideoSection { contrast: Some(10.0), gamma: Some(2.0), ..Default::default() },
            subtitles: SubSection {
                style: StyleSection { size: Some(50.0), ..Default::default() },
                ..Default::default()
            },
            ..Default::default()
        };
        base.overlay(&over);
        assert_eq!(base.video.contrast, Some(10.0), "the later layer wins");
        assert_eq!(base.video.saturation, Some(6.0), "keys it does not set fall through");
        assert_eq!(base.video.gamma, Some(2.0));
        assert_eq!(base.subtitles.style.size, Some(50.0));
        assert_eq!(base.subtitles.style.font.as_deref(), Some("Gandhi Sans"));
        assert_eq!(base.subtitles.delay_ms, Some(-350));
    }

    #[test]
    fn flat_keys_round_trip_and_track_rules_stay_whole() {
        let layer = parse(FRIEREN, "frieren").unwrap().layer;
        let flat = layer.flat();
        assert_eq!(flat["subtitles.style.size"], serde_json::json!(46.0));
        assert!(flat["audio.track"].is_object(), "a rule is one value");
        assert!(!flat.contains_key("audio.track.lang"));
        assert_eq!(Layer::from_flat(&flat).unwrap(), layer);
        let part = layer.only(&["video.contrast".to_string(), "subtitles.delay_ms".to_string()]);
        assert_eq!(part.flat().len(), 2);
        let mut bad = Flat::new();
        bad.insert("video.nonsense".into(), serde_json::json!(1));
        assert!(Layer::from_flat(&bad).is_err());
    }

    #[test]
    fn saving_a_change_touches_only_that_line() {
        let dir = temp_dir("save");
        std::fs::write(dir.join("frieren.toml"), FRIEREN).unwrap();
        let store = PresetStore::open(dir.clone());
        let mut preset = store.get("frieren").unwrap();
        preset.layer.subtitles.delay_ms = Some(-500);
        preset.layer.video.gamma = Some(3.0);
        store.save(&preset).unwrap();
        let text = std::fs::read_to_string(dir.join("frieren.toml")).unwrap();
        let before: Vec<&str> = FRIEREN.lines().collect();
        let after: Vec<&str> = text.lines().collect();
        let changed: Vec<&&str> = after.iter().filter(|line| !before.contains(line)).collect();
        assert_eq!(changed, vec![&"delay_ms = -500   # they run early", &"gamma = 3.0"]);
        assert_eq!(after.len(), before.len() + 1);
        assert_eq!(store.get("frieren").unwrap(), preset);
    }

    #[test]
    fn header_defaults_a_file_left_out_are_not_added() {
        let dir = temp_dir("sparse");
        let text = "id = \"films\"
name = \"Films\"  # mine

[match]
type = \"folder\"
path = \"D:/Films\"

[video]
contrast = 10
";
        std::fs::write(dir.join("films.toml"), text).unwrap();
        let store = PresetStore::open(dir.clone());
        let mut preset = store.get("films").unwrap();
        preset.layer.video.contrast = Some(12.0);
        store.save(&preset).unwrap();
        let saved = std::fs::read_to_string(dir.join("films.toml")).unwrap();
        assert_eq!(saved, text.replace("contrast = 10", "contrast = 12.0"));
        // Once one of them stops being the default it is written.
        preset.enabled = false;
        store.save(&preset).unwrap();
        let saved = std::fs::read_to_string(dir.join("films.toml")).unwrap();
        assert!(saved.contains("enabled = false") && !saved.contains("priority"), "{saved}");
    }

    #[test]
    fn new_presets_get_a_file_and_a_free_id() {
        let dir = temp_dir("new");
        let store = PresetStore::open(dir.clone());
        assert_eq!(slug("Sousou no Frieren (BD)!"), "sousou-no-frieren-bd");
        assert_eq!(slug("日本語"), "preset");
        let mut preset = parse(FRIEREN, "frieren").unwrap();
        preset.id = store.free_id("Frieren");
        store.save(&preset).unwrap();
        assert_eq!(store.free_id("Frieren"), "frieren-2");
        assert_eq!(store.list().len(), 1);
        let text = std::fs::read_to_string(dir.join("frieren.toml")).unwrap();
        assert!(
            text.contains("[subtitles.style]") && text.contains("track = { lang = ["),
            "{text}"
        );
        // A new file follows the documented order: header keys, [match], then the sections.
        let order: Vec<usize> =
            ["schema =", "id =", "name =", "[match]", "[audio]", "[subtitles]", "[video]"]
                .iter()
                .map(|needle| text.find(needle).unwrap_or_else(|| panic!("no {needle}: {text}")))
                .collect();
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
        store.delete("frieren").unwrap();
        assert!(store.list().is_empty());
        assert!(store.delete("frieren").is_err());
        assert!(store.delete("../config").is_err());
    }

    #[test]
    fn a_bad_file_is_skipped_reported_once_and_the_rest_load() {
        let dir = temp_dir("bad");
        std::fs::write(dir.join("frieren.toml"), FRIEREN).unwrap();
        std::fs::write(dir.join("broken.toml"), "id = \"broken\"\nname = \n").unwrap();
        std::fs::write(dir.join("notes.txt"), "not a preset").unwrap();
        let store = PresetStore::open(dir.clone());
        assert_eq!(store.list().len(), 1);
        let problems = store.problems();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].starts_with("presets/broken.toml, line 2"), "{problems:?}");

        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        store.on_problem(move |message| sink.lock().unwrap().push(message.to_string()));
        assert_eq!(seen.lock().unwrap().len(), 1, "reported to a late listener");
        store.reload();
        assert_eq!(seen.lock().unwrap().len(), 1, "not again while it stays broken");
        std::fs::write(dir.join("broken.toml"), "id = \"broken\"\nname = 5\n").unwrap();
        store.reload();
        assert_eq!(seen.lock().unwrap().len(), 2, "a different problem is new");
    }

    #[test]
    fn ids() {
        assert!(is_valid_id("frieren-s2"));
        for bad in ["", "Frieren", "a b", "-a", "a-", "a/b", "a.b"] {
            assert!(!is_valid_id(bad), "{bad}");
        }
    }
}
