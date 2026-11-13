//! Picture adjustments (ARCHITECTURE §6.6, DESIGN_SYSTEM §10.3): a `VideoPatch` becomes mpv
//! property writes and shader-list commands. The results come back through the observed
//! properties, so the mirror stays the single source of the state.

use crate::config::Config;
use crate::config::schema::VideoSync;
use crate::ipc::dto::{VideoAdjust, VideoPatch};

use super::Core;
use super::mpv::Value;

/// One step of applying a patch.
#[derive(Debug, Clone, PartialEq)]
pub enum Write {
    Property(&'static str, Value),
    Command(Vec<String>),
}

/// Paths of the two flip shaders (written to the cache folder at startup).
#[derive(Debug, Clone, Default)]
pub struct FlipShaders {
    pub h: String,
    pub v: String,
}

const FLIP_H: &str = include_str!("../../resources/shaders/flip_h.glsl");
const FLIP_V: &str = include_str!("../../resources/shaders/flip_v.glsl");

/// Writes the shipped flip shaders into `dir` (mpv loads shaders from files) and returns
/// their paths. A write failure only disables flipping.
pub fn write_flip_shaders(dir: &std::path::Path) -> FlipShaders {
    let write = |name: &str, text: &str| -> String {
        let path = dir.join(name);
        let result = std::fs::create_dir_all(dir).and_then(|()| {
            // Unchanged files are left alone: mpv may already have them open.
            if std::fs::read_to_string(&path).ok().as_deref() == Some(text) {
                Ok(())
            } else {
                std::fs::write(&path, text)
            }
        });
        if let Err(err) = result {
            tracing::warn!("could not write the {name} shader: {err}");
        }
        path.display().to_string()
    };
    FlipShaders { h: write("flip_h.glsl", FLIP_H), v: write("flip_v.glsl", FLIP_V) }
}

pub const ASPECTS: &[&str] = &["auto", "16:9", "4:3", "21:9", "2.39", "1:1", "stretch", "fill"];

/// `video-zoom` is log2: 0 = 1×. The Video tab offers 0.5× – 4×.
const ZOOM_RANGE: (f64, f64) = (-1.0, 2.0);

fn clamp(value: f64, low: f64, high: f64) -> f64 {
    if value.is_finite() { value.clamp(low, high) } else { 0.0 }
}

/// `panscan` and `keepaspect` for an aspect name: "fill" crops, "stretch" distorts.
pub fn aspect_fit(aspect: &str) -> (f64, bool) {
    match aspect {
        "stretch" => (0.0, false),
        "fill" => (1.0, true),
        _ => (0.0, true),
    }
}

/// Translates a patch into writes, given the current state (needed for the shader list).
pub fn writes(
    patch: &VideoPatch,
    current: &VideoAdjust,
    flips: &FlipShaders,
    sync: VideoSync,
) -> Vec<Write> {
    let mut out = Vec::new();
    for (name, value) in [
        ("brightness", patch.brightness),
        ("contrast", patch.contrast),
        ("saturation", patch.saturation),
        ("gamma", patch.gamma),
        ("hue", patch.hue),
    ] {
        if let Some(v) = value {
            out.push(Write::Property(name, Value::Double(clamp(v, -100.0, 100.0).round())));
        }
    }
    if let Some(degrees) = patch.rotate {
        let degrees = (degrees / 90 * 90) % 360;
        out.push(Write::Property("video-rotate", Value::Int(i64::from(degrees))));
    }
    if let Some(zoom) = patch.zoom {
        out.push(Write::Property(
            "video-zoom",
            Value::Double(clamp(zoom, ZOOM_RANGE.0, ZOOM_RANGE.1)),
        ));
    }
    if let Some(x) = patch.pan_x {
        out.push(Write::Property("video-pan-x", Value::Double(clamp(x, -1.0, 1.0))));
    }
    if let Some(y) = patch.pan_y {
        out.push(Write::Property("video-pan-y", Value::Double(clamp(y, -1.0, 1.0))));
    }
    if let Some(aspect) = &patch.aspect {
        let (panscan, keep) = aspect_fit(aspect);
        out.push(Write::Property(
            "video-aspect-override",
            Value::Text(super::options::aspect_to_mpv(aspect)),
        ));
        out.push(Write::Property("panscan", Value::Double(panscan)));
        out.push(Write::Property("keepaspect", Value::Flag(keep)));
    }
    if let Some(curve) = &patch.tone_mapping {
        out.push(Write::Property("tone-mapping", Value::Text(curve.clone())));
    }
    if let Some(on) = patch.deband {
        out.push(Write::Property("deband", Value::Flag(on)));
    }
    if let Some(on) = patch.interpolation {
        // Interpolation only works when video is timed to the display (§6.6).
        let sync = if on { "display-resample" } else { sync.as_str() };
        out.push(Write::Property("video-sync", Value::Text(sync.to_string())));
        out.push(Write::Property("interpolation", Value::Flag(on)));
    }
    if patch.flip_h.is_some() || patch.flip_v.is_some() || patch.shaders.is_some() {
        // The whole list is rebuilt: user shaders first, the flips last so they mirror the
        // final picture. `append` takes one path at a time, so no separator parsing is
        // involved (Windows paths contain ':').
        let user = patch.shaders.as_ref().unwrap_or(&current.shaders);
        let mut list: Vec<&str> = user.iter().map(String::as_str).collect();
        if patch.flip_h.unwrap_or(current.flip_h) {
            list.push(&flips.h);
        }
        if patch.flip_v.unwrap_or(current.flip_v) {
            list.push(&flips.v);
        }
        out.push(Write::Command(vec![
            "change-list".into(),
            "glsl-shaders".into(),
            "clr".into(),
            String::new(),
        ]));
        for path in list {
            out.push(Write::Command(vec![
                "change-list".into(),
                "glsl-shaders".into(),
                "append".into(),
                path.to_string(),
            ]));
        }
    }
    out
}

/// "Reset all video settings": back to the configured defaults, shaders off.
pub fn reset_patch(config: &Config) -> VideoPatch {
    let video = &config.video;
    VideoPatch {
        brightness: Some(0.0),
        contrast: Some(0.0),
        saturation: Some(0.0),
        gamma: Some(0.0),
        hue: Some(0.0),
        rotate: Some(0),
        flip_h: Some(false),
        flip_v: Some(false),
        zoom: Some(0.0),
        pan_x: Some(0.0),
        pan_y: Some(0.0),
        aspect: Some(video.default_aspect.clone()),
        tone_mapping: Some(video.tone_mapping.clone()),
        deband: Some(video.deband),
        interpolation: Some(video.interpolation),
        shaders: Some(Vec::new()),
    }
}

impl Core {
    pub(super) fn set_video(&mut self, patch: &VideoPatch) {
        let sync = self.config.video.sync;
        for write in writes(patch, &self.mirror.state.video, &self.flip_shaders, sync) {
            match write {
                Write::Property(name, value) => self.set_property(name, value),
                Write::Command(args) => {
                    let args: Vec<&str> = args.iter().map(String::as_str).collect();
                    self.run_command(&args);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flips() -> FlipShaders {
        FlipShaders {
            h: "C:/cache/shaders/flip_h.glsl".into(),
            v: "C:/cache/shaders/flip_v.glsl".into(),
        }
    }

    fn commands(w: &[Write]) -> Vec<String> {
        w.iter()
            .filter_map(|w| match w {
                Write::Command(args) => Some(args.join(" ")),
                Write::Property(..) => None,
            })
            .collect()
    }

    #[test]
    fn flips_rebuild_the_shader_list_after_user_shaders() {
        let current = VideoAdjust { shaders: vec!["D:/s/a.glsl".into()], ..VideoAdjust::default() };
        let patch = VideoPatch { flip_h: Some(true), ..VideoPatch::default() };
        assert_eq!(
            commands(&writes(&patch, &current, &flips(), VideoSync::Audio)),
            vec![
                "change-list glsl-shaders clr ",
                "change-list glsl-shaders append D:/s/a.glsl",
                "change-list glsl-shaders append C:/cache/shaders/flip_h.glsl",
            ]
        );
    }

    #[test]
    fn aspect_presets() {
        let current = VideoAdjust::default();
        let fill = VideoPatch { aspect: Some("fill".into()), ..VideoPatch::default() };
        let w = writes(&fill, &current, &flips(), VideoSync::Audio);
        assert!(w.contains(&Write::Property("panscan", Value::Double(1.0))));
        let wide = VideoPatch { aspect: Some("2.39".into()), ..VideoPatch::default() };
        let w = writes(&wide, &current, &flips(), VideoSync::Audio);
        assert!(
            w.contains(&Write::Property("video-aspect-override", Value::Text("2.39:1".into())))
        );
        let stretch = VideoPatch { aspect: Some("stretch".into()), ..VideoPatch::default() };
        let w = writes(&stretch, &current, &flips(), VideoSync::Audio);
        assert!(w.contains(&Write::Property("keepaspect", Value::Flag(false))));
    }

    #[test]
    fn values_are_clamped_and_interpolation_sets_sync() {
        let patch = VideoPatch {
            brightness: Some(250.0),
            rotate: Some(450),
            zoom: Some(9.0),
            interpolation: Some(false),
            ..VideoPatch::default()
        };
        let w = writes(&patch, &VideoAdjust::default(), &flips(), VideoSync::Audio);
        assert!(w.contains(&Write::Property("brightness", Value::Double(100.0))));
        assert!(w.contains(&Write::Property("video-rotate", Value::Int(90))));
        assert!(w.contains(&Write::Property("video-zoom", Value::Double(2.0))));
        assert!(w.contains(&Write::Property("video-sync", Value::Text("audio".into()))));
    }
}
