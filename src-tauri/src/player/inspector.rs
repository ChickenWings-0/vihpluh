//! The stream inspector (DESIGN_SYSTEM §10.4): read at 2 Hz on the core thread, only while
//! the Playback tab subscribes (`inspector_subscribe`), and sent as `player://inspector`.

use std::time::{Duration, Instant};

use crate::ipc::dto::{InspectorStats, TrackKind};
use crate::ipc::events;

use super::Core;
use super::mpv::{Mpv, Node};

pub const INSPECTOR_INTERVAL: Duration = Duration::from_millis(500);

/// Bit depth from an FFmpeg pixel format name: `yuv420p10le` → 10, `p010` → 10, `nv12` → 8.
pub fn bit_depth(pixel_format: &str) -> Option<i64> {
    let name = pixel_format.trim_end_matches("le").trim_end_matches("be");
    match name {
        "p010" => return Some(10),
        "p012" => return Some(12),
        "p016" => return Some(16),
        "" => return None,
        _ => {}
    }
    let digits: String = name
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    match digits.parse::<i64>() {
        Ok(depth @ 9..=16) if name.contains('p') => Some(depth),
        _ => Some(8),
    }
}

fn text(node: Option<&Node>) -> Option<String> {
    node.and_then(Node::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

pub fn read(mpv: &Mpv) -> InspectorStats {
    let video = mpv.get_node("video-params");
    let audio = mpv.get_node("audio-params");
    let audio_out = mpv.get_node("audio-out-params");
    // With hardware decoding `pixelformat` names the GPU surface (d3d11, vaapi); the
    // decoded format is in `hw-pixelformat`.
    let pixel_format = video
        .as_ref()
        .and_then(|v| text(v.get("hw-pixelformat")).or_else(|| text(v.get("pixelformat"))));
    let positive = |v: Option<f64>| v.filter(|x| x.is_finite() && *x > 0.0);
    InspectorStats {
        video_codec: mpv.get_string("video-format").filter(|s| !s.is_empty()),
        width: video.as_ref().and_then(|v| v.i64_field("w")),
        height: video.as_ref().and_then(|v| v.i64_field("h")),
        fps: positive(mpv.get_f64("container-fps")),
        display_fps: positive(mpv.get_f64("display-fps")),
        bit_depth: pixel_format.as_deref().and_then(bit_depth),
        pixel_format,
        primaries: video.as_ref().and_then(|v| text(v.get("primaries"))),
        transfer: video.as_ref().and_then(|v| text(v.get("gamma"))),
        video_bitrate: positive(mpv.get_f64("video-bitrate")),
        hwdec: mpv.get_string("hwdec-current").filter(|s| !s.is_empty()),
        vo: mpv.get_string("current-vo").filter(|s| !s.is_empty()),
        dropped_frames: mpv.get_i64("frame-drop-count").unwrap_or(0),
        decoder_dropped_frames: mpv.get_i64("decoder-frame-drop-count").unwrap_or(0),
        delayed_frames: mpv.get_i64("vo-delayed-frame-count").unwrap_or(0),
        audio_codec: mpv.get_string("audio-codec-name").filter(|s| !s.is_empty()),
        sample_rate: audio.as_ref().and_then(|a| a.i64_field("samplerate")),
        channels: audio.as_ref().and_then(|a| text(a.get("hr-channels"))),
        audio_bitrate: positive(mpv.get_f64("audio-bitrate")),
        audio_out: audio_out.as_ref().and_then(|a| {
            let channels = text(a.get("hr-channels"))?;
            Some(match a.i64_field("samplerate") {
                Some(rate) => format!("{channels} · {} kHz", rate as f64 / 1000.0),
                None => channels,
            })
        }),
        ao: mpv.get_string("current-ao").filter(|s| !s.is_empty()),
        sub_codec: None,
        sub_lang: None,
        sub_title: None,
        container: mpv.get_string("file-format").filter(|s| !s.is_empty()),
        file_size: mpv.get_i64("file-size"),
        path: mpv.get_string("path").filter(|s| !s.is_empty()),
        cache_seconds: mpv.get_f64("demuxer-cache-duration"),
        cache_bytes: mpv
            .get_node("demuxer-cache-state")
            .and_then(|state| state.i64_field("fw-bytes")),
    }
}

impl Core {
    pub(super) fn set_inspector(&mut self, enabled: bool) {
        self.inspector = enabled;
        if enabled {
            self.last_inspector = Instant::now() - INSPECTOR_INTERVAL;
        }
    }

    pub(super) fn tick_inspector(&mut self, now: Instant) {
        if !self.inspector || now.duration_since(self.last_inspector) < INSPECTOR_INTERVAL {
            return;
        }
        self.last_inspector = now;
        let mut stats = if self.mirror.state.media.is_some() {
            read(&self.mpv)
        } else {
            InspectorStats::default()
        };
        let state = &self.mirror.state;
        if let Some(track) = state
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Sub && Some(t.id) == state.selected.sub)
        {
            stats.sub_codec = Some(track.codec.clone());
            stats.sub_lang = track.lang.clone();
            stats.sub_title = track.title.clone();
        }
        self.emit(events::PLAYER_INSPECTOR, &stats);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_depth_from_pixel_formats() {
        assert_eq!(bit_depth("yuv420p10le"), Some(10));
        assert_eq!(bit_depth("yuv420p"), Some(8));
        assert_eq!(bit_depth("p010"), Some(10));
        assert_eq!(bit_depth("nv12"), Some(8));
        assert_eq!(bit_depth("yuv444p12"), Some(12));
        assert_eq!(bit_depth(""), None);
    }
}
