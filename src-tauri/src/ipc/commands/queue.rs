use tauri::State;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::player::QueueOp;
use crate::queue::{QueueState, Repeat};

#[tauri::command]
pub fn queue_snapshot(state: State<'_, AppState>) -> QueueState {
    state.player.queue_state()
}

/// Folders expand to their media files, which touches the disk.
#[tauri::command(async)]
pub fn queue_add(
    state: State<'_, AppState>,
    sources: Vec<String>,
    at: Option<usize>,
) -> AppResult<()> {
    let sources = crate::media::files::expand_sources(&sources);
    if sources.is_empty() {
        return Err(AppError::not_found("nothing playable there"));
    }
    state.player.queue(QueueOp::Add { sources, at });
    Ok(())
}

#[tauri::command]
pub fn queue_remove(state: State<'_, AppState>, ids: Vec<String>) {
    state.player.queue(QueueOp::Remove(ids));
}

#[tauri::command]
pub fn queue_move(state: State<'_, AppState>, ids: Vec<String>, to_index: usize) {
    state.player.queue(QueueOp::Move { ids, to_index });
}

#[tauri::command]
pub fn queue_clear(state: State<'_, AppState>, keep_current: bool) {
    state.player.queue(QueueOp::Clear { keep_current });
}

#[tauri::command]
pub fn queue_play(state: State<'_, AppState>, id: String) {
    state.player.queue(QueueOp::Play(id));
}

#[tauri::command]
pub fn queue_next(state: State<'_, AppState>) {
    state.player.queue(QueueOp::Next);
}

#[tauri::command]
pub fn queue_prev(state: State<'_, AppState>) {
    state.player.queue(QueueOp::Previous);
}

#[tauri::command]
pub fn queue_set_repeat(state: State<'_, AppState>, mode: Repeat) {
    state.player.queue(QueueOp::Repeat(mode));
}

#[tauri::command]
pub fn queue_set_shuffle(state: State<'_, AppState>, enabled: bool) {
    state.player.queue(QueueOp::Shuffle(enabled));
}

#[tauri::command]
pub fn queue_set_stop_after_current(state: State<'_, AppState>, enabled: bool) {
    state.player.queue(QueueOp::StopAfterCurrent(enabled));
}
