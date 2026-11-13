//! Cue timings and text from external subtitle files (ARCHITECTURE §11.3): SubRip, ASS/SSA,
//! and WebVTT. The subtitle timeline only needs when each line shows and what it says, so
//! styling is dropped. Parsers never fail: lines they cannot read are skipped.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One subtitle line. Times are seconds in the file's own clock (before `sub-delay`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    /// `None` for bitmap subtitles, which have timings only.
    pub text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Srt,
    Ass,
    Vtt,
}

pub fn format_of(path: &Path) -> Option<Format> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "srt" => Some(Format::Srt),
        "ass" | "ssa" => Some(Format::Ass),
        "vtt" => Some(Format::Vtt),
        _ => None,
    }
}

/// Reads and parses a subtitle file. `None` when the format is not a text format we parse.
pub fn parse_file(path: &Path) -> std::io::Result<Option<Vec<Cue>>> {
    let Some(format) = format_of(path) else { return Ok(None) };
    let bytes = std::fs::read(path)?;
    Ok(Some(parse(&decode(&bytes), format)))
}

pub fn parse(text: &str, format: Format) -> Vec<Cue> {
    let mut cues = match format {
        Format::Srt => parse_srt(text),
        Format::Ass => parse_ass(text),
        Format::Vtt => parse_vtt(text),
    };
    finish(&mut cues);
    cues
}

/// Drops unusable cues and sorts by start time (overlapping cues are all kept).
pub fn finish(cues: &mut Vec<Cue>) {
    cues.retain(|c| c.start.is_finite() && c.end.is_finite() && c.end > c.start && c.start >= 0.0);
    cues.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.end.total_cmp(&b.end)));
}

