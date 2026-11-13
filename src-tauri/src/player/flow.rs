//! What the core does around each file (ARCHITECTURE §6.5, §8.1): open requests become queue
//! operations; every load looks up the file's history off-thread, starts at the resume point,
//! and restores remembered tracks and delays; the end of a file asks the queue what is next;
//! positions and the session are saved as playback goes.

use std::path::Path;
use std::time::{Duration, Instant};

use super::mpv::Value;
use super::{Core, CoreMsg};
use crate::config::schema::ResumeMode;
use crate::ipc::dto::{
    OpenMode, OpenRequest, OsdAction, OsdEvent, OsdKind, Status, Track, TrackKind,
};
use crate::ipc::events;
use crate::library::history::{self, HistoryEntry, TrackMemory};
use crate::queue::session::{self, SavedItem, SavedQueue, Session, UiPatch};
use crate::queue::{self, Decision, NewItem, QueueItem, Repeat};
use crate::window::WindowMode;

/// Resume positions are saved this often while playing (§6.5).
const POSITION_SAVE_EVERY: Duration = Duration::from_secs(5);
/// Session writes are debounced (§9.4).
const SESSION_DEBOUNCE: Duration = Duration::from_secs(1);
/// Toast group for unplayable-file notices.
const QUEUE_ERRORS: &str = "queue-errors";

/// Per-load options from the request that caused it.
#[derive(Debug, Clone, Default)]
pub(crate) struct LoadOpts {
    /// mpv `start=` value; overrides any resume point.
    pub start: Option<String>,
    pub paused: Option<bool>,
    pub no_resume: bool,
}

/// A load waiting for its history lookup.
#[derive(Debug)]
pub(super) struct Pending {
    generation: u64,
    item: QueueItem,
    opts: LoadOpts,
}

/// The result of the off-thread part of a load.
#[derive(Debug)]
pub(super) struct Prepared {
    generation: u64,
    media_key: Option<String>,
    history: Option<HistoryEntry>,
}

/// What the core remembers about the file that is loaded.
#[derive(Debug, Clone)]
pub(super) struct Current {
    pub item_id: String,
    pub media_key: Option<String>,
    pub history: Option<HistoryEntry>,
    /// The position playback resumed at ("always" mode), for the toast.
    pub resumed_at: Option<f64>,
    /// The saved position the "ask" toast offers.
    pub offer_resume: Option<f64>,
}

/// Queue operations from IPC (ARCHITECTURE §7.3 "queue").
#[derive(Debug)]
pub enum QueueOp {
    Add { sources: Vec<String>, at: Option<usize> },
    Remove(Vec<String>),
    Move { ids: Vec<String>, to_index: usize },
    Clear { keep_current: bool },
    Play(String),
    Next,
    Previous,
    Repeat(Repeat),
    Shuffle(bool),
    StopAfterCurrent(bool),
}

impl Core {
    fn history_enabled(&self) -> bool {
        self.db.is_some() && self.config.library.track_history
    }

    // ── Opening ─────────────────────────────────────────────────────────────

    pub(super) fn open(&mut self, request: &OpenRequest, start: Option<String>) {
        if request.sources.is_empty() {
            return;
        }
        let opts = LoadOpts {
            start: start.or_else(|| request.start_at.map(|s| format!("{s}"))),
            paused: request.paused,
            no_resume: request.no_resume.unwrap_or(false),
        };
        let users = || request.sources.iter().map(NewItem::user).collect::<Vec<_>>();
        let decision = match request.mode {
            OpenMode::Replace => {
                // Opening exactly one local file lines up its folder (§8.1).
                let single = request.sources.len() == 1 && !crate::cli::is_url(&request.sources[0]);
                let fill = (single && self.config.playback.auto_queue_folder)
                    .then(|| queue::folder_fill(&request.sources[0]))
                    .flatten();
                match fill {
                    Some((items, index)) => self.queue.replace(items, index),
                    None => self.queue.replace(users(), 0),
                }
            }
            OpenMode::Enqueue => self.queue.add(users(), None).1,
            OpenMode::PlayNext => self.queue.play_next(users()).1,
        };
        self.apply_decision(decision, opts);
    }

