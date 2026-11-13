//! Types crossing IPC (ARCHITECTURE §7.2). All derive `TS` and are exported to
//! `src/ipc/bindings/` by `pnpm gen:bindings`. 64-bit integers are annotated as `number`:
//! they travel as JSON numbers and never approach 2^53.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::paths::PathsDto;
use crate::video::Capabilities;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
    pub mpv_version: String,
    pub ffmpeg_version: String,
    pub capabilities: Capabilities,
    pub paths: PathsDto,
}

// ── Player state ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Status {
    #[default]
    Idle,
    Loading,
    Playing,
    Paused,
    Buffering,
    Seeking,
    Ended,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Direction {
    #[default]
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MediaKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TrackKind {
    Video,
    Audio,
    Sub,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlayerState {
    #[ts(type = "number")]
    pub rev: u64,
    pub status: Status,
    pub media: Option<MediaInfo>,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub direction: Direction,
    pub tracks: Vec<Track>,
    pub selected: SelectedTracks,
    pub chapters: Vec<Chapter>,
    #[ts(type = "number | null")]
    pub chapter: Option<i64>,
    pub audio_delay_ms: i32,
    pub sub_delay_ms: i32,
    pub sub_visible: bool,
    pub sub_style: SubStyle,
    pub video: VideoAdjust,
    pub eq: EqState,
    pub normalize: bool,
    pub audio_device: String,
    pub audio_devices: Vec<AudioDevice>,
    pub loop_file: bool,
    pub ab_loop: AbLoop,
    pub sleep: Option<SleepTimer>,
    /// `hwdec-current`; `"no"` when decoding in software.
    pub hwdec: String,
    pub applied_presets: Vec<String>,
    pub preset_dirty: bool,
    /// The last load error, shown while `status` is `error`.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MediaInfo {
    /// `media_key` (§9.2); empty until it has been computed.
    pub key: String,
    /// Absolute path or URL.
    pub source: String,
    pub is_url: bool,
    pub title: String,
    pub kind: MediaKind,
    pub duration: f64,
    #[ts(type = "number | null")]
    pub file_size: Option<i64>,
    pub container: Option<String>,
    pub series: Option<SeriesInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SeriesInfo {
    pub key: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Track {
    #[ts(type = "number")]
    pub id: i64,
    pub kind: TrackKind,
    pub title: Option<String>,
    pub lang: Option<String>,
    pub codec: String,
    pub default: bool,
    pub forced: bool,
    pub external: bool,
    pub external_path: Option<String>,
    // Video
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub width: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fps: Option<f64>,
    /// `"sdr" | "hdr10" | "hlg" | "dv"`
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hdr: Option<String>,
    /// Cover art embedded in an audio file (a still image, not a real video track).
    pub image: bool,
    // Audio
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub channels: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub sample_rate: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub bitrate: Option<i64>,
    // Subtitles
    /// False for bitmap formats (PGS, VobSub).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text_based: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SelectedTracks {
    #[ts(type = "number | null")]
    pub video: Option<i64>,
    #[ts(type = "number | null")]
    pub audio: Option<i64>,
    #[ts(type = "number | null")]
    pub sub: Option<i64>,
    #[ts(type = "number | null")]
    pub sub2: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Chapter {
    pub index: u32,
    pub title: String,
    pub time: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AbLoop {
    pub a: Option<f64>,
    pub b: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SleepTimer {
    /// `"minutes" | "end_of_file" | "end_of_queue"`
    pub kind: String,
    /// Seconds left for a timed sleep.
    pub remaining: Option<f64>,
    /// The length chosen for a timed sleep.
    pub minutes: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VideoAdjust {
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub gamma: f64,
    pub hue: f64,
    pub rotate: u32,
    pub flip_h: bool,
    pub flip_v: bool,
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,
    /// `"auto" | "16:9" | … | "stretch" | "fill"`
    pub aspect: String,
    pub tone_mapping: String,
    pub deband: bool,
    pub interpolation: bool,
    pub shaders: Vec<String>,
}

impl Default for VideoAdjust {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 0.0,
            saturation: 0.0,
            gamma: 0.0,
            hue: 0.0,
            rotate: 0,
            flip_h: false,
            flip_v: false,
            zoom: 0.0,
            pan_x: 0.0,
            pan_y: 0.0,
            aspect: "auto".to_string(),
            tone_mapping: "auto".to_string(),
            deband: false,
            interpolation: false,
            shaders: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubStyle {
    pub font: String,
    pub size: f64,
    pub scale: f64,
    pub bold: bool,
    pub color: String,
    pub outline_color: String,
    pub outline_size: f64,
    pub shadow_offset: f64,
    /// `"none" | "box"`
    pub background: String,
    pub background_color: String,
    /// 0–100, 100 = bottom.
    pub position: f64,
    pub margin_y: f64,
    /// `"no" | "scale" | "force" | "strip"`
    pub ass_override: String,
}

/// `sub_set_style` — any subset of [`SubStyle`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, optional_fields)]
pub struct SubStylePatch {
    pub font: Option<String>,
    pub size: Option<f64>,
    pub scale: Option<f64>,
    pub bold: Option<bool>,
    pub color: Option<String>,
    pub outline_color: Option<String>,
    pub outline_size: Option<f64>,
    pub shadow_offset: Option<f64>,
    pub background: Option<String>,
    pub background_color: Option<String>,
    pub position: Option<f64>,
    pub margin_y: Option<f64>,
    pub ass_override: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EqState {
    pub enabled: bool,
    /// `"graphic10" | "parametric"`
    pub mode: String,
    pub preset: Option<String>,
    pub preamp_db: f64,
    pub bands: Vec<EqBand>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EqBand {
    pub freq: f64,
    pub gain_db: f64,
    pub q: f64,
    /// `"peak" | "lowshelf" | "highshelf"`
    #[serde(rename = "type")]
    pub kind: String,
}

/// The ten ISO centre frequencies of the graphic equalizer (DESIGN_SYSTEM §10.2).
pub const GRAPHIC_EQ_FREQS: [f64; 10] =
    [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

impl Default for EqState {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: "graphic10".to_string(),
            preset: Some("flat".to_string()),
            preamp_db: 0.0,
            bands: GRAPHIC_EQ_FREQS
                .iter()
                .map(|&freq| EqBand { freq, gain_db: 0.0, q: 1.0, kind: "peak".to_string() })
                .collect(),
        }
    }
}

/// `video_set_adjust` — any subset of [`VideoAdjust`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, optional_fields)]
pub struct VideoPatch {
    pub brightness: Option<f64>,
    pub contrast: Option<f64>,
    pub saturation: Option<f64>,
    pub gamma: Option<f64>,
    pub hue: Option<f64>,
    pub rotate: Option<u32>,
    pub flip_h: Option<bool>,
    pub flip_v: Option<bool>,
    pub zoom: Option<f64>,
    pub pan_x: Option<f64>,
    pub pan_y: Option<f64>,
    pub aspect: Option<String>,
    pub tone_mapping: Option<String>,
    pub deband: Option<bool>,
    pub interpolation: Option<bool>,
    /// Full paths of the user shaders to run, in order (the flip shaders are separate).
    pub shaders: Option<Vec<String>>,
}

/// A shader file in `<config>/shaders/` (`shaders_list`).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShaderEntry {
    pub name: String,
    pub path: String,
}

/// An equalizer preset (KEYBINDS_AND_CONFIG §5.12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EqPreset {
    pub id: String,
    pub name: String,
    /// `"graphic10" | "parametric"`
    pub mode: String,
    pub preamp_db: f64,
    pub bands: Vec<EqBand>,
    /// Shipped with vihpluh (a user preset with the same id replaces it).
    #[serde(default)]
    pub builtin: bool,
}

/// `player_set_sleep_timer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SleepKind {
    Off,
    Minutes,
    EndOfFile,
    EndOfQueue,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SleepRequest {
    pub kind: SleepKind,
    #[serde(default)]
    #[ts(optional)]
    pub minutes: Option<f64>,
}

/// `player://inspector` — the Playback tab's stream info (DESIGN_SYSTEM §10.4), 2 Hz while
/// subscribed. Values mpv does not report are null.
#[derive(Debug, Clone, PartialEq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InspectorStats {
    pub video_codec: Option<String>,
    #[ts(type = "number | null")]
    pub width: Option<i64>,
    #[ts(type = "number | null")]
    pub height: Option<i64>,
    pub fps: Option<f64>,
    pub display_fps: Option<f64>,
    pub pixel_format: Option<String>,
    #[ts(type = "number | null")]
    pub bit_depth: Option<i64>,
    pub primaries: Option<String>,
    pub transfer: Option<String>,
    pub video_bitrate: Option<f64>,
    pub hwdec: Option<String>,
    pub vo: Option<String>,
    #[ts(type = "number")]
    pub dropped_frames: i64,
    #[ts(type = "number")]
    pub decoder_dropped_frames: i64,
    #[ts(type = "number")]
    pub delayed_frames: i64,
    pub audio_codec: Option<String>,
    #[ts(type = "number | null")]
    pub sample_rate: Option<i64>,
    pub channels: Option<String>,
    pub audio_bitrate: Option<f64>,
    pub audio_out: Option<String>,
    pub ao: Option<String>,
    pub sub_codec: Option<String>,
    pub sub_lang: Option<String>,
    pub sub_title: Option<String>,
    pub container: Option<String>,
    #[ts(type = "number | null")]
    pub file_size: Option<i64>,
    pub path: Option<String>,
    pub cache_seconds: Option<f64>,
    #[ts(type = "number | null")]
    pub cache_bytes: Option<i64>,
}

// ── Event payloads (§7.4) ───────────────────────────────────────────────────

/// `player://time` — position, duration, and the end of the buffered range.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TimeEvent {
    pub pos: f64,
    pub dur: f64,
    pub cache_end: f64,
    /// Milliseconds on the core's monotonic clock (diagnostics; the frontend interpolates
    /// from its own receive time).
    pub at: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OsdKind {
    Info,
    Warn,
    Error,
}

/// `osd://show` — a notice that originates in the core.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OsdEvent {
    pub kind: OsdKind,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub action: Option<OsdAction>,
    /// A newer toast with the same group replaces the older one instead of stacking.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OsdAction {
    pub label: String,
    pub action_id: String,
    /// Arguments for parameterised actions (KEYBINDS_AND_CONFIG §3.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "Record<string, unknown>")]
    pub args: Option<serde_json::Value>,
}

impl OsdEvent {
    pub fn new(kind: OsdKind, text: impl Into<String>) -> Self {
        Self { kind, text: text.into(), icon: None, value: None, action: None, group: None }
    }

    pub fn grouped(mut self, group: &str) -> Self {
        self.group = Some(group.to_string());
        self
    }
}

/// `config://changed` — one changed setting.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConfigChanged {
    pub path: String,
    #[ts(type = "unknown")]
    pub value: serde_json::Value,
}

/// `app://open` — sources forwarded from a second instance (informational).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpenEvent {
    pub sources: Vec<String>,
    pub mode: OpenMode,
}

// ── Command requests (§7.3) ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OpenMode {
    #[default]
    Replace,
    Enqueue,
    PlayNext,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpenRequest {
    pub sources: Vec<String>,
    #[serde(default)]
    pub mode: OpenMode,
    #[serde(default)]
    #[ts(optional)]
    pub start_at: Option<f64>,
    #[serde(default)]
    #[ts(optional)]
    pub paused: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub no_resume: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub preset_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SeekKind {
    Absolute,
    Relative,
    Percent,
}

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SeekRequest {
    pub kind: SeekKind,
    pub value: f64,
    pub exact: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TrackSlot {
    Video,
    Audio,
    Sub,
    Sub2,
}
