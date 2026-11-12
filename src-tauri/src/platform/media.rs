//! OS media controls: media keys and the system "now playing" overlay (SMTC on Windows,
//! MPRIS on Linux), through `souvlaki`. Behind `general.media_keys`.

use std::time::Duration;

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

use crate::ipc::dto::Status;

/// What the OS asked for, in vihpluh's terms. Each maps onto an action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaCommand {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    SeekTo(f64),
    SeekBy(f64),
}

/// What the OS overlay shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NowPlaying {
    pub title: Option<String>,
    pub duration: Option<f64>,
    pub status: Status,
    pub position: f64,
}

pub struct MediaSession {
    controls: MediaControls,
    shown: NowPlaying,
}

impl MediaSession {
    /// `window_handle` is the main window's HWND on Windows and ignored elsewhere.
    pub fn new(
        window_handle: Option<isize>,
        on_command: impl Fn(MediaCommand) + Send + 'static,
    ) -> Result<Self, String> {
        let config = PlatformConfig {
            display_name: "vihpluh",
            dbus_name: "vihpluh",
            hwnd: window_handle.map(|h| h as *mut std::ffi::c_void),
        };
        let mut controls = MediaControls::new(config).map_err(|e| format!("{e:?}"))?;
        controls
            .attach(move |event| {
                if let Some(command) = translate(&event) {
                    on_command(command);
                }
            })
            .map_err(|e| format!("{e:?}"))?;
        Ok(Self { controls, shown: NowPlaying::default() })
    }

    /// Publishes the state if it differs from what is already shown. Position alone does not
    /// count as a difference: the OS extrapolates it while playing.
    pub fn update(&mut self, now: &NowPlaying) {
        let metadata_changed = now.title != self.shown.title || now.duration != self.shown.duration;
        let playback_changed = now.status != self.shown.status;
        if metadata_changed {
            let result = self.controls.set_metadata(MediaMetadata {
                title: now.title.as_deref(),
                duration: now.duration.filter(|d| *d > 0.0).map(Duration::from_secs_f64),
                ..MediaMetadata::default()
            });
            if let Err(err) = result {
                tracing::debug!("media controls: metadata update failed: {err:?}");
            }
        }
        if metadata_changed || playback_changed {
            let progress = Some(MediaPosition(Duration::from_secs_f64(now.position.max(0.0))));
            let playback = match now.status {
                Status::Playing | Status::Buffering | Status::Seeking => {
                    MediaPlayback::Playing { progress }
                }
                Status::Paused | Status::Ended => MediaPlayback::Paused { progress },
                Status::Idle | Status::Loading | Status::Error => MediaPlayback::Stopped,
            };
            if let Err(err) = self.controls.set_playback(playback) {
                tracing::debug!("media controls: playback update failed: {err:?}");
            }
        }
        self.shown = now.clone();
    }
}

fn translate(event: &MediaControlEvent) -> Option<MediaCommand> {
    use souvlaki::SeekDirection::{Backward, Forward};
    Some(match event {
        MediaControlEvent::Play => MediaCommand::Play,
        MediaControlEvent::Pause => MediaCommand::Pause,
        MediaControlEvent::Toggle => MediaCommand::Toggle,
        MediaControlEvent::Next => MediaCommand::Next,
        MediaControlEvent::Previous => MediaCommand::Previous,
        MediaControlEvent::Stop => MediaCommand::Stop,
        MediaControlEvent::SetPosition(MediaPosition(position)) => {
            MediaCommand::SeekTo(position.as_secs_f64())
        }
        MediaControlEvent::SeekBy(Forward, amount) => MediaCommand::SeekBy(amount.as_secs_f64()),
        MediaControlEvent::SeekBy(Backward, amount) => MediaCommand::SeekBy(-amount.as_secs_f64()),
        MediaControlEvent::Seek(Forward) => MediaCommand::SeekBy(10.0),
        MediaControlEvent::Seek(Backward) => MediaCommand::SeekBy(-10.0),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_os_events() {
        assert_eq!(translate(&MediaControlEvent::Toggle), Some(MediaCommand::Toggle));
        assert_eq!(translate(&MediaControlEvent::Next), Some(MediaCommand::Next));
        assert_eq!(
            translate(&MediaControlEvent::SetPosition(MediaPosition(Duration::from_secs(90)))),
            Some(MediaCommand::SeekTo(90.0))
        );
        assert_eq!(
            translate(&MediaControlEvent::SeekBy(
                souvlaki::SeekDirection::Backward,
                Duration::from_secs(5)
            )),
            Some(MediaCommand::SeekBy(-5.0))
        );
        assert_eq!(translate(&MediaControlEvent::Raise), None);
    }
}
