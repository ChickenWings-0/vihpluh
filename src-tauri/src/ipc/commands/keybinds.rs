use tauri::State;

use crate::AppState;
use crate::config::keybinds::ResolvedKeymap;
use crate::error::AppResult;

#[tauri::command]
pub fn keybinds_get(state: State<'_, AppState>) -> ResolvedKeymap {
    (*state.keybinds.get()).clone()
}

/// Sets the keys of one action in one context (the hotkey editor). Written to `keybinds.toml`
/// at once; only what differs from the base keymap is stored.
#[tauri::command(async)]
pub fn keybinds_set(
    state: State<'_, AppState>,
    context: String,
    action: String,
    keys: Vec<String>,
) -> AppResult<()> {
    state.keybinds.set(&context, &action, &keys)
}

/// Puts one action, or (without `action`) every action, back to the base keymap.
#[tauri::command(async)]
pub fn keybinds_reset(state: State<'_, AppState>, action: Option<String>) -> AppResult<()> {
    state.keybinds.reset(action.as_deref())
}

/// Switches the base keymap (`vihpluh`, `vlc`, `mpv`, `none`); the user's bindings stay.
#[tauri::command(async)]
pub fn keybinds_set_base(state: State<'_, AppState>, base: String) -> AppResult<()> {
    state.keybinds.set_base(&base)
}

/// Deletes a `[[custom]]` binding by its position in the file.
#[tauri::command(async)]
pub fn keybinds_remove_custom(state: State<'_, AppState>, index: usize) -> AppResult<()> {
    state.keybinds.remove_custom(index)
}
