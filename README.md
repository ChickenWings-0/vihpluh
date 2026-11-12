# vihpluh

A fast, keyboard-driven desktop media player: **mpv's playback engine** under a compact, modern interface
inspired by [Juxtopposed's VLC redesign](https://youtu.be/ERFeOgqk324). Built for Windows 10/11 and Fedora Linux (Wayland and X11).

> **Status: pre-alpha — design complete, implementation starting.**
> This repository currently contains the full technical blueprint (see [Documentation](#documentation)).
> There is no runnable build yet. The build instructions below describe the target workflow and become
> valid when Phase 1 of the [implementation plan](docs/IMPLEMENTATION_PLAN.md) lands.

vihpluh is a personal tool first: no telemetry, no accounts, no onboarding, no update nags, and no
network traffic unless you open a stream. It's public so anyone who finds it useful can build it.

## What it will do

- **Plays everything mpv plays**, with hardware decoding tuned for high-bitrate 4K HEVC / AV1
  (D3D11VA on Windows; VA-API, NVDEC, or Vulkan Video on Linux) and automatic software fallback.
- **Chrome that gets out of the way** — auto-hiding top and bottom bars, a single-row control bar, and a one-key Zen mode that hides everything.
- **A seek bar you can actually grab** — thick, with frame previews on hover, chapter marks, bookmarks, and an optional audio waveform.
- **A right sidebar of live controls**
  - *Subtitles:* delay slider, a visual timeline to line subtitles up against the audio waveform, font / size / outline / position, one-click matching of subtitle files to a whole folder.
  - *Audio:* track and output-device switching, sync delay, 10-band and parametric equalizer with visual presets.
  - *Video:* brightness / contrast / saturation, flip, rotate, zoom, aspect ratio, HDR tone mapping, shaders — all previewed live.
  - *Playback:* speed, frame stepping, A–B loop, sleep timer, stream and codec inspector.
  - *Bookmarks:* `Ctrl+B` to drop one; chapters and bookmarks in a single timeline.
- **Per-show presets** — save subtitle delay, styling, preferred tracks, and picture settings once; they apply automatically to every episode in the series or folder.
- **Queue ≠ playlist ≠ library** — a transient play queue, saved playlists, and a compact library drawer (videos, music, network streams / IPTV `.m3u`, recents, pinned folders) you can browse without interrupting playback.
- **Readable settings** — ten flat, searchable categories in plain English, plus a visual hotkey editor. Everything is stored in clean TOML you can edit by hand.

## Stack

| | |
|---|---|
| Shell | [Tauri 2](https://v2.tauri.app) — one small native binary, system webview |
| Core | Rust |
| Playback | [libmpv](https://mpv.io) (in-process) |
| Interface | TypeScript, React, Tailwind CSS |
| Data | TOML config / presets, JSON playlists, SQLite library |

The reasoning, including the alternatives considered, is in [ARCHITECTURE §2](docs/ARCHITECTURE.md#2-stack-decision).

## Building

> Target workflow — see the status note above.

### Windows 10 / 11

```powershell
winget install --id Rustlang.Rustup
winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install --id OpenJS.NodeJS.LTS
winget install --id pnpm.pnpm
winget install --id LLVM.LLVM
winget install --id 7zip.7zip

git clone https://github.com/ChickenWings-0/vihpluh.git
cd vihpluh
pnpm install
pnpm fetch:libmpv        # downloads the pinned libmpv build into src-tauri/lib/
pnpm tauri dev           # run
pnpm tauri build         # installer in src-tauri/target/release/bundle/
```

No administrator rights for the Visual Studio Build Tools? The GNU Rust toolchain works too:
`rustup toolchain install stable-x86_64-pc-windows-gnu` plus a MinGW-w64 GCC on `PATH`
(for example MSYS2's `mingw-w64-x86_64-gcc`). Details: [ARCHITECTURE §12.1](docs/ARCHITECTURE.md#121-windows-10--11-x64).

### Fedora

```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel \
  gtk3-devel libepoxy-devel wayland-devel mesa-libEGL-devel \
  mpv-devel clang-devel nodejs pnpm
sudo dnf group install c-development
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

git clone https://github.com/ChickenWings-0/vihpluh.git
cd vihpluh
pnpm install
pnpm tauri dev
pnpm tauri build         # RPM in src-tauri/target/release/bundle/rpm/
```

**Codecs on Fedora.** Stock Fedora cannot decode HEVC and ships VA-API drivers without patented codecs.
For HEVC playback and hardware decoding, enable [RPM Fusion](https://rpmfusion.org/Configuration) and run:

```bash
sudo dnf swap ffmpeg-free ffmpeg --allowerasing
sudo dnf install mesa-va-drivers-freeworld      # AMD (Fedora ≤ 43: dnf swap mesa-va-drivers mesa-va-drivers-freeworld)
sudo dnf install intel-media-driver             # Intel
sudo dnf install akmod-nvidia xorg-x11-drv-nvidia-cuda   # NVIDIA
```

`scripts/check-deps.sh` reports anything missing. Full dependency details: [ARCHITECTURE §12](docs/ARCHITECTURE.md#12-build-and-runtime-dependencies).

## Configuration

vihpluh ships sensible defaults and stores only what you change, outside the repository:

| | Windows | Linux |
|---|---|---|
| Settings, keybinds, presets, playlists | `%APPDATA%\vihpluh\` | `~/.config/vihpluh/` |
| Library database, session | `%LOCALAPPDATA%\vihpluh\data\` | `~/.local/share/vihpluh/` |
| Thumbnail / waveform cache | `%LOCALAPPDATA%\vihpluh\cache\` | `~/.cache/vihpluh/` |

Run with `--home <dir>` (or drop a file named `portable` next to the executable) to keep everything in one folder.
Every setting, action, and default shortcut is documented in [KEYBINDS_AND_CONFIG](docs/KEYBINDS_AND_CONFIG.md).

A few defaults to get started: `Space` play/pause · `←` `→` seek · `F` fullscreen · `Z` Zen mode · `Q` queue ·
`\` sidebar · `Ctrl+L` library · `Ctrl+B` bookmark · `G` / `H` subtitle delay · `F1` all shortcuts.

## Documentation

| Document | Contents |
|---|---|
| [ARCHITECTURE](docs/ARCHITECTURE.md) | Stack decision, process model, video pipelines and hardware-decode chains, IPC contract, storage schemas, presets, dependencies |
| [DESIGN_SYSTEM](docs/DESIGN_SYSTEM.md) | Colour and type tokens, geometry, icons, layout metrics, component and panel specs, motion, accessibility |
| [KEYBINDS_AND_CONFIG](docs/KEYBINDS_AND_CONFIG.md) | Action registry, default shortcuts, key notation, hotkey editor, every setting with its label and default |
| [IMPLEMENTATION_PLAN](docs/IMPLEMENTATION_PLAN.md) | Twelve phases with task checklists and verification criteria |

## Credits

- Interface concept inspired by **Juxtopposed**'s VLC redesign, ["I Redesigned VLC After 25 Years"](https://youtu.be/ERFeOgqk324).
- Playback by **[mpv](https://mpv.io)** and **[FFmpeg](https://ffmpeg.org)**.
- vihpluh is an independent project. It is not affiliated with or endorsed by VideoLAN, and VLC is a trademark of VideoLAN.

## License

Source code: [MIT](LICENSE).

Binary releases for Windows bundle libmpv, which is typically built under the GPL; those binaries are
distributed under the corresponding terms. See [ARCHITECTURE §13](docs/ARCHITECTURE.md#13-packaging-licensing-security).
