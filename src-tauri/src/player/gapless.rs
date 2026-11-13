//! Gapless albums (ARCHITECTURE §8.1 note). vihpluh's queue stays the authority, but for
//! music the next item is also appended to mpv's own playlist, so mpv can open it ahead of
//! time (`prefetch-playlist`) and run one track into the next (`gapless-audio`). mpv's
//! playlist is never more than `[current, next]`, and is put right after every queue change.
//!
//! When mpv moves on by itself, the core follows: the queue advances, the new file gets its
//! identity, history, and presets as on any load — only the `loadfile` is skipped.

use std::path::Path;

use super::Core;
use super::flow::LoadOpts;
use crate::config::schema::Gapless as GaplessSetting;
use crate::ipc::dto::MediaKind;
use crate::media::files::AUDIO_EXTENSIONS;
use crate::queue::Decision;

#[derive(Debug, Default)]
pub(super) struct Gapless {
    /// The queue item (id) that sits behind the current file in mpv's playlist.
    appended: Option<String>,
    /// mpv finished the current file and is opening the appended one.
    advancing: bool,
}

/// Whether a path is a local audio file by its extension.
pub fn is_audio_file(source: &str) -> bool {
    !crate::cli::is_url(source)
        && Path::new(source)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

impl Core {
    /// mpv's playlist was replaced or cleared (a new load, stop): nothing is appended now.
    pub(super) fn gapless_reset(&mut self) {
        self.gapless = Gapless::default();
    }

    /// The queue item that should follow the current file without a gap, if any: music
    /// after music, when nothing asks playback to stop at the end of this file.
    fn gapless_next(&self) -> Option<(String, String)> {
        if self.config.audio.gapless == GaplessSetting::No {
            return None;
        }
        let media = self.mirror.state.media.as_ref()?;
        if media.kind != MediaKind::Audio || media.is_url || self.sleep_stops_at_end_of_file() {
            return None;
        }
        // Only once the playing file is the queue's current item.
        let current = self.queue.current()?;
        if self.current.as_ref().map(|c| c.item_id.as_str()) != Some(current.id.as_str()) {
            return None;
        }
        let next = self.queue.peek_after_end()?;
        (next.error.is_none() && is_audio_file(&next.source) && Path::new(&next.source).is_file())
            .then(|| (next.id.clone(), next.source.clone()))
    }

    /// Makes mpv's playlist `[current, next]` again. Called when a file has loaded and after
    /// anything that can change what comes next.
    pub(super) fn sync_gapless(&mut self) {
        let wanted = self.gapless_next();
        if wanted.as_ref().map(|(id, _)| id) == self.gapless.appended.as_ref() {
            return;
        }
        if self.gapless.appended.take().is_some() {
            self.run_command(&["playlist-remove", "1"]);
        }
        if let Some((id, source)) = wanted {
            tracing::debug!("gapless: next track queued in mpv");
            self.run_command(&["loadfile", &source, "append"]);
            self.gapless.appended = Some(id);
        }
    }

    /// The current file ended naturally while a next one was appended: mpv is moving on.
    pub(super) fn gapless_file_ended(&mut self) {
        if self.gapless.appended.is_some() {
            self.gapless.advancing = true;
        }
    }

    /// mpv started the appended file: move the queue to it and take the file over.
    pub(super) fn gapless_file_started(&mut self) {
        if !std::mem::take(&mut self.gapless.advancing) {
            return;
        }
        let Some(next_id) = self.gapless.appended.take() else { return };
        self.finish_current();
        // The played entry leaves mpv's playlist: the new file is entry 0 again.
        self.run_command(&["playlist-remove", "0"]);
        match self.queue.on_ended(self.config.playback.at_queue_end) {
            Decision::Load(item) if item.id == next_id => {
                tracing::debug!("gapless: mpv moved to the next track");
                self.adopt(item);
            }
            // The queue changed its mind in the last moment: do what it says instead.
            other => self.apply_decision(other, LoadOpts::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_audio_files_continue_gaplessly() {
        assert!(is_audio_file("E:/Music/01 Track.FLAC"));
        assert!(is_audio_file("song.mp3"));
        assert!(!is_audio_file("E:/Shows/e01.mkv"));
        assert!(!is_audio_file("https://example.com/stream.mp3"));
        assert!(!is_audio_file("no-extension"));
    }
}
