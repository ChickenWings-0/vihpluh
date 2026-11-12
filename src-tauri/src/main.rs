// Release builds are GUI-subsystem apps on Windows: no console window flashes on launch.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    vihpluh_lib::run()
}
