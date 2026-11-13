use tauri::State;

use crate::AppState;
use crate::error::{AppError, AppResult};

#[tauri::command]
pub fn audio_set_delay(state: State<'_, AppState>, ms: i32) -> AppResult<()> {
    if ms.abs() > 600_000 {
        return Err(AppError::invalid("audio delay is limited to ±10 minutes"));
    }
    Ok(state.player.set_audio_delay(ms)?)
}
