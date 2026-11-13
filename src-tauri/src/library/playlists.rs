//! Playlists (ARCHITECTURE §8, §9.5): named, saved, ordered lists — one JSON file each in
//! `<config>/playlists/`. Also plain M3U8 import and export, which keep `source` and `title`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::presets::{is_valid_id, slug};

pub const SCHEMA_VERSION: u32 = 1;

fn schema_version() -> u32 {
    SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Playlist {
    #[serde(default = "schema_version")]
    pub schema: u32,
    /// The file name stem: a slug of the name.
    pub id: String,
    pub name: String,
    /// RFC 3339.
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    pub items: Vec<PlaylistItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlaylistItem {
    /// Path or URL.
    pub source: String,
    pub title: String,
    /// Lets a moved file be found again through the library.
    #[serde(default)]
    pub media_key: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlaylistSummary {
    pub id: String,
    pub name: String,
    #[ts(type = "number")]
    pub count: usize,
    /// Sum of the known durations, seconds.
    pub duration: f64,
    pub updated_at: String,
}

impl Playlist {
    pub fn summary(&self) -> PlaylistSummary {
        PlaylistSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            count: self.items.len(),
            duration: self.items.iter().filter_map(|i| i.duration).sum(),
            updated_at: self.updated_at.clone(),
        }
    }
}

pub struct PlaylistStore {
    dir: PathBuf,
}

impl PlaylistStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn file(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Every playlist that loads, by name. A file that does not parse is skipped and logged.
    pub fn list(&self) -> Vec<Playlist> {
        let mut playlists: Vec<Playlist> = std::fs::read_dir(&self.dir)
            .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("json"))
            .filter_map(|path| {
                let stem = path.file_stem()?.to_str()?.to_string();
                match self.get(&stem) {
                    Ok(playlist) => Some(playlist),
                    Err(err) => {
                        tracing::warn!("playlist {stem} skipped: {}", err.message);
                        None
                    }
                }
            })
            .collect();
        playlists.sort_by_key(|playlist| playlist.name.to_lowercase());
        playlists
    }

    pub fn get(&self, id: &str) -> AppResult<Playlist> {
        if !is_valid_id(id) {
            return Err(AppError::invalid("not a playlist id"));
        }
        let text = match std::fs::read_to_string(self.file(id)) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(AppError::not_found(format!("no playlist named `{id}`")));
            }
            Err(err) => return Err(err.into()),
        };
        let mut playlist: Playlist = serde_json::from_str(&text)
            .map_err(|e| AppError::invalid(format!("playlists/{id}.json: {e}")))?;
        playlist.id = id.to_string();
        Ok(playlist)
    }

    /// An id for a new playlist called `name`: its slug, with `-2`, `-3`… on collision.
    pub fn free_id(&self, name: &str) -> String {
        let base = match slug(name).as_str() {
            "preset" => "playlist".to_string(),
            other => other.to_string(),
        };
        if !self.file(&base).exists() {
            return base;
        }
        (2..).map(|n| format!("{base}-{n}")).find(|id| !self.file(id).exists()).unwrap_or(base)
    }

    /// Writes a playlist. An empty id means "new": one is made from the name. Returns the id.
    pub fn save(&self, mut playlist: Playlist) -> AppResult<String> {
        if playlist.name.trim().is_empty() {
            return Err(AppError::invalid("the playlist needs a name"));
        }
        playlist.name = playlist.name.trim().to_string();
        if playlist.id.is_empty() {
            playlist.id = self.free_id(&playlist.name);
        } else if !is_valid_id(&playlist.id) {
            return Err(AppError::invalid("not a playlist id"));
        }
        let now = crate::queue::session::now_rfc3339();
        if playlist.created_at.is_empty() {
            playlist.created_at = self
                .get(&playlist.id)
                .map(|existing| existing.created_at)
                .ok()
                .filter(|at| !at.is_empty())
                .unwrap_or_else(|| now.clone());
        }
        playlist.updated_at = now;
        playlist.schema = SCHEMA_VERSION;
        std::fs::create_dir_all(&self.dir)?;
        let json = serde_json::to_vec_pretty(&playlist)
            .map_err(|e| AppError::internal(format!("playlist could not be written: {e}")))?;
        crate::media::thumbs::write_atomic(&self.file(&playlist.id), &json)?;
        Ok(playlist.id)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        if !is_valid_id(id) {
            return Err(AppError::invalid("not a playlist id"));
        }
        match std::fs::remove_file(self.file(id)) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(AppError::not_found(format!("no playlist named `{id}`")))
            }
            Err(err) => Err(err.into()),
        }
    }
}

