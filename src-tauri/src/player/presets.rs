//! Presets in the core (ARCHITECTURE §6.5 steps 2 and 4, §10.4): which presets a file gets,
//! applying their layered settings when it loads, noticing when the live values have moved
//! away from them, and turning live values back into a preset (capture, update).
//!
//! Settings travel as flat maps from dotted key to value (`presets::Flat`), so layering,
//! comparing, and diffing are the same few lines for every setting.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};

use crate::config::Config;
use crate::ipc::dto::{PlayerState, SubStylePatch, Track, TrackKind, VideoPatch};
use crate::presets::rules::{SubChoice, TrackRule};
use crate::presets::{
    AudioSection, Flat, Layer, PlaybackSection, Preset, PresetMatch, StyleSection, SubSection,
    VideoSection, matcher,
};

use super::mpv::Value;
use super::{Core, tracks};

const AUDIO_TRACK: &str = "audio.track";
const SUB_TRACK: &str = "subtitles.track";
/// Property changes echo back from mpv a moment after they are written; the live values are
/// not compared with the presets until then.
const SETTLE: Duration = Duration::from_millis(500);

/// What the core knows about presets and the loaded file.
#[derive(Debug, Default)]
pub(super) struct PresetState {
    /// What the applied presets set (kept across files to know what to put back).
    layer: Layer,
    /// The live values right after the presets were applied. `None` between files.
    expected: Option<Flat>,
    applied_at: Option<Instant>,
    /// A comparison is due once the writes have settled.
    recheck: Option<Instant>,
    /// "Detach for this file".
    detached: bool,
    /// `--preset` / `preset_apply`: applied on top of whatever matches, for this file.
    forced: Option<String>,
    /// A preset override of volume normalisation (the setting itself is not written).
    pub normalize: Option<bool>,
}

impl PresetState {
    pub fn recheck_due(&self) -> Option<Instant> {
        self.recheck
    }
}

/// What a capture or update request gets back.
pub type PresetReply = Sender<Result<Preset, String>>;

#[derive(Debug)]
pub struct CaptureRequest {
    pub name: String,
    pub matcher: PresetMatch,
    /// Any of `audio`, `subtitles`, `video`, `playback`.
    pub sections: Vec<String>,
}

fn file_name(path: &str) -> String {
    path.replace('\\', "/").rsplit('/').next().unwrap_or(path).to_string()
}

fn style_section(style: &crate::ipc::dto::SubStyle) -> StyleSection {
    StyleSection {
        font: Some(style.font.clone()),
        size: Some(style.size),
        scale: Some(style.scale),
        bold: Some(style.bold),
        color: Some(style.color.clone()),
        outline_color: Some(style.outline_color.clone()),
        outline_size: Some(style.outline_size),
        shadow_offset: Some(style.shadow_offset),
        background: Some(style.background.clone()),
        // Without a box the colour is whatever mpv last held; it is not a setting then.
        background_color: (style.background == "box").then(|| style.background_color.clone()),
        position: Some(style.position),
        margin_y: Some(style.margin_y),
        ass_override: Some(style.ass_override.clone()),
    }
}

fn style_patch(style: &StyleSection) -> SubStylePatch {
    SubStylePatch {
        font: style.font.clone(),
        size: style.size,
        scale: style.scale,
        bold: style.bold,
        color: style.color.clone(),
        outline_color: style.outline_color.clone(),
        outline_size: style.outline_size,
        shadow_offset: style.shadow_offset,
        background: style.background.clone(),
        background_color: style.background_color.clone(),
        position: style.position,
        margin_y: style.margin_y,
        ass_override: style.ass_override.clone(),
    }
}

fn video_section(video: &crate::ipc::dto::VideoAdjust) -> VideoSection {
    VideoSection {
        brightness: Some(video.brightness),
        contrast: Some(video.contrast),
        saturation: Some(video.saturation),
        gamma: Some(video.gamma),
        hue: Some(video.hue),
        rotate: Some(video.rotate),
        flip_h: Some(video.flip_h),
        flip_v: Some(video.flip_v),
        zoom: Some(video.zoom),
        pan_x: Some(video.pan_x),
        pan_y: Some(video.pan_y),
        aspect: Some(video.aspect.clone()),
        tone_mapping: Some(video.tone_mapping.clone()),
        deband: Some(video.deband),
        interpolation: Some(video.interpolation),
        shaders: Some(video.shaders.iter().map(|path| file_name(path)).collect()),
    }
}

