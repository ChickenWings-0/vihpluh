//! The thumbnail and waveform cache (ARCHITECTURE §9.7): capped at
//! `performance.cache_limit_mb`, least recently written first, checked at startup and hourly.
//! An entry is one file's thumbnail folder (`thumbs/<key>/`) or one waveform file. Deleting
//! any of it is always safe — everything is regenerated on demand.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

const AREAS: [&str; 2] = ["thumbs", "waveforms"];

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    pub bytes: u64,
    pub modified: SystemTime,
}

/// Total size and newest modification time of a file or folder tree.
fn measure(path: &Path) -> (u64, SystemTime) {
    let Ok(meta) = std::fs::metadata(path) else { return (0, SystemTime::UNIX_EPOCH) };
    let own = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    if meta.is_file() {
        return (meta.len(), own);
    }
    let mut total = 0;
    let mut newest = own;
    if let Ok(children) = std::fs::read_dir(path) {
        for child in children.filter_map(Result::ok) {
            let (bytes, modified) = measure(&child.path());
            total += bytes;
            newest = newest.max(modified);
        }
    }
    (total, newest)
}

pub fn entries(cache: &Path) -> Vec<Entry> {
    let mut out = Vec::new();
    for area in AREAS {
        let Ok(children) = std::fs::read_dir(cache.join(area)) else { continue };
        for child in children.filter_map(Result::ok) {
            let path = child.path();
            let (bytes, modified) = measure(&path);
            out.push(Entry { path, bytes, modified });
        }
    }
    out
}

pub fn size(cache: &Path) -> u64 {
    entries(cache).iter().map(|e| e.bytes).sum()
}

pub fn clear(cache: &Path) -> std::io::Result<()> {
    for area in AREAS {
        match std::fs::remove_dir_all(cache.join(area)) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

/// Which entries to delete so the total fits `limit` bytes: oldest first.
pub fn plan_eviction(mut entries: Vec<Entry>, limit: u64) -> Vec<Entry> {
    let mut total: u64 = entries.iter().map(|e| e.bytes).sum();
    entries.sort_by_key(|e| e.modified);
    let mut evict = Vec::new();
    for entry in entries {
        if total <= limit {
            break;
        }
        total -= entry.bytes;
        evict.push(entry);
    }
    evict
}

/// Deletes the oldest entries until the cache fits. Returns the bytes freed.
pub fn enforce(cache: &Path, limit: u64) -> u64 {
    let mut freed = 0;
    for entry in plan_eviction(entries(cache), limit) {
        let result = if entry.path.is_dir() {
            std::fs::remove_dir_all(&entry.path)
        } else {
            std::fs::remove_file(&entry.path)
        };
        match result {
            Ok(()) => freed += entry.bytes,
            Err(err) => tracing::debug!("cache: could not evict an entry: {err}"),
        }
    }
    if freed > 0 {
        tracing::info!(freed_mb = freed / (1024 * 1024), "cache: evicted old entries");
    }
    freed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn entry(name: &str, bytes: u64, age_s: u64) -> Entry {
        Entry {
            path: PathBuf::from(name),
            bytes,
            modified: SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 - age_s),
        }
    }

    #[test]
    fn evicts_oldest_until_under_the_limit() {
        let all = vec![entry("new", 40, 1), entry("old", 30, 300), entry("mid", 50, 100)];
        let names: Vec<_> = plan_eviction(all.clone(), 60)
            .into_iter()
            .map(|e| e.path.display().to_string())
            .collect();
        assert_eq!(names, ["old", "mid"]);
        assert!(plan_eviction(all, 1000).is_empty());
    }

    #[test]
    fn measures_clears_and_enforces_on_disk() {
        let dir = std::env::temp_dir().join(format!("vihpluh-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("thumbs/f_1/5")).unwrap();
        std::fs::create_dir_all(dir.join("waveforms")).unwrap();
        std::fs::write(dir.join("thumbs/f_1/5/0.jpg"), vec![0u8; 3000]).unwrap();
        std::fs::write(dir.join("waveforms/f_2.a1.vwf"), vec![0u8; 1000]).unwrap();
        std::fs::create_dir_all(dir.join("shaders")).unwrap();
        std::fs::write(dir.join("shaders/flip_h.glsl"), "x").unwrap();
        assert_eq!(size(&dir), 4000, "shaders are not cache entries");
        assert!(enforce(&dir, 2000) >= 1000);
        assert!(size(&dir) <= 2000);
        clear(&dir).unwrap();
        assert_eq!(size(&dir), 0);
        assert!(dir.join("shaders/flip_h.glsl").is_file(), "clear leaves the shaders alone");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
