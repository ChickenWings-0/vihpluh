//! Development-only commands used to verify behaviour from outside the app. Not compiled into
//! release builds.

use tauri::State;

use crate::AppState;

#[tauri::command]
pub fn debug_mpv_get(state: State<'_, AppState>, name: String) -> Option<String> {
    state.player.debug_property(&name)
}