fn video_patch(video: &VideoSection, shader_dir: &std::path::Path) -> VideoPatch {
    VideoPatch {
        brightness: video.brightness,
        contrast: video.contrast,
        saturation: video.saturation,
        gamma: video.gamma,
        hue: video.hue,
        rotate: video.rotate,
        flip_h: video.flip_h,
        flip_v: video.flip_v,
        zoom: video.zoom,
        pan_x: video.pan_x,
        pan_y: video.pan_y,
        aspect: video.aspect.clone(),
        tone_mapping: video.tone_mapping.clone(),
        deband: video.deband,
        interpolation: video.interpolation,
        shaders: video.shaders.as_ref().map(|names| {
            names.iter().map(|name| shader_dir.join(name).display().to_string()).collect()
        }),
    }
}

/// The EQ as a preset names it: `off`, a preset id, or `custom` for an unsaved curve.
fn equalizer_name(state: &PlayerState) -> String {
    match (&state.eq.preset, state.eq.enabled) {
        (_, false) => "off".to_string(),
        (Some(id), true) => id.clone(),
        (None, true) => "custom".to_string(),
    }
}

/// Every preset-able setting as it is right now. Tracks appear as ids (`"off"` for no
/// subtitles), not rules: this is what the live state is compared by.
pub fn live_flat(state: &PlayerState) -> Flat {
    let layer = Layer {
        audio: AudioSection {
            track: None,
            delay_ms: Some(state.audio_delay_ms),
            equalizer: Some(equalizer_name(state)),
            normalize: Some(state.normalize),
        },
        subtitles: SubSection {
            track: None,
            visible: Some(state.sub_visible),
            delay_ms: Some(state.sub_delay_ms),
            style: style_section(&state.sub_style),
        },
        video: video_section(&state.video),
        playback: PlaybackSection { speed: Some(state.speed) },
    };
    let mut flat = layer.flat();
    if let Some(id) = state.selected.audio {
        flat.insert(AUDIO_TRACK.into(), json!(id));
    }
    flat.insert(SUB_TRACK.into(), state.selected.sub.map_or(json!("off"), |id| json!(id)));
    flat
}

/// What every setting goes back to when no preset sets it: the configured defaults.
pub fn defaults_layer(config: &Config) -> Layer {
    let mut mirror = super::state::Mirror::new(config);
    mirror.state.eq.enabled = config.audio.equalizer_enabled;
    mirror.state.eq.preset = Some(config.audio.equalizer_preset.clone());
    Layer::from_flat(
        &live_flat(&mirror.state)
            .into_iter()
            .filter(|(key, _)| key != AUDIO_TRACK && key != SUB_TRACK)
            .collect(),
    )
    .unwrap_or_default()
}

/// Whether two setting values are the same: numbers within a thousandth, text ignoring case
/// (colours come back from mpv in capitals).
fn same(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Number(x), Json::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() < 1e-3,
            _ => x == y,
        },
        (Json::String(x), Json::String(y)) => x.eq_ignore_ascii_case(y),
        (Json::Array(x), Json::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        _ => a == b,
    }
}

/// The keys whose live value differs from `expected`.
pub fn changed_keys(live: &Flat, expected: &Flat) -> Vec<String> {
    expected
        .iter()
        .filter(|(key, value)| live.get(*key).is_none_or(|now| !same(now, value)))
        .map(|(key, _)| key.clone())
        .chain(live.keys().filter(|key| !expected.contains_key(*key)).cloned())
        .collect()
}

