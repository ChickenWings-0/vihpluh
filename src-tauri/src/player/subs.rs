//! Subtitles in the core (ARCHITECTURE §6.6, §10.5, §11.3): live style changes, external
//! subtitle files, the links that bring them back on later loads, and feeding the cues mpv
//! shows into the progressive cue index.

use std::path::Path;

use crate::ipc::dto::{SubStyle, SubStylePatch, Track, TrackKind};
use crate::library::sub_links::SubLink;
use crate::subtitles::cues::Cue;

use super::mpv::{MpvError, Value};
use super::options::SUB_BOX_COLOR;
use super::{Core, Player, tracks};

fn finite(value: f64, low: f64, high: f64) -> Option<f64> {
    value.is_finite().then(|| value.clamp(low, high))
}

/// `#RRGGBB` with the opacity of the shipped background box, in mpv's `#AARRGGBB` form.
fn box_color(rgb: &str) -> String {
    format!("{}{}", &SUB_BOX_COLOR[..3], rgb.trim_start_matches('#'))
}

/// Translates a style patch into mpv property writes. Invalid values are dropped. `current`
/// is needed because the background box is two mpv options with one colour between them.
pub fn style_writes(patch: &SubStylePatch, current: &SubStyle) -> Vec<(&'static str, Value)> {
    let mut out = Vec::new();
    let color = |c: &String| crate::config::is_hex_color(c).then(|| c.to_uppercase());
    if let Some(font) = patch.font.as_ref().filter(|f| !f.trim().is_empty()) {
        out.push(("sub-font", Value::Text(font.trim().to_string())));
    }
    if let Some(size) = patch.size.and_then(|v| finite(v, 10.0, 120.0)) {
        out.push(("sub-font-size", Value::Double(size)));
    }
    if let Some(scale) = patch.scale.and_then(|v| finite(v, 0.5, 3.0)) {
        out.push(("sub-scale", Value::Double(scale)));
    }
    if let Some(bold) = patch.bold {
        out.push(("sub-bold", Value::Flag(bold)));
    }
    if let Some(c) = patch.color.as_ref().and_then(color) {
        out.push(("sub-color", Value::Text(c)));
    }
    if let Some(c) = patch.outline_color.as_ref().and_then(color) {
        out.push(("sub-outline-color", Value::Text(c)));
    }
    if let Some(size) = patch.outline_size.and_then(|v| finite(v, 0.0, 10.0)) {
        out.push(("sub-outline-size", Value::Double(size)));
    }
    if let Some(offset) = patch.shadow_offset.and_then(|v| finite(v, 0.0, 10.0)) {
        out.push(("sub-shadow-offset", Value::Double(offset)));
    }
    let background = patch.background.as_deref().unwrap_or(&current.background);
    let background_color = patch.background_color.as_ref().and_then(color);
    if patch.background.is_some() || background_color.is_some() {
        if background == "box" {
            let rgb = background_color.unwrap_or_else(|| current.background_color.clone());
            out.push(("sub-border-style", Value::Text("background-box".into())));
            out.push(("sub-back-color", Value::Text(box_color(&rgb))));
        } else {
            out.push(("sub-border-style", Value::Text("outline-and-shadow".into())));
            out.push(("sub-back-color", Value::Text("#00000000".into())));
        }
    }
    if let Some(position) = patch.position.and_then(|v| finite(v, 0.0, 100.0)) {
        out.push(("sub-pos", Value::Double(position.round())));
    }
    if let Some(margin) = patch.margin_y.and_then(|v| finite(v, 0.0, 200.0)) {
        // An integer option: mpv rejects a double here.
        out.push(("sub-margin-y", Value::Int(margin.round() as i64)));
    }
    if let Some(mode) = patch.ass_override.as_deref() {
        if matches!(mode, "no" | "scale" | "force" | "strip") {
            out.push(("sub-ass-override", Value::Text(mode.to_string())));
        }
    }
    out
}

/// Whether two paths name the same file (separators and, on Windows, case aside).
pub fn same_path(a: &str, b: &str) -> bool {
    let norm = |p: &str| {
        let p = p.replace('\\', "/");
        if cfg!(windows) { p.to_lowercase() } else { p }
    };
    norm(a) == norm(b)
}

fn external_track<'a>(tracks: &'a [Track], kind: TrackKind, path: &str) -> Option<&'a Track> {
    tracks
        .iter()
        .filter(|t| t.kind == kind && t.external)
        .filter(|t| t.external_path.as_deref().is_some_and(|p| same_path(p, path)))
        .max_by_key(|t| t.id)
}

fn failed(message: &str) -> MpvError {
    MpvError::Api { code: -1, message: message.into() }
}

