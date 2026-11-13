# CLAUDE.md — vihpluh

Guidance for AI coding agents working in this repository. Read this first, every session.

---

## ⛔ STRICT GIT RULE — NON-NEGOTIABLE

> **Whenever creating git commits, NEVER add `Co-Authored-By` trailers or mention Claude/Anthropic
> anywhere in commit messages, PRs, or repo metadata. All commits must be authored solely under the
> repository owner's local git user config.**

Concretely:

- No `Co-Authored-By:` lines. No "Generated with …" footers. No AI/assistant attribution of any kind
  in commit messages, PR titles/bodies, issue text, tags, release notes, code comments, or docs.
- Never pass `--author`, never set `GIT_AUTHOR_*` / `GIT_COMMITTER_*`, never edit `git config user.*`.
  Commit with whatever identity `git config` already has.
- `.claude/settings.local.json` sets `"includeCoAuthoredBy": false` (and an empty `attribution`).
  If that file is missing on a machine, recreate it before committing. It is gitignored on purpose.
- Commit or push only when the owner explicitly asks. Never force-push, never rewrite published history.
- Commit style: [Conventional Commits](https://www.conventionalcommits.org) —
  `feat(queue): …`, `fix(seekbar): …`, `docs: …`, `chore: …`. Imperative, ≤ 72-char subject, body explains *why*.

---

## What this project is

**vihpluh** is a personal, keyboard-driven, zero-bloat desktop media player: mpv's playback engine
behind a UI inspired by Juxtopposed's VLC redesign. Windows 10/11 first, Fedora Linux (Wayland + X11)
at parity. Public MIT repo, but built for one person's daily use — no telemetry, no onboarding, no
accounts, no update nags, no network traffic unless a stream is opened.

**Current status:** Phases 1–5 closed on Windows (Linux pending: spike S2 and the render backend need
the Fedora machine; Phases 3–5 also have a few hands-on checks listed in their notes). Next step is
**Phase 6** in [`docs/IMPLEMENTATION_PLAN.md`](docs/IMPLEMENTATION_PLAN.md). Update this line when a
phase closes.

## Document map — read the relevant one before touching that area

| File | Read it when… |
|---|---|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | touching Rust core, mpv, video surfaces, IPC, storage, build/deps. Source of truth for contracts and schemas. |
| [`docs/DESIGN_SYSTEM.md`](docs/DESIGN_SYSTEM.md) | writing any UI. Tokens, geometry, components, layout metrics, motion. |
| [`docs/KEYBINDS_AND_CONFIG.md`](docs/KEYBINDS_AND_CONFIG.md) | adding a setting, an action, or a shortcut. Source of truth for config keys and action IDs. |
| [`docs/IMPLEMENTATION_PLAN.md`](docs/IMPLEMENTATION_PLAN.md) | deciding what to build next and how to prove it works. |
| [`README.md`](README.md) | public-facing overview and build instructions. Keep it accurate as features land. |

If code and a doc disagree, that is a bug: fix whichever is wrong **in the same change**. A decision
that deviates from a doc must update the doc and add a row to the decision log in `ARCHITECTURE.md` §15.

## Locked stack (do not swap without owner approval)

- **Shell:** Tauri 2 · **Core:** Rust (stable, edition 2024) · **Engine:** libmpv ≥ 0.40 via `libmpv2` crate
- **UI:** TypeScript (strict) · React 19 · Vite · Tailwind CSS 4 · Zustand · Radix primitives · dnd-kit
- **Storage:** TOML (config, keybinds, presets) · JSON (playlists, session) · SQLite via `rusqlite` (library, history, bookmarks)
- **Types across IPC:** generated from Rust with `ts-rs` into `src/ipc/bindings/` — never hand-edit
- **Tooling:** pnpm · Biome (lint + format) · Vitest · `cargo fmt` / `cargo clippy -D warnings` / `cargo test`

## Commands (valid once Phase 1 scaffolding lands)

```sh
pnpm install                  # frontend deps
pnpm fetch:libmpv             # Windows only: downloads pinned libmpv into src-tauri/lib/
pnpm tauri dev                # run the app with hot reload
pnpm tauri build              # release bundle (NSIS on Windows, RPM on Fedora)
pnpm check                    # biome check + tsc --noEmit + vitest run
pnpm gen:bindings             # regenerate src/ipc/bindings from Rust types (ts-rs)
cargo fmt --manifest-path src-tauri/Cargo.toml --all
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Dev runs use a repo-local data root so personal config is never touched: `VIHPLUH_HOME=./.local`
(set automatically by the `tauri dev` script; `.local/` is gitignored).

## Architecture invariants — break none of these

1. **Rust owns truth.** Player state, queue, config, presets, bookmarks, library live in the Rust core.
   The frontend holds only view state (open panels, hover, drafts). No playback logic in React.
2. **mpv is touched in exactly one place:** `src-tauri/src/player/`. Nothing else calls libmpv, and the
   frontend never sends raw mpv commands (the only escape hatch is the `mpv.command` action, routed through the core).
3. **vihpluh's queue is authoritative; mpv's internal playlist is not used** as a source of truth.
4. **Tauri APIs are imported only inside `src/ipc/`.** Components call typed wrappers from
   `src/ipc/commands.ts` / `src/ipc/events.ts`. This keeps the transport swappable (see ARCHITECTURE §4.5).
5. **Every user-triggerable behavior is an action** with an ID in the action registry
   (KEYBINDS_AND_CONFIG §2). Buttons, menus, context menu, shortcuts, and the command palette all
   dispatch the same action — never duplicate handler logic.
6. **Every setting is a typed key in the config schema** with a default, a plain-English label, and a
   tooltip. No hidden settings, no `localStorage` for durable state.
7. **Platform code lives behind traits** in `src-tauri/src/video/` and `src-tauri/src/platform/`.
   No `#[cfg(target_os)]` scattered through feature code; no `navigator.platform` checks in components
   (use the `capabilities` object from `app_info`).
8. **Never block the UI thread or the mpv event thread.** Disk, DB, hashing, scanning, thumbnailing,
   waveform decoding run on worker threads.
9. **No telemetry, no analytics, no auto-update, no remote fonts/scripts.** The webview loads only
   bundled assets. Outbound network happens only for user-opened streams (and optional artwork, off by default).
10. **Personal data never enters the repo.** User config, presets, playlists, DB, caches live in OS
    config/data dirs (or `./.local` in dev). Shipped defaults live in `src-tauri/resources/`.

## Code conventions

**Rust**
- Errors: `thiserror` enums per module, converted to the IPC `AppError { code, message }` at the command boundary. No `unwrap()`/`expect()` outside tests and startup invariants; no panics across FFI.
- `unsafe` only in `player/mpv.rs`, `video/*`, `platform/*`, each block with a `// SAFETY:` comment.
- Logging via `tracing` (`info!` for lifecycle, `debug!` for flow, `warn!`/`error!` for faults). Never log file contents; paths only at `debug`.
- Module layout follows ARCHITECTURE §3 exactly. One Tauri command file per domain under `ipc/commands/`.

**TypeScript / React**
- `strict: true`, no `any`, no non-null `!` without a comment. Function components + hooks only.
- State: one Zustand store per domain in `src/stores/`, fed by IPC events. Select narrowly; never subscribe a component to the whole player store.
- High-frequency values (playback position, drag state) flow through refs/`requestAnimationFrame`, not React state.
- Styling: Tailwind utilities mapped to design tokens. **No raw hex colors, no arbitrary radii, no pill shapes** — use tokens from DESIGN_SYSTEM. Radius is 4 px (controls) or 6 px (surfaces), nothing else.
- Every interactive control: keyboard reachable, visible focus ring, `aria-label`, tooltip showing its current shortcut.
- Files: `PascalCase.tsx` components, `camelCase.ts` modules, colocated `*.test.ts`.

**General**
- Smallest change that satisfies the task. No speculative abstractions, no new dependencies without a one-line justification in the PR/commit body.
- No comments that restate code. Comment *why* (constraints, mpv quirks, platform workarounds) with a link to the mpv option or issue when relevant.
- Do not add files like `NOTES.md`, `TODO.md`, or per-task plans to the repo. Progress is tracked by ticking checkboxes in `docs/IMPLEMENTATION_PLAN.md`.

## Definition of done (every change)

1. `pnpm check` and the three `cargo` commands above pass with zero warnings.
2. The behavior was exercised in the running app (`pnpm tauri dev`) — not just compiled. For anything
   touching playback, test with a real file from `test-media/` (see IMPLEMENTATION_PLAN §2).
3. Any new action/setting/shortcut is registered in the schema **and** documented in `docs/KEYBINDS_AND_CONFIG.md`.
4. Platform-sensitive changes (video surface, window, hwdec, paths) state in the commit body which of
   Windows / Fedora-Wayland / Fedora-X11 were actually tested. Untested platforms are said to be untested.
5. The matching checkbox in `docs/IMPLEMENTATION_PLAN.md` is ticked only when its verification criteria were observed.

## Platform notes that bite

- **Windows:** video is a native child window *under* a transparent WebView2. The page background must
  stay transparent wherever video should show; an opaque full-window element hides the video.
  `libmpv-2.dll` must sit next to the executable. The video host window must never get
  `WS_CLIPSIBLINGS` (the video disappears entirely — ARCHITECTURE §5.2).
- **Windows toolchains:** MSVC is the documented default; the GNU toolchain + MinGW-w64 GCC also builds
  (ARCHITECTURE §12.1). Rust tests belong in the library's unit-test binary, not `src-tauri/tests/` (D17).
- **Verifying in the running app:** set `VIHPLUH_QUIET_TEST=1` (debug builds) so the window opens without
  taking focus, and pass `--mpv-opt mute=yes`. The owner uses this machine while work is in progress:
  verification must not steal focus, play sound, move the pointer, or capture the screen.
- **Linux:** video is rendered through libmpv's render API into a `GtkGLArea` under a transparent
  WebKitGTK webview — `--wid` embedding does not work here (and is impossible on Wayland). All GL calls
  happen on the GTK main thread inside the `render` signal.
- **Wayland:** no global window positioning, no programmatic always-on-top, GNOME has no server-side
  decorations. Never assume those work; check `capabilities`.
- **Fedora codecs:** stock Fedora cannot decode HEVC. Runtime docs must keep pointing to the RPM Fusion steps (ARCHITECTURE §12.2).
- The Fedora dev package is **`mpv-devel`** (not `mpv-libs-devel`).

## Things not to do

- Don't use the VLC cone logo or the name "VLC" in the app's branding/assets (VideoLAN trademarks). The
  cone-*shaped volume slider* is a plain wedge and is fine.
- Don't introduce Electron, a second UI framework, a CSS-in-JS runtime, a component kit with its own
  visual style (MUI, Chakra, shadcn themes), or an ORM.
- Don't add onboarding, tips-of-the-day, changelog popups, ratings prompts, or any modal the user didn't ask for.
- Don't read or write the user's `mpv.conf`/`input.conf` unless `mpv.load_user_config = true`.
