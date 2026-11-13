//! Audio waveforms (ARCHITECTURE §11.2): a one-off job per (media key, audio track) decodes
//! the track through a headless mpv (`ao=pcm` into a pipe, far faster than realtime), reduces
//! it to 50 min/max peak pairs per second, and stores a `.vwf` file in the cache. One job runs
//! at a time; asking for another file, or the player moving to another file, cancels it.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use crate::config::schema::WaveformMode;
use crate::platform::pipe::PcmPipe;
use crate::player::Player;
use crate::player::headless::decode_pcm;

pub const PEAKS_PER_SECOND: u32 = 50;
const SAMPLE_RATE: u32 = 8000;
const SAMPLES_PER_PEAK: usize = (SAMPLE_RATE / PEAKS_PER_SECOND) as usize;
const MAGIC: &[u8; 4] = b"VWF1";
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
pub const EVENT: &str = "media://waveform";

/// A decoded `.vwf` (§11.2): little-endian header, then `peak_count × { i8 min, i8 max }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Vwf {
    pub peaks_per_second: u32,
    pub duration: f32,
    pub peaks: Vec<(i8, i8)>,
}

impl Vwf {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + self.peaks.len() * 2);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.peaks_per_second.to_le_bytes());
        out.extend_from_slice(&(self.peaks.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.duration.to_le_bytes());
        for (min, max) in &self.peaks {
            out.push(min.to_le_bytes()[0]);
            out.push(max.to_le_bytes()[0]);
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 16 || &bytes[..4] != MAGIC {
            return None;
        }
        let word = |at: usize| -> Option<[u8; 4]> { bytes.get(at..at + 4)?.try_into().ok() };
        let peaks_per_second = u32::from_le_bytes(word(4)?);
        let count = u32::from_le_bytes(word(8)?) as usize;
        let duration = f32::from_le_bytes(word(12)?);
        let body = bytes.get(16..16 + count * 2)?;
        let peaks = body
            .chunks_exact(2)
            .map(|p| (i8::from_le_bytes([p[0]]), i8::from_le_bytes([p[1]])))
            .collect();
        Some(Self { peaks_per_second, duration, peaks })
    }
}

/// Folds 8 kHz s16 samples into one (min, max) pair per 160 samples.
#[derive(Debug, Default)]
pub struct PeakReducer {
    min: i16,
    max: i16,
    count: usize,
    carry: Option<u8>,
    pub peaks: Vec<(i8, i8)>,
    pub bytes: u64,
}

impl PeakReducer {
    /// Takes raw little-endian bytes (a read may split a sample).
    pub fn push_bytes(&mut self, mut data: &[u8]) {
        self.bytes += data.len() as u64;
        if let (Some(low), Some(&high)) = (self.carry.take(), data.first()) {
            self.push(i16::from_le_bytes([low, high]));
            data = &data[1..];
        }
        let mut chunks = data.chunks_exact(2);
        for sample in &mut chunks {
            self.push(i16::from_le_bytes([sample[0], sample[1]]));
        }
        self.carry = chunks.remainder().first().copied();
    }

