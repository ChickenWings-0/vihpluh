//! Conversion of mpv's `track-list`, `chapter-list`, and `audio-device-list` nodes into DTOs.

use crate::ipc::dto::{AudioDevice, Chapter, Track, TrackKind};
use crate::player::mpv::Node;

/// Subtitle codecs that carry pictures rather than text.
const BITMAP_SUB_CODECS: &[&str] =
    &["hdmv_pgs_subtitle", "pgssub", "dvd_subtitle", "dvdsub", "dvb_subtitle", "dvbsub", "xsub"];

pub fn parse_tracks(list: &Node) -> Vec<Track> {
    list.as_array().unwrap_or(&[]).iter().filter_map(parse_track).collect()
}

fn parse_track(node: &Node) -> Option<Track> {
    let kind = match node.str_field("type")? {
        "video" => TrackKind::Video,
        "audio" => TrackKind::Audio,
        "sub" => TrackKind::Sub,
        _ => return None,
    };
    let codec = node.str_field("codec").unwrap_or("").to_string();
    let non_empty = |key: &str| node.str_field(key).filter(|s| !s.is_empty()).map(str::to_string);

    let mut track = Track {
        id: node.i64_field("id")?,
        kind,
        title: non_empty("title"),
        lang: non_empty("lang"),
        default: node.bool_field("default").unwrap_or(false),
        forced: node.bool_field("forced").unwrap_or(false),
        external: node.bool_field("external").unwrap_or(false),
        external_path: non_empty("external-filename"),
        width: None,
        height: None,
        fps: None,
        hdr: None,
        image: false,
        channels: None,
        sample_rate: None,
        bitrate: None,
        text_based: None,
        codec,
    };
    match kind {
        TrackKind::Video => {
            track.width = node.i64_field("demux-w");
            track.height = node.i64_field("demux-h");
            track.fps = node.f64_field("demux-fps");
            track.image = node.bool_field("image").unwrap_or(false)
                || node.bool_field("albumart").unwrap_or(false);
            track.bitrate = node.i64_field("demux-bitrate");
            if node.get("dolby-vision-profile").is_some() {
                track.hdr = Some("dv".to_string());
            }
        }
        TrackKind::Audio => {
            track.channels = node
                .str_field("demux-channels")
                .map(friendly_channels)
                .or_else(|| node.i64_field("demux-channel-count").map(|n| n.to_string()));
            track.sample_rate = node.i64_field("demux-samplerate");
            track.bitrate = node.i64_field("demux-bitrate");
        }
        TrackKind::Sub => {
            track.text_based = Some(!BITMAP_SUB_CODECS.contains(&track.codec.as_str()));
        }
    }
    Some(track)
}

/// mpv reports layouts such as `5.1(side)`; the UI shows `5.1`.
fn friendly_channels(layout: &str) -> String {
    match layout {
        "mono" => "1.0".to_string(),
        "stereo" => "2.0".to_string(),
        other => other.split('(').next().unwrap_or(other).to_string(),
    }
}

/// HDR class of the playing video from `video-params` (`gamma` is the transfer function).
pub fn hdr_class(video_params: &Node) -> Option<String> {
    let gamma = video_params.str_field("gamma")?;
    Some(
        match gamma {
            "pq" => "hdr10",
            "hlg" => "hlg",
            _ => "sdr",
        }
        .to_string(),
    )
}

pub fn parse_chapters(list: &Node) -> Vec<Chapter> {
    list.as_array()
        .unwrap_or(&[])
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let title = node.str_field("title").filter(|t| !t.is_empty()).map(str::to_string);
            Chapter {
                index: index as u32,
                title: title.unwrap_or_else(|| format!("Chapter {}", index + 1)),
                time: node.f64_field("time").unwrap_or(0.0),
            }
        })
        .collect()
}

