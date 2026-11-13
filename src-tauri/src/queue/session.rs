//! `session.json` (ARCHITECTURE §9.4): the queue, position, window mode, and open panels, so a
//! relaunch picks up where the last run stopped — paused, at the same spot.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Origin, QueueItem, Repeat};
use crate::window::WindowMode;

pub const SCHEMA: u32 = 1;

/// Which panels were open (frontend view state, persisted here rather than in config).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct UiState {
    pub sidebar_open: bool,
    /// `"subtitles" | "audio" | "video" | "playback" | "bookmarks"`
    pub sidebar_tab: String,
    pub queue_open: bool,
    pub library_open: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            sidebar_open: false,
            sidebar_tab: "subtitles".to_string(),
            queue_open: false,
            library_open: false,
        }
    }
}

/// A partial `UiState` from `session_ui_set`.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UiPatch {
    #[ts(optional)]
    pub sidebar_open: Option<bool>,
    #[ts(optional)]
    pub sidebar_tab: Option<String>,
    #[ts(optional)]
    pub queue_open: Option<bool>,
    #[ts(optional)]
    pub library_open: Option<bool>,
}

impl UiState {
    pub fn apply(&mut self, patch: UiPatch) -> bool {
        let before = self.clone();
        if let Some(v) = patch.sidebar_open {
            self.sidebar_open = v;
        }
        if let Some(v) = patch.sidebar_tab {
            self.sidebar_tab = v;
        }
        if let Some(v) = patch.queue_open {
            self.queue_open = v;
        }
        if let Some(v) = patch.library_open {
            self.library_open = v;
        }
        *self != before
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedItem {
    pub id: String,
    pub source: String,
    pub title: String,
    pub origin: Origin,
    pub origin_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedQueue {
    pub items: Vec<SavedItem>,
    pub current_id: Option<String>,
    pub repeat: Repeat,
    pub shuffle: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub schema: u32,
    pub saved_at: String,
    pub queue: SavedQueue,
    pub position: f64,
    pub paused: bool,
    pub window_mode: WindowMode,
    #[serde(default)]
    pub ui: UiState,
}

impl SavedItem {
    pub fn from_item(item: &QueueItem) -> Self {
        Self {
            id: item.id.clone(),
            source: item.source.clone(),
            title: item.title.clone(),
            origin: item.origin,
            origin_ref: item.origin_ref.clone(),
        }
    }

    pub fn into_item(self) -> QueueItem {
        QueueItem {
            is_url: crate::cli::is_url(&self.source),
            id: self.id,
            source: self.source,
            title: self.title,
            duration: None,
            media_key: None,
            origin: self.origin,
            origin_ref: self.origin_ref,
            error: None,
        }
    }
}

/// Reads a session file. A missing, unreadable, or foreign-schema file is simply no session.
pub fn load(path: &Path) -> Option<Session> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<Session>(&text) {
        Ok(session) if session.schema == SCHEMA => Some(session),
        Ok(session) => {
            tracing::warn!("session.json has schema {}; starting fresh", session.schema);
            None
        }
        Err(err) => {
            tracing::warn!("session.json could not be read ({err}); starting fresh");
            None
        }
    }
}

/// Writes atomically (temporary file + rename).
pub fn save(path: &Path, session: &Session) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(session).map_err(std::io::Error::other)?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, path)
}

/// Seconds since the Unix epoch as an RFC 3339 UTC timestamp.
pub fn rfc3339(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

pub fn now_rfc3339() -> String {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    rfc3339(unix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_794_510_251), "2026-11-12T19:04:11Z");
    }

    #[test]
    fn round_trips_through_disk_and_matches_the_documented_shape() {
        let dir = std::env::temp_dir().join(format!("vihpluh-session-{}", std::process::id()));
        let path = dir.join("session.json");
        let session = Session {
            schema: SCHEMA,
            saved_at: rfc3339(1_794_510_251),
            queue: SavedQueue {
                items: vec![SavedItem {
                    id: "0b6f".into(),
                    source: "D:/Shows/Severance/S02E03.mkv".into(),
                    title: "Severance S02E03".into(),
                    origin: Origin::Folder,
                    origin_ref: None,
                }],
                current_id: Some("0b6f".into()),
                repeat: Repeat::Off,
                shuffle: false,
            },
            position: 1284.5,
            paused: true,
            window_mode: WindowMode::Normal,
            ui: UiState::default(),
        };
        save(&path, &session).unwrap();
        assert_eq!(load(&path), Some(session));
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(json["queue"]["currentId"], "0b6f");
        assert_eq!(json["queue"]["items"][0]["origin"], "folder");
        assert_eq!(json["windowMode"], "normal");
        assert_eq!(json["ui"]["sidebarTab"], "subtitles");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn broken_or_foreign_files_are_no_session() {
        let dir = std::env::temp_dir().join(format!("vihpluh-session-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path), None);
        std::fs::write(&path, r#"{"schema": 99}"#).unwrap();
        assert_eq!(load(&path), None);
        assert_eq!(load(&dir.join("missing.json")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ui_patches_report_whether_anything_changed() {
        let mut ui = UiState::default();
        assert!(ui.apply(UiPatch { queue_open: Some(true), ..UiPatch::default() }));
        assert!(!ui.apply(UiPatch { queue_open: Some(true), ..UiPatch::default() }));
        assert!(ui.queue_open);
    }
}
