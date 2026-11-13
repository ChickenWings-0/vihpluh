use serde::Deserialize;
use tauri::State;
use ts_rs::TS;

use crate::AppState;
use crate::error::AppResult;

/// Fractions of the window (0–1) kept clear of video on each side.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[ts(export)]
pub struct Margins {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

#[tauri::command]
pub fn video_set_margins(state: State<'_, AppState>, req: Margins) -> AppResult<()> {
    Ok(state.player.set_margins(req.left, req.right, req.top, req.bottom)?)
}
