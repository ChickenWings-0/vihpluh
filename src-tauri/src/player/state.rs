//! `PlayerState` mirroring: a pure reducer from mpv events to state (ARCHITECTURE §6.3–§6.4).
//!
//! The reducer never talks to mpv, so the whole playback state machine is unit-testable from
//! recorded event sequences.

use crate::cli::is_url;
use crate::config::Config;
use crate::config::schema::SubBackground;
use crate::ipc::dto::{
    AbLoop, Direction, EqState, MediaInfo, MediaKind, PlayerState, SelectedTracks, Status,
    SubStyle, TrackKind, VideoAdjust,
};
use crate::player::mpv::{EndReason, Event, Node};
use crate::player::tracks;

/// The raw inputs of the status state machine.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatusInputs {
    /// Between an open request (or `start-file`) and `file-loaded` / `end-file`.
    pub loading: bool,
    /// The last load failed and nothing has been opened since.
    pub error: bool,
    pub idle: bool,
    pub pause: bool,
    pub paused_for_cache: bool,
    pub seeking: bool,
    pub eof: bool,
    /// The core decided to hold the last frame after `Ended` (queue exhausted).
    pub hold: bool,
}

/// Derives the user-facing status. Order encodes priority.
pub fn derive_status(i: &StatusInputs) -> Status {
    if i.error {
        Status::Error
    } else if i.loading {
        Status::Loading
    } else if i.idle {
        Status::Idle
    } else if i.eof && !i.hold {
        Status::Ended
    } else if i.seeking {
        Status::Seeking
    } else if i.paused_for_cache {
        Status::Buffering
    } else if i.pause {
        Status::Paused
    } else {
        Status::Playing
    }
}

/// Playback position, kept out of `PlayerState` (it changes every frame).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TimeState {
    pub pos: f64,
    pub dur: f64,
    pub cache_end: f64,
}

/// What an event changed, for the core to act on.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Effects {
    /// `PlayerState` changed: a patch is due.
    pub state: bool,
    /// Position/duration/cache changed: a time event is due at the normal rate.
    pub time: bool,
    /// A time event is due immediately (seek, pause, load).
    pub time_now: bool,
    /// The file just reached its end (`eof-reached` rose).
    pub ended: bool,
    /// A file finished loading.
    pub loaded: bool,
    /// A load failed with this message.
    pub failed: Option<String>,
}

impl Effects {
    fn merge(&mut self, other: Effects) {
        self.state |= other.state;
        self.time |= other.time;
        self.time_now |= other.time_now;
        self.ended |= other.ended;
        self.loaded |= other.loaded;
        if other.failed.is_some() {
            self.failed = other.failed;
        }
    }
}

/// The mirrored state plus the raw mpv values it is derived from.
#[derive(Debug, Clone)]
pub struct Mirror {
    pub state: PlayerState,
    pub time: TimeState,
    inputs: StatusInputs,
    path: String,
    title: String,
    file_format: Option<String>,
    file_size: Option<i64>,
    panscan: f64,
    keepaspect: bool,
    aspect_override: String,
    hdr: Option<String>,
}

impl Mirror {
    pub fn new(config: &Config) -> Self {
        let subs = &config.subtitles;
        let state = PlayerState {
            rev: 0,
            status: Status::Idle,
            media: None,
            volume: config.audio.default_volume,
            muted: false,
            speed: config.playback.default_speed,
            direction: Direction::Forward,
            tracks: Vec::new(),
            selected: SelectedTracks::default(),
            chapters: Vec::new(),
            chapter: None,
            audio_delay_ms: 0,
            sub_delay_ms: 0,
            sub_visible: true,
            sub_style: SubStyle {
                font: subs.font.clone(),
                size: subs.font_size,
                scale: subs.scale,
                bold: subs.bold,
                color: subs.color.clone(),
                outline_color: subs.outline_color.clone(),
                outline_size: subs.outline_size,
                shadow_offset: subs.shadow_offset,
                background: match subs.background {
                    SubBackground::None => "none",
                    SubBackground::Box => "box",
                }
                .to_string(),
                background_color: "#000000".to_string(),
                position: f64::from(subs.position),
                margin_y: f64::from(subs.margin_y),
                ass_override: subs.ass_override.as_str().to_string(),
            },
            video: VideoAdjust {
                aspect: config.video.default_aspect.clone(),
                tone_mapping: config.video.tone_mapping.clone(),
                deband: config.video.deband,
                interpolation: config.video.interpolation,
                ..VideoAdjust::default()
            },
            eq: EqState::default(),
            normalize: config.audio.normalize,
            audio_device: config.audio.device.clone(),
            audio_devices: Vec::new(),
            loop_file: false,
            ab_loop: AbLoop::default(),
            sleep: None,
            hwdec: String::new(),
            applied_presets: Vec::new(),
            preset_dirty: false,
            error: None,
        };
        Self {
            state,
            time: TimeState::default(),
            inputs: StatusInputs { idle: true, ..StatusInputs::default() },
            path: String::new(),
            title: String::new(),
            file_format: None,
            file_size: None,
            panscan: 0.0,
            keepaspect: true,
            aspect_override: "no".to_string(),
            hdr: None,
        }
    }

