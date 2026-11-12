//! OS integration that is not video: console attachment, process environment, and (from
//! Phase 2) power inhibition and media keys. Feature code calls the functions here and never
//! branches on the OS itself.

use crate::config::Config;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

/// Makes `--help` / `--version` output visible when a GUI-subsystem build is started from a
/// terminal. No-op elsewhere.
pub fn attach_parent_console() {
    #[cfg(windows)]
    windows::attach_parent_console();
}

/// Sets process environment that must be in place before the UI toolkit initialises
/// (ARCHITECTURE §5.3). Must be called before any other thread exists.
pub fn prepare_environment(config: &Config) {
    #[cfg(target_os = "linux")]
    linux::prepare_environment(config);
    #[cfg(not(target_os = "linux"))]
    let _ = config;
}
