//! OS integration that is not video: console attachment, process environment, power
//! inhibition, media keys, and instance detection. Feature code calls the functions here and
//! never branches on the OS itself.

pub mod media;
pub mod pipe;
pub mod power;

use crate::config::Config;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
pub(crate) mod windows;

/// Makes `--help` / `--version` output visible when a GUI-subsystem build is started from a
/// terminal. No-op elsewhere.
pub fn attach_parent_console() {
    #[cfg(windows)]
    windows::attach_parent_console();
}

/// The native handle the OS media controls attach to (the HWND on Windows; unused elsewhere).
pub fn native_window_handle(window: &tauri::WebviewWindow) -> Option<isize> {
    #[cfg(windows)]
    {
        window.hwnd().ok().map(|h| h.0 as isize)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        None
    }
}

/// Opens the system file manager with `path` selected (Windows) or its folder open (Linux).
pub fn reveal_in_file_manager(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        // explorer.exe takes "/select," and the path as one argument.
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(path.as_os_str());
        std::process::Command::new("explorer.exe").arg(arg).spawn().map(|_| ())
    }
    #[cfg(not(windows))]
    {
        let dir = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
        std::process::Command::new("xdg-open").arg(dir).spawn().map(|_| ())
    }
}

/// Whether another vihpluh is already running and will accept forwarded arguments.
///
/// Mirrors the marker that `tauri-plugin-single-instance` creates on Windows (a hidden window
/// named `<identifier>-siw`), so the check can run before mpv starts. On Linux the plugin
/// uses D-Bus; there the check is not done early and the plugin alone handles the hand-off
/// (mpv starts briefly in the second process — acceptable until it matters).
pub fn other_instance_running(identifier: &str) -> bool {
    #[cfg(windows)]
    {
        windows::single_instance_window_exists(identifier)
    }
    #[cfg(not(windows))]
    {
        let _ = identifier;
        false
    }
}

/// Shows a window without activating it and behind every other window. Used only by
/// development verification runs so they do not take focus from whatever the user is doing.
/// Returns false where this is not supported.
#[cfg(debug_assertions)]
pub fn show_window_unobtrusively(window: &tauri::WebviewWindow) -> bool {
    #[cfg(windows)]
    {
        windows::show_window_unobtrusively(window)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        false
    }
}

/// Sets process environment that must be in place before the UI toolkit initialises
/// (ARCHITECTURE §5.3). Must be called before any other thread exists.
pub fn prepare_environment(config: &Config) {
    #[cfg(target_os = "linux")]
    linux::prepare_environment(config);
    #[cfg(not(target_os = "linux"))]
    let _ = config;
}
