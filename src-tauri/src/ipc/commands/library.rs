use tauri::State;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::library::history::{self, RecentEntry};

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
