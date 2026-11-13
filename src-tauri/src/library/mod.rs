//! The library (ARCHITECTURE §8.2, §9.6): the database, playback history, the scanned index
//! of the user's folders, playlists, and stream lists.
//!
//! [`Library`] is the service: one low-priority `scan` thread runs folder scans one at a
//! time, then fills in media keys and video durations for whatever still lacks them. Folders
//! are watched, so a file added, renamed, or deleted shows up without a manual rescan.

pub mod db;
pub mod history;
pub mod index;
pub mod iptv;
pub mod music;
pub mod playlists;
pub mod scan;
pub mod sub_links;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::config::store::ConfigStore;
use crate::error::{AppError, AppResult};
use db::Db;
use index::{FolderKind, LibraryFolder};
use scan::{ScanProgress, ScanSettings};

/// A burst of file events (a copy, an unpack) settles for this long before the rescan.
const WATCH_DEBOUNCE: Duration = Duration::from_secs(2);
/// Files filled per pass before the worker looks for a new scan job.
const FILL_BATCH: u32 = 25;

enum Job {
    Scan(i64),
    Stop,
}

type ScanListener = Box<dyn Fn(&ScanProgress) + Send + Sync>;

struct Shared {
    db: Arc<Db>,
    config: Arc<ConfigStore>,
    cancel: AtomicBool,
    /// Receives `library://scan` progress; a callback rather than an `AppHandle` (D32).
    on_scan: OnceLock<ScanListener>,
}

pub struct Library {
    shared: Arc<Shared>,
    jobs: Sender<Job>,
    watchers: Mutex<HashMap<i64, RecommendedWatcher>>,
}

impl Library {
    /// Starts the scan thread. Nothing is scanned until asked (`rescan`, `start`).
    pub fn new(db: Arc<Db>, config: Arc<ConfigStore>) -> std::io::Result<Arc<Self>> {
        let shared = Arc::new(Shared {
            db,
            config,
            cancel: AtomicBool::new(false),
            on_scan: OnceLock::new(),
        });
        let (jobs, rx) = mpsc::channel();
        let worker = shared.clone();
        std::thread::Builder::new().name("scan".into()).spawn(move || run(&worker, &rx))?;
        Ok(Arc::new(Self { shared, jobs, watchers: Mutex::new(HashMap::new()) }))
    }

    pub fn on_scan(&self, listener: impl Fn(&ScanProgress) + Send + Sync + 'static) {
        let _ = self.shared.on_scan.set(Box::new(listener));
    }

    /// Startup: watch every indexed folder and, with `library.scan_on_startup`, scan them.
    pub fn start(&self) {
        let config = self.shared.config.get();
        let folders = self.folders().unwrap_or_default();
        for folder in folders.iter().filter(|f| f.indexed) {
            self.watch(folder);
            if config.library.scan_on_startup {
                let _ = self.jobs.send(Job::Scan(folder.id));
            }
        }
    }

    pub fn folders(&self) -> AppResult<Vec<LibraryFolder>> {
        Ok(self.shared.db.call(|c| index::folders(c))?)
    }

    /// Adds a folder. Indexed folders are scanned at once and watched; a pinned-only folder
    /// is a shortcut that is browsed live.
    pub fn add_folder(
        &self,
        path: &str,
        kind: FolderKind,
        pinned: bool,
        indexed: bool,
    ) -> AppResult<i64> {
        let dir = std::path::absolute(path).unwrap_or_else(|_| path.into());
        if !dir.is_dir() {
            return Err(AppError::not_found(format!("{path} is not a folder")));
        }
        let text = dir.display().to_string();
        let id =
            self.shared.db.call(move |c| index::add_folder(c, &text, kind, pinned, indexed))?;
        let folder = self.shared.db.call(move |c| index::folder(c, id))?;
        if let Some(folder) = folder.filter(|f| f.indexed) {
            self.watch(&folder);
            let _ = self.jobs.send(Job::Scan(id));
        }
        Ok(id)
    }

    pub fn remove_folder(&self, id: i64) -> AppResult<()> {
        if let Ok(mut watchers) = self.watchers.lock() {
            watchers.remove(&id);
        }
        Ok(self.shared.db.call(move |c| index::remove_folder(c, id))?)
    }

    /// Queues a scan of one folder, or of every indexed folder.
    pub fn rescan(&self, folder_id: Option<i64>) -> AppResult<()> {
        let folders = self.folders()?;
        for folder in folders.iter().filter(|f| f.indexed) {
            if folder_id.is_none_or(|id| id == folder.id) {
                let _ = self.jobs.send(Job::Scan(folder.id));
            }
        }
        Ok(())
    }

