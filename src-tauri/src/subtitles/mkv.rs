//! Cues of text subtitles embedded in Matroska files (ARCHITECTURE §11.3). The track's blocks
//! are read straight from the container, off-thread, so the whole timeline is known a moment
//! after the file opens instead of growing as playback goes.

use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use matroska_demuxer::{Frame, MatroskaFile, TrackType};

use super::cues::{self, Cue};

#[derive(Debug, thiserror::Error)]
pub enum MkvError {
    #[error("could not read the file: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a readable Matroska file: {0}")]
    Demux(String),
    #[error("cancelled")]
    Cancelled,
}

/// Matroska codec ids whose blocks are plain text we can read.
fn is_text_codec(codec_id: &str) -> bool {
    matches!(codec_id, "S_TEXT/UTF8" | "S_TEXT/ASS" | "S_TEXT/SSA" | "S_TEXT/WEBVTT")
}

pub fn is_matroska(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "mkv" | "mka" | "mks" | "webm"))
}

/// Reads every cue of the `sub_index`-th subtitle track (0-based, in file order — the order
/// mpv numbers embedded subtitle tracks in). `Ok(None)` when that track is missing or is not
/// a text format.
pub fn extract_file(
    path: &Path,
    sub_index: usize,
    cancel: &AtomicBool,
) -> Result<Option<Vec<Cue>>, MkvError> {
    let file = std::fs::File::open(path)?;
    extract(
        Buffered { inner: BufReader::with_capacity(256 * 1024, file), pos: 0 },
        sub_index,
        cancel,
    )
}

/// A `BufReader` that keeps its buffer across short seeks. The demuxer seeks to every block
/// header; `BufReader::seek` would throw the buffer away each time and re-read it from disk.
struct Buffered<R> {
    inner: BufReader<R>,
    pos: u64,
}

impl<R: Read> Read for Buffered<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl<R: Read + Seek> Seek for Buffered<R> {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let target = match to {
            SeekFrom::Start(at) => at,
            SeekFrom::Current(by) => self.pos.saturating_add_signed(by),
            SeekFrom::End(_) => {
                self.pos = self.inner.seek(to)?;
                return Ok(self.pos);
            }
        };
        let by = target as i64 - self.pos as i64;
        self.inner.seek_relative(by)?;
        self.pos = target;
        Ok(target)
    }
}

pub fn extract<R: Read + Seek>(
    reader: R,
    sub_index: usize,
    cancel: &AtomicBool,
) -> Result<Option<Vec<Cue>>, MkvError> {
    let mut mkv = MatroskaFile::open(reader).map_err(|e| MkvError::Demux(e.to_string()))?;
    let Some(track) =
        mkv.tracks().iter().filter(|t| t.track_type() == TrackType::Subtitle).nth(sub_index)
    else {
        return Ok(None);
    };
    let codec = track.codec_id().to_string();
    if !is_text_codec(&codec) {
        return Ok(None);
    }
    // Compressed tracks (old mkvmerge zlib headers) would need inflating; none is readable
    // as text, so they are left to the progressive index.
    if track.content_encodings().is_some_and(|encodings| !encodings.is_empty()) {
        return Ok(None);
    }
    let number = track.track_number().get();
    // Block times count in units of `timestamp_scale` nanoseconds.
    let unit = mkv.info().timestamp_scale().get() as f64 / 1e9;

    let mut found = Vec::new();
    let mut frame = Frame::default();
    let mut seen = 0u32;
    loop {
        match mkv.next_frame(&mut frame) {
            Ok(true) => {}
            Ok(false) => break,
            // A truncated file still has every cue up to the damage.
            Err(err) => {
                tracing::debug!("subtitle extraction stopped early: {err}");
                break;
            }
        }
        seen = seen.wrapping_add(1);
        if seen % 512 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(MkvError::Cancelled);
        }
        if frame.track != number {
            continue;
        }
        let start = frame.timestamp as f64 * unit;
        // Without a block duration the cue length is unknown; two seconds draws a block.
        let end = start + frame.duration.map_or(2.0, |d| d as f64 * unit);
        let raw = String::from_utf8_lossy(&frame.data);
        found.push(Cue { start, end, text: block_text(&codec, &raw) });
    }
    cues::finish(&mut found);
    Ok(Some(found))
}

/// The readable text of one subtitle block.
fn block_text(codec: &str, raw: &str) -> Option<String> {
    match codec {
        // `ReadOrder,Layer,Style,Name,MarginL,MarginR,MarginV,Effect,Text`
        "S_TEXT/ASS" | "S_TEXT/SSA" => cues::ass_text(raw.splitn(9, ',').nth(8)?),
        _ => {
            let text = cues::strip_markup(raw);
            let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
            (!lines.is_empty()).then(|| lines.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// An 8 s file made with ffmpeg: one Opus track, a SubRip track, an ASS track.
    const FIXTURE: &[u8] = include_bytes!("fixtures/two-subs.mkv");

    fn cue(start: f64, end: f64, text: &str) -> Cue {
        Cue { start, end, text: Some(text.to_string()) }
    }

    fn extract_fixture(index: usize) -> Option<Vec<Cue>> {
        extract(Cursor::new(FIXTURE), index, &AtomicBool::new(false)).unwrap()
    }

    #[test]
    fn subrip_track() {
        assert_eq!(
            extract_fixture(0).unwrap(),
            vec![
                cue(1.0, 2.5, "First line"),
                cue(3.0, 4.0, "Second line"),
                cue(5.25, 7.0, "Third\nline"),
            ]
        );
    }

    #[test]
    fn ass_track_text_keeps_commas_and_drops_overrides() {
        assert_eq!(
            extract_fixture(1).unwrap(),
            vec![cue(0.5, 1.5, "Styled, with a comma"), cue(6.0, 7.5, "Two\nlines")]
        );
    }

    #[test]
    fn a_missing_track_is_none() {
        assert_eq!(extract_fixture(2), None);
    }

    #[test]
    fn cancellation_and_damage() {
        // Truncated anywhere: an error or a shorter list, never a panic.
        for cut in (0..FIXTURE.len()).step_by(97) {
            let _ = extract(Cursor::new(&FIXTURE[..cut]), 0, &AtomicBool::new(false));
        }
        let garbage = vec![0xA5u8; 4096];
        assert!(extract(Cursor::new(garbage), 0, &AtomicBool::new(false)).is_err());
    }

    #[test]
    fn the_buffered_reader_gives_the_same_cues_as_a_plain_one() {
        let reader = Buffered { inner: BufReader::with_capacity(64, Cursor::new(FIXTURE)), pos: 0 };
        let cues = extract(reader, 1, &AtomicBool::new(false)).unwrap();
        assert_eq!(cues, extract_fixture(1));
        let reader = Buffered { inner: BufReader::with_capacity(64, Cursor::new(FIXTURE)), pos: 0 };
        assert_eq!(extract(reader, 0, &AtomicBool::new(false)).unwrap(), extract_fixture(0));
    }

    #[test]
    fn containers_by_extension() {
        assert!(is_matroska(Path::new("a.MKV")));
        assert!(is_matroska(Path::new("a.webm")));
        assert!(!is_matroska(Path::new("a.mp4")));
    }
}
