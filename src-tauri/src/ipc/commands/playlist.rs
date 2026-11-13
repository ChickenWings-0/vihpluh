use std::path::Path;

use tauri::State;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::library::index;
use crate::library::playlists::{self, Playlist, PlaylistItem, PlaylistSummary};

#[tauri::command(async)]
pub fn playlist_list(state: State<'_, AppState>) -> Vec<PlaylistSummary> {
    state.playlists.list().iter().map(Playlist::summary).collect()
}

/// A playlist, with files that have moved pointed at their new place (found through the
/// library by media key, ARCHITECTURE §9.5) — and saved that way.
#[tauri::command(async)]
pub fn playlist_get(state: State<'_, AppState>, id: String) -> AppResult<Playlist> {
    let mut playlist = state.playlists.get(&id)?;
    if let Some(db) = &state.db {
        let moved = playlists::relocate(&mut playlist, |key| {
            let key = key.to_string();
            db.call(move |c| index::path_for_key(c, &key)).ok().flatten()
        });
        if moved > 0 {
            tracing::info!("playlist {id}: {moved} moved files found again");
            state.playlists.save(playlist.clone())?;
        }
    }
    Ok(playlist)
}

/// Writes a playlist; an empty `id` makes a new one. Returns its id.
#[tauri::command(async)]
pub fn playlist_save(state: State<'_, AppState>, playlist: Playlist) -> AppResult<String> {
    state.playlists.save(playlist)
}

#[tauri::command(async)]
pub fn playlist_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.playlists.delete(&id)
}

fn item_for(source: &str) -> PlaylistItem {
    PlaylistItem {
        source: source.to_string(),
        title: crate::queue::title_for(source),
        media_key: crate::media::identity::media_key(source).ok(),
        duration: None,
    }
}

/// Appends files or URLs to a playlist ("Add to playlist").
#[tauri::command(async)]
pub fn playlist_add_items(
    state: State<'_, AppState>,
    id: String,
    sources: Vec<String>,
) -> AppResult<()> {
    let mut playlist = state.playlists.get(&id)?;
    let sources = crate::media::files::expand_sources(&sources);
    playlist.items.extend(sources.iter().map(|source| item_for(source)));
    state.playlists.save(playlist).map(|_| ())
}

/// Imports an `.m3u` / `.m3u8` file as a new playlist named after it. Returns its id.
#[tauri::command(async)]
pub fn playlist_import_m3u(state: State<'_, AppState>, path: String) -> AppResult<String> {
    let file = Path::new(&path);
    let bytes = std::fs::read(file)?;
    let text = crate::subtitles::cues::decode(&bytes);
    let mut items = playlists::parse_m3u(&text, file.parent().unwrap_or(Path::new("")));
    if items.is_empty() {
        return Err(AppError::invalid("that file lists nothing playable"));
    }
    for item in &mut items {
        item.media_key = crate::media::identity::media_key(&item.source).ok();
    }
    let name = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    state.playlists.save(Playlist {
        schema: playlists::SCHEMA_VERSION,
        id: String::new(),
        name: if name.trim().is_empty() { "Imported playlist".into() } else { name },
        created_at: String::new(),
        updated_at: String::new(),
        items,
    })
}

#[tauri::command(async)]
pub fn playlist_export_m3u(state: State<'_, AppState>, id: String, path: String) -> AppResult<()> {
    let playlist = state.playlists.get(&id)?;
    std::fs::write(&path, playlists::to_m3u(&playlist))?;
    Ok(())
}

/// Saves the queue as a new playlist. Returns its id.
#[tauri::command(async)]
pub fn queue_save_as_playlist(state: State<'_, AppState>, name: String) -> AppResult<String> {
    let queue = state.player.queue_state();
    if queue.items.is_empty() {
        return Err(AppError::invalid("the queue is empty"));
    }
    let items = queue
        .items
        .iter()
        .map(|item| PlaylistItem {
            source: item.source.clone(),
            title: item.title.clone(),
            media_key: item
                .media_key
                .clone()
                .or_else(|| crate::media::identity::media_key(&item.source).ok()),
            duration: item.duration,
        })
        .collect();
    state.playlists.save(Playlist {
        schema: playlists::SCHEMA_VERSION,
        id: String::new(),
        name,
        created_at: String::new(),
        updated_at: String::new(),
        items,
    })
}