/// Turns live values into what a preset file holds: track ids become rules, an unsaved EQ
/// curve is left out (a preset can only name a saved one).
fn preset_values(keys: &[String], live: &Flat, tracks: &[Track]) -> Flat {
    let mut out = Flat::new();
    for key in keys {
        let Some(value) = live.get(key) else { continue };
        let rule = |kind: TrackKind| {
            let track = tracks.iter().find(|t| t.kind == kind && Some(t.id) == value.as_i64())?;
            serde_json::to_value(TrackRule::for_track(track, tracks)).ok()
        };
        let stored = match key.as_str() {
            AUDIO_TRACK => rule(TrackKind::Audio),
            SUB_TRACK if value == "off" => Some(json!("off")),
            SUB_TRACK => rule(TrackKind::Sub),
            "audio.equalizer" if value == "custom" => None,
            _ => Some(value.clone()),
        };
        if let Some(stored) = stored {
            out.insert(key.clone(), stored);
        }
    }
    out
}

/// A preset made from the difference between the live values and the defaults, limited to
/// the chosen sections (§10.4 "Capture").
pub fn capture(
    request: &CaptureRequest,
    id: String,
    state: &PlayerState,
    defaults: &Layer,
) -> Result<Preset, String> {
    let live = live_flat(state);
    let base = defaults.flat();
    let in_section =
        |key: &String| request.sections.iter().any(|s| key.starts_with(&format!("{s}.")));
    let audio_tracks = state.tracks.iter().filter(|t| t.kind == TrackKind::Audio).count();
    let keys: Vec<String> = live
        .iter()
        .filter(|(key, _)| in_section(key))
        .filter(|(key, value)| match key.as_str() {
            // A track is worth a rule when there was something to choose between.
            AUDIO_TRACK => audio_tracks > 1,
            SUB_TRACK => *value != "off",
            _ => base.get(*key).is_none_or(|default| !same(value, default)),
        })
        .map(|(key, _)| key.clone())
        .collect();
    let layer = Layer::from_flat(&preset_values(&keys, &live, &state.tracks))?;
    let preset = Preset {
        schema: crate::presets::SCHEMA_VERSION,
        id,
        name: request.name.trim().to_string(),
        enabled: true,
        priority: 0,
        matcher: request.matcher.clone(),
        layer,
    };
    preset.validate()?;
    Ok(preset)
}

impl Core {
    fn shader_dir(&self) -> PathBuf {
        self.presets.dir().parent().map(|p| p.join("shaders")).unwrap_or_default()
    }

    /// A new load is starting: nothing is compared until its presets have been applied.
    pub(super) fn presets_begin_load(&mut self, forced: Option<String>) {
        self.preset.expected = None;
        self.preset.recheck = None;
        self.preset.detached = false;
        self.preset.forced = forced;
        self.set_preset_marks(Vec::new(), false);
    }

    fn set_preset_marks(&mut self, ids: Vec<String>, dirty: bool) {
        let state = &mut self.mirror.state;
        if state.applied_presets != ids || state.preset_dirty != dirty {
            state.applied_presets = ids;
            state.preset_dirty = dirty;
            self.dirty = true;
        }
    }

    /// Works out which presets apply to the loaded file and applies them: settings the last
    /// file's presets set and these do not go back to their defaults first.
    pub(super) fn apply_presets(&mut self) {
        let Some(source) = self.mirror.state.media.as_ref().map(|m| m.source.clone()) else {
            return;
        };
        let all = self.presets.list();
        let mut hits: Vec<&Preset> =
            if self.preset.detached || !self.config.playback.presets_enabled {
                Vec::new()
            } else {
                matcher::matching(&all, &source).into_iter().map(|(preset, _)| preset).collect()
            };
        if let Some(id) = self.preset.forced.clone() {
            match all.iter().find(|p| p.id == id) {
                Some(preset) => {
                    hits.retain(|hit| hit.id != id);
                    hits.push(preset);
                }
                None => tracing::warn!("there is no preset named `{id}`"),
            }
        }
        let layer = matcher::layered(&hits);
        let new_keys = layer.flat();
        let dropped: Vec<String> = self
            .preset
            .layer
            .flat()
            .into_keys()
            .filter(|key| !new_keys.contains_key(key) && key != AUDIO_TRACK && key != SUB_TRACK)
            .collect();
        let reset = defaults_layer(&self.config).only(&dropped);
        let tracks =
            self.mpv.get_node("track-list").map(|n| tracks::parse_tracks(&n)).unwrap_or_default();

        let mut expected = live_flat(&self.mirror.state);
        // Selections made while the file opened may not have echoed back yet.
        if let Some(aid) = self.mpv.get_i64("aid") {
            expected.insert(AUDIO_TRACK.into(), json!(aid));
        }
        let sid = self.mpv.get_i64("sid");
        expected.insert(SUB_TRACK.into(), sid.map_or(json!("off"), |id| json!(id)));

        self.write_layer(&reset, &tracks, &mut expected);
        self.write_layer(&layer, &tracks, &mut expected);
        // No preset asks for normalisation either way any more: the setting decides again.
        if layer.audio.normalize.is_none() && self.preset.normalize.take().is_some() {
            self.sync_normalize();
            expected.insert("audio.normalize".into(), json!(self.config.audio.normalize));
        }

        let ids: Vec<String> = hits.iter().map(|p| p.id.clone()).collect();
        tracing::debug!(?ids, "presets applied");
        self.preset.layer = layer;
        self.preset.expected = Some(expected);
        self.preset.applied_at = Some(Instant::now());
        self.preset.recheck = Some(Instant::now() + SETTLE);
        self.set_preset_marks(ids, false);
    }

