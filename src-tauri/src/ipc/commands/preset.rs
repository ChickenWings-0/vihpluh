use tauri::State;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::player::presets::CaptureRequest;
use crate::presets::matcher::{self, PresetHit};
use crate::presets::{Preset, PresetMatch, PresetSummary};

#[tauri::command(async)]
pub fn preset_list(state: State<'_, AppState>) -> Vec<PresetSummary> {
    state.presets.list().iter().map(Preset::summary).collect()
}

#[tauri::command(async)]
pub fn preset_get(state: State<'_, AppState>, id: String) -> AppResult<Preset> {
    state.presets.get(&id).ok_or_else(|| AppError::not_found(format!("no preset named `{id}`")))
}

/// Writes `presets/<id>.toml`. The file is edited in place: only what changed is rewritten.
#[tauri::command(async)]
pub fn preset_save(state: State<'_, AppState>, preset: Preset) -> AppResult<()> {
    state.presets.save(&preset)
}

#[tauri::command(async)]
pub fn preset_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.presets.delete(&id)
}

/// A preset built from the live values that differ from the defaults, limited to the chosen
/// sections (ARCHITECTURE §10.4). It is returned, not saved: `preset_save` writes it.
#[tauri::command(async)]
pub fn preset_capture_current(
    state: State<'_, AppState>,
    name: String,
    r#match: PresetMatch,
    sections: Vec<String>,
) -> AppResult<Preset> {
    const SECTIONS: [&str; 4] = ["audio", "subtitles", "video", "playback"];
    if let Some(unknown) = sections.iter().find(|s| !SECTIONS.contains(&s.as_str())) {
        return Err(AppError::invalid(format!("`{unknown}` is not a preset section")));
    }
    state
        .player
        .captured_preset(CaptureRequest { name, matcher: r#match, sections })
        .map_err(AppError::invalid)
}

/// "Update": writes the live differences into the most specific applied preset.
#[tauri::command(async)]
pub fn preset_update_current(state: State<'_, AppState>) -> AppResult<String> {
    let preset = state.player.updated_preset().map_err(AppError::invalid)?;
    state.presets.save(&preset)?;
    Ok(preset.id)
}

/// Applies a preset to what is playing, on top of whatever matches it.
#[tauri::command]
pub fn preset_apply(state: State<'_, AppState>, id: String) -> AppResult<()> {
    if state.presets.get(&id).is_none() {
        return Err(AppError::not_found(format!("no preset named `{id}`")));
    }
    state.player.apply_preset(id);
    Ok(())
}

/// "Detach for this file": the live values stay, but no preset is tracked until the next file.
#[tauri::command]
pub fn preset_detach(state: State<'_, AppState>) {
    state.player.detach_presets();
}

/// The enabled presets that apply to a path or URL, in the order they are applied.
#[tauri::command(async)]
pub fn preset_matches_for(state: State<'_, AppState>, source: String) -> Vec<PresetHit> {
    matcher::matching(&state.presets.list(), &source)
        .into_iter()
        .map(|(preset, reason)| PresetHit { id: preset.id.clone(), reason })
        .collect()
}

/// Shows a preset's file in the system file manager.
#[tauri::command(async)]
pub fn preset_reveal(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let path = state.presets.file(&id);
    if !path.is_file() {
        return Err(AppError::not_found(format!("no preset named `{id}`")));
    }
    crate::platform::reveal_in_file_manager(&path)
        .map_err(|e| AppError::internal(format!("could not open the file manager: {e}")))
}
