//! Playback memory (ARCHITECTURE §6.5, §9.6 `history`): resume positions, finished marks,
//! play counts, and per-file track/delay memory. Keyed by `media_key`, independent of the
//! library, so files never added to it are remembered too.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::db::unix_now;

/// A remembered track choice. Stored as JSON in `history.audio_track` / `sub_track`. Rules,
/// not indices: language and title are matched first, the id only as a last resort, because
/// track order differs between files (§10.3; the full rule language arrives in Phase 8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackMemory {
    /// The track was switched off (subtitles).
    #[serde(default)]
    pub off: bool,
    pub lang: Option<String>,
    pub title: Option<String>,
    pub id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    pub media_key: String,
    pub source: String,
    pub title: String,
    pub duration: Option<f64>,
    pub position: f64,
    pub finished: bool,
    pub play_count: i64,
    pub last_played_at: i64,
    pub audio_track: Option<TrackMemory>,
    pub sub_track: Option<TrackMemory>,
    pub sub_delay_ms: Option<i32>,
    pub audio_delay_ms: Option<i32>,
}

/// A row for the idle screen's "Continue watching".
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecentEntry {
    pub media_key: String,
    pub source: String,
    pub title: String,
    pub duration: Option<f64>,
    pub position: f64,
    pub finished: bool,
    #[ts(type = "number")]
    pub last_played_at: i64,
}

fn parse_track(text: Option<String>) -> Option<TrackMemory> {
    text.and_then(|t| serde_json::from_str(&t).ok())
}

pub fn get(conn: &Connection, media_key: &str) -> rusqlite::Result<Option<HistoryEntry>> {
    conn.query_row(
        "SELECT media_key, source, title, duration, position, finished, play_count, last_played_at,
                audio_track, sub_track, sub_delay_ms, audio_delay_ms
           FROM history WHERE media_key = ?1",
        [media_key],
        |r| {
            Ok(HistoryEntry {
                media_key: r.get(0)?,
                source: r.get(1)?,
                title: r.get(2)?,
                duration: r.get(3)?,
                position: r.get(4)?,
                finished: r.get::<_, i64>(5)? != 0,
                play_count: r.get(6)?,
                last_played_at: r.get(7)?,
                audio_track: parse_track(r.get(8)?),
                sub_track: parse_track(r.get(9)?),
                sub_delay_ms: r.get(10)?,
                audio_delay_ms: r.get(11)?,
            })
        },
    )
    .optional()
}

/// A file was opened: create or refresh its row and count the play.
pub fn record_open(
    conn: &Connection,
    media_key: &str,
    source: &str,
    title: &str,
    duration: Option<f64>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO history (media_key, source, title, duration, play_count, last_played_at)
              VALUES (?1, ?2, ?3, ?4, 1, ?5)
         ON CONFLICT(media_key) DO UPDATE SET
              source = excluded.source,
              title = excluded.title,
              duration = COALESCE(excluded.duration, history.duration),
              play_count = history.play_count + 1,
              last_played_at = excluded.last_played_at",
        params![media_key, source, title, duration, unix_now()],
    )?;
    Ok(())
}

/// Saves the resume position. At or past `finished_percent` the file is marked finished and
/// its resume position cleared (§6.5).
pub fn save_position(
    conn: &Connection,
    media_key: &str,
    position: f64,
    duration: Option<f64>,
    finished_percent: u32,
) -> rusqlite::Result<()> {
    let finished = is_finished(position, duration, finished_percent);
    conn.execute(
        "UPDATE history SET position = ?2, finished = MAX(finished, ?3), last_played_at = ?4,
                duration = COALESCE(?5, duration)
          WHERE media_key = ?1",
        params![
            media_key,
            if finished { 0.0 } else { position },
            finished as i64,
            unix_now(),
            duration
        ],
    )?;
    Ok(())
}

/// The file played to its end.
pub fn mark_finished(conn: &Connection, media_key: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE history SET position = 0, finished = 1, last_played_at = ?2 WHERE media_key = ?1",
        params![media_key, unix_now()],
    )?;
    Ok(())
}

pub fn is_finished(position: f64, duration: Option<f64>, finished_percent: u32) -> bool {
    match duration {
        Some(d) if d > 0.0 => position >= d * f64::from(finished_percent) / 100.0,
        _ => false,
    }
}

/// Remembers track choices and sync for this file (`playback.remember_per_file`).
pub fn save_memory(
    conn: &Connection,
    media_key: &str,
    audio: Option<&TrackMemory>,
    sub: Option<&TrackMemory>,
    sub_delay_ms: i32,
    audio_delay_ms: i32,
) -> rusqlite::Result<()> {
    let json = |t: Option<&TrackMemory>| t.and_then(|t| serde_json::to_string(t).ok());
    conn.execute(
        "UPDATE history SET audio_track = ?2, sub_track = ?3, sub_delay_ms = ?4, audio_delay_ms = ?5
          WHERE media_key = ?1",
        params![media_key, json(audio), json(sub), sub_delay_ms, audio_delay_ms],
    )?;
    Ok(())
}

/// Most recently played first.
pub fn recent(conn: &Connection, limit: u32) -> rusqlite::Result<Vec<RecentEntry>> {
    let mut stmt = conn.prepare(
        "SELECT media_key, source, title, duration, position, finished, last_played_at
           FROM history ORDER BY last_played_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |r| {
        Ok(RecentEntry {
            media_key: r.get(0)?,
            source: r.get(1)?,
            title: r.get(2)?,
            duration: r.get(3)?,
            position: r.get(4)?,
            finished: r.get::<_, i64>(5)? != 0,
            last_played_at: r.get(6)?,
        })
    })?;
    rows.collect()
}

