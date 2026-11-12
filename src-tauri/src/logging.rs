//! Logging: `tracing` to stderr and to a size-rotated file (5 × 2 MB, ARCHITECTURE §9.1).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tracing::Level;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const MAX_BYTES: u64 = 2 * 1024 * 1024;
const KEEP_FILES: usize = 5;

pub fn parse_level(name: &str) -> Option<Level> {
    match name.to_ascii_lowercase().as_str() {
        "error" => Some(Level::ERROR),
        "warn" => Some(Level::WARN),
        "info" => Some(Level::INFO),
        "debug" => Some(Level::DEBUG),
        "trace" => Some(Level::TRACE),
        _ => None,
    }
}

/// Installs the global subscriber. A log file that cannot be opened is not fatal: logging
/// continues on stderr only.
pub fn init(log_file: &Path, level: Level) {
    let filter = LevelFilter::from_level(level);
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(io::stderr)
        .with_target(true)
        .with_ansi(false)
        .with_filter(filter);

    let file_layer = RotatingFile::open(log_file, MAX_BYTES, KEEP_FILES).ok().map(|writer| {
        tracing_subscriber::fmt::layer()
            .with_writer(writer)
            .with_target(true)
            .with_ansi(false)
            .with_filter(filter)
    });

    // A second init (tests, re-entry) is harmless; ignore the "already set" error.
    let _ = tracing_subscriber::registry().with(stderr_layer).with(file_layer).try_init();
}

/// Appends to `path`; when it would exceed `max_bytes` the file is rotated to `path.1`,
/// `path.1` to `path.2`, and so on, keeping `keep` files in total.
#[derive(Clone)]
pub struct RotatingFile {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    path: PathBuf,
    file: File,
    written: u64,
    max_bytes: u64,
    keep: usize,
}

impl RotatingFile {
    pub fn open(path: &Path, max_bytes: u64, keep: usize) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let written = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                path: path.to_path_buf(),
                file,
                written,
                max_bytes,
                keep: keep.max(1),
            })),
        })
    }
}

impl Inner {
    fn rotated(&self, index: usize) -> PathBuf {
        let mut name = self.path.as_os_str().to_owned();
        name.push(format!(".{index}"));
        PathBuf::from(name)
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        // path.(keep-2) → path.(keep-1), …, path → path.1; the oldest is overwritten.
        for index in (1..self.keep).rev() {
            let from = if index == 1 { self.path.clone() } else { self.rotated(index - 1) };
            if from.exists() {
                let _ = std::fs::remove_file(self.rotated(index));
                std::fs::rename(&from, self.rotated(index))?;
            }
        }
        self.file = OpenOptions::new().create(true).write(true).truncate(true).open(&self.path)?;
        self.written = 0;
        Ok(())
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().map_err(|_| io::Error::other("log lock poisoned"))?;
        if inner.written > 0 && inner.written + buf.len() as u64 > inner.max_bytes {
            // If rotation fails (file locked by another process) keep appending.
            let _ = inner.rotate();
        }
        let n = inner.file.write(buf)?;
        inner.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.lock().map_err(|_| io::Error::other("log lock poisoned"))?.file.flush()
    }
}

impl<'a> MakeWriter<'a> for RotatingFile {
    type Writer = RotatingFile;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vihpluh-log-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_levels_case_insensitively() {
        assert_eq!(parse_level("WARN"), Some(Level::WARN));
        assert_eq!(parse_level("trace"), Some(Level::TRACE));
        assert_eq!(parse_level("loud"), None);
    }

    #[test]
    fn rotates_when_the_limit_is_reached_and_keeps_a_bounded_set() {
        let dir = temp_dir("rotate");
        let path = dir.join("app.log");
        let mut log = RotatingFile::open(&path, 10, 3).unwrap();
        for chunk in ["aaaaaaaa", "bbbbbbbb", "cccccccc", "dddddddd"] {
            log.write_all(chunk.as_bytes()).unwrap();
        }
        log.flush().unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "dddddddd");
        assert_eq!(std::fs::read_to_string(dir.join("app.log.1")).unwrap(), "cccccccc");
        assert_eq!(std::fs::read_to_string(dir.join("app.log.2")).unwrap(), "bbbbbbbb");
        assert!(!dir.join("app.log.3").exists(), "only `keep` files survive");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn appends_to_an_existing_file() {
        let dir = temp_dir("append");
        let path = dir.join("app.log");
        std::fs::write(&path, "old ").unwrap();
        let mut log = RotatingFile::open(&path, 1024, 3).unwrap();
        log.write_all(b"new").unwrap();
        log.flush().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "old new");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