impl Player {
    /// Live subtitle style (the Subtitles tab). Not saved: defaults live in `subtitles.*`.
    pub fn set_sub_style(&self, patch: &SubStylePatch) -> Result<(), MpvError> {
        let current = match self.snapshot.read() {
            Ok(state) => state.sub_style.clone(),
            Err(poisoned) => poisoned.into_inner().sub_style.clone(),
        };
        for (name, value) in style_writes(patch, &current) {
            self.set(name, value)?;
        }
        Ok(())
    }

    /// Shifts the subtitle delay so the next (1) or previous (−1) line shows now.
    pub fn sub_step(&self, direction: i32) -> Result<(), MpvError> {
        self.command(&["sub-step", if direction < 0 { "-1" } else { "1" }])
    }

    /// The current tracks, as last mirrored.
    pub fn current_tracks(&self) -> Vec<Track> {
        match self.snapshot.read() {
            Ok(state) => state.tracks.clone(),
            Err(poisoned) => poisoned.into_inner().tracks.clone(),
        }
    }

    /// Adds an external subtitle or audio file to the playing media and returns its track
    /// id. Waits for mpv to open the file.
    pub fn add_external(&self, kind: TrackKind, path: &str, select: bool) -> Result<i64, MpvError> {
        let Some(mpv) = self.mpv() else { return Err(failed("the player is shutting down")) };
        if mpv.get_bool("idle-active").unwrap_or(true) {
            return Err(failed("nothing is playing"));
        }
        let command = match kind {
            TrackKind::Sub => "sub-add",
            TrackKind::Audio => "audio-add",
            TrackKind::Video => return Err(failed("only subtitle and audio files can be added")),
        };
        mpv.command(&[command, path, if select { "select" } else { "auto" }])?;
        let list = mpv.get_node("track-list").map(|n| tracks::parse_tracks(&n)).unwrap_or_default();
        external_track(&list, kind, path)
            .map(|t| t.id)
            .ok_or_else(|| failed("the file was not added as a track"))
    }

    /// Removes an external track. Returns its kind and the file it came from.
    pub fn remove_external(&self, id: i64) -> Result<(TrackKind, Option<String>), MpvError> {
        let tracks = self.current_tracks();
        let Some(track) = tracks.iter().find(|t| t.id == id && t.external) else {
            return Err(failed("that track is part of the file and cannot be removed"));
        };
        let command = match track.kind {
            TrackKind::Sub => "sub-remove",
            TrackKind::Audio => "audio-remove",
            TrackKind::Video => return Err(failed("video tracks cannot be removed")),
        };
        self.command(&[command, &id.to_string()])?;
        Ok((track.kind, track.external_path.clone()))
    }
}

impl Core {
    /// mpv moved to another subtitle line: note it for the timeline. Only tracks without a
    /// full index keep what is recorded here (§11.3).
    pub(super) fn record_cue(&mut self) {
        let Some(key) = self.current.as_ref().and_then(|c| c.media_key.clone()) else { return };
        let Some(track_id) = self.mirror.state.selected.sub else { return };
        let (Some(start), Some(end)) = (self.mpv.get_f64("sub-start"), self.mpv.get_f64("sub-end"))
        else {
            return;
        };
        let text = self.mpv.get_string("sub-text").filter(|t| !t.trim().is_empty());
        self.cues.record(&key, track_id, Cue { start, end, text });
    }

