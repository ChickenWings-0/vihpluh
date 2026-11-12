//! Windows Embed backend (ARCHITECTURE §5.2).
//!
//! We own a host window and hand it to mpv as `wid`; mpv creates its D3D11 swapchain inside it.
//! Once the main window exists the host becomes its bottom-most child, so the transparent
//! WebView2 above composites over the video and receives all input.
//!
//! The host is created *before* the main window (hidden, top-level) so mpv can bring up its
//! video output and open the first file while the webview is still starting; `bind` then
//! reparents it. Owning the host (instead of passing the top-level HWND) also keeps the
//! z-order deterministic. The host must not clip against its siblings — see `adopt`.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
use windows::Win32::Graphics::Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GWL_EXSTYLE, GWL_STYLE, GetClientRect, HWND_BOTTOM,
    RegisterClassExW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SetParent,
    SetWindowLongPtrW, SetWindowPos, WM_SIZE, WNDCLASSEXW, WS_CHILD, WS_CLIPCHILDREN,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::w;

use super::{Backend, Capabilities, GpuVendor, Renderer, Session, VideoError, VideoSurface};
use crate::config::Config;
use crate::config::schema::{AutoOff, GpuApi};
use crate::player::mpv::Mpv;

/// Size of the host before it is adopted; mpv resizes with it afterwards.
const INITIAL_SIZE: (i32, i32) = (1280, 720);

pub struct EmbedSurface {
    /// The host window, stored as an integer so the surface is `Send + Sync`.
    host: isize,
    bound: AtomicBool,
    gpu_api: GpuApi,
    hdr_passthrough: bool,
    gpu_vendor: GpuVendor,
}

impl EmbedSurface {
    /// Must be called on the main thread (the host's messages are pumped by its event loop).
    pub fn new(config: &Config) -> Result<Self, VideoError> {
        let gpu_api = match config.video.gpu_api {
            GpuApi::Auto => GpuApi::D3d11,
            other => other,
        };
        Ok(Self {
            host: create_host()?,
            bound: AtomicBool::new(false),
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

    fn bind(&self, window: &tauri::WebviewWindow, _mpv: &Mpv) -> Result<(), VideoError> {
        if self.bound.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let parent = window.hwnd().map_err(|e| VideoError::Attach(e.to_string()))?;
        adopt(self.host, parent.0 as isize)
    }

    fn detach(&self) {
        // Once adopted the host is destroyed with the main window; nothing to release here.
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

/// Creates the host as a hidden top-level window that never appears in the taskbar.
fn create_host() -> Result<isize, VideoError> {
    let class_name = w!("vihpluh-video-host");
    // SAFETY: plain Win32 window creation; every pointer passed is valid for its call.
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
        // Returns 0 when the class already exists: fine.
        RegisterClassExW(&class);

        let host = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name,
            w!("vihpluh video"),
            WS_POPUP | WS_CLIPCHILDREN,
            0,
            0,
            INITIAL_SIZE.0,
            INITIAL_SIZE.1,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| VideoError::Create(e.to_string()))?;
        Ok(host.0 as isize)
    }
}

/// Makes the host the bottom-most child of the main window and keeps it filling the client
/// area. Must run on the thread that owns both windows (the main thread).
fn adopt(host: isize, parent: isize) -> Result<(), VideoError> {
    // SAFETY: both windows belong to this thread; the style values are valid window styles.
    unsafe {
        // Per SetParent's contract: switch from popup to child style first.
        let child_style = (WS_CHILD | WS_CLIPCHILDREN).0 as isize;
        SetWindowLongPtrW(hwnd(host), GWL_STYLE, child_style);
        SetWindowLongPtrW(hwnd(host), GWL_EXSTYLE, WS_EX_NOACTIVATE.0 as isize);
        SetParent(hwnd(host), Some(hwnd(parent))).map_err(|e| VideoError::Attach(e.to_string()))?;
        // Windows forces WS_CLIPSIBLINGS onto top-level windows, so the bit survives the first
        // call above; clear it now that the host is a child. It must be off: with it, the
        // WebView2 sibling above (opaque as far as USER is concerned) clips the host's visible
        // region to nothing and DWM culls mpv's swapchain entirely — the window then shows
        // the desktop instead of the video (spike S1).
        SetWindowLongPtrW(hwnd(host), GWL_STYLE, child_style);

        let mut rect = RECT::default();
        GetClientRect(hwnd(parent), &mut rect).map_err(|e| VideoError::Attach(e.to_string()))?;
        // Below the WebView2 child so the transparent web UI composites over the video.
        SetWindowPos(
            hwnd(host),
            Some(HWND_BOTTOM),
            0,
            0,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
        )
        .map_err(|e| VideoError::Attach(e.to_string()))?;
    }
    follow_parent_size(host, parent)
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
        return Err(VideoError::Attach("could not hook the main window's size changes".into()));
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
