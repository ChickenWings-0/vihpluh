//! Command-line interface (ARCHITECTURE §4.6).

use std::path::{Path, PathBuf};

use clap::Parser;

#[derive(Debug, Clone, Default, Parser)]
#[command(
    name = "vihpluh",
    about = "A fast, keyboard-driven desktop media player",
    disable_version_flag = true
)]
pub struct Cli {
    /// Files, folders, or URLs to open
    #[arg(value_name = "FILES_OR_URLS")]
    pub sources: Vec<String>,

    /// Add to the running instance's queue instead of replacing it
    #[arg(long, conflicts_with = "play_next")]
    pub enqueue: bool,

    /// Insert after the current item
    #[arg(long)]
    pub play_next: bool,

    /// Start position: seconds, mm:ss, hh:mm:ss, or N%
    #[arg(long, value_name = "TIME", value_parser = parse_start)]
    pub start: Option<StartPosition>,

    /// Start in fullscreen
    #[arg(long)]
    pub fullscreen: bool,

    /// Start in picture-in-picture
    #[arg(long)]
    pub pip: bool,

    /// Start in Zen mode
    #[arg(long)]
    pub zen: bool,

    /// Load without starting playback
    #[arg(long)]
    pub paused: bool,

    /// Ignore saved resume position
    #[arg(long)]
    pub no_resume: bool,

    /// Do not restore or save the session queue
    #[arg(long)]
    pub no_session: bool,

    /// Force a preset for the opened items
    #[arg(long, value_name = "ID")]
    pub preset: Option<String>,

    /// Use DIR as the config/data/cache root (portable mode)
    #[arg(long, value_name = "DIR")]
    pub home: Option<PathBuf>,

    /// Log detail: error, warn, info, debug, or trace
    #[arg(long, value_name = "LEVEL", value_parser = ["error", "warn", "info", "debug", "trace"])]
    pub log_level: Option<String>,

    /// Extra raw mpv option (repeatable; applied before init)
    #[arg(long = "mpv-opt", value_name = "K=V", value_parser = parse_mpv_opt, allow_hyphen_values = true)]
    pub mpv_opts: Vec<(String, String)>,

    /// Print version, mpv/ffmpeg versions, video backend
    #[arg(short = 'V', long)]
    pub version: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StartPosition {
    Seconds(f64),
    Percent(f64),
}

impl StartPosition {
    /// The value for mpv's per-file `start` option.
    pub fn to_mpv(self) -> String {
        match self {
            Self::Seconds(s) => format!("{s}"),
            Self::Percent(p) => format!("{p}%"),
        }
    }
}

pub fn parse_start(text: &str) -> Result<StartPosition, String> {
    let text = text.trim();
    if let Some(percent) = text.strip_suffix('%') {
        let value: f64 =
            percent.trim().parse().map_err(|_| format!("invalid percentage: {text}"))?;
        if !(0.0..=100.0).contains(&value) {
            return Err(format!("percentage out of range: {text}"));
        }
        return Ok(StartPosition::Percent(value));
    }
    let parts: Vec<&str> = text.split(':').collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
        return Err(format!("invalid time: {text}"));
    }
    let mut seconds = 0.0;
    for (index, part) in parts.iter().enumerate() {
        let value: f64 = part.parse().map_err(|_| format!("invalid time: {text}"))?;
        // Only the leading field may exceed 59 (e.g. "90:00" or plain seconds).
        if value < 0.0 || (index > 0 && value >= 60.0) {
            return Err(format!("invalid time: {text}"));
        }
        seconds = seconds * 60.0 + value;
    }
    Ok(StartPosition::Seconds(seconds))
}

fn parse_mpv_opt(text: &str) -> Result<(String, String), String> {
    let (key, value) = text.split_once('=').ok_or_else(|| format!("expected K=V, got {text}"))?;
    let key = key.trim().trim_start_matches("--");
    if key.is_empty() {
        return Err(format!("expected K=V, got {text}"));
    }
    Ok((key.to_string(), value.to_string()))
}

pub fn is_url(source: &str) -> bool {
    // A scheme is at least two characters, so Windows drive letters ("C:\…") are not URLs.
    match source.split_once("://") {
        Some((scheme, _)) => {
            scheme.len() >= 2
                && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        }
        None => false,
    }
}

/// Makes local paths absolute against `cwd` so they stay valid when argv is forwarded to an
/// already-running instance. URLs pass through untouched.
pub fn normalize_sources(sources: &[String], cwd: &Path) -> Vec<String> {
    sources
        .iter()
        .map(|source| {
            if is_url(source) {
                return source.clone();
            }
            let path = Path::new(source);
            let joined = if path.is_absolute() { path.to_path_buf() } else { cwd.join(path) };
            std::path::absolute(&joined).unwrap_or(joined).display().to_string()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_start_positions() {
        assert_eq!(parse_start("90"), Ok(StartPosition::Seconds(90.0)));
        assert_eq!(parse_start("1.5"), Ok(StartPosition::Seconds(1.5)));
        assert_eq!(parse_start("12:34"), Ok(StartPosition::Seconds(754.0)));
        assert_eq!(parse_start("1:02:03"), Ok(StartPosition::Seconds(3723.0)));
        assert_eq!(parse_start("90:00"), Ok(StartPosition::Seconds(5400.0)));
        assert_eq!(parse_start("50%"), Ok(StartPosition::Percent(50.0)));
    }

    #[test]
    fn rejects_bad_start_positions() {
        for bad in ["", "abc", "1:99", "1:2:3:4", "150%", "-5", "1::2"] {
            assert!(parse_start(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn start_position_formats_for_mpv() {
        assert_eq!(StartPosition::Seconds(754.0).to_mpv(), "754");
        assert_eq!(StartPosition::Percent(12.5).to_mpv(), "12.5%");
    }

    #[test]
    fn parses_flags_and_sources() {
        let cli = Cli::try_parse_from([
            "vihpluh",
            "--paused",
            "--start",
            "1:00",
            "--mpv-opt",
            "scale=ewa_lanczos",
            "--mpv-opt",
            "--deband=yes",
            "a.mkv",
            "https://example.com/v.m3u8",
        ])
        .unwrap();
        assert!(cli.paused);
        assert_eq!(cli.start, Some(StartPosition::Seconds(60.0)));
        assert_eq!(
            cli.mpv_opts,
            vec![("scale".into(), "ewa_lanczos".into()), ("deband".into(), "yes".into())]
        );
        assert_eq!(cli.sources.len(), 2);
    }

    #[test]
    fn enqueue_and_play_next_are_exclusive() {
        assert!(Cli::try_parse_from(["vihpluh", "--enqueue", "--play-next", "a.mkv"]).is_err());
    }

    #[test]
    fn recognises_urls_but_not_drive_letters() {
        assert!(is_url("https://example.com/a.m3u8"));
        assert!(is_url("rtsp://cam/stream"));
        assert!(!is_url("C:\\Videos\\a.mkv"));
        assert!(!is_url("C://Videos/a.mkv"));
        assert!(!is_url("relative/path.mkv"));
    }

    #[test]
    fn normalizes_relative_paths_and_keeps_urls() {
        let cwd = std::env::temp_dir();
        let out = normalize_sources(&["clip.mkv".into(), "http://host/x".into()], &cwd);
        assert_eq!(out[0], cwd.join("clip.mkv").display().to_string());
        assert_eq!(out[1], "http://host/x");
    }
}
