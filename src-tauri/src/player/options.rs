//! mpv initialisation options for the primary handle (ARCHITECTURE §6.2).
//!
//! Applied in this order: fixed → from config → video surface → hwdec chain → `[mpv.options]`
//! → `--mpv-opt`. Later entries override earlier ones.

use crate::config::Config;
use crate::config::schema::{
    AudioChannels, Gapless, IntOrKeyword, ReplayGain, SubAutoLoad, SubBackground, VideoQuality,
    VideoSync,
};

pub type OptionList = Vec<(String, String)>;

fn yes_no(value: bool) -> String {
    if value { "yes" } else { "no" }.to_string()
}

fn push(list: &mut OptionList, name: &str, value: impl Into<String>) {
    list.push((name.to_string(), value.into()));
}

/// Options that make mpv a silent engine: vihpluh draws all UI and owns all input.
pub fn fixed(config: &Config, ytdlp_available: bool) -> OptionList {
    let mut o = OptionList::new();
    push(&mut o, "config", yes_no(config.mpv.load_user_config));
    push(&mut o, "load-scripts", "no");
    push(&mut o, "ytdl", yes_no(ytdlp_available && config.network.use_ytdlp));
    push(&mut o, "osc", "no");
    push(&mut o, "osd-level", "0");
    push(&mut o, "osd-bar", "no");
    push(&mut o, "input-default-bindings", "no");
    push(&mut o, "input-vo-keyboard", "no");
    push(&mut o, "input-cursor", "no");
    push(&mut o, "cursor-autohide", "no");
    // The core decides what happens at end of file (ARCHITECTURE §6.4).
    push(&mut o, "idle", "yes");
    push(&mut o, "keep-open", "yes");
    push(&mut o, "force-window", "yes");
    // Resume is vihpluh's job (§6.5).
    push(&mut o, "save-position-on-quit", "no");
    push(&mut o, "terminal", "no");
    push(&mut o, "msg-level", "all=warn");
    // vihpluh keeps the display awake itself, the same way on every platform
    // (platform/power.rs).
    push(&mut o, "stop-screensaver", "no");
    o
}

/// mpv's built-in scripts (stats overlay, console, auto-profiles, …) are separate from
/// `load-scripts`. None is wanted: vihpluh draws its own UI, and loading them costs startup
/// time. Each switch only exists from the mpv release that introduced its script, so a
/// rejection here is expected on older builds and is not worth a warning.
pub const OPTIONAL_SCRIPT_SWITCHES: &[&str] = &[
    "load-stats-overlay",
    "load-console",
    "load-osd-console",
    "load-auto-profiles",
    "load-select",
    "load-positioning",
    "load-commands",
    "load-context-menu",
];

