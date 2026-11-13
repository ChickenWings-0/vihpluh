//! Seek-preview thumbnails (ARCHITECTURE §11.1): one worker thread with its own headless,
//! software-rendering mpv handle on the current file. Requests are latest-wins — a new one
//! cancels everything queued — and while idle the worker pre-generates the neighbours of the
//! last bucket asked for. Frames are 320 px wide JPEGs (q70) cached under
//! `cache/thumbs/<key>/<step>/<bucket>.jpg` (§9.7). The main video is never touched.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use crate::ipc::dto::{MediaInfo, MediaKind};
use crate::player::headless::Thumbnailer;

pub const WIDTH: usize = 320;
const QUALITY: u8 = 70;
/// Neighbours of the last hovered bucket generated while idle (§11.1).
const NEIGHBOURS: i64 = 8;
/// The preview handle is closed after this long without requests (frees decoder memory).
const IDLE_CLOSE: Duration = Duration::from_secs(60);

/// Seconds per bucket for a file of `duration` seconds: `clamp(duration / 300, 2, 30)`.
pub fn bucket_step(duration: f64) -> u32 {
    if !duration.is_finite() || duration <= 0.0 {
        return 2;
    }
    (duration / 300.0).round().clamp(2.0, 30.0) as u32
}

/// Where a bucket's frame is taken: its middle. A keyframe seek to a time exactly on a
/// keyframe can land on the previous one, so bucket starts (often keyframes) are avoided.
pub fn bucket_time(bucket: u32, step: u32, duration: f64) -> f64 {
    let middle = (f64::from(bucket) + 0.5) * f64::from(step);
    middle.min((duration - 0.5).max(0.0))
}

/// The media key as a directory name (`f:0123…` → `f_0123…`).
pub fn key_dir(key: &str) -> String {
    key.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

pub fn cache_path(cache: &Path, key: &str, step: u32, bucket: u32) -> PathBuf {
    cache.join("thumbs").join(key_dir(key)).join(step.to_string()).join(format!("{bucket}.jpg"))
}

/// Whether previews apply to this media at all (not streams, not audio-only).
pub fn eligible(media: &MediaInfo) -> bool {
    !media.is_url && media.kind == MediaKind::Video && !media.key.is_empty() && media.duration > 0.0
}

pub type Reply = Box<dyn FnOnce(Option<Vec<u8>>) + Send>;

pub struct Request {
    pub media: MediaInfo,
    pub step: u32,
    pub bucket: u32,
    pub hwdec: String,
    pub reply: Reply,
}

enum Msg {
    Request(Box<Request>),
    Shutdown,
}

pub struct ThumbService {
    tx: Sender<Msg>,
    cache: PathBuf,
}

impl ThumbService {
    pub fn start(cache: PathBuf) -> std::io::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let dir = cache.clone();
        std::thread::Builder::new()
            .name("thumbs".into())
            .spawn(move || Worker { cache: dir, open: None, last: None }.run(&rx))?;
        Ok(Self { tx, cache })
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache
    }

    /// A cached thumbnail, read on the caller's thread (no worker round trip).
    pub fn cached(&self, key: &str, step: u32, bucket: u32) -> Option<Vec<u8>> {
        std::fs::read(cache_path(&self.cache, key, step, bucket)).ok()
    }

    pub fn request(&self, request: Request) {
        if let Err(mpsc::SendError(Msg::Request(request))) =
            self.tx.send(Msg::Request(Box::new(request)))
        {
            (request.reply)(None);
        }
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(Msg::Shutdown);
    }
}

struct Open {
    key: String,
    source: String,
    duration: f64,
    hwdec: String,
    thumbnailer: Thumbnailer,
}

struct Worker {
    cache: PathBuf,
    open: Option<Open>,
    /// (key, step, bucket) of the last request, for neighbour pre-generation.
    last: Option<(String, u32, u32)>,
}

