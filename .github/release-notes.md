## Downloads

| File | For |
|---|---|
| `vihpluh_*_x64-setup.exe` | Windows 10 / 11 — installs for the current user, no administrator rights needed |
| `vihpluh-*-windows-x64-portable.zip` | Windows 10 / 11 — unzip anywhere; settings and library stay in the folder |
| `vihpluh-*.x86_64.rpm` | Fedora — `sudo dnf install ./vihpluh-*.rpm` |

Each file has a `.sha256` next to it.

**Fedora and HEVC.** Stock Fedora cannot decode HEVC. For HEVC playback and hardware decoding,
follow the RPM Fusion steps in the [README](https://github.com/ChickenWings-0/vihpluh#fedora).

## Licence of these binaries

vihpluh's own source code is MIT-licensed.

The **Windows** downloads bundle `libmpv-2.dll` from the
[shinchiro/mpv-winbuild-cmake](https://github.com/shinchiro/mpv-winbuild-cmake) build pinned in
[`scripts/fetch-libmpv.ps1`](https://github.com/ChickenWings-0/vihpluh/blob/main/scripts/fetch-libmpv.ps1).
That build includes GPL-licensed parts of mpv and FFmpeg, so the Windows installer and portable
zip are distributed under the **GNU General Public License, version 3 or later**. Corresponding
source: this repository at this tag, [mpv](https://github.com/mpv-player/mpv),
[FFmpeg](https://git.ffmpeg.org/ffmpeg.git), and the library sources and build recipes in the
repository named above, at the release the script pins.

The **Fedora** package bundles no playback code; it uses the system's `mpv-libs`.

Every bundled component and its licence is listed in
[THIRD_PARTY_NOTICES.md](https://github.com/ChickenWings-0/vihpluh/blob/main/THIRD_PARTY_NOTICES.md).
