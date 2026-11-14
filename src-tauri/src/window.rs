//! The main window's modes (DESIGN_SYSTEM §9): normal / maximized / fullscreen /
//! picture-in-picture, always-on-top, the `window://state` event that keeps the frontend in
//! step, and the size and place the window comes back to next time.
//!
//! Picture-in-picture is a mode of this same window (D12): entering remembers the geometry
//! and mode, shrinks the window to a corner and keeps it on top; leaving puts everything back.
//! Where the session cannot place windows or keep them on top (Wayland), only the size and
//! the chrome change.

pub mod geometry;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, LogicalSize, PhysicalPosition, PhysicalSize, WebviewWindow, WindowEvent};
use ts_rs::TS;

use crate::config::schema::Corner;
use crate::error::AppResult;
use geometry::{Rect, ResizeEdge};

pub const EVENT_WINDOW_STATE: &str = "window://state";

/// Smallest normal window, logical pixels (the same as in `tauri.conf.json`).
const NORMAL_MIN: (f64, f64) = (480.0, 270.0);

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

/// A step of an edge drag in picture-in-picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ResizePhase {
    Start,
    Move,
    End,
}

/// What entering picture-in-picture needs to know (settings and what the session can do).
#[derive(Debug, Clone, Copy)]
pub struct PipSettings {
    /// `interface.pip_width`, logical pixels.
    pub width: f64,
    pub corner: Corner,
    /// The picture's shape (width / height).
    pub aspect: f64,
    pub can_position: bool,
    pub can_stay_on_top: bool,
    /// `interface.window_decorations = "native"`: the frame is taken off while in PiP.
    pub native_decorations: bool,
}

/// What leaving picture-in-picture goes back to.
#[derive(Debug, Clone, Copy)]
struct Before {
    rect: Rect,
    maximized: bool,
    fullscreen: bool,
    always_on_top: bool,
}

/// The remembered window (`window.json` in the data directory), physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remembered {
    pub rect: Rect,
    pub maximized: bool,
}

#[derive(Default)]
pub struct WindowManager {
    state: Mutex<WindowState>,
    before_pip: Mutex<Option<Before>>,
    pip: Mutex<Option<PipSettings>>,
    resize_start: Mutex<Option<Rect>>,
    /// The last rectangle the window had while plain (not maximized, fullscreen, or PiP).
    last_normal: Mutex<Option<Rect>>,
    store: Mutex<Option<PathBuf>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Outer position with the inner size: the pair `set_position` + `set_size` reproduce.
fn rect_of(window: &WebviewWindow) -> tauri::Result<Rect> {
    let position = window.outer_position()?;
    let size = window.inner_size()?;
    Ok(Rect { x: position.x, y: position.y, width: size.width, height: size.height })
}

fn place(window: &WebviewWindow, rect: Rect, can_position: bool) -> tauri::Result<()> {
    window.set_size(PhysicalSize::new(rect.width, rect.height))?;
    if can_position {
        window.set_position(PhysicalPosition::new(rect.x, rect.y))?;
    }
    Ok(())
}

impl WindowManager {
    pub fn state(&self) -> WindowState {
        *lock(&self.state)
    }

    /// Re-reads what the OS knows and emits `window://state` if anything changed.
    pub fn refresh(&self, window: &WebviewWindow) {
        let mut next = self.state();
        next.maximized = window.is_maximized().unwrap_or(false);
        next.minimized = window.is_minimized().unwrap_or(false);
        next.focused = window.is_focused().unwrap_or(false);
        if next.mode != WindowMode::Pip {
            let fullscreen = window.is_fullscreen().unwrap_or(false);
            // Leaving fullscreen by other means (a window manager shortcut) returns to normal.
            next.mode = if fullscreen { WindowMode::Fullscreen } else { WindowMode::Normal };
        }
        if next.mode == WindowMode::Normal && !next.maximized && !next.minimized {
            if let Ok(rect) = rect_of(window) {
                if rect.width > 0 && rect.height > 0 {
                    *lock(&self.last_normal) = Some(rect);
                }
            }
        }
        self.publish(window, next);
    }

    fn publish(&self, window: &WebviewWindow, next: WindowState) {
        let changed = {
            let mut state = lock(&self.state);
            let changed = *state != next;
            *state = next;
            changed
        };
        if changed {
            let _ = window.emit(EVENT_WINDOW_STATE, next);
        }
    }

    /// Hooks the window's events. Call once after the window is built. `on_minimized` hears
    /// each change of the minimized state (for `general.pause_on_minimize`).
    pub fn track(window: &WebviewWindow, on_minimized: impl Fn(bool) + Send + 'static) {
        let tracked = window.clone();
        window.on_window_event(move |event| {
            let Some(manager) = tauri::Manager::try_state::<WindowManager>(&tracked) else {
                return;
            };
            match event {
                WindowEvent::Resized(_)
                | WindowEvent::Moved(_)
                | WindowEvent::Focused(_)
                | WindowEvent::ScaleFactorChanged { .. } => {
                    let was_minimized = manager.state().minimized;
                    manager.refresh(&tracked);
                    let minimized = manager.state().minimized;
                    if minimized != was_minimized {
                        on_minimized(minimized);
                    }
                }
                WindowEvent::CloseRequested { .. } => manager.remember(&tracked),
                _ => {}
            }
        });
    }

