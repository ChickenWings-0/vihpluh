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