    /// Writes a layer's settings to mpv and records in `expected` what the live values
    /// should become.
    fn write_layer(&mut self, layer: &Layer, tracks: &[Track], expected: &mut Flat) {
        for (key, value) in layer.flat() {
            if key != AUDIO_TRACK && key != SUB_TRACK {
                expected.insert(key, value);
            }
        }
        let audio = &layer.audio;
        if let Some(rule) = &audio.track {
            match rule.resolve(tracks, TrackKind::Audio) {
                Some(id) => {
                    self.set_property("aid", Value::Int(id));
                    expected.insert(AUDIO_TRACK.into(), json!(id));
                }
                None => tracing::info!("preset audio track rule matched nothing"),
            }
        }
        if let Some(ms) = audio.delay_ms {
            self.set_property("audio-delay", Value::Double(f64::from(ms) / 1000.0));
        }
        if let Some(name) = &audio.equalizer {
            let mut eq = self.mirror.state.eq.clone();
            if name == "off" {
                eq.enabled = false;
            } else if let Some(preset) = self.eq_presets.get(name) {
                eq.enabled = true;
                eq.mode = preset.mode;
                eq.preamp_db = preset.preamp_db;
                eq.bands = preset.bands;
                eq.preset = Some(preset.id);
            } else {
                tracing::warn!("a preset names the unknown equalizer preset `{name}`");
                expected
                    .insert("audio.equalizer".into(), json!(equalizer_name(&self.mirror.state)));
            }
            self.set_eq(eq);
        }
        if let Some(on) = audio.normalize {
            self.preset.normalize = Some(on);
            self.sync_normalize();
        }

        let subs = &layer.subtitles;
        if let Some(rule) = &subs.track {
            match rule.resolve(tracks) {
                SubChoice::Off => {
                    self.set_property("sid", Value::Text("no".into()));
                    expected.insert(SUB_TRACK.into(), json!("off"));
                }
                SubChoice::Track(id) => {
                    self.set_property("sid", Value::Int(id));
                    expected.insert(SUB_TRACK.into(), json!(id));
                }
                SubChoice::Unmatched => {
                    tracing::info!("preset subtitle track rule matched nothing")
                }
            }
        }
        if let Some(visible) = subs.visible {
            self.set_property("sub-visibility", Value::Flag(visible));
        }
        if let Some(ms) = subs.delay_ms {
            self.set_property("sub-delay", Value::Double(f64::from(ms) / 1000.0));
        }
        if !subs.style.is_empty() {
            let patch = style_patch(&subs.style);
            for (name, value) in super::subs::style_writes(&patch, &self.mirror.state.sub_style) {
                self.set_property(name, value);
            }
        }
        if !layer.video.is_empty() {
            let patch = video_patch(&layer.video, &self.shader_dir());
            self.set_video(&patch);
        }
        if let Some(speed) = layer.playback.speed {
            self.set_property("speed", Value::Double(speed.clamp(0.25, 4.0)));
        }
    }