    pub fn set_mode(
        &self,
        window: &WebviewWindow,
        mode: WindowMode,
        pip: PipSettings,
    ) -> AppResult<()> {
        let current = self.state().mode;
        if current == mode {
            return Ok(());
        }
        if current == WindowMode::Pip {
            self.leave_pip(window)?;
            if mode == WindowMode::Fullscreen {
                window.set_fullscreen(true)?;
            }
        } else {
            match mode {
                WindowMode::Pip => self.enter_pip(window, pip)?,
                WindowMode::Fullscreen => window.set_fullscreen(true)?,
                WindowMode::Normal => window.set_fullscreen(false)?,
            }
        }
        let mut next = self.state();
        next.mode = match mode {
            WindowMode::Pip => WindowMode::Pip,
            // Leaving PiP returns to the mode it was entered from.
            _ if window.is_fullscreen().unwrap_or(false) => WindowMode::Fullscreen,
            _ => WindowMode::Normal,
        };
        next.maximized = window.is_maximized().unwrap_or(false);
        if mode == WindowMode::Pip && pip.can_stay_on_top {
            next.always_on_top = true;
        }
        self.publish(window, next);
        Ok(())
    }

    fn enter_pip(&self, window: &WebviewWindow, pip: PipSettings) -> AppResult<()> {
        let fullscreen = window.is_fullscreen()?;
        let maximized = window.is_maximized()?;
        if fullscreen {
            window.set_fullscreen(false)?;
        }
        if maximized {
            window.unmaximize()?;
        }
        let before = Before {
            rect: rect_of(window)?,
            maximized,
            fullscreen,
            always_on_top: self.state().always_on_top,
        };
        *lock(&self.before_pip) = Some(before);
        *lock(&self.pip) = Some(pip);

        let scale = window.scale_factor()?;
        if pip.native_decorations {
            window.set_decorations(false)?;
        }
        window.set_min_size(Some(LogicalSize::new(geometry::PIP_MIN.0, geometry::PIP_MIN.1)))?;
        if pip.can_position {
            // The edges resize through `pip_resize`, which keeps the picture's shape; the
            // toolkit's own free resize is switched off meanwhile.
            window.set_resizable(false)?;
        }
        if pip.can_stay_on_top {
            window.set_always_on_top(true)?;
        }
        let size = geometry::pip_size(pip.width * scale, pip.aspect, scale);
        let mut rect = Rect { x: before.rect.x, y: before.rect.y, width: size.0, height: size.1 };
        if pip.can_position {
            if let Some(monitor) = window.current_monitor()? {
                let area = monitor.work_area();
                let work = Rect {
                    x: area.position.x,
                    y: area.position.y,
                    width: area.size.width,
                    height: area.size.height,
                };
                let margin = (geometry::PIP_MARGIN * monitor.scale_factor()).round() as i32;
                (rect.x, rect.y) = geometry::pip_position(work, size, pip.corner, margin);
                // `set_position` places the outer frame, which on Windows includes an
                // invisible border around an undecorated window: shift by it so the part
                // that can be seen sits on the margin.
                if let (Ok(inner), Ok(outer)) = (window.inner_position(), window.outer_position()) {
                    rect.x -= inner.x - outer.x;
                    rect.y -= inner.y - outer.y;
                }
            }
        }
        place(window, rect, pip.can_position)?;
        Ok(())
    }

    fn leave_pip(&self, window: &WebviewWindow) -> AppResult<()> {
        let pip = lock(&self.pip).take();
        let Some(before) = lock(&self.before_pip).take() else { return Ok(()) };
        *lock(&self.resize_start) = None;
        let can_position = pip.is_none_or(|p| p.can_position);
        window.set_always_on_top(before.always_on_top)?;
        window.set_resizable(true)?;
        window.set_min_size(Some(LogicalSize::new(NORMAL_MIN.0, NORMAL_MIN.1)))?;
        if pip.is_some_and(|p| p.native_decorations) {
            window.set_decorations(true)?;
        }
        place(window, before.rect, can_position)?;
        if before.maximized {
            window.maximize()?;
        }
        if before.fullscreen {
            window.set_fullscreen(true)?;
        }
        let mut next = self.state();
        next.always_on_top = before.always_on_top;
        *lock(&self.state) = next;
        Ok(())
    }

    /// An edge drag in picture-in-picture: `dx`/`dy` are logical pixels since `Start`.
    pub fn pip_resize(
        &self,
        window: &WebviewWindow,
        phase: ResizePhase,
        edge: ResizeEdge,
        dx: f64,
        dy: f64,
    ) -> AppResult<()> {
        let Some(pip) = *lock(&self.pip) else { return Ok(()) };
        match phase {
            ResizePhase::Start => *lock(&self.resize_start) = Some(rect_of(window)?),
            ResizePhase::End => *lock(&self.resize_start) = None,
            ResizePhase::Move => {
                let Some(start) = *lock(&self.resize_start) else { return Ok(()) };
                let scale = window.scale_factor()?;
                let (dx, dy) = ((dx * scale).round() as i32, (dy * scale).round() as i32);
                let rect = geometry::pip_resize(start, edge, dx, dy, pip.aspect, scale);
                place(window, rect, pip.can_position)?;
            }
        }
        Ok(())
    }