    pub fn inputs(&self) -> &StatusInputs {
        &self.inputs
    }

    /// An open request was issued: show `loading` before mpv reports anything.
    pub fn begin_loading(&mut self) -> Effects {
        self.inputs.loading = true;
        self.inputs.error = false;
        self.inputs.hold = false;
        self.state.error = None;
        self.refresh_status()
    }

    /// The queue is exhausted and the last frame stays on screen: `Ended` → `Paused`.
    pub fn hold_at_end(&mut self) -> Effects {
        self.inputs.hold = true;
        self.refresh_status()
    }

    /// A load failed before mpv was involved (e.g. the file is missing).
    pub fn fail(&mut self, message: &str) -> Effects {
        self.inputs.loading = false;
        self.inputs.error = true;
        self.state.error = Some(message.to_string());
        let mut effects = self.refresh_status();
        effects.state = true;
        effects.failed = Some(message.to_string());
        effects
    }

    /// `stop`: also leaves the error state.
    pub fn clear_error(&mut self) -> Effects {
        self.inputs.error = false;
        self.inputs.loading = false;
        self.state.error = None;
        self.refresh_status()
    }

    pub fn on_event(&mut self, event: &Event) -> Effects {
        match event {
            Event::StartFile => {
                self.inputs.loading = true;
                self.inputs.error = false;
                self.inputs.hold = false;
                self.state.error = None;
                self.refresh_status()
            }
            Event::FileLoaded => {
                self.inputs.loading = false;
                let mut effects = self.refresh_status();
                effects.merge(self.refresh_media());
                effects.loaded = true;
                effects.time_now = true;
                effects
            }
            Event::EndFile { reason, error } => {
                self.inputs.loading = false;
                let mut effects = Effects::default();
                if *reason == EndReason::Error {
                    let message =
                        error.clone().unwrap_or_else(|| "the file could not be played".into());
                    self.inputs.error = true;
                    self.state.error = Some(message.clone());
                    effects.failed = Some(message);
                    effects.state = true;
                }
                effects.merge(self.refresh_status());
                effects
            }
            Event::Seek | Event::PlaybackRestart => {
                Effects { time_now: true, ..Effects::default() }
            }
            Event::PropertyChange { name, value, .. } => self.on_property(name, value),
            _ => Effects::default(),
        }
    }

    fn refresh_status(&mut self) -> Effects {
        let status = derive_status(&self.inputs);
        let changed = status != self.state.status;
        self.state.status = status;
        Effects { state: changed, time_now: changed, ..Effects::default() }
    }

    /// Rebuilds `state.media` from the raw path/title/format/size and the track list.
    fn refresh_media(&mut self) -> Effects {
        let media = if self.path.is_empty() || self.inputs.idle {
            None
        } else {
            let has_video =
                self.state.tracks.iter().any(|t| t.kind == TrackKind::Video && !t.image);
            let previous_key =
                self.state.media.as_ref().filter(|m| m.source == self.path).map(|m| m.key.clone());
            Some(MediaInfo {
                key: previous_key.unwrap_or_default(),
                source: self.path.clone(),
                is_url: is_url(&self.path),
                title: if self.title.is_empty() { self.path.clone() } else { self.title.clone() },
                kind: if has_video { MediaKind::Video } else { MediaKind::Audio },
                duration: self.time.dur,
                file_size: self.file_size,
                container: self.file_format.clone(),
                series: self
                    .state
                    .media
                    .as_ref()
                    .filter(|m| m.source == self.path)
                    .and_then(|m| m.series.clone()),
            })
        };
        let changed = media != self.state.media;
        self.state.media = media;
        Effects { state: changed, ..Effects::default() }
    }