    pub(super) fn queue_op(&mut self, op: QueueOp) {
        let position = self.mirror.time.pos;
        let decision = match op {
            QueueOp::Add { sources, at } => {
                self.queue.add(sources.into_iter().map(NewItem::user).collect(), at).1
            }
            QueueOp::Remove(ids) => {
                self.queue.remove(&ids);
                Decision::Nothing
            }
            QueueOp::Move { ids, to_index } => {
                self.queue.move_items(&ids, to_index);
                Decision::Nothing
            }
            QueueOp::Clear { keep_current } => {
                self.queue.clear(keep_current);
                if !keep_current {
                    self.stop();
                }
                Decision::Nothing
            }
            QueueOp::Play(id) => self.queue.select(&id),
            QueueOp::Next => self.queue.go_next(),
            QueueOp::Previous => self.queue.go_previous(position),
            QueueOp::Repeat(repeat) => {
                self.queue.set_repeat(repeat);
                Decision::Nothing
            }
            QueueOp::Shuffle(on) => {
                self.queue.set_shuffle(on);
                Decision::Nothing
            }
            QueueOp::StopAfterCurrent(on) => {
                self.queue.set_stop_after_current(on);
                Decision::Nothing
            }
        };
        self.apply_decision(decision, LoadOpts::default());
    }

    pub(super) fn apply_decision(&mut self, decision: Decision, opts: LoadOpts) {
        match decision {
            Decision::Load(item) => self.load(item, opts),
            Decision::Restart => {
                self.run_command(&["seek", "0", "absolute+exact"]);
                self.set_property("pause", Value::Flag(false));
            }
            Decision::Hold => {
                let effects = self.mirror.hold_at_end();
                self.apply(effects);
            }
            Decision::Stop => self.run_command(&["stop"]),
            Decision::Quit => {
                self.save_progress();
                if let Some(app) = self.app.get() {
                    app.exit(0);
                }
            }
            Decision::GiveUp(failed) => {
                self.run_command(&["stop"]);
                let text = format!(
                    "Stopped after {} files in a row could not be played: {}",
                    failed.len(),
                    failed.join(", ")
                );
                let osd = OsdEvent::new(OsdKind::Error, text).grouped(QUEUE_ERRORS);
                self.emit(events::OSD_SHOW, &osd);
            }
            Decision::Nothing => {}
        }
    }

    // ── Loading ─────────────────────────────────────────────────────────────

    /// Starts loading `item`: identity and history are looked up off-thread, then
    /// `on_prepared` issues the actual `loadfile`.
    pub(super) fn load(&mut self, item: QueueItem, opts: LoadOpts) {
        self.save_progress();
        self.generation += 1;
        self.current = None;
        if !item.is_url && !Path::new(&item.source).is_file() {
            let effects = self.mirror.fail("file not found");
            self.apply_failure(effects);
            return;
        }
        let effects = self.mirror.begin_loading();
        self.apply(effects);

        let generation = self.generation;
        let source = item.source.clone();
        let db = if self.history_enabled() { self.db.clone() } else { None };
        let tx = self.tx.clone();
        self.pending = Some(Pending { generation, item, opts });
        let spawned = std::thread::Builder::new().name("prepare".into()).spawn(move || {
            let media_key = match crate::media::identity::media_key(&source) {
                Ok(key) => Some(key),
                Err(err) => {
                    tracing::debug!("no media key for this file: {err}");
                    None
                }
            };
            let history = match (&db, &media_key) {
                (Some(db), Some(key)) => {
                    let key = key.clone();
                    db.call(move |c| history::get(c, &key)).unwrap_or_else(|err| {
                        tracing::warn!("history lookup failed: {err}");
                        None
                    })
                }
                _ => None,
            };
            let _ =
                tx.send(CoreMsg::Prepared(Box::new(Prepared { generation, media_key, history })));
        });
        if let Err(err) = spawned {
            tracing::warn!("could not start the prepare thread ({err}); loading without history");
            self.on_prepared(Box::new(Prepared { generation, media_key: None, history: None }));
        }
    }