    /// Attaches the subtitle files linked to this media (auto-match, or added by hand on an
    /// earlier day) that mpv did not find by itself.
    pub(super) fn attach_sub_links(&mut self, links: &[SubLink]) {
        if links.is_empty() {
            return;
        }
        let present =
            self.mpv.get_node("track-list").map(|n| tracks::parse_tracks(&n)).unwrap_or_default();
        // With subtitles shown by default and none chosen yet, the first linked file shows.
        let nothing_selected = self.mpv.get_string("sid").is_none_or(|sid| sid == "no");
        let mut select = self.config.subtitles.show_by_default && nothing_selected;
        for link in links {
            if external_track(&present, TrackKind::Sub, &link.path).is_some() {
                continue;
            }
            if !Path::new(&link.path).is_file() {
                tracing::debug!("a linked subtitle file is gone; skipped");
                continue;
            }
            let flag = if std::mem::take(&mut select) { "select" } else { "auto" };
            let title = Path::new(&link.path).file_name().and_then(|n| n.to_str()).unwrap_or("");
            let result = match &link.lang {
                Some(lang) => self.mpv.command(&["sub-add", &link.path, flag, title, lang]),
                None => self.mpv.command(&["sub-add", &link.path, flag]),
            };
            if let Err(err) = result {
                tracing::warn!("could not attach a linked subtitle file: {err}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::state::Mirror;

    fn current() -> SubStyle {
        Mirror::new(&crate::config::defaults()).state.sub_style
    }

    fn value<'a>(writes: &'a [(&'static str, Value)], name: &str) -> Option<&'a Value> {
        writes.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
    }

    #[test]
    fn a_patch_writes_only_what_it_names() {
        let patch =
            SubStylePatch { size: Some(46.0), bold: Some(true), ..SubStylePatch::default() };
        let writes = style_writes(&patch, &current());
        assert_eq!(writes.len(), 2);
        assert_eq!(value(&writes, "sub-font-size"), Some(&Value::Double(46.0)));
        assert_eq!(value(&writes, "sub-bold"), Some(&Value::Flag(true)));
        assert_eq!(style_writes(&SubStylePatch::default(), &current()), vec![]);
    }

    #[test]
    fn values_are_clamped_and_bad_ones_dropped() {
        let patch = SubStylePatch {
            font: Some("  ".into()),
            size: Some(500.0),
            scale: Some(f64::NAN),
            color: Some("red".into()),
            outline_color: Some("#00ff7f".into()),
            position: Some(120.0),
            ass_override: Some("yes".into()),
            ..SubStylePatch::default()
        };
        let writes = style_writes(&patch, &current());
        assert_eq!(value(&writes, "sub-font"), None);
        assert_eq!(value(&writes, "sub-font-size"), Some(&Value::Double(120.0)));
        assert_eq!(value(&writes, "sub-scale"), None);
        assert_eq!(value(&writes, "sub-color"), None);
        assert_eq!(value(&writes, "sub-outline-color"), Some(&Value::Text("#00FF7F".into())));
        assert_eq!(value(&writes, "sub-pos"), Some(&Value::Double(100.0)));
        assert_eq!(value(&writes, "sub-ass-override"), None);
    }

    #[test]
    fn the_background_box_is_two_options() {
        let on = SubStylePatch { background: Some("box".into()), ..SubStylePatch::default() };
        let writes = style_writes(&on, &current());
        assert_eq!(value(&writes, "sub-border-style"), Some(&Value::Text("background-box".into())));
        assert_eq!(value(&writes, "sub-back-color"), Some(&Value::Text("#C0000000".into())));

        let mut boxed = current();
        boxed.background = "box".into();
        let recolor =
            SubStylePatch { background_color: Some("#102030".into()), ..SubStylePatch::default() };
        let writes = style_writes(&recolor, &boxed);
        assert_eq!(value(&writes, "sub-back-color"), Some(&Value::Text("#C0102030".into())));

        let off = SubStylePatch { background: Some("none".into()), ..SubStylePatch::default() };
        let writes = style_writes(&off, &boxed);
        assert_eq!(
            value(&writes, "sub-border-style"),
            Some(&Value::Text("outline-and-shadow".into()))
        );
        assert_eq!(value(&writes, "sub-back-color"), Some(&Value::Text("#00000000".into())));
    }

    /// Every property a style patch can write exists in the linked libmpv and takes the value.
    #[test]
    fn linked_libmpv_accepts_every_style_write() {
        use crate::player::mpv::Mpv;
        let mpv = Mpv::create().unwrap();
        for (k, v) in [("vo", "null"), ("ao", "null"), ("config", "no")] {
            mpv.set_option(k, v).unwrap();
        }
        mpv.initialize().unwrap();
        let patch = SubStylePatch {
            font: Some("Segoe UI".into()),
            size: Some(40.0),
            scale: Some(1.25),
            bold: Some(true),
            color: Some("#FFFFFF".into()),
            outline_color: Some("#000000".into()),
            outline_size: Some(2.5),
            shadow_offset: Some(1.0),
            background: Some("box".into()),
            background_color: Some("#101010".into()),
            position: Some(96.0),
            margin_y: Some(30.0),
            ass_override: Some("force".into()),
        };
        for (name, value) in style_writes(&patch, &current()) {
            let result = match &value {
                Value::Text(text) => mpv.set_string(name, text),
                Value::Double(number) => mpv.set_f64(name, *number),
                Value::Flag(flag) => mpv.set_bool(name, *flag),
                Value::Int(number) => mpv.set_i64(name, *number),
            };
            result.unwrap_or_else(|err| panic!("mpv rejected {name} = {value:?}: {err}"));
        }
    }

    #[test]
    fn paths_compare_across_separators() {
        assert!(same_path("D:\\s\\Subs\\a.srt", "D:/s/Subs/a.srt"));
        assert!(!same_path("D:/s/a.srt", "D:/s/b.srt"));
    }
}