/// Points items whose file has moved at its new place, found by media key. `locate` answers
/// "where is the file with this key now?" (the library index). Returns how many moved.
pub fn relocate(playlist: &mut Playlist, locate: impl Fn(&str) -> Option<String>) -> usize {
    let mut moved = 0;
    for item in &mut playlist.items {
        if crate::cli::is_url(&item.source) || Path::new(&item.source).is_file() {
            continue;
        }
        let found = item.media_key.as_deref().and_then(&locate);
        if let Some(path) = found.filter(|path| Path::new(path).is_file()) {
            item.source = path;
            moved += 1;
        }
    }
    moved
}

// ── M3U8 ────────────────────────────────────────────────────────────────────

/// The entries of an M3U / M3U8 playlist: `(source, title, duration)`. Relative paths are
/// resolved against `base` (the folder the file is in).
pub fn parse_m3u(text: &str, base: &Path) -> Vec<PlaylistItem> {
    let mut items = Vec::new();
    let mut pending: Option<(Option<f64>, String)> = None;
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            continue;
        }
        if let Some(info) = line.strip_prefix("#EXTINF:") {
            let (head, title) = info.split_once(',').unwrap_or((info, ""));
            let duration = head
                .split_whitespace()
                .next()
                .and_then(|d| d.parse::<f64>().ok())
                .filter(|d| *d > 0.0);
            pending = Some((duration, title.trim().to_string()));
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let source = if crate::cli::is_url(line) || Path::new(line).is_absolute() {
            line.to_string()
        } else {
            base.join(line).display().to_string()
        };
        let (duration, title) = pending.take().unwrap_or((None, String::new()));
        let title = if title.is_empty() { crate::queue::title_for(&source) } else { title };
        items.push(PlaylistItem { source, title, media_key: None, duration });
    }
    items
}

