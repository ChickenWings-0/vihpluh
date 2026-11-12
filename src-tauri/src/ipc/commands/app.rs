use tauri::{Manager, State, WebviewWindow};

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::ipc::dto::AppInfo;

/// Runs off the main thread: reading the versions waits for mpv's core, which is still
/// bringing up the video output during startup.
#[tauri::command(async)]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    let versions = state.player.versions();
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        mpv_version: versions.mpv.clone(),
        ffmpeg_version: versions.ffmpeg.clone(),
        capabilities: state.capabilities,
        paths: state.paths.dto(),
    }
}

/// The frontend has painted its first frame: reveal the window (ARCHITECTURE §4.3 step 7).
#[tauri::command]
pub fn app_ready(window: WebviewWindow) -> AppResult<()> {
    crate::reveal_main_window(window.app_handle());
    Ok(())
}

#[tauri::command]
pub fn app_quit(app: tauri::AppHandle) -> AppResult<()> {
    app.exit(0);
    Ok(())
}

impl From<tauri::Error> for AppError {
    fn from(err: tauri::Error) -> Self {
        AppError::internal(err.to_string())
    }
}
