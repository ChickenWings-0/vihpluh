//! Track rules (ARCHITECTURE §10.3). Presets store rules, not track numbers, because track
//! order differs between episodes: a rule is resolved against each file's real track list.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ipc::dto::{Track, TrackKind};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct TrackRule {
    /// Language codes, any of which matches (ISO 639-1/2, case-insensitive).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>")]
    pub lang: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_contains: Option<String>,
    /// Audio: a channel layout such as `5.1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forced: Option<bool>,
    /// Subtitles: rank external files above embedded tracks within the same tier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer_external: Option<bool>,
    /// Last resort, for files with untagged tracks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number")]
    pub id: Option<i64>,
}

/// A subtitle rule, or the word `"off"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum SubTrackRule {
    Keyword(String),
    Rule(TrackRule),
}

/// What a subtitle rule asks for on a given file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubChoice {
    Off,
    Track(i64),
    /// The rule matched nothing: mpv's own selection stays.
    Unmatched,
}

fn text(value: &Option<String>) -> Option<String> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_lowercase)
}

impl TrackRule {
    fn lang_matches(&self, track: &Track) -> bool {
        track
            .lang
            .as_deref()
            .is_some_and(|lang| self.lang.iter().any(|code| code.eq_ignore_ascii_case(lang)))
    }

    /// The extra conditions of the first tier; each applies only when the rule sets it.
    fn details_match(&self, track: &Track) -> bool {
        let title_ok = text(&self.title_contains).is_none_or(|needle| {
            track.title.as_deref().is_some_and(|title| title.to_lowercase().contains(&needle))
        });
        let channels_ok = text(&self.channels).is_none_or(|wanted| {
            track.channels.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(&wanted))
        });
        title_ok && channels_ok && self.forced.is_none_or(|forced| track.forced == forced)
    }

    fn has_details(&self) -> bool {
        text(&self.title_contains).is_some()
            || text(&self.channels).is_some()
            || self.forced.is_some()
    }

    /// The first tier that yields a candidate wins; within a tier external files come first
    /// when the rule prefers them, then the lowest id.
    pub fn resolve(&self, tracks: &[Track], kind: TrackKind) -> Option<i64> {
        let of_kind: Vec<&Track> = tracks.iter().filter(|t| t.kind == kind).collect();
        let pick = |candidates: Vec<&&Track>| -> Option<i64> {
            let prefer = self.prefer_external.unwrap_or(false);
            candidates.into_iter().min_by_key(|t| (prefer && !t.external, t.id)).map(|t| t.id)
        };
        let any_lang = self.lang.is_empty();
        // 1. Language and every detail the rule gives (a rule without languages needs a
        //    detail, or it would match every track).
        let full: Vec<&&Track> = of_kind
            .iter()
            .filter(|t| {
                (any_lang || self.lang_matches(t))
                    && self.details_match(t)
                    && (!any_lang || self.has_details())
            })
            .collect();
        if let Some(id) = pick(full) {
            return Some(id);
        }
        // 2. Language only.
        if let Some(id) = pick(of_kind.iter().filter(|t| self.lang_matches(t)).collect()) {
            return Some(id);
        }
        // 3. The remembered id, if such a track exists.
        self.id.filter(|id| of_kind.iter().any(|t| t.id == *id))
    }

    /// A rule that finds `track` again in another file: its language, a title only when the
    /// file has several tracks of that language to tell apart, and the id as a last resort.
    pub fn for_track(track: &Track, all: &[Track]) -> Self {
        let same_lang = all
            .iter()
            .filter(|t| t.kind == track.kind && t.lang.is_some() && t.lang == track.lang)
            .count();
        let needs_title = same_lang > 1 || track.lang.is_none();
        Self {
            lang: track.lang.iter().cloned().collect(),
            title_contains: text(&track.title).filter(|_| needs_title && !track.external),
            channels: None,
            forced: None,
            prefer_external: (track.kind == TrackKind::Sub && track.external).then_some(true),
            id: Some(track.id),
        }
    }
}

