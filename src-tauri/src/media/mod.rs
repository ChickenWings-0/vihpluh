//! Media services: what counts as media, identity, thumbnails, waveforms (ARCHITECTURE §11).

pub mod cache;
pub mod files;
pub mod identity;
pub mod thumbs;
pub mod waveform;

use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;

/// The longest background work waits for a first frame that does not come.
const FIRST_FRAME_WAIT: Duration = Duration::from_secs(3);

/// Holds background media work (thumbnails, embedded cue reads, waveform decodes) back while
/// the primary player is opening a file, so it does not compete with time-to-first-frame
/// (ARCHITECTURE §14). Workers wait on their own threads; the player only flips the flag.
pub struct FirstFrameGate {
    opening: Mutex<bool>,
    shown: Condvar,
}

impl FirstFrameGate {
    pub const fn new() -> Self {
        Self { opening: Mutex::new(false), shown: Condvar::new() }
    }

    /// A file started opening: hold background work back.
    pub fn opening(&self) {
        *self.opening.lock().unwrap_or_else(PoisonError::into_inner) = true;
    }

    /// The first frame is up (or the load ended without one): let background work go.
    pub fn shown(&self) {
        *self.opening.lock().unwrap_or_else(PoisonError::into_inner) = false;
        self.shown.notify_all();
    }

    /// Blocks the calling worker until the first frame is up, for at most `limit`.
    pub fn wait(&self, limit: Duration) {
        let opening = self.opening.lock().unwrap_or_else(PoisonError::into_inner);
        let _ = self.shown.wait_timeout_while(opening, limit, |opening| *opening);
    }
}

impl Default for FirstFrameGate {
    fn default() -> Self {
        Self::new()
    }
}

/// The primary player's gate.
pub static FIRST_FRAME: FirstFrameGate = FirstFrameGate::new();

/// What a background media worker calls before its heavy part.
pub fn wait_for_first_frame() {
    let started = std::time::Instant::now();
    FIRST_FRAME.wait(FIRST_FRAME_WAIT);
    let held = started.elapsed();
    if held >= Duration::from_millis(1) {
        tracing::debug!(
            ms = held.as_millis() as u64,
            "background media work held for the first frame"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Instant;

    #[test]
    fn the_gate_holds_work_until_the_first_frame_and_never_longer_than_its_limit() {
        let gate = Arc::new(FirstFrameGate::new());
        let started = Instant::now();
        gate.wait(Duration::from_secs(5));
        assert!(started.elapsed() < Duration::from_millis(500), "open: no wait");

        gate.opening();
        let started = Instant::now();
        gate.wait(Duration::from_millis(50));
        assert!(started.elapsed() >= Duration::from_millis(50), "held until the limit");

        let opener = gate.clone();
        let frame = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            opener.shown();
        });
        let started = Instant::now();
        gate.wait(Duration::from_secs(5));
        assert!(started.elapsed() < Duration::from_secs(2), "released by the first frame");
        frame.join().unwrap();
    }
}
