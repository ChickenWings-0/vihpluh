use std::os::windows::io::IntoRawHandle;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
    SetStdHandle,
};

pub fn attach_parent_console() {
    // SAFETY: plain console API calls. AttachConsole fails harmlessly when there is no parent
    // console or one is already attached (debug builds).
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        // A GUI-subsystem process started from a terminal has null standard handles: point
        // those at the console. Handles that are already valid (output redirected to a pipe
        // or file) are left alone.
        for id in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let usable = GetStdHandle(id).is_ok_and(|h| !h.is_invalid() && !h.0.is_null());
            if usable {
                continue;
            }
            if let Ok(console) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
                let _ = SetStdHandle(id, HANDLE(console.into_raw_handle()));
            }
        }
    }
}

#[cfg(debug_assertions)]
pub fn show_window_unobtrusively(window: &tauri::WebviewWindow) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_BOTTOM, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
        ShowWindow,
    };

    let Ok(handle) = window.hwnd() else { return false };
    let handle = HWND(handle.0.cast());
    // SAFETY: `handle` is the live main window, owned by the calling (main) thread.
    unsafe {
        let _ = ShowWindow(handle, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(
            handle,
            Some(HWND_BOTTOM),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
    true
}
