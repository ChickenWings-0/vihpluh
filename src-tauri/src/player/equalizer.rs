//! The equalizer and loudness filters (ARCHITECTURE §6.6). Pure: builds mpv `af` entries
//! and decides when a change can be sent to the running filter with `af-command` (no
//! rebuild, so no dropout — verified in spike S4) instead of replacing it.

use crate::ipc::dto::{EqBand, EqState};

pub const EQ_LABEL: &str = "vihpluh-eq";
pub const NORM_LABEL: &str = "vihpluh-norm";

/// `dynaudnorm` settings from §6.6: gentle (gaussian window 5 frames of 250 ms), with a
/// target peak below full scale so it never clips.
pub const NORM_ENTRY: &str = "@vihpluh-norm:lavfi=[dynaudnorm=g=5:f=250:r=0.9:p=0.5]";

/// Compact decimal text (`-2`, `3.5`, `0.75`) for filter arguments.
fn num(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".to_string() } else { text.to_string() }
}

/// The lavfi filter that implements one band, and its instance name inside the graph.
fn band_filter(index: usize, band: &EqBand) -> &'static str {
    let _ = index;
    match band.kind.as_str() {
        "lowshelf" => "bass",
        "highshelf" => "treble",
        _ => "equalizer",
    }
}

fn instance(index: usize, band: &EqBand) -> String {
    format!("{}@b{index}", band_filter(index, band))
}

/// The `af` entry for `eq`: `@vihpluh-eq:lavfi=[volume=…dB,equalizer@b0=…,…]`.
/// Graphic bands are one octave wide; parametric bands use their Q.
pub fn eq_entry(eq: &EqState) -> String {
    let graphic = eq.mode == "graphic10";
    let mut parts = vec![format!("volume={}dB", num(eq.preamp_db))];
    for (i, band) in eq.bands.iter().enumerate() {
        let width = if graphic { "t=o:w=1".to_string() } else { format!("t=q:w={}", num(band.q)) };
        parts.push(format!(
            "{}=f={}:{width}:g={}",
            instance(i, band),
            num(band.freq),
            num(band.gain_db)
        ));
    }
    format!("@{EQ_LABEL}:lavfi=[{}]", parts.join(","))
}

/// The `af-command` argument lists that turn the running filter for `old` into `new`, or
/// `None` when the change needs a rebuild (band added/removed, frequency, Q, type, mode).
pub fn live_commands(old: &EqState, new: &EqState) -> Option<Vec<Vec<String>>> {
    if old.mode != new.mode || old.bands.len() != new.bands.len() {
        return None;
    }
    let mut commands = Vec::new();
    for (i, (a, b)) in old.bands.iter().zip(&new.bands).enumerate() {
        if a.freq != b.freq || a.q != b.q || a.kind != b.kind {
            return None;
        }
        if a.gain_db != b.gain_db {
            commands.push(vec![
                "af-command".to_string(),
                EQ_LABEL.to_string(),
                "g".to_string(),
                num(b.gain_db),
                instance(i, b),
            ]);
        }
    }
    if old.preamp_db != new.preamp_db {
        commands.push(vec![
            "af-command".to_string(),
            EQ_LABEL.to_string(),
            "volume".to_string(),
            format!("{}dB", num(new.preamp_db)),
            "volume".to_string(),
        ]);
    }
    Some(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::eq_presets::graphic_bands;

    fn graphic(gains: [f64; 10], preamp: f64) -> EqState {
        EqState {
            enabled: true,
            mode: "graphic10".into(),
            preset: None,
            preamp_db: preamp,
            bands: graphic_bands(&gains),
        }
    }

    #[test]
    fn graphic_entry() {
        let eq = graphic([-2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 6.5], -2.0);
        let entry = eq_entry(&eq);
        assert!(
            entry.starts_with("@vihpluh-eq:lavfi=[volume=-2dB,equalizer@b0=f=31:t=o:w=1:g=-2,")
        );
        assert!(entry.ends_with("equalizer@b9=f=16000:t=o:w=1:g=6.5]"));
    }

    #[test]
    fn parametric_entry_maps_shelves() {
        let eq = EqState {
            enabled: true,
            mode: "parametric".into(),
            preset: None,
            preamp_db: -4.5,
            bands: vec![
                EqBand { freq: 105.0, gain_db: 5.5, q: 0.7, kind: "lowshelf".into() },
                EqBand { freq: 3300.0, gain_db: -2.0, q: 2.0, kind: "peak".into() },
                EqBand { freq: 10000.0, gain_db: 1.5, q: 0.7, kind: "highshelf".into() },
            ],
        };
        assert_eq!(
            eq_entry(&eq),
            "@vihpluh-eq:lavfi=[volume=-4.5dB,bass@b0=f=105:t=q:w=0.7:g=5.5,\
             equalizer@b1=f=3300:t=q:w=2:g=-2,treble@b2=f=10000:t=q:w=0.7:g=1.5]"
        );
    }

    #[test]
    fn gain_and_preamp_changes_are_live() {
        let old = graphic([0.0; 10], 0.0);
        let mut new = old.clone();
        new.bands[3].gain_db = 4.5;
        new.preamp_db = -3.0;
        let commands = live_commands(&old, &new).unwrap();
        assert_eq!(
            commands,
            vec![
                vec!["af-command", "vihpluh-eq", "g", "4.5", "equalizer@b3"],
                vec!["af-command", "vihpluh-eq", "volume", "-3dB", "volume"],
            ]
        );
        assert_eq!(live_commands(&old, &old).unwrap(), Vec::<Vec<String>>::new());
    }

    #[test]
    fn structural_changes_need_a_rebuild() {
        let old = graphic([0.0; 10], 0.0);
        let mut new = old.clone();
        new.bands[0].q = 2.0;
        assert!(live_commands(&old, &new).is_none());
        let mut new = old.clone();
        new.bands.pop();
        assert!(live_commands(&old, &new).is_none());
        let mut new = old.clone();
        new.mode = "parametric".into();
        assert!(live_commands(&old, &new).is_none());
    }

    #[test]
    fn numbers_are_compact() {
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(12.0), "12");
        assert_eq!(num(0.125), "0.125");
        assert_eq!(num(-2.5), "-2.5");
    }

    /// The entries parse in the linked libmpv's `af` syntax (labels, nested lavfi graph).
    #[test]
    fn linked_libmpv_accepts_the_entries() {
        use crate::player::mpv::Mpv;
        let mpv = Mpv::create().unwrap();
        for (k, v) in [("vo", "null"), ("ao", "null"), ("config", "no")] {
            mpv.set_option(k, v).unwrap();
        }
        mpv.initialize().unwrap();
        let eq = graphic([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -3.5], -1.0);
        mpv.command(&["af", "pre", &eq_entry(&eq)]).unwrap();
        mpv.command(&["af", "add", NORM_ENTRY]).unwrap();
        let af = mpv.get_node("af").unwrap();
        let labels: Vec<&str> =
            af.as_array().unwrap().iter().filter_map(|f| f.str_field("label")).collect();
        assert_eq!(labels, [EQ_LABEL, NORM_LABEL]);
        mpv.command(&["af", "remove", &format!("@{EQ_LABEL}")]).unwrap();
        assert_eq!(mpv.get_node("af").unwrap().as_array().unwrap().len(), 1);
    }
}
