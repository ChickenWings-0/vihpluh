//! The library scanner (ARCHITECTURE §8.2): walks a folder, works out what is new, changed,
//! or gone since the last scan by `(size, mtime)`, and brings the `media` rows in line.
//! Metadata comes from the file name for video and from tags for audio. Media keys and
//! video durations are not part of a scan: they are filled in afterwards, lazily.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde::Serialize;
use ts_rs::TS;

use super::db::{Db, DbError};
use super::index::{self, FolderKind, LibraryFolder, MediaRow};
use super::music;
use crate::media::files::{AUDIO_EXTENSIONS, VIDEO_EXTENSIONS};
use crate::presets::matcher::glob_matches;
use crate::presets::series;

/// `library://scan` — progress of a folder scan.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScanProgress {
    #[ts(type = "number")]
    pub folder_id: i64,
    /// Files handled so far.
    #[ts(type = "number")]
    pub scanned: u64,
    /// Files found in all; `None` while the folder is still being walked.
    #[ts(type = "number | null")]
    pub total: Option<u64>,
    pub done: bool,
}

/// The settings a scan follows (`library.*`).
#[derive(Debug, Clone, Default)]
pub struct ScanSettings {
    pub extra_extensions: Vec<String>,
    pub ignore_patterns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanSummary {
    pub found: usize,
    pub added_or_changed: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub path: PathBuf,
    pub size: i64,
    pub mtime: i64,
    pub audio: bool,
}

/// Progress is reported at most this often (§7.4: 4 Hz).
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
/// Rows are written in transactions of this many.
const BATCH: usize = 500;

fn forward(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn ignored(path: &Path, settings: &ScanSettings) -> bool {
    let text = forward(path);
    settings.ignore_patterns.iter().any(|pattern| glob_matches(pattern, &text))
}

/// Whether a file belongs in a folder of this kind, and if so whether it is audio.
fn classify(path: &Path, kind: FolderKind, settings: &ScanSettings) -> Option<bool> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let audio = AUDIO_EXTENSIONS.contains(&ext.as_str());
    let video = VIDEO_EXTENSIONS.contains(&ext.as_str())
        || settings
            .extra_extensions
            .iter()
            .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(&ext));
    match kind {
        FolderKind::Video => video.then_some(false),
        FolderKind::Music => audio.then_some(true),
        FolderKind::Mixed => (audio || video).then_some(audio),
    }
}

/// Every media file under `root`, skipping what the ignore patterns name. Unreadable folders
/// are passed over. `on_file` is called for each file found (for progress).
pub fn walk(
    root: &Path,
    kind: FolderKind,
    settings: &ScanSettings,
    cancel: &AtomicBool,
    mut on_file: impl FnMut(usize),
) -> Vec<Found> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if ignored(&path, settings) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else { continue };
            if file_type.is_dir() {
                pending.push(path);
                continue;
            }
            let Some(audio) = classify(&path, kind, settings) else { continue };
            let Ok(meta) = entry.metadata() else { continue };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64);
            found.push(Found { path, size: meta.len() as i64, mtime, audio });
            on_file(found.len());
        }
    }
    found
}

/// The row for a file: tags for audio (falling back to the name), the name for video.
pub fn describe(file: &Found) -> MediaRow {
    let path = file.path.display().to_string();
    let stem = file.path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
    let mut row = MediaRow {
        path,
        kind: if file.audio { "audio" } else { "video" },
        size: file.size,
        mtime: file.mtime,
        title: stem,
        ..MediaRow::default()
    };
    if file.audio {
        if let Some(tags) = music::read(&file.path) {
            if let Some(title) = tags.title {
                row.title = title;
            }
            row.artist = tags.artist;
            row.album_artist = tags.album_artist;
            row.album = tags.album;
            row.track_no = tags.track_no;
            row.disc_no = tags.disc_no;
            row.year = tags.year;
            row.duration = tags.duration;
        }
    } else {
        let parsed = series::parse(&row.path);
        if parsed.episode.is_some() && !parsed.key.is_empty() {
            row.series_key = Some(parsed.key);
            row.season = parsed.season;
            row.episode = parsed.episode;
        }
    }
    row
}