impl SubTrackRule {
    pub fn resolve(&self, tracks: &[Track]) -> SubChoice {
        match self {
            Self::Keyword(_) => SubChoice::Off,
            Self::Rule(rule) => {
                rule.resolve(tracks, TrackKind::Sub).map_or(SubChoice::Unmatched, SubChoice::Track)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: i64, kind: TrackKind, lang: Option<&str>, title: Option<&str>) -> Track {
        Track {
            id,
            kind,
            title: title.map(str::to_string),
            lang: lang.map(str::to_string),
            codec: "x".into(),
            default: false,
            forced: false,
            external: false,
            external_path: None,
            width: None,
            height: None,
            fps: None,
            hdr: None,
            image: false,
            channels: None,
            sample_rate: None,
            bitrate: None,
            text_based: None,
        }
    }

    fn rule(lang: &[&str]) -> TrackRule {
        TrackRule { lang: lang.iter().map(|s| (*s).to_string()).collect(), ..TrackRule::default() }
    }

    fn subs() -> Vec<Track> {
        let mut signs = track(2, TrackKind::Sub, Some("eng"), Some("Signs & Songs"));
        signs.forced = true;
        let mut external = track(4, TrackKind::Sub, Some("eng"), None);
        external.external = true;
        vec![
            signs,
            track(3, TrackKind::Sub, Some("eng"), Some("Full Subtitles")),
            external,
            track(5, TrackKind::Sub, Some("spa"), None),
            track(1, TrackKind::Audio, Some("jpn"), None),
        ]
    }

    /// The resolution table of §10.3, tier by tier.
    #[test]
    fn tiers() {
        let tracks = subs();
        let sub = |r: &TrackRule| r.resolve(&tracks, TrackKind::Sub);
        // 1. language + title
        let full = TrackRule { title_contains: Some("FULL".into()), ..rule(&["eng", "en"]) };
        assert_eq!(sub(&full), Some(3));
        // 1. language + forced
        let forced = TrackRule { forced: Some(true), ..rule(&["eng"]) };
        assert_eq!(sub(&forced), Some(2));
        let not_forced = TrackRule { forced: Some(false), ..rule(&["eng"]) };
        assert_eq!(sub(&not_forced), Some(3), "ties go to the lowest id");
        // 2. the title is not there: language only
        let missing = TrackRule { title_contains: Some("commentary".into()), ..rule(&["ENG"]) };
        assert_eq!(sub(&missing), Some(2));
        // 3. no such language: the id, when a track has it
        let by_id = TrackRule { id: Some(5), ..rule(&["fre"]) };
        assert_eq!(sub(&by_id), Some(5));
        let wrong_kind = TrackRule { id: Some(1), ..rule(&["fre"]) };
        assert_eq!(sub(&wrong_kind), None, "id 1 is an audio track");
        // 4. nothing
        assert_eq!(sub(&rule(&["fre"])), None);
        assert_eq!(sub(&TrackRule::default()), None, "an empty rule matches nothing");
    }

    #[test]
    fn external_files_rank_first_when_preferred() {
        let tracks = subs();
        let prefer = TrackRule { prefer_external: Some(true), ..rule(&["eng"]) };
        assert_eq!(prefer.resolve(&tracks, TrackKind::Sub), Some(4));
        // …but only within a tier: a title match on an embedded track is a better tier.
        let titled = TrackRule { title_contains: Some("full".into()), ..prefer };
        assert_eq!(titled.resolve(&tracks, TrackKind::Sub), Some(3));
    }

    #[test]
    fn rules_without_a_language_use_their_details() {
        let mut tracks = vec![
            track(1, TrackKind::Audio, None, Some("Stereo")),
            track(2, TrackKind::Audio, None, Some("Surround")),
        ];
        tracks[1].channels = Some("5.1".into());
        let surround = TrackRule { channels: Some("5.1".into()), ..TrackRule::default() };
        assert_eq!(surround.resolve(&tracks, TrackKind::Audio), Some(2));
        let titled = TrackRule { title_contains: Some("stereo".into()), ..TrackRule::default() };
        assert_eq!(titled.resolve(&tracks, TrackKind::Audio), Some(1));
    }

    #[test]
    fn subtitle_rules_can_say_off() {
        let tracks = subs();
        assert_eq!(SubTrackRule::Keyword("off".into()).resolve(&tracks), SubChoice::Off);
        assert_eq!(SubTrackRule::Rule(rule(&["spa"])).resolve(&tracks), SubChoice::Track(5));
        assert_eq!(SubTrackRule::Rule(rule(&["kor"])).resolve(&tracks), SubChoice::Unmatched);
    }

    #[test]
    fn a_captured_rule_finds_its_track_in_a_reordered_file() {
        let tracks = subs();
        let chosen = &tracks[1]; // eng "Full Subtitles", id 3
        let captured = TrackRule::for_track(chosen, &tracks);
        assert_eq!(captured.lang, vec!["eng"]);
        assert_eq!(captured.title_contains.as_deref(), Some("full subtitles"));
        // Another episode: same tracks, different order and ids.
        let reordered = vec![
            track(1, TrackKind::Sub, Some("spa"), None),
            track(2, TrackKind::Sub, Some("eng"), Some("Full Subtitles")),
            track(3, TrackKind::Sub, Some("eng"), Some("Signs & Songs")),
        ];
        assert_eq!(captured.resolve(&reordered, TrackKind::Sub), Some(2));
        // A lone track of its language needs no title.
        let spanish = TrackRule::for_track(&tracks[3], &tracks);
        assert_eq!(spanish.title_contains, None);
        let external = TrackRule::for_track(&tracks[2], &tracks);
        assert_eq!(external.prefer_external, Some(true));
    }

    #[test]
    fn rules_read_from_toml_as_tables_or_off() {
        #[derive(Deserialize)]
        struct Holder {
            track: SubTrackRule,
        }
        let table: Holder =
            toml::from_str("track = { lang = [\"eng\"], title_contains = \"full\", id = 2 }")
                .unwrap();
        assert!(matches!(table.track, SubTrackRule::Rule(ref r) if r.id == Some(2)));
        let off: Holder = toml::from_str("track = \"off\"").unwrap();
        assert_eq!(off.track, SubTrackRule::Keyword("off".into()));
        assert!(toml::from_str::<Holder>("track = { langs = [\"eng\"] }").is_err());
    }
}
