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

/// Installed font families for the subtitle font picker, sorted, each once.
pub fn installed_fonts() -> Vec<String> {
    #[cfg(windows)]
    let faces = windows::installed_font_faces();
    #[cfg(target_os = "linux")]
    let faces = linux::installed_font_faces();
    #[cfg(not(any(windows, target_os = "linux")))]
    let faces: Vec<String> = Vec::new();
    tidy_font_list(faces)
}

/// Drops vertical-writing aliases (`@Name` on Windows) and blanks, removes duplicates, and
/// sorts without regard to case.
fn tidy_font_list(faces: Vec<String>) -> Vec<String> {
    let mut names: Vec<String> = faces
        .into_iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty() && !name.starts_with('@'))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_lists_are_sorted_and_unique() {
        let faces = ["Segoe UI", "@MS Gothic", "arial", "Segoe UI", " ", "MS Gothic", "Arial"];
        assert_eq!(
            tidy_font_list(faces.iter().map(|s| (*s).to_string()).collect()),
            vec!["arial", "Arial", "MS Gothic", "Segoe UI"]
        );
    }

    #[cfg(windows)]
    #[test]
    fn this_machine_has_fonts() {
        let fonts = installed_fonts();
        assert!(!fonts.is_empty());
        assert!(fonts.iter().all(|f| !f.starts_with('@')));
    }
}