    fn push(&mut self, sample: i16) {
        if self.count == 0 {
            (self.min, self.max) = (sample, sample);
        } else {
            self.min = self.min.min(sample);
            self.max = self.max.max(sample);
        }
        self.count += 1;
        if self.count == SAMPLES_PER_PEAK {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.count > 0 {
            // i16 → i8 keeps the top byte (arithmetic shift preserves the sign).
            self.peaks.push(((self.min >> 8) as i8, (self.max >> 8) as i8));
            self.count = 0;
        }
    }

    pub fn finish(mut self) -> Vec<(i8, i8)> {
        self.flush();
        self.peaks
    }
}

pub fn cache_path(cache: &Path, key: &str, track: i64) -> PathBuf {
    cache.join("waveforms").join(format!("{}.a{track}.vwf", super::thumbs::key_dir(key)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum WaveformStatus {
    Ready,
    Running,
    Queued,
    Unavailable,
    Cancelled,
    Failed,
}

/// `media://waveform` — job progress, 4 Hz.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WaveformProgress {
    pub media_key: String,
    #[ts(type = "number")]
    pub track_id: i64,
    pub status: WaveformStatus,
    /// 0–1.
    pub progress: f64,
}

struct Job {
    key: String,
    track: i64,
    cancel: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
}

pub struct WaveformService {
    cache: PathBuf,
    player: Arc<Player>,
    app: OnceLock<AppHandle>,
    job: Mutex<Option<Job>>,
}

impl WaveformService {
    pub fn new(cache: PathBuf, player: Arc<Player>) -> Self {
        Self { cache, player, app: OnceLock::new(), job: Mutex::new(None) }
    }

    pub fn attach(&self, app: AppHandle) {
        let _ = self.app.set(app);
    }

    pub fn get(&self, key: &str, track: i64) -> Option<Vec<u8>> {
        std::fs::read(cache_path(&self.cache, key, track)).ok()
    }

    /// Cancels whatever is running (file changed, app closing).
    pub fn cancel(&self) {
        if let Ok(mut job) = self.job.lock() {
            if let Some(job) = job.take() {
                job.cancel.store(true, Ordering::Release);
            }
        }
    }

    pub fn request(&self, key: &str, track: i64, mode: WaveformMode) -> WaveformStatus {
        if mode == WaveformMode::Off {
            return WaveformStatus::Unavailable;
        }
        let Some(media) = self.player.current_media().filter(|m| m.key == key) else {
            return WaveformStatus::Unavailable;
        };
        if media.is_url || media.duration <= 0.0 {
            return WaveformStatus::Unavailable;
        }
        let path = cache_path(&self.cache, key, track);
        if path.is_file() {
            return WaveformStatus::Ready;
        }
        let Ok(mut slot) = self.job.lock() else { return WaveformStatus::Failed };
        if let Some(job) = slot.as_ref() {
            if job.key == key && job.track == track && !job.done.load(Ordering::Acquire) {
                return WaveformStatus::Running;
            }
            job.cancel.store(true, Ordering::Release);
        }
        let job = Job {
            key: key.to_string(),
            track,
            cancel: Arc::new(AtomicBool::new(false)),
            done: Arc::new(AtomicBool::new(false)),
        };
        let run = Run {
            key: key.to_string(),
            track,
            source: media.source,
            duration: media.duration,
            path,
            cancel: job.cancel.clone(),
            done: job.done.clone(),
            app: self.app.get().cloned(),
            player: self.player.clone(),
        };
        if let Err(err) =
            std::thread::Builder::new().name("waveform".into()).spawn(move || run.run())
        {
            tracing::warn!("waveform: could not start the job: {err}");
            return WaveformStatus::Failed;
        }
        *slot = Some(job);
        WaveformStatus::Running
    }
}

struct Run {
    key: String,
    track: i64,
    source: String,
    duration: f64,
    path: PathBuf,
    cancel: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    app: Option<AppHandle>,
    player: Arc<Player>,
}

impl Run {
    fn emit(&self, status: WaveformStatus, progress: f64) {
        if let Some(app) = &self.app {
            let payload = WaveformProgress {
                media_key: self.key.clone(),
                track_id: self.track,
                status,
                progress,
            };
            let _ = app.emit(EVENT, payload);
        }
    }

    fn run(self) {
        let started = Instant::now();
        let status = match self.decode() {
            Ok(peaks) => {
                let vwf = Vwf {
                    peaks_per_second: PEAKS_PER_SECOND,
                    duration: self.duration as f32,
                    peaks,
                };
                match super::thumbs::write_atomic(&self.path, &vwf.encode()) {
                    Ok(()) => {
                        tracing::info!(
                            secs = started.elapsed().as_secs_f64(),
                            peaks = vwf.peaks.len(),
                            "waveform: done"
                        );
                        WaveformStatus::Ready
                    }
                    Err(err) => {
                        tracing::warn!("waveform: could not write the cache file: {err}");
                        WaveformStatus::Failed
                    }
                }
            }
            Err(status) => status,
        };
        self.done.store(true, Ordering::Release);
        self.emit(status, if status == WaveformStatus::Ready { 1.0 } else { 0.0 });
    }

    fn decode(&self) -> Result<Vec<(i8, i8)>, WaveformStatus> {
        let pipe = PcmPipe::create("wave").map_err(|err| {
            tracing::warn!("waveform: no pipe: {err}");
            WaveformStatus::Failed
        })?;
        let out = pipe.path.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let expected = (self.duration * f64::from(SAMPLE_RATE) * 2.0).max(1.0);
        let reader = {
            let (finished, cancel) = (finished.clone(), self.cancel.clone());
            let (app, key, track, player) =
                (self.app.clone(), self.key.clone(), self.track, self.player.clone());
            std::thread::Builder::new()
                .name("waveform-read".into())
                .spawn(move || -> PeakReducer {
                    let mut reducer = PeakReducer::default();
                    let Ok(mut input) = pipe.into_reader(finished) else { return reducer };
                    let mut buf = vec![0u8; 1 << 16];
                    let mut last = Instant::now();
                    loop {
                        match input.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => reducer.push_bytes(&buf[..n]),
                        }
                        if last.elapsed() >= PROGRESS_EVERY {
                            last = Instant::now();
                            // The player moved on to another file: this job is moot.
                            if player.current_media().is_none_or(|m| m.key != key) {
                                cancel.store(true, Ordering::Release);
                            }
                            if let Some(app) = &app {
                                let _ = app.emit(
                                    EVENT,
                                    WaveformProgress {
                                        media_key: key.clone(),
                                        track_id: track,
                                        status: WaveformStatus::Running,
                                        progress: (reducer.bytes as f64 / expected).min(0.99),
                                    },
                                );
                            }
                        }
                    }
                    reducer
                })
                .map_err(|_| WaveformStatus::Failed)?
        };
        self.emit(WaveformStatus::Running, 0.0);
        let result = decode_pcm(&self.source, Some(self.track), &out, &self.cancel);
        finished.store(true, Ordering::Release);
        PcmPipe::release(&out);
        let reducer = reader.join().map_err(|_| WaveformStatus::Failed)?;
        if self.cancel.load(Ordering::Acquire) {
            tracing::debug!("waveform: cancelled");
            return Err(WaveformStatus::Cancelled);
        }
        if let Err(err) = result {
            tracing::warn!("waveform: decoding failed: {err}");
            return Err(WaveformStatus::Failed);
        }
        Ok(reducer.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vwf_round_trip() {
        let vwf =
            Vwf { peaks_per_second: 50, duration: 12.5, peaks: vec![(-128, 127), (0, 0), (-3, 9)] };
        let bytes = vwf.encode();
        assert_eq!(&bytes[..4], b"VWF1");
        assert_eq!(bytes.len(), 16 + 6);
        assert_eq!(Vwf::decode(&bytes), Some(vwf));
        assert_eq!(Vwf::decode(b"VWF0aaaaaaaaaaaa"), None);
        assert_eq!(Vwf::decode(&bytes[..18]), None, "truncated body");
    }

    #[test]
    fn reducer_makes_50_pairs_per_second_and_survives_split_samples() {
        let mut reducer = PeakReducer::default();
        // One second of a ramp from -16000 to +16000, delivered in odd-sized reads.
        let samples: Vec<i16> = (0..8000).map(|i| (i * 4 - 16000) as i16).collect();
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        for chunk in bytes.chunks(333) {
            reducer.push_bytes(chunk);
        }
        let peaks = reducer.finish();
        assert_eq!(peaks.len(), 50);
        assert_eq!(peaks[0], ((-16000i16 >> 8) as i8, ((159 * 4 - 16000) as i16 >> 8) as i8));
        assert!(peaks.windows(2).all(|w| w[1].0 >= w[0].0), "a ramp rises");
    }

    /// The whole path on a real file: headless decode → pipe → peaks, faster than realtime.
    #[test]
    fn decodes_a_real_track_through_the_pipe() {
        let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../test-media/T3-h264-1080p.mp4");
        if !Path::new(file).is_file() {
            return;
        }
        let started = Instant::now();
        let pipe = PcmPipe::create("test").unwrap();
        let out = pipe.path.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let flag = finished.clone();
        let reader = std::thread::spawn(move || {
            let mut reducer = PeakReducer::default();
            let mut input = pipe.into_reader(flag).unwrap();
            let mut buf = vec![0u8; 1 << 16];
            while let Ok(n @ 1..) = input.read(&mut buf) {
                reducer.push_bytes(&buf[..n]);
            }
            reducer.finish()
        });
        decode_pcm(file, None, &out, &AtomicBool::new(false)).unwrap();
        finished.store(true, Ordering::Release);
        PcmPipe::release(&out);
        let peaks = reader.join().unwrap();
        let seconds = peaks.len() as f64 / f64::from(PEAKS_PER_SECOND);
        assert!(seconds > 5.0, "{seconds} s of peaks");
        assert!(started.elapsed().as_secs_f64() < seconds / 5.0, "faster than realtime");
        assert!(peaks.iter().any(|(min, max)| max - min > 4), "real audio, not silence");
    }

    /// Cancelling before any audio flows must not leave the reader waiting forever.
    #[test]
    fn cancel_releases_a_waiting_reader() {
        let pipe = PcmPipe::create("cancel").unwrap();
        let out = pipe.path.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let flag = finished.clone();
        let reader = std::thread::spawn(move || {
            let mut input = pipe.into_reader(flag).unwrap();
            let mut buf = [0u8; 64];
            input.read(&mut buf).unwrap_or(0)
        });
        std::thread::sleep(Duration::from_millis(100));
        finished.store(true, Ordering::Release);
        PcmPipe::release(&out);
        assert_eq!(reader.join().unwrap(), 0);
    }
}
