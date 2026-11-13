use tauri::{State, WebviewWindow};

use crate::error::AppResult;
use crate::window::{WindowManager, WindowMode, WindowState};

#[tauri::command]
pub fn window_get_state(manager: State<'_, WindowManager>, window: WebviewWindow) -> WindowState {
    manager.refresh(&window);
    manager.state()
}

#[tauri::command]
pub fn window_set_mode(
    manager: State<'_, WindowManager>,
    window: WebviewWindow,
    mode: WindowMode,
) -> AppResult<()> {
    manager.set_mode(&window, mode)
}

#[tauri::command]
pub fn window_set_always_on_top(
    manager: State<'_, WindowManager>,
    window: WebviewWindow,
    enabled: bool,
) -> AppResult<()> {
    manager.set_always_on_top(&window, enabled)
}

#[tauri::command]
pub fn window_minimize(window: WebviewWindow) -> AppResult<()> {
    Ok(window.minimize()?)
}

#[tauri::command]
pub fn window_toggle_maximize(window: WebviewWindow) -> AppResult<()> {
    if window.is_maximized()? {
        window.unmaximize()?;
    } else {
        window.maximize()?;
    }
    Ok(())
}

#[tauri::command]
pub fn window_close(window: WebviewWindow) -> AppResult<()> {
    Ok(window.close()?)
}
