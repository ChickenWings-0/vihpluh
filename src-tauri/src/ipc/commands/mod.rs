//! Tauri command handlers, one file per domain.
//!
//! Handlers that only queue work for mpv return immediately. Anything that can wait on mpv
//! or touch the disk is marked `#[tauri::command(async)]` so it runs off the main thread.

pub mod app;
pub mod config;
#[cfg(debug_assertions)]
pub mod debug;
pub mod player;
pub mod tracks;
