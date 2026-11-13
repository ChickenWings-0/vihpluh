//! Music metadata from tags (ARCHITECTURE §8.2), read with `lofty`. A file whose tags cannot
//! be read is still indexed, under its file name.

use std::path::Path;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::ItemKey;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<u32>,
    pub duration: Option<f64>,
}

/// `3`, `03`, or `3/12` → 3.
pub fn leading_number(text: &str) -> Option<u32> {
    let digits: String = text.trim().chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok().filter(|n| *n > 0)
}

/// `1961`, `1961-06-25`, `1961/06` → 1961.
pub fn year_of(text: &str) -> Option<u32> {
    let text = text.trim();
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    (digits.len() == 4).then(|| digits.parse().ok()).flatten().filter(|y| (1000..=9999).contains(y))
}

pub fn read(path: &Path) -> Option<Tags> {
    let file = match lofty::read_from_path(path) {
        Ok(file) => file,
        Err(err) => {
            tracing::debug!("tags could not be read: {err}");
            return None;
        }
    };
    let duration = file.properties().duration().as_secs_f64();
    let mut tags = Tags { duration: (duration > 0.0).then_some(duration), ..Tags::default() };
    let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) else { return Some(tags) };
    let text = |key: ItemKey| {
        tag.get_string(key).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
    };
    tags.title = text(ItemKey::TrackTitle);
    tags.artist = text(ItemKey::TrackArtist);
    tags.album_artist = text(ItemKey::AlbumArtist);
    tags.album = text(ItemKey::AlbumTitle);
    tags.track_no = text(ItemKey::TrackNumber).as_deref().and_then(leading_number);
    tags.disc_no = text(ItemKey::DiscNumber).as_deref().and_then(leading_number);
    tags.year =
        text(ItemKey::Year).or_else(|| text(ItemKey::RecordingDate)).as_deref().and_then(year_of);
    Some(tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_years_in_the_shapes_tags_use() {
        assert_eq!(leading_number("3"), Some(3));
        assert_eq!(leading_number("03/12"), Some(3));
        assert_eq!(leading_number(" 7 "), Some(7));
        assert_eq!(leading_number("A1"), None);
        assert_eq!(leading_number("0"), None);
        assert_eq!(year_of("1961"), Some(1961));
        assert_eq!(year_of("1961-06-25"), Some(1961));
        assert_eq!(year_of("61"), None);
        assert_eq!(year_of(""), None);
    }

    #[test]
    fn unreadable_files_have_no_tags() {
        let path = std::env::temp_dir().join(format!("vihpluh-notags-{}.flac", std::process::id()));
        std::fs::write(&path, b"not a flac file").unwrap();
        assert_eq!(read(&path), None);
        let _ = std::fs::remove_file(&path);
        assert_eq!(read(Path::new("Z:/does/not/exist.mp3")), None);
    }
}
