use tauri::State;

use crate::AppState;
use crate::queue::session::{UiPatch, UiState};

#[tauri::command]
pub fn session_ui_get(state: State<'_, AppState>) -> UiState {
    state.player.ui_state()
}

/// Debounced into `session.json` (ARCHITECTURE §9.4).
#[tauri::command]
pub fn session_ui_set(state: State<'_, AppState>, patch: UiPatch) {
    state.player.set_ui(patch);
}