/// Options derived from `config.toml`. `screenshot_dir` is the resolved folder (the setting,
/// or the Pictures fallback).
pub fn from_config(config: &Config, screenshot_dir: &str) -> OptionList {
    let mut o = OptionList::new();
    let (general, video, audio, subs, net, perf) = (
        &config.general,
        &config.video,
        &config.audio,
        &config.subtitles,
        &config.network,
        &config.performance,
    );

    // The quality profile goes first so the specific options below override it.
    match video.quality {
        VideoQuality::Fast => push(&mut o, "profile", "fast"),
        VideoQuality::High => push(&mut o, "profile", "high-quality"),
        VideoQuality::Balanced => {}
    }

    if !screenshot_dir.is_empty() {
        push(&mut o, "screenshot-directory", screenshot_dir);
    }
    push(&mut o, "screenshot-template", general.screenshot_template.clone());
    push(&mut o, "screenshot-format", general.screenshot_format.as_str());

    // Decoding and cache (§5.4 supporting options).
    push(&mut o, "hwdec-codecs", "all");
    push(&mut o, "vd-lavc-threads", perf.software_threads.to_string());
    push(&mut o, "vd-lavc-dr", "auto");
    push(&mut o, "vd-lavc-film-grain", "gpu");
    push(&mut o, "cache", "yes");
    push(&mut o, "demuxer-max-bytes", format!("{}MiB", perf.demuxer_cache_mb));
    push(&mut o, "demuxer-max-back-bytes", format!("{}MiB", perf.demuxer_back_cache_mb));
    push(&mut o, "demuxer-readahead-secs", perf.readahead_secs.to_string());

    // Video.
    let sync = if video.interpolation { VideoSync::DisplayResample } else { video.sync };
    push(
        &mut o,
        "video-sync",
        match sync {
            VideoSync::Audio => "audio",
            VideoSync::DisplayResample => "display-resample",
        },
    );
    push(&mut o, "interpolation", yes_no(video.interpolation));
    push(&mut o, "deband", yes_no(video.deband));
    push(&mut o, "tone-mapping", video.tone_mapping.clone());
    push(&mut o, "target-peak", keyword_or_int(&video.target_peak));
    push(&mut o, "video-aspect-override", aspect_to_mpv(&video.default_aspect));

    // Audio.
    push(&mut o, "audio-device", audio.device.clone());
    push(&mut o, "alang", audio.preferred_languages.join(","));
    push(&mut o, "volume", audio.default_volume.to_string());
    push(&mut o, "volume-max", audio.max_volume.to_string());
    push(
        &mut o,
        "audio-channels",
        match audio.channels {
            AudioChannels::Auto => "auto-safe",
            AudioChannels::Stereo => "stereo",
            AudioChannels::Surround51 => "5.1",
            AudioChannels::Surround71 => "7.1",
        },
    );
    push(&mut o, "audio-spdif", audio.passthrough.join(","));
    push(&mut o, "audio-exclusive", yes_no(audio.exclusive));
    push(
        &mut o,
        "gapless-audio",
        match audio.gapless {
            Gapless::Weak => "weak",
            Gapless::Yes => "yes",
            Gapless::No => "no",
        },
    );
    push(
        &mut o,
        "replaygain",
        match audio.replaygain {
            ReplayGain::Off => "no",
            ReplayGain::Track => "track",
            ReplayGain::Album => "album",
        },
    );
    push(&mut o, "audio-pitch-correction", yes_no(config.playback.pitch_correction));
    push(&mut o, "speed", config.playback.default_speed.to_string());

    // Subtitles.
    push(&mut o, "slang", subs.preferred_languages.join(","));
    push(
        &mut o,
        "sub-auto",
        match subs.auto_load {
            SubAutoLoad::Off => "no",
            SubAutoLoad::Exact => "exact",
            SubAutoLoad::Fuzzy => "fuzzy",
            SubAutoLoad::All => "all",
        },
    );
    push(&mut o, "sub-file-paths", subs.extra_dirs.join(PATH_LIST_SEPARATOR));
    // Off: only forced tracks are auto-selected (signs, foreign dialogue).
    push(&mut o, "subs-fallback", if subs.show_by_default { "default" } else { "no" });
    push(&mut o, "subs-fallback-forced", "yes");
    push(
        &mut o,
        "subs-with-matching-audio",
        if !subs.show_by_default || subs.forced_only_when_audio_matches { "forced" } else { "yes" },
    );
    o.extend(sub_style(config));
    push(&mut o, "sub-fix-timing", yes_no(subs.fix_timing));
    push(&mut o, "sub-codepage", subs.encoding.clone());

    // Network.
    if !net.user_agent.is_empty() {
        push(&mut o, "user-agent", net.user_agent.clone());
    }
    if !net.proxy.is_empty() {
        push(&mut o, "http-proxy", net.proxy.clone());
    }
    push(&mut o, "cache-secs", net.cache_secs.to_string());
    push(&mut o, "network-timeout", net.timeout_s.to_string());
    if net.auto_reconnect {
        push(&mut o, "stream-lavf-o", "reconnect=1,reconnect_streamed=1,reconnect_delay_max=5");
    }
    push(&mut o, "hls-bitrate", keyword_or_int(&net.hls_bitrate));
    push(&mut o, "ytdl-format", net.ytdlp_format.clone());
    o
}

