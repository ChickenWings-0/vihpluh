//! Windows side of `platform::taskbar`: `ITaskbarList3` thumbnail buttons on the main window.
//!
//! The buttons can only be added once the shell has created the window's taskbar button; it
//! announces that with the registered `TaskbarButtonCreated` message (again after Explorer
//! restarts), so the main window is subclassed to hear it and the clicks (`WM_COMMAND` with
//! `THBN_CLICKED`).

use std::cell::{Cell, RefCell};
use std::ffi::c_void;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Shell::{
    DefSubclassProc, ITaskbarList3, SetWindowSubclass, THB_FLAGS, THB_ICON, THB_TOOLTIP,
    THBF_ENABLED, THBN_CLICKED, THUMBBUTTON, TaskbarList,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, GetSystemMetrics, HICON, ICONINFO, RegisterWindowMessageW, SM_CXSMICON,
    WM_COMMAND,
};
use windows::core::w;

use super::media::MediaCommand;
use super::taskbar::{Button, Glyph, rasterize};

/// Subclass id for the taskbar hook on the main window.
const SUBCLASS_ID: usize = 0x7668_7462; // "vhtb"

struct Toolbar {
    window: isize,
    list: ITaskbarList3,
    icons: [HICON; 4],
    created_message: u32,
    added: Cell<bool>,
    playing: Cell<bool>,
    on_click: Box<dyn Fn(MediaCommand)>,
}

thread_local! {
    /// Lives as long as the main thread: the window it serves does too.
    static TOOLBAR: RefCell<Option<Toolbar>> = const { RefCell::new(None) };
}

fn hwnd(value: isize) -> HWND {
    HWND(value as *mut c_void)
}

pub fn install(window: &tauri::WebviewWindow, on_click: Box<dyn Fn(MediaCommand)>) -> bool {
    let Ok(handle) = window.hwnd() else { return false };
    let handle = handle.0 as isize;
    // SAFETY: COM is initialised on the main thread by the toolkit; the calls below are
    // plain shell and GDI calls with valid arguments, made on the window's own thread.
    let toolbar = unsafe {
        let Ok(list) =
            CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_INPROC_SERVER)
        else {
            tracing::debug!("taskbar: no ITaskbarList3; the thumbnail buttons are off");
            return false;
        };
        if list.HrInit().is_err() {
            return false;
        }
        let size = GetSystemMetrics(SM_CXSMICON).clamp(16, 64) as usize;
        let dark_glyphs = taskbar_is_light();
        let icon = |glyph| make_icon(&rasterize(glyph, size), size, dark_glyphs);
        let (Some(previous), Some(play), Some(pause), Some(next)) =
            (icon(Glyph::Previous), icon(Glyph::Play), icon(Glyph::Pause), icon(Glyph::Next))
        else {
            tracing::debug!("taskbar: could not draw the button icons");
            return false;
        };
        Toolbar {
            window: handle,
            list,
            icons: [previous, play, pause, next],
            created_message: RegisterWindowMessageW(w!("TaskbarButtonCreated")),
            added: Cell::new(false),
            playing: Cell::new(false),
            on_click,
        }
    };
    // The taskbar button may exist already (the window is usually shown by now).
    toolbar.add_buttons();
    TOOLBAR.with(|slot| *slot.borrow_mut() = Some(toolbar));
    // SAFETY: called on the thread that owns the window; the procedure only reads the
    // thread-local above.
    let hooked = unsafe { SetWindowSubclass(hwnd(handle), Some(window_proc), SUBCLASS_ID, 0) };
    hooked.as_bool()
}

pub fn set_playing(playing: bool) {
    TOOLBAR.with(|slot| {
        if let Some(toolbar) = slot.borrow().as_ref() {
            if toolbar.playing.replace(playing) != playing {
                toolbar.update_toggle();
            }
        }
    });
}

impl Toolbar {
    fn button(&self, button: Button) -> THUMBBUTTON {
        let (icon, tip) = match button {
            Button::Previous => (self.icons[0], "Previous"),
            Button::Toggle if self.playing.get() => (self.icons[2], "Pause"),
            Button::Toggle => (self.icons[1], "Play"),
            Button::Next => (self.icons[3], "Next"),
        };
        let mut thumb = THUMBBUTTON {
            dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
            iId: button as u32,
            hIcon: icon,
            dwFlags: THBF_ENABLED,
            ..Default::default()
        };
        for (slot, unit) in thumb.szTip.iter_mut().zip(tip.encode_utf16()) {
            *slot = unit;
        }
        thumb
    }

