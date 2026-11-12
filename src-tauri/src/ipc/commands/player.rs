use tauri::State;

use crate::AppState;
use crate::error::{AppError, AppResult};

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
