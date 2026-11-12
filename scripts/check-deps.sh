#!/usr/bin/env bash
# Verifies the Fedora build and runtime prerequisites for vihpluh (ARCHITECTURE §12.2).
# Prints what is missing and exits non-zero if anything required is absent.
#
#   scripts/check-deps.sh            check build + runtime
#   scripts/check-deps.sh --runtime  check runtime only (codecs, hardware decoding)

set -u

RUNTIME_ONLY=0
[ "${1:-}" = "--runtime" ] && RUNTIME_ONLY=1

missing=()
warnings=()

ok()   { printf '  \033[32mok\033[0m    %s\n' "$1"; }
bad()  { printf '  \033[31mMISSING\033[0m %s\n' "$1"; missing+=("$1${2:+ — $2}"); }
warn() { printf '  \033[33mwarn\033[0m  %s\n' "$1"; warnings+=("$1${2:+ — $2}"); }
have() { command -v "$1" >/dev/null 2>&1; }

need_rpm() {
  if rpm -q "$1" >/dev/null 2>&1; then ok "$1"; else bad "$1" "sudo dnf install $1"; fi
}

if ! have rpm || ! have dnf; then
  echo "This script targets Fedora (rpm/dnf not found)." >&2
  exit 2
fi

echo "Session"
session="${XDG_SESSION_TYPE:-unknown}"
case "$session" in
  wayland) ok "Wayland session${WAYLAND_DISPLAY:+ ($WAYLAND_DISPLAY)}" ;;
  x11)     ok "X11 session (${DISPLAY:-no DISPLAY})" ;;
  *)       warn "session type is '$session'" "run from a graphical session to check hardware decoding" ;;
esac
echo "  desktop: ${XDG_CURRENT_DESKTOP:-unknown}"

if [ "$RUNTIME_ONLY" -eq 0 ]; then
  echo
  echo "Build packages"
  for pkg in webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel \
             librsvg2-devel libxdo-devel gtk3-devel libepoxy-devel wayland-devel \
             mesa-libEGL-devel mpv-devel clang-devel nodejs; do
    need_rpm "$pkg"
  done
  if have gcc && have make; then ok "C toolchain (gcc, make)"; else bad "C toolchain" "sudo dnf group install c-development"; fi
  if have pkg-config && pkg-config --exists mpv; then
    ok "libmpv $(pkg-config --modversion mpv) via pkg-config"
  else
    bad "pkg-config mpv" "sudo dnf install mpv-devel"
  fi

  echo
  echo "Toolchain"
  if have rustc && have cargo; then ok "$(rustc --version)"; else bad "Rust" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"; fi
  if have node; then ok "node $(node --version)"; else bad "Node.js" "sudo dnf install nodejs"; fi
  if have pnpm; then ok "pnpm $(pnpm --version)"; else bad "pnpm" "sudo dnf install pnpm  (or: sudo npm install -g pnpm)"; fi
fi

echo
echo "Runtime packages"
for pkg in mpv-libs webkit2gtk4.1 gtk3 libepoxy; do need_rpm "$pkg"; done

echo
echo "Codecs"
if rpm -q ffmpeg-free >/dev/null 2>&1 || rpm -q libavcodec-free >/dev/null 2>&1; then
  bad "full FFmpeg (ffmpeg-free is installed: no HEVC)" \
      "enable RPM Fusion, then: sudo dnf swap ffmpeg-free ffmpeg --allowerasing"
fi
if have ffmpeg; then
  decoders="$(ffmpeg -hide_banner -decoders 2>/dev/null)"
  for codec in hevc h264 av1 vp9; do
    if printf '%s\n' "$decoders" | grep -Eq "^ V[.A-Z]{5} +(lib)?${codec}[a-z0-9_]* "; then
      ok "decoder: $codec"
    elif [ "$codec" = hevc ] || [ "$codec" = h264 ]; then
      bad "decoder: $codec" "sudo dnf swap ffmpeg-free ffmpeg --allowerasing (RPM Fusion)"
    else
      warn "decoder: $codec not found"
    fi
  done
else
  warn "ffmpeg command not found" "cannot list decoders; install ffmpeg from RPM Fusion to check"
fi

echo
echo "Hardware decoding"
nvidia=0
if [ -e /proc/driver/nvidia/version ]; then
  nvidia=1
  if have nvidia-smi && nvidia-smi >/dev/null 2>&1; then
    ok "NVIDIA driver $(nvidia-smi --query-gpu=driver_version --format=csv,noheader 2>/dev/null | head -1)"
  else
    warn "NVIDIA kernel module loaded but nvidia-smi failed" "sudo dnf install xorg-x11-drv-nvidia-cuda"
  fi
  if rpm -q xorg-x11-drv-nvidia-cuda-libs >/dev/null 2>&1 || rpm -q xorg-x11-drv-nvidia-cuda >/dev/null 2>&1; then
    ok "NVDEC libraries (xorg-x11-drv-nvidia-cuda)"
  else
    bad "NVDEC libraries" "sudo dnf install xorg-x11-drv-nvidia-cuda"
  fi
fi

if [ "$nvidia" -eq 0 ]; then
  if have vainfo; then
    profiles="$(vainfo 2>/dev/null)"
    if [ -z "$profiles" ]; then
      warn "vainfo produced no output" "VA-API driver missing or no graphical session"
    else
      printf '%s\n' "$profiles" | grep -q "VAProfileHEVCMain10" \
        && ok "VA-API: HEVC Main 10" \
        || bad "VA-API: HEVC Main 10 profile" "AMD: sudo dnf install mesa-va-drivers-freeworld · Intel: sudo dnf install intel-media-driver"
      printf '%s\n' "$profiles" | grep -q "VAProfileH264" \
        && ok "VA-API: H.264" \
        || bad "VA-API: H.264 profile" "AMD: sudo dnf install mesa-va-drivers-freeworld · Intel: sudo dnf install intel-media-driver"
      printf '%s\n' "$profiles" | grep -q "VAProfileAV1" \
        && ok "VA-API: AV1" \
        || warn "VA-API: no AV1 profile" "older GPUs decode AV1 in software (dav1d)"
    fi
  else
    warn "vainfo not installed" "sudo dnf install libva-utils"
  fi
fi

echo
if [ "${#warnings[@]}" -gt 0 ]; then
  echo "Warnings:"
  for w in "${warnings[@]}"; do echo "  - $w"; done
fi
if [ "${#missing[@]}" -gt 0 ]; then
  echo "Missing (${#missing[@]}):"
  for m in "${missing[@]}"; do echo "  - $m"; done
  exit 1
fi
echo "All required dependencies are present."
