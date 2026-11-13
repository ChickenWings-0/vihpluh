//! A one-way byte channel from a headless mpv (`ao=pcm`, `ao-pcm-file=<path>`) to a reader
//! thread, for waveform decoding (ARCHITECTURE §11.2).
//!
//! - Windows: an inbound byte-mode named pipe (`\\.\pipe\…`), verified in spike S4.
//! - Elsewhere: a temporary file read as it grows and deleted afterwards (the documented
//!   fallback; the Unix FIFO path is checked with spike S2 on Fedora).
//!
//! The reader ends when the writer is done: for the pipe when mpv closes it, for the file when
//! the caller sets the `finished` flag and the file is drained.

use std::io::{self, Read};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct PcmPipe {
    /// What mpv writes to.
    pub path: String,
    inner: Inner,
}

enum Inner {
    #[cfg(windows)]
    Pipe(super::windows::NamedPipe),
    #[cfg_attr(windows, allow(dead_code))]
    File(PathBuf),
}

impl PcmPipe {
    pub fn create(tag: &str) -> io::Result<Self> {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!("vihpluh-{tag}-{}-{n}", std::process::id());
        #[cfg(windows)]
        {
            let path = format!(r"\\.\pipe\{name}");
            let pipe = super::windows::NamedPipe::create_inbound(&path)?;
            Ok(Self { path, inner: Inner::Pipe(pipe) })
        }
        #[cfg(not(windows))]
        {
            let path = std::env::temp_dir().join(format!("{name}.pcm"));
            std::fs::File::create(&path)?;
            Ok(Self { path: path.display().to_string(), inner: Inner::File(path) })
        }
    }

    /// Waits for the writer and returns the reading end. `finished` must be set once the
    /// writer (mpv) has stopped; on Windows it also releases a reader still waiting for a
    /// writer that never came.
    pub fn into_reader(self, finished: Arc<AtomicBool>) -> io::Result<Box<dyn Read + Send>> {
        match self.inner {
            #[cfg(windows)]
            Inner::Pipe(pipe) => Ok(Box::new(pipe.connect()?)),
            Inner::File(path) => {
                let file = std::fs::File::open(&path)?;
                Ok(Box::new(GrowingFile { file, path, finished }))
            }
        }
    }

    /// Unblocks a reader waiting in `into_reader` when the writer will never connect (the job
    /// was cancelled before mpv opened its output).
    pub fn release(path: &str) {
        if cfg!(windows) {
            // Connecting as a client completes the server's ConnectNamedPipe; closing at once
            // gives the reader end-of-file.
            let _ = std::fs::OpenOptions::new().write(true).open(path);
        }
    }
}

/// A file that another process is still appending to.
struct GrowingFile {
    file: std::fs::File,
    path: PathBuf,
    finished: Arc<AtomicBool>,
}

impl Read for GrowingFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let done = self.finished.load(Ordering::Acquire);
            let n = self.file.read(buf)?;
            if n > 0 || done {
                return Ok(n);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

impl Drop for GrowingFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
