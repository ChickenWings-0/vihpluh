use tauri::State;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::ipc::dto::{AbLoop, Direction, OpenRequest, PlayerState, SeekRequest, SleepRequest};

#[tauri::command]
pub fn player_snapshot(state: State<'_, AppState>) -> PlayerState {
    state.player.snapshot()
}

/// Folders expand to their media files, which touches the disk, so this runs off the main
/// thread.
#[tauri::command(async)]
pub fn player_open(state: State<'_, AppState>, mut req: OpenRequest) -> AppResult<()> {
    req.sources = crate::media::files::expand_sources(&req.sources);
    if req.sources.is_empty() {
        return Err(AppError::not_found("nothing playable there"));
    }
    state.player.open(req);
    Ok(())
}

#[tauri::command]
pub fn player_set_paused(state: State<'_, AppState>, paused: bool) {
    state.player.set_paused(paused);
}

#[tauri::command]
pub fn player_toggle_pause(state: State<'_, AppState>) {
    state.player.toggle_pause();
}

#[tauri::command]
pub fn player_seek(state: State<'_, AppState>, req: SeekRequest) -> AppResult<()> {
    if !req.value.is_finite() {
        return Err(AppError::invalid("seek target must be a finite number"));
    }
    Ok(state.player.seek(req)?)
}

#[tauri::command]
pub fn player_frame_step(state: State<'_, AppState>, direction: Direction) -> AppResult<()> {
    Ok(state.player.frame_step(direction)?)
}

#[tauri::command]
pub fn player_set_speed(state: State<'_, AppState>, value: f64) -> AppResult<()> {
    if !value.is_finite() {
        return Err(AppError::invalid("speed must be a finite number"));
    }
    Ok(state.player.set_speed(value)?)
}

#[tauri::command]
pub fn player_set_volume(state: State<'_, AppState>, value: f64) -> AppResult<()> {
    if !value.is_finite() {
        return Err(AppError::invalid("volume must be a finite number"));
    }
    Ok(state.player.set_volume(value)?)
}

#[tauri::command]
pub fn player_set_muted(state: State<'_, AppState>, muted: bool) -> AppResult<()> {
    Ok(state.player.set_muted(muted)?)
}

#[tauri::command]
pub fn player_set_direction(state: State<'_, AppState>, direction: Direction) -> AppResult<()> {
    Ok(state.player.set_direction(direction)?)
}

#[tauri::command]
pub fn player_set_loop_file(state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    Ok(state.player.set_loop_file(enabled)?)
}

#[tauri::command]
pub fn player_set_ab_loop(state: State<'_, AppState>, req: AbLoop) -> AppResult<()> {
    Ok(state.player.set_ab_loop(req)?)
}

#[tauri::command]
pub fn player_stop(state: State<'_, AppState>) {
    state.player.stop();
}

/// Waits for mpv to write the file, so it runs off the main thread.
#[tauri::command(async)]
pub fn player_screenshot(state: State<'_, AppState>, subtitles: bool) -> AppResult<String> {
    Ok(state.player.screenshot(subtitles)?)
}

/// Backs the `mpv.command` action only; nothing else in the frontend sends raw mpv commands.
#[tauri::command]
pub fn player_mpv_command(state: State<'_, AppState>, args: Vec<String>) -> AppResult<()> {
    if args.is_empty() {
        return Err(AppError::invalid("an mpv command needs at least a name"));
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    state.player.mpv_command(&args)?;
    Ok(())
}

#[tauri::command]
pub fn player_set_sleep_timer(state: State<'_, AppState>, req: SleepRequest) {
    state.player.set_sleep(req);
}

#[tauri::command]
pub fn inspector_subscribe(state: State<'_, AppState>, enabled: bool) {
    state.player.subscribe_inspector(enabled);
}
