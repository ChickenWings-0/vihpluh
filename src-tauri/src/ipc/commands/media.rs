use serde::Serialize;
use tauri::State;
use tauri::ipc::Response;
use ts_rs::TS;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::media::waveform::WaveformStatus;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WaveformRequestResult {
    pub status: WaveformStatus,
}

/// Starts (or reports) the waveform for an audio track of the current file (§11.2).
#[tauri::command(async)]
pub fn waveform_request(
    state: State<'_, AppState>,
    media_key: String,
    track_id: i64,
) -> WaveformRequestResult {
    let mode = state.config.get().performance.waveform;
    WaveformRequestResult { status: state.waveform.request(&media_key, track_id, mode) }
}

/// The finished `.vwf` file, as raw bytes (an `ArrayBuffer` in the webview).
#[tauri::command(async)]
pub fn waveform_get(
    state: State<'_, AppState>,
    media_key: String,
    track_id: i64,
) -> AppResult<Response> {
    state
        .waveform
        .get(&media_key, track_id)
        .map(Response::new)
        .ok_or_else(|| AppError::not_found("no waveform for this track yet"))
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CacheInfo {
    #[ts(type = "number")]
    pub bytes: u64,
    #[ts(type = "number")]
    pub limit_bytes: u64,
}

/// Disk used by thumbnails and waveforms (§9.7).
#[tauri::command(async)]
pub fn cache_info(state: State<'_, AppState>) -> CacheInfo {
    let limit = u64::from(state.config.get().performance.cache_limit_mb) * 1024 * 1024;
    CacheInfo { bytes: crate::media::cache::size(&state.paths.cache), limit_bytes: limit }
}

/// "Clear cache": deletes every thumbnail and waveform. Always safe (§9.7).
#[tauri::command(async)]
pub fn cache_clear(state: State<'_, AppState>) -> AppResult<()> {
    state.waveform.cancel();
    crate::media::cache::clear(&state.paths.cache)?;
    Ok(())
}
