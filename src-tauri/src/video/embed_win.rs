//! Windows Embed backend (ARCHITECTURE §5.2).
//!
//! We own a child window at the bottom of the top-level window's z-order and hand it to mpv as
//! `wid`. mpv creates its D3D11 swapchain inside it; the transparent WebView2 sits above and
//! receives all input. Owning the host (instead of passing the top-level HWND) keeps the
//! z-order deterministic. The host must not clip against its siblings — see `create_host`.

use std::ffi::c_void;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
use windows::Win32::Graphics::Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, HWND_BOTTOM, RegisterClassExW, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos, WM_SIZE, WNDCLASSEXW, WS_CHILD,
    WS_CLIPCHILDREN, WS_EX_NOACTIVATE, WS_VISIBLE,
};
use windows::core::w;

use super::{Backend, Capabilities, GpuVendor, Renderer, Session, VideoError, VideoSurface};
use crate::config::Config;
use crate::config::schema::{AutoOff, GpuApi};
use crate::player::mpv::Mpv;

pub struct EmbedSurface {
    /// The host child window, stored as an integer so the surface is `Send + Sync`.
    host: isize,
    gpu_api: GpuApi,
    hdr_passthrough: bool,
    gpu_vendor: GpuVendor,
}

impl EmbedSurface {
    pub fn new(window: &tauri::WebviewWindow, config: &Config) -> Result<Self, VideoError> {
        let parent = window.hwnd().map_err(|e| VideoError::Create(e.to_string()))?;
        let parent = parent.0 as isize;
        let host = create_host(parent)?;

        follow_parent_size(host, parent)?;

        let gpu_api = match config.video.gpu_api {
            GpuApi::Auto => GpuApi::D3d11,
            other => other,
        };
        Ok(Self {
            host,
            gpu_api,
            hdr_passthrough: config.video.hdr_passthrough == AutoOff::Auto
                && gpu_api == GpuApi::D3d11,
            gpu_vendor: primary_gpu_vendor(),
        })
    }
}

impl VideoSurface for EmbedSurface {
    fn init_options(&self) -> Vec<(&'static str, String)> {
        let (api, context) = match self.gpu_api {
            GpuApi::Vulkan => ("vulkan", "winvk"),
            GpuApi::Opengl => ("opengl", "win"),
            GpuApi::Auto | GpuApi::D3d11 => ("d3d11", "d3d11"),
        };
        vec![
            ("vo", "gpu-next".to_string()),
            ("gpu-api", api.to_string()),
            ("gpu-context", context.to_string()),
            ("wid", self.host.to_string()),
            (
                "target-colorspace-hint",
                if self.hdr_passthrough { "auto" } else { "no" }.to_string(),
            ),
        ]
    }

    fn attach(&self, _mpv: &Mpv) -> Result<(), VideoError> {
        // Nothing to do: mpv created its window inside the host during initialisation.
        Ok(())
    }

    fn detach(&self) {
        // The host is a child of the main window and is destroyed with it.
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            backend: Backend::Embed,
            session: Session::Windows,
            renderer: Renderer::GpuNext,
            hdr_passthrough: self.hdr_passthrough,
            window_positioning: true,
            always_on_top: true,
            native_decorations: true,
            gpu_vendor: self.gpu_vendor,
        }
    }
}

fn hwnd(value: isize) -> HWND {
    HWND(value as *mut c_void)
}

unsafe extern "system" fn host_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: forwarding the unmodified arguments of a window procedure call.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

fn create_host(parent: isize) -> Result<isize, VideoError> {
    let class_name = w!("vihpluh-video-host");
    // SAFETY: plain Win32 window creation on the thread that owns `parent` (the main thread);
    // every pointer passed is valid for the duration of its call.
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| VideoError::Create(e.to_string()))?;
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(host_proc),
            hInstance: instance.into(),
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            lpszClassName: class_name,
            ..Default::default()
        };
        // Returns 0 when the class already exists (second window in the same process): fine.
        RegisterClassExW(&class);

        let mut rect = RECT::default();
        GetClientRect(hwnd(parent), &mut rect).map_err(|e| VideoError::Create(e.to_string()))?;

        // No WS_CLIPSIBLINGS: with it, the WebView2 sibling above (opaque as far as USER is
        // concerned) clips the host's visible region to nothing and DWM culls mpv's swapchain
        // entirely — the window then shows the desktop instead of the video (spike S1).
        let host = CreateWindowExW(
            WS_EX_NOACTIVATE,
            class_name,
            w!("vihpluh video"),
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
            0,
            0,
            rect.right - rect.left,
            rect.bottom - rect.top,
            Some(hwnd(parent)),
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| VideoError::Create(e.to_string()))?;

        // Below the WebView2 child so the transparent web UI composites over the video.
        SetWindowPos(host, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)
            .map_err(|e| VideoError::Create(e.to_string()))?;
        Ok(host.0 as isize)
    }
}

/// Subclass id for the size-tracking hook on the main window.
const SUBCLASS_ID: usize = 0x7668_706c; // "vhpl"

/// Keeps the host filling the client area by hooking the parent's `WM_SIZE` directly; this
/// catches every resize (including the ones the toolkit performs before its own events are
/// delivered). mpv's window in turn follows its parent, the host.
fn follow_parent_size(host: isize, parent: isize) -> Result<(), VideoError> {
    // SAFETY: called on the thread that owns `parent`; the callback only uses the handles
    // passed to it and the host handle carried in `ref_data`.
    let installed =
        unsafe { SetWindowSubclass(hwnd(parent), Some(parent_proc), SUBCLASS_ID, host as usize) };
    if !installed.as_bool() {
        return Err(VideoError::Create("could not hook the main window's size changes".into()));
    }
    fit_to_parent(host, parent);
    Ok(())
}

unsafe extern "system" fn parent_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    ref_data: usize,
) -> LRESULT {
    if message == WM_SIZE {
        fit_to_parent(ref_data as isize, window.0 as isize);
    }
    // SAFETY: forwarding the unmodified arguments down the subclass chain.
    unsafe { DefSubclassProc(window, message, wparam, lparam) }
}

fn fit_to_parent(host: isize, parent: isize) {
    // SAFETY: both handles belong to this process; failure (window already destroyed during
    // shutdown) is harmless and ignored.
    unsafe {
        let mut rect = RECT::default();
        if GetClientRect(hwnd(parent), &mut rect).is_ok() {
            let _ = SetWindowPos(
                hwnd(host),
                None,
                0,
                0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

/// Vendor of the first DXGI adapter (the one D3D11 picks by default).
fn primary_gpu_vendor() -> GpuVendor {
    // SAFETY: standard DXGI enumeration; COM objects are released when dropped.
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else {
            return GpuVendor::Other;
        };
        let Ok(adapter) = factory.EnumAdapters1(0) else {
            return GpuVendor::Other;
        };
        match adapter.GetDesc1() {
            Ok(desc) => GpuVendor::from_pci_id(desc.VendorId),
            Err(_) => GpuVendor::Other,
        }
    }
}
