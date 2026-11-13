use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::AppState;
use crate::bookmarks::{self, Bookmark, BookmarksChanged};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::ipc::events;
use crate::library::db::Db;

fn db(state: &AppState) -> AppResult<&Arc<Db>> {
    state.db.as_ref().ok_or_else(|| {
        AppError::new(ErrorCode::Db, "the library database is unavailable, so bookmarks are off")
    })
}

fn announce(app: &AppHandle, media_key: &str) {
    let _ =
        app.emit(events::BOOKMARKS_CHANGED, &BookmarksChanged { media_key: media_key.to_string() });
}

#[tauri::command(async)]
pub fn bookmark_list(state: State<'_, AppState>, media_key: String) -> AppResult<Vec<Bookmark>> {
    Ok(db(&state)?.call(move |c| bookmarks::list(c, &media_key))?)
}

/// Adds a bookmark to what is playing. Defaults: the current position, the next
/// "Bookmark N" label.
#[tauri::command(async)]
pub fn bookmark_add(
    app: AppHandle,
    state: State<'_, AppState>,
    position: Option<f64>,
    label: Option<String>,
) -> AppResult<Bookmark> {
    let key = state
        .player
        .current_media()
        .map(|m| m.key)
        .filter(|key| !key.is_empty())
        .ok_or_else(|| AppError::invalid("nothing is playing"))?;
    let position = position
        .or_else(|| state.player.position())
        .ok_or_else(|| AppError::invalid("the position is not known yet"))?;
    let media_key = key.clone();
    let bookmark =
        db(&state)?.call(move |c| bookmarks::add(c, &media_key, position, label.as_deref()))?;
    announce(&app, &key);
    Ok(bookmark)
}

#[tauri::command(async)]
pub fn bookmark_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    label: Option<String>,
    position: Option<f64>,
) -> AppResult<()> {
    let updated =
        db(&state)?.call(move |c| bookmarks::update(c, id, label.as_deref(), position))?;
    let bookmark = updated.ok_or_else(|| AppError::not_found("that bookmark no longer exists"))?;
    announce(&app, &bookmark.media_key);
    Ok(())
}

#[tauri::command(async)]
pub fn bookmark_remove(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let removed = db(&state)?.call(move |c| bookmarks::remove(c, id))?;
    if let Some(bookmark) = removed {
        announce(&app, &bookmark.media_key);
    }
    Ok(())
}
