//! vihpluh — application wiring (ARCHITECTURE §4.3–§4.4).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod cli;
pub mod config;
pub mod error;
pub mod ipc;
pub mod logging;
pub mod paths;
pub mod platform;
pub mod player;
pub mod video;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use clap::Parser;
use tauri::{AppHandle, Manager, RunEvent, WebviewWindowBuilder};

use cli::Cli;
use config::Config;
use paths::Paths;
use player::Player;
use video::Capabilities;

/// Shared application state, managed by Tauri.
pub struct AppState {
    pub paths: Paths,
    pub config: Config,
    pub player: Arc<Player>,
    pub capabilities: Capabilities,
    revealed: AtomicBool,
}

/// If mpv hangs during shutdown (e.g. on a dead network stream) the process exits anyway.
const SHUTDOWN_WATCHDOG: Duration = Duration::from_secs(2);
/// If the frontend never reports its first paint, the window is shown regardless.
const REVEAL_FALLBACK: Duration = Duration::from_secs(3);

pub fn run() -> ExitCode {
    let wants_console = std::env::args()
        .skip(1)
        .any(|arg| matches!(arg.as_str(), "-h" | "--help" | "-V" | "--version"));
    if wants_console {
        platform::attach_parent_console();
    }

    let mut cli = Cli::parse();
    if cli.version {
        print_version();
        return ExitCode::SUCCESS;
    }
    if let Ok(cwd) = std::env::current_dir() {
        cli.sources = cli::normalize_sources(&cli.sources, &cwd);
    }

    let paths = match Paths::resolve(cli.home.as_deref()) {
        Ok(paths) => paths,
        Err(err) => {
            eprintln!("vihpluh: {err}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = paths.ensure() {
        eprintln!("vihpluh: {err}");
        return ExitCode::FAILURE;
    }

    let loaded = config::load(&paths.config_file());
    let level = cli
        .log_level
        .as_deref()
        .and_then(logging::parse_level)
        .or_else(|| logging::parse_level(loaded.config.general.log_level.as_str()))
        .unwrap_or(tracing::Level::WARN);
    logging::init(&paths.log_file(), level);
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        config = %paths.config.display(),
        data = %paths.data.display(),
        source = ?paths.source,
        "starting"
    );
    for warning in &loaded.warnings {
        tracing::warn!("{warning}");
    }

    // Must precede toolkit initialisation and any thread creation.
    platform::prepare_environment(&loaded.config);

    match launch(cli, paths, loaded.config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("fatal: {err}");
            eprintln!("vihpluh: {err}");
            ExitCode::FAILURE
        }
    }
}

fn launch(cli: Cli, paths: Paths, config: Config) -> Result<(), tauri::Error> {
    let app = tauri::Builder::default()
        .setup(move |app| {
            setup(app, &cli, paths, config)?;
            Ok(())
        })
        .invoke_handler(invoke_handler())
        .build(tauri::generate_context!())?;

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            shutdown(app);
        }
    });
    Ok(())
}

#[cfg(debug_assertions)]
fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        ipc::commands::app::app_info,
        ipc::commands::app::app_ready,
        ipc::commands::app::app_quit,
        ipc::commands::player::player_mpv_command,
        ipc::commands::debug::debug_mpv_get,
    ]
}

#[cfg(not(debug_assertions))]
fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        ipc::commands::app::app_info,
        ipc::commands::app::app_ready,
        ipc::commands::app::app_quit,
        ipc::commands::player::player_mpv_command,
    ]
}

fn setup(
    app: &mut tauri::App,
    cli: &Cli,
    paths: Paths,
    config: Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let window_config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .cloned()
        .ok_or("the main window is missing from tauri.conf.json")?;
    // Webview storage lives under our data root so `--home` / portable mode keep everything
    // in one place.
    let window = WebviewWindowBuilder::from_config(app.handle(), &window_config)?
        .data_directory(paths.data.join("webview"))
        .build()?;

    let surface = video::create(&window, &config)?;
    let capabilities = surface.capabilities();
    let player = Player::start(app.handle().clone(), &config, surface, &cli.mpv_opts)?;

    if let Some(source) = cli.sources.first() {
        let start = cli.start.map(cli::StartPosition::to_mpv);
        player.open(source, start.as_deref(), cli.paused)?;
    }

    app.manage(AppState { paths, config, player, capabilities, revealed: AtomicBool::new(false) });

    let handle = app.handle().clone();
    std::thread::spawn(move || {
        std::thread::sleep(REVEAL_FALLBACK);
        reveal_main_window(&handle);
    });
    Ok(())
}

/// Shows the main window once. Called when the frontend reports its first paint, or by the
/// fallback timer.
pub fn reveal_main_window(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else { return };
    if state.revealed.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        tracing::info!("revealing the main window");
        if let Err(err) = window.show().and_then(|()| window.set_focus()) {
            tracing::warn!("could not show the main window: {err}");
        }
    }
}

fn shutdown(app: &AppHandle) {
    std::thread::spawn(|| {
        std::thread::sleep(SHUTDOWN_WATCHDOG);
        tracing::warn!("shutdown watchdog fired: forcing exit");
        std::process::exit(0);
    });
    if let Some(state) = app.try_state::<AppState>() {
        state.player.shutdown();
    }
}

/// `--version`: app, mpv, and FFmpeg versions plus the video backend of this build.
fn print_version() {
    println!("vihpluh {}", env!("CARGO_PKG_VERSION"));
    match headless_versions() {
        Ok((mpv, ffmpeg)) => {
            println!("{mpv}");
            println!("ffmpeg {ffmpeg}");
        }
        Err(err) => println!("libmpv unavailable: {err}"),
    }
    println!("video backend: {}", video::backend_name());
}

fn headless_versions() -> Result<(String, String), player::mpv::MpvError> {
    let mpv = player::mpv::Mpv::create()?;
    for (name, value) in [("vo", "null"), ("ao", "null"), ("config", "no"), ("terminal", "no")] {
        mpv.set_option(name, value)?;
    }
    mpv.initialize()?;
    Ok((
        mpv.get_string("mpv-version").unwrap_or_default(),
        mpv.get_string("ffmpeg-version").unwrap_or_default(),
    ))
}
