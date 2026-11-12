//! The video surface abstraction (ARCHITECTURE §5.1).
//!
//! libmpv can present video two ways and neither works everywhere, so there are two backends
//! behind one trait: Embed (`--wid`, Windows) and Render (render API into a GL widget, Linux).
//! Nothing outside this module knows which one is active except through [`Capabilities`].

use std::sync::Arc;

use serde::Serialize;
use ts_rs::TS;

use crate::config::Config;
use crate::player::mpv::Mpv;

#[cfg(windows)]
mod embed_win;
#[cfg(target_os = "linux")]
mod render_gtk;

#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    #[error("could not create the video surface: {0}")]
    Create(String),
    #[error("could not attach the video surface: {0}")]
    Attach(String),
    #[error("{0}")]
    Unsupported(String),
}

pub trait VideoSurface: Send + Sync {
    /// mpv options that must be set before `mpv_initialize` (vo, wid, gpu-api, …).
    fn init_options(&self) -> Vec<(&'static str, String)>;
    /// Called once, on the main thread, when the main window exists (mpv is already
    /// initialised): moves the surface into the window, creating the render context where
    /// applicable.
    fn bind(&self, window: &tauri::WebviewWindow, mpv: &Mpv) -> Result<(), VideoError>;
    /// Tears down GPU resources. Must run before the mpv handle is destroyed.
    fn detach(&self);
    fn capabilities(&self) -> Capabilities;
}

/// What the active backend and session can do; serialised to the frontend via `app_info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Capabilities {
    pub backend: Backend,
    pub session: Session,
    pub renderer: Renderer,
    pub hdr_passthrough: bool,
    pub window_positioning: bool,
    pub always_on_top: bool,
    pub native_decorations: bool,
    pub gpu_vendor: GpuVendor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Backend {
    Embed,
    Render,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Session {
    Windows,
    Wayland,
    X11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Renderer {
    GpuNext,
    Gpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Other,
}

impl GpuVendor {
    /// Maps a PCI vendor id.
    pub fn from_pci_id(id: u32) -> Self {
        match id {
            0x10DE => Self::Nvidia,
            0x1002 | 0x1022 => Self::Amd,
            0x8086 => Self::Intel,
            _ => Self::Other,
        }
    }
}

impl Backend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Embed => "embed",
            Self::Render => "render",
        }
    }
}

/// Creates the surface for this platform. Must be called on the main thread, before mpv is
/// initialised. The main window does not need to exist yet: the surface joins it in
/// [`VideoSurface::bind`], which lets mpv start up in parallel with the webview.
pub fn create(config: &Config) -> Result<Arc<dyn VideoSurface>, VideoError> {
    #[cfg(windows)]
    {
        Ok(Arc::new(embed_win::EmbedSurface::new(config)?))
    }
    #[cfg(target_os = "linux")]
    {
        Ok(Arc::new(render_gtk::RenderSurface::new(config)?))
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = config;
        Err(VideoError::Unsupported("this platform has no video backend".to_string()))
    }
}

/// The backend this build would use, without creating anything (for `--version`).
pub fn backend_name() -> &'static str {
    if cfg!(windows) {
        "embed (native child window, gpu-next / Direct3D 11)"
    } else if cfg!(target_os = "linux") {
        "render (libmpv render API, OpenGL)"
    } else {
        "none"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_pci_vendor_ids() {
        assert_eq!(GpuVendor::from_pci_id(0x10DE), GpuVendor::Nvidia);
        assert_eq!(GpuVendor::from_pci_id(0x1002), GpuVendor::Amd);
        assert_eq!(GpuVendor::from_pci_id(0x8086), GpuVendor::Intel);
        assert_eq!(GpuVendor::from_pci_id(0x1414), GpuVendor::Other);
    }
}
