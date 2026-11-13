//! Audio policy in the core: the equalizer and loudness filters (§6.6) and output-device
//! fallback.
//!
//! The EQ lives in mpv's `af` list as one labelled entry. Gain and preamp moves go to the
//! running filter with `af-command`, which is glitch-free but leaves the `af` option text
//! behind; that text is what mpv uses to build the filter for the next file, so it is
//! rewritten before the next file starts (`on_start_file`). Structural changes replace the
//! entry, debounced so a burst of edits rebuilds once.

use std::time::{Duration, Instant};

use crate::config::Config;
use crate::config::eq_presets::EqPresetStore;
use crate::ipc::dto::{EqState, OsdEvent, OsdKind};
use crate::ipc::events;

use super::Core;
use super::equalizer::{self, EQ_LABEL, NORM_ENTRY, NORM_LABEL};

/// Structural EQ changes are applied this long after the last edit (§6.6).
pub const EQ_REBUILD_DEBOUNCE: Duration = Duration::from_millis(80);

/// The filter chain as it stands in mpv.
#[derive(Debug, Default)]
pub struct Filters {
    /// The EQ as the running filter has it (None: no EQ entry).
    pub eq_running: Option<EqState>,
    /// The `af` text no longer matches `eq_running` (live commands were sent).
    pub eq_text_stale: bool,
    pub eq_rebuild_due: Option<Instant>,
    pub norm: bool,
}

/// The EQ the config asks for: on/off plus a preset, starting from `current` so a custom
/// curve survives when only the on/off switch changes.
pub fn eq_from_config(config: &Config, presets: &EqPresetStore, current: &EqState) -> EqState {
    let audio = &config.audio;
    let mut eq = current.clone();
    eq.enabled = audio.equalizer_enabled;
    if current.preset.as_deref() != Some(audio.equalizer_preset.as_str()) {
        match presets.get(&audio.equalizer_preset) {
            Some(preset) => {
                eq.mode = preset.mode;
                eq.preamp_db = preset.preamp_db;
                eq.bands = preset.bands;
                eq.preset = Some(preset.id);
            }
            None => tracing::warn!("unknown equalizer preset `{}`", audio.equalizer_preset),
        }
    }
    eq
}

impl Core {
    /// Puts the configured EQ and normaliser in place (startup).
    pub(super) fn init_audio_filters(&mut self) {
        let eq = eq_from_config(&self.config, &self.eq_presets, &self.mirror.state.eq);
        self.mirror.state.eq = eq;
        self.dirty = true;
        self.rebuild_eq();
        self.sync_normalize();
    }

    fn audio_running(&self) -> bool {
        self.mirror.state.media.is_some() && self.mirror.state.selected.audio.is_some()
    }

    pub(super) fn set_eq(&mut self, eq: EqState) {
        if eq == self.mirror.state.eq {
            return;
        }
        self.mirror.state.eq = eq.clone();
        self.dirty = true;
        if eq.enabled && self.audio_running() {
            if let Some(running) = &self.filters.eq_running {
                if let Some(commands) = equalizer::live_commands(running, &eq) {
                    for args in &commands {
                        let args: Vec<&str> = args.iter().map(String::as_str).collect();
                        self.run_command(&args);
                    }
                    self.filters.eq_running = Some(eq);
                    self.filters.eq_text_stale = !commands.is_empty() || self.filters.eq_text_stale;
                    return;
                }
            }
        }
        self.filters.eq_rebuild_due = Some(Instant::now() + EQ_REBUILD_DEBOUNCE);
    }

    /// Replaces the EQ entry with one built from the current state.
    pub(super) fn rebuild_eq(&mut self) {
        self.filters.eq_rebuild_due = None;
        self.filters.eq_text_stale = false;
        if self.filters.eq_running.take().is_some() {
            self.run_command(&["af", "remove", &format!("@{EQ_LABEL}")]);
        }
        let eq = self.mirror.state.eq.clone();
        if eq.enabled {
            // `pre`: the EQ comes before the normaliser and any user filters.
            self.run_command(&["af", "pre", &equalizer::eq_entry(&eq)]);
            self.filters.eq_running = Some(eq);
        }
    }

    /// A new file is starting: make the `af` text match what was heard last.
    pub(super) fn on_start_file(&mut self) {
        if self.filters.eq_text_stale || self.filters.eq_rebuild_due.is_some() {
            self.rebuild_eq();
        }
    }

    pub(super) fn tick_audio(&mut self, now: Instant) {
        if self.filters.eq_rebuild_due.is_some_and(|due| now >= due) {
            self.rebuild_eq();
        }
    }

    pub(super) fn sync_normalize(&mut self) {
        let wanted = self.config.audio.normalize;
        if wanted != self.filters.norm {
            if wanted {
                self.run_command(&["af", "add", NORM_ENTRY]);
            } else {
                self.run_command(&["af", "remove", &format!("@{NORM_LABEL}")]);
            }
            self.filters.norm = wanted;
        }
        if self.mirror.state.normalize != wanted {
            self.mirror.state.normalize = wanted;
            self.dirty = true;
        }
    }

    /// Settings changed: apply EQ and normaliser switches that came from the file (an edit
    /// made through the UI already matches the live state, so nothing happens twice).
    pub(super) fn audio_config_changed(&mut self, old: &Config) {
        if old.audio.normalize != self.config.audio.normalize {
            self.sync_normalize();
        }
        let (a, b) = (&old.audio, &self.config.audio);
        let live = &self.mirror.state.eq;
        let wanted = (b.equalizer_enabled, Some(b.equalizer_preset.as_str()));
        let changed = (a.equalizer_enabled, a.equalizer_preset.as_str())
            != (b.equalizer_enabled, b.equalizer_preset.as_str());
        if changed && (live.enabled, live.preset.as_deref()) != wanted {
            let eq = eq_from_config(&self.config, &self.eq_presets, live);
            self.set_eq(eq);
        }
    }

    /// The device list changed: when the chosen device is gone (unplugged), fall back to the
    /// system default instead of going silent.
    pub(super) fn check_audio_device(&mut self) {
        let state = &self.mirror.state;
        let device = state.audio_device.clone();
        if device == "auto" || state.audio_devices.is_empty() {
            return;
        }
        if state.audio_devices.iter().any(|d| d.name == device) {
            return;
        }
        tracing::info!("audio device {device} disappeared; using the system default");
        self.set_property("audio-device", super::mpv::Value::Text("auto".into()));
        let osd =
            OsdEvent::new(OsdKind::Warn, "Audio device disconnected — using the system default");
        self.emit(events::OSD_SHOW, &osd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_picks_preset_but_keeps_a_custom_curve_on_toggle() {
        let dir = std::env::temp_dir().join(format!("vihpluh-eqcfg-{}", std::process::id()));
        let presets = EqPresetStore::new(dir.join("eq-presets.toml"));
        let mut config = crate::config::defaults();
        config.audio.equalizer_enabled = true;
        config.audio.equalizer_preset = "dialogue".into();
        let eq = eq_from_config(&config, &presets, &EqState::default());
        assert!(eq.enabled);
        assert_eq!(eq.preset.as_deref(), Some("dialogue"));
        assert_eq!(eq.preamp_db, -2.0);
        assert_eq!(eq.bands[5].gain_db, 4.0);

        let mut custom = eq.clone();
        custom.bands[0].gain_db = 9.0;
        config.audio.equalizer_enabled = false;
        let off = eq_from_config(&config, &presets, &custom);
        assert!(!off.enabled);
        assert_eq!(off.bands[0].gain_db, 9.0, "toggling does not reset the curve");
    }
}
