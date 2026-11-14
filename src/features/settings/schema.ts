/**
 * The one settings schema (KEYBINDS_AND_CONFIG §6 rule 1): every key of `config.toml`
 * §5.1–§5.10 with its label, tooltip, control, and search keywords. The Settings pages are
 * generated from this, and nothing else decides what a setting looks like.
 *
 * `SETTINGS` is typed against the `Config` generated from Rust, so a key added to or removed
 * from the core without a matching entry here does not compile; `schema.test.ts` checks the
 * rest (defaults, control types, and that labels and tooltips match the reference document).
 */
import { ASPECTS } from "@/actions/definitions/media";
import type { Capabilities } from "@/ipc/bindings/Capabilities";
import type { Config } from "@/ipc/bindings/Config";

export const CATEGORIES = [
  { id: "general", label: "General" },
  { id: "interface", label: "Interface" },
  { id: "controls", label: "Controls" },
  { id: "playback", label: "Playback" },
  { id: "video", label: "Video" },
  { id: "audio", label: "Audio" },
  { id: "subtitles", label: "Subtitles" },
  { id: "library", label: "Library" },
  { id: "network", label: "Network" },
  { id: "performance", label: "Performance" },
] as const;

export type Category = (typeof CATEGORIES)[number]["id"];

/** `"subtitles.font_size"`, … — every leaf of the ten sections. */
export type SettingPath = { [S in Category]: `${S}.${keyof Config[S] & string}` }[Category];

export interface Choice {
  value: string;
  label: string;
}

export type Control =
  | { kind: "toggle" }
  /** ≤ 4 options show as a segmented control, more as a select. `other`: a value typed in
   * the config file that is not in the list is shown as one more option. */
  | { kind: "choice"; options: Choice[]; other?: boolean }
  /** `slider: false` for open-ended step sizes: an exact field only. */
  | {
      kind: "number";
      min: number;
      max: number;
      step: number;
      unit?: string;
      slider?: boolean;
    }
  /** A keyword (`auto`, `max`…) or an exact number. */
  | {
      kind: "keyword";
      options: Choice[];
      min: number;
      max: number;
      unit: string;
      placeholder: string;
    }
  | { kind: "text"; placeholder?: string; mono?: boolean }
  | { kind: "list"; placeholder: string; suggestions?: string[] }
  /** `swatches`: the accent picker's eight presets. */
  | { kind: "color"; swatches?: boolean }
  | { kind: "path"; placeholder: string }
  | { kind: "audio_device" }
  | { kind: "font" }
  | { kind: "eq_preset" };

export interface SettingDef {
  label: string;
  tooltip: string;
  control: Control;
  /** Extra search words: what people call this, including the mpv option behind it. */
  keywords?: string[];
  /** Applies after restart. */
  restart?: true;
  /** Shown only on these platforms. */
  platforms?: ("windows" | "linux")[];
  /** Shown only when the session can do this. */
  needs?: "hdrPassthrough" | "windowPositioning";
}

export interface Setting extends SettingDef {
  path: SettingPath;
  category: Category;
}

const toggle: Control = { kind: "toggle" };
const choice = (options: [value: string, label: string][], other = false): Control => ({
  kind: "choice",
  options: options.map(([value, label]) => ({ value, label })),
  other: other || undefined,
});
const range = (min: number, max: number, step: number, unit?: string): Control => ({
  kind: "number",
  min,
  max,
  step,
  unit,
});
const field = (min: number, max: number, step: number, unit?: string): Control => ({
  kind: "number",
  min,
  max,
  step,
  unit,
  slider: false,
});