/// The subtitle style options; also applied live when the style changes.
pub fn sub_style(config: &Config) -> OptionList {
    let subs = &config.subtitles;
    let mut o = OptionList::new();
    if !subs.font.is_empty() {
        push(&mut o, "sub-font", subs.font.clone());
    }
    push(&mut o, "sub-font-size", subs.font_size.to_string());
    push(&mut o, "sub-scale", subs.scale.to_string());
    push(&mut o, "sub-bold", yes_no(subs.bold));
    push(&mut o, "sub-color", subs.color.clone());
    push(&mut o, "sub-outline-color", subs.outline_color.clone());
    push(&mut o, "sub-outline-size", subs.outline_size.to_string());
    push(&mut o, "sub-shadow-offset", subs.shadow_offset.to_string());
    match subs.background {
        SubBackground::None => {
            push(&mut o, "sub-border-style", "outline-and-shadow");
            push(&mut o, "sub-back-color", "#00000000");
        }
        SubBackground::Box => {
            push(&mut o, "sub-border-style", "background-box");
            push(&mut o, "sub-back-color", SUB_BOX_COLOR);
        }
    }
    push(&mut o, "sub-pos", subs.position.to_string());
    push(&mut o, "sub-margin-y", subs.margin_y.to_string());
    push(&mut o, "sub-ass-override", subs.ass_override.as_str());
    o
}

/// 75 % black, in mpv's `#AARRGGBB` form.
pub const SUB_BOX_COLOR: &str = "#C0000000";

#[cfg(windows)]
const PATH_LIST_SEPARATOR: &str = ";";
#[cfg(not(windows))]
const PATH_LIST_SEPARATOR: &str = ":";

fn keyword_or_int(value: &IntOrKeyword) -> String {
    match value {
        IntOrKeyword::Int(n) => n.to_string(),
        IntOrKeyword::Keyword(k) => k.clone(),
    }
}

/// Maps the aspect names used in config and the UI to `video-aspect-override` values.
/// "stretch" and "fill" are not aspect overrides; they are handled with `keepaspect`/`panscan`.
pub fn aspect_to_mpv(aspect: &str) -> String {
    match aspect {
        "" | "auto" | "stretch" | "fill" => "no".to_string(),
        "2.39" => "2.39:1".to_string(),
        other => other.to_string(),
    }
}

/// The raw `[mpv.options]` table from config, as option strings.
pub fn passthrough(config: &Config) -> OptionList {
    config
        .mpv
        .options
        .iter()
        .map(|(name, value)| {
            let text = match value {
                toml::Value::String(s) => s.clone(),
                toml::Value::Boolean(b) => yes_no(*b),
                other => other.to_string(),
            };
            (name.trim_start_matches("--").to_string(), text)
        })
        .collect()
}

/// Options that are starting values rather than settings: re-applying them when the config
/// changes would undo what the user did since (volume, speed) or re-run a whole profile.
const NOT_LIVE: &[&str] = &["volume", "speed", "profile"];

/// The config-derived options that are applied again, live, when a setting changes.
pub fn live(config: &Config, screenshot_dir: &str) -> OptionList {
    from_config(config, screenshot_dir)
        .into_iter()
        .filter(|(name, _)| !NOT_LIVE.contains(&name.as_str()))
        .collect()
}