pub fn to_m3u(playlist: &Playlist) -> String {
    let mut out = String::from("#EXTM3U\n");
    for item in &playlist.items {
        // A title must stay on its line.
        let title = item.title.replace(['\r', '\n'], " ");
        let seconds = item.duration.map_or(-1, |d| d.round() as i64);
        out.push_str(&format!("#EXTINF:{seconds},{title}\n{}\n", item.source));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vihpluh-pl-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn item(source: &str, title: &str, key: Option<&str>, duration: Option<f64>) -> PlaylistItem {
        PlaylistItem {
            source: source.into(),
            title: title.into(),
            media_key: key.map(str::to_string),
            duration,
        }
    }

    fn playlist(name: &str, items: Vec<PlaylistItem>) -> Playlist {
        Playlist {
            schema: 1,
            id: String::new(),
            name: name.into(),
            created_at: String::new(),
            updated_at: String::new(),
            items,
        }
    }

    #[test]
    fn save_list_get_delete_and_slug_collisions() {
        let store = PlaylistStore::new(temp_dir("store"));
        let items = vec![
            item("E:/Music/01.flac", "My Foolish Heart", Some("f:8e24"), Some(298.2)),
            item("https://example.com/stream.m3u8", "Radio", Some("u:5f2a"), None),
        ];
        let id = store.save(playlist("Late Night Jazz", items.clone())).unwrap();
        assert_eq!(id, "late-night-jazz");
        let second = store.save(playlist("Late night jazz!", vec![])).unwrap();
        assert_eq!(second, "late-night-jazz-2");
        assert_eq!(
            store.save(playlist("  ", vec![])).unwrap_err().message,
            "the playlist needs a name"
        );

        let loaded = store.get(&id).unwrap();
        assert_eq!(loaded.items, items);
        assert!(!loaded.created_at.is_empty());
        let summary = loaded.summary();
        assert_eq!((summary.count, summary.duration), (2, 298.2));

        // Saving again keeps the creation time.
        let mut renamed = loaded.clone();
        renamed.name = "Jazz".into();
        renamed.created_at.clear();
        store.save(renamed).unwrap();
        assert_eq!(store.get(&id).unwrap().created_at, loaded.created_at);
        assert_eq!(
            store.list().iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            vec!["Jazz", "Late night jazz!"]
        );

        store.delete(&id).unwrap();
        assert!(store.get(&id).is_err());
        assert!(store.delete("../config").is_err());
    }

    #[test]
    fn a_broken_file_is_skipped() {
        let dir = temp_dir("broken");
        std::fs::write(dir.join("bad.json"), "{ not json").unwrap();
        let store = PlaylistStore::new(dir);
        store.save(playlist("Good", vec![])).unwrap();
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn moved_files_are_found_again_by_media_key() {
        let dir = temp_dir("relocate");
        let here = dir.join("here.flac");
        let moved_to = dir.join("moved.flac");
        std::fs::write(&here, b"x").unwrap();
        std::fs::write(&moved_to, b"x").unwrap();
        let mut list = playlist(
            "p",
            vec![
                item(&here.display().to_string(), "stays", Some("f:1"), None),
                item("Z:/old/place.flac", "moved", Some("f:2"), None),
                item("Z:/old/gone.flac", "gone", Some("f:3"), None),
                item("Z:/old/nokey.flac", "no key", None, None),
                item("https://example.com/a", "url", Some("u:1"), None),
            ],
        );
        let target = moved_to.display().to_string();
        let moved = relocate(&mut list, |key| match key {
            "f:2" => Some(target.clone()),
            "f:3" => Some("Z:/also/missing.flac".into()),
            _ => None,
        });
        assert_eq!(moved, 1);
        assert_eq!(list.items[1].source, target);
        assert_eq!(list.items[2].source, "Z:/old/gone.flac", "an unusable answer changes nothing");
    }

    #[test]
    fn m3u_round_trip_keeps_source_and_title() {
        let list = playlist(
            "p",
            vec![
                item("E:/Music/01 My Foolish Heart.flac", "My Foolish Heart", None, Some(298.0)),
                item("https://example.com/stream.m3u8", "Radio, live", None, None),
            ],
        );
        let text = to_m3u(&list);
        assert!(text.starts_with("#EXTM3U\n#EXTINF:298,My Foolish Heart\n"));
        let back = parse_m3u(&text, Path::new("E:/Music"));
        let pairs = |items: &[PlaylistItem]| {
            items
                .iter()
                .map(|i| (i.source.clone(), i.title.clone(), i.duration))
                .collect::<Vec<_>>()
        };
        assert_eq!(pairs(&back), pairs(&list.items));
    }

    #[test]
    fn m3u_without_extinf_relative_paths_and_noise() {
        let text = "\u{feff}#EXTM3U\r\n\r\n# a comment\r\nsub/a.mp3\r\n#EXTINF:-1,Named\r\nD:/abs/b.mp3\r\nhttp://h/c.mp3\r\n#EXTINF:broken\r\n";
        let items = parse_m3u(text, Path::new("D:/lists"));
        assert_eq!(items.len(), 3);
        assert!(items[0].source.replace('\\', "/").ends_with("D:/lists/sub/a.mp3"));
        assert_eq!(items[0].title, "a.mp3");
        assert_eq!((items[1].title.as_str(), items[1].duration), ("Named", None));
        assert_eq!(items[2].source, "http://h/c.mp3");
    }
}