    pub(super) fn on_prepared(&mut self, prepared: Box<Prepared>) {
        // A newer load superseded this one.
        let Some(pending) = self.pending.take_if(|p| p.generation == prepared.generation) else {
            return;
        };
        let Pending { item, opts, .. } = pending;
        let playback = &self.config.playback;
        let mut paused = opts.paused.unwrap_or(!playback.autoplay);
        let mut start = opts.start.clone();
        let mut resumed_at = None;
        let mut offer_resume = None;
        if start.is_none() && !opts.no_resume && playback.resume != ResumeMode::Never {
            let point = prepared
                .history
                .as_ref()
                .and_then(|h| history::resume_point(h, item.duration, playback.finished_percent));
            match (point, playback.resume) {
                (Some(at), ResumeMode::Always) => {
                    start = Some(format!("{at}"));
                    resumed_at = Some(at);
                }
                (Some(at), ResumeMode::Ask) => {
                    paused = true;
                    offer_resume = Some(at);
                }
                _ => {}
            }
        }

        self.set_property("pause", Value::Flag(paused));
        match &start {
            // loadfile <url> <flags> <index> <options>
            Some(start) => {
                let options = format!("start={start}");
                self.run_command(&["loadfile", &item.source, "replace", "-1", &options]);
            }
            None => self.run_command(&["loadfile", &item.source, "replace"]),
        }

        if let (Some(db), Some(key)) =
            (self.db.as_ref().filter(|_| self.config.library.track_history), &prepared.media_key)
        {
            let (key, source, title) = (key.clone(), item.source.clone(), item.title.clone());
            db.spawn("record open", move |c| history::record_open(c, &key, &source, &title, None));
        }
        self.current = Some(Current {
            item_id: item.id,
            media_key: prepared.media_key,
            history: prepared.history,
            resumed_at,
            offer_resume,
        });
        self.last_position_save = Instant::now();
    }

    /// The file is open: update the queue item, restore per-file memory, say where we resumed.
    pub(super) fn on_file_loaded(&mut self) {
        let Some(current) = self.current.clone() else { return };
        // Another item was chosen while this one was opening.
        if self.queue.current().map(|i| i.id.as_str()) != Some(current.item_id.as_str()) {
            return;
        }
        let title = self.mirror.state.media.as_ref().map(|m| m.title.clone());
        let duration = self.mpv.get_f64("duration");
        self.queue.on_loaded(title.as_deref(), duration, current.media_key.as_deref());

        if self.config.playback.remember_per_file {
            if let Some(entry) = &current.history {
                self.restore_memory(entry);
            }
        }
        if let Some(at) = current.resumed_at {
            let mut osd = OsdEvent::new(OsdKind::Info, format!("Resumed at {}", format_clock(at)));
            osd.action = Some(OsdAction {
                label: "Start over".into(),
                action_id: "player.seek_start".into(),
                args: None,
            });
            self.emit(events::OSD_SHOW, &osd);
        }
        if let Some(at) = current.offer_resume {
            let mut osd =
                OsdEvent::new(OsdKind::Info, format!("You stopped at {}", format_clock(at)));
            osd.action = Some(OsdAction {
                label: "Resume".into(),
                action_id: "player.seek_absolute".into(),
                args: Some(serde_json::json!({ "seconds": at })),
            });
            self.emit(events::OSD_SHOW, &osd);
        }
    }

