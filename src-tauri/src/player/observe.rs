//! The property observation table (ARCHITECTURE §6.3). Adding a mirrored field means adding
//! one row here and one arm in `state::Mirror::on_property`.

use super::mpv::{Format, Mpv, MpvResult};

/// Every property the core observes on the primary handle. The observation id is the row
/// index + 1; events are matched by name, so ids only need to be unique.
pub const OBSERVED: &[(&str, Format)] = &[
    // Status inputs (§6.4)
    ("idle-active", Format::Flag),
    ("pause", Format::Flag),
    ("paused-for-cache", Format::Flag),
    ("seeking", Format::Flag),
    ("eof-reached", Format::Flag),
    // Time → player://time
    ("time-pos", Format::Double),
    ("duration", Format::Double),
    ("demuxer-cache-time", Format::Double),
    // Media
    ("path", Format::String),
    ("media-title", Format::String),
    ("file-format", Format::String),
    ("file-size", Format::Int),
    // Tracks and chapters
    ("track-list", Format::Node),
    ("vid", Format::Node),
    ("aid", Format::Node),
    ("sid", Format::Node),
    ("secondary-sid", Format::Node),
    ("chapter-list", Format::Node),
    ("chapter", Format::Int),
    // Transport
    ("volume", Format::Double),
    ("mute", Format::Flag),
    ("speed", Format::Double),
    ("play-direction", Format::String),
    ("loop-file", Format::Node),
    ("ab-loop-a", Format::Node),
    ("ab-loop-b", Format::Node),
    // Sync
    ("audio-delay", Format::Double),
    ("sub-delay", Format::Double),
    ("sub-visibility", Format::Flag),
    // Audio output
    ("audio-device", Format::String),
    ("audio-device-list", Format::Node),
    // Picture
    ("brightness", Format::Double),
    ("contrast", Format::Double),
    ("saturation", Format::Double),
    ("gamma", Format::Double),
    ("hue", Format::Double),
    ("video-rotate", Format::Node),
    ("video-zoom", Format::Double),
    ("video-pan-x", Format::Double),
    ("video-pan-y", Format::Double),
    ("video-aspect-override", Format::String),
    ("panscan", Format::Double),
    ("keepaspect", Format::Flag),
    ("tone-mapping", Format::String),
    ("deband", Format::Flag),
    ("interpolation", Format::Flag),
    ("glsl-shaders", Format::Node),
    ("video-params", Format::Node),
    ("hwdec-current", Format::String),
    // Subtitle style
    ("sub-font", Format::String),
    ("sub-font-size", Format::Double),
    ("sub-scale", Format::Double),
    ("sub-bold", Format::Flag),
    ("sub-color", Format::String),
    ("sub-outline-color", Format::String),
    ("sub-outline-size", Format::Double),
    ("sub-shadow-offset", Format::Double),
    ("sub-border-style", Format::String),
    ("sub-back-color", Format::String),
    ("sub-pos", Format::Double),
    ("sub-margin-y", Format::Double),
    ("sub-ass-override", Format::String),
];

/// Registers every row. A property the linked mpv does not know is skipped with a warning
/// instead of failing startup (§12.3).
pub fn observe_all(mpv: &Mpv) -> MpvResult<()> {
    for (index, (name, format)) in OBSERVED.iter().enumerate() {
        if let Err(err) = mpv.observe(index as u64 + 1, name, *format) {
            tracing::warn!("cannot observe mpv property {name}: {err}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_unique() {
        let mut names: Vec<&str> = OBSERVED.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before);
    }

    /// Every row is a real property of the linked libmpv.
    #[test]
    fn linked_libmpv_knows_every_observed_property() {
        let mpv = Mpv::create().unwrap();
        for (k, v) in [("vo", "null"), ("ao", "null"), ("config", "no")] {
            mpv.set_option(k, v).unwrap();
        }
        mpv.initialize().unwrap();
        for (index, (name, format)) in OBSERVED.iter().enumerate() {
            assert!(mpv.property_known(name), "mpv has no property named {name}");
            mpv.observe(index as u64 + 1, name, *format)
                .unwrap_or_else(|err| panic!("mpv cannot observe {name}: {err}"));
        }
    }
}
