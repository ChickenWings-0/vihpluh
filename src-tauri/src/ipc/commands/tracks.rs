use tauri::State;

use crate::AppState;
use crate::error::AppResult;
use crate::ipc::dto::TrackSlot;

#[tauri::command]
pub fn tracks_select(
    state: State<'_, AppState>,
    kind: TrackSlot,
    id: Option<i64>,
) -> AppResult<()> {
    Ok(state.player.select_track(kind, id)?)
}
