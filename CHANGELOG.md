# Changelog

All notable changes to vihpluh are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — Unreleased

The first version. Built and exercised on Windows 11. The Linux build (Fedora, Wayland and X11) is
written up to its render backend, which waits on spike S2 on bare-metal Fedora. Open checks are
listed in [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md).

### Added

#### Playback core
- libmpv (≥ 0.40) in-process, behind a Rust core that owns all player state and mirrors mpv's
  properties to the interface through typed, coalesced patches (`player://patch`, 5 Hz `player://time`).
- Windows video surface: a native child window under a transparent WebView2, with D3D11VA hardware
  decoding (zero-copy) and automatic software fallback.
- Playback state machine with clear error states; unplayable and empty files show a message instead of hanging.
- Single instance: a second launch forwards its files to the open window (`general.open_behavior`).
- Command line: files, folders (natural order), `.m3u` / `.m3u8`, URLs; `--enqueue`, `--play-next`,
  `--start` (seconds, `mm:ss`, `hh:mm:ss`, `N%`), `--fullscreen`, `--pip`, `--zen`, `--paused`,
  `--no-resume`, `--no-session`, `--preset`, `--home`, `--log-level`, `--mpv-opt`, `--version`.
- OS media controls (SMTC) and media keys; display sleep held off while video plays.
- Screenshots with or without subtitles (PNG / JPG / WebP, mpv name template). Names that are already
  taken get a sequence number.

#### Interface
- Auto-hiding top and bottom bars, Zen mode (`Z`), fullscreen, always on top, responsive collapse down
  to 480 px, custom or native window decorations.
- Seek bar with buffer range, chapter gaps, bookmark ticks, hover thumbnails, an optional audio
  waveform, and a right-click menu (bookmark, A–B points, copy timestamp).
- Cone-shaped volume slider with a boost zone above 100 %.
- Context menu, ☰ menu, command palette (`Ctrl+Shift+P`), shortcuts sheet (`F1`), on-screen feedback
  with an optional shortcut badge, and toasts with undo.
- Dark and light themes, `system` following, any accent colour (with contrast guards for text, focus
  ring, and fills), interface scale, reduced motion.
- Idle screen with open buttons, Continue watching, and recent folders.
- Picture-in-picture (`Ctrl+P`): corner placement, aspect-locked resize, compact chrome, exact restore
  of the previous size, place, and mode.
- Windows taskbar thumbnail toolbar (previous / play-pause / next) and window size and place memory.

#### Queue and session
- Queue panel: drag reorder, multi-select, play next, remove with undo, OS file drop at a position,
  repeat (off / all / one), shuffle, stop after current, error skip with a three-strike stop.
- Opening one file queues the rest of its folder in natural order.
- Resume (`always` / `ask` / `never`) with "Start over", watched marking at `playback.finished_percent`,
  and per-file memory of tracks and delays.
- Session restore (queue, item, position; restored paused).

#### Sidebar
- **Subtitles:** track list, delay with `G` / `H` and line sync with `Ctrl+G` / `Ctrl+H`, a visual
  timeline (waveform plus cue lane; drag to shift, `Shift` to snap to speech onsets), styling (font,
  size presets, colours, outline, shadow, box, position), four styled-subtitle (ASS) modes, and one-click
  season auto-matching of external subtitle files.
- **Audio:** track and output device, preferred languages, delay, normalisation, and a 10-band and
  parametric equalizer with a live response curve, visual presets, and saved custom presets.
- **Video:** picture adjustments, aspect, crop to fill, rotate, mirror / flip (hardware decoding kept),
  zoom and pan, tone-mapping curves, HDR passthrough status, deband, interpolation, and user GLSL shaders
  from the `shaders` folder.
- **Playback:** speed, frame stepping, reverse, file and A–B loops, sleep timer with fade-out, and a
  stream inspector (codec, resolution, frame rate, bit depth, HDR, bitrates, decoder, dropped frames).
- **Bookmarks:** `Ctrl+B` quick add, rename, delete with undo; chapters and bookmarks on one list, with
  `PageUp` / `PageDown` across both.

#### Presets
- Per-show, per-folder, and pattern presets in `presets/<id>.toml`: tracks as rules (language, title,
  channels, forced, external), delays, subtitle style, picture, speed.
- Specificity order (glob → series → folder tree → folder) with priorities, layered key by key under
  per-file memory and the command line.
- Save from the player (`Ctrl+S`), modified indicator and Update, a presets manager, live reload of
  hand-edited files, and clear errors (file and line) for broken ones.

#### Media services
- Seek-preview thumbnails from a headless mpv handle (latest-wins worker, neighbour pre-generation,
  disk cache).
- Audio waveforms decoded headless through a pipe into a compact `.vwf` cache.
- Full subtitle cue index for external text subtitles and embedded MKV text tracks, progressive for
  the rest.
- Size-capped cache with a Clear button.
- Background media work waits for the first video frame of a newly opened file.

#### Library, playlists, streams
- Library drawer: Videos (all, series, folders), Music (albums, artists, tracks), Streams, Recent,
  Folders, Playlists; full-text search; play now / play next / enqueue without interrupting playback.
- Folder scanning with file-system watching, ignore patterns, series parsing from file names, music
  tags, and lazy durations. Pinned folders browsed live without indexing.
- Playlists as JSON, M3U8 import and export, and relocation of moved files by content hash (`media_key`).
- IPTV: `.m3u` stream lists from a file or an address, groups, per-channel user agent and referrer
  (`#EXTVLCOPT`), manual refresh, and channel names in the queue and the OS media controls.
- Gapless music playback through a two-entry mpv playlist window; albums queue in disc and track order.
- Open URL (`Ctrl+N`), with yt-dlp for web page links when installed.

#### Settings and keybindings
- Settings overlay: ten flat, searchable categories in plain English, generated from one schema, with
  per-row and per-page reset and restart markers. `config.toml` stores only changes and keeps comments.
- Hotkey editor: capture, conflict resolution, find by key, per-row and global reset; base keymaps
  `vihpluh`, `vlc`, `mpv`, `none`; `[[custom]]` parameterised bindings including raw mpv commands.
- Config migration scaffold with `.bak` backups.

#### Packaging and project
- Windows per-user NSIS installer with file associations, and a portable zip (`portable` marker keeps
  all data beside the executable).
- Fedora RPM configuration with runtime dependencies and desktop MIME types (not yet built).
- GitHub Actions CI (Windows and Fedora: fmt, clippy, tests, `pnpm check`, build) and a tag-driven
  draft release workflow (not yet run).
- `THIRD_PARTY_NOTICES.md` generated from the Rust and npm dependency trees.
- Documentation: architecture, design system, keybinds and configuration, implementation plan, user
  guide, contributing guide, security policy.

### Known limitations
- Linux: the video render backend is a placeholder until spike S2. Nothing has been run on Fedora.
- Performance budgets missed on Windows (ARCHITECTURE §14): first frame under load (0.9–1.8 s vs 0.9 s),
  memory while playing 4K (~740 MB vs 450 MB, mostly the webview), and background connections made by the
  WebView2 runtime itself.
- Snap Layouts are not offered on the custom maximize button (use native decorations).
- HDR passthrough with an HDR display, multi-monitor behaviour, and screen-reader use are not yet verified.

[0.1.0]: https://github.com/ChickenWings-0/vihpluh/releases/tag/v0.1.0
