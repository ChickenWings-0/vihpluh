use tauri::State;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::ipc::dto::{TrackKind, TrackSlot};
use crate::library::sub_links::{self, LinkSource, SubLink};

#[tauri::command]
pub fn tracks_select(
    state: State<'_, AppState>,
    kind: TrackSlot,
    id: Option<i64>,
) -> AppResult<()> {
    Ok(state.player.select_track(kind, id)?)
}

/// The identity of the file that is playing, for remembering what was added to it.
fn current_key(state: &AppState) -> Option<String> {
    state.player.current_media().map(|m| m.key).filter(|key| !key.is_empty())
}

/// Adds an external subtitle or audio file to the playing media. A subtitle file is
/// remembered for that media and comes back on later loads (ARCHITECTURE §10.5).
#[tauri::command(async)]
pub fn tracks_add_external(
    state: State<'_, AppState>,
    kind: TrackKind,
    path: String,
    select: bool,
) -> AppResult<i64> {
    if !std::path::Path::new(&path).is_file() {
        return Err(AppError::not_found(format!("{path} does not exist")));
    }
    let id = state.player.add_external(kind, &path, select)?;
    if kind == TrackKind::Sub {
        if let (Some(db), Some(key)) = (&state.db, current_key(&state)) {
            let link = SubLink { path, lang: None };
            db.spawn("remember subtitle file", move |c| {
                sub_links::add(c, &key, &link, LinkSource::Manual)
            });
        }
    }
    Ok(id)
}

/// Removes an external track; a subtitle file is also forgotten for this media.
#[tauri::command(async)]
pub fn tracks_remove_external(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let (kind, path) = state.player.remove_external(id)?;
    if let (TrackKind::Sub, Some(path), Some(db), Some(key)) =
        (kind, path, &state.db, current_key(&state))
    {
        // Links are stored as given; compare loosely so `\` vs `/` cannot leave one behind.
        db.spawn("forget subtitle file", move |c| {
            for link in sub_links::for_media(c, &key)? {
                if crate::player::subs::same_path(&link.path, &path) {
                    sub_links::remove(c, &key, &link.path)?;
                }
            }
            Ok(())
        });
    }
    Ok(())
}
