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

pub fn single_instance_window_exists(identifier: &str) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
    use windows::core::HSTRING;

    let class = HSTRING::from(format!("{identifier}-sic"));
    let title = HSTRING::from(format!("{identifier}-siw"));
    // SAFETY: FindWindowW only reads the two NUL-terminated strings.
    unsafe { FindWindowW(&class, &title) }.is_ok_and(|h| !h.0.is_null())
}

/// The server end of an inbound, byte-mode named pipe (waveform PCM, see `platform::pipe`).
pub struct NamedPipe {
    handle: HANDLE,
}

// SAFETY: a pipe handle may be used from any thread; it is moved to the reader thread.
unsafe impl Send for NamedPipe {}

impl NamedPipe {
    pub fn create_inbound(path: &str) -> std::io::Result<Self> {
        use windows::Win32::Storage::FileSystem::PIPE_ACCESS_INBOUND;
        use windows::Win32::System::Pipes::{CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_WAIT};
        use windows::core::HSTRING;
        // SAFETY: valid NUL-terminated name; one instance, 1 MiB inbound buffer, default
        // security. The returned handle is owned by `NamedPipe` and closed on drop.
        let handle = unsafe {
            CreateNamedPipeW(
                &HSTRING::from(path),
                PIPE_ACCESS_INBOUND,
                PIPE_TYPE_BYTE | PIPE_WAIT,
                1,
                0,
                1 << 20,
                0,
                None,
            )
        };
        if handle.is_invalid() {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self { handle })
    }

    /// Blocks until a writer connects, then hands the handle over as a `File` to read from.
    pub fn connect(self) -> std::io::Result<std::fs::File> {
        use std::os::windows::io::FromRawHandle;
        use windows::Win32::System::Pipes::ConnectNamedPipe;
        let handle = self.handle;
        std::mem::forget(self);
        // SAFETY: `handle` is a valid pipe server handle. ERROR_PIPE_CONNECTED (the writer
        // connected first) is success too, so the result is ignored and reading decides.
        let _ = unsafe { ConnectNamedPipe(handle, None) };
        // SAFETY: we own the handle (ownership moves to the File, which closes it).
        Ok(unsafe { std::fs::File::from_raw_handle(handle.0) })
    }
}

impl Drop for NamedPipe {
    fn drop(&mut self) {
        // SAFETY: we own the handle and close it once.
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.handle) };
    }
}
