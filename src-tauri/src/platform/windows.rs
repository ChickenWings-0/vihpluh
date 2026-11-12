use std::os::windows::io::IntoRawHandle;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle,
};

pub fn attach_parent_console() {
    // SAFETY: plain console API calls. AttachConsole fails harmlessly when there is no parent
    // console or one is already attached (debug builds), in which case nothing else is done.
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            return;
        }
        // A GUI-subsystem process starts with null standard handles; point them at the console.
        for id in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            if let Ok(console) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
                let _ = SetStdHandle(id, HANDLE(console.into_raw_handle()));
            }
        }
    }
}