    fn add_buttons(&self) {
        let buttons =
            [self.button(Button::Previous), self.button(Button::Toggle), self.button(Button::Next)];
        // SAFETY: `list` is a live COM object used on its own thread; `buttons` outlives
        // the call. Adding fails harmlessly while the taskbar button does not exist yet.
        match unsafe { self.list.ThumbBarAddButtons(hwnd(self.window), &buttons) } {
            Ok(()) => {
                self.added.set(true);
                tracing::debug!("taskbar: thumbnail buttons added");
            }
            Err(err) => tracing::debug!("taskbar: buttons not added yet ({err})"),
        }
    }

    fn update_toggle(&self) {
        if !self.added.get() {
            return;
        }
        let buttons = [self.button(Button::Toggle)];
        // SAFETY: as in `add_buttons`.
        if let Err(err) = unsafe { self.list.ThumbBarUpdateButtons(hwnd(self.window), &buttons) } {
            tracing::debug!("taskbar: button update failed: {err}");
        }
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _ref_data: usize,
) -> LRESULT {
    // `try_borrow`: a click handler that re-enters the message loop must not panic here
    // (no panics across FFI).
    TOOLBAR.with(|slot| {
        let Ok(guard) = slot.try_borrow() else { return };
        let Some(toolbar) = guard.as_ref() else { return };
        if message == toolbar.created_message {
            // First creation, or Explorer restarted: the buttons have to be added (again).
            toolbar.added.set(false);
            toolbar.add_buttons();
        } else if message == WM_COMMAND && ((wparam.0 >> 16) & 0xffff) as u32 == THBN_CLICKED {
            if let Some(button) = Button::from_id((wparam.0 & 0xffff) as u32) {
                (toolbar.on_click)(button.command());
            }
        }
    });
    // SAFETY: forwarding the unmodified arguments down the subclass chain.
    unsafe { DefSubclassProc(window, message, wparam, lparam) }
}

/// Whether the taskbar uses the light theme (then the glyphs are drawn dark).
fn taskbar_is_light() -> bool {
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: reads one DWORD into `value`, whose size is passed alongside.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&raw mut size),
        )
    };
    status.is_ok() && value == 1
}

/// Builds an icon from per-pixel coverage: white (or near-black) with the coverage as alpha.
///
/// # Safety
/// GDI calls only; the bitmaps are deleted before returning (the icon keeps its own copies).
unsafe fn make_icon(coverage: &[u8], size: usize, dark: bool) -> Option<HICON> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            // Negative height: rows run from the top, as `coverage` does.
            biHeight: -(size as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    // SAFETY: `info` describes a 32-bit bitmap of size × size; `bits` receives its pixels.
    let color =
        unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) }.ok()?;
    if bits.is_null() || coverage.len() != size * size {
        // SAFETY: `color` is a bitmap we own.
        let _ = unsafe { DeleteObject(color.into()) };
        return None;
    }
    let level: u32 = if dark { 0x1a } else { 0xff };
    // SAFETY: the DIB section holds exactly size × size 32-bit pixels.
    let pixels = unsafe { std::slice::from_raw_parts_mut(bits.cast::<u32>(), size * size) };
    for (pixel, &alpha) in pixels.iter_mut().zip(coverage) {
        // Premultiplied BGRA.
        let channel = level * u32::from(alpha) / 255;
        *pixel = (u32::from(alpha) << 24) | (channel << 16) | (channel << 8) | channel;
    }
    // SAFETY: a monochrome mask of the same size; with per-pixel alpha its bits are unused.
    let mask = unsafe { CreateBitmap(size as i32, size as i32, 1, 1, None) };
    let icon_info =
        ICONINFO { fIcon: true.into(), hbmMask: mask, hbmColor: color, ..Default::default() };
    // SAFETY: both bitmaps are valid; the icon copies them.
    let icon = unsafe { CreateIconIndirect(&icon_info) }.ok();
    // SAFETY: we own both bitmaps and no longer need them.
    unsafe {
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
    }
    icon
}
