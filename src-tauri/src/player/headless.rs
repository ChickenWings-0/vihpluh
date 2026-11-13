//! Headless mpv handles for the media services (ARCHITECTURE §11): a software-rendering
//! thumbnailer and a PCM decoder for waveforms. They never touch the primary handle or any
//! window; `media::thumbs` and `media::waveform` schedule them, cache, and encode.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::mpv::{EndReason, Event, Mpv, MpvError, MpvResult, SwRender};
use super::options::OPTIONAL_SCRIPT_SWITCHES;

/// Options every headless handle shares: no config, scripts, terminal, or network lookups.
const QUIET: &[(&str, &str)] = &[
    ("config", "no"),
    ("load-scripts", "no"),
    ("terminal", "no"),
    ("input-default-bindings", "no"),
    ("ytdl", "no"),
    ("sub-auto", "no"),
    ("audio-file-auto", "no"),
    ("resume-playback", "no"),
];

fn handle(options: &[(&str, &str)]) -> MpvResult<Mpv> {
    let mpv = Mpv::create()?;
    for (name, value) in QUIET.iter().chain(options) {
        mpv.set_option(name, value)?;
    }
    for name in OPTIONAL_SCRIPT_SWITCHES {
        let _ = mpv.set_option(name, "no");
    }
    mpv.initialize()?;
    Ok(mpv)
}

fn timeout(what: &str) -> MpvError {
    MpvError::Api { code: -1, message: format!("timed out waiting for {what}") }
}

/// Waits for `FileLoaded`, failing on an end-of-file error first.
fn wait_loaded(mpv: &Mpv, limit: Duration) -> MpvResult<()> {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        match mpv.wait_event(0.05) {
            Event::FileLoaded => return Ok(()),
            Event::EndFile { error, .. } => {
                return Err(MpvError::Api {
                    code: -1,
                    message: error.unwrap_or_else(|| "the file could not be opened".into()),
                });
            }
            _ => {}
        }
    }
    Err(timeout("the file to open"))
}

/// One file opened for preview frames (§11.1): paused, video only, keyframe seeks, frames
/// rendered in software at a small size.
pub struct Thumbnailer {
    render: SwRender,
    /// Display aspect (width / height) of the video.
    pub aspect: f64,
}

impl Thumbnailer {
    pub fn open(source: &str, hwdec: &str) -> MpvResult<Self> {
        let mpv = handle(&[
            ("vo", "libmpv"),
            ("ao", "null"),
            ("aid", "no"),
            ("sid", "no"),
            ("pause", "yes"),
            ("hr-seek", "no"),
            ("hwdec", hwdec),
            ("vd-lavc-skiploopfilter", "all"),
            ("vd-lavc-fast", "yes"),
            ("vd-lavc-threads", "2"),
            ("demuxer-readahead-secs", "0"),
            ("cache", "no"),
            ("keep-open", "yes"),
        ])?;
        let render = SwRender::new(mpv)?;
        render.mpv().command(&["loadfile", source])?;
        wait_loaded(render.mpv(), Duration::from_secs(10))?;
        let params = render.mpv().get_node("video-params");
        let (w, h) = params
            .as_ref()
            .and_then(|p| Some((p.f64_field("dw")?, p.f64_field("dh")?)))
            .filter(|(w, h)| *w > 0.0 && *h > 0.0)
            .unwrap_or((16.0, 9.0));
        Ok(Self { render, aspect: w / h })
    }

    /// The frame nearest the keyframe at or before `seconds`, as `rgb0` pixels.
    pub fn frame_at(&self, seconds: f64, width: usize, height: usize) -> MpvResult<Vec<u8>> {
        let mpv = self.render.mpv();
        mpv.command(&["seek", &format!("{seconds:.3}"), "absolute+keyframes"])?;
        // Verified in S4: after a paused keyframe seek the render context reports a frame.
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            if self.render.frame_ready() {
                return self.render.render(width, height);
            }
            let _ = mpv.wait_event(0.005);
        }
        Err(timeout("a preview frame"))
    }
}

/// Decodes one audio track as fast as the CPU allows to `out` (a pipe or file path) as
/// 8 kHz mono signed 16-bit little-endian PCM (§11.2). Returns when the track ends, or early
/// when `cancel` is set.
pub fn decode_pcm(
    source: &str,
    track: Option<i64>,
    out: &str,
    cancel: &AtomicBool,
) -> MpvResult<()> {
    let aid = track.map_or_else(|| "auto".to_string(), |id| id.to_string());
    let mpv = handle(&[
        ("vo", "null"),
        ("vid", "no"),
        ("sid", "no"),
        ("aid", &aid),
        ("ao", "pcm"),
        ("ao-pcm-waveheader", "no"),
        ("ao-pcm-file", out),
        ("audio-format", "s16"),
        ("audio-channels", "mono"),
        ("audio-samplerate", "8000"),
        ("cache", "no"),
    ])?;
    mpv.command(&["loadfile", source])?;
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(MpvError::Api { code: -1, message: "cancelled".into() });
        }
        match mpv.wait_event(0.1) {
            Event::EndFile { reason: EndReason::Eof, .. } => return Ok(()),
            Event::EndFile { error, .. } => {
                return Err(MpvError::Api {
                    code: -1,
                    message: error.unwrap_or_else(|| "decoding stopped".into()),
                });
            }
            Event::Shutdown => return Err(MpvError::Create),
            _ => {}
        }
    }
}

