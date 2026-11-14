# Contributing to vihpluh

vihpluh is a personal, keyboard-driven media player. It is public so others can build it, learn from
it, or send fixes. Small, focused changes are the easiest to review. Open an issue before starting
anything large, because the stack and the scope are deliberately fixed (see
[ARCHITECTURE §1–2](docs/ARCHITECTURE.md#1-goals-and-non-goals)).

Read these before touching an area:

| Document | When |
|---|---|
| [ARCHITECTURE](docs/ARCHITECTURE.md) | Rust core, mpv, video surfaces, IPC, storage, build. The source of truth for contracts and schemas. |
| [DESIGN_SYSTEM](docs/DESIGN_SYSTEM.md) | Any UI: tokens, geometry, components, motion. |
| [KEYBINDS_AND_CONFIG](docs/KEYBINDS_AND_CONFIG.md) | Adding an action, shortcut, or setting. The source of truth for action IDs and config keys. |
| [IMPLEMENTATION_PLAN](docs/IMPLEMENTATION_PLAN.md) | What is done, what is open, and how each item is verified. |

If code and a document disagree, that is a bug: fix whichever is wrong **in the same change**. A
decision that departs from a document updates it and adds a row to the decision log
(ARCHITECTURE §15).

---

## 1. Setting up

Requirements: Rust stable (edition 2024, so ≥ 1.85), Node 22 LTS or newer, pnpm, and libmpv ≥ 0.40.

### 1.1 Windows 10 / 11 — MSVC (documented default)

```powershell
winget install --id Rustlang.Rustup
winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install --id OpenJS.NodeJS.LTS
winget install --id pnpm.pnpm
winget install --id LLVM.LLVM          # llvm-dlltool makes mpv.lib from the DLL
winget install --id 7zip.7zip          # unpacks the libmpv archive
winget install --id Git.Git
rustup default stable-msvc

git clone https://github.com/ChickenWings-0/vihpluh.git
cd vihpluh
pnpm install
pnpm fetch:libmpv                       # pinned libmpv build → src-tauri/lib/windows-x64/
pnpm tauri dev
```

`pnpm fetch:libmpv` downloads the pinned shinchiro build, checks its SHA-256, extracts
`libmpv-2.dll` and the headers, and writes an MSVC import library. `-- -Force` re-fetches.
`src-tauri/build.rs` copies the DLL next to the built executable.

### 1.2 Windows — GNU toolchain (no administrator rights)

The Visual Studio Build Tools need administrator rights. The GNU toolchain builds the same tree:

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup override set stable-x86_64-pc-windows-gnu     # in the repo
# MinGW-w64 GCC from MSYS2 (also provides windres):
#   pacman -S mingw-w64-x86_64-gcc
$env:PATH = "C:\msys64\mingw64\bin;$env:PATH"
pnpm fetch:libmpv
pnpm tauri dev
```

`build.rs` links `libmpv.dll.a` when `target_env = "gnu"`. A GNU build links `WebView2Loader.dll`
dynamically, so it must sit beside the executable (the portable script includes it).

### 1.3 Fedora (Wayland and X11)

```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel \
  gtk3-devel libepoxy-devel wayland-devel mesa-libEGL-devel \
  mpv-devel clang-devel nodejs pnpm
sudo dnf group install c-development
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

pnpm install
scripts/check-deps.sh                  # build + runtime prerequisites
pnpm tauri dev
```

The dev package is **`mpv-devel`** (not `mpv-libs-devel`). If `pnpm` is not packaged:
`sudo npm install -g pnpm`.

**Codecs.** Stock Fedora cannot decode HEVC, and its Mesa VA-API drivers lack patented codecs. For
real testing, enable RPM Fusion:

```bash
sudo dnf install \
  https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm \
  https://mirrors.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-$(rpm -E %fedora).noarch.rpm
sudo dnf swap ffmpeg-free ffmpeg --allowerasing
sudo dnf install mesa-va-drivers-freeworld      # AMD / Intel Arc
sudo dnf install intel-media-driver             # Intel iGPU
sudo dnf install akmod-nvidia xorg-x11-drv-nvidia-cuda   # NVIDIA
sudo dnf install libva-utils vulkan-tools       # vainfo, vulkaninfo
scripts/check-deps.sh --runtime
```

> The Linux render backend (`src-tauri/src/video/render_gtk.rs`) is still a placeholder until spike
> S2 runs on bare-metal Fedora. See IMPLEMENTATION_PLAN §3.

### 1.4 Development data never touches your own config

`pnpm tauri dev` sets `VIHPLUH_HOME=./.local` (gitignored), and a debug build without any override uses
`<repo>/.local` too. Your installed vihpluh's settings stay untouched. Never commit anything personal:
config, presets, playlists, databases, caches, or media names.

---

## 2. Scripts

| Command | What it does |
|---|---|
| `pnpm install` | Frontend dependencies. |
| `pnpm dev` | Vite dev server only (the frontend, without the shell). |
| `pnpm build` | `tsc --noEmit` + Vite production build of the frontend. |
| `pnpm tauri dev` | Runs the app with hot reload, with `VIHPLUH_HOME=./.local`. |
| `pnpm tauri build` | Release bundle: NSIS installer (Windows) in `src-tauri/target/release/bundle/nsis/`, RPM (Fedora) in `…/bundle/rpm/`. |
| `pnpm package:portable` | Windows, after `pnpm tauri build`: `vihpluh-<version>-windows-x64-portable.zip` + `.sha256` in `…/bundle/portable/`. |
| `pnpm check` | `biome check .` + `tsc --noEmit` + `vitest run`. |
| `pnpm gen:bindings` | Regenerates `src/ipc/bindings/` from the Rust types (runs the ts-rs export tests). |
| `pnpm gen:notices` | Regenerates `THIRD_PARTY_NOTICES.md` from `cargo metadata` and `pnpm licenses`. Run after changing dependencies. |
| `pnpm fetch:libmpv` | Windows: downloads and verifies the pinned libmpv into `src-tauri/lib/`. |
| `scripts/check-deps.sh [--runtime]` | Fedora: lists missing build / runtime packages, HEVC decoders, VA-API profiles, NVIDIA driver, session type. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all` | Format Rust. |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | Lint Rust. |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Rust tests. |

Rust tests live in the library's unit-test modules, not in `src-tauri/tests/` (a separate integration
test binary cannot start; ARCHITECTURE D17).

---

## 3. Architecture invariants

Break none of these. Reviews check them first.

1. **Rust owns truth.** Player state, queue, config, presets, bookmarks, and library live in the Rust
   core. The frontend holds only view state (open panels, hover, drafts). No playback logic in React.
2. **mpv is touched in exactly one place:** `src-tauri/src/player/`. Nothing else calls libmpv, and the
   frontend never sends raw mpv commands (the only escape hatch is the `mpv.command` action, routed
   through the core).
3. **vihpluh's queue is authoritative.** mpv's internal playlist is not a source of truth.
4. **Tauri APIs are imported only inside `src/ipc/`.** Components call typed wrappers from
   `src/ipc/commands.ts` / `src/ipc/events.ts`.
5. **Every user-triggerable behaviour is an action** with an ID in the action registry. Buttons,
   menus, the context menu, shortcuts, and the command palette dispatch the same action. Never
   duplicate handler logic.
6. **Every setting is a typed key in the config schema** with a default, a plain-English label, and a
   tooltip. No hidden settings, and no `localStorage` for durable state.
7. **Platform code lives behind traits** in `src-tauri/src/video/` and `src-tauri/src/platform/`. No
   `#[cfg(target_os)]` scattered through feature code, and no platform sniffing in components (use the
   `capabilities` object from `app_info`).
8. **Never block the UI thread or the mpv event thread.** Disk, database, hashing, scanning,
   thumbnailing, and waveform decoding run on worker threads.
9. **No telemetry, analytics, auto-update, or remote fonts and scripts.** The webview loads only bundled
   assets. Outbound network traffic happens only for user-opened streams.
10. **Personal data never enters the repo.** Shipped defaults live in `src-tauri/resources/`.

### Code conventions (short form)

- **Rust:** `thiserror` enums per module, converted to `AppError { code, message }` at the command
  boundary. No `unwrap()` / `expect()` outside tests and startup invariants. `unsafe` only in
  `player/mpv.rs`, `video/*`, `platform/*`, each block with a `// SAFETY:` comment. Logging via
  `tracing`, with paths only at `debug` and never file contents.
- **TypeScript:** `strict`, no `any`, no unexplained `!`. Function components and hooks. One Zustand
  store per domain in `src/stores/`, selected narrowly. High-frequency values (position, drags) go
  through refs and `requestAnimationFrame`, not React state.
- **Styling:** Tailwind utilities mapped to design tokens. No raw hex colours, no arbitrary radii, no
  pill shapes (radius is 4 px for controls and 6 px for surfaces).
- **Controls:** keyboard reachable, visible focus ring, `aria-label`, and a tooltip with the current
  shortcut.
- **Comments** explain *why* (constraints, mpv quirks, platform workarounds), never restate code.
- **No new dependency** without a one-line justification in the commit body.

---

## 4. How to…

### 4.1 Add a user action

1. **Define it** in the matching `src/actions/definitions/<domain>.ts` array (`playbackActions`,
   `queueActions`, `subtitleActions`, …), registered by `src/actions/index.ts`:

   For example, from `src/actions/definitions/media.ts`:

   ```ts
   {
     id: "video.toggle_deband",          // <domain>.<verb_object>; panel actions use the panel's prefix
     label: "Debanding on/off",          // menu / palette / hotkey-editor text
     group: "video",
     icon: Sparkles,
     isOn: () => Boolean(video()?.deband),   // check mark in menus
     run: () => {
       const on = !video()?.deband;
       return adjust(                    // sends the change to the core over IPC
         { deband: on },
         { icon: Sparkles, label: on ? "Debanding on" : "Debanding off" },  // OSD feedback
       );
     },
   },
   ```

   Optional fields: `isEnabled` (a disabled action does nothing at all) and `repeatable` (true only for
   steps: seek, volume, speed, delay, zoom). `run` calls typed IPC wrappers, and the state change
   happens in the Rust core.
2. **Document it**: add a row to the right table in KEYBINDS_AND_CONFIG §2 (ID, label, default keys
   or "—").
3. **Bind it (optional)** in `src-tauri/resources/keymaps/vihpluh.toml` under `[bind]`, only if it
   deserves a default key. If VLC or mpv has an equivalent, add it to `keymaps/vlc.toml` /
   `keymaps/mpv.toml` too.
4. **Surface it** in a menu (`src/features/chrome/menus.ts`) only if it is essential. The command palette
   and the hotkey editor pick it up automatically.
5. `src/actions/actions.test.ts` fails if the shipped keymap names an action the registry lacks, or if an
   action has no label or a panel action is scoped to the wrong panel.

Parameterised actions (for `[[custom]]` bindings only) are handled in `src/actions/parameterised.ts`
and listed in KEYBINDS_AND_CONFIG §3.4.

### 4.2 Add a setting

Every key appears in four places, and the schema-parity test (`src/features/settings/schema.test.ts`)
fails if any of them disagree:

1. **Rust:** add the field to its section struct in `src-tauri/src/config/schema.rs` (with serde
   default handling like its neighbours).
2. **Default:** add `key = value` to the same table in `src-tauri/resources/default.toml`.
3. **Docs:** add a row to the matching table in KEYBINDS_AND_CONFIG §5: key, type, default, **label**
   (≤ 6 words, plain English, no mpv option names) and **tooltip** (≤ 2 sentences: what it does and when
   to change it). Mark ⟳ if it needs a restart.
4. **UI schema:** add the entry in `src/features/settings/schema.ts` with the same label and tooltip,
   a `control`, `keywords`, and `restart` / `platforms` where they apply.
5. **Wire it:** the owning module applies the value on `config://changed` (for mpv options, through
   `src-tauri/src/player/options.rs`), unless it is marked restart-only.
6. Run `pnpm gen:bindings` (the `Config` type is generated), then `pnpm check` and `cargo test`.

The test checks that every Rust key has a schema entry and vice versa, that every default matches
`default.toml`, and that every label, tooltip, and restart mark matches the tables in §5.

### 4.3 Add an IPC command or event

1. **Types.** Put DTOs in `src-tauri/src/ipc/dto.rs` (or next to their module) with
   `#[derive(Serialize/Deserialize, TS)]`, `#[serde(rename_all = "camelCase")]`, and `#[ts(export)]`.
2. **Command.** Add a function to the domain's file under `src-tauri/src/ipc/commands/`:

   ```rust
   // Shape only. `(async)` runs it off the main thread: use it for anything touching disk.
   #[tauri::command(async)]
   pub fn thing_do(state: State<'_, AppState>, id: i64) -> AppResult<ThingResult> {
       // validate input, call the owning module, convert its error into AppError
   }
   ```

   Register it in the `invoke_handler` list in `src-tauri/src/lib.rs`.
3. **Event.** Add the name constant to `src-tauri/src/ipc/events.rs` (`"<domain>://<what>"`) and emit
   it with a typed payload.
4. **Generate:** `pnpm gen:bindings` writes `src/ipc/bindings/*.ts`. Never hand-edit those files.
5. **Wrap:** add a typed wrapper to `src/ipc/commands.ts`
   (`export const thingDo = (id: number) => invoke<ThingResult>("thing_do", { id });`) or to
   `src/ipc/events.ts` (`listen<Payload>("thing://changed", handler)`). Components import only these
   wrappers.
6. **Document** the command or event in ARCHITECTURE §7.3 / §7.4.

---

## 5. Before you commit

### 5.1 Gates (all must pass with zero warnings)

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
pnpm check
```

### 5.2 Definition of done

1. The gates above pass.
2. The behaviour was exercised in the running app (`pnpm tauri dev`), not just compiled. Anything touching
   playback is tested with a real file (the test-media list is in IMPLEMENTATION_PLAN §2; the files
   themselves are not in the repo).
3. A new action, setting, or shortcut is in its schema **and** in KEYBINDS_AND_CONFIG.
4. Platform-sensitive changes (video surface, window, hwdec, paths) say in the commit body which of
   Windows / Fedora-Wayland / Fedora-X11 were actually tested. Untested platforms are named as untested.
5. A checkbox in IMPLEMENTATION_PLAN is ticked only when its verification was observed.

### 5.3 Commit messages

[Conventional Commits](https://www.conventionalcommits.org), imperative, subject ≤ 72 characters,
body explaining *why*:

```
fix(seekbar): keep the drag when pointer capture is lost

Windows WebView2 drops capture when the pointer leaves the window
mid-drag, which ended the scrub early.

Tested: Windows. Untested: Fedora Wayland, Fedora X11.
```

Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`, `chore`. Scopes follow the
domain (`queue`, `subtitles`, `library`, `window`, `config`, …). Keep commits to one logical change,
and do not add generated-by or co-author trailers.

---

## 6. Licensing of contributions

Source code is MIT ([LICENSE](LICENSE)). By contributing you agree that your contribution is released
under the same licence. Windows binaries bundle a GPL libmpv build and are distributed under GPL-3.0-or-later.
See [ARCHITECTURE §13](docs/ARCHITECTURE.md#13-packaging-licensing-security).
