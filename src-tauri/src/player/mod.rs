//! The player core: owns the primary mpv handle and is the only path by which playback state
//! changes (ARCHITECTURE §4.1, §6).
//!
//! Threads: `mpv-events` blocks in `mpv_wait_event` and forwards events; `core` is a
//! single-threaded actor that folds those events into [`state::Mirror`], applies playback
//! policy, and emits `player://patch` / `player://time`. IPC handlers reach mpv through
//! [`Player`]: simple property writes go straight to mpv (asynchronously) and come back as
//! events; anything that needs policy goes through the core.

mod flow;
pub mod hwdec;
pub mod mpv;
pub mod observe;
pub mod options;
pub mod state;
pub mod tracks;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use crate::config::Config;
use crate::ipc::dto::{
    AbLoop, Direction, OpenRequest, OsdEvent, OsdKind, PlayerState, SeekKind, SeekRequest, Status,
    TimeEvent, TrackSlot,
};
use crate::ipc::events;
use crate::library::db::Db;
use crate::platform::media::{MediaCommand, MediaSession, NowPlaying};
use crate::platform::power::{self, Inhibitor};
use crate::queue::session::{Session, UiPatch, UiState};
use crate::queue::{Queue, QueueState};
use crate::video::VideoSurface;
pub use flow::QueueOp;
use flow::{Current, Pending, Prepared};
use mpv::{Event, Mpv, MpvError, Value};
use state::{Effects, Mirror};

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error(transparent)]
    Mpv(#[from] MpvError),
    #[error(transparent)]
    Video(#[from] crate::video::VideoError),
    #[error("could not start the {0} thread: {1}")]
    Thread(&'static str, std::io::Error),
}

/// `player://patch` is coalesced to at most this rate (§7.4).
const PATCH_INTERVAL: Duration = Duration::from_millis(33);
/// `player://time` cadence while playing (§7.4).
const TIME_INTERVAL: Duration = Duration::from_millis(200);
/// How long the core sleeps when nothing is pending.
const IDLE_TICK: Duration = Duration::from_millis(500);

/// Messages consumed by the core actor.
enum CoreMsg {
    Mpv(Event),
    /// The request, plus an mpv `start=` value that overrides `request.start_at` (the CLI
    /// accepts percentages, which `OpenRequest` cannot express).
    Open(OpenRequest, Option<String>),
    SetPaused(Option<bool>),
    Stop,
    /// Re-emit the current time (a frontend just took a snapshot).
    Resync,
    Media(MediaCommand),
    /// The app and window exist: start media controls. Carries the window handle.
    Attached(Option<isize>),
    ConfigChanged(Arc<Config>),
    Queue(QueueOp),
    /// The off-thread part of a load finished (identity + history).
    Prepared(Box<Prepared>),
    Ui(UiPatch),
    Restore(Box<Session>),
    Shutdown,
}

/// Live decoder statistics for the development overlay (replaced by the stream inspector in
/// Phase 5).
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DebugStats {
    pub hwdec: String,
    pub vo: String,
    pub video_codec: String,
    #[ts(type = "number")]
    pub width: i64,
    #[ts(type = "number")]
    pub height: i64,
    pub fps: f64,
    #[ts(type = "number")]
    pub dropped_frames: i64,
    #[ts(type = "number")]
    pub decoder_dropped_frames: i64,
    pub path: String,
}

#[derive(Debug, Clone, Default)]
pub struct Versions {
    pub mpv: String,
    pub ffmpeg: String,
}

/// The handle IPC code talks to.
pub struct Player {
    mpv: Mutex<Option<Arc<Mpv>>>,
    surface: Arc<dyn VideoSurface>,
    core_tx: Sender<CoreMsg>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    stopping: Arc<AtomicBool>,
    hwdec_chain: String,
    versions: OnceLock<Versions>,
    /// Set once the Tauri app exists; until then the core has nobody to emit to.
    app: Arc<OnceLock<AppHandle>>,
    /// The latest `PlayerState`, shared with the core for `player_snapshot`.
    snapshot: Arc<RwLock<PlayerState>>,
    max_volume: Arc<RwLock<f64>>,
    queue_snapshot: Arc<RwLock<QueueState>>,
    ui: Arc<RwLock<UiState>>,
}

/// The screenshot folder used when `general.screenshot_dir` is empty (see
/// `Paths::default_screenshot_dir`). Set once at startup.
static DEFAULT_SCREENSHOT_DIR: OnceLock<String> = OnceLock::new();

impl Player {
    /// Creates the primary mpv handle and starts the threads.
    ///
    /// This runs before the Tauri app and its window exist, so mpv can bring up its video
    /// output and start opening a file while the webview is still starting. Call
    /// [`Player::attach`] once the window is there.
    pub fn start(
        config: Arc<Config>,
        surface: Arc<dyn VideoSurface>,
        default_screenshot_dir: &std::path::Path,
        cli_mpv_opts: &[(String, String)],
        db: Option<Arc<Db>>,
        session_path: Option<PathBuf>,
    ) -> Result<Arc<Self>, PlayerError> {
        let _ = DEFAULT_SCREENSHOT_DIR.set(default_screenshot_dir.display().to_string());
        let caps = surface.capabilities();
        let chain =
            hwdec::hwdec_chain(caps.backend, caps.session, caps.gpu_vendor, &config.video.hwdec);
        let init = options::build(
            &config,
            ytdlp_on_path(),
            surface.init_options(),
            chain.clone(),
            &screenshot_dir(&config),
            cli_mpv_opts,
        );

        let mpv = Mpv::create()?;
        for (name, value) in &init {
            // A missing option downgrades a feature instead of failing startup (§12.3).
            if let Err(err) = mpv.set_option(name, value) {
                tracing::warn!("mpv rejected option {name}={value}: {err}");
            }
        }
        for name in options::OPTIONAL_SCRIPT_SWITCHES {
            if let Err(err) = mpv.set_option(name, "no") {
                tracing::debug!("mpv has no {name} option: {err}");
            }
        }
        mpv.initialize()?;
        mpv.request_log_messages("warn")?;
        observe::observe_all(&mpv)?;
        // Nothing here may wait on mpv's core: it is busy creating the video output, and this
        // is the main thread.
        tracing::info!(backend = caps.backend.as_str(), hwdec = %chain, "player started");

        let mpv = Arc::new(mpv);
        let stopping = Arc::new(AtomicBool::new(false));
        let (core_tx, core_rx) = mpsc::channel();

        let events = {
            let (mpv, tx, stopping) = (mpv.clone(), core_tx.clone(), stopping.clone());
            std::thread::Builder::new()
                .name("mpv-events".into())
                .spawn(move || event_loop(&mpv, &tx, &stopping))
                .map_err(|e| PlayerError::Thread("mpv-events", e))?
        };

        let app = Arc::new(OnceLock::new());
        let mirror = Mirror::new(&config);
        let snapshot = Arc::new(RwLock::new(mirror.state.clone()));
        let max_volume = Arc::new(RwLock::new(f64::from(config.audio.max_volume)));
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x5eed);
        let queue = Queue::new(seed);
        let queue_snapshot = Arc::new(RwLock::new(queue.state()));
        let ui = Arc::new(RwLock::new(UiState::default()));
        let core = {
            let mut core = Core {
                app: app.clone(),
                mpv: mpv.clone(),
                tx: core_tx.clone(),
                config,
                mirror,
                snapshot: snapshot.clone(),
                max_volume: max_volume.clone(),
                emitted: serde_json::Map::new(),
                rev: 0,
                dirty: true,
                time_dirty: false,
                time_now: false,
                last_patch: Instant::now(),
                last_time: Instant::now(),
                started: Instant::now(),
                hwdec_requested: chain != "no",
                software_notice_shown: false,
                inhibitor: Inhibitor::new(),
                media: None,
                queue,
                queue_snapshot: queue_snapshot.clone(),
                queue_rev: u64::MAX,
                db,
                generation: 0,
                pending: None,
                current: None,
                last_position_save: Instant::now(),
                session_path,
                session_due: None,
                ui: ui.clone(),
            };
            std::thread::Builder::new()
                .name("core".into())
                .spawn(move || core.run(&core_rx))
                .map_err(|e| PlayerError::Thread("core", e))?
        };

        Ok(Arc::new(Self {
            mpv: Mutex::new(Some(mpv)),
            surface,
            core_tx,
            threads: Mutex::new(vec![events, core]),
            stopping,
            hwdec_chain: chain,
            versions: OnceLock::new(),
            app,
            snapshot,
            max_volume,
            queue_snapshot,
            ui,
        }))
    }

    /// Connects the player to the app and its main window: events start flowing to the
    /// frontend and the video surface moves into the window. Main thread only.
    pub fn attach(
        &self,
        app: AppHandle,
        window: &tauri::WebviewWindow,
        window_handle: Option<isize>,
    ) -> Result<(), PlayerError> {
        let _ = self.app.set(app);
        if let Some(mpv) = self.mpv() {
            self.surface.bind(window, &mpv)?;
        }
        self.send(CoreMsg::Attached(window_handle));
        Ok(())
    }

    fn mpv(&self) -> Option<Arc<Mpv>> {
        self.mpv.lock().ok().and_then(|guard| guard.clone())
    }

    fn send(&self, message: CoreMsg) {
        if self.core_tx.send(message).is_err() {
            tracing::debug!("the core has stopped; message dropped");
        }
    }

    fn set(&self, name: &str, value: Value) -> Result<(), MpvError> {
        match self.mpv() {
            Some(mpv) => mpv.set_async(name, value),
            None => Ok(()),
        }
    }

    fn command(&self, args: &[&str]) -> Result<(), MpvError> {
        match self.mpv() {
            Some(mpv) => mpv.command_async(0, args),
            None => Ok(()),
        }
    }

    /// mpv and FFmpeg versions. The first call waits for mpv's core, so keep it off the main
    /// thread during startup.
    pub fn versions(&self) -> &Versions {
        self.versions.get_or_init(|| match self.mpv() {
            Some(mpv) => Versions {
                mpv: mpv.get_string("mpv-version").unwrap_or_default(),
                ffmpeg: mpv.get_string("ffmpeg-version").unwrap_or_default(),
            },
            None => Versions::default(),
        })
    }

    pub fn hwdec_chain(&self) -> &str {
        &self.hwdec_chain
    }

    // ── Commands (ARCHITECTURE §7.3) ────────────────────────────────────────

    /// The current state. Also asks the core to re-emit the position, so a frontend that
    /// just mounted has both.
    pub fn snapshot(&self) -> PlayerState {
        self.send(CoreMsg::Resync);
        match self.snapshot.read() {
            Ok(state) => state.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub fn open(&self, request: OpenRequest) {
        self.send(CoreMsg::Open(request, None));
    }

    pub fn queue(&self, op: QueueOp) {
        self.send(CoreMsg::Queue(op));
    }

    pub fn queue_state(&self) -> QueueState {
        match self.queue_snapshot.read() {
            Ok(state) => state.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub fn ui_state(&self) -> UiState {
        self.ui.read().map(|ui| ui.clone()).unwrap_or_default()
    }

    pub fn set_ui(&self, patch: UiPatch) {
        self.send(CoreMsg::Ui(patch));
    }

    /// Puts back last session's queue and loads its current item, paused (§9.4).
    pub fn restore_session(&self, session: Session) {
        self.send(CoreMsg::Restore(Box::new(session)));
    }

    /// Opens what the command line named (first launch or a forwarded second launch).
    pub fn open_cli(
        &self,
        sources: Vec<String>,
        start: Option<crate::cli::StartPosition>,
        paused: bool,
        mode: crate::ipc::dto::OpenMode,
    ) {
        let request =
            OpenRequest { sources, mode, paused: paused.then_some(true), ..OpenRequest::default() };
        self.send(CoreMsg::Open(request, start.map(crate::cli::StartPosition::to_mpv)));
    }

    pub fn set_paused(&self, paused: bool) {
        self.send(CoreMsg::SetPaused(Some(paused)));
    }

    pub fn toggle_pause(&self) {
        self.send(CoreMsg::SetPaused(None));
    }

    pub fn stop(&self) {
        self.send(CoreMsg::Stop);
    }

    pub fn seek(&self, request: SeekRequest) -> Result<(), MpvError> {
        let value = format!("{}", request.value);
        self.command(&["seek", &value, seek_flags(request.kind, request.exact)])
    }

    pub fn frame_step(&self, direction: Direction) -> Result<(), MpvError> {
        match direction {
            Direction::Forward => self.command(&["frame-step"]),
            Direction::Backward => self.command(&["frame-back-step"]),
        }
    }

    pub fn set_speed(&self, speed: f64) -> Result<(), MpvError> {
        self.set("speed", Value::Double(speed.clamp(0.25, 4.0)))
    }

    pub fn set_volume(&self, volume: f64) -> Result<(), MpvError> {
        let max = self.max_volume.read().map(|m| *m).unwrap_or(130.0);
        self.set("volume", Value::Double(volume.clamp(0.0, max)))
    }

    pub fn set_muted(&self, muted: bool) -> Result<(), MpvError> {
        self.set("mute", Value::Flag(muted))
    }

    pub fn set_direction(&self, direction: Direction) -> Result<(), MpvError> {
        let value = match direction {
            Direction::Forward => "forward",
            Direction::Backward => "backward",
        };
        self.set("play-direction", Value::Text(value.to_string()))
    }

    pub fn set_loop_file(&self, enabled: bool) -> Result<(), MpvError> {
        self.set("loop-file", Value::Text(if enabled { "inf" } else { "no" }.to_string()))
    }

    pub fn set_ab_loop(&self, points: AbLoop) -> Result<(), MpvError> {
        let text = |p: Option<f64>| p.map_or_else(|| "no".to_string(), |v| format!("{v}"));
        self.set("ab-loop-a", Value::Text(text(points.a)))?;
        self.set("ab-loop-b", Value::Text(text(points.b)))
    }

    /// Shrinks the picture so a docked panel never covers it (ARCHITECTURE §5.1, D5).
    pub fn set_margins(
        &self,
        left: f64,
        right: f64,
        top: f64,
        bottom: f64,
    ) -> Result<(), MpvError> {
        let clamp = |v: f64| if v.is_finite() { v.clamp(0.0, 0.9) } else { 0.0 };
        self.set("video-margin-ratio-left", Value::Double(clamp(left)))?;
        self.set("video-margin-ratio-right", Value::Double(clamp(right)))?;
        self.set("video-margin-ratio-top", Value::Double(clamp(top)))?;
        self.set("video-margin-ratio-bottom", Value::Double(clamp(bottom)))
    }

    pub fn set_audio_delay(&self, ms: i32) -> Result<(), MpvError> {
        self.set("audio-delay", Value::Double(f64::from(ms) / 1000.0))
    }

    pub fn set_sub_delay(&self, ms: i32) -> Result<(), MpvError> {
        self.set("sub-delay", Value::Double(f64::from(ms) / 1000.0))
    }

    pub fn set_sub_visible(&self, visible: bool) -> Result<(), MpvError> {
        self.set("sub-visibility", Value::Flag(visible))
    }

    pub fn select_track(&self, slot: TrackSlot, id: Option<i64>) -> Result<(), MpvError> {
        let property = match slot {
            TrackSlot::Video => "vid",
            TrackSlot::Audio => "aid",
            TrackSlot::Sub => "sid",
            TrackSlot::Sub2 => "secondary-sid",
        };
        let value = id.map_or_else(|| "no".to_string(), |id| id.to_string());
        self.set(property, Value::Text(value))
    }

    /// Takes a screenshot and returns its path. Waits for mpv to write the file. Fails when
    /// nothing is loaded (mpv would otherwise save a blank picture).
    pub fn screenshot(&self, subtitles: bool) -> Result<String, MpvError> {
        let Some(mpv) = self.mpv() else {
            return Err(MpvError::Api { code: -1, message: "the player is shutting down".into() });
        };
        if mpv.get_bool("idle-active").unwrap_or(true) {
            return Err(MpvError::Api { code: -1, message: "nothing is playing".into() });
        }
        let mode = if subtitles { "subtitles" } else { "video" };
        let result = mpv.command_ret(&["screenshot", mode])?;
        Ok(result.str_field("filename").unwrap_or_default().to_string())
    }

    /// Runs a raw mpv command (backs the `mpv.command` action; ARCHITECTURE §7.3).
    pub fn mpv_command(&self, args: &[&str]) -> Result<(), MpvError> {
        self.command(args)
    }

    /// The effective configuration changed; the core applies what can change live.
    pub fn config_changed(&self, config: Arc<Config>) {
        self.send(CoreMsg::ConfigChanged(config));
    }

    /// Reads an mpv property as text. Development builds only (verification tooling).
    #[cfg(debug_assertions)]
    pub fn debug_property(&self, name: &str) -> Option<String> {
        self.mpv().and_then(|mpv| mpv.get_string(name))
    }

    /// Stops the threads and destroys the mpv handle (ARCHITECTURE §4.4). Idempotent.
    pub fn shutdown(&self) {
        if self.stopping.swap(true, Ordering::SeqCst) {
            return;
        }
        let mpv = self.mpv.lock().ok().and_then(|mut guard| guard.take());
        if let Some(mpv) = &mpv {
            mpv.wakeup();
        }
        let _ = self.core_tx.send(CoreMsg::Shutdown);
        let started = std::time::Instant::now();
        if let Ok(mut threads) = self.threads.lock() {
            for thread in threads.drain(..) {
                let name = thread.thread().name().unwrap_or("?").to_string();
                let _ = thread.join();
                tracing::debug!("shutdown: {name} joined after {:?}", started.elapsed());
            }
        }
        // GPU resources go before the handle that feeds them.
        self.surface.detach();
        drop(mpv);
        tracing::info!("player stopped after {:?}", started.elapsed());
    }
}

fn seek_flags(kind: SeekKind, exact: bool) -> &'static str {
    match (kind, exact) {
        (SeekKind::Absolute, true) => "absolute+exact",
        (SeekKind::Absolute, false) => "absolute+keyframes",
        (SeekKind::Relative, true) => "relative+exact",
        (SeekKind::Relative, false) => "relative+keyframes",
        (SeekKind::Percent, true) => "absolute-percent+exact",
        (SeekKind::Percent, false) => "absolute-percent+keyframes",
    }
}

/// `general.screenshot_dir`, or the default folder when it is empty.
fn screenshot_dir(config: &Config) -> String {
    if !config.general.screenshot_dir.is_empty() {
        return config.general.screenshot_dir.clone();
    }
    DEFAULT_SCREENSHOT_DIR.get().cloned().unwrap_or_default()
}

fn ytdlp_on_path() -> bool {
    let names: &[&str] = if cfg!(windows) { &["yt-dlp.exe"] } else { &["yt-dlp"] };
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| names.iter().any(|name| dir.join(name).is_file()))
    })
}

fn event_loop(mpv: &Mpv, tx: &Sender<CoreMsg>, stopping: &AtomicBool) {
    while !stopping.load(Ordering::SeqCst) {
        match mpv.wait_event(1.0) {
            Event::None => {}
            Event::Shutdown => {
                let _ = tx.send(CoreMsg::Mpv(Event::Shutdown));
                break;
            }
            Event::LogMessage { prefix, level, text } => forward_log(&prefix, &level, &text),
            event => {
                if tx.send(CoreMsg::Mpv(event)).is_err() {
                    break;
                }
            }
        }
    }
}

fn forward_log(prefix: &str, level: &str, text: &str) {
    match level {
        "fatal" | "error" => tracing::error!(target: "mpv", "[{prefix}] {text}"),
        "warn" => tracing::warn!(target: "mpv", "[{prefix}] {text}"),
        "info" => tracing::info!(target: "mpv", "[{prefix}] {text}"),
        _ => tracing::debug!(target: "mpv", "[{prefix}] {text}"),
    }
}

/// The single-threaded state owner.
struct Core {
    app: Arc<OnceLock<AppHandle>>,
    mpv: Arc<Mpv>,
    tx: Sender<CoreMsg>,
    config: Arc<Config>,
    mirror: Mirror,
    snapshot: Arc<RwLock<PlayerState>>,
    max_volume: Arc<RwLock<f64>>,
    /// The state as last sent to the frontend, for computing patches.
    emitted: serde_json::Map<String, serde_json::Value>,
    rev: u64,
    dirty: bool,
    time_dirty: bool,
    time_now: bool,
    last_patch: Instant,
    last_time: Instant,
    started: Instant,
    /// False when the user turned hardware decoding off, so software decoding is expected.
    hwdec_requested: bool,
    software_notice_shown: bool,
    inhibitor: Inhibitor,
    media: Option<MediaSession>,
    queue: Queue,
    queue_snapshot: Arc<RwLock<QueueState>>,
    /// The queue revision last emitted.
    queue_rev: u64,
    db: Option<Arc<Db>>,
    /// Increases with every load; stale off-thread results are ignored.
    generation: u64,
    pending: Option<Pending>,
    current: Option<Current>,
    last_position_save: Instant,
    /// `None` with `--no-session`.
    session_path: Option<PathBuf>,
    session_due: Option<Instant>,
    ui: Arc<RwLock<UiState>>,
}

impl Core {
    fn run(&mut self, rx: &Receiver<CoreMsg>) {
        loop {
            match rx.recv_timeout(self.next_wait()) {
                Ok(CoreMsg::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(message) => self.handle(message),
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.tick();
            self.flush();
        }
        // Last words: where we were, for resume and for the next session.
        self.save_progress();
        self.write_session();
        self.inhibitor.set(power::Wake::None);
    }

    /// How long to block: until the next patch or time event is due, else an idle tick.
    fn next_wait(&self) -> Duration {
        let now = Instant::now();
        let mut wait = IDLE_TICK;
        if let Some(due) = self.session_due {
            wait = wait.min(due.saturating_duration_since(now));
        }
        if self.dirty {
            wait = wait.min((self.last_patch + PATCH_INTERVAL).saturating_duration_since(now));
        }
        if self.time_now {
            wait = Duration::ZERO;
        } else if self.time_dirty {
            wait = wait.min((self.last_time + TIME_INTERVAL).saturating_duration_since(now));
        }
        wait
    }

    fn handle(&mut self, message: CoreMsg) {
        match message {
            CoreMsg::Mpv(event) => self.on_mpv_event(&event),
            CoreMsg::Open(request, start) => self.open(&request, start),
            CoreMsg::SetPaused(paused) => self.set_paused(paused),
            CoreMsg::Stop => self.stop(),
            CoreMsg::Resync => self.time_now = true,
            CoreMsg::Media(command) => self.on_media_command(command),
            CoreMsg::Attached(window_handle) => self.on_attached(window_handle),
            CoreMsg::ConfigChanged(config) => self.on_config_changed(config),
            CoreMsg::Queue(op) => self.queue_op(op),
            CoreMsg::Prepared(prepared) => self.on_prepared(prepared),
            CoreMsg::Ui(patch) => self.set_ui(patch),
            CoreMsg::Restore(session) => self.restore_session(*session),
            CoreMsg::Shutdown => {}
        }
    }

    fn run_command(&self, args: &[&str]) {
        if let Err(err) = self.mpv.command_async(0, args) {
            tracing::warn!("mpv command {args:?} failed: {err}");
        }
    }

    fn set_property(&self, name: &str, value: Value) {
        if let Err(err) = self.mpv.set_async(name, value) {
            tracing::warn!("mpv property {name} could not be set: {err}");
        }
    }

    fn apply(&mut self, effects: Effects) {
        self.dirty |= effects.state;
        self.time_dirty |= effects.time;
        self.time_now |= effects.time_now;
        if effects.loaded {
            self.software_notice_shown = false;
        }
        if effects.ended {
            self.on_ended();
        }
    }

    fn on_mpv_event(&mut self, event: &Event) {
        match event {
            Event::Shutdown => tracing::info!("mpv shut down"),
            Event::AsyncError { id, error } => {
                tracing::warn!(id, "an mpv command or property write failed: {error}");
            }
            Event::EndFile { reason, error } => tracing::debug!(?reason, ?error, "end of file"),
            Event::FileLoaded => tracing::debug!("file loaded"),
            Event::PlaybackRestart => tracing::debug!("playback restart (first frame shown)"),
            _ => {}
        }
        let before_hwdec = self.mirror.state.hwdec.clone();
        let was_paused = self.mirror.inputs().pause;
        let effects = self.mirror.on_event(event);
        if effects.failed.is_some() {
            self.apply_failure(effects);
        } else {
            self.apply(effects);
        }
        if self.mirror.state.hwdec != before_hwdec {
            self.on_hwdec_changed();
        }
        match event {
            Event::FileLoaded => self.on_file_loaded(),
            // Save where we are on pause and after a seek (not during a drag: debounced).
            Event::PlaybackRestart
                if self.last_position_save.elapsed() >= Duration::from_secs(1) =>
            {
                self.save_progress();
            }
            Event::PropertyChange { name, .. }
                if name == "pause" && self.mirror.inputs().pause && !was_paused =>
            {
                self.save_progress();
            }
            _ => {}
        }
    }

    /// The cause of any stutter should never be a mystery (ARCHITECTURE §5.4): say so when a
    /// file ends up on the software decoder although hardware decoding was requested.
    fn on_hwdec_changed(&mut self) {
        match self.mirror.state.hwdec.as_str() {
            "" => {}
            "no" if self.hwdec_requested => {
                let codec = self.mpv.get_string("video-codec").unwrap_or_default();
                tracing::warn!("software decoding: no hardware decoder available for {codec}");
                let height = self.mpv.get_i64("height").unwrap_or(0);
                if height >= 2160 && !self.software_notice_shown {
                    self.software_notice_shown = true;
                    let text =
                        format!("Software decoding — hardware decoder unavailable for {codec}");
                    self.emit(events::OSD_SHOW, &OsdEvent::new(OsdKind::Warn, text));
                }
            }
            "no" => tracing::info!("software decoding (hardware decoding is turned off)"),
            decoder => tracing::info!("hardware decoding with {decoder}"),
        }
    }

    fn on_media_command(&mut self, command: MediaCommand) {
        match command {
            MediaCommand::Play => self.set_paused(Some(false)),
            MediaCommand::Pause => self.set_paused(Some(true)),
            MediaCommand::Toggle => self.set_paused(None),
            MediaCommand::Stop => self.stop(),
            MediaCommand::SeekTo(position) => {
                self.run_command(&["seek", &format!("{position}"), "absolute+exact"]);
            }
            MediaCommand::SeekBy(delta) => {
                self.run_command(&["seek", &format!("{delta}"), "relative+exact"]);
            }
            MediaCommand::Next => self.queue_op(QueueOp::Next),
            MediaCommand::Previous => self.queue_op(QueueOp::Previous),
        }
    }

    fn on_attached(&mut self, window_handle: Option<isize>) {
        // The frontend is about to mount: make sure it gets the position too.
        self.time_now = true;
        if !self.config.general.media_keys || self.media.is_some() {
            return;
        }
        let tx = self.tx.clone();
        match MediaSession::new(window_handle, move |command| {
            let _ = tx.send(CoreMsg::Media(command));
        }) {
            Ok(session) => {
                self.media = Some(session);
                self.dirty = true;
            }
            Err(err) => tracing::warn!("media keys are unavailable: {err}"),
        }
    }

    /// Applies what can change live: every mpv option derived from config whose value differs.
    fn on_config_changed(&mut self, config: Arc<Config>) {
        let old = options::live(&self.config, &screenshot_dir(&self.config));
        let new = options::live(&config, &screenshot_dir(&config));
        for (name, value) in &new {
            let previous = old.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v);
            if previous != Some(value) {
                tracing::debug!("config: mpv {name} = {value}");
                self.set_property(name, Value::Text(value.clone()));
            }
        }
        if let Ok(mut max) = self.max_volume.write() {
            *max = f64::from(config.audio.max_volume);
        }
        self.config = config;
    }

    // ── Output ──────────────────────────────────────────────────────────────

    fn emit<T: Serialize + Clone>(&self, event: &str, payload: &T) {
        if let Some(app) = self.app.get() {
            if let Err(err) = app.emit(event, payload) {
                tracing::debug!("could not emit {event}: {err}");
            }
        }
    }

    /// Sends whatever is due: a coalesced state patch and/or a time event.
    fn flush(&mut self) {
        let now = Instant::now();
        if self.dirty && now.duration_since(self.last_patch) >= PATCH_INTERVAL {
            self.flush_patch();
            self.last_patch = now;
            self.dirty = false;
        }
        let playing = self.mirror.state.status == Status::Playing;
        let due = self.time_now
            || (self.time_dirty && playing && now.duration_since(self.last_time) >= TIME_INTERVAL);
        if due {
            let time = self.mirror.time;
            let event = TimeEvent {
                pos: time.pos,
                dur: time.dur,
                cache_end: time.cache_end,
                at: self.started.elapsed().as_secs_f64() * 1000.0,
            };
            self.emit(events::PLAYER_TIME, &event);
            self.last_time = now;
            self.time_dirty = false;
            self.time_now = false;
            if cfg!(debug_assertions) {
                self.emit(events::DEBUG_STATS, &self.debug_stats());
            }
        }
    }

    /// Emits the top-level keys that changed since the last patch (§7.4), updates the shared
    /// snapshot, and keeps the OS in step (sleep inhibition, media overlay).
    fn flush_patch(&mut self) {
        let state = &self.mirror.state;
        self.inhibitor.set(power::required(state.status, state.media.as_ref().map(|m| m.kind)));
        if let Some(media) = &mut self.media {
            media.update(&NowPlaying {
                title: state.media.as_ref().map(|m| m.title.clone()),
                duration: state.media.as_ref().map(|m| m.duration),
                status: state.status,
                position: self.mirror.time.pos,
            });
        }

        let Ok(serde_json::Value::Object(current)) = serde_json::to_value(state) else { return };
        let mut patch = serde_json::Map::new();
        for (key, value) in &current {
            if key != "rev" && self.emitted.get(key) != Some(value) {
                patch.insert(key.clone(), value.clone());
            }
        }
        if patch.is_empty() {
            return;
        }
        self.rev += 1;
        self.mirror.state.rev = self.rev;
        patch.insert("rev".to_string(), serde_json::Value::from(self.rev));
        self.emitted = current;
        match self.snapshot.write() {
            Ok(mut snapshot) => *snapshot = self.mirror.state.clone(),
            Err(poisoned) => *poisoned.into_inner() = self.mirror.state.clone(),
        }
        self.emit(events::PLAYER_PATCH, &serde_json::Value::Object(patch));
    }

    fn debug_stats(&self) -> DebugStats {
        let mpv = &self.mpv;
        DebugStats {
            hwdec: mpv.get_string("hwdec-current").unwrap_or_default(),
            vo: mpv.get_string("current-vo").unwrap_or_default(),
            video_codec: mpv.get_string("video-codec").unwrap_or_default(),
            width: mpv.get_i64("width").unwrap_or(0),
            height: mpv.get_i64("height").unwrap_or(0),
            fps: mpv.get_f64("container-fps").unwrap_or(0.0),
            dropped_frames: mpv.get_i64("frame-drop-count").unwrap_or(0),
            decoder_dropped_frames: mpv.get_i64("decoder-frame-drop-count").unwrap_or(0),
            path: mpv.get_string("path").unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seek_flags_cover_every_combination() {
        assert_eq!(seek_flags(SeekKind::Absolute, true), "absolute+exact");
        assert_eq!(seek_flags(SeekKind::Relative, false), "relative+keyframes");
        assert_eq!(seek_flags(SeekKind::Percent, true), "absolute-percent+exact");
    }

    #[test]
    fn screenshot_dir_prefers_the_setting() {
        let mut config = crate::config::defaults();
        config.general.screenshot_dir = "D:/Shots".into();
        assert_eq!(screenshot_dir(&config), "D:/Shots");
        config.general.screenshot_dir.clear();
        let _ = DEFAULT_SCREENSHOT_DIR.set("X:/home/screenshots".into());
        assert_eq!(screenshot_dir(&config), "X:/home/screenshots");
    }
}