/// Scans one folder and updates its rows. `progress` hears about it at most four times a
/// second, and once more at the end.
pub fn scan_folder(
    db: &Db,
    folder: &LibraryFolder,
    settings: &ScanSettings,
    cancel: &AtomicBool,
    mut progress: impl FnMut(ScanProgress),
) -> Result<ScanSummary, DbError> {
    let id = folder.id;
    let mut last = Instant::now();
    let mut tick = |scanned: u64, total: Option<u64>, progress: &mut dyn FnMut(ScanProgress)| {
        if last.elapsed() >= PROGRESS_EVERY {
            last = Instant::now();
            progress(ScanProgress { folder_id: id, scanned, total, done: false });
        }
    };
    progress(ScanProgress { folder_id: id, scanned: 0, total: None, done: false });
    let found = walk(Path::new(&folder.path), folder.kind, settings, cancel, |n| {
        tick(n as u64, None, &mut progress);
    });
    if cancel.load(Ordering::Relaxed) {
        return Ok(ScanSummary::default());
    }

    let known: HashMap<String, (i64, i64)> = db
        .call(move |c| index::known_files(c, id))?
        .into_iter()
        .map(|(path, size, mtime)| (path, (size, mtime)))
        .collect();
    let mut seen = std::collections::HashSet::with_capacity(found.len());
    let total = found.len() as u64;
    let mut changed = 0;
    let mut batch: Vec<MediaRow> = Vec::with_capacity(BATCH);
    let flush = |batch: &mut Vec<MediaRow>| -> Result<(), DbError> {
        if batch.is_empty() {
            return Ok(());
        }
        let rows = std::mem::take(batch);
        db.call(move |c| index::upsert(c, id, &rows))
    };
    for (n, file) in found.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let path = file.path.display().to_string();
        let unchanged = known.get(&path) == Some(&(file.size, file.mtime));
        seen.insert(path);
        if !unchanged {
            batch.push(describe(file));
            changed += 1;
            if batch.len() >= BATCH {
                flush(&mut batch)?;
            }
        }
        tick(n as u64 + 1, Some(total), &mut progress);
    }
    flush(&mut batch)?;

    let gone: Vec<String> = known.into_keys().filter(|path| !seen.contains(path)).collect();
    let removed = gone.len();
    if !gone.is_empty() && !cancel.load(Ordering::Relaxed) {
        db.call(move |c| index::remove_paths(c, &gone))?;
    }
    progress(ScanProgress { folder_id: id, scanned: total, total: Some(total), done: true });
    Ok(ScanSummary { found: found.len(), added_or_changed: changed, removed })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::index::{LibraryItem, LibraryQuery, LibraryView, MediaKind};

    fn temp_tree(name: &str, files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vihpluh-scan-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for file in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"x").unwrap();
        }
        dir
    }

    fn settings() -> ScanSettings {
        ScanSettings {
            extra_extensions: vec![".rmvb".into()],
            ignore_patterns: vec!["**/.*".into(), "**/*sample*".into()],
        }
    }

    fn folder(db: &Db, dir: &Path, kind: FolderKind) -> LibraryFolder {
        let path = dir.display().to_string();
        db.call(move |c| {
            let id = index::add_folder(c, &path, kind, false, true)?;
            Ok(index::folder(c, id)?.unwrap())
        })
        .unwrap()
    }

    fn titles(db: &Db, kind: MediaKind) -> Vec<String> {
        db.call(move |c| {
            index::query(
                c,
                &LibraryQuery {
                    kind,
                    view: LibraryView::All,
                    parent: None,
                    search: None,
                    limit: 1000,
                    offset: 0,
                },
            )
        })
        .unwrap()
        .items
        .into_iter()
        .filter_map(|item| match item {
            LibraryItem::Media(m) => Some(m.title),
            LibraryItem::Group(_) => None,
        })
        .collect()
    }

    const TREE: &[&str] = &[
        "Show/Show S01E01.mkv",
        "Show/Show S01E02.mkv",
        "Show/Subs/Show S01E01.srt",
        "Show/sample-clip.mkv",
        "Films/Alien.mp4",
        "Films/Old.rmvb",
        ".hidden/Secret.mkv",
        "Music/01 Track.flac",
        "notes.txt",
    ];

    #[test]
    fn a_scan_indexes_media_and_skips_what_is_ignored() {
        let dir = temp_tree("index", TREE);
        let db = Db::in_memory().unwrap();
        let f = folder(&db, &dir, FolderKind::Mixed);
        let mut events = Vec::new();
        let summary =
            scan_folder(&db, &f, &settings(), &AtomicBool::new(false), |p| events.push(p)).unwrap();
        assert_eq!(summary, ScanSummary { found: 5, added_or_changed: 5, removed: 0 });
        assert_eq!(
            titles(&db, MediaKind::Video),
            vec!["Alien", "Old", "Show S01E01", "Show S01E02"]
        );
        // The fake FLAC has no readable tags: it is listed under its file name.
        assert_eq!(titles(&db, MediaKind::Audio), vec!["01 Track"]);
        assert_eq!(events.first().map(|p| (p.scanned, p.total, p.done)), Some((0, None, false)));
        assert_eq!(events.last().map(|p| (p.scanned, p.total, p.done)), Some((5, Some(5), true)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn folder_kinds_filter_what_is_indexed() {
        let dir = temp_tree("kinds", TREE);
        let db = Db::in_memory().unwrap();
        let f = folder(&db, &dir, FolderKind::Music);
        let summary = scan_folder(&db, &f, &settings(), &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(summary.found, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rescan_touches_only_what_changed() {
        let dir = temp_tree("rescan", TREE);
        let db = Db::in_memory().unwrap();
        let f = folder(&db, &dir, FolderKind::Mixed);
        let none = AtomicBool::new(false);
        scan_folder(&db, &f, &settings(), &none, |_| {}).unwrap();
        let again = scan_folder(&db, &f, &settings(), &none, |_| {}).unwrap();
        assert_eq!(again, ScanSummary { found: 5, added_or_changed: 0, removed: 0 });

        std::fs::remove_file(dir.join("Films/Alien.mp4")).unwrap();
        std::fs::rename(dir.join("Show/Show S01E02.mkv"), dir.join("Show/Show S01E03.mkv"))
            .unwrap();
        std::fs::write(dir.join("Films/New Film.mkv"), b"longer than before").unwrap();
        std::fs::write(dir.join("Films/Old.rmvb"), b"changed size").unwrap();
        let third = scan_folder(&db, &f, &settings(), &none, |_| {}).unwrap();
        assert_eq!(third, ScanSummary { found: 5, added_or_changed: 3, removed: 2 });
        assert_eq!(
            titles(&db, MediaKind::Video),
            vec!["New Film", "Old", "Show S01E01", "Show S01E03"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn episodes_get_their_series_and_films_do_not() {
        let dir = temp_tree("series", TREE);
        let files = walk(&dir, FolderKind::Video, &settings(), &AtomicBool::new(false), |_| {});
        let rows: Vec<MediaRow> = files.iter().map(describe).collect();
        let episode = rows.iter().find(|r| r.title == "Show S01E02").unwrap();
        assert_eq!(
            (episode.series_key.as_deref(), episode.season, episode.episode),
            (Some("show"), Some(1), Some(2))
        );
        let film = rows.iter().find(|r| r.title == "Alien").unwrap();
        assert_eq!(film.series_key, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cancelled_scan_changes_nothing() {
        let dir = temp_tree("cancel", TREE);
        let db = Db::in_memory().unwrap();
        let f = folder(&db, &dir, FolderKind::Mixed);
        let summary = scan_folder(&db, &f, &settings(), &AtomicBool::new(true), |_| {}).unwrap();
        assert_eq!(summary, ScanSummary::default());
        assert!(titles(&db, MediaKind::Video).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The A§14 budget: at least 2 000 files a second with metadata from file names.
    #[test]
    fn scanning_two_thousand_files_is_quick() {
        let names: Vec<String> = (0..2000)
            .map(|n| format!("S{:02}/Show S{:02}E{:03}.mkv", n / 100, n / 100, n % 100))
            .collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let dir = temp_tree("speed", &refs);
        let db = Db::in_memory().unwrap();
        let f = folder(&db, &dir, FolderKind::Video);
        let started = Instant::now();
        let summary = scan_folder(&db, &f, &settings(), &AtomicBool::new(false), |_| {}).unwrap();
        let took = started.elapsed();
        assert_eq!(summary.found, 2000);
        // Generous for a loaded development machine; the measured rate goes in the phase notes.
        assert!(took < Duration::from_secs(5), "2 000 files took {took:?}");
        eprintln!("scan rate: {:.0} files/s", 2000.0 / took.as_secs_f64());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