    fn refresh_aspect(&mut self) -> bool {
        let aspect = if !self.keepaspect {
            "stretch".to_string()
        } else if self.panscan >= 0.999 {
            "fill".to_string()
        } else {
            aspect_name(&self.aspect_override)
        };
        set(&mut self.state.video.aspect, aspect)
    }

    fn apply_hdr(&mut self) -> bool {
        let selected = self.state.selected.video;
        let hdr = self.hdr.clone();
        let mut changed = false;
        for track in &mut self.state.tracks {
            if track.kind != TrackKind::Video || track.hdr.as_deref() == Some("dv") {
                continue;
            }
            let wanted = if Some(track.id) == selected { hdr.clone() } else { None };
            changed |= set(&mut track.hdr, wanted);
        }
        changed
    }

    fn on_property(&mut self, name: &str, value: &Node) -> Effects {
        let mut effects = Effects::default();
        let flag = value.as_bool().unwrap_or(false);
        let number = value.as_f64().unwrap_or(0.0);
        let text = value.as_str().unwrap_or("");
        let s = &mut self.state;

        let changed = match name {
            // ── Status inputs ───────────────────────────────────────────────
            "idle-active" => {
                self.inputs.idle = flag;
                if flag {
                    self.inputs.eof = false;
                    self.inputs.hold = false;
                }
                effects.merge(self.refresh_status());
                effects.merge(self.refresh_media());
                false
            }
            "pause" => {
                self.inputs.pause = flag;
                effects.merge(self.refresh_status());
                effects.time_now = true;
                false
            }
            "paused-for-cache" => {
                self.inputs.paused_for_cache = flag;
                effects.merge(self.refresh_status());
                false
            }
            "seeking" => {
                self.inputs.seeking = flag;
                effects.merge(self.refresh_status());
                false
            }
            "eof-reached" => {
                let rose = flag && !self.inputs.eof;
                self.inputs.eof = flag;
                if !flag {
                    self.inputs.hold = false;
                }
                effects.merge(self.refresh_status());
                effects.ended = rose;
                false
            }
            // ── Time ────────────────────────────────────────────────────────
            "time-pos" => {
                if matches!(value, Node::None) {
                    return effects;
                }
                self.time.pos = number;
                effects.time = true;
                false
            }
            "duration" => {
                self.time.dur = number;
                effects.time = true;
                effects.merge(self.refresh_media());
                false
            }
            "demuxer-cache-time" => {
                self.time.cache_end = number;
                effects.time = true;
                false
            }
            // ── Media ───────────────────────────────────────────────────────
            "path" => {
                self.path = text.to_string();
                effects.merge(self.refresh_media());
                false
            }
            "media-title" => {
                self.title = text.to_string();
                effects.merge(self.refresh_media());
                false
            }
            "file-format" => {
                self.file_format = (!text.is_empty()).then(|| text.to_string());
                effects.merge(self.refresh_media());
                false
            }
            "file-size" => {
                self.file_size = value.as_i64();
                effects.merge(self.refresh_media());
                false
            }
            // ── Tracks ──────────────────────────────────────────────────────
            "track-list" => {
                let changed = set(&mut s.tracks, tracks::parse_tracks(value));
                let hdr = self.apply_hdr();
                effects.merge(self.refresh_media());
                changed || hdr
            }
            "vid" => {
                let changed = set(&mut s.selected.video, value.as_i64());
                changed | self.apply_hdr()
            }
            "aid" => set(&mut s.selected.audio, value.as_i64()),
            "sid" => set(&mut s.selected.sub, value.as_i64()),
            "secondary-sid" => set(&mut s.selected.sub2, value.as_i64()),
            "chapter-list" => set(&mut s.chapters, tracks::parse_chapters(value)),
            "chapter" => set(&mut s.chapter, value.as_i64().filter(|c| *c >= 0)),
            // ── Transport ───────────────────────────────────────────────────
            "volume" => set(&mut s.volume, number),
            "mute" => set(&mut s.muted, flag),
            "speed" => set(&mut s.speed, number),
            "play-direction" => set(
                &mut s.direction,
                if text == "backward" { Direction::Backward } else { Direction::Forward },
            ),
            "loop-file" => {
                // "no" arrives as a false flag or the string "no"; anything else loops.
                let looping = match value {
                    Node::Flag(b) => *b,
                    Node::String(t) => t != "no",
                    Node::Int(n) => *n > 0,
                    _ => false,
                };
                set(&mut s.loop_file, looping)
            }
            "ab-loop-a" => set(&mut s.ab_loop.a, loop_point(value)),
            "ab-loop-b" => set(&mut s.ab_loop.b, loop_point(value)),
            // ── Sync ────────────────────────────────────────────────────────
            "audio-delay" => set(&mut s.audio_delay_ms, seconds_to_ms(number)),
            "sub-delay" => set(&mut s.sub_delay_ms, seconds_to_ms(number)),
            "sub-visibility" => set(&mut s.sub_visible, flag),
            // ── Audio output ────────────────────────────────────────────────
            "audio-device" => set(&mut s.audio_device, text.to_string()),
            "audio-device-list" => set(&mut s.audio_devices, tracks::parse_audio_devices(value)),
            // ── Picture ─────────────────────────────────────────────────────
            "brightness" => set(&mut s.video.brightness, number),
            "contrast" => set(&mut s.video.contrast, number),
            "saturation" => set(&mut s.video.saturation, number),
            "gamma" => set(&mut s.video.gamma, number),
            "hue" => set(&mut s.video.hue, number),
            "video-rotate" => {
                let degrees = value.as_i64().unwrap_or(0).rem_euclid(360) as u32;
                set(&mut s.video.rotate, degrees)
            }
            "video-zoom" => set(&mut s.video.zoom, number),
            "video-pan-x" => set(&mut s.video.pan_x, number),
            "video-pan-y" => set(&mut s.video.pan_y, number),
            "video-aspect-override" => {
                self.aspect_override = text.to_string();
                self.refresh_aspect()
            }
            "panscan" => {
                self.panscan = number;
                self.refresh_aspect()
            }
            "keepaspect" => {
                self.keepaspect = flag;
                self.refresh_aspect()
            }
            "tone-mapping" => set(&mut s.video.tone_mapping, text.to_string()),
            "deband" => set(&mut s.video.deband, flag),
            "interpolation" => set(&mut s.video.interpolation, flag),
            "glsl-shaders" => {
                let shaders: Vec<String> = value
                    .as_array()
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(|n| n.as_str().map(str::to_string))
                    .collect();
                let flip_h = shaders.iter().any(|p| p.ends_with("flip_h.glsl"));
                let flip_v = shaders.iter().any(|p| p.ends_with("flip_v.glsl"));
                let user: Vec<String> = shaders
                    .into_iter()
                    .filter(|p| !p.ends_with("flip_h.glsl") && !p.ends_with("flip_v.glsl"))
                    .collect();
                set(&mut s.video.flip_h, flip_h)
                    | set(&mut s.video.flip_v, flip_v)
                    | set(&mut s.video.shaders, user)
            }
            "video-params" => {
                self.hdr = tracks::hdr_class(value);
                self.apply_hdr()
            }
            "hwdec-current" => set(&mut s.hwdec, text.to_string()),
            // ── Subtitle style ──────────────────────────────────────────────
            "sub-font" => set(&mut s.sub_style.font, text.to_string()),
            "sub-font-size" => set(&mut s.sub_style.size, number),
            "sub-scale" => set(&mut s.sub_style.scale, number),
            "sub-bold" => set(&mut s.sub_style.bold, flag),
            "sub-color" => set(&mut s.sub_style.color, rgb(text)),
            "sub-outline-color" => set(&mut s.sub_style.outline_color, rgb(text)),
            "sub-outline-size" => set(&mut s.sub_style.outline_size, number),
            "sub-shadow-offset" => set(&mut s.sub_style.shadow_offset, number),
            "sub-border-style" => set(
                &mut s.sub_style.background,
                if text == "outline-and-shadow" { "none" } else { "box" }.to_string(),
            ),
            "sub-back-color" => set(&mut s.sub_style.background_color, rgb(text)),
            "sub-pos" => set(&mut s.sub_style.position, number),
            "sub-margin-y" => set(&mut s.sub_style.margin_y, number),
            "sub-ass-override" => {
                // vihpluh's four modes are no/scale/force/strip; mpv's extra "yes" (apply the
                // style overrides without the size multiplier) is closest to "scale".
                let mode = if text == "yes" { "scale" } else { text };
                set(&mut s.sub_style.ass_override, mode.to_string())
            }
            _ => false,
        };
        effects.state |= changed;
        effects
    }
}

