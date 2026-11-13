use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use tauri::State;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::ipc::dto::{SubStylePatch, TrackKind};
use crate::library::sub_links::{self, LinkSource, SubLink};
use crate::player::subs::same_path;
use crate::subtitles::automatch::{self, SubMatchReport};
use crate::subtitles::{Coverage, CueWindow, TrackSource};

#[tauri::command]
pub fn sub_set_delay(state: State<'_, AppState>, ms: i32) -> AppResult<()> {
    if ms.abs() > 600_000 {
        return Err(AppError::invalid("subtitle delay is limited to ±10 minutes"));
    }
    Ok(state.player.set_sub_delay(ms)?)
}

#[tauri::command]
pub fn sub_set_visible(state: State<'_, AppState>, visible: bool) -> AppResult<()> {
    Ok(state.player.set_sub_visible(visible)?)
}

/// Live subtitle style: any subset of `SubStyle` (DESIGN_SYSTEM §10.1).
#[tauri::command]
pub fn sub_set_style(state: State<'_, AppState>, patch: SubStylePatch) -> AppResult<()> {
    Ok(state.player.set_sub_style(&patch)?)
}

/// Shifts the delay so the next (1) or previous (−1) line shows now.
#[tauri::command]
pub fn sub_step(state: State<'_, AppState>, direction: i32) -> AppResult<()> {
    if direction != 1 && direction != -1 {
        return Err(AppError::invalid("direction must be 1 or -1"));
    }
    Ok(state.player.sub_step(direction)?)
}

/// The cues of a subtitle track between `from` and `to` seconds, for the subtitle timeline
/// (ARCHITECTURE §11.3). Times are the file's own, before the subtitle delay.
#[tauri::command(async)]
pub fn sub_get_cues(state: State<'_, AppState>, track_id: i64, from: f64, to: f64) -> CueWindow {
    let none = CueWindow { cues: Vec::new(), coverage: Coverage::None, text: true };
    let Some(media) = state.player.current_media().filter(|m| !m.key.is_empty()) else {
        return none;
    };
    let tracks = state.player.current_tracks();
    let Some(track) = tracks.iter().find(|t| t.id == track_id && t.kind == TrackKind::Sub) else {
        return none;
    };
    // mpv numbers embedded subtitle tracks in file order, so the count of embedded tracks
    // before this one is its position in the container.
    let embedded_index = (!track.external).then(|| {
        tracks.iter().filter(|t| t.kind == TrackKind::Sub && !t.external && t.id < track_id).count()
    });
    let source = TrackSource {
        media_key: media.key,
        source: media.source,
        is_url: media.is_url,
        track_id,
        external_path: track.external_path.clone().filter(|_| track.external),
        text_based: track.text_based.unwrap_or(true),
        embedded_index,
    };
    state.cues.query(&source, from, to)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MatchScope {
    /// The videos in the queue.
    Queue,
    /// The videos in the current file's folder.
    Folder,
}

/// Pairs external subtitle files with videos (ARCHITECTURE §10.5). With `apply`, the pairs
/// are saved so each video loads its subtitles from now on, and the current file gets its
/// own at once.
#[tauri::command(async)]
pub fn sub_automatch(
    state: State<'_, AppState>,
    scope: MatchScope,
    apply: bool,
) -> AppResult<SubMatchReport> {
    let current = state.player.current_media().filter(|m| !m.is_url);
    let videos: Vec<String> = match scope {
        MatchScope::Queue => state
            .player
            .queue_state()
            .items
            .into_iter()
            .filter(|item| !item.is_url)
            .map(|item| item.source)
            .collect(),
        MatchScope::Folder => {
            let Some(media) = &current else {
                return Err(AppError::invalid("open a local file first"));
            };
            let dir = Path::new(&media.source).parent().unwrap_or(Path::new(""));
            crate::media::files::list_folder(dir)
                .into_iter()
                .map(|p| p.display().to_string())
                .collect()
        }
    };
    let dirs: BTreeSet<PathBuf> =
        videos.iter().filter_map(|v| Path::new(v).parent().map(Path::to_path_buf)).collect();
    let subs: Vec<String> = dirs
        .iter()
        .flat_map(|dir| automatch::candidates(dir))
        .map(|p| p.display().to_string())
        .collect();
    let mut report = automatch::plan(&videos, &subs);
    if !apply {
        return Ok(report);
    }

    let Some(db) = &state.db else {
        return Err(AppError::new(
            crate::error::ErrorCode::Db,
            "the library database is unavailable, so matches cannot be saved",
        ));
    };
    for pair in &report.matched {
        let key = match crate::media::identity::media_key(&pair.video) {
            Ok(key) => key,
            Err(err) => {
                tracing::warn!("auto-match: no media key for a video ({err}); skipped");
                continue;
            }
        };
        let links: Vec<SubLink> = pair
            .subs
            .iter()
            .map(|s| SubLink { path: s.path.clone(), lang: s.lang.clone() })
            .collect();
        db.call(move |c| {
            links.iter().try_for_each(|link| sub_links::add(c, &key, link, LinkSource::Auto))
        })?;
    }
    // The file that is playing gets its subtitles now; the rest get theirs when they load.
    if let Some(media) = &current {
        let mine = report.matched.iter().find(|pair| same_path(&pair.video, &media.source));
        let mut select = state.player.snapshot().selected.sub.is_none();
        for sub in mine.map(|pair| pair.subs.as_slice()).unwrap_or(&[]) {
            let present = state.player.current_tracks().iter().any(|t| {
                t.external && t.external_path.as_deref().is_some_and(|p| same_path(p, &sub.path))
            });
            if present {
                continue;
            }
            match state.player.add_external(TrackKind::Sub, &sub.path, select) {
                Ok(_) => select = false,
                Err(err) => tracing::warn!("auto-match: could not attach a subtitle: {err}"),
            }
        }
    }
    report.applied = true;
    Ok(report)
}

/// Installed font families, for the subtitle font picker.
#[tauri::command(async)]
pub fn fonts_list() -> Vec<String> {
    crate::platform::installed_fonts()
}