    /// Called every loop: once the writes have settled, compare.
    pub(super) fn tick_presets(&mut self, now: Instant) {
        if self.preset.recheck.is_some_and(|due| now >= due) {
            self.preset.recheck = None;
            self.refresh_preset_dirty();
        }
    }

    /// Updates `presetDirty`: whether the live values differ from what the presets produced.
    pub(super) fn refresh_preset_dirty(&mut self) {
        if self.preset.recheck.is_some() {
            return;
        }
        let dirty = match &self.preset.expected {
            Some(expected) if !self.mirror.state.applied_presets.is_empty() => {
                !changed_keys(&live_flat(&self.mirror.state), expected).is_empty()
            }
            _ => false,
        };
        if self.mirror.state.preset_dirty != dirty {
            self.mirror.state.preset_dirty = dirty;
            self.dirty = true;
        }
    }

    /// `preset_apply`: this preset on top of whatever matches, for the loaded file.
    pub(super) fn force_preset(&mut self, id: String) {
        self.preset.forced = Some(id);
        self.preset.detached = false;
        self.apply_presets();
    }

    /// "Detach for this file": the live values stay, but they are no longer any preset's.
    pub(super) fn detach_presets(&mut self) {
        self.preset.detached = true;
        self.preset.forced = None;
        self.preset.expected = None;
        self.preset.layer = Layer::default();
        self.set_preset_marks(Vec::new(), false);
    }

    /// The most specific applied preset with the live differences written into it. The caller
    /// saves it; the live values are the new baseline from here on.
    pub(super) fn updated_preset(&mut self) -> Result<Preset, String> {
        let id = self.mirror.state.applied_presets.last().cloned().ok_or("no preset is applied")?;
        let mut preset =
            self.presets.get(&id).ok_or_else(|| format!("the preset `{id}` no longer exists"))?;
        let expected = self.preset.expected.clone().ok_or("nothing is playing")?;
        let live = live_flat(&self.mirror.state);
        let keys = changed_keys(&live, &expected);
        let mut flat = preset.layer.flat();
        flat.extend(preset_values(&keys, &live, &self.mirror.state.tracks));
        preset.layer = Layer::from_flat(&flat)?;
        preset.validate()?;
        self.preset.expected = Some(live);
        self.refresh_preset_dirty();
        Ok(preset)
    }

