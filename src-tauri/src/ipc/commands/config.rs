use tauri::State;

use crate::AppState;
use crate::config::Config;
use crate::error::AppResult;

#[tauri::command]
pub fn config_get(state: State<'_, AppState>) -> Config {
    (*state.config.get()).clone()
}

/// The shipped defaults: what a row is compared with to mark it as changed, and what its
/// reset goes back to.
#[tauri::command]
pub fn config_defaults() -> Config {
    crate::config::defaults()
}

/// Writes one setting (dotted path, e.g. `"subtitles.font_size"`). Touches the disk.
#[tauri::command(async)]
pub fn config_set(
    state: State<'_, AppState>,
    path: String,
    value: serde_json::Value,
) -> AppResult<()> {
    state.config.set(&path, &value)
}

/// Resets one setting, or everything when `path` is omitted. Touches the disk.
#[tauri::command(async)]
pub fn config_reset(state: State<'_, AppState>, path: Option<String>) -> AppResult<()> {
    state.config.reset(path.as_deref())
}