export const SETTINGS: Record<SettingPath, SettingDef> = {
  // ── General (§5.1) ─────────────────────────────────────────────────────────
  "general.restore_session": {
    label: "Reopen last session",
    tooltip: "Bring back the queue and position from last time, paused.",
    control: toggle,
    keywords: ["startup", "resume", "restore"],
  },
  "general.single_instance": {
    label: "Use a single window",
    tooltip: "Files opened from the file manager go to the window that's already open.",
    control: toggle,
    keywords: ["instance", "multiple windows"],
    restart: true,
  },
  "general.open_behavior": {
    label: "When opening files while something is playing",
    tooltip: "Replace the queue, add to its end, or play right after the current item.",
    control: choice([
      ["replace", "Replace queue"],
      ["enqueue", "Add to end"],
      ["play_next", "Play next"],
    ]),
    keywords: ["enqueue", "open"],
  },
  "general.pause_on_minimize": {
    label: "Pause when minimized",
    tooltip: "Pauses video (not music) when the window is minimized.",
    control: toggle,
  },
  "general.media_keys": {
    label: "Media keys and system media controls",
    tooltip: "Respond to play/pause/next keys and show what's playing in the OS media overlay.",
    control: toggle,
    keywords: ["smtc", "mpris", "keyboard", "headset"],
    restart: true,
  },
  "general.screenshot_dir": {
    label: "Screenshot folder",
    tooltip: 'Empty = your Pictures folder, in "vihpluh".',
    control: { kind: "path", placeholder: "Pictures/vihpluh" },
    keywords: ["capture", "snapshot", "directory"],
  },
  "general.screenshot_format": {
    label: "Screenshot format",
    tooltip: "PNG is lossless; JPG and WebP are smaller.",
    control: choice([
      ["png", "PNG"],
      ["jpg", "JPG"],
      ["webp", "WebP"],
    ]),
    keywords: ["capture", "snapshot", "image"],
  },
  "general.screenshot_template": {
    label: "Screenshot file name",
    tooltip:
      "mpv template: %F = file name, %wH.%wM.%wS = timestamp. A name already taken gets -0001, -0002….",
    control: { kind: "text", mono: true },
    keywords: ["capture", "snapshot", "template"],
  },
  "general.log_level": {
    label: "Log detail",
    tooltip: "Higher levels help diagnose problems but write more to disk.",
    control: choice([
      ["error", "Errors only"],
      ["warn", "Warnings"],
      ["info", "Info"],
      ["debug", "Debug"],
      ["trace", "Everything"],
    ]),
    keywords: ["logging", "debug", "troubleshoot"],
  },

  // ── Interface (§5.2) ───────────────────────────────────────────────────────
  "interface.theme": {
    label: "Theme",
    tooltip: '"System" follows your OS setting.',
    control: choice([
      ["dark", "Dark"],
      ["light", "Light"],
      ["system", "System"],
    ]),
    keywords: ["dark mode", "light mode", "appearance", "colour scheme"],
  },
  "interface.accent": {
    label: "Accent colour",
    tooltip: "Used for progress, selection, and active controls.",
    control: { kind: "color", swatches: true },
    keywords: ["color", "highlight", "orange", "appearance"],
  },
  "interface.ui_scale": {
    label: "Interface size",
    tooltip: "Scales all controls and text. Doesn't affect video or subtitles.",
    control: range(0.8, 1.5, 0.05, "×"),
    keywords: ["zoom", "scale", "dpi", "bigger", "smaller", "font size"],
  },
  "interface.window_decorations": {
    label: "Title bar",
    tooltip: "Custom blends into the player; native uses your OS window frame.",
    control: choice([
      ["custom", "Custom"],
      ["native", "Native"],
    ]),
    keywords: ["decorations", "frame", "border", "csd"],
    restart: true,
  },
  "interface.autohide_delay_ms": {
    label: "Hide controls after",
    tooltip: "How long the mouse must be still before the bars hide.",
    control: range(500, 10000, 100, "ms"),
    keywords: ["auto-hide", "idle", "bars"],
  },
  "interface.autohide_in_windowed": {
    label: "Auto-hide controls when not fullscreen",
    tooltip: "Off keeps the bars visible in a window.",
    control: toggle,
    keywords: ["bars", "windowed"],
  },
  "interface.sidebar_mode": {
    label: "Side panels",
    tooltip:
      "Dock shrinks the video beside the panel; overlay floats the panel on top. Auto docks in a window and overlays in fullscreen.",
    control: choice([
      ["auto", "Auto"],
      ["dock", "Dock"],
      ["overlay", "Overlay"],
    ]),
    keywords: ["sidebar", "queue", "panel"],
  },
  "interface.sidebar_width": {
    label: "Side panel width",
    tooltip: "Also adjustable by dragging the panel edge.",
    control: range(320, 480, 4, "px"),
    keywords: ["sidebar", "queue", "dock"],
  },
  "interface.seek_preview": {
    label: "Thumbnail previews on the seek bar",
    tooltip: "Shows the frame under the pointer. Local files only.",
    control: toggle,
    keywords: ["thumbnails", "hover", "scrub"],
  },
  "interface.seek_waveform": {
    label: "Waveform seek bar",
    tooltip: "Draws the audio waveform in the seek bar so you can see quiet and loud parts.",
    control: toggle,
    keywords: ["audio", "peaks"],
  },
  "interface.seek_chapter_marks": {
    label: "Chapter marks on the seek bar",
    tooltip: "Splits the bar at chapter boundaries.",
    control: toggle,
    keywords: ["chapters", "segments"],
  },
  "interface.time_display": {
    label: "Right-hand time",
    tooltip: "Total length or time left. Click the time to switch.",
    control: choice([
      ["total", "Total"],
      ["remaining", "Remaining"],
    ]),
    keywords: ["duration", "countdown"],
  },
  "interface.osd_enabled": {
    label: "On-screen feedback",
    tooltip: "Briefly shows volume, seek position, and similar when controls are hidden.",
    control: toggle,
    keywords: ["osd", "overlay", "flash"],
  },
  "interface.osd_shortcut_badge": {
    label: "Show the shortcut I pressed",
    tooltip: "Adds the key you pressed to the on-screen feedback.",
    control: toggle,
    keywords: ["osd", "key", "badge", "hotkey"],
  },
  "interface.osd_position": {
    label: "Feedback position",
    tooltip: "Where on-screen feedback appears.",
    control: choice([
      ["top-left", "Top left"],
      ["top-center", "Top centre"],
      ["top-right", "Top right"],
    ]),
    keywords: ["osd"],
  },
  "interface.osd_duration_ms": {
    label: "Feedback duration",
    tooltip: "How long on-screen feedback stays visible.",
    control: range(300, 5000, 100, "ms"),
    keywords: ["osd"],
  },
  "interface.tooltip_shortcuts": {
    label: "Show shortcuts in tooltips",
    tooltip: "Adds each control's current key to its tooltip.",
    control: toggle,
    keywords: ["hotkey", "key cap", "hint"],
  },
  "interface.reduce_motion": {
    label: "Reduce motion",
    tooltip: "Turns off interface animation.",
    control: choice([
      ["system", "System"],
      ["on", "On"],
      ["off", "Off"],
    ]),
    keywords: ["animation", "accessibility", "transitions"],
  },
  "interface.pip_width": {
    label: "Picture-in-picture width",
    tooltip: "Height follows the video's shape.",
    control: range(240, 960, 10, "px"),
    keywords: ["pip", "mini player"],
  },
  "interface.pip_corner": {
    label: "Picture-in-picture corner",
    tooltip: "Not available on Wayland, where the compositor places windows.",
    control: choice([
      ["top-left", "Top left"],
      ["top-right", "Top right"],
      ["bottom-left", "Bottom left"],
      ["bottom-right", "Bottom right"],
    ]),
    keywords: ["pip", "mini player", "position"],
    needs: "windowPositioning",
  },

  // ── Controls (§5.3) ────────────────────────────────────────────────────────
  "controls.seek_small_s": {
    label: "Small seek step",
    tooltip: "Seconds for ← / →.",
    control: field(0.1, 3600, 1, "s"),
    keywords: ["skip", "jump", "arrow"],
  },
  "controls.seek_medium_s": {
    label: "Medium seek step",
    tooltip: "Seconds for J / L.",
    control: field(0.1, 3600, 1, "s"),
    keywords: ["skip", "jump"],
  },
  "controls.seek_large_s": {
    label: "Large seek step",
    tooltip: "Seconds for Ctrl+← / Ctrl+→.",
    control: field(0.1, 3600, 5, "s"),
    keywords: ["skip", "jump"],
  },
  "controls.precise_seek": {
    label: "Land exactly where I seek",
    tooltip: "Off jumps to the nearest keyframe: faster on very large files, less exact.",
    control: toggle,
    keywords: ["keyframe", "exact", "hr-seek"],
  },
  "controls.volume_step": {
    label: "Volume step",
    tooltip: "Percent per key press or wheel notch.",
    control: field(0.1, 50, 1, "%"),
    keywords: ["wheel", "scroll"],
  },
  "controls.speed_step": {
    label: "Speed step",
    tooltip: "Change in playback speed per key press.",
    control: field(0.01, 1, 0.05, "×"),
    keywords: ["rate", "faster", "slower"],
  },
  "controls.sub_delay_step_ms": {
    label: "Subtitle delay step",
    tooltip: "Milliseconds per key press.",
    control: field(1, 5000, 10, "ms"),
    keywords: ["sync", "offset"],
  },
  "controls.audio_delay_step_ms": {
    label: "Audio delay step",
    tooltip: "Milliseconds per key press.",
    control: field(1, 5000, 10, "ms"),
    keywords: ["sync", "offset", "lip sync"],
  },
  "controls.sub_size_step": {
    label: "Subtitle size step",
    tooltip: "Change in scale per key press.",
    control: field(0.01, 1, 0.01, "×"),
    keywords: ["scale", "bigger", "smaller"],
  },
  "controls.zoom_step": {
    label: "Zoom step",
    tooltip: "Change in video zoom per key press.",
    control: field(0.01, 1, 0.05, "×"),
    keywords: ["magnify", "scale"],
  },
  "controls.drag_moves_window": {
    label: "Drag the video to move the window",
    tooltip: "Off leaves dragging on the video doing nothing.",
    control: toggle,
    keywords: ["mouse", "move"],
  },
  "controls.wheel_invert": {
    label: "Reverse scroll direction",
    tooltip: "Swaps what wheel-up and wheel-down do.",
    control: toggle,
    keywords: ["mouse", "wheel", "natural scrolling", "trackpad"],
  },

  // ── Playback (§5.4) ────────────────────────────────────────────────────────
  "playback.resume": {
    label: "Continue where I left off",
    tooltip: '"Ask" starts from the beginning and offers to resume.',
    control: choice([
      ["always", "Always"],
      ["ask", "Ask"],
      ["never", "Never"],
    ]),
    keywords: ["resume", "position", "remember"],
  },
  "playback.remember_per_file": {
    label: "Remember tracks and delays per file",
    tooltip: "Restores the audio track, subtitle track, and sync you last used for each file.",
    control: toggle,
    keywords: ["language", "subtitle", "audio", "sync"],
  },
  "playback.auto_queue_folder": {
    label: "Queue the rest of the folder",
    tooltip: "Opening one file lines up the others in its folder so Next plays the next episode.",
    control: toggle,
    keywords: ["episodes", "autoplay next", "playlist"],
  },
  "playback.at_queue_end": {
    label: "When the queue finishes",
    tooltip: "Hold keeps the last frame on screen; stop returns to the start screen.",
    control: choice([
      ["hold", "Hold"],
      ["stop", "Stop"],
      ["quit", "Quit"],
    ]),
    keywords: ["end", "finish", "keep-open"],
  },
  "playback.autoplay": {
    label: "Start playing when a file opens",
    tooltip: "Off opens files paused.",
    control: toggle,
    keywords: ["pause", "auto play"],
  },
  "playback.default_speed": {
    label: "Default speed",
    tooltip: "The speed every file starts at.",
    control: range(0.25, 4, 0.05, "×"),
    keywords: ["rate", "faster", "slower"],
  },
  "playback.pitch_correction": {
    label: "Keep pitch when changing speed",
    tooltip: "Off makes voices higher or lower as speed changes.",
    control: toggle,
    keywords: ["scaletempo", "chipmunk", "audio"],
  },
  "playback.sleep_action": {
    label: "When the sleep timer ends",
    tooltip: "Pause playback, or quit vihpluh.",
    control: choice([
      ["pause", "Pause"],
      ["quit", "Quit"],
    ]),
    keywords: ["timer", "bedtime"],
  },
  "playback.finished_percent": {
    label: "Count as finished at",
    tooltip: "Past this point a file is marked watched and won't resume.",
    control: range(80, 100, 1, "%"),
    keywords: ["watched", "resume", "credits"],
  },
  "playback.presets_enabled": {
    label: "Apply presets automatically",
    tooltip: "Presets are saved from the bar at the bottom of the side panel.",
    control: toggle,
    keywords: ["profile", "series", "folder"],
  },

  // ── Video (§5.5) ───────────────────────────────────────────────────────────
  "video.hwdec": {
    label: "Hardware decoding",
    tooltip:
      "Uses your graphics card to decode video. Turn off only to troubleshoot. A custom mpv value can be typed in the config file.",
    control: choice(
      [
        ["auto", "Auto"],
        ["off", "Off"],
      ],
      true,
    ),
    keywords: ["hwdec", "gpu", "acceleration", "d3d11va", "vaapi", "nvdec"],
  },
  "video.gpu_api": {
    label: "Graphics API (Windows)",
    tooltip: "Auto uses Direct3D 11, the most reliable choice with HDR.",
    control: choice([
      ["auto", "Auto"],
      ["d3d11", "Direct3D 11"],
      ["vulkan", "Vulkan"],
      ["opengl", "OpenGL"],
    ]),
    keywords: ["gpu-api", "renderer", "d3d11", "directx"],
    restart: true,
    platforms: ["windows"],
  },
  "video.quality": {
    label: "Scaling quality",
    tooltip: "High looks sharper when upscaling but needs a stronger graphics card.",
    control: choice([
      ["fast", "Fast"],
      ["balanced", "Balanced"],
      ["high", "High"],
    ]),
    keywords: ["upscaling", "sharpness", "profile", "high-quality", "scaler"],
  },
  "video.tone_mapping": {
    label: "HDR tone mapping",
    tooltip: "How HDR video is fitted to a non-HDR screen. Auto picks a good curve.",
    control: choice(
      [
        ["auto", "Auto"],
        ["bt.2390", "BT.2390"],
        ["hable", "Hable (filmic)"],
        ["mobius", "Möbius"],
        ["reinhard", "Reinhard"],
        ["clip", "Clip"],
        ["spline", "Spline"],
        ["bt.2446a", "BT.2446a"],
        ["st2094-40", "ST 2094-40"],
      ],
      true,
    ),
    keywords: ["hdr", "tone-mapping", "sdr", "dolby vision", "hdr10"],
  },
  "video.hdr_passthrough": {
    label: "HDR passthrough",
    tooltip: "Sends HDR untouched to an HDR display. Windows only.",
    control: choice([
      ["auto", "Auto"],
      ["off", "Off"],
    ]),
    keywords: ["hdr", "hdr10", "target-colorspace-hint", "display"],
    needs: "hdrPassthrough",
  },
  "video.target_peak": {
    label: "Display peak brightness",
    tooltip: "Set if tone-mapped HDR looks too dark or too bright.",
    control: {
      kind: "keyword",
      options: [{ value: "auto", label: "Auto" }],
      min: 10,
      max: 10000,
      unit: "nits",
      placeholder: "nits",
    },
    keywords: ["hdr", "nits", "target-peak", "brightness"],
  },
  "video.deband": {
    label: "Debanding",
    tooltip: "Smooths colour banding in gradients.",
    control: toggle,
    keywords: ["banding", "gradient", "deband"],
  },
  "video.interpolation": {
    label: "Motion interpolation",
    tooltip: "Smooths motion by blending frames to match your display.",
    control: toggle,
    keywords: ["smooth motion", "frame blending", "judder"],
  },
  "video.sync": {
    label: "Timing source",
    tooltip: "Display sync reduces judder but slightly resamples audio.",
    control: choice([
      ["audio", "Audio"],
      ["display-resample", "Display"],
    ]),
    keywords: ["video-sync", "judder", "display-resample", "vsync"],
  },
  "video.default_aspect": {
    label: "Default aspect ratio",
    tooltip: "The shape every video starts with. Auto uses the file's own.",
    control: choice(
      ASPECTS.map((a) => [a.value, a.label]),
      true,
    ),
    keywords: ["stretch", "fill", "crop", "16:9", "4:3"],
  },
  "video.linux_force_xwayland": {
    label: "Run through XWayland (Linux)",
    tooltip: "Compatibility switch if video misbehaves under Wayland.",
    control: toggle,
    keywords: ["wayland", "x11", "gdk_backend"],
    restart: true,
    platforms: ["linux"],
  },

  // ── Audio (§5.6) ───────────────────────────────────────────────────────────
  "audio.device": {
    label: "Output device",
    tooltip: "Auto follows the system default.",
    control: { kind: "audio_device" },
    keywords: ["speakers", "headphones", "sound card", "audio-device"],
  },
  "audio.preferred_languages": {
    label: "Preferred audio languages",
    tooltip: "First available language in this list is chosen. Use codes like eng, jpn.",
    control: { kind: "list", placeholder: "Add a language code" },
    keywords: ["alang", "dub", "track"],
  },
  "audio.remember_volume": {
    label: "Remember volume",
    tooltip: "Starts at the volume you last used.",
    control: toggle,
    keywords: ["loudness", "restore"],
  },
  "audio.default_volume": {
    label: "Starting volume",
    tooltip: "Used when volume isn't remembered.",
    control: range(0, 150, 1, "%"),
    keywords: ["loudness"],
  },
  "audio.max_volume": {
    label: "Maximum volume",
    tooltip: "Above 100 % boosts quiet audio and can distort.",
    control: range(100, 150, 5, "%"),
    keywords: ["boost", "amplify", "volume-max"],
  },
  "audio.normalize": {
    label: "Volume normalisation",
    tooltip: "Evens out loud and quiet moments.",
    control: toggle,
    keywords: ["dynaudnorm", "night mode", "loudness", "compress", "normalization"],
  },
  "audio.channels": {
    label: "Speaker layout",
    tooltip: "Auto matches your output device.",
    control: choice([
      ["auto", "Auto"],
      ["stereo", "Stereo"],
      ["5.1", "5.1"],
      ["7.1", "7.1"],
    ]),
    keywords: ["surround", "downmix", "audio-channels"],
  },
  "audio.passthrough": {
    label: "Pass through to receiver",
    tooltip: "Formats sent undecoded to an AV receiver: ac3, eac3, dts, dts-hd, truehd.",
    control: {
      kind: "list",
      placeholder: "Add a format",
      suggestions: ["ac3", "eac3", "dts", "dts-hd", "truehd"],
    },
    keywords: ["spdif", "bitstream", "hdmi", "dolby", "atmos"],
  },
  "audio.exclusive": {
    label: "Exclusive output",
    tooltip: "Bypasses the system mixer. Other apps go silent while playing.",
    control: toggle,
    keywords: ["wasapi", "bit perfect", "audio-exclusive"],
  },
  "audio.equalizer_enabled": {
    label: "Equalizer",
    tooltip: "Shapes the sound with the preset below. The side panel has the full editor.",
    control: toggle,
    keywords: ["eq", "bass", "treble"],
  },
  "audio.equalizer_preset": {
    label: "Equalizer preset",
    tooltip: "The preset used when the equalizer is on.",
    control: { kind: "eq_preset" },
    keywords: ["eq", "bass", "treble"],
  },
  "audio.gapless": {
    label: "Gapless playback",
    tooltip: "Removes the gap between tracks of the same album.",
    control: choice([
      ["weak", "Same format"],
      ["yes", "Always"],
      ["no", "Off"],
    ]),
    keywords: ["album", "gap", "gapless-audio"],
  },
  "audio.replaygain": {
    label: "ReplayGain",
    tooltip: "Uses loudness tags in music files to level volume.",
    control: choice([
      ["off", "Off"],
      ["track", "Track"],
      ["album", "Album"],
    ]),
    keywords: ["loudness", "normalise", "music"],
  },

  // ── Subtitles (§5.7) ───────────────────────────────────────────────────────
  "subtitles.preferred_languages": {
    label: "Preferred subtitle languages",
    tooltip: "First available language in this list is chosen.",
    control: { kind: "list", placeholder: "Add a language code" },
    keywords: ["slang", "track", "captions"],
  },
  "subtitles.show_by_default": {
    label: "Show subtitles by default",
    tooltip: "Off shows only forced subtitles (signs and foreign dialogue).",
    control: toggle,
    keywords: ["captions", "forced", "auto"],
  },
  "subtitles.forced_only_when_audio_matches": {
    label: "Only forced subtitles when I understand the audio",
    tooltip: "If the audio is in one of your preferred audio languages, full subtitles stay off.",
    control: toggle,
    keywords: ["forced", "signs", "language"],
  },
  "subtitles.auto_load": {
    label: "Load matching subtitle files",
    tooltip:
      "Exact needs the same file name; fuzzy accepts similar names; all loads every subtitle in the folder.",
    control: choice([
      ["off", "Off"],
      ["exact", "Exact"],
      ["fuzzy", "Similar names"],
      ["all", "All"],
    ]),
    keywords: ["external", "srt", "sub-auto", "sidecar"],
  },
  "subtitles.extra_dirs": {
    label: "Subtitle subfolders",
    tooltip: "Folder names next to the video to search for subtitles.",
    control: { kind: "list", placeholder: "Add a folder name" },
    keywords: ["external", "subs", "directory", "sub-file-paths"],
  },
  "subtitles.font": {
    label: "Font",
    tooltip: "Empty uses the system sans-serif.",
    control: { kind: "font" },
    keywords: ["typeface", "family", "sub-font"],
  },
  "subtitles.font_size": {
    label: "Font size",
    tooltip: "Exact size; presets S/M/L/XL in the side panel scale it.",
    control: range(10, 120, 1),
    keywords: ["subtitle size", "text size", "bigger", "smaller"],
  },
  "subtitles.scale": {
    label: "Size multiplier",
    tooltip: "Multiplies the size of every subtitle, styled ones included.",
    control: range(0.5, 3, 0.05, "×"),
    keywords: ["subtitle size", "sub-scale", "bigger", "smaller"],
  },
  "subtitles.bold": {
    label: "Bold",
    tooltip: "Draws subtitle text in a bold weight.",
    control: toggle,
    keywords: ["weight", "thick"],
  },
  "subtitles.color": {
    label: "Text colour",
    tooltip: "The colour of subtitle text.",
    control: { kind: "color" },
    keywords: ["color", "white", "yellow"],
  },
  "subtitles.outline_color": {
    label: "Outline colour",
    tooltip: "The colour of the border around subtitle text.",
    control: { kind: "color" },
    keywords: ["color", "border", "stroke"],
  },
  "subtitles.outline_size": {
    label: "Outline thickness",
    tooltip: "How thick the border around subtitle text is. 0 turns it off.",
    control: range(0, 10, 0.5),
    keywords: ["border", "stroke"],
  },
  "subtitles.shadow_offset": {
    label: "Shadow",
    tooltip: "How far the drop shadow sits from the text. 0 turns it off.",
    control: range(0, 10, 0.5),
    keywords: ["drop shadow"],
  },
  "subtitles.background": {
    label: "Background",
    tooltip: "A box improves legibility over busy scenes.",
    control: choice([
      ["none", "None"],
      ["box", "Box"],
    ]),
    keywords: ["box", "opaque", "legibility"],
  },
  "subtitles.position": {
    label: "Vertical position",
    tooltip: "100 = bottom. Lower moves subtitles up.",
    control: range(0, 100, 1, "%"),
    keywords: ["sub-pos", "raise", "move"],
  },
  "subtitles.margin_y": {
    label: "Bottom margin",
    tooltip: "Space kept between subtitles and the edge of the picture.",
    control: range(0, 200, 1, "px"),
    keywords: ["padding", "edge", "sub-margin-y"],
  },
  "subtitles.ass_override": {
    label: "Styled (ASS) subtitles",
    tooltip:
      "Keep their styling; keep it but apply my size; override with my style; or strip to plain text.",
    control: choice([
      ["no", "Keep styling"],
      ["scale", "Apply my size"],
      ["force", "Use my style"],
      ["strip", "Plain text"],
    ]),
    keywords: ["anime", "ssa", "typesetting", "sub-ass-override"],
  },
  "subtitles.fix_timing": {
    label: "Close small gaps between lines",
    tooltip: "Removes flicker between consecutive lines.",
    control: toggle,
    keywords: ["flicker", "timing"],
  },
  "subtitles.encoding": {
    label: "Text encoding",
    tooltip: "Change only if characters look garbled.",
    control: { kind: "text", mono: true, placeholder: "auto" },
    keywords: ["codepage", "charset", "utf-8", "garbled"],
  },

  // ── Library (§5.8) ─────────────────────────────────────────────────────────
  "library.track_history": {
    label: "Remember what I played",
    tooltip: "Needed for Recent and for continuing where you left off. Off writes no history.",
    control: toggle,
    keywords: ["history", "privacy", "recent", "resume"],
  },
  "library.scan_on_startup": {
    label: "Check folders for changes at startup",
    tooltip: "Looks for new, moved, and deleted files each time vihpluh starts.",
    control: toggle,
    keywords: ["scan", "rescan", "index"],
  },
  "library.watch_folders": {
    label: "Watch folders while running",
    tooltip: "Picks up new files without a rescan.",
    control: toggle,
    keywords: ["monitor", "scan", "live"],
  },
  "library.group_series": {
    label: "Group episodes into series",
    tooltip: "Uses file names to group seasons and episodes.",
    control: toggle,
    keywords: ["tv", "season", "shows"],
  },
  "library.extra_extensions": {
    label: "Additional file types",
    tooltip: "Extensions to treat as media, without the dot.",
    control: { kind: "list", placeholder: "Add an extension" },
    keywords: ["formats", "extension"],
  },
  "library.ignore_patterns": {
    label: "Ignore",
    tooltip: "Paths matching these patterns are skipped.",
    control: { kind: "list", placeholder: "Add a pattern" },
    keywords: ["exclude", "skip", "glob", "sample"],
  },
  "library.recent_limit": {
    label: "Recent items to keep",
    tooltip: "How many entries the Recent list holds. 0 keeps none.",
    control: range(0, 500, 10),
    keywords: ["history"],
  },

  // ── Network (§5.9) ─────────────────────────────────────────────────────────
  "network.use_ytdlp": {
    label: "Play web page links with yt-dlp",
    tooltip:
      "Needs yt-dlp installed. Lets you paste video page addresses, not just direct streams.",
    control: toggle,
    keywords: ["youtube", "ytdl", "url", "website"],
  },
  "network.ytdlp_format": {
    label: "Preferred stream quality",
    tooltip: "yt-dlp format expression.",
    control: { kind: "text", mono: true },
    keywords: ["youtube", "resolution", "ytdl-format", "1080p", "4k"],
  },
  "network.user_agent": {
    label: "Browser identity",
    tooltip: "Some streams need a specific user agent. Empty uses the default.",
    control: { kind: "text", placeholder: "Default" },
    keywords: ["user-agent", "header"],
  },
  "network.proxy": {
    label: "Proxy",
    tooltip: "For example http://127.0.0.1:8080.",
    control: { kind: "text", mono: true, placeholder: "None" },
    keywords: ["http-proxy", "vpn"],
  },
  "network.cache_secs": {
    label: "Stream buffer",
    tooltip: "Seconds of stream to buffer ahead.",
    control: range(1, 600, 1, "s"),
    keywords: ["cache", "buffering", "stutter"],
  },
  "network.timeout_s": {
    label: "Connection timeout",
    tooltip: "How long to wait for a stream to answer before giving up.",
    control: range(1, 120, 1, "s"),
    keywords: ["network-timeout", "slow"],
  },
  "network.auto_reconnect": {
    label: "Reconnect dropped streams",
    tooltip: "Tries again when a stream's connection drops instead of stopping.",
    control: toggle,
    keywords: ["retry", "iptv", "live"],
  },
  "network.hls_bitrate": {
    label: "Adaptive stream quality",
    tooltip:
      "Which variant of an adaptive stream to play: the best, the lightest, or a bitrate cap.",
    control: {
      kind: "keyword",
      options: [
        { value: "max", label: "Highest" },
        { value: "min", label: "Lowest" },
      ],
      min: 1,
      max: 1_000_000_000,
      unit: "bit/s",
      placeholder: "bit/s cap",
    },
    keywords: ["hls", "bitrate", "iptv", "m3u8"],
  },
  "network.load_remote_artwork": {
    label: "Load channel logos",
    tooltip:
      "Downloads images referenced by stream lists. Off keeps vihpluh fully offline unless you open a stream.",
    control: toggle,
    keywords: ["images", "privacy", "offline", "iptv", "artwork"],
  },

  // ── Performance (§5.10) ────────────────────────────────────────────────────
  "performance.demuxer_cache_mb": {
    label: "Read-ahead memory",
    tooltip:
      "Memory for buffering ahead. Larger helps high-bitrate files on slow or network drives.",
    control: range(32, 4096, 32, "MB"),
    keywords: ["cache", "buffer", "ram", "demuxer-max-bytes"],
  },
  "performance.demuxer_back_cache_mb": {
    label: "Seek-back memory",
    tooltip: "Memory kept behind the playhead so short rewinds are instant.",
    control: range(0, 2048, 16, "MB"),
    keywords: ["cache", "buffer", "ram", "rewind"],
  },
  "performance.readahead_secs": {
    label: "Read-ahead time",
    tooltip: "How many seconds of the file are read ahead of the playhead.",
    control: range(1, 120, 1, "s"),
    keywords: ["cache", "buffer"],
  },
  "performance.software_threads": {
    label: "Software decoding threads",
    tooltip: "0 = automatic.",
    control: range(0, 64, 1),
    keywords: ["cpu", "cores", "vd-lavc-threads"],
  },
  "performance.thumbnail_hwdec": {
    label: "Decoder for previews",
    tooltip: 'Use "no" if previews cause stutter.',
    control: { kind: "text", mono: true },
    keywords: ["thumbnails", "hwdec", "seek preview"],
  },
  "performance.waveform": {
    label: "Analyse audio for waveforms",
    tooltip: "On demand analyses a file the first time a waveform is shown.",
    control: choice([
      ["on_demand", "On demand"],
      ["always", "Always"],
      ["off", "Off"],
    ]),
    keywords: ["waveform", "analysis", "background"],
  },
  "performance.cache_limit_mb": {
    label: "Disk cache limit",
    tooltip: "Space for thumbnails and waveforms. Oldest are removed first.",
    control: range(64, 16384, 64, "MB"),
    keywords: ["storage", "thumbnails", "disk space"],
  },
  "performance.webkit_dmabuf": {
    label: "Interface GPU buffers (Linux)",
    tooltip: "Compatibility switch for blank or garbled interface on some NVIDIA setups.",
    control: choice([
      ["auto", "Auto"],
      ["on", "On"],
      ["off", "Off"],
    ]),
    keywords: ["nvidia", "wayland", "webkit", "dmabuf", "blank window"],
    restart: true,
    platforms: ["linux"],
  },
};

