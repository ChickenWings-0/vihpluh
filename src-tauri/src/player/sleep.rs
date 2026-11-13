//! The sleep timer (ARCHITECTURE §6.6): after N minutes of wall-clock time, at the end of the
//! current file, or at the end of the queue, playback pauses (or vihpluh quits, per
//! `playback.sleep_action`). The volume fades out over the last 10 s and is put back
//! afterwards, so the next session does not start silent. Pure; the core drives it.

use std::time::{Duration, Instant};

use crate::config::schema::SleepAction;
use crate::ipc::dto::{OsdEvent, OsdKind, SleepKind, SleepRequest, SleepTimer, Status};
use crate::ipc::events;
use crate::queue::Decision;

use super::Core;
use super::mpv::Value;

pub const FADE: Duration = Duration::from_secs(10);
/// How often the volume steps while fading.
pub const FADE_STEP: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Minutes { deadline: Instant, minutes: f64 },
    EndOfFile,
    EndOfQueue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sleep {
    pub mode: Mode,
    /// The volume before the fade began.
    pub faded_from: Option<f64>,
}

/// What the core should do on this tick.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Nothing,
    /// Set the volume (fading).
    Volume(f64),
    /// Time is up: stop playback (pause or quit), then restore this volume.
    Fire {
        restore_volume: Option<f64>,
    },
}

impl Sleep {
    pub fn new(kind: SleepKind, minutes: Option<f64>, now: Instant) -> Option<Self> {
        let mode = match kind {
            SleepKind::Off => return None,
            SleepKind::Minutes => {
                let minutes = minutes.filter(|m| m.is_finite() && *m > 0.0)?;
                Mode::Minutes { deadline: now + Duration::from_secs_f64(minutes * 60.0), minutes }
            }
            SleepKind::EndOfFile => Mode::EndOfFile,
            SleepKind::EndOfQueue => Mode::EndOfQueue,
        };
        Some(Self { mode, faded_from: None })
    }

    /// What the frontend shows. `remaining` is whole seconds so the patch rate stays ≤ 1 Hz.
    pub fn dto(&self, now: Instant) -> SleepTimer {
        match &self.mode {
            Mode::Minutes { deadline, minutes } => SleepTimer {
                kind: "minutes".into(),
                remaining: Some(deadline.saturating_duration_since(now).as_secs_f64().ceil()),
                minutes: Some(*minutes),
            },
            Mode::EndOfFile => {
                SleepTimer { kind: "end_of_file".into(), remaining: None, minutes: None }
            }
            Mode::EndOfQueue => {
                SleepTimer { kind: "end_of_queue".into(), remaining: None, minutes: None }
            }
        }
    }

    /// Advances the timer.
    ///
    /// `left_in_media` is the playing time left before the timer's end point for the
    /// end-of-file / end-of-queue modes (None when it does not apply yet, e.g. not on the
    /// last queue item, or nothing is playing). Those modes fire from the core's end-of-file
    /// handling, not from here; this only fades them.
    pub fn tick(&mut self, now: Instant, volume: f64, left_in_media: Option<f64>) -> Step {
        let left = match &self.mode {
            Mode::Minutes { deadline, .. } => {
                let left = deadline.saturating_duration_since(now);
                if left.is_zero() {
                    return Step::Fire { restore_volume: self.faded_from };
                }
                Some(left.as_secs_f64())
            }
            Mode::EndOfFile | Mode::EndOfQueue => left_in_media,
        };
        let Some(left) = left else { return self.unfade() };
        if left > FADE.as_secs_f64() {
            return self.unfade();
        }
        let from = *self.faded_from.get_or_insert(volume);
        Step::Volume((from * left / FADE.as_secs_f64()).max(0.0))
    }

    /// The fade was entered but the end point moved away (seek back): put the volume back.
    fn unfade(&mut self) -> Step {
        match self.faded_from.take() {
            Some(volume) => Step::Volume(volume),
            None => Step::Nothing,
        }
    }

    pub fn is_fading(&self) -> bool {
        self.faded_from.is_some()
    }
}

impl Core {
    pub(super) fn set_sleep(&mut self, request: SleepRequest) {
        if let Some(volume) = self.sleep.take().and_then(|s| s.faded_from) {
            self.set_property("volume", Value::Double(volume));
        }
        self.sleep = Sleep::new(request.kind, request.minutes, Instant::now());
        self.refresh_sleep();
    }

    fn refresh_sleep(&mut self) {
        let dto = self.sleep.as_ref().map(|s| s.dto(Instant::now()));
        if dto != self.mirror.state.sleep {
            self.mirror.state.sleep = dto;
            self.dirty = true;
        }
    }