/// Reads durations for the library (§8.2), one file after another on one handle: nothing is
/// decoded or shown, the file is only opened far enough for the demuxer to report its length.
pub struct DurationProbe {
    mpv: Mpv,
}

impl DurationProbe {
    pub fn new() -> MpvResult<Self> {
        let mpv = handle(&[
            // Tracks stay selectable: with none, mpv refuses to load the file at all. Paused
            // with null outputs, nothing is shown or heard and next to nothing is decoded.
            ("vo", "null"),
            ("ao", "null"),
            ("sid", "no"),
            ("pause", "yes"),
            ("hwdec", "no"),
            ("cache", "no"),
            ("demuxer-readahead-secs", "0"),
            ("idle", "yes"),
        ])?;
        Ok(Self { mpv })
    }

    /// The file's duration in seconds, or `None` when it cannot be opened or does not say.
    pub fn duration(&self, source: &str) -> Option<f64> {
        self.mpv.command(&["loadfile", source]).ok()?;
        // The file before this one ends (reason: stop) as this one replaces it; only an
        // error ending is about the file being opened now.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut loaded = false;
        while Instant::now() < deadline {
            match self.mpv.wait_event(0.05) {
                Event::FileLoaded => {
                    loaded = true;
                    break;
                }
                Event::EndFile { reason: EndReason::Error, .. } | Event::Shutdown => break,
                _ => {}
            }
        }
        let duration = if loaded { self.mpv.get_f64("duration") } else { None };
        duration.filter(|d| d.is_finite() && *d > 0.0)
    }
}

/// Downloads a URL to a file through mpv's own network stack (`stream-dump`), so stream
/// lists need no HTTP client of their own and follow the same proxy and user-agent settings
/// as playback. `options` are extra mpv options (`user-agent`, `http-proxy`, …).
pub fn fetch_url(
    url: &str,
    dest: &std::path::Path,
    options: &[(String, String)],
    limit: Duration,
) -> MpvResult<()> {
    let dest_text = dest.display().to_string();
    let mut all: Vec<(&str, &str)> =
        vec![("vo", "null"), ("ao", "null"), ("stream-dump", &dest_text), ("cache", "no")];
    all.extend(options.iter().map(|(name, value)| (name.as_str(), value.as_str())));
    let mpv = handle(&all)?;
    mpv.command(&["loadfile", url])?;
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        match mpv.wait_event(0.1) {
            Event::EndFile { reason: EndReason::Error, error } => {
                return Err(MpvError::Api {
                    code: -1,
                    message: error.unwrap_or_else(|| "the download failed".into()),
                });
            }
            // A finished dump ends the "file" (as end of file, or as a stop).
            Event::EndFile { .. } => return Ok(()),
            Event::Shutdown => return Err(MpvError::Create),
            _ => {}
        }
    }
    Err(timeout("the download"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stream list is fetched as bytes, whatever it is: here a local file through `file://`.
    #[test]
    fn stream_dump_copies_a_source_to_a_file() {
        let dir = std::env::temp_dir().join(format!("vihpluh-fetch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("list.m3u");
        std::fs::write(&source, "#EXTM3U\n#EXTINF:-1,One\nhttp://example.com/1\n").unwrap();
        let dest = dir.join("copy.m3u");
        fetch_url(&source.display().to_string(), &dest, &[], Duration::from_secs(10)).unwrap();
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            "#EXTM3U\n#EXTINF:-1,One\nhttp://example.com/1\n"
        );
        let missing = dir.join("nope.m3u").display().to_string();
        assert!(fetch_url(&missing, &dest, &[], Duration::from_secs(10)).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The embedded subtitle fixture is a real (audio-only) Matroska file of eight seconds.
    #[test]
    fn a_real_file_reports_its_duration() {
        let path =
            std::env::temp_dir().join(format!("vihpluh-probe-ok-{}.mkv", std::process::id()));
        std::fs::write(&path, include_bytes!("../subtitles/fixtures/two-subs.mkv")).unwrap();
        let probe = DurationProbe::new().unwrap();
        let duration = probe.duration(&path.display().to_string()).unwrap();
        assert!((7.5..8.5).contains(&duration), "{duration}");
        // The handle is reused for the next file.
        assert!(probe.duration(&path.display().to_string()).is_some());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_file_that_is_not_media_has_no_duration() {
        let path = std::env::temp_dir().join(format!("vihpluh-probe-{}.mkv", std::process::id()));
        std::fs::write(&path, b"not a video").unwrap();
        let probe = DurationProbe::new().unwrap();
        assert_eq!(probe.duration(&path.display().to_string()), None);
        assert_eq!(probe.duration("Z:/missing.mkv"), None);
        let _ = std::fs::remove_file(&path);
    }
}
