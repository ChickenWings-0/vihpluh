//! The typed configuration (KEYBINDS_AND_CONFIG §5). Every key here has a default in
//! `resources/default.toml` and a row in the docs; the three must stay in step.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Config {
    pub schema: u32,
    pub general: General,
    pub interface: Interface,
    pub controls: Controls,
    pub playback: Playback,
    pub video: Video,
    pub audio: Audio,
    pub subtitles: Subtitles,
    pub library: Library,
    pub network: Network,
    pub performance: Performance,
    pub mpv: MpvSection,
    pub env: BTreeMap<String, String>,
}

// ── General ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct General {
    pub restore_session: bool,
    pub single_instance: bool,
    pub open_behavior: OpenBehavior,
    pub pause_on_minimize: bool,
    pub media_keys: bool,
    pub screenshot_dir: String,
    pub screenshot_format: ScreenshotFormat,
    pub screenshot_template: String,
    pub log_level: LogLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OpenBehavior {
    Replace,
    Enqueue,
    PlayNext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ScreenshotFormat {
    Png,
    Jpg,
    Webp,
}

impl ScreenshotFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpg => "jpg",
            Self::Webp => "webp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

// ── Interface ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Interface {
    pub theme: Theme,
    pub accent: String,
    pub ui_scale: f64,
    pub window_decorations: WindowDecorations,
    pub autohide_delay_ms: u32,
    pub autohide_in_windowed: bool,
    pub sidebar_mode: SidebarMode,
    pub sidebar_width: u32,
    pub seek_preview: bool,
    pub seek_waveform: bool,
    pub seek_chapter_marks: bool,
    pub time_display: TimeDisplay,
    pub osd_enabled: bool,
    pub osd_shortcut_badge: bool,
    pub osd_position: OsdPosition,
    pub osd_duration_ms: u32,
    pub tooltip_shortcuts: bool,
    pub reduce_motion: ReduceMotion,
    pub pip_width: u32,
    pub pip_corner: Corner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WindowDecorations {
    Custom,
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SidebarMode {
    Auto,
    Dock,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TimeDisplay {
    Total,
    Remaining,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum OsdPosition {
    TopLeft,
    TopCenter,
    TopRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReduceMotion {
    System,
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

// ── Controls ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Controls {
    pub seek_small_s: f64,
    pub seek_medium_s: f64,
    pub seek_large_s: f64,
    pub precise_seek: bool,
    pub volume_step: f64,
    pub speed_step: f64,
    pub sub_delay_step_ms: i32,
    pub audio_delay_step_ms: i32,
    pub sub_size_step: f64,
    pub zoom_step: f64,
    pub drag_moves_window: bool,
    pub wheel_invert: bool,
}

// ── Playback ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Playback {
    pub resume: ResumeMode,
    pub remember_per_file: bool,
    pub auto_queue_folder: bool,
    pub at_queue_end: AtQueueEnd,
    pub autoplay: bool,
    pub default_speed: f64,
    pub pitch_correction: bool,
    pub sleep_action: SleepAction,
    pub finished_percent: u32,
    pub presets_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ResumeMode {
    Always,
    Ask,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AtQueueEnd {
    Hold,
    Stop,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SleepAction {
    Pause,
    Quit,
}

// ── Video ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Video {
    /// `auto`, `off`, or a raw mpv `hwdec` value.
    pub hwdec: String,
    pub gpu_api: GpuApi,
    pub quality: VideoQuality,
    pub tone_mapping: String,
    pub hdr_passthrough: AutoOff,
    pub target_peak: IntOrKeyword,
    pub deband: bool,
    pub interpolation: bool,
    pub sync: VideoSync,
    pub default_aspect: String,
    pub linux_force_xwayland: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GpuApi {
    Auto,
    D3d11,
    Vulkan,
    Opengl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum VideoQuality {
    Fast,
    Balanced,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AutoOff {
    Auto,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum VideoSync {
    Audio,
    DisplayResample,
}

/// A value that is either a number or a keyword such as `"auto"` / `"max"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(untagged)]
pub enum IntOrKeyword {
    Int(i64),
    Keyword(String),
}

// ── Audio ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Audio {
    pub device: String,
    pub preferred_languages: Vec<String>,
    pub remember_volume: bool,
    pub default_volume: f64,
    pub max_volume: u32,
    pub normalize: bool,
    pub channels: AudioChannels,
    pub passthrough: Vec<String>,
    pub exclusive: bool,
    pub equalizer_enabled: bool,
    pub equalizer_preset: String,
    pub gapless: Gapless,
    pub replaygain: ReplayGain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum AudioChannels {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "stereo")]
    Stereo,
    #[serde(rename = "5.1")]
    Surround51,
    #[serde(rename = "7.1")]
    Surround71,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Gapless {
    Weak,
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReplayGain {
    Off,
    Track,
    Album,
}

// ── Subtitles ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Subtitles {
    pub preferred_languages: Vec<String>,
    pub show_by_default: bool,
    pub forced_only_when_audio_matches: bool,
    pub auto_load: SubAutoLoad,
    pub extra_dirs: Vec<String>,
    pub font: String,
    pub font_size: f64,
    pub scale: f64,
    pub bold: bool,
    pub color: String,
    pub outline_color: String,
    pub outline_size: f64,
    pub shadow_offset: f64,
    pub background: SubBackground,
    pub position: u32,
    pub margin_y: u32,
    pub ass_override: AssOverride,
    pub fix_timing: bool,
    pub encoding: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SubAutoLoad {
    Off,
    Exact,
    Fuzzy,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SubBackground {
    None,
    Box,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AssOverride {
    No,
    Scale,
    Force,
    Strip,
}

impl AssOverride {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::Scale => "scale",
            Self::Force => "force",
            Self::Strip => "strip",
        }
    }
}

// ── Library ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Library {
    pub track_history: bool,
    pub scan_on_startup: bool,
    pub watch_folders: bool,
    pub group_series: bool,
    pub extra_extensions: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub recent_limit: u32,
}

// ── Network ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Network {
    pub use_ytdlp: bool,
    pub ytdlp_format: String,
    pub user_agent: String,
    pub proxy: String,
    pub cache_secs: u32,
    pub timeout_s: u32,
    pub auto_reconnect: bool,
    pub hls_bitrate: IntOrKeyword,
    pub load_remote_artwork: bool,
}

// ── Performance ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Performance {
    pub demuxer_cache_mb: u32,
    pub demuxer_back_cache_mb: u32,
    pub readahead_secs: u32,
    pub software_threads: u32,
    pub thumbnail_hwdec: String,
    pub waveform: WaveformMode,
    pub cache_limit_mb: u32,
    pub webkit_dmabuf: AutoOnOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WaveformMode {
    OnDemand,
    Always,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AutoOnOff {
    Auto,
    On,
    Off,
}

// ── Advanced (file only) ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct MpvSection {
    pub load_user_config: bool,
    /// Raw mpv options, applied last. Values may be strings, numbers, or booleans in the file.
    #[ts(type = "Record<string, string | number | boolean>")]
    pub options: BTreeMap<String, toml::Value>,
}

/// Inclusive numeric bounds for keys that have them (KEYBINDS_AND_CONFIG §5). A user value
/// outside its range falls back to the default.
pub const RANGES: &[(&str, f64, f64)] = &[
    ("interface.ui_scale", 0.8, 1.5),
    ("interface.autohide_delay_ms", 500.0, 10_000.0),
    ("interface.sidebar_width", 320.0, 480.0),
    ("interface.osd_duration_ms", 300.0, 5000.0),
    ("interface.pip_width", 240.0, 960.0),
    ("controls.seek_small_s", 0.1, 3600.0),
    ("controls.seek_medium_s", 0.1, 3600.0),
    ("controls.seek_large_s", 0.1, 3600.0),
    ("controls.volume_step", 0.1, 50.0),
    ("controls.speed_step", 0.01, 1.0),
    ("controls.sub_delay_step_ms", 1.0, 5000.0),
    ("controls.audio_delay_step_ms", 1.0, 5000.0),
    ("controls.sub_size_step", 0.01, 1.0),
    ("controls.zoom_step", 0.01, 1.0),
    ("playback.default_speed", 0.25, 4.0),
    ("playback.finished_percent", 80.0, 100.0),
    ("audio.default_volume", 0.0, 150.0),
    ("audio.max_volume", 100.0, 150.0),
    ("subtitles.font_size", 10.0, 120.0),
    ("subtitles.scale", 0.5, 3.0),
    ("subtitles.outline_size", 0.0, 10.0),
    ("subtitles.shadow_offset", 0.0, 10.0),
    ("subtitles.position", 0.0, 100.0),
    ("subtitles.margin_y", 0.0, 200.0),
    ("library.recent_limit", 0.0, 500.0),
    ("network.cache_secs", 1.0, 600.0),
    ("network.timeout_s", 1.0, 120.0),
    ("performance.demuxer_cache_mb", 32.0, 4096.0),
    ("performance.demuxer_back_cache_mb", 0.0, 2048.0),
    ("performance.readahead_secs", 1.0, 120.0),
    ("performance.software_threads", 0.0, 64.0),
    ("performance.cache_limit_mb", 64.0, 16_384.0),
];

/// Tables whose keys are free-form and therefore never reported as unknown.
pub const FREE_FORM_TABLES: &[&str] = &["mpv.options", "env"];

/// Colour keys, validated as `#RRGGBB`.
pub const COLOR_KEYS: &[&str] = &["interface.accent", "subtitles.color", "subtitles.outline_color"];