    /// Re-selects remembered tracks and sync. Track ids differ between files, so tracks are
    /// matched by language and title first and by id only as a last resort.
    fn restore_memory(&mut self, entry: &HistoryEntry) {
        let tracks = self
            .mpv
            .get_node("track-list")
            .map(|n| super::tracks::parse_tracks(&n))
            .unwrap_or_default();
        if let Some(memory) = &entry.audio_track {
            if let Some(id) = match_track(&tracks, TrackKind::Audio, memory) {
                self.set_property("aid", Value::Int(id));
            }
        }
        if let Some(memory) = &entry.sub_track {
            if memory.off {
                self.set_property("sid", Value::Text("no".into()));
            } else if let Some(id) = match_track(&tracks, TrackKind::Sub, memory) {
                self.set_property("sid", Value::Int(id));
            }
        }
        if let Some(ms) = entry.sub_delay_ms.filter(|ms| *ms != 0) {
            self.set_property("sub-delay", Value::Double(f64::from(ms) / 1000.0));
        }
        if let Some(ms) = entry.audio_delay_ms.filter(|ms| *ms != 0) {
            self.set_property("audio-delay", Value::Double(f64::from(ms) / 1000.0));
        }
    }

    /// A failure seen by the mirror (mpv could not open the file, or it was missing).
    pub(super) fn apply_failure(&mut self, effects: crate::player::state::Effects) {
        let failed = effects.failed.clone();
        self.apply(effects);
        if let Some(message) = failed {
            self.current = None;
            // Name the queue's item: the mirrored media may still be the previous file.
            let title = self.queue.current().map(|item| item.title.clone());
            let decision = self.queue.on_error(&message, self.config.playback.at_queue_end);
            // Skips share one toast with the 3-strike summary, so a run of bad files ends
            // as a single notice instead of a stack.
            if !matches!(decision, Decision::GiveUp(_)) {
                let text = match &title {
                    Some(name) if !name.is_empty() => format!("Can't play {name}: {message}"),
                    _ => format!("Can't play this file: {message}"),
                };
                let osd = OsdEvent::new(OsdKind::Error, text).grouped(QUEUE_ERRORS);
                self.emit(events::OSD_SHOW, &osd);
            }
            self.apply_decision(decision, LoadOpts::default());
        }
    }

    /// The file played to its end.
    pub(super) fn on_ended(&mut self) {
        if let (Some(db), Some(key)) =
            (self.history_db(), self.current.as_ref().and_then(|c| c.media_key.clone()))
        {
            db.spawn("mark finished", move |c| history::mark_finished(c, &key));
        }
        let decision = match self.sleep_at_end_of_file() {
            Some(decision) => decision,
            None => self.queue.on_ended(self.config.playback.at_queue_end),
        };
        self.apply_decision(decision, LoadOpts::default());
    }

    pub(super) fn stop(&mut self) {
        self.save_progress();
        self.pending = None;
        self.current = None;
        self.run_command(&["stop"]);
        let effects = self.mirror.clear_error();
        self.apply(effects);
    }

    /// `None` toggles. Play on a finished file starts it over; play while stopped reloads the
    /// current queue item.
    pub(super) fn set_paused(&mut self, paused: Option<bool>) {
        let inputs = self.mirror.inputs();
        let paused = paused.unwrap_or(!inputs.pause);
        if !paused && inputs.idle && self.pending.is_none() {
            if let Some(item) = self.queue.current().cloned() {
                self.load(item, LoadOpts { paused: Some(false), ..LoadOpts::default() });
                return;
            }
        }
        if !paused && inputs.eof {
            self.run_command(&["seek", "0", "absolute+exact"]);
        }
        self.set_property("pause", Value::Flag(paused));
    }

    // ── Saving ──────────────────────────────────────────────────────────────

    fn history_db(&self) -> Option<std::sync::Arc<crate::library::db::Db>> {
        self.db.clone().filter(|_| self.config.library.track_history)
    }

