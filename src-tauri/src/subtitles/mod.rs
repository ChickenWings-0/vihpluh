//! The subtitle cue index (ARCHITECTURE §11.3): when each line of the selected subtitle track
//! shows, and what it says, for the subtitle timeline. Sources, best first: external text
//! files (parsed whole), text tracks embedded in Matroska files (read from the container
//! off-thread and cached), and for everything else the cues mpv reports as playback reaches
//! them (partial coverage that grows).

pub mod automatch;
pub mod cues;
pub mod mkv;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use ts_rs::TS;

use cues::Cue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Coverage {
    /// Every cue of the track is known.
    Full,
    /// Only the cues playback has passed so far.
    Partial,
    None,
}

/// `sub_get_cues` — the cues that overlap a time window.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CueWindow {
    pub cues: Vec<Cue>,
    pub coverage: Coverage,
    /// False for bitmap subtitles: blocks without text.
    pub text: bool,
}

/// `media://cues` — the index for a track grew or finished.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CuesEvent {
    pub media_key: String,
    #[ts(type = "number")]
    pub track_id: i64,
    pub coverage: Coverage,
}

pub const CUES_EVENT: &str = "media://cues";

/// What the index needs to know about a subtitle track to find its cues.
#[derive(Debug, Clone)]
pub struct TrackSource {
    pub media_key: String,
    /// Path or URL of the media file.
    pub source: String,
    pub is_url: bool,
    pub track_id: i64,
    /// Set for external subtitle files.
    pub external_path: Option<String>,
    pub text_based: bool,
    /// Position among the file's embedded subtitle tracks (0-based), for embedded tracks.
    pub embedded_index: Option<usize>,
}

/// Growth of a progressive index is announced at most this often.
const EMIT_EVERY: Duration = Duration::from_millis(500);

#[derive(Default)]
struct Inner {
    /// Everything below belongs to this file; another file starts over.
    media_key: String,
    full: HashMap<i64, Arc<Vec<Cue>>>,
    partial: HashMap<i64, Vec<Cue>>,
    /// Running container reads, by track.
    jobs: HashMap<i64, Arc<AtomicBool>>,
    /// Tracks whose file could not give a full index; they stay progressive.
    no_full: HashSet<i64>,
    last_emit: Option<Instant>,
}

impl Inner {
    fn enter(&mut self, media_key: &str) {
        if self.media_key == media_key {
            return;
        }
        for cancel in self.jobs.values() {
            cancel.store(true, Ordering::Relaxed);
        }
        *self = Inner { media_key: media_key.to_string(), ..Inner::default() };
    }
}

/// Receives `media://cues` events. A plain callback rather than an `AppHandle`, so this module
/// (and its tests) stay free of the Tauri runtime (D17).
type Notify = Box<dyn Fn(&CuesEvent) + Send + Sync>;

pub struct CueIndex {
    cache: PathBuf,
    notify: OnceLock<Notify>,
    inner: Mutex<Inner>,
}

impl CueIndex {
    pub fn new(cache: PathBuf) -> Arc<Self> {
        Arc::new(Self { cache, notify: OnceLock::new(), inner: Mutex::new(Inner::default()) })
    }