    fn watch(&self, folder: &LibraryFolder) {
        if !self.shared.config.get().library.watch_folders {
            return;
        }
        let Ok(mut watchers) = self.watchers.lock() else { return };
        if watchers.contains_key(&folder.id) {
            return;
        }
        match watch_tree(Path::new(&folder.path), folder.id, self.jobs.clone()) {
            Ok(watcher) => {
                watchers.insert(folder.id, watcher);
            }
            Err(err) => tracing::warn!("a library folder will not be watched: {err}"),
        }
    }

    /// Stops scanning and filling (shutdown).
    pub fn shutdown(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
        let _ = self.jobs.send(Job::Stop);
        if let Ok(mut watchers) = self.watchers.lock() {
            watchers.clear();
        }
    }
}

fn settings(shared: &Shared) -> ScanSettings {
    let library = &shared.config.get().library;
    ScanSettings {
        extra_extensions: library.extra_extensions.clone(),
        ignore_patterns: library.ignore_patterns.clone(),
    }
}

fn scan(shared: &Shared, folder_id: i64) {
    let folder = match shared.db.call(move |c| index::folder(c, folder_id)) {
        Ok(Some(folder)) if folder.indexed => folder,
        Ok(_) => return,
        Err(err) => {
            tracing::warn!("library: folder lookup failed: {err}");
            return;
        }
    };
    let started = std::time::Instant::now();
    let result = scan::scan_folder(&shared.db, &folder, &settings(shared), &shared.cancel, |p| {
        if let Some(listener) = shared.on_scan.get() {
            listener(&p);
        }
    });
    match result {
        Ok(summary) => tracing::info!(
            found = summary.found,
            changed = summary.added_or_changed,
            removed = summary.removed,
            "library folder scanned in {:?}",
            started.elapsed()
        ),
        Err(err) => tracing::warn!("library scan failed: {err}"),
    }
}

/// Fills in media keys (two small reads per file) and video durations (a headless open) for
/// a few files. Returns whether anything was left to do.
fn fill_some(shared: &Shared, probe: &mut Option<crate::player::headless::DurationProbe>) -> bool {
    let waiting = match shared.db.call(|c| index::unfilled(c, FILL_BATCH)) {
        Ok(waiting) => waiting,
        Err(_) => return false,
    };
    if waiting.is_empty() {
        return false;
    }
    for (id, path, kind, has_key) in waiting {
        if shared.cancel.load(Ordering::Relaxed) {
            return false;
        }
        // A key that cannot be computed (file gone, unreadable) is recorded as empty so the
        // file is not retried on every pass; the next scan clears it if the file changes.
        let key = (!has_key).then(|| crate::media::identity::media_key(&path).unwrap_or_default());
        let duration = (kind == "video").then(|| {
            if probe.is_none() {
                *probe = crate::player::headless::DurationProbe::new().ok();
            }
            // −1: asked, not known.
            probe.as_ref().and_then(|p| p.duration(&path)).unwrap_or(-1.0)
        });
        let result = shared.db.call(move |c| index::fill(c, id, key.as_deref(), duration));
        if let Err(err) = result {
            tracing::debug!("library: could not store a file's details: {err}");
        }
    }
    true
}

fn run(shared: &Shared, jobs: &Receiver<Job>) {
    let mut probe = None;
    loop {
        // Scans first; when none is waiting, fill a few files and look again.
        let job = match jobs.try_recv() {
            Ok(job) => job,
            Err(TryRecvError::Disconnected) => return,
            Err(TryRecvError::Empty) => {
                if fill_some(shared, &mut probe) {
                    continue;
                }
                // Idle: the probe's mpv handle is not kept around.
                probe = None;
                match jobs.recv() {
                    Ok(job) => job,
                    Err(_) => return,
                }
            }
        };
        match job {
            Job::Stop => return,
            Job::Scan(id) => scan(shared, id),
        }
        if shared.cancel.load(Ordering::Relaxed) {
            return;
        }
    }
}

/// Watches a folder tree and queues a rescan once changes have settled.
fn watch_tree(dir: &Path, folder_id: i64, jobs: Sender<Job>) -> notify::Result<RecommendedWatcher> {
    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        if result.is_ok_and(|event| !event.kind.is_access()) {
            let _ = tx.send(());
        }
    })?;
    watcher.watch(dir, RecursiveMode::Recursive)?;
    let spawned = std::thread::Builder::new().name("watch-library".into()).spawn(move || {
        while rx.recv().is_ok() {
            loop {
                match rx.recv_timeout(WATCH_DEBOUNCE) {
                    Ok(()) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            if jobs.send(Job::Scan(folder_id)).is_err() {
                return;
            }
        }
    });
    if let Err(err) = spawned {
        tracing::warn!("could not start a library watcher thread: {err}");
    }
    Ok(watcher)
}
