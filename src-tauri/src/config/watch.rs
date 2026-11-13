//! Watching one file for external edits, debounced (editors often save in several steps:
//! truncate, write, rename). Shared by `config.toml` and `keybinds.toml`.

use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::error::{AppError, AppResult};

const DEBOUNCE: Duration = Duration::from_millis(150);

/// Calls `on_change` (on a background thread) after `path` was created, written, renamed,
/// or removed and then left alone for a moment. The parent directory is created if needed so
/// there is something to watch. Keep the returned watcher alive for as long as watching
/// should continue; `on_change` returns `false` to stop.
pub fn watch_file(
    path: &Path,
    name: &str,
    mut on_change: impl FnMut() -> bool + Send + 'static,
) -> AppResult<RecommendedWatcher> {
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    std::fs::create_dir_all(&dir)?;
    let file_name = path.file_name().map(ToOwned::to_owned);

    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        let Ok(event) = result else { return };
        let touches_file = event.paths.iter().any(|p| p.file_name() == file_name.as_deref());
        if touches_file && !event.kind.is_access() {
            let _ = tx.send(());
        }
    })
    .map_err(|e| AppError::internal(format!("cannot watch {name}: {e}")))?;
    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|e| AppError::internal(format!("cannot watch {}: {e}", dir.display())))?;

    std::thread::Builder::new()
        .name(format!("watch-{name}"))
        .spawn(move || {
            while rx.recv().is_ok() {
                loop {
                    match rx.recv_timeout(DEBOUNCE) {
                        Ok(()) => {}
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                if !on_change() {
                    return;
                }
            }
        })
        .map_err(|e| AppError::internal(format!("cannot start the {name} watcher: {e}")))?;
    Ok(watcher)
}
