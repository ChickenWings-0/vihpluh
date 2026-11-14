use tauri::{State, WebviewWindow};

use crate::AppState;
use crate::config::schema::WindowDecorations;
use crate::error::AppResult;
use crate::window::geometry::{ResizeEdge, sane_aspect};
use crate::window::{PipSettings, ResizePhase, WindowManager, WindowMode, WindowState};

#[tauri::command]
pub fn window_get_state(manager: State<'_, WindowManager>, window: WebviewWindow) -> WindowState {
    manager.refresh(&window);
    manager.state()
}

fn pip_settings(state: &AppState) -> PipSettings {
    let config = state.config.get();
    PipSettings {
        width: f64::from(config.interface.pip_width),
        corner: config.interface.pip_corner,
        aspect: sane_aspect(state.player.video_aspect()),
        can_position: state.capabilities.window_positioning,
        can_stay_on_top: state.capabilities.always_on_top,
        native_decorations: config.interface.window_decorations == WindowDecorations::Native,
    }
}

/// Switches between normal, fullscreen, and picture-in-picture. Leaving picture-in-picture
/// for `normal` returns to the geometry and mode it was entered from.
#[tauri::command]
pub fn window_set_mode(
    state: State<'_, AppState>,
    manager: State<'_, WindowManager>,
    window: WebviewWindow,
    mode: WindowMode,
) -> AppResult<()> {
    manager.set_mode(&window, mode, pip_settings(&state))
}

/// One step of dragging an edge of the picture-in-picture window; the shape is kept.
#[tauri::command]
pub fn window_pip_resize(
    manager: State<'_, WindowManager>,
    window: WebviewWindow,
    phase: ResizePhase,
    edge: ResizeEdge,
    dx: f64,
    dy: f64,
) -> AppResult<()> {
    manager.pip_resize(&window, phase, edge, dx, dy)
}

/// Refits the picture-in-picture window to the shape of what is playing now.
#[tauri::command]
pub fn window_pip_refit(
    state: State<'_, AppState>,
    manager: State<'_, WindowManager>,
    window: WebviewWindow,
) -> AppResult<()> {
    manager.pip_refit(&window, sane_aspect(state.player.video_aspect()))
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
