//! Keeping the display or the system awake while something plays.
//!
//! vihpluh does this itself on every platform (mpv's own `stop-screensaver` is switched off):
//! the Linux render backend has no mpv window to do it, and one code path is easier to reason
//! about than two.

use crate::ipc::dto::{MediaKind, Status};

/// How much of the machine must stay awake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wake {
    /// Nothing is playing: normal power management.
    #[default]
    None,
    /// Audio is playing: the system stays awake, the display may turn off.
    System,
    /// Video is playing: the display stays on.
    Display,
}

/// The level required for a playback state. Only actual playback holds the machine awake;
/// paused, ended, and idle let it sleep.
pub fn required(status: Status, kind: Option<MediaKind>) -> Wake {
    match (status, kind) {
        (Status::Playing | Status::Buffering | Status::Seeking, Some(MediaKind::Video)) => {
            Wake::Display
        }
        (Status::Playing | Status::Buffering | Status::Seeking, Some(MediaKind::Audio)) => {
            Wake::System
        }
        _ => Wake::None,
    }
}

/// Applies wake levels. All calls must come from one thread (the Windows state is per-thread).
pub struct Inhibitor {
    current: Wake,
}

impl Inhibitor {
    pub fn new() -> Self {
        Self { current: Wake::None }
    }

    pub fn set(&mut self, wake: Wake) {
        if wake == self.current {
            return;
        }
        tracing::debug!(?wake, "power: wake level changed");
        apply(wake);
        self.current = wake;
    }
}

impl Default for Inhibitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Inhibitor {
    fn drop(&mut self) {
        self.set(Wake::None);
    }
}

#[cfg(windows)]
fn apply(wake: Wake) {
    use windows::Win32::System::Power::{
        ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
    };
    let flags = match wake {
        Wake::None => ES_CONTINUOUS,
        Wake::System => ES_CONTINUOUS | ES_SYSTEM_REQUIRED,
        Wake::Display => ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED,
    };
    // SAFETY: a plain flag-setting call with no pointers.
    let previous = unsafe { SetThreadExecutionState(flags) };
    if previous.0 == 0 {
        tracing::warn!("power: SetThreadExecutionState failed");
    }
}

#[cfg(not(windows))]
fn apply(wake: Wake) {
    // Linux: `GtkApplication::inhibit` arrives with the render backend (spike S2). Until then
    // the request is only logged, so display sleep is not prevented there.
    tracing::debug!(?wake, "power: inhibition is not implemented on this platform yet");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_active_playback_keeps_the_machine_awake() {
        use MediaKind::{Audio, Video};
        assert_eq!(required(Status::Playing, Some(Video)), Wake::Display);
        assert_eq!(required(Status::Buffering, Some(Video)), Wake::Display);
        assert_eq!(required(Status::Seeking, Some(Video)), Wake::Display);
        assert_eq!(required(Status::Playing, Some(Audio)), Wake::System);
        assert_eq!(required(Status::Paused, Some(Video)), Wake::None);
        assert_eq!(required(Status::Ended, Some(Video)), Wake::None);
        assert_eq!(required(Status::Idle, None), Wake::None);
        assert_eq!(required(Status::Error, Some(Video)), Wake::None);
        assert_eq!(required(Status::Playing, None), Wake::None);
    }
}
