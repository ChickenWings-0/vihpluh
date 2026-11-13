//! Which presets apply to a file, and in what order (ARCHITECTURE §10.4): least specific
//! first, so that later ones override earlier ones key by key.

use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use super::{Layer, MatchType, Preset, series};
use crate::cli::is_url;

/// `preset_matches_for` — a preset that applies to a source, and why.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PresetHit {
    pub id: String,
    pub reason: String,
}

/// Forward slashes, no trailing slash, lowercase on Windows (paths there ignore case).
pub fn normalize_path(path: &str) -> String {
    let mut out = path.replace('\\', "/");
    while out.len() > 1 && out.ends_with('/') && !out.ends_with(":/") {
        out.pop();
    }
    if cfg!(windows) { out.to_lowercase() } else { out }
}

/// Expands a leading `~` to the home directory.
fn expand_home(path: &str) -> String {
    match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            match directories::UserDirs::new() {
                Some(dirs) => format!("{}{rest}", dirs.home_dir().display()),
                None => path.to_string(),
            }
        }
        _ => path.to_string(),
    }
}

fn parent_of(source: &str) -> String {
    normalize_path(&Path::new(source).parent().map(|p| p.display().to_string()).unwrap_or_default())
}

/// Case-insensitive glob over a whole path: `*` stays inside one path segment, `**` crosses
/// separators, `?` is one character that is not a separator.
pub fn glob_matches(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.replace('\\', "/").to_lowercase().chars().collect();
    let text: Vec<char> = text.replace('\\', "/").to_lowercase().chars().collect();
    fn go(p: &[char], t: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some('*') if p.get(1) == Some(&'*') => {
                // `**/` also matches no directory at all.
                let rest = &p[2..];
                let rest_no_slash = rest.strip_prefix(&['/']).unwrap_or(rest);
                (0..=t.len()).any(|i| go(rest, &t[i..]) || go(rest_no_slash, &t[i..]))
            }
            Some('*') => {
                let limit = t.iter().position(|c| *c == '/').unwrap_or(t.len());
                (0..=limit).any(|i| go(&p[1..], &t[i..]))
            }
            Some('?') => t.first().is_some_and(|c| *c != '/') && go(&p[1..], &t[1..]),
            Some(c) => t.first() == Some(c) && go(&p[1..], &t[1..]),
        }
    }
    go(&pattern, &text)
}

/// Why `preset` applies to `source`, or `None` when it does not. URLs match only globs.
pub fn hit(preset: &Preset, source: &str) -> Option<String> {
    let m = &preset.matcher;
    let url = is_url(source);
    match m.kind {
        MatchType::Glob => {
            let pattern = m.glob.as_deref()?;
            glob_matches(pattern, source).then(|| format!("matches the pattern {pattern}"))
        }
        _ if url => None,
        MatchType::Series => {
            let wanted = m.series.as_deref()?.trim().to_lowercase();
            let parsed = series::parse(source);
            (parsed.episode.is_some() && parsed.key == wanted)
                .then(|| format!("an episode of \"{wanted}\""))
        }
        MatchType::Folder => {
            let folder = normalize_path(&expand_home(m.path.as_deref()?));
            (parent_of(source) == folder).then(|| "in this folder".to_string())
        }
        MatchType::FolderRecursive => {
            let folder = normalize_path(&expand_home(m.path.as_deref()?));
            let parent = parent_of(source);
            let prefix = if folder.ends_with('/') { folder.clone() } else { format!("{folder}/") };
            (parent == folder || parent.starts_with(&prefix))
                .then(|| "in this folder or one inside it".to_string())
        }
    }
}

/// The enabled presets that apply to `source`, in the order they are applied: by
/// specificity, then priority (higher later), then id.
pub fn matching<'a>(presets: &'a [Preset], source: &str) -> Vec<(&'a Preset, String)> {
    let mut hits: Vec<(&Preset, String)> = presets
        .iter()
        .filter(|p| p.enabled)
        .filter_map(|p| hit(p, source).map(|reason| (p, reason)))
        .collect();
    hits.sort_by(|(a, _), (b, _)| {
        (a.matcher.kind, a.priority, &a.id).cmp(&(b.matcher.kind, b.priority, &b.id))
    });
    hits
}

