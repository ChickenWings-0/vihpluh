//! The player core: owns the primary mpv handle and is the only path by which playback state
//! changes (ARCHITECTURE §4.1, §6).
//!
//! Threads: `mpv-events` blocks in `mpv_wait_event` and forwards events; `core` is a
//! single-threaded actor that consumes those events plus commands from IPC handlers.

pub mod hwdec;
pub mod mpv;
pub mod options;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use crate::config::Config;
use crate::video::VideoSurface;
use mpv::{Event, Mpv, MpvError};

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error(transparent)]
    Mpv(#[from] MpvError),
    #[error(transparent)]
    Video(#[from] crate::video::VideoError),
    #[error("could not start the {0} thread: {1}")]
    Thread(&'static str, std::io::Error),
}

/// Messages consumed by the core actor.
#[derive(Debug)]
enum CoreMsg {
    Mpv(Event),
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

pub const EVENT_DEBUG_STATS: &str = "debug://stats";

/// Cheap, cloneable access to the player for IPC handlers.
pub struct Player {
    mpv: Mutex<Option<Arc<Mpv>>>,
    surface: Arc<dyn VideoSurface>,
    core_tx: Sender<CoreMsg>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    stopping: Arc<AtomicBool>,
    hwdec_chain: String,
    versions: Versions,
}

#[derive(Debug, Clone, Default)]
pub struct Versions {
    pub mpv: String,
    pub ffmpeg: String,
}

impl Player {
    /// Creates the primary mpv handle, attaches the video surface, and starts the threads.
    pub fn start(
        app: AppHandle,
        config: &Config,
        surface: Arc<dyn VideoSurface>,
        cli_mpv_opts: &[(String, String)],
    ) -> Result<Arc<Self>, PlayerError> {
        let caps = surface.capabilities();
        let chain =
            hwdec::hwdec_chain(caps.backend, caps.session, caps.gpu_vendor, &config.video.hwdec);
        let init = options::build(
            config,
            ytdlp_on_path(),
            surface.init_options(),
            chain.clone(),
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
        surface.attach(&mpv)?;

        let versions = Versions {
            mpv: mpv.get_string("mpv-version").unwrap_or_default(),
            ffmpeg: mpv.get_string("ffmpeg-version").unwrap_or_default(),
        };
        tracing::info!(
            mpv = %versions.mpv,
            ffmpeg = %versions.ffmpeg,
            backend = caps.backend.as_str(),
            hwdec = %chain,
            "player started"
        );

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
        let core = {
            let mpv = mpv.clone();
            std::thread::Builder::new()
                .name("core".into())
                .spawn(move || Core { app, mpv }.run(&core_rx))
                .map_err(|e| PlayerError::Thread("core", e))?
        };

        Ok(Arc::new(Self {
            mpv: Mutex::new(Some(mpv)),
            surface,
            core_tx,
            threads: Mutex::new(vec![events, core]),
            stopping,
            hwdec_chain: chain,
            versions,
        }))
    }

    fn mpv(&self) -> Option<Arc<Mpv>> {
        self.mpv.lock().ok().and_then(|guard| guard.clone())
    }

    pub fn versions(&self) -> &Versions {
        &self.versions
    }

    pub fn hwdec_chain(&self) -> &str {
        &self.hwdec_chain
    }

    /// Replaces whatever is playing with `source`.
    pub fn open(&self, source: &str, start: Option<&str>, paused: bool) -> Result<(), MpvError> {
        let Some(mpv) = self.mpv() else { return Ok(()) };
        mpv.set_bool("pause", paused)?;
        match start {
            // loadfile <url> <flags> <index> <options>
            Some(start) => mpv.command_async(
                0,
                &["loadfile", source, "replace", "-1", &format!("start={start}")],
            ),
            None => mpv.command_async(0, &["loadfile", source, "replace"]),
        }
    }

    /// Runs a raw mpv command (backs the `mpv.command` action; ARCHITECTURE §7.3).
    pub fn mpv_command(&self, args: &[&str]) -> Result<(), MpvError> {
        match self.mpv() {
            Some(mpv) => mpv.command_async(0, args),
            None => Ok(()),
        }
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
        if let Ok(mut threads) = self.threads.lock() {
            for thread in threads.drain(..) {
                let _ = thread.join();
            }
        }
        // GPU resources go before the handle that feeds them.
        self.surface.detach();
        drop(mpv);
        tracing::info!("player stopped");
    }
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
    app: AppHandle,
    mpv: Arc<Mpv>,
}

impl Core {
    fn run(self, rx: &Receiver<CoreMsg>) {
        let mut last_stats = DebugStats::default();
        loop {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(CoreMsg::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(CoreMsg::Mpv(event)) => self.on_mpv_event(event),
                Err(RecvTimeoutError::Timeout) => {}
            }
            if cfg!(debug_assertions) {
                let stats = self.debug_stats();
                if stats != last_stats {
                    let _ = self.app.emit(EVENT_DEBUG_STATS, &stats);
                    last_stats = stats;
                }
            }
        }
    }

    fn on_mpv_event(&self, event: Event) {
        match event {
            Event::FileLoaded => tracing::info!("file loaded"),
            Event::EndFile { reason, error } => {
                tracing::info!(?reason, ?error, "end of file");
            }
            Event::AsyncError { id, error } => tracing::warn!(id, "mpv command failed: {error}"),
            Event::Shutdown => tracing::info!("mpv shut down"),
            _ => {}
        }
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