/** Every setting in page order. */
export const SETTING_LIST: Setting[] = (
  Object.entries(SETTINGS) as [SettingPath, SettingDef][]
).map(([path, def]) => ({ ...def, path, category: path.slice(0, path.indexOf(".")) as Category }));

/** Whether a setting applies to this platform and session (§6 rule 6: hidden, not disabled). */
export function isAvailable(setting: SettingDef, caps: Capabilities | undefined): boolean {
  if (!caps) return true;
  if (setting.platforms) {
    const platform = caps.session === "windows" ? "windows" : "linux";
    if (!setting.platforms.includes(platform)) return false;
  }
  if (setting.needs && !caps[setting.needs]) return false;
  return true;
}

/** Reads a dotted setting path out of a config. */
export function valueAt(config: Config, path: SettingPath): unknown {
  const dot = path.indexOf(".");
  const section = config[path.slice(0, dot) as Category] as unknown as Record<string, unknown>;
  return section[path.slice(dot + 1)];
}

/** Value equality for settings: numbers within float noise, lists element by element. */
export function sameValue(a: unknown, b: unknown): boolean {
  if (typeof a === "number" && typeof b === "number") return Math.abs(a - b) < 1e-9;
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length === b.length && a.every((item, i) => sameValue(item, b[i]));
  }
  return a === b;
}
