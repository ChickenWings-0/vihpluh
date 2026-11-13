//! Subtitle auto-matching (ARCHITECTURE §10.5): pairs external subtitle files with videos
//! even when their names differ. Scoring and assignment are pure functions over path lists;
//! only `candidates` touches the disk.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use crate::presets::series::{self, SeriesKey};

pub const SUB_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "vtt", "sub", "idx", "sup"];
/// Child folders that hold subtitles (compared case-insensitively).
const SUB_DIRS: &[&str] = &["subs", "subtitles", "sub"];
/// Pairs scoring below this are discarded.
const MIN_SCORE: u32 = 50;
/// Two subtitles of one language this close together are reported, not guessed.
const AMBIGUITY_MARGIN: u32 = 5;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubMatchReport {
    pub matched: Vec<SubMatch>,
    pub unmatched_videos: Vec<String>,
    pub unmatched_subs: Vec<String>,
    pub ambiguous: Vec<Ambiguity>,
    /// The matches were saved and will load with their videos.
    pub applied: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubMatch {
    pub video: String,
    pub subs: Vec<MatchedSub>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MatchedSub {
    pub path: String,
    pub lang: Option<String>,
    pub score: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Ambiguity {
    pub video: String,
    pub lang: Option<String>,
    pub subs: Vec<String>,
}

pub fn is_subtitle_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SUB_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Subtitle files that could belong to videos in `dir`: the folder itself, its `Subs`-like
/// child folders, and one level of folders inside those (`Subs/<video stem>/`).
pub fn candidates(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let list = |dir: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default()
    };
    for path in list(dir) {
        if path.is_file() {
            if is_subtitle_file(&path) {
                found.push(path);
            }
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_ascii_lowercase();
        if !SUB_DIRS.contains(&name.as_str()) {
            continue;
        }
        for inner in list(&path) {
            if inner.is_file() {
                if is_subtitle_file(&inner) {
                    found.push(inner);
                }
            } else {
                found.extend(
                    list(&inner).into_iter().filter(|p| p.is_file() && is_subtitle_file(p)),
                );
            }
        }
    }
    found.sort();
    found
}

/// Words on the end of a subtitle name that describe the track, not the episode.
const TRACK_TAGS: &[&str] = &["forced", "sdh", "cc", "hi", "default", "full", "signs", "songs"];

/// Language codes and names recognised as a suffix, with their ISO 639-2 form.
const LANGUAGES: &[(&str, &str)] = &[
    ("en", "eng"),
    ("eng", "eng"),
    ("english", "eng"),
    ("es", "spa"),
    ("spa", "spa"),
    ("esp", "spa"),
    ("spanish", "spa"),
    ("espanol", "spa"),
    ("fr", "fre"),
    ("fre", "fre"),
    ("fra", "fre"),
    ("french", "fre"),
    ("de", "ger"),
    ("ger", "ger"),
    ("deu", "ger"),
    ("german", "ger"),
    ("it", "ita"),
    ("ita", "ita"),
    ("italian", "ita"),
    ("pt", "por"),
    ("por", "por"),
    ("portuguese", "por"),
    ("ja", "jpn"),
    ("jp", "jpn"),
    ("jpn", "jpn"),
    ("japanese", "jpn"),
    ("ko", "kor"),
    ("kor", "kor"),
    ("korean", "kor"),
    ("zh", "chi"),
    ("chi", "chi"),
    ("zho", "chi"),
    ("chs", "chi"),
    ("cht", "chi"),
    ("chinese", "chi"),
    ("ru", "rus"),
    ("rus", "rus"),
    ("russian", "rus"),
    ("ar", "ara"),
    ("ara", "ara"),
    ("arabic", "ara"),
    ("hi", "hin"),
    ("hin", "hin"),
    ("hindi", "hin"),
    ("nl", "dut"),
    ("dut", "dut"),
    ("nld", "dut"),
    ("dutch", "dut"),
    ("pl", "pol"),
    ("pol", "pol"),
    ("polish", "pol"),
    ("tr", "tur"),
    ("tur", "tur"),
    ("turkish", "tur"),
    ("sv", "swe"),
    ("swe", "swe"),
    ("swedish", "swe"),
    ("no", "nor"),
    ("nor", "nor"),
    ("norwegian", "nor"),
    ("da", "dan"),
    ("dan", "dan"),
    ("danish", "dan"),
    ("fi", "fin"),
    ("fin", "fin"),
    ("finnish", "fin"),
    ("cs", "cze"),
    ("cze", "cze"),
    ("ces", "cze"),
    ("czech", "cze"),
    ("el", "gre"),
    ("gre", "gre"),
    ("ell", "gre"),
    ("greek", "gre"),
    ("he", "heb"),
    ("heb", "heb"),
    ("hebrew", "heb"),
    ("hu", "hun"),
    ("hun", "hun"),
    ("hungarian", "hun"),
    ("id", "ind"),
    ("ind", "ind"),
    ("indonesian", "ind"),
    ("th", "tha"),
    ("tha", "tha"),
    ("thai", "tha"),
    ("uk", "ukr"),
    ("ukr", "ukr"),
    ("ukrainian", "ukr"),
    ("vi", "vie"),
    ("vie", "vie"),
    ("vietnamese", "vie"),
    ("ro", "rum"),
    ("rum", "rum"),
    ("ron", "rum"),
    ("romanian", "rum"),
];

fn language_of(token: &str) -> Option<&'static str> {
    let token = token.to_ascii_lowercase();
    // `pt-BR`, `zh_Hans`: the part before the region decides.
    let base = token.split(['-', '_']).next().unwrap_or("");
    LANGUAGES.iter().find(|(name, _)| *name == token || *name == base).map(|(_, code)| *code)
}

/// A subtitle file name split into the part that names the episode and its language.
#[derive(Debug, Clone, PartialEq)]
struct SubName {
    /// The stem with language and track tags removed.
    base: String,
    lang: Option<&'static str>,
}

/// Peels `.en`, `.eng.forced`, `_English`, `.pt-BR` off the end of a subtitle stem.
fn split_language(stem: &str) -> SubName {
    let mut base = stem;
    // Trailing tokens that are track tags or languages, nearest the name last.
    let mut tail: Vec<&str> = Vec::new();
    while let Some(at) = base.rfind(['.', '_', ' ']) {
        let token = &base[at + 1..];
        let is_tag = TRACK_TAGS.contains(&token.to_ascii_lowercase().as_str());
        if !is_tag && language_of(token).is_none() {
            break;
        }
        tail.push(token);
        base = &base[..at];
    }
    // "hi" is both Hindi and "hearing impaired": next to another language it is the tag.
    let is_hi = |t: &&str| t.eq_ignore_ascii_case("hi");
    let lang = tail
        .iter()
        .rev()
        .filter(|t| !is_hi(t))
        .find_map(|t| language_of(t))
        .or_else(|| tail.iter().find(|t| is_hi(t)).and_then(|t| language_of(t)));
    SubName { base: base.trim_end_matches(['.', '_', ' ', '-']).to_string(), lang }
}

fn stem_of(path: &str) -> &str {
    Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("")
}

fn parent_name(path: &str) -> String {
    Path::new(path)
        .parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}

fn tokens(name: &str) -> BTreeSet<String> {
    series::normalize(name)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Jaccard similarity of the two names' word sets, 0–1.
fn similarity(a: &str, b: &str) -> f64 {
    let (a, b) = (tokens(a), tokens(b));
    let union = a.union(&b).count();
    if union == 0 { 0.0 } else { a.intersection(&b).count() as f64 / union as f64 }
}

fn keys_related(a: &SeriesKey, b: &SeriesKey) -> bool {
    !a.key.is_empty() && !b.key.is_empty() && (a.key.contains(&b.key) || b.key.contains(&a.key))
}

/// How well a subtitle file fits a video, 0–100 (§10.5 step 3).
fn score(video: &str, sub: &str) -> u32 {
    let video_stem = stem_of(video);
    let name = split_language(stem_of(sub));
    let (v_norm, s_norm) = (series::normalize(video_stem), series::normalize(&name.base));
    if v_norm.is_empty() {
        return 0;
    }
    // Exact stem prefix: `Show S01E1.en.srt` for `Show S01E1.mkv` — but not `Show S01E10`.
    let prefix = s_norm
        .strip_prefix(&v_norm)
        .is_some_and(|rest| rest.chars().next().is_none_or(|c| !c.is_alphanumeric()));
    // `Subs/<video stem>/2_English.srt`.
    let own_folder = series::normalize(&parent_name(sub)) == v_norm;
    if prefix || own_folder {
        return 100;
    }
    let v = series::parse_name(video_stem, &parent_name(video));
    let s = series::parse_name(&name.base, "");
    if let (Some(ve), Some(se)) = (v.episode, s.episode) {
        let seasons_agree = match (v.season, s.season) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        };
        // Two different episodes never pair, however alike the names are.
        return match (ve == se && seasons_agree, keys_related(&v, &s)) {
            (true, true) => 90,
            (true, false) => 80,
            (false, _) => 0,
        };
    }
    (similarity(video_stem, &name.base) * 60.0).round() as u32
}

/// Scores every pair and assigns subtitles to videos (§10.5 steps 3–5).
pub fn plan(videos: &[String], subs: &[String]) -> SubMatchReport {
    // Each subtitle belongs to the video(s) it fits best.
    let mut per_video: BTreeMap<usize, Vec<(usize, u32)>> = BTreeMap::new();
    let mut unmatched_subs = Vec::new();
    for (s, sub) in subs.iter().enumerate() {
        let scores: Vec<u32> = videos.iter().map(|video| score(video, sub)).collect();
        let best = scores.iter().copied().max().unwrap_or(0);
        if best < MIN_SCORE {
            unmatched_subs.push(sub.clone());
            continue;
        }
        for (v, &points) in scores.iter().enumerate() {
            if points == best {
                per_video.entry(v).or_default().push((s, points));
            }
        }
    }

    let mut report = SubMatchReport {
        matched: Vec::new(),
        unmatched_videos: Vec::new(),
        unmatched_subs,
        ambiguous: Vec::new(),
        applied: false,
    };
    for (v, video) in videos.iter().enumerate() {
        let mut by_lang: BTreeMap<Option<&'static str>, Vec<(usize, u32)>> = BTreeMap::new();
        for &(s, points) in per_video.get(&v).map(Vec::as_slice).unwrap_or(&[]) {
            by_lang.entry(split_language(stem_of(&subs[s])).lang).or_default().push((s, points));
        }
        let mut chosen = Vec::new();
        let mut undecided = false;
        for (lang, mut group) in by_lang {
            group.sort_by(|a, b| b.1.cmp(&a.1).then(subs[a.0].cmp(&subs[b.0])));
            let top = group[0].1;
            let close: Vec<&(usize, u32)> =
                group.iter().filter(|(_, p)| top - p <= AMBIGUITY_MARGIN).collect();
            if close.len() > 1 {
                undecided = true;
                report.ambiguous.push(Ambiguity {
                    video: video.clone(),
                    lang: lang.map(str::to_string),
                    subs: close.iter().map(|(s, _)| subs[*s].clone()).collect(),
                });
            } else {
                chosen.push(MatchedSub {
                    path: subs[group[0].0].clone(),
                    lang: lang.map(str::to_string),
                    score: top,
                });
            }
        }
        if chosen.is_empty() {
            // A video with only ambiguous candidates is listed there, not as "nothing found".
            if !undecided {
                report.unmatched_videos.push(video.clone());
            }
        } else {
            report.matched.push(SubMatch { video: video.clone(), subs: chosen });
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    fn subs_of<'a>(report: &'a SubMatchReport, video: &str) -> Vec<&'a str> {
        report
            .matched
            .iter()
            .find(|m| m.video == video)
            .map(|m| m.subs.iter().map(|s| s.path.as_str()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn language_suffixes() {
        let name = |stem: &str| {
            let n = split_language(stem);
            (n.base, n.lang)
        };
        assert_eq!(name("Show S01E01.en"), ("Show S01E01".into(), Some("eng")));
        assert_eq!(name("Show S01E01.eng.forced"), ("Show S01E01".into(), Some("eng")));
        assert_eq!(name("Show S01E01.pt-BR"), ("Show S01E01".into(), Some("por")));
        assert_eq!(name("Show S01E01_English"), ("Show S01E01".into(), Some("eng")));
        assert_eq!(name("Show S01E01.en.hi"), ("Show S01E01".into(), Some("eng")));
        assert_eq!(name("Show S01E01.hi"), ("Show S01E01".into(), Some("hin")));
        assert_eq!(name("Show S01E01"), ("Show S01E01".into(), None));
        assert_eq!(name("It"), ("It".into(), None), "a bare title is not a language");
    }

    #[test]
    fn exact_prefix_respects_number_boundaries() {
        assert_eq!(score("D:/s/Show S01E1.mkv", "D:/s/Show S01E1.en.srt"), 100);
        assert_eq!(score("D:/s/Show S01E1.mkv", "D:/s/Show S01E10.en.srt"), 0);
        assert_eq!(score("D:/s/Show S01E1.mkv", "D:/s/Subs/Show S01E1/2_English.srt"), 100);
    }

    #[test]
    fn episode_numbers_decide_when_names_differ() {
        let video = "D:/s/Show S01E3.mkv";
        assert_eq!(score(video, "D:/s/Subs/show.1x03.en.srt"), 90);
        assert_eq!(score(video, "D:/s/Subs/[Grp] Show - 03 [1080p].ass"), 90);
        assert_eq!(score(video, "D:/s/Subs/Completely Different - 03.srt"), 80);
        assert_eq!(score(video, "D:/s/Subs/show.1x04.en.srt"), 0);
        assert_eq!(score(video, "D:/s/Subs/show.2x03.en.srt"), 0, "another season");
    }

    #[test]
    fn films_fall_back_to_word_similarity() {
        let video = "D:/m/Blade Runner 2049 (2017).mkv";
        assert_eq!(score(video, "D:/m/blade.runner.2049.srt"), 100, "prefix after normalising");
        assert!(score(video, "D:/m/Blade Runner 2049 Final Cut.en.srt") >= 50);
        assert!(score(video, "D:/m/Alien.en.srt") < 50);
    }

    /// A T6-like season: every subtitle named differently from its video.
    #[test]
    fn a_season_with_naming_chaos_pairs_completely() {
        let videos: Vec<String> = (1..=10).map(|n| format!("D:/T6/Show S01E{n}.mkv")).collect();
        let subs = strings(&[
            "D:/T6/Subs/show.s01e01.en.srt",
            "D:/T6/Subs/show.s01e01.es.srt",
            "D:/T6/Subs/Show - 02 [Group].eng.ass",
            "D:/T6/Subs/show.1x03.en.srt",
            "D:/T6/Subs/Show.S01E04.1080p.WEB-DL.English.srt",
            "D:/T6/Subs/SHOW_S01E05_en.srt",
            "D:/T6/Subs/Show Episode 06.en.vtt",
            "D:/T6/Subs/show s1 e07.srt",
            "D:/T6/Subs/Show S01E8/2_English.srt",
            "D:/T6/Subs/show.109.en.srt",
            "D:/T6/Subs/Show - 10.en.srt",
            "D:/T6/Subs/readme.txt.srt",
        ]);
        let report = plan(&videos, &subs);
        assert_eq!(report.ambiguous, vec![]);
        assert_eq!(
            subs_of(&report, "D:/T6/Show S01E1.mkv"),
            vec!["D:/T6/Subs/show.s01e01.es.srt", "D:/T6/Subs/show.s01e01.en.srt"]
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
        );
        for (n, sub) in [
            (2, "D:/T6/Subs/Show - 02 [Group].eng.ass"),
            (3, "D:/T6/Subs/show.1x03.en.srt"),
            (4, "D:/T6/Subs/Show.S01E04.1080p.WEB-DL.English.srt"),
            (5, "D:/T6/Subs/SHOW_S01E05_en.srt"),
            (6, "D:/T6/Subs/Show Episode 06.en.vtt"),
            (7, "D:/T6/Subs/show s1 e07.srt"),
            (8, "D:/T6/Subs/Show S01E8/2_English.srt"),
            (10, "D:/T6/Subs/Show - 10.en.srt"),
        ] {
            assert_eq!(subs_of(&report, &format!("D:/T6/Show S01E{n}.mkv")), vec![sub], "E{n}");
        }
        // `show.109` is not an episode marker we read; nothing is guessed for episode 9.
        assert_eq!(report.unmatched_videos, strings(&["D:/T6/Show S01E9.mkv"]));
        assert_eq!(
            report.unmatched_subs,
            strings(&["D:/T6/Subs/show.109.en.srt", "D:/T6/Subs/readme.txt.srt"])
        );
    }

    #[test]
    fn two_equally_good_subtitles_of_one_language_are_reported_not_guessed() {
        let videos = strings(&["D:/s/Show S01E2.mkv"]);
        let subs = strings(&[
            "D:/s/Subs/show.1x02.en.srt",
            "D:/s/Subs/Show - 02.en.ass",
            "D:/s/Subs/show.1x02.es.srt",
        ]);
        let report = plan(&videos, &subs);
        assert_eq!(report.ambiguous.len(), 1);
        assert_eq!(report.ambiguous[0].lang.as_deref(), Some("eng"));
        assert_eq!(report.ambiguous[0].subs.len(), 2);
        assert_eq!(report.unmatched_videos, Vec::<String>::new());
        // The unambiguous language still pairs.
        assert_eq!(subs_of(&report, "D:/s/Show S01E2.mkv"), vec!["D:/s/Subs/show.1x02.es.srt"]);
    }

    #[test]
    fn a_clearly_better_subtitle_wins_without_ambiguity() {
        let videos = strings(&["D:/s/Show S01E2.mkv"]);
        let subs = strings(&["D:/s/Show S01E2.en.srt", "D:/s/Subs/Other - 02.en.srt"]);
        let report = plan(&videos, &subs);
        assert_eq!(report.ambiguous, vec![]);
        assert_eq!(subs_of(&report, "D:/s/Show S01E2.mkv"), vec!["D:/s/Show S01E2.en.srt"]);
    }

    #[test]
    fn empty_and_odd_input() {
        assert_eq!(plan(&[], &[]).matched, vec![]);
        let report = plan(&strings(&["", ".mkv", "a.mkv"]), &strings(&["", ".srt", "..", "a"]));
        assert!(report.ambiguous.len() + report.matched.len() <= 3);
    }

    #[test]
    fn candidates_cover_the_folder_and_subs_folders() {
        let dir = std::env::temp_dir().join(format!("vihpluh-automatch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["", "Subs", "Subs/Show S01E1", "Extras", "subtitles"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        for file in [
            "a.mkv",
            "a.srt",
            "Subs/b.ASS",
            "Subs/Show S01E1/2_English.srt",
            "Subs/notes.txt",
            "Extras/c.srt",
            "subtitles/d.vtt",
        ] {
            std::fs::write(dir.join(file), b"x").unwrap();
        }
        let found: Vec<String> = candidates(&dir)
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(
            found,
            vec!["Subs/Show S01E1/2_English.srt", "Subs/b.ASS", "a.srt", "subtitles/d.vtt"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