    /// Sets where index changes are announced (once, at startup).
    pub fn on_change(&self, notify: impl Fn(&CuesEvent) + Send + Sync + 'static) {
        let _ = self.notify.set(Box::new(notify));
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn emit(&self, media_key: &str, track_id: i64, coverage: Coverage) {
        if let Some(notify) = self.notify.get() {
            notify(&CuesEvent { media_key: media_key.to_string(), track_id, coverage });
        }
    }

    pub fn cache_path(&self, media_key: &str, track_id: i64) -> PathBuf {
        let key = crate::media::thumbs::key_dir(media_key);
        self.cache.join("cues").join(format!("{key}.s{track_id}.json"))
    }

    /// The cues of `track` overlapping `from..to` seconds. The first call for a track loads
    /// its full index if one can be had; an embedded track's read runs in the background and
    /// is announced with `media://cues` when it finishes.
    pub fn query(self: &Arc<Self>, track: &TrackSource, from: f64, to: f64) -> CueWindow {
        let known = {
            let mut inner = self.lock();
            inner.enter(&track.media_key);
            inner.full.get(&track.track_id).cloned()
        };
        let full = known.or_else(|| self.load_full(track));
        let inner = self.lock();
        match full {
            Some(cues) => CueWindow {
                cues: window(&cues, from, to),
                coverage: Coverage::Full,
                text: track.text_based,
            },
            None => {
                let cues = inner.partial.get(&track.track_id).map(Vec::as_slice).unwrap_or(&[]);
                CueWindow {
                    cues: window(cues, from, to),
                    coverage: if cues.is_empty() { Coverage::None } else { Coverage::Partial },
                    text: track.text_based,
                }
            }
        }
    }

    /// Tries the sources that give a whole index. Returns it when it is available now.
    fn load_full(self: &Arc<Self>, track: &TrackSource) -> Option<Arc<Vec<Cue>>> {
        let id = track.track_id;
        if self.lock().no_full.contains(&id) {
            return None;
        }
        if let Some(path) = &track.external_path {
            let parsed = match cues::parse_file(Path::new(path)) {
                Ok(parsed) => parsed,
                Err(err) => {
                    tracing::debug!("could not read a subtitle file for its cues: {err}");
                    None
                }
            };
            return self.store(&track.media_key, id, parsed);
        }
        let path = Path::new(&track.source);
        let index = track
            .embedded_index
            .filter(|_| track.text_based && !track.is_url && mkv::is_matroska(path));
        let Some(index) = index else {
            self.lock().no_full.insert(id);
            return None;
        };
        let cache_file = self.cache_path(&track.media_key, id);
        if let Some(cached) = std::fs::read(&cache_file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<Cue>>(&bytes).ok())
        {
            return self.store(&track.media_key, id, Some(cached));
        }
        self.start_job(track, index, cache_file);
        None
    }

    /// Records the outcome of a full load: the index, or that this track has none.
    fn store(&self, media_key: &str, id: i64, cues: Option<Vec<Cue>>) -> Option<Arc<Vec<Cue>>> {
        let mut inner = self.lock();
        if inner.media_key != media_key {
            return None;
        }
        match cues {
            Some(cues) => {
                let cues = Arc::new(cues);
                inner.full.insert(id, cues.clone());
                inner.partial.remove(&id);
                Some(cues)
            }
            None => {
                inner.no_full.insert(id);
                None
            }
        }
    }

    fn start_job(self: &Arc<Self>, track: &TrackSource, index: usize, cache_file: PathBuf) {
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut inner = self.lock();
            if inner.jobs.contains_key(&track.track_id) {
                return;
            }
            inner.jobs.insert(track.track_id, cancel.clone());
        }
        let (service, key, id) = (self.clone(), track.media_key.clone(), track.track_id);
        let source = PathBuf::from(&track.source);
        let spawned = std::thread::Builder::new().name("cues".into()).spawn(move || {
            let started = Instant::now();
            let result = mkv::extract_file(&source, index, &cancel);
            service.lock().jobs.remove(&id);
            let cues = match result {
                Ok(cues) => cues,
                Err(mkv::MkvError::Cancelled) => return,
                Err(err) => {
                    tracing::warn!("could not read embedded subtitles: {err}");
                    None
                }
            };
            if let Some(cues) = &cues {
                tracing::debug!(
                    count = cues.len(),
                    "embedded subtitle cues read in {:?}",
                    started.elapsed()
                );
                if let Ok(json) = serde_json::to_vec(cues) {
                    if let Err(err) = crate::media::thumbs::write_atomic(&cache_file, &json) {
                        tracing::debug!("could not cache subtitle cues: {err}");
                    }
                }
            }
            if service.store(&key, id, cues).is_some() {
                service.emit(&key, id, Coverage::Full);
            }
        });
        if let Err(err) = spawned {
            tracing::warn!("could not start the subtitle cue reader: {err}");
            self.lock().jobs.remove(&track.track_id);
        }
    }

    /// A cue mpv is showing (or has prefetched): added to the track's progressive index
    /// unless the whole track is already known.
    pub fn record(&self, media_key: &str, track_id: i64, cue: Cue) {
        if media_key.is_empty()
            || cue.end.partial_cmp(&cue.start) != Some(std::cmp::Ordering::Greater)
            || !cue.start.is_finite()
        {
            return;
        }
        let announce = {
            let mut inner = self.lock();
            inner.enter(media_key);
            if inner.full.contains_key(&track_id) {
                return;
            }
            let list = inner.partial.entry(track_id).or_default();
            if !insert(list, cue) {
                return;
            }
            let now = Instant::now();
            let due = inner.last_emit.is_none_or(|at| now.duration_since(at) >= EMIT_EVERY);
            if due {
                inner.last_emit = Some(now);
            }
            due
        };
        if announce {
            self.emit(media_key, track_id, Coverage::Partial);
        }
    }
}

/// Inserts in start order. A cue with the same start replaces the one already there (mpv
/// reports a line again when its end or text becomes known). Returns whether anything changed.
fn insert(list: &mut Vec<Cue>, cue: Cue) -> bool {
    let at = list.partition_point(|c| c.start < cue.start - 0.0005);
    match list.get_mut(at) {
        Some(existing) if (existing.start - cue.start).abs() <= 0.0005 => {
            if *existing == cue {
                return false;
            }
            *existing = cue;
        }
        _ => list.insert(at, cue),
    }
    true
}

