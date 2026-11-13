//! Tauri command handlers, one file per domain.
//!
//! Handlers that only queue work for mpv return immediately. Anything that can wait on mpv
//! or touch the disk is marked `#[tauri::command(async)]` so it runs off the main thread.

pub mod app;
pub mod audio;
pub mod config;
#[cfg(debug_assertions)]
pub mod debug;
pub mod keybinds;
pub mod library;
pub mod media;
pub mod player;
pub mod queue;
pub mod session;
pub mod sub;
pub mod tracks;
pub mod video;
pub mod window;
