use tauri::State;

use crate::AppState;
use crate::config::keybinds::ResolvedKeymap;

#[tauri::command]
pub fn keybinds_get(state: State<'_, AppState>) -> ResolvedKeymap {
    (*state.keybinds.get()).clone()
}
