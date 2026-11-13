//! The main window's modes (DESIGN_SYSTEM §9): normal / maximized / fullscreen, always-on-top,
//! and the `window://state` event that keeps the frontend in step. Picture-in-picture is a
//! mode of this same window and arrives in Phase 11.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, WebviewWindow, WindowEvent};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

pub const EVENT_WINDOW_STATE: &str = "window://state";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum WindowMode {
    #[default]
    Normal,
    Fullscreen,
    Pip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WindowState {
    pub mode: WindowMode,
    pub maximized: bool,
    pub minimized: bool,
    pub focused: bool,
    pub always_on_top: bool,
}

#[derive(Default)]
pub struct WindowManager {
    state: Mutex<WindowState>,
}

impl WindowManager {
    pub fn state(&self) -> WindowState {
        self.state.lock().map(|s| *s).unwrap_or_default()
    }

    /// Re-reads what the OS knows and emits `window://state` if anything changed.
    pub fn refresh(&self, window: &WebviewWindow) {
        let mut next = self.state();
        next.maximized = window.is_maximized().unwrap_or(false);
        next.minimized = window.is_minimized().unwrap_or(false);
        next.focused = window.is_focused().unwrap_or(false);
        let fullscreen = window.is_fullscreen().unwrap_or(false);
        // Leaving fullscreen by other means (e.g. a window manager shortcut) returns to normal.
        if next.mode == WindowMode::Fullscreen && !fullscreen {
            next.mode = WindowMode::Normal;
        } else if fullscreen {
            next.mode = WindowMode::Fullscreen;
        }
        self.publish(window, next);
    }

    fn publish(&self, window: &WebviewWindow, next: WindowState) {
        let changed = match self.state.lock() {
            Ok(mut state) if *state != next => {
                *state = next;
                true
            }
            _ => false,
        };
        if changed {
            let _ = window.emit(EVENT_WINDOW_STATE, next);
        }
    }

    /// Hooks the window's events. Call once after the window is built.
    pub fn track(window: &WebviewWindow) {
        let tracked = window.clone();
        window.on_window_event(move |event| {
            if matches!(
                event,
                WindowEvent::Resized(_)
                    | WindowEvent::Focused(_)
                    | WindowEvent::ScaleFactorChanged { .. }
            ) {
                if let Some(manager) = tauri::Manager::try_state::<WindowManager>(&tracked) {
                    manager.refresh(&tracked);
                }
            }
        });
    }

    pub fn set_mode(&self, window: &WebviewWindow, mode: WindowMode) -> AppResult<()> {
        match mode {
            WindowMode::Pip => {
                return Err(AppError::unsupported("picture-in-picture arrives in a later phase"));
            }
            WindowMode::Fullscreen => window.set_fullscreen(true)?,
            WindowMode::Normal => window.set_fullscreen(false)?,
        }
        let mut next = self.state();
        next.mode = mode;
        self.publish(window, next);
        Ok(())
    }

    pub fn set_always_on_top(&self, window: &WebviewWindow, enabled: bool) -> AppResult<()> {
        window.set_always_on_top(enabled)?;
        let mut next = self.state();
        next.always_on_top = enabled;
        self.publish(window, next);
        Ok(())
    }
}