    pub(super) fn captured_preset(&mut self, request: &CaptureRequest) -> Result<Preset, String> {
        if self.mirror.state.media.is_none() {
            return Err("nothing is playing".into());
        }
        if request.name.trim().is_empty() {
            return Err("the preset needs a name".into());
        }
        let id = self.presets.free_id(&request.name);
        capture(request, id, &self.mirror.state, &defaults_layer(&self.config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::state::Mirror;
    use crate::presets::MatchType;
    use crate::presets::rules::SubTrackRule;

    fn state() -> PlayerState {
        Mirror::new(&crate::config::defaults()).state
    }

    fn track(id: i64, kind: TrackKind, lang: &str, title: &str) -> Track {
        Track {
            id,
            kind,
            title: Some(title.to_string()),
            lang: Some(lang.to_string()),
            codec: "x".into(),
            default: false,
            forced: false,
            external: false,
            external_path: None,
            width: None,
            height: None,
            fps: None,
            hdr: None,
            image: false,
            channels: None,
            sample_rate: None,
            bitrate: None,
            text_based: None,
        }
    }

    fn request(sections: &[&str]) -> CaptureRequest {
        CaptureRequest {
            name: "Show".into(),
            matcher: PresetMatch {
                kind: MatchType::Series,
                series: Some("show".into()),
                path: None,
                glob: None,
            },
            sections: sections.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn the_defaults_and_a_fresh_state_agree() {
        let config = crate::config::defaults();
        let defaults = defaults_layer(&config).flat();
        let live = live_flat(&state());
        for (key, value) in &defaults {
            assert!(same(&live[key], value), "{key}: {} vs {value}", live[key]);
        }
        assert_eq!(live[SUB_TRACK], json!("off"));
        assert!(!defaults.contains_key(SUB_TRACK) && !defaults.contains_key(AUDIO_TRACK));
    }

    #[test]
    fn live_values_differ_from_expected_only_where_they_moved() {
        let mut s = state();
        let expected = live_flat(&s);
        assert_eq!(changed_keys(&live_flat(&s), &expected), Vec::<String>::new());
        s.video.contrast = 4.0;
        s.sub_delay_ms = -350;
        s.sub_style.color = "#ffffff".into();
        s.speed = 1.0004;
        s.selected.sub = Some(2);
        let mut changed = changed_keys(&live_flat(&s), &expected);
        changed.sort();
        assert_eq!(changed, vec!["subtitles.delay_ms", "subtitles.track", "video.contrast"]);
    }

    /// The Phase 8 scenario: delay −350 ms, the English "Full" track, more contrast.
    #[test]
    fn capture_is_the_difference_from_defaults_with_tracks_as_rules() {
        let mut s = state();
        s.media = Mirror::new(&crate::config::defaults()).state.media;
        s.tracks = vec![
            track(1, TrackKind::Audio, "jpn", "Stereo"),
            track(2, TrackKind::Audio, "eng", "Dub"),
            track(1, TrackKind::Sub, "eng", "Signs"),
            track(2, TrackKind::Sub, "eng", "Full"),
        ];
        s.selected.audio = Some(1);
        s.selected.sub = Some(2);
        s.sub_delay_ms = -350;
        s.video.contrast = 4.0;
        let defaults = defaults_layer(&crate::config::defaults());
        let preset = capture(
            &request(&["audio", "subtitles", "video", "playback"]),
            "show".into(),
            &s,
            &defaults,
        )
        .unwrap();
        let flat = preset.layer.flat();
        let mut keys: Vec<&str> = flat.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["audio.track", "subtitles.delay_ms", "subtitles.track", "video.contrast"]
        );
        let Some(SubTrackRule::Rule(rule)) = &preset.layer.subtitles.track else {
            panic!("a rule was expected")
        };
        assert_eq!(rule.lang, vec!["eng"]);
        assert_eq!(rule.title_contains.as_deref(), Some("full"));
        // Another episode with the tracks in another order gets the same track.
        let other =
            vec![track(1, TrackKind::Sub, "eng", "Full"), track(2, TrackKind::Sub, "eng", "Signs")];
        assert_eq!(rule.resolve(&other, TrackKind::Sub), Some(1));

        // Only the chosen sections are captured.
        let video_only = capture(&request(&["video"]), "show".into(), &s, &defaults).unwrap();
        assert_eq!(video_only.layer.flat().len(), 1);
        assert_eq!(video_only.layer.video.contrast, Some(4.0));
    }

    #[test]
    fn nothing_changed_captures_nothing_and_an_unsaved_eq_is_left_out() {
        let mut s = state();
        let defaults = defaults_layer(&crate::config::defaults());
        let all = ["audio", "subtitles", "video", "playback"];
        assert!(capture(&request(&all), "x".into(), &s, &defaults).unwrap().layer.is_empty());
        s.eq.enabled = true;
        s.eq.preset = None;
        assert!(capture(&request(&all), "x".into(), &s, &defaults).unwrap().layer.is_empty());
        s.eq.preset = Some("dialogue".into());
        let preset = capture(&request(&all), "x".into(), &s, &defaults).unwrap();
        assert_eq!(preset.layer.audio.equalizer.as_deref(), Some("dialogue"));
    }

    #[test]
    fn shaders_are_stored_by_name_and_resolved_in_the_shaders_folder() {
        let mut s = state();
        s.video.shaders = vec!["C:\\cfg\\shaders\\Anime4K.glsl".into()];
        assert_eq!(live_flat(&s)["video.shaders"], json!(["Anime4K.glsl"]));
        let section =
            VideoSection { shaders: Some(vec!["Anime4K.glsl".into()]), ..Default::default() };
        let patch = video_patch(&section, std::path::Path::new("C:/cfg/shaders"));
        let path = patch.shaders.unwrap().remove(0);
        assert!(path.replace('\\', "/").ends_with("C:/cfg/shaders/Anime4K.glsl"), "{path}");
    }
}