/// The complete, ordered option list for the primary handle.
pub fn build(
    config: &Config,
    ytdlp_available: bool,
    surface: Vec<(&'static str, String)>,
    hwdec: String,
    screenshot_dir: &str,
    cli: &[(String, String)],
) -> OptionList {
    let mut o = fixed(config, ytdlp_available);
    o.extend(from_config(config, screenshot_dir));
    o.extend(surface.into_iter().map(|(name, value)| (name.to_string(), value)));
    push(&mut o, "hwdec", hwdec);
    o.extend(passthrough(config));
    o.extend(cli.iter().cloned());
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use crate::player::mpv::Mpv;

    fn value<'a>(list: &'a OptionList, name: &str) -> Option<&'a str> {
        // Last one wins, as when applied to mpv in order.
        list.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    #[test]
    fn defaults_produce_the_documented_values() {
        let o = build(
            &config::defaults(),
            false,
            vec![("vo", "gpu-next".into())],
            "d3d11va".into(),
            "C:/Pictures/vihpluh",
            &[],
        );
        assert_eq!(value(&o, "screenshot-directory"), Some("C:/Pictures/vihpluh"));
        assert_eq!(value(&o, "stop-screensaver"), Some("no"));
        assert_eq!(value(&o, "config"), Some("no"));
        assert_eq!(value(&o, "ytdl"), Some("no"));
        assert_eq!(value(&o, "keep-open"), Some("yes"));
        assert_eq!(value(&o, "demuxer-max-bytes"), Some("512MiB"));
        assert_eq!(value(&o, "demuxer-max-back-bytes"), Some("128MiB"));
        assert_eq!(value(&o, "video-sync"), Some("audio"));
        assert_eq!(value(&o, "volume-max"), Some("130"));
        assert_eq!(value(&o, "alang"), Some("eng,en"));
        assert_eq!(value(&o, "sub-auto"), Some("fuzzy"));
        assert_eq!(value(&o, "sub-ass-override"), Some("scale"));
        assert_eq!(value(&o, "hwdec"), Some("d3d11va"));
        assert_eq!(value(&o, "vo"), Some("gpu-next"));
        assert_eq!(value(&o, "profile"), None, "balanced sets no profile");
        assert_eq!(value(&o, "user-agent"), None, "empty strings are not passed");
    }

    #[test]
    fn later_layers_override_earlier_ones() {
        let loaded = config::from_user_text(
            "[video]\ninterpolation = true\n[mpv.options]\nhwdec = \"no\"\ndeband = true\nscale = \"ewa_lanczos\"\n",
        );
        assert_eq!(loaded.warnings, vec![]);
        let cli = vec![("scale".to_string(), "bilinear".to_string())];
        let o = build(&loaded.config, true, vec![], "d3d11va".into(), "", &cli);
        assert_eq!(value(&o, "video-sync"), Some("display-resample"), "interpolation implies it");
        assert_eq!(value(&o, "hwdec"), Some("no"), "[mpv.options] beats the computed chain");
        assert_eq!(value(&o, "deband"), Some("yes"), "booleans become yes/no");
        assert_eq!(value(&o, "scale"), Some("bilinear"), "--mpv-opt beats [mpv.options]");
        assert_eq!(value(&o, "ytdl"), Some("yes"));
    }

    #[test]
    fn live_options_leave_starting_values_alone() {
        let live = live(&config::defaults(), "");
        assert_eq!(value(&live, "volume"), None);
        assert_eq!(value(&live, "speed"), None);
        assert_eq!(value(&live, "volume-max"), Some("130"));
        assert_eq!(value(&live, "sub-font-size"), Some("42"));
    }

    #[test]
    fn aspect_names_map_to_mpv_values() {
        assert_eq!(aspect_to_mpv("auto"), "no");
        assert_eq!(aspect_to_mpv("16:9"), "16:9");
        assert_eq!(aspect_to_mpv("2.39"), "2.39:1");
        assert_eq!(aspect_to_mpv("fill"), "no");
    }

    /// Spike S4(d): every option name and value we generate is accepted by the linked libmpv.
    #[test]
    fn linked_libmpv_accepts_every_generated_option() {
        let mut variants = vec![config::defaults()];
        let all_on = config::from_user_text(
            r##"
            [general]
            screenshot_dir = "shots"
            screenshot_format = "webp"
            [video]
            quality = "high"
            interpolation = true
            deband = true
            target_peak = 203
            default_aspect = "2.39"
            [audio]
            channels = "5.1"
            passthrough = ["ac3", "dts-hd"]
            exclusive = true
            gapless = "yes"
            replaygain = "album"
            [subtitles]
            show_by_default = false
            auto_load = "all"
            font = "Segoe UI"
            bold = true
            background = "box"
            ass_override = "strip"
            fix_timing = true
            [network]
            user_agent = "test"
            proxy = "http://127.0.0.1:8080"
            hls_bitrate = 2000000
            "##,
        );
        assert_eq!(all_on.warnings, vec![]);
        variants.push(all_on.config);
        let fast = config::from_user_text("[video]\nquality = \"fast\"\n");
        variants.push(fast.config);

        for config in variants {
            let mpv = Mpv::create().unwrap();
            let options =
                build(&config, true, vec![("vo", "null".into())], "no".into(), "shots", &[]);
            for (name, value) in options {
                // A window must not appear during tests.
                if name == "force-window" {
                    continue;
                }
                mpv.set_option(&name, &value)
                    .unwrap_or_else(|err| panic!("mpv rejected {name}={value}: {err}"));
            }
            mpv.set_option("ao", "null").unwrap();
            mpv.initialize().unwrap();
        }
    }
}