impl Worker {
    fn run(&mut self, rx: &Receiver<Msg>) {
        let mut pending: Vec<u32> = Vec::new();
        let mut last_request = Instant::now();
        loop {
            // Idle work only while nothing is waiting.
            let wait = if pending.is_empty() { Duration::from_secs(5) } else { Duration::ZERO };
            let message = match rx.recv_timeout(wait) {
                Ok(message) => Some(message),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            };
            match message {
                Some(Msg::Shutdown) => return,
                Some(Msg::Request(first)) => {
                    // Latest wins: answer everything older with "no thumbnail".
                    let mut latest = first;
                    while let Ok(next) = rx.try_recv() {
                        match next {
                            Msg::Request(newer) => {
                                (std::mem::replace(&mut latest, newer).reply)(None);
                            }
                            Msg::Shutdown => {
                                (latest.reply)(None);
                                return;
                            }
                        }
                    }
                    last_request = Instant::now();
                    let Request { media, step, bucket, hwdec, reply } = *latest;
                    let bytes = self.generate(&media, step, bucket, &hwdec);
                    reply(bytes);
                    self.last = Some((media.key.clone(), step, bucket));
                    pending = neighbours(bucket, media.duration, step);
                }
                None => {
                    if let (Some(bucket), Some((key, step, _))) =
                        (pending.first().copied(), &self.last)
                    {
                        pending.remove(0);
                        let (key, step) = (key.clone(), *step);
                        if !cache_path(&self.cache, &key, step, bucket).is_file() {
                            if let Some(open) = &self.open {
                                if open.key == key {
                                    let media = MediaInfo {
                                        key: key.clone(),
                                        source: open.source.clone(),
                                        is_url: false,
                                        title: String::new(),
                                        kind: MediaKind::Video,
                                        duration: open.duration,
                                        file_size: None,
                                        container: None,
                                        series: None,
                                    };
                                    let hwdec = open.hwdec.clone();
                                    let _ = self.generate(&media, step, bucket, &hwdec);
                                }
                            }
                        }
                    } else if self.open.is_some() && last_request.elapsed() >= IDLE_CLOSE {
                        tracing::debug!("thumbs: closing the idle preview handle");
                        self.open = None;
                    }
                }
            }
        }
    }

    fn generate(
        &mut self,
        media: &MediaInfo,
        step: u32,
        bucket: u32,
        hwdec: &str,
    ) -> Option<Vec<u8>> {
        let path = cache_path(&self.cache, &media.key, step, bucket);
        if let Ok(bytes) = std::fs::read(&path) {
            return Some(bytes);
        }
        let started = Instant::now();
        let reopen = self
            .open
            .as_ref()
            .is_none_or(|o| o.key != media.key || o.source != media.source || o.hwdec != hwdec);
        if reopen {
            self.open = None;
            match Thumbnailer::open(&media.source, hwdec) {
                Ok(thumbnailer) => {
                    self.open = Some(Open {
                        key: media.key.clone(),
                        source: media.source.clone(),
                        duration: media.duration,
                        hwdec: hwdec.to_string(),
                        thumbnailer,
                    });
                }
                Err(err) => {
                    tracing::warn!("thumbs: could not open the file for previews: {err}");
                    return None;
                }
            }
        }
        let open = self.open.as_ref()?;
        let height = ((WIDTH as f64 / open.thumbnailer.aspect).round() as usize).max(2) & !1;
        let at = bucket_time(bucket, step, media.duration);
        let pixels = match open.thumbnailer.frame_at(at, WIDTH, height) {
            Ok(pixels) => pixels,
            Err(err) => {
                tracing::debug!("thumbs: no frame at {at:.1} s: {err}");
                return None;
            }
        };
        let jpeg = encode_jpeg(&pixels, WIDTH, height)?;
        if let Err(err) = write_atomic(&path, &jpeg) {
            tracing::debug!("thumbs: could not cache a thumbnail: {err}");
        }
        tracing::debug!(
            bucket,
            step,
            ms = started.elapsed().as_millis() as u64,
            reopened = reopen,
            "thumbs: generated"
        );
        Some(jpeg)
    }
}

/// ±1, ±2, … ±8 buckets around `bucket`, nearest first, inside the file.
fn neighbours(bucket: u32, duration: f64, step: u32) -> Vec<u32> {
    let last = (duration / f64::from(step)).floor().max(0.0) as i64;
    let mut out = Vec::new();
    for d in 1..=NEIGHBOURS {
        for b in [i64::from(bucket) + d, i64::from(bucket) - d] {
            if (0..=last).contains(&b) {
                out.push(b as u32);
            }
        }
    }
    out
}

