use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::ipc::dto::MediaKind;
use crate::library::Library;
use crate::library::history::{self, RecentEntry};
use crate::library::index::{
    self, FolderKind, LibraryFolder, LibraryPage, LibraryQuery, MediaEntry,
};
use crate::media::files::{AUDIO_EXTENSIONS, is_media_file, natural_cmp};

fn library(state: &AppState) -> AppResult<&Arc<Library>> {
    state
        .library
        .as_ref()
        .ok_or_else(|| AppError::unsupported("the library database is unavailable"))
}

/// Most recently played first (idle screen "Continue watching", library "Recent").
#[tauri::command(async)]
pub fn library_recent(state: State<'_, AppState>, limit: u32) -> AppResult<Vec<RecentEntry>> {
    let Some(db) = &state.db else { return Ok(Vec::new()) };
    if !state.config.get().library.track_history {
        return Ok(Vec::new());
    }
    Ok(db.call(move |c| history::recent(c, limit.min(500)))?)
}

/// Forgets one file's history, or all of it.
#[tauri::command(async)]
pub fn library_clear_history(
    state: State<'_, AppState>,
    media_key: Option<String>,
) -> AppResult<()> {
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| AppError::unsupported("the library database is unavailable"))?;
    Ok(db.call(move |c| history::clear(c, media_key.as_deref()))?)
}

#[tauri::command(async)]
pub fn library_folders(state: State<'_, AppState>) -> AppResult<Vec<LibraryFolder>> {
    library(&state)?.folders()
}

/// Adds a folder to the library (scanned and watched), or — with `pinned` and without
/// `indexed` — pins it as a shortcut that is only browsed live.
#[tauri::command(async)]
pub fn library_add_folder(
    state: State<'_, AppState>,
    path: String,
    kind: FolderKind,
    pinned: bool,
    indexed: Option<bool>,
) -> AppResult<i64> {
    library(&state)?.add_folder(&path, kind, pinned, indexed.unwrap_or(!pinned))
}

#[tauri::command(async)]
pub fn library_remove_folder(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    library(&state)?.remove_folder(id)
}

#[tauri::command(async)]
pub fn library_rescan(state: State<'_, AppState>, folder_id: Option<i64>) -> AppResult<()> {
    library(&state)?.rescan(folder_id)
}

#[tauri::command(async)]
pub fn library_query(state: State<'_, AppState>, query: LibraryQuery) -> AppResult<LibraryPage> {
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| AppError::unsupported("the library database is unavailable"))?;
    let query = LibraryQuery { limit: query.limit.clamp(1, 500), ..query };
    Ok(db.call(move |c| index::query(c, &query))?)
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderListing {
    pub path: String,
    /// The folder above, or `None` at the top of a drive.
    pub parent: Option<String>,
    pub dirs: Vec<DirEntry>,
    pub files: Vec<MediaEntry>,
}

/// A folder as it is on disk right now: its subfolders and media files, in natural order.
/// Works for folders that are not in the library (pinned shortcuts).
#[tauri::command(async)]
pub fn library_browse(state: State<'_, AppState>, path: String) -> AppResult<FolderListing> {
    let dir = Path::new(&path);
    let entries = std::fs::read_dir(dir).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => AppError::not_found(format!("{path} no longer exists")),
        _ => AppError::from(err),
    })?;
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            dirs.push(DirEntry { name, path: entry_path.display().to_string() });
        } else if is_media_file(&entry_path) {
            files.push(entry_path);
        }
    }
    dirs.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    files.sort_by(|a, b| {
        let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
        natural_cmp(&name(a).unwrap_or_default(), &name(b).unwrap_or_default())
    });
    let sources: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
    // Resume progress, for files that were played.
    let progress = match &state.db {
        Some(db) if state.config.get().library.track_history => {
            let sources = sources.clone();
            db.call(move |c| {
                let mut statement = c.prepare(
                    "SELECT position, finished, duration FROM history WHERE source = ?1",
                )?;
                sources
                    .iter()
                    .map(|source| {
                        use rusqlite::OptionalExtension;
                        statement
                            .query_row([source], |row| {
                                Ok((
                                    row.get::<_, f64>(0)?,
                                    row.get::<_, bool>(1)?,
                                    row.get::<_, Option<f64>>(2)?,
                                ))
                            })
                            .optional()
                    })
                    .collect::<rusqlite::Result<Vec<_>>>()
            })?
        }
        _ => vec![None; sources.len()],
    };
    let files = files
        .iter()
        .zip(sources)
        .zip(progress)
        .map(|((file, source), played)| {
            let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            let audio = AUDIO_EXTENSIONS.contains(&ext.as_str());
            MediaEntry {
                id: 0,
                path: source,
                kind: if audio { MediaKind::Audio } else { MediaKind::Video },
                title: file
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                duration: played.and_then(|p| p.2),
                series_key: None,
                season: None,
                episode: None,
                artist: None,
                album_artist: None,
                album: None,
                track_no: None,
                disc_no: None,
                year: None,
                position: played.map(|p| p.0).filter(|p| *p > 0.0),
                finished: played.is_some_and(|p| p.1),
            }
        })
        .collect();
    Ok(FolderListing {
        parent: dir.parent().filter(|p| !p.as_os_str().is_empty()).map(|p| p.display().to_string()),
        path,
        dirs,
        files,
    })
}