/// Forgets one file, or everything.
pub fn clear(conn: &Connection, media_key: Option<&str>) -> rusqlite::Result<()> {
    match media_key {
        Some(key) => conn.execute("DELETE FROM history WHERE media_key = ?1", [key])?,
        None => conn.execute("DELETE FROM history", [])?,
    };
    Ok(())
}

/// Where to start a file, given its history (§6.5). `None` = from the beginning.
pub fn resume_point(
    entry: &HistoryEntry,
    duration: Option<f64>,
    finished_percent: u32,
) -> Option<f64> {
    if entry.finished || entry.position <= 60.0 {
        return None;
    }
    let duration = duration.or(entry.duration);
    if is_finished(entry.position, duration, finished_percent) {
        return None;
    }
    Some((entry.position - 3.0).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::super::db::Db;
    use super::*;

    fn db() -> Db {
        Db::in_memory().unwrap()
    }

    #[test]
    fn open_save_and_read_back() {
        let db = db();
        db.call(|c| {
            record_open(c, "f:1", "D:/a.mkv", "a.mkv", Some(1420.0))?;
            save_position(c, "f:1", 754.5, Some(1420.0), 95)?;
            record_open(c, "f:1", "E:/moved/a.mkv", "a.mkv", None)
        })
        .unwrap();
        let entry = db.call(|c| get(c, "f:1")).unwrap().unwrap();
        assert_eq!(entry.source, "E:/moved/a.mkv", "the last known path wins");
        assert_eq!(entry.position, 754.5);
        assert_eq!(entry.duration, Some(1420.0), "kept when the new open doesn't know it");
        assert_eq!(entry.play_count, 2);
        assert!(!entry.finished);
        assert!(db.call(|c| get(c, "f:missing")).unwrap().is_none());
    }

    #[test]
    fn finishing_clears_the_resume_point() {
        let db = db();
        db.call(|c| {
            record_open(c, "f:1", "a", "a", Some(1000.0))?;
            save_position(c, "f:1", 960.0, Some(1000.0), 95)
        })
        .unwrap();
        let entry = db.call(|c| get(c, "f:1")).unwrap().unwrap();
        assert!(entry.finished);
        assert_eq!(entry.position, 0.0);
        // Rewatching later keeps the finished mark but resumes from the new spot.
        db.call(|c| save_position(c, "f:1", 300.0, Some(1000.0), 95)).unwrap();
        let entry = db.call(|c| get(c, "f:1")).unwrap().unwrap();
        assert!(entry.finished);
        assert_eq!(entry.position, 300.0);
    }

    #[test]
    fn resume_rules() {
        let entry = |position: f64, finished: bool| HistoryEntry {
            media_key: "k".into(),
            source: "s".into(),
            title: "t".into(),
            duration: Some(1000.0),
            position,
            finished,
            play_count: 1,
            last_played_at: 0,
            audio_track: None,
            sub_track: None,
            sub_delay_ms: None,
            audio_delay_ms: None,
        };
        assert_eq!(resume_point(&entry(754.0, false), None, 95), Some(751.0), "3 s early");
        assert_eq!(resume_point(&entry(60.0, false), None, 95), None, "too early to matter");
        assert_eq!(resume_point(&entry(960.0, false), None, 95), None, "past finished");
        assert_eq!(resume_point(&entry(500.0, true), None, 95), None, "finished");
        assert_eq!(resume_point(&entry(900.0, false), Some(1000.0), 80), None, "custom threshold");
    }

    #[test]
    fn per_file_memory_round_trips() {
        let db = db();
        let audio = TrackMemory {
            off: false,
            lang: Some("jpn".into()),
            title: Some("Stereo".into()),
            id: Some(2),
        };
        let sub = TrackMemory { off: true, lang: None, title: None, id: None };
        let (a, s) = (audio.clone(), sub.clone());
        db.call(move |c| {
            record_open(c, "f:1", "a", "a", None)?;
            save_memory(c, "f:1", Some(&a), Some(&s), -350, 100)
        })
        .unwrap();
        let entry = db.call(|c| get(c, "f:1")).unwrap().unwrap();
        assert_eq!(entry.audio_track, Some(audio));
        assert_eq!(entry.sub_track, Some(sub));
        assert_eq!(entry.sub_delay_ms, Some(-350));
        assert_eq!(entry.audio_delay_ms, Some(100));
    }

    #[test]
    fn recent_and_clear() {
        let db = db();
        db.call(|c| {
            record_open(c, "f:1", "a", "a", None)?;
            c.execute("UPDATE history SET last_played_at = 10 WHERE media_key = 'f:1'", [])?;
            record_open(c, "f:2", "b", "b", None)?;
            Ok(())
        })
        .unwrap();
        let rows = db.call(|c| recent(c, 10)).unwrap();
        assert_eq!(
            rows.iter().map(|r| r.media_key.as_str()).collect::<Vec<_>>(),
            vec!["f:2", "f:1"]
        );
        db.call(|c| clear(c, Some("f:2"))).unwrap();
        assert_eq!(db.call(|c| recent(c, 10)).unwrap().len(), 1);
        db.call(|c| clear(c, None)).unwrap();
        assert!(db.call(|c| recent(c, 10)).unwrap().is_empty());
    }
}