/// Bytes to text: UTF-8 (with or without a BOM), UTF-16 with a BOM, else Latin-1 so that
/// timings still parse from files in a legacy encoding.
pub fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    let utf16 = |rest: &[u8], big: bool| {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|p| {
                if big {
                    u16::from_be_bytes([p[0], p[1]])
                } else {
                    u16::from_le_bytes([p[0], p[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, false);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, true);
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

/// `hh:mm:ss,mmm`, `mm:ss.mmm`, `h:mm:ss.cc` — any of the clock forms the three formats use.
pub fn parse_clock(text: &str) -> Option<f64> {
    let text = text.trim();
    let (clock, fraction) = match text.rsplit_once([',', '.']) {
        Some((clock, fraction)) => (clock, fraction),
        None => (text, ""),
    };
    if !fraction.chars().all(|c| c.is_ascii_digit()) || fraction.len() > 9 {
        return None;
    }
    let mut seconds = 0.0;
    let mut parts = 0;
    for part in clock.split(':') {
        if part.is_empty() || part.len() > 6 || !part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
        parts += 1;
    }
    if !(1..=3).contains(&parts) {
        return None;
    }
    if !fraction.is_empty() {
        seconds += fraction.parse::<f64>().ok()? / 10f64.powi(fraction.len() as i32);
    }
    Some(seconds)
}

/// `start --> end [settings]`.
fn parse_timing(line: &str) -> Option<(f64, f64)> {
    let (start, rest) = line.split_once("-->")?;
    let end = rest.split_whitespace().next()?;
    Some((parse_clock(start)?, parse_clock(end)?))
}

/// Removes `<i>`-style tags and `{\an8}`-style override blocks.
pub fn strip_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut angle = 0u32;
    let mut brace = 0u32;
    for c in text.chars() {
        match c {
            '<' => angle += 1,
            '>' if angle > 0 => angle -= 1,
            '{' => brace += 1,
            '}' if brace > 0 => brace -= 1,
            _ if angle == 0 && brace == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn clean_text(lines: &[&str]) -> Option<String> {
    let text = lines
        .iter()
        .map(|l| strip_markup(l).trim().to_string())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

/// SubRip: blocks of `index`, `timing`, text lines. The index line is optional in the wild,
/// so a block is anchored on its timing line.
fn parse_srt(text: &str) -> Vec<Cue> {
    parse_blocks(text)
}

/// WebVTT: like SubRip with a header, optional cue identifiers, and NOTE/STYLE/REGION blocks.
fn parse_vtt(text: &str) -> Vec<Cue> {
    parse_blocks(text)
}

fn parse_blocks(text: &str) -> Vec<Cue> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
    let mut cues = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((start, end)) = parse_timing(lines[i]) else {
            i += 1;
            continue;
        };
        i += 1;
        let from = i;
        // Text runs to the next blank line, or to the next timing line when the blank line
        // between two blocks is missing (its index line, if any, goes with that next block).
        while i < lines.len() && !lines[i].trim().is_empty() && parse_timing(lines[i]).is_none() {
            i += 1;
        }
        let mut body = &lines[from..i];
        let next_is_timing = i < lines.len() && parse_timing(lines[i]).is_some();
        if next_is_timing && body.last().is_some_and(|l| l.trim().parse::<u32>().is_ok()) {
            body = &body[..body.len() - 1];
        }
        cues.push(Cue { start, end, text: clean_text(body) });
    }
    cues
}

/// ASS/SSA: `Dialogue:` lines in `[Events]`, with the column order given by `Format:`.
fn parse_ass(text: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    let mut in_events = false;
    // The defaults of ASS v4+; SSA v4 has `Marked` first instead of `Layer`.
    let mut columns: Vec<String> = [
        "layer", "start", "end", "style", "name", "marginl", "marginr", "marginv", "effect", "text",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_events = line.eq_ignore_ascii_case("[events]");
            continue;
        }
        if !in_events {
            continue;
        }
        if let Some(format) = strip_prefix_ci(line, "format:") {
            columns = format.split(',').map(|c| c.trim().to_ascii_lowercase()).collect();
            continue;
        }
        let Some(rest) = strip_prefix_ci(line, "dialogue:") else { continue };
        let index = |name: &str| columns.iter().position(|c| c == name);
        let (Some(start_i), Some(end_i), Some(text_i)) =
            (index("start"), index("end"), index("text"))
        else {
            continue;
        };
        // The text column is last and may itself contain commas.
        let fields: Vec<&str> = rest.splitn(columns.len(), ',').collect();
        let (Some(start), Some(end)) = (
            fields.get(start_i).and_then(|f| parse_clock(f)),
            fields.get(end_i).and_then(|f| parse_clock(f)),
        ) else {
            continue;
        };
        let text = fields.get(text_i).and_then(|t| ass_text(t));
        cues.push(Cue { start, end, text });
    }
    cues
}

fn strip_prefix_ci<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let head = line.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &line[prefix.len()..])
}

/// The readable text of an ASS event: override blocks removed, `\N`/`\n` as line breaks,
/// `\h` as a space. Drawing commands (`{\p1}…`) have no text.
pub fn ass_text(raw: &str) -> Option<String> {
    if raw.contains("\\p1") || raw.contains("\\p2") {
        return None;
    }
    let plain = strip_markup(raw).replace("\\N", "\n").replace("\\n", "\n").replace("\\h", " ");
    let lines: Vec<&str> = plain.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start: f64, end: f64, text: &str) -> Cue {
        Cue { start, end, text: Some(text.to_string()) }
    }

    #[test]
    fn clocks() {
        assert_eq!(parse_clock("00:00:01,000"), Some(1.0));
        assert_eq!(parse_clock("01:02:03.500"), Some(3723.5));
        assert_eq!(parse_clock("02:03.25"), Some(123.25));
        assert_eq!(parse_clock("0:00:05.50"), Some(5.5));
        assert_eq!(parse_clock(" 7 "), Some(7.0));
        assert_eq!(parse_clock("1:2:3:4"), None);
        assert_eq!(parse_clock("aa:bb"), None);
        assert_eq!(parse_clock(""), None);
        assert_eq!(parse_clock("-1:00"), None);
    }

    #[test]
    fn srt_basic_with_markup_and_multiple_lines() {
        let text = "1\n00:00:01,000 --> 00:00:03,500\n<i>I told you</i>\nso.\n\n2\n00:00:05,000 --> 00:00:06,000 X1:10\n{\\an8}Run!\n";
        assert_eq!(
            parse(text, Format::Srt),
            vec![cue(1.0, 3.5, "I told you\nso."), cue(5.0, 6.0, "Run!")]
        );
    }

    #[test]
    fn srt_with_bom_crlf_and_missing_blank_lines() {
        let text = "\u{feff}1\r\n00:00:01,000 --> 00:00:02,000\r\nOne\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\nTwo\r\n\r\n\r\n00:00:05,000 --> 00:00:06,000\r\nNo index\r\n";
        let decoded = decode(text.as_bytes());
        // The BOM survives `decode` only when it is not at the start as bytes; here it is.
        let cues = parse(&decoded, Format::Srt);
        assert_eq!(
            cues,
            vec![cue(1.0, 2.0, "One"), cue(3.0, 4.0, "Two"), cue(5.0, 6.0, "No index")]
        );
    }

    #[test]
    fn overlapping_and_unordered_cues_are_kept_and_sorted() {
        let text = "1\n00:00:10,000 --> 00:00:14,000\nLate\n\n2\n00:00:01,000 --> 00:00:12,000\nLong\n\n3\n00:00:02,000 --> 00:00:03,000\nInside\n";
        let cues = parse(text, Format::Srt);
        assert_eq!(cues.iter().map(|c| c.start).collect::<Vec<_>>(), vec![1.0, 2.0, 10.0]);
    }

    #[test]
    fn malformed_srt_is_skipped_not_fatal() {
        let text = "garbage\n00:00:xx,000 --> 00:00:02,000\nBad time\n\n5\n00:00:04,000 --> 00:00:03,000\nBackwards\n\n6\n00:00:05,000 --> 00:00:06,000\n\n7\n00:00:07,000 -->\nNo end\n\n8\n00:00:08,000 --> 00:00:09,000\nGood\n";
        let cues = parse(text, Format::Srt);
        assert_eq!(cues, vec![Cue { start: 5.0, end: 6.0, text: None }, cue(8.0, 9.0, "Good")]);
    }

    #[test]
    fn vtt_header_notes_identifiers_and_settings() {
        let text = "WEBVTT - title\n\nNOTE a comment\nover two lines\n\nSTYLE\n::cue { color: red }\n\nintro\n00:01.000 --> 00:02.500 line:90% align:center\n<v Ann>Hello</v>\n\n01:00:00.000 --> 01:00:01.000\nAn hour in\n";
        assert_eq!(
            parse(text, Format::Vtt),
            vec![cue(1.0, 2.5, "Hello"), cue(3600.0, 3601.0, "An hour in")]
        );
    }

    #[test]
    fn ass_dialogue_with_commas_overrides_and_breaks() {
        let text = "[Script Info]\nTitle: x\n\n[V4+ Styles]\nFormat: Name, Fontname\nStyle: Default,Arial\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nComment: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,not shown\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,{\\i1}Well, well{\\i0}\\Nhello\\hthere\nDialogue: 0,0:00:04.00,0:00:05.00,Signs,,0,0,0,,{\\p1}m 0 0 l 10 10{\\p0}\n";
        assert_eq!(
            parse(text, Format::Ass),
            vec![
                cue(1.0, 3.5, "Well, well\nhello there"),
                Cue { start: 4.0, end: 5.0, text: None }
            ]
        );
    }

    #[test]
    fn ssa_column_order_follows_the_format_line() {
        let text = "[Events]\nFormat: Marked, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: Marked=0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Old style\nDialogue: broken line\n[Fonts]\nDialogue: 0,0:00:09.00,0:00:10.00,Default,,0,0,0,,outside events\n";
        assert_eq!(parse(text, Format::Ass), vec![cue(1.0, 2.0, "Old style")]);
    }

    #[test]
    fn encodings() {
        assert_eq!(decode(b"\xEF\xBB\xBFabc"), "abc");
        assert_eq!(decode(&[0xFF, 0xFE, b'h', 0, b'i', 0]), "hi");
        assert_eq!(decode(&[0xFE, 0xFF, 0, b'h', 0, b'i']), "hi");
        assert_eq!(decode(&[b'c', b'a', b'f', 0xE9]), "café", "Latin-1 fallback");
    }

    #[test]
    fn formats_by_extension() {
        assert_eq!(format_of(Path::new("a.SRT")), Some(Format::Srt));
        assert_eq!(format_of(Path::new("a.ssa")), Some(Format::Ass));
        assert_eq!(format_of(Path::new("a.vtt")), Some(Format::Vtt));
        assert_eq!(format_of(Path::new("a.sup")), None);
    }

    /// Mangled input of every kind must come back as "some cues", never a panic.
    #[test]
    fn hostile_input_does_not_panic() {
        let samples = [
            "",
            "\n\n\n",
            "-->",
            "--> -->",
            "00:00:01,000 --> 00:00:02,000",
            "99999999:99:99,999 --> 1",
            "1\n00:00:01,000 --> 00:00:02,000\n<<<<{{{{\n",
            "[Events]\nFormat:\nDialogue:\nDialogue: ,,,,,,,,,\n",
            "[Events]\nFormat: Text\nDialogue: only text\n",
            "[Events]\nFormat: Start, End, Text\nDialogue: 0:00:01.00\n",
            "WEBVTT\n\n00:00.000 --> 00:00.000\n",
            "\u{0}\u{1}\u{2} --> \u{3}",
            "é --> ü\n日本語\n",
        ];
        for sample in samples {
            for format in [Format::Srt, Format::Ass, Format::Vtt] {
                let _ = parse(sample, format);
            }
            let _ = decode(sample.as_bytes());
        }
        // Truncations of a valid file at every byte.
        let valid = "1\n00:00:01,000 --> 00:00:03,500\n<i>I told you</i>\n\n[Events]\nDialogue: 0,0:00:01.00,0:00:03.50,D,,0,0,0,,{\\i1}x\n";
        for cut in 0..valid.len() {
            if valid.is_char_boundary(cut) {
                for format in [Format::Srt, Format::Ass, Format::Vtt] {
                    let _ = parse(&valid[..cut], format);
                }
            }
        }
    }
}
