//! Which files are media, and how a folder turns into an ordered list (ARCHITECTURE §8.1–§8.2).

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

pub const VIDEO_EXTENSIONS: &[&str] =
    &["mkv", "mp4", "m4v", "mov", "webm", "avi", "ts", "m2ts", "wmv", "flv", "mpg", "mpeg", "ogv"];
pub const AUDIO_EXTENSIONS: &[&str] =
    &["flac", "mp3", "m4a", "aac", "ogg", "opus", "wav", "wv", "ape", "aiff", "alac", "dsf"];

pub fn is_media_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|ext| {
        let ext = ext.to_ascii_lowercase();
        VIDEO_EXTENSIONS.contains(&ext.as_str()) || AUDIO_EXTENSIONS.contains(&ext.as_str())
    })
}

/// Compares names the way people read them: `E2` before `E10`, case-insensitive.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut digits = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(c);
                        it.next();
                    }
                    digits
                };
                let (da, db) = (take(&mut a), take(&mut b));
                let (ta, tb) = (da.trim_start_matches('0'), db.trim_start_matches('0'));
                let order = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                // Equal values: fewer leading zeros first, so the order is total.
                let order = order.then_with(|| da.len().cmp(&db.len()));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// The media files directly inside `dir`, in natural order. Unreadable folders are empty.
pub fn list_folder(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_media_file(path))
        .collect();
    files.sort_by(|a, b| {
        let name = |p: &PathBuf| p.file_name().map(|n| n.to_string_lossy().into_owned());
        natural_cmp(&name(a).unwrap_or_default(), &name(b).unwrap_or_default())
    });
    files
}

/// Replaces every folder in `sources` with its media files (non-recursive, natural order).
/// URLs and files pass through unchanged.
pub fn expand_sources(sources: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for source in sources {
        let path = Path::new(source);
        if !crate::cli::is_url(source) && path.is_dir() {
            out.extend(list_folder(path).into_iter().map(|p| p.display().to_string()));
        } else {
            out.push(source.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_media_extensions_case_insensitively() {
        assert!(is_media_file(Path::new("a/Episode.MKV")));
        assert!(is_media_file(Path::new("song.flac")));
        assert!(!is_media_file(Path::new("subs.srt")));
        assert!(!is_media_file(Path::new("noext")));
    }

    #[test]
    fn natural_order() {
        let mut names = vec!["E10.mkv", "e2.mkv", "E1.mkv", "E02.mkv", "Extra.mkv", "E100.mkv"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, vec!["E1.mkv", "e2.mkv", "E02.mkv", "E10.mkv", "E100.mkv", "Extra.mkv"]);
        assert_eq!(natural_cmp("Show S01E09", "Show S01E10"), Ordering::Less);
        assert_eq!(natural_cmp("a", "A"), Ordering::Equal);
    }

    #[test]
    fn folders_expand_to_their_media_in_order() {
        let dir = std::env::temp_dir().join(format!("vihpluh-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Subs")).unwrap();
        for name in ["E10.mkv", "E2.mkv", "E1.mkv", "notes.txt", "E1.srt"] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        let listed: Vec<String> = list_folder(&dir)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(listed, vec!["E1.mkv", "E2.mkv", "E10.mkv"]);

        let expanded =
            expand_sources(&["https://example.com/a.m3u8".to_string(), dir.display().to_string()]);
        assert_eq!(expanded.len(), 4);
        assert_eq!(expanded[0], "https://example.com/a.m3u8");
        assert!(expanded[1].ends_with("E1.mkv"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
