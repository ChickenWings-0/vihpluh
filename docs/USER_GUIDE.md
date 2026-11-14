# vihpluh — User Guide

How to install, run, and get the most out of vihpluh. Every shortcut below is the default of the
`vihpluh` keymap; all of them can be changed (see [Customization](#9-customization-and-keybindings)).
The complete list of actions, shortcuts, and settings is in [KEYBINDS_AND_CONFIG](KEYBINDS_AND_CONFIG.md).

> **Platform status.** Version 0.1.0 is released for Windows, and everything here has been exercised
> on Windows 11. On Linux the app builds and its tests pass, but the video backend is not finished and
> it has not been run on a desktop yet; Linux paths and behaviour below describe the intended design.

## Contents

1. [Installation and portable mode](#1-installation-and-portable-mode)
2. [Command-line reference](#2-command-line-reference)
3. [Playback and the queue](#3-playback-and-the-queue)
4. [Subtitles](#4-subtitles)
5. [Per-show and per-folder presets](#5-per-show-and-per-folder-presets)
6. [Audio, equalizer, and gapless playback](#6-audio-equalizer-and-gapless-playback)
7. [Video adjustments, HDR, and GLSL shaders](#7-video-adjustments-hdr-and-glsl-shaders)
8. [Library, playlists, and IPTV streams](#8-library-playlists-and-iptv-streams)
9. [Customization and keybindings](#9-customization-and-keybindings)
10. [Troubleshooting](#10-troubleshooting)

---

## 1. Installation and portable mode

### 1.1 Windows

Download either package from [GitHub Releases](https://github.com/ChickenWings-0/vihpluh/releases/latest).

| Package | What it does |
|---|---|
| `vihpluh_<version>_x64-setup.exe` | Per-user installer (no administrator rights). Installs `vihpluh.exe` with `libmpv-2.dll` beside it and registers "Open with" for common video, audio, and playlist types. |
| `vihpluh-<version>-windows-x64-portable.zip` | Unzip anywhere (a USB stick works). It contains an empty file named `portable`, so every setting and cache stays in a `data` folder next to the executable. |

The WebView2 runtime is part of Windows 11 and Windows 10 1803+. If it is missing:
`winget install --id Microsoft.EdgeWebView2Runtime`.

### 1.2 Fedora

There is no prebuilt RPM on the releases page yet: build one from source with `pnpm tauri build`
(steps in the [README](../README.md#fedora)). Installing it with
`sudo dnf install ./vihpluh-<version>.x86_64.rpm` pulls in `mpv-libs`, `webkit2gtk4.1`, `gtk3`, and
`libepoxy`. Stock Fedora cannot decode HEVC: enable [RPM Fusion](https://rpmfusion.org/Configuration)
and follow the codec steps in the [README](../README.md#fedora). `scripts/check-deps.sh --runtime`
reports what is still missing.

### 1.3 Where vihpluh keeps its files

vihpluh stores only what you change. An untouched install writes no config file at all.

**Standard install:**

| What | Windows | Linux |
|---|---|---|
| Config: `config.toml`, `keybinds.toml`, `eq-presets.toml`, `presets/`, `playlists/`, `shaders/` | `%APPDATA%\vihpluh\` | `~/.config/vihpluh/` (`$XDG_CONFIG_HOME`) |
| Data: `vihpluh.db` (library, history, bookmarks), `session.json`, `window.json` | `%LOCALAPPDATA%\vihpluh\data\` | `~/.local/share/vihpluh/` (`$XDG_DATA_HOME`) |
| Cache: `thumbs/`, `waveforms/`, `cues/` | `%LOCALAPPDATA%\vihpluh\cache\` | `~/.cache/vihpluh/` (`$XDG_CACHE_HOME`) |
| Logs: `vihpluh.log` (rotated, 5 × 2 MB) | `%LOCALAPPDATA%\vihpluh\logs\` | `~/.local/state/vihpluh/` (`$XDG_STATE_HOME`) |
| Screenshots (when `general.screenshot_dir` is empty) | `Pictures\vihpluh\` | `~/Pictures/vihpluh/` |

The cache is always safe to delete. Settings → Performance shows its size and has a **Clear cache**
button; `performance.cache_limit_mb` (default 1024) caps it.

### 1.4 One folder for everything (portable mode)

Any of these puts all files under a single root, checked in this order:

1. `--home <DIR>` on the command line,
2. the `VIHPLUH_HOME` environment variable,
3. a file named `portable` next to the executable (the root is then `<exe folder>/data`).

The layout under the root is always:

```
<root>/
├── config/       config.toml, keybinds.toml, eq-presets.toml, presets/, playlists/, shaders/
├── data/         vihpluh.db, session.json, window.json (and the WebView2 profile on Windows)
├── cache/        thumbs/, waveforms/, cues/
├── logs/         vihpluh.log
└── screenshots/  used when general.screenshot_dir is empty
```

In these modes, empty screenshot settings write to `<root>/screenshots`, never to your Pictures
folder, so a portable copy leaves nothing behind on the machine.

---

## 2. Command-line reference

```
vihpluh [OPTIONS] [FILES_OR_URLS]...
```

**Positional arguments.** Any mix of files, folders, and URLs.

- A **folder** expands to the media files directly inside it (not subfolders), in natural order
  (`E2` before `E10`).
- An **`.m3u` / `.m3u8` file** expands to the entries it lists. Relative entries are resolved
  against the playlist's folder.
- A **URL** (`https://…`, `rtsp://…`, and anything else mpv/FFmpeg understands) is played as a
  stream. With yt-dlp installed, web page links work too (`network.use_ytdlp`).
- Relative paths are made absolute before they are handed to an already-running window.

| Option | Effect |
|---|---|
| `--enqueue` | Add to the running window's queue instead of replacing it. Cannot be combined with `--play-next`. |
| `--play-next` | Insert right after the current item. |
| `--start <TIME>` | Start position: seconds (`90`, `1.5`), `mm:ss`, `hh:mm:ss`, or a percentage (`50%`). Only the leading field may exceed 59 (`90:00` is fine). |
| `--fullscreen` | Start in fullscreen. |
| `--pip` | Start in picture-in-picture. |
| `--zen` | Start in Zen mode (everything hidden). |
| `--paused` | Load without starting playback. |
| `--no-resume` | Ignore the saved resume position for what is opened. |
| `--no-session` | Do not restore or save the session queue, nor the window's size and place. |
| `--preset <ID>` | Force a preset (by its id, the file name without `.toml`) on the opened items, whether or not its match rule applies. |
| `--home <DIR>` | Use `DIR` as the root for config, data, cache, and logs ([§1.4](#14-one-folder-for-everything-portable-mode)). |
| `--log-level <LEVEL>` | `error`, `warn`, `info`, `debug`, or `trace`, for this run (overrides `general.log_level`). |
| `--mpv-opt <K=V>` | A raw mpv option, applied before playback starts. Repeatable; a leading `--` on the key is accepted. |
| `-V`, `--version` | Print the vihpluh, mpv, and FFmpeg versions and the video backend. |
| `-h`, `--help` | Print help. |

When vihpluh is already open (`general.single_instance = true`, the default), a second launch hands
its arguments to the open window and exits. Without `--enqueue` / `--play-next`, the open window
follows `general.open_behavior` (`replace` by default).

**Examples:**

```sh
vihpluh "D:/Shows/Severance/S02E03.mkv"               # plays it and queues the rest of the folder
vihpluh "D:/Shows/Severance/"                          # the whole folder, in natural order
vihpluh --start 12:30 --paused movie.mkv               # open paused at 12:30
vihpluh --start 50% movie.mkv
vihpluh --enqueue album/*.flac                         # add to the queue of the open window
vihpluh --play-next "https://example.com/live.m3u8"
vihpluh mix.m3u8                                       # play what the list contains
vihpluh --preset night-mode film.mkv
vihpluh --mpv-opt scale=ewa_lanczossharp --mpv-opt deband=yes film.mkv
vihpluh --home "E:/vihpluh-test" --no-session --log-level debug
```

---

## 3. Playback and the queue

### 3.1 Opening things

| How | Result |
|---|---|
| `Ctrl+O` / `Ctrl+Shift+O` | Open a file / a folder. |
| `Ctrl+N` | Open a URL (any stream mpv understands; page links with yt-dlp). |
| `Ctrl+V` | Play the path or URL on the clipboard. |
| Drag files onto the **video** | Replaces what is playing ("Drop to play"). |
| Drag files onto the **queue panel** | Inserts them at the line under the pointer ("Drop to add to the queue"). |
| Double-click a file in the file manager | Goes to the open window (see `general.open_behavior`). |

> Holding Shift while dropping does **not** enqueue: the webview cannot see modifier keys during an
> OS drag. Drop onto the queue panel (`Q`) to add instead of replacing.

### 3.2 Folder auto-queueing

Opening exactly **one local file** with `playback.auto_queue_folder = true` (the default) fills the
queue with every media file in that folder, in natural order, and starts at the file you opened.
`N` / `P` then walk the season. As soon as you edit the queue by hand (reorder, remove, add), it
becomes an ordinary queue.

### 3.3 The queue

`Q` opens the queue panel. The queue is what plays next, right now. It is separate from saved
playlists and from the library, and editing it never interrupts playback.

- Drag rows to reorder. `Enter` plays the selected row, `Delete` removes it (with Undo),
  `Alt+Up` / `Alt+Down` move it, `Ctrl+A` selects all.
- `N` / `P` next / previous. Previous restarts the current item if it has played for more than 5 s.
- `R` cycles repeat: off → all → one. `Shift+R` shuffles: every item plays once, in a shuffled
  order that starts with the current item. The visible list order does not change.
- "Stop after this item" is a one-shot flag.
- When the queue ends, `playback.at_queue_end` decides: `hold` (keep the last frame, default),
  `stop` (back to the start screen), or `quit`.
- Files that cannot be played are marked with an error and skipped. Three failures in a row stop
  playback with one summary message.

### 3.4 Session restore

With `general.restore_session = true` (default), quitting saves the queue, the current item, and the
position. The next launch brings them back **paused** at the same spot. The window's size,
position, and maximized state are remembered separately. `--no-session` skips both for one run.

### 3.5 Resume

`playback.resume` decides what happens when you reopen a file you stopped part-way through:

| Mode | Behaviour |
|---|---|
| `always` (default) | Starts 3 s before where you stopped, with a "Start over" option in the message. |
| `ask` | Opens the file **paused** at the beginning with "You stopped at 12:34 · Resume". |
| `never` | Always starts at the beginning. |

A file past `playback.finished_percent` (default 95 %) counts as watched and starts from 0.
With `playback.remember_per_file = true`, the audio track, subtitle track, and both delays you last
used for a file come back too. Resume and memory need `library.track_history = true`. Turning it off
writes no history at all.

### 3.6 Picture-in-picture

`Ctrl+P` shrinks the window into a small always-on-top player in a screen corner
(`interface.pip_corner`, default bottom-right; `interface.pip_width`, default 480 px). Hovering shows
compact controls. Drag the picture to move it, and drag an edge to resize it; the video's shape is kept.
`Ctrl+P` or `Escape` restores the exact previous size, position, and mode. On Wayland the compositor
places windows, so the corner setting and always-on-top are not available there.

### 3.7 Other playback tools

- Seek: `←` / `→` (5 s), `J` / `L` (10 s), `Ctrl+←` / `Ctrl+→` (60 s), `Shift+←` / `Shift+→`
  (1 s, frame-exact), `0`–`9` jump to 0–90 %, `Home` to the start, `,` / `.` step a frame.
- Speed: `[` / `]` (±0.1×), `Shift+[` / `Shift+]` (÷2 / ×2), `Backspace` resets.
- `Shift+L` A–B loop (set A → set B → clear). `Ctrl+T` sleep timer (off → 15 → 30 → 45 → 60 min →
  end of file); it fades the volume before pausing (or quitting, `playback.sleep_action`).
- `Ctrl+B` drops a bookmark. `B` shows bookmarks and chapters on one list, and
  `PageDown` / `PageUp` step through both. Right-click the seek bar for "Add bookmark here",
  A–B points, and "Copy timestamp".
- `S` / `Shift+S` take a screenshot with / without subtitles (`general.screenshot_format`,
  `general.screenshot_template`). If the name is already taken, `-0001`, `-0002`, … is added.
- `I` shows the stream inspector: codec, resolution, frame rate, bit depth, HDR, bitrate, the
  decoder in use (for example `d3d11va (hardware)` or `software`), dropped and delayed frames.
- `F` fullscreen, `Z` Zen mode (hides everything, including the cursor), `T` always on top,
  `\` sidebar, `Ctrl+Shift+P` command palette, `F1` every shortcut.

---

## 4. Subtitles

Open the Subtitles tab with `Alt+1` (or `\` for the last tab used). `V` cycles tracks,
`Shift+V` shows / hides them, and `Alt+V` adds a subtitle file.

### 4.1 Fixing sync

| Keys | Effect |
|---|---|
| `G` / `H` | Subtitle delay −50 ms / +50 ms (`controls.sub_delay_step_ms`). |
| `Ctrl+G` | Shift the subtitles so the **previous** line starts right now. |
| `Ctrl+H` | Shift the subtitles so the **next** line starts right now. |
| `Alt+G` | Reset the delay. |

`Ctrl+G` / `Ctrl+H` are the quickest way to fix a file that is out by seconds. Pause when you hear
a line begin, press `Ctrl+H`, and that line moves to now. `G` / `H` then trim the result.

### 4.2 The subtitle timeline

The Subtitles tab draws the audio waveform with the subtitle lines on a lane beneath it, around a
fixed playhead. Lines show where they would appear with the current delay.

- **Drag the cue lane** sideways to change the delay live. The delay slider and value follow.
- **Hold `Shift` while dragging** to snap the nearest line's start to where sound begins in the
  waveform. Speech onsets make this exact to about one waveform step (20 ms).
- **Drag the waveform** to scrub the video.
- The two buttons beside the timeline do the same as `Ctrl+G` / `Ctrl+H`.
- Zoom changes how many seconds each side of now are shown.

The lines come from the whole subtitle track when it can be read up front: external
`srt` / `ass` / `ssa` / `vtt` files, and text tracks embedded in MKV (read once in the background and
cached). Other tracks (MP4 text, PGS / VobSub pictures, streams) fill in as they play, and the parts not
yet known are hatched.

### 4.3 Styled (ASS) subtitles

`subtitles.ass_override` (in the tab as four buttons) decides what happens to styled subtitles:

| Setting value | Button | Behaviour |
|---|---|---|
| `no` | Keep | Shown exactly as the author made them. |
| `scale` (default) | Scale | Author's styling, but your size applies. |
| `force` | Override | Dialogue in your font, size, and colours. Signs and positioned text are left alone. |
| `strip` | Strip | All styling and positioning removed: plain text at the bottom. |

Font, size (S / M / L / XL presets plus an exact size), bold, colours, outline, shadow, background
box, and position are in the same tab (`Alt+-` / `Alt+=` size, `Alt+Up` / `Alt+Down` position).

### 4.4 One-click season matching

**Auto-match folder** (Subtitles tab, or the `subtitles.automatch` action) pairs the subtitle files
in the current folder with its videos, even when the names differ:

- It looks next to each video and in subfolders named `subs`, `subtitles`, or `sub` (and
  `subs/<video name>/`), for `srt ass ssa vtt sub idx sup` files.
- Files are paired by name and by season / episode number (`S01E05`, `1x05`, `- 05`, …). Two
  different episode numbers never pair. A language suffix (`.en.srt`, `.eng.ass`, `_English`) keeps
  one subtitle per language.
- A card reports what matched, which videos and files were left over, and anything ambiguous (two
  candidates with near-equal scores). Ambiguous pairs are listed, never guessed.

Matches are remembered for each file (by content, so renaming or moving it keeps them) and attached
every time the file opens. mpv's own name matching (`subtitles.auto_load`, default `fuzzy`) stays on as
well.

---

## 5. Per-show and per-folder presets

A **preset** is a saved bundle of subtitle, audio, video, and playback settings plus a rule for which
files it applies to. Save one on the first episode and every later episode opens with it applied:
the right tracks (even when track order differs between files), the subtitle delay, the styling, and
the picture settings.

### 5.1 Saving from the player

1. Set things up on one file: tracks, delay, subtitle style, picture.
2. Press `Ctrl+S` (or **Save as…** in the preset strip at the bottom of the sidebar).
3. Give it a name, choose **Applies to**, and tick the sections to save (Audio, Subtitles, Video,
   Playback). Only settings that differ from your defaults are saved, and selected tracks are saved as
   rules (language plus a distinctive title word), not as track numbers.

**Applies to** offers `This series: "<name>"` (when the file name parses as an episode),
`This folder`, `This folder and subfolders`, and `Custom pattern` (a glob).

While a preset is applied, the strip shows its name. Change anything and a **modified** dot appears
with an **Update** button, which writes only what changed into the file. The ⋯ menu has
"Detach for this file", "Manage presets…" (Settings → Playback → Presets), and "Reveal preset file".
`playback.presets_enabled = false` stops automatic application; `--preset <ID>` forces one.

### 5.2 Which preset wins

Every enabled preset whose rule matches the file applies, **least specific first**, so a more specific
preset overrides a broader one **key by key**. Keys it does not set fall through.

| Specificity | `type` | Matches when |
|---|---|---|
| 1 (lowest) | `glob` | the pattern matches the full path (case-insensitive). The only type that matches URLs. |
| 2 | `series` | the file's series key equals `series` |
| 3 | `folder_recursive` | the file is anywhere under `path` |
| 4 (highest) | `folder` | the file's own folder is `path` |

Within one level, higher `priority` applies later (wins). Ties are broken by `id`.
There is no "all files" type. A preset for everything is `type = "glob"` with `glob = "**"`.

The full order on every load is: your settings → matching presets → what you last used on this file
(per-file memory) → command-line options.

**Series key.** The file name is lowercased, bracketed tags (`[Group]`, `(1080p)`) and release tags
(`2160p`, `x265`, `web-dl`, …) are removed, and the text before the episode marker is the series:
`[SubsPlease] Sousou no Frieren - 05 (1080p).mkv` → `sousou no frieren`;
`Severance.S02E03.2160p.WEB-DL.mkv` → `severance`. A file with no episode marker (a film) uses its
folder name.

### 5.3 Editing `presets/<id>.toml` by hand

Presets live one per file in `<config>/presets/`. The folder is watched: saving a change in a text
editor re-applies it to the playing file. A file with a mistake is skipped, with one message naming
the file and line, and the other presets keep working. Only keys you write are applied.

```toml
schema   = 1
id       = "frieren"              # must equal the file name (frieren.toml); a-z, 0-9, -
name     = "Frieren"              # shown in the preset strip
enabled  = true                   # optional, default true
priority = 0                      # optional; breaks ties between equally specific presets

[match]
type   = "series"                 # "series" | "folder" | "folder_recursive" | "glob"
series = "sousou no frieren"      # for type = "series"
# path = "D:/Anime/Frieren"       # for "folder" / "folder_recursive" (forward slashes, ~ allowed)
# glob = "**/Frieren*/**/*.mkv"   # for "glob": full path or URL, case-insensitive

[audio]
# A track rule: which track to pick, whatever its number in this file.
track     = { lang = ["jpn", "ja"] }
delay_ms  = 0
equalizer = "dialogue"            # an equalizer preset id, or "off"
normalize = false

[subtitles]
# Rules are tried in order:
#   1. lang + title_contains + channels/forced (those that are set)
#   2. lang only
#   3. id (the track number), for files whose tracks have no language
# Nothing matching leaves mpv's own choice. track = "off" turns subtitles off.
track    = { lang = ["eng", "en"], title_contains = "full", forced = false, prefer_external = true }
visible  = true
delay_ms = -350

[subtitles.style]                 # any of these
font         = "Gandhi Sans"
size         = 46
scale        = 1.0
bold         = false
color        = "#FFFFFF"
outline_color = "#000000"
outline_size = 3.0
shadow_offset = 0
background   = "none"             # "none" | "box"
position     = 96                 # 100 = bottom
margin_y     = 30
ass_override = "scale"            # "no" | "scale" | "force" | "strip"

[video]                           # any of these
brightness   = 0
contrast     = 4
saturation   = 6
gamma        = 0
hue          = 0
rotate       = 0                  # 0, 90, 180, 270
flip_h       = false
flip_v       = false
aspect       = "auto"             # e.g. "16:9", "2.39:1"
tone_mapping = "auto"
deband       = true
interpolation = false
shaders      = ["Anime4K_Clamp_Highlights.glsl"]   # file names in <config>/shaders/

[playback]
speed = 1.0
```

Track rule fields: `lang` (list of ISO 639-1/2 codes, any matches), `title_contains` (text in the
track title, case-insensitive), `channels` (audio layout such as `"5.1"`), `forced` (subtitles),
`prefer_external` (rank subtitle files above embedded tracks), and `id` (last resort).

---

## 6. Audio, equalizer, and gapless playback

The Audio tab (`Alt+2`) has the track, output device, preferred languages, audio delay
(`Ctrl+-` / `Ctrl+=`), volume normalisation, and the equalizer. `A` cycles tracks and `Shift+A`
cycles output devices.

### 6.1 Equalizer

`E` toggles the equalizer. Two modes:

- **10-band**: fixed bands at 31, 62, 125, 250, 500 Hz, 1, 2, 4, 8, 16 kHz. Drag a node up or down.
- **Parametric**: up to 8 freely placed nodes. Double-click empty space to add one, drag it in
  frequency and gain, use the wheel over a node for its width (Q), and right-click for its type
  (Peak, Low shelf, High shelf) or "Delete node".

The curve shows the true combined response. **Preamp** lowers the overall level to leave headroom
for boosts. Gain changes apply instantly without a gap.

**Presets.** Built-in: Flat, Bass, Treble, Dialogue, Night, Loudness, Rock, Classical, Electronic.
**Save** (the dashed chip) stores the current curve under a name in `<config>/eq-presets.toml`, and
custom presets can be deleted from their chip. The file can also be edited by hand:

```toml
schema = 1

[[preset]]
id     = "dialogue"                         # same id as a built-in replaces it
name   = "Dialogue"
mode   = "graphic10"
preamp = -2.0
gains  = [-2, -2, -1, 0, 2, 4, 4, 2, 0, -1]   # dB at 31 … 16k Hz

[[preset]]
id     = "my-headphones"
name   = "HD 600"
mode   = "parametric"
preamp = -4.5
bands  = [
  { type = "lowshelf",  freq = 105,   gain = 5.5,  q = 0.70 },
  { type = "peak",      freq = 3300,  gain = -2.0, q = 2.00 },
  { type = "highshelf", freq = 10000, gain = 1.5,  q = 0.70 },
]
```

### 6.2 Loudness

- **Volume normalisation** (`audio.normalize`, the Audio tab switch): evens out loud and quiet
  moments as it plays. Good for films at night.
- **ReplayGain** (`audio.replaygain`: `off`, `track`, `album`): uses the loudness tags in music files.
- **Maximum volume** (`audio.max_volume`, default 130 %): above 100 % the volume bar turns red and
  quiet audio is boosted (it can distort).

### 6.3 Gapless music

When a music file is followed by another music file in the queue, vihpluh hands the next track to mpv
ahead of time so one runs straight into the next, with no gap or click between tracks of a
continuous album. `audio.gapless`: `weak` (default; gapless when the formats match), `yes`, or `no`.
It does not apply to video, streams, repeat-one, "stop after this item", or a sleep timer that ends
with the file. Albums opened from the library queue in disc and track order.

---

## 7. Video adjustments, HDR, and GLSL shaders

The Video tab (`Alt+3`) changes the picture live: brightness, contrast, saturation, gamma, hue;
aspect ratio (`X` cycles, `C` crops to fill); rotate (`Ctrl+R`), mirror, flip; zoom (`=` / `-`,
`Ctrl+wheel`) with Alt-drag to pan; debanding and motion interpolation. `Shift+Backspace` resets
zoom, pan, rotation, and flip. Rotate and flip keep hardware decoding.

### 7.1 HDR

| Setting | Default | What it does |
|---|---|---|
| `video.hdr_passthrough` | `auto` | Windows only: sends HDR untouched to a display with Windows HDR turned on. |
| `video.tone_mapping` | `auto` | Fits HDR to an SDR display. The Video tab offers the curves. |
| `video.target_peak` | `"auto"` | Your display's peak brightness in nits, if tone-mapped HDR looks too dark or too bright. |
| `video.gpu_api` | `auto` | Windows: Direct3D 11, the most reliable choice for HDR. |

A chip in the Video tab says whether the picture is passed through or tone-mapped. On Linux HDR is
always tone-mapped.

> HDR passthrough with an HDR display has not yet been verified (no HDR display on the test machine).

### 7.2 Custom shaders

1. In the Video tab, click **Open folder** next to Shaders. This opens `<config>/shaders/`
   (Windows: `%APPDATA%\vihpluh\shaders\`, Linux: `~/.config/vihpluh/shaders/`) and creates it if needed.
2. Drop `.glsl` files there (mpv user shaders, for example FSRCNNX or Anime4K).
3. They are listed by name in natural order the next time the Video tab opens (or right after
   **Open folder**). Tick a shader to run it and untick to stop. Changes apply live, in the order
   ticked.

Shaders can be part of a preset (`shaders = ["name.glsl"]` in `[video]`, by file name). For a shader
on every file, add it to `[mpv.options]` in `config.toml` instead, with its full path:
`glsl-shaders = "C:/Users/you/AppData/Roaming/vihpluh/shaders/FSRCNNX_x2_8-0-4-1.glsl"`.
(In mpv, `~~/` means mpv's own config folder, not vihpluh's.)

---

## 8. Library, playlists, and IPTV streams

`Ctrl+L` opens the library drawer. It never interrupts playback: browse, search (`Ctrl+F` or `/`),
and enqueue while something plays. In the drawer, `Enter` plays now, `Shift+Enter` plays next,
`Ctrl+Enter` adds to the queue, and `Backspace` goes up a level. `Escape` or a click outside closes it.

Sections: **Videos** (All, Series grouped by show and season, Folders), **Music** (Albums, Artists,
Tracks), **Streams**, **Recent**, **Folders**, **Playlists**.

### 8.1 Indexed folders vs pinned folders

- **Library folders** (Add folder… → Videos / Music / Videos and music, or Settings → Library) are
  scanned and indexed: titles, series and episodes from file names, music tags, durations. They are
  watched while vihpluh runs (`library.watch_folders`) and checked at startup
  (`library.scan_on_startup`), so new, renamed, and deleted files show up without a rescan.
  `library.ignore_patterns` skips hidden files and samples by default.
- **Pinned folders** are shortcuts browsed live from disk ("Pin this folder" while browsing,
  or "Pin" on a folder row). They are not indexed, so pinning a huge or slow drive costs nothing.

### 8.2 Playlists

Playlists are named, saved lists, kept as `<config>/playlists/<id>.json`.

- **Save queue as playlist** (queue header), **Add to playlist** / **New playlist…** on any library row.
- In a playlist: rename, move rows up / down, remove rows, delete the playlist.
- **Import M3U…** reads an `.m3u` / `.m3u8`. **Export as M3U8…** writes one (you type the file path).
  Sources and titles round-trip.
- **Moved files are found again.** Each item remembers its file's `media_key`, a hash of the
  file's size and contents rather than its path. If a file has moved or been renamed when the playlist
  opens, the library is searched for the same content and the item points there.

### 8.3 IPTV and stream lists

Streams → **Add M3U…** → *From a file…* or *From an address…*. The list is read once and its channels
are shown by group. **Refresh** (row menu) reads it again and replaces the channels. Nothing
refreshes by itself. Lists fetched from an address use your `network.user_agent`, `network.proxy`,
and `network.timeout_s`.

Supported in lists:

```
#EXTM3U
#EXTINF:-1 tvg-id="news.one" tvg-name="News One HD" tvg-logo="http://…/news.png" group-title="News",News One
#EXTVLCOPT:http-user-agent=Mozilla/5.0 (X11; Linux x86_64)
#EXTVLCOPT:http-referrer=http://portal.example/
http://example.com/news1.m3u8
#EXTINF:-1,Music Channel
#EXTGRP:Music
http://example.com/music.ts
```

`#EXTVLCOPT:http-user-agent=…` and `http-referrer=…` are sent with that channel only. `group-title`
or `#EXTGRP` sets the group. Channels appear in the queue, the top bar, and the OS media controls
under the name the list gives them. Channel logos are not downloaded (`network.load_remote_artwork`
is off by default, and logos are not displayed in this version).

`Ctrl+N` (Open URL) plays any single address. Streams buffer `network.cache_secs` ahead and reconnect
when dropped (`network.auto_reconnect`). `network.hls_bitrate` picks the quality of adaptive streams.

---

## 9. Customization and keybindings

### 9.1 Settings

`Ctrl+,` opens Settings: ten categories (General, Interface, Controls, Playback, Video, Audio,
Subtitles, Library, Network, Performance), searchable by label, description, or dotted key (pasting
`subtitles.font_size` finds it). Every row has an ⓘ explanation and a reset, and changed rows are
marked. Settings with ⟳ apply after a restart. `config.toml` holds only what you changed, keeps your
comments, and is watched: edits in a text editor apply live.

Interface highlights: `interface.theme` (`dark`, `light`, `system`), `interface.accent` (any colour;
a very pale or very dark one is adjusted just enough to stay visible), `interface.ui_scale`,
`interface.window_decorations` (`custom` or `native`), `interface.seek_waveform`.

Power users can pass raw mpv options in `config.toml` (unsupported, applied last):

```toml
[mpv]
load_user_config = false     # true also reads your own mpv.conf (never input.conf)

[mpv.options]
scale = "ewa_lanczossharp"
```

### 9.2 Base keymaps

Settings → Controls → **Keymap**, or `base` in `keybinds.toml`:

| Base | Defaults |
|---|---|
| `vihpluh` | The default: YouTube / mpv habits for transport, VLC letters for tracks and sync. |
| `vlc` | VLC's stock bindings mapped onto vihpluh's actions. |
| `mpv` | mpv's stock bindings mapped onto vihpluh's actions. |
| `none` | Nothing bound. Build your own. |

Your own changes are kept and re-applied on top when you switch.

### 9.3 The hotkey editor

Settings → Controls lists every action, grouped, with its keys:

- **+** captures a new key (press it; a mouse button or wheel notch over the box works too).
  `Escape` cancels.
- Click a key to replace it, hover and ✕ (or `Backspace`) to remove it, ⟳ to reset a row.
- If the key is already used, a bar offers **Replace**, **Keep both off**, or **Cancel**. Nothing
  changes until you choose. `Tab` / `Shift+Tab` are reserved.
- **Find by key**: press a combination to see what it does.
- Every change is saved to `keybinds.toml` at once and shows in menus and tooltips immediately.

Keys name physical positions on a US layout, so bindings stay put when you switch keyboard layout.
The editor shows your layout's own characters.

### 9.4 `keybinds.toml` and custom bindings

```toml
schema = 1
base   = "vihpluh"

[bind]                                    # an action listed here replaces its default keys
"player.seek_back_medium"  = ["J", "Num4"]
"window.toggle_zen"        = ["Z"]        # drops the middle-click default
"player.seek_forward_large" = []          # unbound

[context.queue.bind]                      # only while the queue panel has focus
"queue.remove_selected"    = ["Delete", "Backspace"]

[[custom]]                                # parameterised actions, file only
keys   = ["Ctrl+Shift+Right"]
action = "player.seek_relative"
args   = { seconds = 85 }                 # skip an opening

[[custom]]
keys   = ["F9"]
action = "player.set_speed"
args   = { value = 1.75 }

[[custom]]
keys   = ["Ctrl+D"]
label  = "Toggle deinterlace"             # shown in the OSD and the shortcuts sheet
action = "mpv.command"
args   = { command = ["cycle", "deinterlace"] }   # passed to mpv verbatim
```

Notation: modifiers `Ctrl`, `Alt`, `Shift`, `Meta`, in that order, joined by `+`. Keys: `A`–`Z`,
`0`–`9`, `F1`–`F24`, `Space`, `Enter`, arrows (`Up`, …), `PageUp`, punctuation by its unshifted glyph,
numpad `Num0`–`Num9`, `NumAdd`, …; mouse `MouseLeft`, `MouseLeftDouble`, `MouseMiddle`, `MouseRight`,
`MouseBack`, `MouseForward`; wheel `WheelUp`, `WheelDown`, `WheelLeft`, `WheelRight`. Mouse and wheel
bindings work only over the video.

Actions available to `[[custom]]`: `player.seek_relative` `{ seconds, exact? }`,
`player.seek_absolute` `{ seconds }` or `{ percent }`, `player.set_speed` `{ value }`,
`player.set_volume` `{ value }`, `audio.set_delay` / `subtitles.set_delay` `{ ms }`,
`audio.set_equalizer_preset` `{ id }`, `video.set_aspect` `{ value }`, `presets.apply` `{ id }`,
`player.set_sleep_timer` `{ minutes }` or `{ kind = "off" | "end_of_file" | "end_of_queue" }`, and
`mpv.command` `{ command = [...] }`. Custom rows appear in the hotkey editor (they can be deleted
there) and in the `F1` sheet.

The shortcut badge (`interface.osd_shortcut_badge`) shows which key fired each action, which helps
when learning a keymap.

---

## 10. Troubleshooting

| Symptom | What to do |
|---|---|
| Video stutters or drops frames | `I` shows the decoder: it should say `(hardware)`. Settings → Video → Hardware decoding should be *Auto*. Try Scaling quality *Fast*. |
| Seek previews cause stutter | Set `performance.thumbnail_hwdec = "no"` or turn off `interface.seek_preview`. |
| HEVC shows no picture (Fedora) | Follow the RPM Fusion codec steps in the README, then `scripts/check-deps.sh --runtime`. |
| Blank or garbled interface (Linux, NVIDIA) | `performance.webkit_dmabuf = "off"`, or `video.linux_force_xwayland = true`. |
| Subtitle characters look garbled | Set `subtitles.encoding` to the file's code page. |
| A stream will not play | Check `network.user_agent` / `network.proxy`. Some lists need the per-channel `#EXTVLCOPT` lines. |
| Something else | Run with `--log-level debug` and read `vihpluh.log` (paths in [§1.3](#13-where-vihpluh-keeps-its-files)). When reporting a bug, include the log and what `I` shows. |