/// The cues overlapping `from..to`, from a list sorted by start.
fn window(cues: &[Cue], from: f64, to: f64) -> Vec<Cue> {
    let end = cues.partition_point(|c| c.start <= to);
    cues[..end].iter().filter(|c| c.end >= from).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start: f64, end: f64, text: &str) -> Cue {
        Cue { start, end, text: Some(text.to_string()) }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vihpluh-cues-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn track(key: &str, id: i64) -> TrackSource {
        TrackSource {
            media_key: key.into(),
            source: "D:/x/file.mp4".into(),
            is_url: false,
            track_id: id,
            external_path: None,
            text_based: true,
            embedded_index: Some(0),
        }
    }

    #[test]
    fn progressive_cues_grow_sorted_and_deduplicated() {
        let index = CueIndex::new(temp_dir("progressive"));
        let t = track("f:1", 1);
        assert_eq!(index.query(&t, 0.0, 100.0).coverage, Coverage::None);
        index.record("f:1", 1, cue(10.0, 12.0, "b"));
        index.record("f:1", 1, cue(1.0, 3.0, "a"));
        index.record("f:1", 1, cue(10.0, 12.0, "b"));
        index.record("f:1", 1, cue(10.0, 12.5, "b, longer"));
        index.record("f:1", 1, cue(5.0, 5.0, "empty"));
        index.record("f:1", 2, cue(50.0, 51.0, "other track"));
        let all = index.query(&t, 0.0, 100.0);
        assert_eq!(all.coverage, Coverage::Partial);
        assert_eq!(all.cues, vec![cue(1.0, 3.0, "a"), cue(10.0, 12.5, "b, longer")]);
        // The window keeps cues that overlap either edge.
        assert_eq!(index.query(&t, 2.5, 9.0).cues, vec![cue(1.0, 3.0, "a")]);
        assert_eq!(index.query(&t, 12.6, 20.0).cues, vec![]);
    }

    #[test]
    fn another_file_starts_over() {
        let index = CueIndex::new(temp_dir("switch"));
        index.record("f:1", 1, cue(1.0, 2.0, "a"));
        assert_eq!(index.query(&track("f:2", 1), 0.0, 10.0).coverage, Coverage::None);
        assert_eq!(index.query(&track("f:1", 1), 0.0, 10.0).coverage, Coverage::None);
    }

    #[test]
    fn external_files_give_full_coverage_at_once() {
        let dir = temp_dir("external");
        let srt = dir.join("a.en.srt");
        std::fs::write(&srt, "1\n00:00:01,000 --> 00:00:02,000\nHello\n").unwrap();
        let index = CueIndex::new(dir.clone());
        let mut t = track("f:1", 3);
        t.external_path = Some(srt.display().to_string());
        t.embedded_index = None;
        let got = index.query(&t, 0.0, 10.0);
        assert_eq!(got.coverage, Coverage::Full);
        assert_eq!(got.cues, vec![cue(1.0, 2.0, "Hello")]);
        // Cues mpv reports later do not disturb a full index.
        index.record("f:1", 3, cue(5.0, 6.0, "late"));
        assert_eq!(index.query(&t, 0.0, 10.0).cues.len(), 1);

        // A format we cannot parse stays progressive.
        let mut bitmap = track("f:1", 4);
        bitmap.external_path = Some(dir.join("a.sup").display().to_string());
        bitmap.text_based = false;
        assert_eq!(index.query(&bitmap, 0.0, 10.0).coverage, Coverage::None);
        index.record("f:1", 4, Cue { start: 1.0, end: 2.0, text: None });
        let got = index.query(&bitmap, 0.0, 10.0);
        assert_eq!((got.coverage, got.text), (Coverage::Partial, false));
    }

    #[test]
    fn embedded_matroska_tracks_are_read_in_the_background_and_cached() {
        let dir = temp_dir("embedded");
        let file = dir.join("two-subs.mkv");
        std::fs::write(&file, include_bytes!("fixtures/two-subs.mkv")).unwrap();
        let index = CueIndex::new(dir.clone());
        let mut t = track("f:9", 2);
        t.source = file.display().to_string();
        t.embedded_index = Some(1);
        let deadline = Instant::now() + Duration::from_secs(10);
        let got = loop {
            let got = index.query(&t, 0.0, 100.0);
            if got.coverage == Coverage::Full || Instant::now() > deadline {
                break got;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(got.coverage, Coverage::Full);
        assert_eq!(got.cues.len(), 2);
        assert!(index.cache_path("f:9", 2).is_file(), "written to cache/cues/");

        // A fresh index (next launch) loads the cached file without touching the container.
        std::fs::remove_file(&file).unwrap();
        let again = CueIndex::new(dir);
        assert_eq!(again.query(&t, 0.0, 100.0).coverage, Coverage::Full);
    }
}