/// The layers of `presets`, laid over each other in order.
pub fn layered(presets: &[&Preset]) -> Layer {
    let mut layer = Layer::default();
    for preset in presets {
        layer.overlay(&preset.layer);
    }
    layer
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::{PresetMatch, VideoSection};

    fn preset(id: &str, kind: MatchType, value: &str, priority: i32) -> Preset {
        let some = Some(value.to_string());
        Preset {
            schema: 1,
            id: id.into(),
            name: id.into(),
            enabled: true,
            priority,
            matcher: PresetMatch {
                kind,
                series: (kind == MatchType::Series).then(|| value.to_string()),
                path: matches!(kind, MatchType::Folder | MatchType::FolderRecursive)
                    .then(|| value.to_string()),
                glob: if kind == MatchType::Glob { some } else { None },
            },
            layer: Layer::default(),
        }
    }

    fn ids(presets: &[Preset], source: &str) -> Vec<String> {
        matching(presets, source).into_iter().map(|(p, _)| p.id.clone()).collect()
    }

    /// Every row of the §10.4 table.
    #[test]
    fn match_types() {
        let file = "D:/Anime/Frieren/Season 1/[Grp] Sousou no Frieren - 05 [1080p].mkv";
        let cases: &[(MatchType, &str, bool)] = &[
            (MatchType::Glob, "**/Frieren*/**/*.mkv", true),
            (MatchType::Glob, "**/frieren/**", true),
            (MatchType::Glob, "*.mkv", false),
            (MatchType::Glob, "**/*.mp4", false),
            (MatchType::Series, "sousou no frieren", true),
            (MatchType::Series, "Sousou No Frieren", true),
            (MatchType::Series, "frieren", false),
            (MatchType::FolderRecursive, "D:/Anime", true),
            (MatchType::FolderRecursive, "D:/Anime/Frieren/Season 1/", true),
            (MatchType::FolderRecursive, "D:/Ani", false),
            (MatchType::FolderRecursive, "D:/Anime/Frieren/Season 2", false),
            (MatchType::Folder, "D:\\Anime\\Frieren\\Season 1", true),
            (MatchType::Folder, "D:/Anime/Frieren", false),
        ];
        for (kind, value, expected) in cases {
            let p = preset("p", *kind, value, 0);
            assert_eq!(hit(&p, file).is_some(), *expected, "{kind:?} {value}");
        }
    }

    #[test]
    fn films_have_no_series_and_urls_match_only_globs() {
        let film = "D:/Movies/Blade Runner 2049 (2017).mkv";
        assert!(hit(&preset("p", MatchType::Series, "movies", 0), film).is_none());
        let url = "https://example.com/live/S01E02/stream.m3u8";
        assert!(hit(&preset("p", MatchType::Series, "live", 0), url).is_none());
        assert!(
            hit(&preset("p", MatchType::FolderRecursive, "https://example.com", 0), url).is_none()
        );
        assert!(hit(&preset("p", MatchType::Glob, "https://example.com/**", 0), url).is_some());
    }

    #[test]
    fn order_is_specificity_then_priority_then_id() {
        let file = "D:/Anime/Frieren/Sousou no Frieren - 05.mkv";
        let mut presets = vec![
            preset("folder", MatchType::Folder, "D:/Anime/Frieren", 0),
            preset("tree", MatchType::FolderRecursive, "D:/Anime", 0),
            preset("series-b", MatchType::Series, "sousou no frieren", 0),
            preset("series-a", MatchType::Series, "sousou no frieren", 0),
            preset("series-low", MatchType::Series, "sousou no frieren", -1),
            preset("series-high", MatchType::Series, "sousou no frieren", 5),
            preset("glob", MatchType::Glob, "**/*.mkv", 99),
            preset("other", MatchType::Folder, "D:/Films", 0),
        ];
        assert_eq!(
            ids(&presets, file),
            vec!["glob", "series-low", "series-a", "series-b", "series-high", "tree", "folder"]
        );
        presets[0].enabled = false;
        assert!(!ids(&presets, file).contains(&"folder".to_string()));
    }

    #[test]
    fn a_folder_preset_overrides_a_series_preset_key_by_key() {
        let file = "D:/Anime/Frieren/Sousou no Frieren - 05.mkv";
        let mut series = preset("series", MatchType::Series, "sousou no frieren", 0);
        series.layer.video =
            VideoSection { contrast: Some(4.0), saturation: Some(6.0), ..VideoSection::default() };
        let mut folder = preset("folder", MatchType::Folder, "D:/Anime/Frieren", 0);
        folder.layer.video = VideoSection { contrast: Some(10.0), ..VideoSection::default() };
        let presets = vec![folder, series];
        let hits: Vec<&Preset> = matching(&presets, file).into_iter().map(|(p, _)| p).collect();
        let layer = layered(&hits);
        assert_eq!(layer.video.contrast, Some(10.0), "the folder preset is more specific");
        assert_eq!(layer.video.saturation, Some(6.0), "what it does not set falls through");
    }

    #[test]
    fn globs() {
        assert!(glob_matches("**/Frieren*/**/*.mkv", "D:/Anime/Frieren BD/S1/e01.mkv"));
        assert!(glob_matches("**/Frieren*/**/*.mkv", "D:/Anime/Frieren BD/e01.mkv"));
        assert!(glob_matches("d:/anime/*/e??.MKV", "D:\\Anime\\Show\\e01.mkv"));
        assert!(!glob_matches("d:/anime/*/e??.mkv", "D:/Anime/Show/S1/e01.mkv"));
        assert!(!glob_matches("d:/anime/*.mkv", "D:/Anime/Show/e01.mkv"));
        assert!(glob_matches("**", "anything/at/all"));
        assert!(!glob_matches("", "x"));
    }

    #[test]
    fn paths_are_compared_normalised() {
        assert_eq!(normalize_path("D:\\Anime\\"), normalize_path("D:/Anime"));
        assert_ne!(normalize_path("D:/Anime"), normalize_path("D:/Animation"));
        assert!(expand_home("~/Videos").len() > "~/Videos".len());
        assert_eq!(expand_home("~name"), "~name");
    }
}