/// The Video tab's name for an `video-aspect-override` value. mpv reports ratios back as
/// decimals (`16:9` → `1.777778`) and "no" as a number ≤ 0 (−1 or −2 by version).
fn aspect_name(value: &str) -> String {
    const NAMED: [(&str, f64); 5] = [
        ("16:9", 16.0 / 9.0),
        ("4:3", 4.0 / 3.0),
        ("21:9", 21.0 / 9.0),
        ("2.39", 2.39),
        ("1:1", 1.0),
    ];
    let ratio = match value.split_once(':') {
        Some((w, h)) => {
            w.trim().parse::<f64>().ok().zip(h.trim().parse::<f64>().ok()).map(|(w, h)| w / h)
        }
        None => value.trim().parse::<f64>().ok(),
    };
    match ratio {
        None if value == "no" || value.is_empty() => "auto".to_string(),
        None => value.to_string(),
        Some(r) if !r.is_finite() || r <= 0.0 => "auto".to_string(),
        Some(r) => NAMED
            .iter()
            .find(|(_, named)| (r - named).abs() < 0.005)
            .map_or_else(|| format!("{r:.2}"), |(name, _)| (*name).to_string()),
    }
}

fn set<T: PartialEq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        false
    } else {
        *slot = value;
        true
    }
}

