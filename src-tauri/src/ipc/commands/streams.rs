use std::sync::Arc;
use std::time::Duration;

use tauri::State;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::library::db::Db;
use crate::library::iptv::{self, StreamGroup, StreamPage, StreamSource};

fn db(state: &AppState) -> AppResult<&Arc<Db>> {
    state.db.as_ref().ok_or_else(|| AppError::unsupported("the library database is unavailable"))
}

/// The channel names stream lists give the URLs among `sources`, so the queue shows them.
/// Runs on the calling (worker) thread; an unavailable database just means no names.
pub fn channel_titles(
    state: &AppState,
    sources: &[String],
) -> std::collections::HashMap<String, String> {
    let urls: Vec<String> = sources.iter().filter(|s| crate::cli::is_url(s)).cloned().collect();
    match (&state.db, urls.is_empty()) {
        (Some(db), false) => db.call(move |c| iptv::names_for(c, &urls)).unwrap_or_else(|err| {
            tracing::warn!("channel name lookup failed: {err}");
            Default::default()
        }),
        _ => Default::default(),
    }
}

#[tauri::command(async)]
pub fn streams_sources(state: State<'_, AppState>) -> AppResult<Vec<StreamSource>> {
    Ok(db(&state)?.call(|c| iptv::sources(c))?)
}

/// Reads a source's list — a local `.m3u`, or a URL fetched through mpv's network stack with
/// the configured user agent and proxy — and replaces its channels.
fn refresh(state: &AppState, id: i64) -> AppResult<usize> {
    let db = db(state)?;
    let source = db
        .call(move |c| iptv::source(c, id))?
        .ok_or_else(|| AppError::not_found("that stream source no longer exists"))?;
    let bytes = if crate::cli::is_url(&source.location) {
        let config = state.config.get();
        let mut options = Vec::new();
        if !config.network.user_agent.is_empty() {
            options.push(("user-agent".to_string(), config.network.user_agent.clone()));
        }
        if !config.network.proxy.is_empty() {
            options.push(("http-proxy".to_string(), config.network.proxy.clone()));
        }
        options.push(("network-timeout".to_string(), config.network.timeout_s.to_string()));
        let dir = state.paths.cache.join("streams");
        std::fs::create_dir_all(&dir)?;
        let file = dir.join(format!("source-{id}.m3u"));
        let limit = Duration::from_secs(u64::from(config.network.timeout_s) + 30);
        let fetched = crate::player::headless::fetch_url(&source.location, &file, &options, limit);
        let bytes = fetched.map_err(AppError::from).and_then(|()| Ok(std::fs::read(&file)?));
        let _ = std::fs::remove_file(&file);
        bytes.map_err(|err| {
            AppError::new(err.code, format!("could not download the list: {}", err.message))
        })?
    } else {
        std::fs::read(&source.location).map_err(|err| {
            AppError::not_found(format!("could not read {}: {err}", source.location))
        })?
    };
    let channels = iptv::parse(&crate::subtitles::cues::decode(&bytes));
    if channels.is_empty() {
        return Err(AppError::invalid("that list has no channels"));
    }
    let count = channels.len();
    db.call(move |c| iptv::replace_channels(c, id, &channels))?;
    Ok(count)
}

/// Adds a stream source (a URL or a file) and loads its channels. Returns its id.
#[tauri::command(async)]
pub fn streams_add_source(
    state: State<'_, AppState>,
    name: String,
    location: String,
) -> AppResult<i64> {
    let (name, location) = (name.trim().to_string(), location.trim().to_string());
    if location.is_empty() {
        return Err(AppError::invalid("give the list's address or file"));
    }
    let name = if name.is_empty() { crate::queue::title_for(&location) } else { name };
    let db = db(&state)?;
    let stored = location.clone();
    let id = db.call(move |c| iptv::add_source(c, &name, &stored))?;
    if let Err(err) = refresh(&state, id) {
        // A source that never loaded is not kept.
        db.call(move |c| iptv::remove_source(c, id))?;
        return Err(err);
    }
    Ok(id)
}

/// Re-reads a source and replaces its channels. Nothing refreshes by itself.
#[tauri::command(async)]
pub fn streams_refresh(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    refresh(&state, id).map(|_| ())
}

#[tauri::command(async)]
pub fn streams_remove_source(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    Ok(db(&state)?.call(move |c| iptv::remove_source(c, id))?)
}

#[tauri::command(async)]
pub fn streams_groups(state: State<'_, AppState>, source_id: i64) -> AppResult<Vec<StreamGroup>> {
    Ok(db(&state)?.call(move |c| iptv::groups(c, source_id))?)
}

/// Channels of a source and group (`group: ""` lists the ungrouped ones), or every channel
/// matching `search`.
#[tauri::command(async)]
pub fn streams_list(
    state: State<'_, AppState>,
    source_id: Option<i64>,
    group: Option<String>,
    search: Option<String>,
    limit: u32,
    offset: u32,
) -> AppResult<StreamPage> {
    let ungrouped = group.as_deref() == Some("");
    let group = group.filter(|g| !g.is_empty());
    Ok(db(&state)?.call(move |c| {
        iptv::list(
            c,
            source_id,
            group.as_deref(),
            ungrouped,
            search.as_deref(),
            limit.clamp(1, 500),
            offset,
        )
    })?)
}
