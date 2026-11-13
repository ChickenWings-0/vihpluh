//! Series keys (ARCHITECTURE §10.2): a deterministic normalisation of a file name into
//! `(series_key, season, episode)`. Used by subtitle auto-matching, presets, and the library.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SeriesKey {
    pub key: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

/// Words that describe a release rather than a title.
const RELEASE_TAGS: &[&str] = &[
    "2160p", "1080p", "1080i", "720p", "576p", "480p", "4k", "uhd", "x264", "x265", "h264", "h265",
    "hevc", "avc", "av1", "bluray", "blu-ray", "bdrip", "brrip", "bdremux", "web-dl", "webdl",
    "webrip", "web", "hdtv", "dvdrip", "hdr", "hdr10", "dv", "10bit", "8bit", "remux", "proper",
    "repack", "aac", "flac", "opus", "dts", "ac3", "eac3",
];

/// Parses a path or bare file name. Without an episode marker the key is the parent folder's
/// name (movies, specials).
pub fn parse(path: &str) -> SeriesKey {
    let path = Path::new(path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let folder = path.parent().and_then(Path::file_name).and_then(|s| s.to_str()).unwrap_or("");
    parse_name(stem, folder)
}

/// `stem` is the file name without its extension; `folder` the parent directory's name.
pub fn parse_name(stem: &str, folder: &str) -> SeriesKey {
    let text = normalize(stem);
    let chars: Vec<char> = text.chars().collect();
    let found = season_episode(&chars)
        .or_else(|| season_x_episode(&chars))
        .or_else(|| episode_word(&chars))
        .or_else(|| dash_number(&chars))
        .or_else(|| trailing_number(&chars));
    match found {
        Some(marker) => SeriesKey {
            key: clean_key(&chars[..marker.at].iter().collect::<String>()),
            season: marker.season,
            episode: Some(marker.episode),
        },
        None => SeriesKey { key: normalize(folder), season: None, episode: None },
    }
}

/// Steps 1–2: bracketed groups removed, `.` and `_` become spaces, lowercase, single spaces.
pub fn normalize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut depth = 0u32;
    for c in name.chars() {
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            '.' | '_' => out.push(' '),
            _ => out.extend(c.to_lowercase()),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct Marker {
    /// Index of the marker's first character: the key is everything before it.
    at: usize,
    season: Option<u32>,
    episode: u32,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

/// A word boundary lies before index `i`.
fn boundary_before(chars: &[char], i: usize) -> bool {
    i == 0 || !is_word(chars[i - 1])
}

/// A word boundary lies at index `i` (the first index after a token).
fn boundary_after(chars: &[char], i: usize) -> bool {
    i >= chars.len() || !is_word(chars[i])
}

/// Reads up to `max` ASCII digits at `i`; returns the value and the index after them.
fn digits(chars: &[char], i: usize, max: usize) -> Option<(u32, usize)> {
    let mut end = i;
    let mut value = 0u32;
    while end < chars.len() && end - i < max && chars[end].is_ascii_digit() {
        value = value * 10 + chars[end].to_digit(10).unwrap_or(0);
        end += 1;
    }
    (end > i).then_some((value, end))
}

/// `s(\d{1,2})\s?e(\d{1,3})` — `S02E03`, `s1 e10`.
fn season_episode(chars: &[char]) -> Option<Marker> {
    (0..chars.len()).find_map(|i| {
        if chars[i] != 's' || !boundary_before(chars, i) {
            return None;
        }
        let (season, mut j) = digits(chars, i + 1, 2)?;
        if chars.get(j) == Some(&' ') {
            j += 1;
        }
        if chars.get(j) != Some(&'e') {
            return None;
        }
        let (episode, _) = digits(chars, j + 1, 3)?;
        Some(Marker { at: i, season: Some(season), episode })
    })
}

/// `\b(\d{1,2})x(\d{2,3})\b` — `3x07`.
fn season_x_episode(chars: &[char]) -> Option<Marker> {
    (0..chars.len()).find_map(|i| {
        if !boundary_before(chars, i) {
            return None;
        }
        let (season, j) = digits(chars, i, 2)?;
        if chars.get(j) != Some(&'x') {
            return None;
        }
        let (episode, end) = digits(chars, j + 1, 3)?;
        (end - (j + 1) >= 2 && boundary_after(chars, end)).then_some(Marker {
            at: i,
            season: Some(season),
            episode,
        })
    })
}

/// `\b(?:ep?|episode)\s?(\d{1,3})\b` — `Episode 12`, `ep12`, `e 3`.
fn episode_word(chars: &[char]) -> Option<Marker> {
    (0..chars.len()).find_map(|i| {
        if !boundary_before(chars, i) {
            return None;
        }
        ["episode", "ep", "e"].iter().find_map(|word| {
            let end = i + word.len();
            if end > chars.len() || !chars[i..end].iter().copied().eq(word.chars()) {
                return None;
            }
            let start = if chars.get(end) == Some(&' ') { end + 1 } else { end };
            let (episode, after) = digits(chars, start, 3)?;
            boundary_after(chars, after).then_some(Marker { at: i, season: None, episode })
        })
    })
}

/// `\s-\s(\d{1,3})(?:v\d)?\b` — `Title - 05`, `Title - 05v2`.
fn dash_number(chars: &[char]) -> Option<Marker> {
    (0..chars.len().saturating_sub(3)).find_map(|i| {
        if chars[i] != ' ' || chars[i + 1] != '-' || chars[i + 2] != ' ' {
            return None;
        }
        let (episode, mut end) = digits(chars, i + 3, 3)?;
        if chars.get(end) == Some(&'v') && chars.get(end + 1).is_some_and(char::is_ascii_digit) {
            end += 2;
        }
        boundary_after(chars, end).then_some(Marker { at: i, season: None, episode })
    })
}

/// `\s(\d{2,3})$` once trailing release tags are gone — `Monster 074`.
fn trailing_number(chars: &[char]) -> Option<Marker> {
    let text: String = chars.iter().collect();
    let mut words: Vec<&str> = text.split(' ').collect();
    while words.last().is_some_and(|w| is_release_tag(w)) {
        words.pop();
    }
    let last = words.last()?;
    if words.len() < 2 || !(2..=3).contains(&last.len()) {
        return None;
    }
    let episode = last.parse::<u32>().ok().filter(|_| last.chars().all(|c| c.is_ascii_digit()))?;
    let at = words[..words.len() - 1].iter().map(|w| w.chars().count() + 1).sum::<usize>() - 1;
    Some(Marker { at, season: None, episode })
}

fn is_release_tag(word: &str) -> bool {
    RELEASE_TAGS.contains(&word)
}

fn is_year(word: &str) -> bool {
    word.len() == 4
        && (word.starts_with("19") || word.starts_with("20"))
        && word.chars().all(|c| c.is_ascii_digit())
}

/// Step 4: trailing separators, a trailing year, and release tags come off the key.
fn clean_key(before: &str) -> String {
    let trimmed = before.trim_end_matches([' ', '-', ':']);
    let mut words: Vec<&str> = trimmed.split_whitespace().collect();
    while words.len() > 1 && words.last().is_some_and(|w| is_release_tag(w)) {
        words.pop();
    }
    if words.len() > 1 && words.last().is_some_and(|w| is_year(w)) {
        words.pop();
    }
    words.join(" ").trim_end_matches([' ', '-', ':']).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(path: &str) -> (String, Option<u32>, Option<u32>) {
        let parsed = parse(path);
        (parsed.key, parsed.season, parsed.episode)
    }

    /// The vectors of ARCHITECTURE §10.2, which must pass exactly.
    #[test]
    fn documented_vectors() {
        assert_eq!(
            key("Severance.S02E03.2160p.ATVP.WEB-DL.DDP5.1.HEVC-GRP.mkv"),
            ("severance".into(), Some(2), Some(3))
        );
        assert_eq!(
            key("[SubsPlease] Sousou no Frieren - 05 (1080p) [ABCD1234].mkv"),
            ("sousou no frieren".into(), None, Some(5))
        );
        assert_eq!(key("The.Wire.3x07.Back.Burners.mkv"), ("the wire".into(), Some(3), Some(7)));
        assert_eq!(key("Monster - 074.mkv"), ("monster".into(), None, Some(74)));
        assert_eq!(key("Dark (2017) S01E01.mkv"), ("dark".into(), Some(1), Some(1)));
        assert_eq!(key("Movies/Blade Runner 2049 (2017).mkv"), ("movies".into(), None, None));
    }

    #[test]
    fn marker_priority_and_variants() {
        assert_eq!(key("Show S01E1.mkv"), ("show".into(), Some(1), Some(1)));
        assert_eq!(key("Show S01E10.mkv"), ("show".into(), Some(1), Some(10)));
        assert_eq!(key("show_s1 e04_720p.mkv"), ("show".into(), Some(1), Some(4)));
        assert_eq!(key("Some Show Episode 12.mkv"), ("some show".into(), None, Some(12)));
        assert_eq!(key("Some Show Ep07.mkv"), ("some show".into(), None, Some(7)));
        assert_eq!(key("[Grp] Title - 05v2 [720p].mkv"), ("title".into(), None, Some(5)));
        assert_eq!(key("Monster 074.mkv"), ("monster".into(), None, Some(74)));
        assert_eq!(key("Monster 074 1080p x265.mkv"), ("monster".into(), None, Some(74)));
        // The season/episode form wins even when a dash-number appears earlier.
        assert_eq!(key("Show - 2 - S03E04.mkv"), ("show - 2".into(), Some(3), Some(4)));
    }

    #[test]
    fn things_that_are_not_markers() {
        // A resolution is not "season x episode"; a year is not an episode.
        assert_eq!(key("Clips/holiday 1920x1080.mp4"), ("clips".into(), None, None));
        assert_eq!(key("Films/Alien 1979.mkv"), ("films".into(), None, None));
        // Letters before `s` or `e` keep ordinary words from matching.
        assert_eq!(key("Docs/files2e3.mkv"), ("docs".into(), None, None));
    }

    #[test]
    fn key_cleanup() {
        assert_eq!(key("Dark.2017.S01E02.mkv").0, "dark");
        assert_eq!(key("Show: - S01E02.mkv").0, "show");
        assert_eq!(key("Show.1080p.BluRay.S01E02.mkv").0, "show");
        assert_eq!(normalize("  [x] A_B.c  (d) "), "a b c");
    }

    #[test]
    fn odd_input_does_not_panic() {
        for name in
            ["", ".", "-", " - ", "s", "e", "s1e", "((((", "]]]]", "é - 05", "日本語 第05話"]
        {
            let _ = parse(name);
            let _ = parse_name(name, name);
        }
    }
}
