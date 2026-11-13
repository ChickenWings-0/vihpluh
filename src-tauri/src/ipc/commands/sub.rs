use tauri::State;

use crate::AppState;
use crate::error::{AppError, AppResult};

#[tauri::command]
pub fn sub_set_delay(state: State<'_, AppState>, ms: i32) -> AppResult<()> {
    if ms.abs() > 600_000 {
        return Err(AppError::invalid("subtitle delay is limited to ±10 minutes"));
    }
    Ok(state.player.set_sub_delay(ms)?)
}

#[tauri::command]
pub fn sub_set_visible(state: State<'_, AppState>, visible: bool) -> AppResult<()> {
    Ok(state.player.set_sub_visible(visible)?)
}
