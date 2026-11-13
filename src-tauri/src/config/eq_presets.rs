//! Equalizer presets (KEYBINDS_AND_CONFIG §5.12): the shipped built-ins
//! (`resources/eq-presets.toml`) plus the user's `eq-presets.toml`, where a preset with a
//! built-in's id replaces it. The user file is small and read on every call, so hand edits
//! show up without a watcher. Saving rewrites the user file (comments in it are not kept).

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::ipc::dto::{EqBand, EqPreset, GRAPHIC_EQ_FREQS};

const BUILTIN_TOML: &str = include_str!("../../resources/eq-presets.toml");
const MAX_PARAMETRIC_BANDS: usize = 8;

#[derive(Debug, Default, Serialize, Deserialize)]
struct PresetFile {
    #[serde(default = "schema_version")]
    schema: u32,
    #[serde(default)]
    preset: Vec<PresetRow>,
}

fn schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PresetRow {
    id: String,
    name: String,
    #[serde(default = "graphic")]
    mode: String,
    #[serde(default)]
    preamp: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gains: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bands: Option<Vec<BandRow>>,
}

fn graphic() -> String {
    "graphic10".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BandRow {
    #[serde(rename = "type", default = "peak")]
    kind: String,
    freq: f64,
    gain: f64,
    #[serde(default = "default_q")]
    q: f64,
}

fn peak() -> String {
    "peak".to_string()
}

fn default_q() -> f64 {
    0.7
}

impl PresetRow {
    fn to_preset(&self, builtin: bool) -> Result<EqPreset, String> {
        let bands = match self.mode.as_str() {
            "graphic10" => {
                let gains = self.gains.clone().unwrap_or_default();
                if gains.len() != GRAPHIC_EQ_FREQS.len() {
                    return Err(format!("preset `{}` needs exactly 10 gains", self.id));
                }
                graphic_bands(&gains)
            }
            "parametric" => self
                .bands
                .clone()
                .unwrap_or_default()
                .into_iter()
                .take(MAX_PARAMETRIC_BANDS)
                .map(|b| EqBand { freq: b.freq, gain_db: b.gain, q: b.q, kind: b.kind })
                .collect(),
            other => return Err(format!("preset `{}` has unknown mode `{other}`", self.id)),
        };
        let preset = EqPreset {
            id: self.id.clone(),
            name: self.name.clone(),
            mode: self.mode.clone(),
            preamp_db: self.preamp,
            bands,
            builtin,
        };
        validate(&preset).map_err(|e| format!("preset `{}`: {e}", self.id))?;
        Ok(preset)
    }

    fn from_preset(preset: &EqPreset) -> Self {
        let graphic = preset.mode == "graphic10";
        Self {
            id: preset.id.clone(),
            name: preset.name.clone(),
            mode: preset.mode.clone(),
            preamp: preset.preamp_db,
            gains: graphic.then(|| preset.bands.iter().map(|b| b.gain_db).collect()),
            bands: (!graphic).then(|| {
                preset
                    .bands
                    .iter()
                    .map(|b| BandRow {
                        kind: b.kind.clone(),
                        freq: b.freq,
                        gain: b.gain_db,
                        q: b.q,
                    })
                    .collect()
            }),
        }
    }
}

/// The ten graphic-EQ bands with the given gains.
pub fn graphic_bands(gains: &[f64]) -> Vec<EqBand> {
    GRAPHIC_EQ_FREQS
        .iter()
        .zip(gains)
        .map(|(&freq, &gain_db)| EqBand { freq, gain_db, q: 1.0, kind: "peak".to_string() })
        .collect()
}

/// Range checks shared by presets and `audio_set_eq`.
pub fn validate(preset: &EqPreset) -> Result<(), String> {
    if !(-24.0..=24.0).contains(&preset.preamp_db) {
        return Err("preamp must be within ±24 dB".into());
    }
    match preset.mode.as_str() {
        "graphic10" if preset.bands.len() != GRAPHIC_EQ_FREQS.len() => {
            return Err("a 10-band preset needs 10 bands".into());
        }
        "graphic10" | "parametric" => {}
        other => return Err(format!("unknown mode `{other}`")),
    }
    if preset.bands.len() > MAX_PARAMETRIC_BANDS.max(GRAPHIC_EQ_FREQS.len()) {
        return Err("too many bands".into());
    }
    for band in &preset.bands {
        if !(20.0..=20_000.0).contains(&band.freq) {
            return Err(format!("band frequency {} Hz is outside 20 Hz – 20 kHz", band.freq));
        }
        if !(-24.0..=24.0).contains(&band.gain_db) {
            return Err("band gains must be within ±24 dB".into());
        }
        if !(0.1..=10.0).contains(&band.q) {
            return Err("Q must be within 0.1–10".into());
        }
        if !matches!(band.kind.as_str(), "peak" | "lowshelf" | "highshelf") {
            return Err(format!("unknown band type `{}`", band.kind));
        }
    }
    Ok(())
}

fn parse(text: &str) -> Result<PresetFile, String> {
    toml::from_str::<PresetFile>(text).map_err(|e| e.message().to_string())
}

fn builtins() -> Vec<EqPreset> {
    match parse(BUILTIN_TOML) {
        Ok(file) => file.preset.iter().filter_map(|row| row.to_preset(true).ok()).collect(),
        Err(err) => {
            tracing::error!("built-in EQ presets are invalid: {err}");
            Vec::new()
        }
    }
}

pub struct EqPresetStore {
    path: PathBuf,
    /// Serialises read-modify-write of the user file.
    write: Mutex<()>,
}

impl EqPresetStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path, write: Mutex::new(()) }
    }

    fn user_rows(&self) -> Vec<PresetRow> {
        let Ok(text) = std::fs::read_to_string(&self.path) else { return Vec::new() };
        match parse(&text) {
            Ok(file) => file.preset,
            Err(err) => {
                tracing::warn!("eq-presets.toml could not be read: {err}");
                Vec::new()
            }
        }
    }

    /// Built-ins in their shipped order (replaced in place by user overrides), then the
    /// user's own presets.
    pub fn list(&self) -> Vec<EqPreset> {
        let mut presets = builtins();
        for row in self.user_rows() {
            match row.to_preset(false) {
                Ok(preset) => match presets.iter_mut().find(|p| p.id == preset.id) {
                    Some(slot) => *slot = preset,
                    None => presets.push(preset),
                },
                Err(err) => tracing::warn!("eq-presets.toml: {err}"),
            }
        }
        presets
    }

    pub fn get(&self, id: &str) -> Option<EqPreset> {
        self.list().into_iter().find(|p| p.id == id)
    }

    pub fn save(&self, preset: &EqPreset) -> AppResult<()> {
        if preset.id.trim().is_empty() || preset.name.trim().is_empty() {
            return Err(AppError::invalid("a preset needs an id and a name"));
        }
        validate(preset).map_err(AppError::invalid)?;
        let _guard = self.write.lock();
        let mut rows = self.user_rows();
        let row = PresetRow::from_preset(preset);
        match rows.iter_mut().find(|r| r.id == preset.id) {
            Some(slot) => *slot = row,
            None => rows.push(row),
        }
        self.write_rows(rows)
    }

    /// Removes a user preset. Deleting an override brings the built-in back; a built-in
    /// itself cannot be deleted.
    pub fn delete(&self, id: &str) -> AppResult<()> {
        let _guard = self.write.lock();
        let mut rows = self.user_rows();
        let before = rows.len();
        rows.retain(|r| r.id != id);
        if rows.len() == before {
            return Err(AppError::not_found(format!("no user preset `{id}`")));
        }
        self.write_rows(rows)
    }

    fn write_rows(&self, rows: Vec<PresetRow>) -> AppResult<()> {
        let file = PresetFile { schema: 1, preset: rows };
        let text = toml::to_string(&file).map_err(|e| AppError::internal(e.to_string()))?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (EqPresetStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "vihpluh-eq-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        (EqPresetStore::new(dir.join("eq-presets.toml")), dir)
    }

    fn rand_suffix() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    #[test]
    fn builtins_are_complete_and_valid() {
        let ids: Vec<String> = builtins().into_iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            [
                "flat",
                "bass",
                "treble",
                "dialogue",
                "night",
                "loudness",
                "rock",
                "classical",
                "electronic"
            ]
        );
    }

    #[test]
    fn user_presets_override_and_append() {
        let (store, dir) = store();
        let mut flat = store.get("flat").unwrap();
        flat.name = "My flat".into();
        flat.builtin = false;
        store.save(&flat).unwrap();
        let parametric = EqPreset {
            id: "hd600".into(),
            name: "HD 600".into(),
            mode: "parametric".into(),
            preamp_db: -4.5,
            bands: vec![
                EqBand { freq: 105.0, gain_db: 5.5, q: 0.7, kind: "lowshelf".into() },
                EqBand { freq: 3300.0, gain_db: -2.0, q: 2.0, kind: "peak".into() },
            ],
            builtin: false,
        };
        store.save(&parametric).unwrap();

        let list = store.list();
        assert_eq!(list[0].name, "My flat", "override keeps the built-in's place");
        assert!(!list[0].builtin);
        assert_eq!(list.last().unwrap(), &parametric, "round-trips through the file");

        store.delete("flat").unwrap();
        assert_eq!(store.get("flat").unwrap().name, "Flat", "deleting an override restores it");
        assert!(store.delete("bass").is_err(), "built-ins cannot be deleted");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_out_of_range_values() {
        let mut preset = builtins().remove(0);
        preset.bands[0].gain_db = 40.0;
        assert!(validate(&preset).is_err());
        let mut preset = builtins().remove(0);
        preset.bands.pop();
        assert!(validate(&preset).is_err(), "graphic presets need all ten bands");
    }
}
