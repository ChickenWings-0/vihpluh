//! Hardware-decode chain selection (ARCHITECTURE §5.4).
//!
//! The result is mpv's `hwdec` option: a priority list. mpv tries each entry in order and falls
//! back to software decoding by itself when none works.

use crate::video::{Backend, GpuVendor, Session};

/// Computes the `hwdec` value from the platform facts and the `video.hwdec` setting
/// (`"auto"`, `"off"`, or a raw mpv value passed through verbatim).
pub fn hwdec_chain(backend: Backend, session: Session, vendor: GpuVendor, setting: &str) -> String {
    match setting.trim() {
        "" | "auto" => auto_chain(backend, session, vendor).to_string(),
        "off" | "no" => "no".to_string(),
        custom => custom.to_string(),
    }
}

fn auto_chain(backend: Backend, session: Session, vendor: GpuVendor) -> &'static str {
    match (backend, session) {
        // d3d11va is zero-copy with the D3D11 renderer on every vendor. NVDEC and DXVA2 cannot
        // share surfaces with it, so they are copy-back; plain nvdec stays listed so it goes
        // zero-copy if video.gpu_api = "vulkan".
        (Backend::Embed, _) | (_, Session::Windows) => "d3d11va,nvdec,nvdec-copy,dxva2-copy",
        // Prefer NVDEC directly over the VA-API shim on the NVIDIA proprietary driver.
        (Backend::Render, Session::Wayland | Session::X11) if vendor == GpuVendor::Nvidia => {
            "nvdec,vulkan-copy,nvdec-copy"
        }
        // EGL context: VA-API surfaces import as dmabufs, zero-copy.
        (Backend::Render, Session::Wayland) => "vaapi,vulkan-copy,vaapi-copy",
        // GTK3 on X11 gives a GLX context: no zero-copy VA-API.
        (Backend::Render, Session::X11) => "vaapi-copy,vulkan-copy",
    }
}

/// Whether a `hwdec-current` value means frames stay on the GPU (no copy-back).
pub fn is_zero_copy(hwdec_current: &str) -> bool {
    !hwdec_current.is_empty() && hwdec_current != "no" && !hwdec_current.ends_with("-copy")
}

#[cfg(test)]
mod tests {
    use super::*;

    use Backend::{Embed, Render};
    use GpuVendor::{Amd, Intel, Nvidia, Other};
    use Session::{Wayland, Windows, X11};

    #[test]
    fn windows_uses_d3d11va_first_on_every_vendor() {
        for vendor in [Nvidia, Amd, Intel, Other] {
            assert_eq!(
                hwdec_chain(Embed, Windows, vendor, "auto"),
                "d3d11va,nvdec,nvdec-copy,dxva2-copy"
            );
        }
    }

    #[test]
    fn wayland_amd_and_intel_use_zero_copy_vaapi() {
        for vendor in [Amd, Intel, Other] {
            assert_eq!(
                hwdec_chain(Render, Wayland, vendor, "auto"),
                "vaapi,vulkan-copy,vaapi-copy"
            );
        }
    }

    #[test]
    fn wayland_nvidia_prefers_nvdec() {
        assert_eq!(hwdec_chain(Render, Wayland, Nvidia, "auto"), "nvdec,vulkan-copy,nvdec-copy");
    }

    #[test]
    fn x11_amd_and_intel_use_copy_back_vaapi() {
        for vendor in [Amd, Intel, Other] {
            assert_eq!(hwdec_chain(Render, X11, vendor, "auto"), "vaapi-copy,vulkan-copy");
        }
    }

    #[test]
    fn x11_nvidia_prefers_nvdec() {
        assert_eq!(hwdec_chain(Render, X11, Nvidia, "auto"), "nvdec,vulkan-copy,nvdec-copy");
    }

    #[test]
    fn off_forces_software_everywhere() {
        for (backend, session) in [(Embed, Windows), (Render, Wayland), (Render, X11)] {
            assert_eq!(hwdec_chain(backend, session, Nvidia, "off"), "no");
            assert_eq!(hwdec_chain(backend, session, Amd, "no"), "no");
        }
    }

    #[test]
    fn custom_values_pass_through_verbatim() {
        assert_eq!(hwdec_chain(Embed, Windows, Nvidia, "nvdec-copy"), "nvdec-copy");
        assert_eq!(hwdec_chain(Render, Wayland, Amd, " vulkan,vaapi "), "vulkan,vaapi");
    }

    #[test]
    fn empty_setting_means_auto() {
        assert_eq!(hwdec_chain(Embed, Windows, Amd, ""), hwdec_chain(Embed, Windows, Amd, "auto"));
    }

    #[test]
    fn zero_copy_detection() {
        assert!(is_zero_copy("d3d11va"));
        assert!(is_zero_copy("vaapi"));
        assert!(!is_zero_copy("nvdec-copy"));
        assert!(!is_zero_copy("no"));
        assert!(!is_zero_copy(""));
    }
}
