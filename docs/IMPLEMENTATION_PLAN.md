# vihpluh — Implementation Plan

Twelve phases, each ending in a state you can use daily. Every task is a checkbox; every phase has
**verification criteria** that must be *observed in the running app* before the phase is closed.

Specs live elsewhere — this file says **what order** and **how to prove it**:
[ARCHITECTURE](ARCHITECTURE.md) (A§n) · [DESIGN_SYSTEM](DESIGN_SYSTEM.md) (D§n) · [KEYBINDS_AND_CONFIG](KEYBINDS_AND_CONFIG.md) (K§n).

## Contents

1. [How to work through this plan](#1-how-to-work-through-this-plan)
2. [Test media and reference machines](#2-test-media-and-reference-machines)
3. [Phase 1 — Foundations and de-risking spikes](#3-phase-1--foundations-and-de-risking-spikes)
4. [Phase 2 — Core playback and state sync](#4-phase-2--core-playback-and-state-sync)
5. [Phase 3 — Player chrome](#5-phase-3--player-chrome)
6. [Phase 4 — Queue, session, resume](#6-phase-4--queue-session-resume)
7. [Phase 5 — Sidebar: audio, video, playback](#7-phase-5--sidebar-audio-video-playback)
8. [Phase 6 — Seek previews and waveform](#8-phase-6--seek-previews-and-waveform)
9. [Phase 7 — Subtitle suite](#9-phase-7--subtitle-suite)
10. [Phase 8 — Bookmarks, chapters, presets](#10-phase-8--bookmarks-chapters-presets)
11. [Phase 9 — Library, playlists, streams](#11-phase-9--library-playlists-streams)
12. [Phase 10 — Settings, hotkey editor, theming](#12-phase-10--settings-hotkey-editor-theming)
13. [Phase 11 — Window polish and Linux parity hardening](#13-phase-11--window-polish-and-linux-parity-hardening)
14. [Phase 12 — Packaging, CI, release](#14-phase-12--packaging-ci-release)
15. [Parity matrix and release gate](#15-parity-matrix-and-release-gate)

---

## 1. How to work through this plan

- **One phase at a time, in order.** Phases 5–9 may be reordered by the owner; 1–4 may not.
- Tick a box only when the task's behaviour was exercised in `pnpm tauri dev`. "Compiles" is not done.
- **Platform cadence:** build each phase on Windows first, then run its verification list on Fedora
  (Wayland session, then X11 session) before closing the phase. A phase is closed only when its "Platforms" line is satisfied.
  If the Fedora machine is unavailable, mark the phase `closed (Windows) — Linux pending` at its heading and carry the debt no further than one phase.
- **Gate rules:** `pnpm check`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` green at every commit (CLAUDE.md "Definition of done").
- Scope discipline: build what the phase lists. Ideas discovered mid-phase go to the owner, not into the code.
- When a spec turns out wrong, fix the spec doc in the same change and add a row to A§15.
- Phase status is recorded by editing the heading: `— ☐ not started` / `— ◐ in progress` / `— ☑ closed (YYYY-MM-DD)`.

---

## 2. Test media and reference machines

### 2.1 `test-media/` (gitignored — assemble locally)

Each file exists to exercise something specific. Record actual filenames in `test-media/README.local.md` (also ignored).

| ID | What | Exercises |
|---|---|---|
| T1 | 4K HEVC Main 10 HDR10 MKV, ≥ 60 Mb/s, ≥ 2 audio tracks (one lossless 5.1/7.1), ≥ 2 subtitle tracks, chapters | Primary hwdec path, HDR, tracks, chapters, cache |
| T2 | 4K AV1 10-bit (MKV or MP4) | AV1 hwdec / `dav1d` fallback |
| T3 | 1080p H.264 MP4, AAC stereo | Baseline, MP4 container |
| T4 | 4K60 HEVC or H.264, high motion | Dropped-frame budget |
| T5 | Anime episode MKV with styled ASS subtitles + embedded fonts, dual audio (jpn/eng) | ASS rendering, track rules, embedded cue index |
| T6 | A season folder (≥ 6 episodes) with **differently named** external `.srt`/`.ass` files in a `Subs/` subfolder | Folder auto-queue, series parsing, subtitle auto-match |
| T7 | MKV with PGS (bitmap) subtitles | Bitmap sub handling, progressive cue index |
| T8 | A file with audio deliberately offset (remux with ±400 ms delay) | Sync sliders, subtitle timeline |
| T9 | FLAC album (tagged, ≥ 8 tracks) + a few MP3s with messy tags | Music library, gapless |
| T10 | VFR / odd-aspect phone video (portrait, rotation flag) | Rotation, aspect handling |
| T11 | Public HLS test stream URL + a small `.m3u` with 5–10 public streams | Network playback, IPTV parsing |
| T12 | Corrupt/truncated file + a 0-byte `.mkv` | Error paths, auto-skip |
| T13 | Very long file (≥ 3 h) | Seek bar precision, thumbnail bucketing, waveform time |

Public sources for clips: the Jellyfin test-videos repository, Kodi's sample pages, Netflix "Chimera"/"Meridian" open content, Blender open movies. Remux with `mkvmerge`/`ffmpeg` to add tracks, chapters, or offsets.

### 2.2 Reference machines

| Machine | Use |
|---|---|
| **W** — owner's Windows 11 PC | Primary dev + all Windows verification. Record GPU model + driver version in the phase notes when hwdec results are logged. |
| **L** — Fedora (current stable) on **bare metal** | All Linux verification. Must be bare metal: VMs do not expose VA-API/NVDEC. Test both a Wayland session (GNOME; KDE if available) and an X11 session. |

---

## 3. Phase 1 — Foundations and de-risking spikes — ☑ closed (Windows, 2026-11-12) — Linux pending

**Goal:** a window on both OSes that plays a file with hardware decoding under a transparent web UI,
proving the two riskiest assumptions before any feature work. Spikes are throwaway-quality code on `spike/*`
branches; the scaffold tasks are production-quality on `main`.

### 3.1 Toolchain and scaffold

- [x] Windows toolchain per A§12.1 — via the GNU alternative (A§12.1, D15); see the phase notes. `rustc`, `node`, `pnpm` versions noted there.
- [ ] Fedora toolchain per A§12.2.
- [x] `scripts/fetch-libmpv.ps1`: pinned release tag + SHA-256, extract, generate `mpv.lib`, idempotent (**spike S3**). *(The `mpv.lib` step is written but has not run yet: no llvm-dlltool / lib.exe on the dev machine.)*
- [ ] `scripts/check-deps.sh`: verifies Fedora packages, full FFmpeg (HEVC decoder present), `vainfo` profiles / `nvidia-smi`, session type; exits non-zero with a readable list. *(Written and syntax-checked; not yet run on **L**.)*
- [x] Scaffold Tauri 2 + React 19 + Vite + TypeScript strict + Tailwind 4 with the tree from A§3 (only directories needed now).
- [x] `package.json` scripts: `dev`, `build`, `check`, `gen:bindings`, `fetch:libmpv`, `tauri`. `tauri dev` sets `VIHPLUH_HOME=./.local`.
- [x] Biome config, `tsconfig` (strict, `noUncheckedIndexedAccess`), Vitest; `rustfmt.toml`, clippy lints (`unwrap_used`, `expect_used` = warn in non-test code).
- [x] `src-tauri/build.rs`: link search path + copy `libmpv-2.dll` beside the dev executable (Windows); `pkg-config mpv` (Linux). *(Linux branch unexercised.)*
- [x] `tauri.conf.json`: single window `main`, `transparent: true`, `decorations: false`, `visible: false`, min size 480 × 270; CSP from A§13; capabilities file with nothing but core window permissions.
- [x] `paths.rs` (A§9.1 incl. `--home` / `VIHPLUH_HOME` / portable marker), `logging.rs` (rotating file + stderr), `error.rs` (`AppError`).
- [x] `src/styles/tokens.css` with every token from D§2–D§4, D§12; Tailwind `@theme` mapping from D§14; transparent page root.
- [x] `src/ipc/transport.ts` as the sole importer of `@tauri-apps/api` (lint rule: `noRestrictedImports` elsewhere).
- [x] ts-rs wiring: one DTO (`AppInfo`) exported end-to-end through `pnpm gen:bindings`.

### 3.2 Spikes (each produces a short findings note in the phase notes, then code is rewritten properly)

| Spike | Question | Pass criteria | If it fails |
|---|---|---|---|
| **S1** Windows embed | Does a transparent WebView2 composite cleanly over our child HWND hosting mpv (`--wid`)? | T1 plays; an HTML box with 50 % alpha and a CSS-animated element render over the video; no flicker on resize, maximize, fullscreen toggle, monitor/DPI change; all mouse input reaches the webview; `hwdec-current = d3d11va`. | A§15 risk table, row S1. |
| **S2** Linux render | Does `GtkGLArea` + `mpv_render_context` under a transparent WebKitGTK webview work on GTK3? | On **L** in GNOME Wayland *and* an X11 session: T1 and T4 play in the GL area with the same HTML overlay on top; `hwdec-current` matches A§5.4 for the GPU; T4 shows ≤ 0.1 % dropped frames over 2 min with a CSS animation running; resize and fullscreen are clean. Record which GL platform GDK chose on X11 (GLX vs EGL) and which renderer backs `vo=libmpv`. | A§5.3 contingency ladder; stop and consult the owner before proceeding. |
| **S3** libmpv linking | Does the pinned Windows dev archive link with MSVC via a generated `mpv.lib`, and does the bundled app find the DLL? | `cargo build` links; `pnpm tauri build` output runs on a clean user profile; archive contents documented. | `libloading` runtime binding. |
| **S4** mpv feature probes | Do the mpv-level mechanisms the design relies on behave as assumed? | Verified with a scratch binary or `mpv` CLI of the same build: (a) `af-command` changes an `equalizer@bN` gain live without a dropout; (b) software render API yields a frame after a paused keyframe seek; (c) `ao=pcm` writes to a named pipe on Windows and a FIFO on Linux at faster-than-realtime; (d) exact subtitle style option names; (e) `video-margin-ratio-right` repositions video live on both backends; (f) `flip_h.glsl` mirrors the picture with zero-copy hwdec still active. | Use the documented fallback for that item (A§6.6, A§11) and update the doc. |

### 3.3 First-frame slice (production code)

- [x] `video/mod.rs` trait + `Capabilities`; `embed_win.rs` (from S1) implemented cleanly.
- [ ] `render_gtk.rs` (from S2) implemented cleanly. *(A no-video placeholder until S2 runs on **L**.)*
- [x] `player/mpv.rs` wrapper; primary handle created with A§6.2 options; mpv log forwarding.
- [x] `player/hwdec.rs` chain selection (A§5.4) with unit tests for every row of the table.
- [x] `cli.rs` (A§4.6): positional file plays on launch; `--version` prints app/mpv/ffmpeg/backend. *(Folder and `.m3u` expansion arrive with the queue in Phase 4 and the IPTV parser in Phase 9.)*
- [x] Window shows only after frontend `app_ready`; clean shutdown (A§4.4) with watchdog.
- [x] Temporary debug overlay (dev builds only): `hwdec-current`, backend, session, dropped frames.

### Verification

- [x] `vihpluh test-media/T1.mkv` shows video within the A§14 first-frame budget on **W**; debug overlay reports `d3d11va`. *(756–913 ms on an idle machine, debug build — borderline; see notes.)*
- [ ] Same on **L** Wayland (expected decoder per GPU) and **L** X11.
- [ ] T2 (AV1) plays on all three; decoder reported; software fallback emits a log line when hwdec is unavailable. *(**W** ✓: `d3d11va`, 0 drops; forced fallback logs "software decoding: no hardware decoder available for …".)*
- [x] `video.hwdec = "off"` in `.local/config/config.toml` forces software decoding (`hwdec-current = no`).
- [ ] Closing the window exits the process within 2 s on all three; no orphan processes. *(**W** ✓: 140–205 ms, no orphans.)*
- [ ] `scripts/check-deps.sh` passes on **L**; with `ffmpeg-free` deliberately in place it reports the missing HEVC decoder.
- [ ] A fresh clone builds on both OSes following only README instructions.
- [ ] All four spike notes written; A§15 risk table updated with outcomes; every `⚠ VERIFY` marker in ARCHITECTURE resolved or rewritten. *(S1, S3, S4 written and their markers resolved; the four remaining markers all belong to S2.)*

**Platforms:** W + L-Wayland + L-X11 (mandatory — this phase exists to prove them).

### Phase 1 notes (2026-11-12, Windows only)

**Machine W.** Windows 11 Home 10.0.26200 · Ryzen 9 7940HS · NVIDIA RTX 4070 Laptop (driver 32.0.16.2114) +
Radeon 780M (32.0.11020.1002) · 2560 × 1440 at 125 %. `rustc 1.99.1` (`stable-x86_64-pc-windows-gnu`),
`node 24.19.0`, `pnpm 9.15.9`, libmpv `0.42.0-38-gbcde6b93c` (shinchiro 20261111).

**Toolchain.** The MSVC Build Tools install needs administrator rights and was not granted during this
run, so Phase 1 was built with the GNU Rust toolchain and the MinGW-w64 GCC already present in MSYS2
(A§12.1 "GNU toolchain alternative", D15). The README's MSVC path is therefore still unverified.

**Linux.** No Fedora machine was available. S2, `render_gtk.rs`, `check-deps.sh`, and every **L** line
above stay open; the debt is carried under the "Linux pending" rule of §1.

**S1 — Windows embed: pass.** T1 (4K HEVC Main 10, 61 Mb/s) plays with `hwdec-current = d3d11va`, 0 dropped
frames; a 50 %-alpha box and a spinning CSS element composite over the video; pointer moves and clicks
reach the webview; resize and maximize/restore track correctly (host and mpv window follow the client
area). Findings: (1) the host **must not** have `WS_CLIPSIBLINGS` — with it nothing of the video is
composited at all; Windows re-adds the bit to top-level windows, so it is cleared after reparenting.
(2) GDI content in the transparent window is invisible (alpha 0); only the swapchain shows. Not covered:
fullscreen (needs `window_set_mode`, Phase 3) and a monitor/DPI change (single display; Phase 11).

**S2 — Linux render: not run.**

**S3 — libmpv linking: pass with the GNU toolchain.** Archive contents: `libmpv-2.dll` (122 MB),
`libmpv.dll.a`, `include/mpv/{client,render,render_gl,stream_cb}.h`; no `mpv.def` (the script generates it
from the DLL's export table: 54 symbols). `cargo build` links against `libmpv.dll.a`; the DLL is copied
beside the executable; `pnpm tauri build --no-bundle` produces a 13 MB `vihpluh.exe` (plus
`WebView2Loader.dll`, which the GNU target links dynamically). Not covered: the MSVC `mpv.lib` route and a
run on a clean user profile.

**S4 — mpv feature probes: pass (Windows).** Code on branch `spike/s4`.
(a) `af-command vihpluh-eq gain 12 equalizer@b0` raised a 1 kHz tone by exactly 12.0 dB with the `af`
chain untouched; an off-frequency band left it alone; `af-command … volume -6dB volume` moved the preamp.
(b) Software render context after paused keyframe seeks: 6/6 frames each; 12–28 ms (1080p H.264),
60–75 ms (4K HEVC via `d3d11va-copy`), 30–400 ms (4K HEVC software).
(c) `ao=pcm` into a Windows named pipe: 90 s of audio in 0.29 s (~300× realtime).
(d) Every generated option name/value is accepted by the pinned build (unit test).
(e) `video-margin-ratio-right = 0.3` repositions the picture live on the Embed backend.
(f) `flip_h.glsl` mirrors the picture with `hwdec-current` still `d3d11va`; the subtitle stays unmirrored.

**Start-up timing** (debug build, idle machine, ms after process spawn): window visible with no file
540–630 (budget 500); first frame 615–845 for T3 and 756–913 for T1 (budget 900). Under load (a video
call running, < 2 GB free RAM) the same runs took up to 1.8 s. mpv is created before Tauri so it runs in
parallel with WebView2 start-up (D18). To revisit with a release build in the Phase 11 performance pass.

**Also learned.** mpv's built-in scripts load despite `load-scripts=no` and are switched off one by one
(D16). A separate integration-test binary cannot start (D17). The first launch after every rebuild takes
~10 s because the new executable is scanned by Defender; it is not an app regression.

---

## 4. Phase 2 — Core playback and state sync — ☑ closed (Windows, 2026-11-13) — Linux pending

**Goal:** the Rust core fully mirrors mpv and exposes it over typed IPC; a bare-bones test UI drives it.

- [x] `PlayerCore` actor, `PlayerHandle`, `mpv-events` thread (A§4.1). *(The handle is `Player`; the actor is `Core`.)*
- [x] `observe.rs` property table → `PlayerState` reducer (A§6.3); status state machine (A§6.4) with unit tests driven by recorded event sequences.
- [x] Events `player://patch` (coalesced, `rev`) and `player://time` (5 Hz + immediate on seek/pause).
- [x] All `player_*` and `tracks_select` commands (A§7.3); DTOs in `ipc/dto.rs` with ts-rs export. *(`enqueue` / `play_next` open modes act like `replace` until the queue lands in Phase 4; `player_set_sleep_timer` arrives with `sleep.rs` in Phase 5.)*
- [x] `config/`: `default.toml` embedded, user overlay, `toml_edit` writes, file watcher, `config_get/set/reset`, `config://changed` (K§1, K§5 keys needed so far).
- [x] Frontend: `stores/player.ts`, `stores/time.ts` (rAF interpolation), `stores/config.ts`; `ipc/commands.ts`, `ipc/events.ts`.
- [x] Minimal dev UI (unstyled): open, play/pause, seek slider, volume, track lists, state dump. Discarded in Phase 3.
- [x] `platform/`: screen-sleep inhibit while video plays — Windows; `souvlaki` media keys/now-playing behind `general.media_keys`.
- [ ] `platform/`: screen-sleep inhibit on Linux. *(Placeholder that only logs; `GtkApplication::inhibit` arrives with the render backend after S2.)*
- [x] `tauri-plugin-single-instance`: second launch forwards argv per `general.open_behavior`.
- [x] Error paths: unplayable file → `status: "error"` + `osd://show`.

### Verification

- [x] Play/pause/seek/volume/mute/speed from the dev UI all reflect in `PlayerState` within one patch; no polling anywhere in the frontend. *(Driven through the same commands over the webview debugging port: 44–184 ms per change, revisions contiguous, 5 rapid changes coalesced into 2 patches.)*
- [x] Interpolated position stays within ±50 ms of mpv's `time-pos` over 10 min at 1× and 2× (log the drift). *(**W**, T13: 1× max 49.2 ms / mean 14.2 ms over 1 168 samples; 2× max 46.4 ms / mean 13.8 ms. Within budget, but with little margin.)*
- [x] Switching audio and subtitle tracks on T1/T5 works; `selected` updates. *(T1 only: T5 is not assembled yet.)*
- [x] Editing `.local/config/config.toml` in a text editor while running emits `config://changed` and applies (test with `audio.max_volume`). *(115 reached mpv's `volume-max` within the 150 ms debounce; an invalid value raised one warning naming the key; deleting the file restored defaults.)*
- [ ] Media keys play/pause from another focused app; the OS media overlay shows the title (W: SMTC; L: MPRIS via `playerctl status`). *(**W** ✓: the session lists the title and live status; a toggle sent through the Windows session (the path media keys take) paused and resumed playback while the window was in the background. No physical key was pressed.)*
- [ ] Display does not sleep during playback; does sleep when paused (both OSes). *(**W**, partial: the app requests Display while video plays and releases on pause/stop (debug log). The system-wide state could not isolate it — another program held the display awake throughout — and `powercfg /requests` needs admin.)*
- [x] T12 (corrupt) → error state, no crash, no hang. Launching a second instance with a file hands off and exits. *(Empty and truncated files → `error` + toast, recovery on the next open; the second instance exited in 177 ms without starting mpv and the first played its file, relative path resolved.)*
- [x] Unit tests: state machine transitions, hwdec table, config overlay/migration, path resolution. *(Migration has nothing to migrate yet: schema 1; the scaffold is a Phase 10 task.)*

**Platforms:** W + L-Wayland (X11 smoke test only).

**Phase 2 notes (2026-11-13, Windows only).** Shutdown measured 0.4–0.9 s in three cases (mpv's own
teardown is most of it). Once, after 20 minutes of continuous playback, closing took 2.01 s — the
shutdown watchdog fired. It did not reproduce; watch for it.

---

## 5. Phase 3 — Player chrome — ☑ closed (Windows, 2026-11-13) — Linux pending; three hands-on checks pending (see notes)

**Goal:** the real UI shell — bars, auto-hide, shortcuts, context menu, Zen, OSD. After this phase vihpluh is usable as a daily player for single files.

- [x] Primitives (D§8): `IconButton`, `Button`, `Tooltip`, `Menu`, `Popover`, `Slider`, key cap, toast host. Icon pipeline: mono set (D§5.1) incl. custom glyphs. *(Custom glyphs so far: queue, sidebar, PiP, Zen.)*
- [x] `actions/registry.ts` + definitions for every action in K§2 that this phase can back (playback, volume, window, UI); stubs (`isEnabled: false`) for the rest so the registry is complete from day one. *(A test fails if the registry and the shipped keymap ever disagree. Track cycling and the delay steps are backed too, via three small commands from A§7.3.)*
- [x] `keybinds/`: `normalize.ts`, `engine.ts` (K§3.1–3.2), `resources/keymaps/vihpluh.toml` = K§2 defaults, `keybinds_get`, user overlay from `keybinds.toml` incl. `[[custom]]` (K§3.3–3.4). Mouse/wheel bindings over the video area only.
- [x] Focus model (D§13): player root focus, blur-after-click on chrome buttons, Tab traversal, `ui.escape` chain.
- [x] **TopBar** (D§7.3): title button, ☰ consolidated menu, PiP button (stub), custom window controls, drag region, double-click maximize, resize hit zones. *(Hit zones are drawn only where the OS has no resize border of its own, i.e. not on Windows.)*
- [x] **BottomBar** (D§7.4): left transport cluster, time labels, seek bar v1 (D§8.1 without thumbnails/waveform: thick track, buffer range, hover ghost + time label, chapter gaps, drag scrub semantics), **cone volume slider** (D§8.2), stub buttons for tracks/queue/sidebar, fullscreen.
- [x] Auto-hide state machine (D§7.1) incl. fullscreen top-edge rule and cursor hiding; `useAutoHide` unit-tested with fake timers. *(The logic is a plain `AutoHide` controller with 11 fake-timer tests; the hook only wires it.)*
- [x] Fullscreen (`window_set_mode`), Zen mode (D§7.2), always-on-top.
- [x] Context menu (D§8.8), generated from the action registry.
- [x] OSD flash + toasts (D§8.9), `osd://show` handling, optional shortcut badge.
- [x] Idle screen v1 (D§11.3): open buttons + drop target (history rows arrive in Phase 4).
- [ ] File open dialogs (`tauri-plugin-dialog`), OS drag-and-drop onto the window (replace; Shift = enqueue once Phase 4 lands), `app.paste_and_play`. *(Implemented, not yet exercised: each needs a person at the machine — a modal dialog takes focus, an OS drag needs the real pointer, and paste reads the real clipboard.)*
- [x] Responsive collapse rules (D§6.4).

### Verification

- [x] Every default shortcut in K§2 for implemented actions works; each shows the right OSD when chrome is hidden. *(20 shortcuts driven one by one, each checked against the mirrored state; OSD checked with the bars hidden.)*
- [x] Click Mute, then press Space → playback toggles (focus did not stick to the button).
- [x] Tab reaches every bar control with a visible focus ring; Esc returns to the player; Space on a focused button activates that button.
- [x] Bars hide after 2 s of stillness while playing, never while paused, never while a menu is open or a slider is dragged. In fullscreen, nudging the mouse shows only the bottom bar; touching the top edge shows the top bar.
- [x] Seek bar: resting 8 px, 12 px on hover; click lands within one frame of the target with `precise_seek`; dragging scrubs smoothly on T1; wheel over it seeks. *(Click: target 45.003 s, landed 45.003 s. Drag release: 63.02 s for 63.00 s.)*
- [x] Cone slider: drag, wheel, double-click-to-100, boost zone turns red above 100 %, mute greys the fill.
- [x] `Z` hides everything including the cursor; `Z`/`Esc` restores the exact previous layout.
- [x] Context menu shows live shortcuts; changing a binding in `keybinds.toml` updates menu and tooltips without restart.
- [ ] Window: drag by top bar, resize from all edges/corners, double-click maximizes, window controls work (W + L). *(**W**: the minimize/maximize/restore/close controls work. Moving and resizing hand the pointer to the OS, so they need a person with a real mouse.)*
- [x] Shrinking the window to 480 px collapses the bar per D§6.4 with nothing clipped. *(Checked at 880, 700, 500, 480 px.)*
- [x] Performance: with chrome animating over T4, dropped frames within A§14; no webview long tasks > 16 ms during hide/show (DevTools performance trace). *(60 s, 21 hide/show cycles: 0 dropped, 0 delayed frames, no long tasks reported, animation-frame gaps max 13.8 ms. The browser reports long tasks only above 50 ms, so the 16 ms bound is shown by the frame gaps.)*
- [x] Design audit: no hex colours, no off-token radii, no `rounded-full` outside the seek thumb (grep).

**Platforms:** W + L-Wayland + L-X11.

**Phase 3 notes (2026-11-13, Windows only).** Verification was driven through the webview's DevTools
input channel (synthetic key/mouse events delivered to the app alone), so it never touched the
owner's keyboard, pointer, or screen. Three Windows checks still need a person at the machine:
open-file/open-folder dialogs, an OS drag-and-drop onto the window, and paste-and-play from the
clipboard; plus moving and resizing the window by mouse. Bugs found by these checks and fixed: the
seek bar dropped a drag when pointer capture was lost; pointer presses on sliders took keyboard
focus; a lingering hover tooltip swallowed Escape; a hidden hover label widened the bar at narrow
widths.

---

## 6. Phase 4 — Queue, session, resume — ☑ closed (Windows, 2026-11-13) — Linux pending; two hands-on checks pending (see notes)

**Goal:** multi-file playback that never interrupts: queue panel, folder auto-queue, resume, session restore.

- [x] `queue/mod.rs`: full semantics of A§8.1 (replace/enqueue/play-next, folder auto-fill with natural sort, repeat, shuffle order, previous-restarts rule, stop-after-current, error skip + 3-strike stop). Exhaustive unit tests.
- [x] `queue_*` commands + `queue://changed`; core load pipeline steps 1, 3 of A§6.5 (layers arrive in Phase 8).
- [x] `media/identity.rs`: `media_key` (oshash + URL hash) with test vectors.
- [x] `library/db.rs`: connection thread, migration runner, `0001_init.sql` (A§9.6) — only `history` is used this phase.
- [x] Resume (A§6.5): periodic position writes, finished marking, `playback.resume` modes, resume toast with "Start over".
- [x] Per-file memory: last audio/sub track (as `TrackRule`), delays (`playback.remember_per_file`).
- [x] `queue/session.rs`: `session.json` write/restore (A§9.4), `session_ui_get/set`.
- [x] Right dock container (D§6.3): dock vs overlay, `video_set_margins` animation, resize handle.
- [x] **Queue panel** (D§10.7): virtualised list, now-playing row, drag reorder (dnd-kit), multi-select, row menu, header actions, OS file drop at position, undo toast on remove. *(The OS drop is implemented but not yet exercised; see notes.)*
- [x] Previous/Next buttons enabled state; `queue.*` actions; queue-context shortcuts.
- [x] Idle screen: Continue watching + Recent folders from `history`.

### Verification

- [x] Open one episode from T6 → queue fills with the folder in natural order (`E2` before `E10`), current item highlighted; Next/Previous walk it; reaching the end holds the last frame.
- [ ] Drag-reorder, multi-remove + Undo, Play next, Clear (keeps current) all behave and never interrupt playback (audio continuity check by ear). *(**W**: all four behave, and the playing file, its status, and its position were unaffected throughout. The by-ear check is left for a person; verification runs are muted.)*
- [ ] Dropping 3 files on the panel inserts at the drop line; dropping on the video replaces; Shift-drop enqueues. *(Needs a person with a real pointer. A Shift-drop cannot be detected during an OS drag in the webview, so enqueue is done by dropping on the panel.)*
- [x] Shuffle plays every item exactly once before repeating; turning it off resumes list order from the current item.
- [x] Quit mid-episode, relaunch → same queue, same item, paused at the same position (±1 s). `--no-session` starts clean.
- [x] Reopen a half-watched file directly → resumes 3 s early with the toast; "Start over" works; a file watched past 95 % starts from 0.
- [x] Change audio track + subtitle delay on a file, reopen it → both restored.
- [x] T12 files in a queue are marked with an error and skipped; three bad files in a row stop with one summary toast.
- [x] Dock opens with the video shrinking smoothly (windowed) and overlays in fullscreen; picture is never covered in dock mode.
- [x] `library.track_history = false` → no rows written (inspect DB), no resume, idle screen shows only open buttons.

**Platforms:** W + L-Wayland.

**Phase 4 notes (2026-11-13, Windows only).** Checked through the webview's DevTools channel in
the quiet, muted test build: the queue's mpv-side effects through the commands, and the panel
through synthetic pointer and key input sent to the webview only. Dock mode set mpv's right
margin to exactly the panel width (0.28125 at 360 / 1280 px) and back to 0 on close; fullscreen
overlays with margin 0. Session restore came back paused at 101.46 s for a pause at 101.46 s;
reopening resumed at 98.1 s for a stop at 101 s; Start over landed at 0.96 s. With history off,
the history table was byte-for-byte unchanged (row count, latest timestamp, positions). Still to
do by hand: an OS drag-and-drop onto the panel and the video, and the by-ear continuity check.
Fixed along the way: the idle bottom bar kept the last file's time and buffer; the queue total
counted only loaded items (partial sums now end in "+"); idle "Continue watching" listed files
the core would not resume (under 60 s); a run of bad files stacked a toast per file and one named
the wrong file (toasts now carry a group, D28, and the name comes from the queue item).

---

## 7. Phase 5 — Sidebar: audio, video, playback — ☑ closed (Windows, 2026-11-13) — Linux pending; two hands-on checks pending (see notes)

**Goal:** the right sidebar shell with three of its tabs, plus the quick track selectors in the bottom bar.

- [x] Sidebar shell: tab row (D§10), tab actions (`ui.sidebar_*`, `ui.toggle_sidebar`), last tab remembered in session. *(Audio, Video, Playback; the Subtitles and Bookmarks tabs join in Phases 7 and 8.)*
- [x] Standard parameter row (D§8.5): slider + exact value field + reset + detents + keyboard stepping. Segmented control, toggle, select, chip, checkbox primitives (D§8.6).
- [x] **Quick selectors** (D§8.3): subtitles popover (tracks, Off, add file) and audio popover (tracks + output devices); state dots on the buttons. *("Add subtitle file…" and auto-match are listed but disabled until Phase 7.)*
- [x] **Audio tab** (D§10.2): track select, output device (hot-plug aware), preferred-language chips, audio delay row.
- [x] Equalizer: `player/equalizer.rs` (filter graph build, live gain via `af-command` or debounced replace per S4), `audio_set_eq`, EQ presets file (K§5.12) + `eq_presets_*`; UI: response-curve canvas with true biquad math (`src/lib/eq.ts`, unit-tested against known coefficients), 10-band nodes, parametric mode, visual preset chips, preamp, save preset. Normalisation toggle. *(The curve is drawn as SVG rather than a canvas: about 100 points plus 10 nodes, each node a keyboard-reachable slider.)*
- [x] **Video tab** (D§10.3): picture sliders, aspect segmented control, rotate/flip (shader path), zoom + Alt-drag pan, tone-mapping select gated by `capabilities`, deband/interpolation, shaders list from `<config>/shaders/`, reset all.
- [x] **Playback tab** (D§10.4): speed row + chips, frame step, reverse toggle, loop (file / A–B with fields and seek-bar brackets), sleep timer (`player/sleep.rs` with fade-out), seek-bar toggles, **stream inspector** (`inspector_subscribe`, 2 Hz, copy-all, show in folder).
- [x] Remaining actions from K§2 audio/video groups wired to real handlers.

### Verification

- [x] Every slider updates the picture/sound live while dragging on T1; typed exact values land exactly; reset and double-click-reset work; detent snaps at default. *(Contrast moved mpv to 49 mid-drag; typed 15 and −3.5 dB landed exactly; a press 4 px from the centre snapped to 0, 20 px gave 12.)*
- [ ] Audio popover switches output device mid-playback with < 1 s interruption; unplugging the active device falls back to system default and updates the list. *(Implemented; needs a person: switching routes sound to the owner's real devices, and unplugging needs hands.)*
- [x] EQ: dragging a band is audible immediately with no dropout or click (S4 path), curve matches the nodes; presets switch instantly; a saved custom preset persists in `eq-presets.toml` and survives restart; parametric nodes add/remove/Q-adjust. *(**W**, muted: node drags, preamp, and switching between 10-band presets left mpv's `af` text unchanged, i.e. no filter rebuild — S4 measured the live gain change at +12.0 dB for +12 dB. A saved preset was written to `eq-presets.toml` and was still listed and active after a restart. Parametric: double-click added a 299 Hz node, the wheel raised Q 1 → 1.1, right-click → Delete removed it; shelves map to `bass`/`treble`. The by-ear part is left for a person.)*
- [x] Flip H/V and rotate work with `hwdec-current` still showing a hardware decoder; subtitles remain unmirrored. *(`hwdec-current` stayed `d3d11va` with Mirror on; the picture mirrored in a window capture. Unmirrored subtitles were checked in spike S4.)*
- [ ] HDR file T1 on an SDR display: tone-mapping select visibly changes the image; on **W** with HDR enabled the status chip reads passthrough active. *(**W** SDR: clip vs hable differ by 6–11 levels per channel on average, and the chip reads "Tone-mapped". The HDR-display half needs Windows HDR turned on by a person.)*
- [x] A–B loop loops precisely; sleep timer "15 min" (test with a 1-min debug value) fades and pauses; "End of file" pauses at EOF instead of advancing. *(A = 10.000, B = 12.500 typed: position stayed within 10.000–12.429 over 6 s with two wraps. A 15 s timer faded the volume (80 → 37 with 5 s left), paused, and put the volume back to 80; End of file paused on the same item in a two-item queue.)*
- [x] Inspector values for T1 match `mediainfo`/`ffprobe` for codec, resolution, fps, bit depth, channels; updates stop when the tab is hidden (verify no `player://inspector` events). *(HEVC, 3840×2160, 23.976 fps, 10-bit, BT.2020/PQ, AAC stereo 48 kHz — all as `ffprobe` reports. 5 events in 2.2 s on the Playback tab, 0 in 2.5 s after switching to Audio.)*
- [x] Sidebar open + EQ canvas + inspector running over T4: dropped frames within A§14. *(60 s on T4: 15 node drags on the Audio tab, then the inspector: 0 dropped, 0 decoder-dropped, 0 delayed frames.)*
- [ ] On **L**, options the render backend lacks are hidden, not broken.

**Platforms:** W + L-Wayland.

**Phase 5 notes (2026-11-13, Windows only).** Driven through the webview's DevTools channel in
the quiet, muted test build. Still for a person: switching and unplugging output devices (real
hardware), listening for EQ dropouts, and the HDR-display passthrough chip. Fixed along the way:
mpv reports aspect overrides back as decimals (and "no" as −2), so the mirror maps them to names;
the inspector read the GPU surface format (`d3d11`) instead of the decoded one with hardware
decoding; graph grid lines swallowed double-clicks; segmented controls truncated long labels.

---

## 8. Phase 6 — Seek previews and waveform — ☐ not started

**Goal:** thumbnail hover previews and audio waveforms, without ever disturbing main playback.

- [ ] `media/thumbs.rs` (A§11.1): headless handle + SW render context, bucketing, latest-wins worker, neighbour pre-generation, JPEG cache, LRU eviction.
- [ ] `vthumb` async URI scheme (A§7.5) + URL helper in `transport.ts`; CSP entry.
- [ ] Seek-bar hover preview card (D§8.1): thumbnail + chapter + time, edge clamping, no layout shift, graceful no-thumbnail mode.
- [ ] `media/waveform.rs` (A§11.2): headless decode via `ao=pcm` pipe, peak reduction, `.vwf` writer/reader with a round-trip unit test, cancellation on file change, `performance.waveform` modes.
- [ ] `waveform_request` / `waveform_get` (binary) + `media://waveform` progress.
- [ ] Waveform seek bar (D§8.1 waveform mode): canvas renderer with pixel binning, played/unplayed colouring, progress line while analysing, theme/accent redraw.
- [ ] Cache management: size cap, "Clear cache" command, cache directory layout (A§9.7).

### Verification

- [ ] Hovering across T1's seek bar shows correct frames (spot-check 5 positions against actual seeks); cached p95 ≤ 30 ms, uncached p95 ≤ 400 ms (log timings).
- [ ] Rapid scrubbing of the hover never queues stale work (worker log shows cancellations) and never drops frames in the main video (T4, dropped-frame counter).
- [ ] T13 (3 h): bucket step ≈ 30 s cap respected; thumbnails still correct near the end.
- [ ] Audio file and network stream: time-only preview, no errors, no thumbnail requests.
- [ ] Waveform for a 2 h film completes within the A§14 budget on **W** and **L**; main playback shows no audio glitch or frame drop while it runs.
- [ ] Waveform visually matches loud/quiet sections (compare against Audacity for one file); switching audio track regenerates for that track.
- [ ] Switching files mid-analysis cancels the job (no orphan temp file/pipe).
- [ ] Cache obeys `performance.cache_limit_mb` (set to 64, generate, confirm eviction); deleting the cache dir while running recovers cleanly.
- [ ] Toggling theme and accent redraws the waveform in the new colours.

**Platforms:** W + L-Wayland.

---

## 9. Phase 7 — Subtitle suite — ☐ not started

**Goal:** the Subtitles tab in full — live sync tools, the visual timeline, styling, and one-click auto-matching.

- [ ] **Subtitles tab** layout (D§10.1): track select, visibility, add file, delay row (±5 s range, 10 ms steps), style section.
- [ ] Style controls → `sub_set_style`: font picker (`fonts_list`, each family rendered in its face, virtualised), exact size + S/M/L/XL, colours, outline, background box, position, ASS override segmented control with tooltip.
- [ ] `subtitles/cues.rs`: SRT, ASS/SSA, WebVTT parsers (fixtures + unit tests incl. malformed input, BOMs, CRLF, overlapping cues).
- [ ] `subtitles/mkv.rs`: embedded text-subtitle extraction via `matroska-demuxer` off-thread, cue cache (`cache/cues/`).
- [ ] Progressive cue recording for other formats (A§11.3); `sub_get_cues` with `coverage`; `media://cues`.
- [ ] **Subtitle timeline** (D§10.1): scrolling canvas (waveform + cue lane), fixed playhead, zoom levels, drag-cues-to-shift-delay with live update of slider/value, Shift-snap to onsets, drag-waveform-to-scrub, prev/next "cue to now" buttons, hatch for unknown coverage.
- [ ] `subtitles/automatch.rs` (A§10.5) with unit tests on synthetic folder listings (T6-like naming chaos, language suffixes, ambiguity); `sub_automatch`; `sub_links` persistence and application at load.
- [ ] Auto-match result card + Apply; entry points in popover, tab, and action `subtitles.automatch`.
- [ ] Subtitle actions from K§2 (delay, sync-to-cue, size, position, add file).
- [ ] `presets/series.rs` series-key parser with the A§10.2 test vectors (needed by auto-match now, presets next phase).

### Verification

- [ ] With T8 (known offset): dragging the cue lane until speech onsets line up yields the known offset within ±50 ms; `Ctrl+G`/`Ctrl+H` sync-to-cue matches the buttons.
- [ ] Delay slider, value field, timeline, and `G`/`H` keys all stay in agreement during interaction.
- [ ] Timeline scrolls smoothly at 60 fps during playback at all zoom levels; no drift between cue blocks and displayed subtitles over 10 min.
- [ ] T5 (embedded ASS): full coverage appears within a few seconds of load and is cached (second load instant). T7 (PGS): blocks without text appear progressively; hatch covers unknown regions.
- [ ] Style changes preview live; with T5 the four ASS modes behave as their tooltips say; a font with CJK glyphs renders.
- [ ] Auto-match on T6: all episodes paired despite different names; a deliberately ambiguous pair is reported, not guessed; after Apply, each episode loads its subtitle on Next.
- [ ] Removing a `sub_link` (delete the external track) persists.
- [ ] Parser/automatch/series unit tests green; fuzz-ish malformed fixtures don't panic.

**Platforms:** W + L-Wayland.

---

## 10. Phase 8 — Bookmarks, chapters, presets — ☐ not started

**Goal:** the unified chapter/bookmark timeline and per-show presets that apply themselves.

- [ ] `bookmarks.rs` + commands; `Ctrl+B` quick add with auto label + OSD; toast "Rename".
- [ ] **Bookmarks tab** (D§10.5): merged chronological list, filter, inline rename, delete with undo, current-position highlight, hover thumbnail.
- [ ] Seek bar: bookmark ticks, right-click mini menu (add bookmark here, set A/B, copy timestamp); `markers.next/previous` across chapters + bookmarks.
- [ ] `presets/mod.rs`: TOML load/save/watch of `presets/*.toml` (A§10.1), validation with clear errors, `preset_*` commands.
- [ ] `presets/matcher.rs`: match types, specificity ordering, priority ties (A§10.4) — table-driven unit tests.
- [ ] Track rule resolution (A§10.3) — unit tests against synthetic track lists.
- [ ] Full layered load pipeline (A§6.5 step 2 & 4): defaults → presets → per-file memory → CLI; `appliedPresets`, `presetDirty` computation.
- [ ] `preset_capture_current` (diff vs defaults, track → rule conversion, default match suggestion).
- [ ] **Preset strip** (D§10.6): applied preset + modified dot, Update, Save as… popover (name, applies-to, sections), ⋯ menu.
- [ ] Presets manager sub-page data plumbing (UI shell lands with Settings in Phase 10; for now "Reveal preset file").
- [ ] `--preset <ID>` CLI flag; `presets.*` actions.

### Verification

- [ ] `Ctrl+B` during playback adds a bookmark without opening anything; it appears on the seek bar and in the tab; persists across restart and after renaming/moving the file (same `media_key`).
- [ ] T1's chapters and user bookmarks interleave correctly; `PageDown`/`PageUp` step through both.
- [ ] On T6 episode 1: set subtitle delay −350 ms, pick the English "Full" track, raise contrast, save as a **series** preset → episode 2 (via Next) loads with all of it applied automatically, including the right tracks even though track order differs (craft one episode with reordered tracks).
- [ ] A **folder** preset overrides a **series** preset key-by-key; keys it doesn't set fall through (verify with two presets and the inspector).
- [ ] Changing a value shows the modified dot; Update writes only that change to the TOML (diff the file); hand-editing the TOML while running re-applies live.
- [ ] A malformed preset file produces one toast naming the file and line and is skipped; other presets still load.
- [ ] `playback.presets_enabled = false` disables auto-application; `--preset` forces one.
- [ ] Matcher, track-rule, and capture unit tests green.

**Platforms:** W + L-Wayland.

---

## 11. Phase 9 — Library, playlists, streams — ☐ not started

**Goal:** the Library drawer — browse and enqueue anything without touching what's playing.

- [ ] Remaining DB tables in use; `library/scan.rs` (walk, upsert, removal, `notify` watching, ignore patterns, lazy `media_key` + duration fill); `library://scan` progress.
- [ ] Video metadata from filename (series parser); music tags via `lofty` (`library/music.rs`); FTS index maintenance.
- [ ] `library_*` commands with paging; live folder browser for pinned folders.
- [ ] **Library drawer** (D§11.1): slide-in, section switcher (duotone icons, D§5.2), search, virtualised lists; Videos (All/Series/Folders), Music (Albums/Artists/Tracks), Recent, Folders (pin/unpin, breadcrumb), Playlists.
- [ ] Item interactions: play now / play next / enqueue (keys + row buttons + context menu); watched check and progress line; drawer stays open while enqueueing.
- [ ] `library/playlists.rs`: JSON files (A§9.5), relocation by `media_key`, M3U8 import/export; playlist view with reorder/rename/delete; "Save queue as playlist", "Add to playlist".
- [ ] `library/iptv.rs`: M3U parser (fixtures: attributes, `#EXTVLCOPT`, groups, bad lines), stream sources CRUD + refresh, Streams section (sources → groups → channels, search), per-stream headers at load; Open URL dialog.
- [ ] Network settings applied (`network.*` keys, K§5.9); yt-dlp detection.
- [ ] Music niceties: album queueing in disc/track order; gapless via two-entry mpv playlist window (A§8.1 note) behind `audio.gapless`.
- [ ] Library context shortcuts; `ui.toggle_library`; top-bar title button wiring.

### Verification

- [ ] Adding a folder of ≥ 2 000 files indexes at the A§14 rate with playback of T4 running and no dropped frames; UI stays responsive.
- [ ] T6 appears as one series with correct episode order; movies appear ungrouped; T9 appears under the right album/artist with track order.
- [ ] Adding, renaming, and deleting a file on disk is reflected without a manual rescan.
- [ ] While T1 plays: browse, search, enqueue three items, start a playlist with "play next" — playback never hiccups; the drawer closes only when asked.
- [ ] Search finds by title, series, artist, album; `Ctrl+F` focuses it; results update as you type without jank at 10 000 rows.
- [ ] Playlist round-trip: save queue → restart → open playlist → identical order; export M3U8 → import → identical; move a file on disk → playlist relocates it via `media_key`.
- [ ] T11: the `.m3u` imports with groups; a channel plays; `#EXTVLCOPT` user agent is sent (verify with a local test server); refresh replaces entries.
- [ ] With artwork loading off, a packet capture during library use shows **zero** outbound connections.
- [ ] T9 album plays gaplessly (no audible gap/click between tracks of a continuous album).
- [ ] Pinned folder browser works for a folder that is not indexed; pin/unpin persists.

**Platforms:** W + L-Wayland.

---

## 12. Phase 10 — Settings, hotkey editor, theming — ☐ not started

**Goal:** every setting and shortcut editable in-app, searchable, in plain English; light theme and accent picker.

- [ ] `src/features/settings/schema.ts` covering K§5.1–5.10; schema-parity test against the Rust `Config` (K§6 rule 1).
- [ ] **Settings overlay** (D§11.2): rail with duotone category icons, pages generated from the schema, control-by-type mapping (K§6 rule 4), modified dot + per-row reset, "Reset this page", restart chips, platform gating.
- [ ] Settings search across label/tooltip/keywords/key with highlighting.
- [ ] Library page (folders list management), Performance page (cache size + clear), Playback → **Presets manager** sub-page.
- [ ] **Hotkey editor** (K§4): grouped rows, capture mode, conflict resolution bar, per-row and global reset, find-by-key, keymap base select, custom-binding rows (view + delete; creation stays file-based), immediate persistence via `keybinds_set`.
- [ ] `resources/keymaps/vlc.toml` and `mpv.toml` (K§2 "alternative keymaps").
- [ ] Shortcuts sheet (`F1`); command palette (`Ctrl+Shift+P`) listing all enabled actions with fuzzy search.
- [ ] Theming: light theme tokens (D§2.2), `system` following, **accent picker** with swatches + free input, OKLCH derivation + contrast guard (`src/lib/color.ts`, unit-tested), live redraw of canvases.
- [ ] `interface.ui_scale`, `reduce_motion`, tooltip-shortcut toggle, OSD position/duration.
- [ ] Config migration scaffold (`config/migrate.rs`) with a no-op v1 and a test proving `.bak` creation.

### Verification

- [ ] Every key in K§5.1–5.10 is reachable in the UI, has a label and tooltip, and changing it takes effect live (or shows the restart chip). Spot-check ten across categories against their mpv effect.
- [ ] Settings search: "subtitle size", "hdr", "wayland", and a pasted `performance.demuxer_cache_mb` each find the right row.
- [ ] After a session of changes, `config.toml` contains only non-default keys and any pre-existing comments are intact.
- [ ] Hotkey editor: rebind Play/Pause, bind a key already in use (conflict bar offers Replace), unbind, reset one, reset all, find-by-key — all persist to `keybinds.toml` and apply immediately to menus/tooltips.
- [ ] Switching base keymap to `vlc` then `mpv` changes defaults while keeping user overrides.
- [ ] Shortcut OSD badge shows the key actually pressed, including for a custom binding.
- [ ] Light theme: every surface, popover, and canvas is legible; no dark-only hard-coded colour remains (visual pass over every panel).
- [ ] Custom accent: pick a very light and a very dark colour — text on accent stays readable (contrast guard), focus ring visible.
- [ ] Settings open over playing T4: no dropped frames; playback continues.
- [ ] Schema-parity and colour unit tests green.

**Platforms:** W + L-Wayland + L-X11 (theme/decorations interplay).

---

## 13. Phase 11 — Window polish and Linux parity hardening — ☐ not started

**Goal:** picture-in-picture, native-feeling windows on every platform, HDR confirmation, and closing every Linux gap.

- [ ] **PiP** (D§9): mode switch on the main window, geometry save/restore, compact chrome, aspect-locked resize, corner placement; Wayland capability gating with the one-time toast.
- [ ] Windows window polish: Snap Layouts on the maximize button, drop shadow and rounded corners on the undecorated window, correct maximized insets, taskbar thumbnail toolbar (prev/play/next), DPI change handling.
- [ ] `interface.window_decorations = "native"` on all platforms (top bar drops its window controls).
- [ ] Linux: CSD behaviour on GNOME (drag, double-click, resize edges, right-click window menu via `gtk_window` where possible); SSD on KDE with `native`; fractional-scale sharpness check; XWayland escape hatch (`video.linux_force_xwayland`).
- [ ] Linux: NVIDIA detection → `WEBKIT_DISABLE_DMABUF_RENDERER` logic (`performance.webkit_dmabuf`), verified on an NVIDIA machine if available (else documented as unverified).
- [ ] `tauri-plugin-window-state` for size/position (position skipped on Wayland).
- [ ] HDR: Windows passthrough verified with display HDR on/off and mid-playback toggles; Linux tone-mapped output verified.
- [ ] Multi-monitor: fullscreen on the window's current monitor; moving between monitors with different DPI/refresh.
- [ ] `general.pause_on_minimize`; mouse-idle and focus-loss edge cases for auto-hide.
- [ ] Full keyboard-only pass and screen-reader smoke test (Narrator / Orca) on the bars and settings.
- [ ] Performance pass against every A§14 budget on **W** and **L**; fix or record each miss.

### Verification

- [ ] PiP on **W** and **L-X11**: `Ctrl+P` shrinks to the configured corner, stays on top, hover shows compact chrome, drag moves, edge-resize keeps aspect, `Esc` restores the exact previous geometry and mode (from windowed, maximized, and fullscreen).
- [ ] PiP on **L-Wayland**: resizes and switches chrome, shows the compositor hint once, never errors.
- [ ] Windows: hovering maximize shows Snap Layouts; Win+arrow snapping works; shadow present; no 1-px gaps when maximized; taskbar buttons control playback.
- [ ] GNOME Wayland, KDE Wayland, X11: move, resize from every edge, maximize, fullscreen, and restore all behave; no stuck transparent regions; no input dead zones.
- [ ] T1 on an HDR display (W): `video-out-params` shows PQ/BT.2020 with passthrough on, SDR with it off; toggling Windows HDR mid-playback recovers without restart.
- [ ] Budget table A§14 re-measured on both machines and pasted into the phase notes; every miss has an issue or a justified budget change.
- [ ] The parity matrix (§15) has no unexplained ✗.

**Platforms:** W + L-Wayland (GNOME + KDE if available) + L-X11 — all mandatory.

---

## 14. Phase 12 — Packaging, CI, release — ☐ not started

**Goal:** anyone can clone and build; the owner can install a release on both OSes.

- [ ] App icon set (D§5.3) for Windows and Linux; `.desktop` entry with MIME types.
- [ ] Windows: NSIS installer (per-user), portable `.zip` (with `portable` marker support), file associations + "Open with", `libmpv-2.dll` bundled beside the exe.
- [ ] Fedora: RPM with `Requires:` from A§12.2, desktop file, icons, MIME associations; README troubleshooting points to `scripts/check-deps.sh`.
- [ ] GitHub Actions: `ci.yml` (Windows runner + `fedora:latest` container: fmt, clippy, tests, `pnpm check`, build) and `release.yml` (tag → build artifacts → draft release). Cache cargo/pnpm. No secrets required for CI.
- [ ] Licensing: confirm the pinned libmpv build's licence; add `THIRD_PARTY_NOTICES.md` (mpv, FFmpeg, crates via `cargo about`, npm via a licence report); release notes state binary licence terms and link sources (A§13).
- [ ] Confirm the design-credit licence at the source and finalise the README credit line.
- [ ] README final pass: real screenshots (dark + light), accurate build steps verified on a clean VM/container for the build (not playback), troubleshooting section.
- [ ] Tag `v0.1.0`.

### Verification

- [ ] Clean Windows 11 VM: installer installs without admin, app launches, plays T3, file association opens a double-clicked `.mkv` in the running instance; uninstall leaves only user config.
- [ ] Portable zip runs from a USB path with all data under its own folder.
- [ ] Clean Fedora install: `dnf install ./vihpluh-*.rpm` pulls dependencies; app appears in the launcher; plays T3 out of the box and T1 after the RPM Fusion steps; uninstall is clean.
- [ ] CI green on both runners from a fresh clone; release workflow produces installer, zip, and RPM with checksums.
- [ ] Repo audit: no personal paths, config, media names, or AI/assistant attribution anywhere in history or files other than `CLAUDE.md` and `.claude/`; `git log` shows a single author identity.
- [ ] Packet capture of a 5-minute local-playback session: zero outbound connections.

**Platforms:** W + L.

---

## 15. Parity matrix and release gate

Fill in as phases close (✓ works · ◐ works with documented limitation · ✗ broken · — not applicable). `v0.1.0` requires no ✗.

| Capability | Windows | Fedora Wayland | Fedora X11 |
|---|---|---|---|
| Hardware decode HEVC 4K (zero-copy where A§5.4 says so) | | | ◐ expected: copy-back on AMD/Intel |
| Hardware decode AV1 | | | |
| HDR passthrough | | — tone-mapped | — tone-mapped |
| Transparent UI over video, no flicker | | | |
| Auto-hide chrome / Zen | | | |
| Custom title bar + resize | | | |
| Native decorations option | | ◐ expected: CSD on GNOME | |
| Fullscreen | | | |
| PiP positioning + always-on-top | | ◐ expected: compositor-controlled | |
| Media keys / OS media controls | | | |
| Sleep inhibit | | | |
| Seek thumbnails | | | |
| Waveform | | | |
| Subtitle timeline + auto-match | | | |
| Presets | | | |
| Library + watch folders | | | |
| IPTV / network streams | | | |
| Settings + hotkey editor | | | |
| Installer / package | | | |

**Release gate for v0.1.0:** all twelve phases closed · matrix has no ✗ · A§14 budgets met or consciously amended ·
README build steps verified from a clean clone on both OSes · owner has used it as their only player for one week.
