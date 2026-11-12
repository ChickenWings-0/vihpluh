//! Linux Render backend (ARCHITECTURE §5.3) — **placeholder until spike S2 runs on bare-metal
//! Fedora** (IMPLEMENTATION_PLAN §3.2).
//!
//! The real backend renders through `mpv_render_context` into a `GtkGLArea` placed under a
//! transparent WebKitGTK webview inside a `GtkOverlay`. That design is unproven, so no code
//! depends on it yet: this placeholder runs mpv without a video output so the rest of the app
//! (audio, UI, IPC) can be developed and tested on Linux in the meantime.

use super::{Backend, Capabilities, GpuVendor, Renderer, Session, VideoError, VideoSurface};
use crate::config::Config;
use crate::player::mpv::Mpv;

pub struct RenderSurface {
    session: Session,
}

impl RenderSurface {
    pub fn new(_config: &Config) -> Result<Self, VideoError> {
        tracing::warn!(
            "the Linux video backend is not implemented yet (pending spike S2): audio only"
        );
        // Placeholder detection; the real backend asks GDK which backend it actually chose.
        let session = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            Session::Wayland
        } else {
            Session::X11
        };
        Ok(Self { session })
    }
}

impl VideoSurface for RenderSurface {
    fn init_options(&self) -> Vec<(&'static str, String)> {
        vec![("vo", "null".to_string())]
    }

    fn bind(&self, _window: &tauri::WebviewWindow, _mpv: &Mpv) -> Result<(), VideoError> {
        Ok(())
    }

    fn detach(&self) {}

    fn capabilities(&self) -> Capabilities {
        let wayland = self.session == Session::Wayland;
        Capabilities {
            backend: Backend::Render,
            session: self.session,
            renderer: Renderer::Gpu,
            hdr_passthrough: false,
            window_positioning: !wayland,
            always_on_top: !wayland,
            native_decorations: !wayland,
            gpu_vendor: GpuVendor::Other,
        }
    }
}