fn seconds_to_ms(seconds: f64) -> i32 {
    (seconds * 1000.0).round() as i32
}

/// `ab-loop-a/b` is a number, or "no" when unset.
fn loop_point(value: &Node) -> Option<f64> {
    match value {
        Node::Double(d) => Some(*d),
        Node::Int(i) => Some(*i as f64),
        _ => None,
    }
}

/// mpv prints colours as `#AARRGGBB`; the UI uses `#RRGGBB`.
fn rgb(color: &str) -> String {
    if color.len() == 9 && color.starts_with('#') {
        format!("#{}", &color[3..])
    } else {
        color.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;

    fn prop(name: &str, value: Node) -> Event {
        Event::PropertyChange { id: 0, name: name.to_string(), value }
    }

    fn flag(name: &str, value: bool) -> Event {
        prop(name, Node::Flag(value))
    }

    fn text(name: &str, value: &str) -> Event {
        prop(name, Node::String(value.to_string()))
    }

    fn mirror() -> Mirror {
        Mirror::new(&config::defaults())
    }

    /// Feeds events and returns the status after each one that changed it.
    fn statuses(mirror: &mut Mirror, events: &[Event]) -> Vec<Status> {
        let mut seen = Vec::new();
        for event in events {
            let before = mirror.state.status;
            mirror.on_event(event);
            if mirror.state.status != before {
                seen.push(mirror.state.status);
            }
        }
        seen
    }

    /// The order mpv delivers events in when a file is opened from idle.
    fn load_sequence(path: &str) -> Vec<Event> {
        vec![
            Event::StartFile,
            flag("idle-active", false),
            text("path", path),
            text("media-title", "Episode 1"),
            prop("duration", Node::Double(1420.0)),
            Event::FileLoaded,
            flag("seeking", true),
            Event::Seek,
            flag("seeking", false),
            Event::PlaybackRestart,
        ]
    }

    #[test]
    fn status_priority_order() {
        let playing = StatusInputs::default();
        assert_eq!(derive_status(&playing), Status::Playing);
        assert_eq!(derive_status(&StatusInputs { pause: true, ..playing.clone() }), Status::Paused);
        assert_eq!(
            derive_status(&StatusInputs { pause: true, paused_for_cache: true, ..playing.clone() }),
            Status::Buffering
        );
        assert_eq!(
            derive_status(&StatusInputs { pause: true, seeking: true, ..playing.clone() }),
            Status::Seeking
        );
        assert_eq!(
            derive_status(&StatusInputs { eof: true, pause: true, ..playing.clone() }),
            Status::Ended
        );
        assert_eq!(
            derive_status(&StatusInputs { eof: true, pause: true, hold: true, ..playing.clone() }),
            Status::Paused,
            "holding the last frame reads as paused"
        );
        assert_eq!(derive_status(&StatusInputs { idle: true, ..playing.clone() }), Status::Idle);
        assert_eq!(
            derive_status(&StatusInputs { idle: true, loading: true, ..playing.clone() }),
            Status::Loading
        );
        assert_eq!(
            derive_status(&StatusInputs { idle: true, loading: true, error: true, ..playing }),
            Status::Error
        );
    }

    #[test]
    fn idle_to_loading_to_playing() {
        let mut m = mirror();
        assert_eq!(m.state.status, Status::Idle);
        m.begin_loading();
        assert_eq!(m.state.status, Status::Loading, "the open request shows at once");
        let seen = statuses(&mut m, &load_sequence("D:/Shows/e1.mkv"));
        // The recorded sequence includes the initial seek mpv does after loading.
        assert_eq!(seen, vec![Status::Playing, Status::Seeking, Status::Playing]);
        let media = m.state.media.clone().unwrap();
        assert_eq!(media.source, "D:/Shows/e1.mkv");
        assert_eq!(media.title, "Episode 1");
        assert_eq!(media.duration, 1420.0);
        assert!(!media.is_url);
    }

    #[test]
    fn loading_paused_then_play_pause() {
        let mut m = mirror();
        m.on_event(&flag("pause", true));
        m.begin_loading();
        let seen = statuses(&mut m, &load_sequence("a.mkv"));
        assert_eq!(seen.last(), Some(&Status::Paused), "--paused / session restore");
        assert_eq!(statuses(&mut m, &[flag("pause", false)]), vec![Status::Playing]);
        assert_eq!(statuses(&mut m, &[flag("pause", true)]), vec![Status::Paused]);
    }

    #[test]
    fn seeking_returns_to_the_state_it_came_from() {
        let mut m = mirror();
        statuses(&mut m, &load_sequence("a.mkv"));
        let seek =
            [flag("seeking", true), Event::Seek, flag("seeking", false), Event::PlaybackRestart];
        assert_eq!(statuses(&mut m, &seek), vec![Status::Seeking, Status::Playing]);
        m.on_event(&flag("pause", true));
        assert_eq!(statuses(&mut m, &seek), vec![Status::Seeking, Status::Paused]);
    }

    #[test]
    fn buffering_while_the_cache_refills() {
        let mut m = mirror();
        statuses(&mut m, &load_sequence("https://example.com/live.m3u8"));
        assert!(m.state.media.as_ref().unwrap().is_url);
        assert_eq!(
            statuses(&mut m, &[flag("paused-for-cache", true), flag("paused-for-cache", false)]),
            vec![Status::Buffering, Status::Playing]
        );
    }

    #[test]
    fn end_of_file_then_hold_or_idle() {
        let mut m = mirror();
        statuses(&mut m, &load_sequence("a.mkv"));
        // keep-open: mpv pauses on the last frame and raises eof-reached.
        let effects = m.on_event(&flag("eof-reached", true));
        assert!(effects.ended, "the rising edge is reported once");
        m.on_event(&flag("pause", true));
        assert_eq!(m.state.status, Status::Ended);
        assert!(!m.on_event(&flag("eof-reached", true)).ended, "no second report");

        m.hold_at_end();
        assert_eq!(m.state.status, Status::Paused, "at_queue_end = hold");

        // Seeking back out of the end clears both eof and the hold.
        statuses(&mut m, &[flag("eof-reached", false), flag("pause", false)]);
        assert_eq!(m.state.status, Status::Playing);
        assert!(!m.inputs().hold);

        // at_queue_end = stop: mpv goes idle.
        m.on_event(&flag("eof-reached", true));
        let seen = statuses(
            &mut m,
            &[Event::EndFile { reason: EndReason::Stop, error: None }, flag("idle-active", true)],
        );
        assert_eq!(seen, vec![Status::Idle]);
        assert_eq!(m.state.media, None);
    }

    #[test]
    fn a_failed_load_is_an_error_until_something_else_is_opened() {
        let mut m = mirror();
        m.begin_loading();
        m.on_event(&Event::StartFile);
        let effects = m.on_event(&Event::EndFile {
            reason: EndReason::Error,
            error: Some("unrecognized file format".into()),
        });
        assert_eq!(effects.failed.as_deref(), Some("unrecognized file format"));
        m.on_event(&flag("idle-active", true));
        assert_eq!(m.state.status, Status::Error, "idle does not hide the error");
        assert_eq!(m.state.error.as_deref(), Some("unrecognized file format"));

        m.begin_loading();
        assert_eq!(m.state.status, Status::Loading);
        assert_eq!(m.state.error, None);
        statuses(&mut m, &load_sequence("good.mkv"));
        assert_eq!(m.state.status, Status::Playing);
    }

    #[test]
    fn a_failure_before_mpv_is_an_error_too() {
        let mut m = mirror();
        m.begin_loading();
        let effects = m.fail("file not found");
        assert_eq!(m.state.status, Status::Error);
        assert_eq!(effects.failed.as_deref(), Some("file not found"));
        m.begin_loading();
        assert_eq!(m.state.status, Status::Loading);
    }

    #[test]
    fn stop_leaves_the_error_state() {
        let mut m = mirror();
        m.on_event(&Event::StartFile);
        m.on_event(&Event::EndFile { reason: EndReason::Error, error: None });
        assert_eq!(m.state.status, Status::Error);
        m.clear_error();
        assert_eq!(m.state.status, Status::Idle);
    }

    #[test]
    fn stop_while_playing_goes_idle() {
        let mut m = mirror();
        statuses(&mut m, &load_sequence("a.mkv"));
        let seen = statuses(
            &mut m,
            &[Event::EndFile { reason: EndReason::Stop, error: None }, flag("idle-active", true)],
        );
        assert_eq!(seen, vec![Status::Idle]);
    }

    #[test]
    fn properties_mirror_into_state_and_report_changes_once() {
        let mut m = mirror();
        assert!(m.on_event(&prop("volume", Node::Double(85.0))).state);
        assert!(!m.on_event(&prop("volume", Node::Double(85.0))).state, "no change, no patch");
        assert_eq!(m.state.volume, 85.0);

        m.on_event(&flag("mute", true));
        m.on_event(&prop("speed", Node::Double(1.5)));
        m.on_event(&prop("sub-delay", Node::Double(-0.35)));
        m.on_event(&prop("audio-delay", Node::Double(0.1004)));
        m.on_event(&text("play-direction", "backward"));
        m.on_event(&text("hwdec-current", "d3d11va"));
        assert!(m.state.muted);
        assert_eq!(m.state.speed, 1.5);
        assert_eq!(m.state.sub_delay_ms, -350);
        assert_eq!(m.state.audio_delay_ms, 100);
        assert_eq!(m.state.direction, Direction::Backward);
        assert_eq!(m.state.hwdec, "d3d11va");
    }

    #[test]
    fn track_selection_handles_no_and_auto() {
        let mut m = mirror();
        m.on_event(&prop("aid", Node::Int(2)));
        m.on_event(&prop("sid", Node::Flag(false)));
        m.on_event(&prop("vid", Node::String("auto".into())));
        assert_eq!(m.state.selected.audio, Some(2));
        assert_eq!(m.state.selected.sub, None);
        assert_eq!(m.state.selected.video, None);
    }

    #[test]
    fn loop_and_ab_loop_values() {
        let mut m = mirror();
        m.on_event(&text("loop-file", "inf"));
        assert!(m.state.loop_file);
        m.on_event(&prop("loop-file", Node::Flag(false)));
        assert!(!m.state.loop_file);
        m.on_event(&prop("ab-loop-a", Node::Double(12.5)));
        m.on_event(&text("ab-loop-b", "no"));
        assert_eq!(m.state.ab_loop, AbLoop { a: Some(12.5), b: None });
    }

    #[test]
    fn aspect_combines_override_panscan_and_keepaspect() {
        let mut m = mirror();
        assert_eq!(m.state.video.aspect, "auto");
        m.on_event(&text("video-aspect-override", "16:9"));
        assert_eq!(m.state.video.aspect, "16:9");
        m.on_event(&text("video-aspect-override", "2.39:1"));
        assert_eq!(m.state.video.aspect, "2.39");
        m.on_event(&prop("panscan", Node::Double(1.0)));
        assert_eq!(m.state.video.aspect, "fill");
        m.on_event(&flag("keepaspect", false));
        assert_eq!(m.state.video.aspect, "stretch");
        m.on_event(&flag("keepaspect", true));
        m.on_event(&prop("panscan", Node::Double(0.0)));
        m.on_event(&text("video-aspect-override", "no"));
        assert_eq!(m.state.video.aspect, "auto");
        m.on_event(&text("video-aspect-override", "16:9"));
        m.on_event(&text("video-aspect-override", "-2.000000"));
        assert_eq!(m.state.video.aspect, "auto", "mpv 0.42 reports `no` as -2");
        m.on_event(&text("video-aspect-override", "1.777778"));
        assert_eq!(m.state.video.aspect, "16:9", "ratios come back as decimals");
        m.on_event(&text("video-aspect-override", "2.390000"));
        assert_eq!(m.state.video.aspect, "2.39");
        m.on_event(&text("video-aspect-override", "1.850000"));
        assert_eq!(m.state.video.aspect, "1.85");
    }

    #[test]
    fn flip_shaders_are_separated_from_user_shaders() {
        let mut m = mirror();
        m.on_event(&prop(
            "glsl-shaders",
            Node::Array(vec![
                Node::String("C:/app/resources/shaders/flip_h.glsl".into()),
                Node::String("C:/cfg/shaders/Anime4K.glsl".into()),
            ]),
        ));
        assert!(m.state.video.flip_h);
        assert!(!m.state.video.flip_v);
        assert_eq!(m.state.video.shaders, vec!["C:/cfg/shaders/Anime4K.glsl"]);
    }

    #[test]
    fn media_kind_follows_the_track_list() {
        let mut m = mirror();
        m.on_event(&flag("idle-active", false));
        m.on_event(&text("path", "song.flac"));
        assert_eq!(m.state.media.as_ref().unwrap().kind, MediaKind::Audio);
        let video = Node::Map(vec![
            ("id".into(), Node::Int(1)),
            ("type".into(), Node::String("video".into())),
            ("codec".into(), Node::String("h264".into())),
        ]);
        m.on_event(&prop("track-list", Node::Array(vec![video])));
        assert_eq!(m.state.media.as_ref().unwrap().kind, MediaKind::Video);
    }

    #[test]
    fn hdr_is_reported_on_the_selected_video_track() {
        let mut m = mirror();
        let track = |id: i64| {
            Node::Map(vec![
                ("id".into(), Node::Int(id)),
                ("type".into(), Node::String("video".into())),
                ("codec".into(), Node::String("hevc".into())),
            ])
        };
        m.on_event(&prop("track-list", Node::Array(vec![track(1), track(2)])));
        m.on_event(&prop("vid", Node::Int(1)));
        m.on_event(&prop(
            "video-params",
            Node::Map(vec![("gamma".into(), Node::String("pq".into()))]),
        ));
        assert_eq!(m.state.tracks[0].hdr.as_deref(), Some("hdr10"));
        assert_eq!(m.state.tracks[1].hdr, None);
    }

    #[test]
    fn time_properties_do_not_touch_player_state() {
        let mut m = mirror();
        let effects = m.on_event(&prop("time-pos", Node::Double(12.5)));
        assert!(effects.time && !effects.state);
        assert_eq!(m.time.pos, 12.5);
        let effects = m.on_event(&prop("time-pos", Node::None));
        assert!(!effects.time, "an unavailable position keeps the last value");
        assert_eq!(m.time.pos, 12.5);
        m.on_event(&prop("demuxer-cache-time", Node::Double(40.0)));
        assert_eq!(m.time.cache_end, 40.0);
    }

    #[test]
    fn pause_and_seek_request_an_immediate_time_event() {
        let mut m = mirror();
        assert!(m.on_event(&flag("pause", true)).time_now);
        assert!(m.on_event(&Event::Seek).time_now);
        assert!(m.on_event(&Event::PlaybackRestart).time_now);
    }

    #[test]
    fn colours_drop_the_alpha_channel() {
        assert_eq!(rgb("#FFFFFFFF"), "#FFFFFF");
        assert_eq!(rgb("#C0000000"), "#000000");
        assert_eq!(rgb("#FFAA00"), "#FFAA00");
    }

    #[test]
    fn ass_override_maps_mpvs_default() {
        let mut m = mirror();
        m.on_event(&text("sub-ass-override", "yes"));
        assert_eq!(m.state.sub_style.ass_override, "scale");
        m.on_event(&text("sub-ass-override", "strip"));
        assert_eq!(m.state.sub_style.ass_override, "strip");
    }
}
