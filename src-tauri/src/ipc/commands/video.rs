use serde::Deserialize;
use tauri::State;
use ts_rs::TS;

use crate::AppState;
use crate::error::AppResult;
use crate::ipc::dto::{ShaderEntry, VideoPatch};

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

#[tauri::command]
pub fn video_set_adjust(state: State<'_, AppState>, patch: VideoPatch) {
    state.player.set_video(patch);
}

#[tauri::command]
pub fn video_reset(state: State<'_, AppState>) {
    state.player.reset_video();
}

/// The `.glsl` files in `<config>/shaders/`, sorted by name.
#[tauri::command(async)]
pub fn shaders_list(state: State<'_, AppState>) -> Vec<ShaderEntry> {
    let dir = state.paths.config.join("shaders");
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut shaders: Vec<ShaderEntry> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file() && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("glsl"))
        })
        .map(|path| ShaderEntry {
            name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            path: path.display().to_string(),
        })
        .collect();
    shaders.sort_by(|a, b| crate::media::files::natural_cmp(&a.name, &b.name));
    shaders
}