/// `rgb0` pixels → JPEG.
pub fn encode_jpeg(rgb0: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(width * height / 4);
    let encoder = jpeg_encoder::Encoder::new(&mut out, QUALITY);
    // The fourth byte is padding; RGBA tells the encoder to skip it.
    match encoder.encode(rgb0, width as u16, height as u16, jpeg_encoder::ColorType::Rgba) {
        Ok(()) => Some(out),
        Err(err) => {
            tracing::debug!("thumbs: JPEG encoding failed: {err}");
            None
        }
    }
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_steps() {
        assert_eq!(bucket_step(150.0), 2, "short files: every 2 s");
        assert_eq!(bucket_step(1420.0), 5);
        assert_eq!(bucket_step(3.0 * 3600.0), 30, "3 h caps at 30 s");
        assert_eq!(bucket_step(f64::NAN), 2);
    }

    #[test]
    fn frames_come_from_the_middle_of_each_bucket() {
        assert_eq!(bucket_time(3, 2, 60.0), 7.0);
        assert_eq!(bucket_time(29, 2, 60.0), 59.0, "clamped before the end");
    }

    #[test]
    fn neighbours_are_nearest_first_and_in_range() {
        assert_eq!(&neighbours(0, 100.0, 2)[..3], &[1, 2, 3]);
        let n = neighbours(10, 100.0, 2);
        assert_eq!(&n[..4], &[11, 9, 12, 8]);
        assert_eq!(n.len(), 16);
        assert!(neighbours(50, 100.0, 2).iter().all(|b| *b <= 50));
    }

    #[test]
    fn key_dirs_are_file_safe() {
        assert_eq!(key_dir("f:00ab"), "f_00ab");
        assert_eq!(key_dir("u:a/b?c"), "u_a_b_c");
    }

    #[test]
    fn jpeg_round_trip_header() {
        let pixels = vec![128u8; 16 * 8 * 4];
        let jpeg = encode_jpeg(&pixels, 16, 8).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8], "SOI marker");
    }

    /// End to end on a real file: a frame decodes and encodes within the uncached budget.
    #[test]
    fn renders_a_real_frame() {
        let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../test-media/T3-h264-1080p.mp4");
        if !Path::new(file).is_file() {
            return;
        }
        let thumbnailer = Thumbnailer::open(file, "no").unwrap();
        let height = ((WIDTH as f64 / thumbnailer.aspect).round() as usize) & !1;
        let started = Instant::now();
        let pixels = thumbnailer.frame_at(5.0, WIDTH, height).unwrap();
        let distinct: std::collections::HashSet<&[u8]> = pixels.chunks(4).collect();
        assert!(distinct.len() > 8, "a real picture");
        assert!(encode_jpeg(&pixels, WIDTH, height).is_some());
        assert!(started.elapsed() < Duration::from_millis(400));
    }

    /// A frame belongs to the time asked for, not to the previous request: asking for the
    /// same time again (after another one) gives the same picture.
    #[test]
    fn frames_follow_the_requested_time() {
        let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../test-media/T3-h264-1080p.mp4");
        if !Path::new(file).is_file() {
            return;
        }
        let thumbnailer = Thumbnailer::open(file, "no").unwrap();
        let (w, h) = (160, 90);
        let diff = |a: &[u8], b: &[u8]| -> f64 {
            a.iter().zip(b).map(|(x, y)| f64::from(x.abs_diff(*y))).sum::<f64>() / a.len() as f64
        };
        let first = thumbnailer.frame_at(4.0, w, h).unwrap();
        let other = thumbnailer.frame_at(14.0, w, h).unwrap();
        let again = thumbnailer.frame_at(4.0, w, h).unwrap();
        assert!(diff(&first, &other) > 1.0, "different times look different");
        assert!(
            diff(&first, &again) < 0.5,
            "the same time looks the same: {}",
            diff(&first, &again)
        );
    }
}