    /// Saves the resume position and per-file memory of the loaded file (§6.5).
    pub(super) fn save_progress(&mut self) {
        let Some(db) = self.history_db() else { return };
        let Some(key) = self.current.as_ref().and_then(|c| c.media_key.clone()) else { return };
        if self.mirror.state.media.is_none() {
            return;
        }
        self.last_position_save = Instant::now();
        let position = self.mirror.time.pos;
        let duration = (self.mirror.time.dur > 0.0).then_some(self.mirror.time.dur);
        let finished_percent = self.config.playback.finished_percent;
        let memory = self.config.playback.remember_per_file.then(|| {
            let state = &self.mirror.state;
            (
                remember(&state.tracks, state.selected.audio, false),
                remember(&state.tracks, state.selected.sub, true),
                state.sub_delay_ms,
                state.audio_delay_ms,
            )
        });
        db.spawn("save position", move |c| {
            history::save_position(c, &key, position, duration, finished_percent)?;
            if let Some((audio, sub, sub_delay, audio_delay)) = memory {
                history::save_memory(
                    c,
                    &key,
                    audio.as_ref(),
                    sub.as_ref(),
                    sub_delay,
                    audio_delay,
                )?;
            }
            Ok(())
        });
    }

    /// Called every loop: periodic position saves, queue events, debounced session writes.
    pub(super) fn tick(&mut self) {
        let now = Instant::now();
        self.tick_audio(now);
        self.tick_sleep(now);
        self.tick_inspector(now);
        if self.mirror.state.status == Status::Playing
            && now.duration_since(self.last_position_save) >= POSITION_SAVE_EVERY
        {
            self.save_progress();
        }
        let state = self.queue.state();
        if state.rev != self.queue_rev {
            self.queue_rev = state.rev;
            if let Ok(mut snapshot) = self.queue_snapshot.write() {
                *snapshot = state.clone();
            }
            self.emit(events::QUEUE_CHANGED, &state);
            self.session_due.get_or_insert(now + SESSION_DEBOUNCE);
        }
        if self.session_due.is_some_and(|due| now >= due) {
            self.session_due = None;
            self.write_session();
        }
    }

    pub(super) fn set_ui(&mut self, patch: UiPatch) {
        let changed = match self.ui.write() {
            Ok(mut ui) => ui.apply(patch),
            Err(_) => false,
        };
        if changed {
            self.session_due.get_or_insert(Instant::now() + SESSION_DEBOUNCE);
        }
    }

    pub(super) fn write_session(&self) {
        let Some(path) = &self.session_path else { return };
        let state = self.queue.state();
        let session = Session {
            schema: session::SCHEMA,
            saved_at: session::now_rfc3339(),
            queue: SavedQueue {
                items: state.items.iter().map(SavedItem::from_item).collect(),
                current_id: state.current_id.clone(),
                repeat: state.repeat,
                shuffle: state.shuffle,
            },
            position: if self.mirror.state.media.is_some() { self.mirror.time.pos } else { 0.0 },
            paused: true,
            window_mode: WindowMode::Normal,
            ui: self.ui.read().map(|ui| ui.clone()).unwrap_or_default(),
        };
        if let Err(err) = session::save(path, &session) {
            tracing::warn!("could not save the session: {err}");
        }
    }

    /// Puts back the queue from last time and loads its current item, paused, where it was.
    pub(super) fn restore_session(&mut self, saved: Session) {
        let items: Vec<QueueItem> =
            saved.queue.items.into_iter().map(SavedItem::into_item).collect();
        self.queue.restore(items, saved.queue.current_id, saved.queue.repeat, saved.queue.shuffle);
        if let Ok(mut ui) = self.ui.write() {
            *ui = saved.ui;
        }
        if let Some(item) = self.queue.current().cloned() {
            let start = (saved.position > 0.0).then(|| format!("{}", saved.position));
            self.load(item, LoadOpts { start, paused: Some(true), no_resume: true });
        }
    }
}