pub fn parse_audio_devices(list: &Node) -> Vec<AudioDevice> {
    list.as_array()
        .unwrap_or(&[])
        .iter()
        .filter_map(|node| {
            let name = node.str_field("name")?.to_string();
            let description = match node.str_field("description") {
                Some("Autoselect device") | None => "System default".to_string(),
                Some(other) => other.to_string(),
            };
            Some(AudioDevice { name, description })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, Node)]) -> Node {
        Node::Map(entries.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect())
    }

    fn s(text: &str) -> Node {
        Node::String(text.to_string())
    }

    #[test]
    fn parses_video_audio_and_subtitle_tracks() {
        let list = Node::Array(vec![
            map(&[
                ("id", Node::Int(1)),
                ("type", s("video")),
                ("codec", s("hevc")),
                ("demux-w", Node::Int(3840)),
                ("demux-h", Node::Int(2160)),
                ("demux-fps", Node::Double(23.976)),
                ("default", Node::Flag(true)),
            ]),
            map(&[
                ("id", Node::Int(1)),
                ("type", s("audio")),
                ("codec", s("flac")),
                ("lang", s("eng")),
                ("title", s("Surround 5.1")),
                ("demux-channels", s("5.1(side)")),
                ("demux-channel-count", Node::Int(6)),
                ("demux-samplerate", Node::Int(48000)),
            ]),
            map(&[
                ("id", Node::Int(2)),
                ("type", s("sub")),
                ("codec", s("hdmv_pgs_subtitle")),
                ("lang", s("spa")),
                ("forced", Node::Flag(true)),
            ]),
            map(&[
                ("id", Node::Int(3)),
                ("type", s("sub")),
                ("codec", s("subrip")),
                ("external", Node::Flag(true)),
                ("external-filename", s("D:/Subs/ep1.en.srt")),
                ("title", s("")),
            ]),
            map(&[("id", Node::Int(9)), ("type", s("attachment"))]),
        ]);
        let tracks = parse_tracks(&list);
        assert_eq!(tracks.len(), 4, "unknown types are skipped");

        assert_eq!(tracks[0].kind, TrackKind::Video);
        assert_eq!((tracks[0].width, tracks[0].height), (Some(3840), Some(2160)));
        assert_eq!(tracks[0].fps, Some(23.976));
        assert!(tracks[0].default);

        assert_eq!(tracks[1].kind, TrackKind::Audio);
        assert_eq!(tracks[1].channels.as_deref(), Some("5.1"));
        assert_eq!(tracks[1].sample_rate, Some(48000));
        assert_eq!(tracks[1].lang.as_deref(), Some("eng"));
        assert_eq!(tracks[1].title.as_deref(), Some("Surround 5.1"));

        assert_eq!(tracks[2].text_based, Some(false), "PGS is a bitmap format");
        assert!(tracks[2].forced);

        assert_eq!(tracks[3].text_based, Some(true));
        assert!(tracks[3].external);
        assert_eq!(tracks[3].external_path.as_deref(), Some("D:/Subs/ep1.en.srt"));
        assert_eq!(tracks[3].title, None, "empty titles become None");
    }

    #[test]
    fn cover_art_is_marked_as_an_image() {
        let list = Node::Array(vec![map(&[
            ("id", Node::Int(1)),
            ("type", s("video")),
            ("codec", s("mjpeg")),
            ("albumart", Node::Flag(true)),
        ])]);
        assert!(parse_tracks(&list)[0].image);
    }

    #[test]
    fn channel_layouts_are_simplified() {
        assert_eq!(friendly_channels("stereo"), "2.0");
        assert_eq!(friendly_channels("mono"), "1.0");
        assert_eq!(friendly_channels("7.1(wide)"), "7.1");
        assert_eq!(friendly_channels("5.1"), "5.1");
    }

    #[test]
    fn hdr_class_follows_the_transfer_function() {
        assert_eq!(hdr_class(&map(&[("gamma", s("pq"))])).as_deref(), Some("hdr10"));
        assert_eq!(hdr_class(&map(&[("gamma", s("hlg"))])).as_deref(), Some("hlg"));
        assert_eq!(hdr_class(&map(&[("gamma", s("bt.1886"))])).as_deref(), Some("sdr"));
        assert_eq!(hdr_class(&Node::None), None);
    }

    #[test]
    fn chapters_get_fallback_titles() {
        let list = Node::Array(vec![
            map(&[("title", s("Cold Open")), ("time", Node::Double(0.0))]),
            map(&[("title", s("")), ("time", Node::Double(312.5))]),
        ]);
        let chapters = parse_chapters(&list);
        assert_eq!(chapters[0].title, "Cold Open");
        assert_eq!(chapters[1].title, "Chapter 2");
        assert_eq!(chapters[1].time, 312.5);
        assert_eq!(chapters[1].index, 1);
    }

    #[test]
    fn audio_devices_rename_the_auto_entry() {
        let list = Node::Array(vec![
            map(&[("name", s("auto")), ("description", s("Autoselect device"))]),
            map(&[("name", s("wasapi/{1}")), ("description", s("Speakers (Realtek)"))]),
        ]);
        let devices = parse_audio_devices(&list);
        assert_eq!(devices[0].description, "System default");
        assert_eq!(devices[1].name, "wasapi/{1}");
    }
}