    /// The picture changed shape (another file): keep the width, refit the height, and keep
    /// the window against its corner.
    pub fn pip_refit(&self, window: &WebviewWindow, aspect: f64) -> AppResult<()> {
        let pip = {
            let mut guard = lock(&self.pip);
            let Some(pip) = guard.as_mut() else { return Ok(()) };
            if (pip.aspect - aspect).abs() < 0.005 {
                return Ok(());
            }
            pip.aspect = aspect;
            *pip
        };
        let now = rect_of(window)?;
        let scale = window.scale_factor()?;
        let (width, height) = geometry::pip_size(f64::from(now.width), aspect, scale);
        let right = matches!(pip.corner, Corner::TopRight | Corner::BottomRight);
        let bottom = matches!(pip.corner, Corner::BottomLeft | Corner::BottomRight);
        let rect = Rect {
            x: if right { now.right() - width as i32 } else { now.x },
            y: if bottom { now.bottom() - height as i32 } else { now.y },
            width,
            height,
        };
        place(window, rect, pip.can_position)?;
        Ok(())
    }

    pub fn set_always_on_top(&self, window: &WebviewWindow, enabled: bool) -> AppResult<()> {
        window.set_always_on_top(enabled)?;
        let mut next = self.state();
        next.always_on_top = enabled;
        self.publish(window, next);
        Ok(())
    }

    // ── Remembering the window between runs ─────────────────────────────────

    /// Puts the window where it was last time, if that place is still on a screen. Call
    /// before the window is shown. `path` is where `remember` writes from now on.
    pub fn restore(&self, window: &WebviewWindow, path: PathBuf, can_position: bool) {
        let remembered = read_remembered(&path);
        *lock(&self.store) = Some(path);
        let Some(remembered) = remembered else { return };
        let monitors: Vec<Rect> = window
            .available_monitors()
            .unwrap_or_default()
            .iter()
            .map(|m| {
                let area = m.work_area();
                Rect {
                    x: area.position.x,
                    y: area.position.y,
                    width: area.size.width,
                    height: area.size.height,
                }
            })
            .collect();
        let reachable = geometry::is_reachable(remembered.rect, &monitors);
        // A window that would open off every screen keeps its remembered size only.
        if let Err(err) = place(window, remembered.rect, can_position && reachable) {
            tracing::warn!("could not restore the window's size and place: {err}");
            return;
        }
        *lock(&self.last_normal) = Some(remembered.rect);
        if remembered.maximized {
            let _ = window.maximize();
        }
    }

    /// Writes the window's plain size and place (and whether it was maximized) for next
    /// time. In fullscreen or picture-in-picture, that is what it would go back to.
    pub fn remember(&self, window: &WebviewWindow) {
        let Some(path) = lock(&self.store).clone() else { return };
        let before = *lock(&self.before_pip);
        let (rect, maximized) = match before {
            Some(before) => (Some(before.rect), before.maximized),
            None => (*lock(&self.last_normal), window.is_maximized().unwrap_or(false)),
        };
        let Some(rect) = rect else { return };
        let remembered = Remembered { rect, maximized };
        match serde_json::to_vec_pretty(&remembered) {
            Ok(bytes) => {
                if let Err(err) = crate::media::thumbs::write_atomic(&path, &bytes) {
                    tracing::warn!("could not save the window's size and place: {err}");
                }
            }
            Err(err) => tracing::warn!("could not save the window's size and place: {err}"),
        }
    }
}

/// `None` for a missing, unreadable, or implausible file.
fn read_remembered(path: &Path) -> Option<Remembered> {
    let text = std::fs::read_to_string(path).ok()?;
    let remembered: Remembered = serde_json::from_str(&text).ok()?;
    let rect = remembered.rect;
    let plausible = (200..=20_000).contains(&rect.width) && (120..=20_000).contains(&rect.height);
    plausible.then_some(remembered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vihpluh-window-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("window.json")
    }

    #[test]
    fn a_remembered_window_round_trips() {
        let path = temp("round-trip");
        let remembered = Remembered {
            rect: Rect { x: -1200, y: 80, width: 1600, height: 900 },
            maximized: true,
        };
        std::fs::write(&path, serde_json::to_vec(&remembered).unwrap()).unwrap();
        assert_eq!(read_remembered(&path), Some(remembered));
    }

    #[test]
    fn missing_broken_and_implausible_files_are_ignored() {
        let path = temp("broken");
        assert_eq!(read_remembered(&path), None, "no file");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(read_remembered(&path), None);
        std::fs::write(&path, r#"{"rect":{"x":0,"y":0,"width":3,"height":2},"maximized":false}"#)
            .unwrap();
        assert_eq!(read_remembered(&path), None, "a 3 × 2 window is not one");
    }
}
