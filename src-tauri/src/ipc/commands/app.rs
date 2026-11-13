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

/// Shows a file selected in the system file manager (or its folder, where selecting is not
/// possible). Runs the file manager as a separate process; nothing waits for it.
#[tauri::command(async)]
pub fn app_reveal(path: String) -> AppResult<()> {
    let target = std::path::PathBuf::from(&path);
    if !target.exists() {
        return Err(AppError::not_found(format!("{path} no longer exists")));
    }
    crate::platform::reveal_in_file_manager(&target)
        .map_err(|e| AppError::internal(format!("could not open the file manager: {e}")))
}

#[derive(Debug, Clone, Copy, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PickKind {
    Files,
    Folder,
}

/// Shows the native "open" dialog and returns what was chosen (empty when cancelled).
/// The dialog is modal and blocks this call, so it runs off the main thread.
#[tauri::command(async)]
pub fn app_pick_media(window: WebviewWindow, kind: PickKind) -> Vec<String> {
    use crate::media::files::{AUDIO_EXTENSIONS, VIDEO_EXTENSIONS};
    use tauri_plugin_dialog::DialogExt;

    let media: Vec<&str> = VIDEO_EXTENSIONS.iter().chain(AUDIO_EXTENSIONS).copied().collect();
    let dialog = window.dialog().file().set_parent(&window);
    let picked = match kind {
        PickKind::Files => dialog
            .set_title("Open file")
            .add_filter("Media", &media)
            .add_filter("Video", VIDEO_EXTENSIONS)
            .add_filter("Audio", AUDIO_EXTENSIONS)
            .add_filter("All files", &["*"])
            .blocking_pick_files()
            .unwrap_or_default(),
        PickKind::Folder => {
            dialog.set_title("Open folder").blocking_pick_folder().into_iter().collect()
        }
    };
    picked
        .into_iter()
        .map(|path| match path.as_path() {
            Some(path) => path.display().to_string(),
            None => path.to_string(),
        })
        .collect()
}