/// `m:ss` or `h:mm:ss`, as the UI shows times (DESIGN_SYSTEM §7.4).
pub(super) fn format_clock(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 { seconds as u64 } else { 0 };
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// What to remember about a selected track (`None` id = nothing selected).
fn remember(tracks: &[Track], id: Option<i64>, allow_off: bool) -> Option<TrackMemory> {
    match id {
        None if allow_off => Some(TrackMemory { off: true, lang: None, title: None, id: None }),
        None => None,
        Some(id) => tracks.iter().find(|t| t.id == id).map(|t| TrackMemory {
            off: false,
            lang: t.lang.clone(),
            title: t.title.clone(),
            id: Some(t.id),
        }),
    }
}

/// The best track for a memory: same language and title, else same language, else same id.
pub(super) fn match_track(tracks: &[Track], kind: TrackKind, memory: &TrackMemory) -> Option<i64> {
    let of_kind: Vec<&Track> = tracks.iter().filter(|t| t.kind == kind).collect();
    let same_lang = |t: &&&Track| memory.lang.is_some() && t.lang == memory.lang;
    of_kind
        .iter()
        .find(|t| same_lang(t) && t.title == memory.title)
        .or_else(|| of_kind.iter().find(|t| same_lang(t)))
        .or_else(|| of_kind.iter().find(|t| Some(t.id) == memory.id))
        .map(|t| t.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: i64, kind: TrackKind, lang: Option<&str>, title: Option<&str>) -> Track {
        Track {
            id,
            kind,
            title: title.map(str::to_string),
            lang: lang.map(str::to_string),
            codec: "x".into(),
            default: false,
            forced: false,
            external: false,
            external_path: None,
            width: None,
            height: None,
            fps: None,
            hdr: None,
            image: false,
            channels: None,
            sample_rate: None,
            bitrate: None,
            text_based: None,
        }
    }

    #[test]
    fn remembered_tracks_are_matched_by_language_and_title_before_id() {
        let tracks = vec![
            track(1, TrackKind::Audio, Some("eng"), Some("Commentary")),
            track(2, TrackKind::Audio, Some("jpn"), Some("Stereo")),
            track(3, TrackKind::Audio, Some("eng"), Some("5.1")),
        ];
        let memory = |lang: Option<&str>, title: Option<&str>, id: i64| TrackMemory {
            off: false,
            lang: lang.map(str::to_string),
            title: title.map(str::to_string),
            id: Some(id),
        };
        assert_eq!(
            match_track(&tracks, TrackKind::Audio, &memory(Some("eng"), Some("5.1"), 1)),
            Some(3)
        );
        assert_eq!(
            match_track(&tracks, TrackKind::Audio, &memory(Some("jpn"), Some("other"), 1)),
            Some(2)
        );
        assert_eq!(match_track(&tracks, TrackKind::Audio, &memory(None, None, 3)), Some(3));
        assert_eq!(match_track(&tracks, TrackKind::Audio, &memory(Some("fre"), None, 9)), None);
        assert_eq!(match_track(&tracks, TrackKind::Sub, &memory(Some("eng"), None, 1)), None);
    }

    #[test]
    fn clock_format() {
        assert_eq!(format_clock(754.9), "12:34");
        assert_eq!(format_clock(4354.0), "1:12:34");
        assert_eq!(format_clock(-1.0), "0:00");
    }

    #[test]
    fn remembering_a_selection() {
        let tracks = vec![track(2, TrackKind::Sub, Some("eng"), Some("Full"))];
        let sub = remember(&tracks, Some(2), true).unwrap();
        assert_eq!(sub.lang.as_deref(), Some("eng"));
        assert!(remember(&tracks, None, true).unwrap().off);
        assert!(remember(&tracks, None, false).is_none());
    }
}