    /// Playing time left before the timer's end point, for the end-of-file/queue fades.
    fn sleep_left_in_media(&self, mode: &Mode) -> Option<f64> {
        let state = &self.mirror.state;
        let time = self.mirror.time;
        if state.status != Status::Playing || time.dur <= 0.0 {
            return None;
        }
        let last = match mode {
            Mode::EndOfFile => true,
            Mode::EndOfQueue => !self.queue.state().can_next,
            Mode::Minutes { .. } => return None,
        };
        last.then(|| (time.dur - time.pos).max(0.0) / state.speed.max(0.01))
    }

    pub(super) fn tick_sleep(&mut self, now: Instant) {
        let Some(mode) = self.sleep.as_ref().map(|s| s.mode.clone()) else { return };
        let left = self.sleep_left_in_media(&mode);
        let volume = self.mirror.state.volume;
        let step = match self.sleep.as_mut() {
            Some(sleep) => sleep.tick(now, volume, left),
            None => return,
        };
        match step {
            Step::Nothing => {}
            Step::Volume(v) => self.set_property("volume", Value::Double(v)),
            Step::Fire { restore_volume } => {
                self.sleep = None;
                let decision = match self.config.playback.sleep_action {
                    SleepAction::Quit => Decision::Quit,
                    SleepAction::Pause => {
                        self.set_property("pause", Value::Flag(true));
                        Decision::Nothing
                    }
                };
                self.after_sleep(restore_volume);
                self.apply_decision(decision, super::flow::LoadOpts::default());
            }
        }
        self.refresh_sleep();
    }

    /// The current file ended. Returns the decision that replaces the queue's when the
    /// timer ends here; `None` lets the queue decide.
    pub(super) fn sleep_at_end_of_file(&mut self) -> Option<Decision> {
        let mode = self.sleep.as_ref().map(|s| s.mode.clone())?;
        let last = match mode {
            Mode::EndOfFile => true,
            Mode::EndOfQueue => !self.queue.state().can_next,
            Mode::Minutes { .. } => false,
        };
        if !last {
            return None;
        }
        let restore = self.sleep.take().and_then(|s| s.faded_from);
        self.after_sleep(restore);
        self.refresh_sleep();
        Some(match self.config.playback.sleep_action {
            SleepAction::Quit => Decision::Quit,
            SleepAction::Pause => Decision::Hold,
        })
    }

    fn after_sleep(&mut self, restore_volume: Option<f64>) {
        if let Some(volume) = restore_volume {
            self.set_property("volume", Value::Double(volume));
        }
        let osd = OsdEvent::new(OsdKind::Info, "Sleep timer: playback paused");
        self.emit(events::OSD_SHOW, &osd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_fade_then_fire_and_restore() {
        let t0 = Instant::now();
        let mut sleep = Sleep::new(SleepKind::Minutes, Some(1.0), t0).unwrap();
        assert_eq!(sleep.dto(t0).remaining, Some(60.0));
        assert_eq!(sleep.tick(t0 + Duration::from_secs(30), 80.0, None), Step::Nothing);
        assert_eq!(sleep.tick(t0 + Duration::from_secs(55), 80.0, None), Step::Volume(40.0));
        // The fade's own volume writes must not move the reference point.
        assert_eq!(sleep.tick(t0 + Duration::from_secs(58), 40.0, None), Step::Volume(16.0));
        assert_eq!(
            sleep.tick(t0 + Duration::from_secs(60), 16.0, None),
            Step::Fire { restore_volume: Some(80.0) }
        );
    }

    #[test]
    fn end_of_file_fades_on_media_time_and_unfades_on_seek_back() {
        let now = Instant::now();
        let mut sleep = Sleep::new(SleepKind::EndOfFile, None, now).unwrap();
        assert_eq!(sleep.tick(now, 100.0, Some(300.0)), Step::Nothing);
        assert_eq!(sleep.tick(now, 100.0, Some(5.0)), Step::Volume(50.0));
        assert!(sleep.is_fading());
        assert_eq!(sleep.tick(now, 50.0, Some(120.0)), Step::Volume(100.0), "seeked back");
        assert!(!sleep.is_fading());
    }

    #[test]
    fn off_and_bad_minutes_mean_no_timer() {
        let now = Instant::now();
        assert!(Sleep::new(SleepKind::Off, None, now).is_none());
        assert!(Sleep::new(SleepKind::Minutes, None, now).is_none());
        assert!(Sleep::new(SleepKind::Minutes, Some(-1.0), now).is_none());
    }
}
