use tauri::State;

use crate::AppState;
use crate::config::eq_presets;
use crate::error::{AppError, AppResult};
use crate::ipc::dto::{EqPreset, EqState};

#[tauri::command]
pub fn audio_set_delay(state: State<'_, AppState>, ms: i32) -> AppResult<()> {
    if ms.abs() > 600_000 {
        return Err(AppError::invalid("audio delay is limited to ±10 minutes"));
    }
    Ok(state.player.set_audio_delay(ms)?)
}

#[tauri::command]
pub fn audio_set_device(state: State<'_, AppState>, name: String) -> AppResult<()> {
    Ok(state.player.set_audio_device(&name)?)
}

/// Applies an equalizer state. Gain and preamp moves reach the running filter at once; the
/// on/off switch and the chosen preset are also saved as the defaults for next time.
#[tauri::command(async)]
pub fn audio_set_eq(state: State<'_, AppState>, eq: EqState) -> AppResult<()> {
    let as_preset = EqPreset {
        id: String::new(),
        name: String::new(),
        mode: eq.mode.clone(),
        preamp_db: eq.preamp_db,
        bands: eq.bands.clone(),
        builtin: false,
    };
    eq_presets::validate(&as_preset).map_err(AppError::invalid)?;
    state.player.set_eq(eq.clone());
    let config = state.config.get();
    if config.audio.equalizer_enabled != eq.enabled {
        state.config.set("audio.equalizer_enabled", &serde_json::Value::Bool(eq.enabled))?;
    }
    if let Some(preset) = eq.preset.filter(|p| *p != config.audio.equalizer_preset) {
        state.config.set("audio.equalizer_preset", &serde_json::Value::String(preset))?;
    }
    Ok(())
}

/// Volume normalisation is a setting (`audio.normalize`); the core follows it live.
#[tauri::command(async)]
pub fn audio_set_normalize(state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    state.config.set("audio.normalize", &serde_json::Value::Bool(enabled))
}

#[tauri::command(async)]
pub fn eq_presets_list(state: State<'_, AppState>) -> Vec<EqPreset> {
    state.eq_presets.list()
}

#[tauri::command(async)]
pub fn eq_preset_save(state: State<'_, AppState>, preset: EqPreset) -> AppResult<()> {
    state.eq_presets.save(&preset)
}

#[tauri::command(async)]
pub fn eq_preset_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.eq_presets.delete(&id)
}
