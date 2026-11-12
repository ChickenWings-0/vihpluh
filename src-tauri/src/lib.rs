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
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewWindowBuilder};

use cli::Cli;
use config::schema::OpenBehavior;
use config::store::{ConfigChange, ConfigStore};
use ipc::dto::{ConfigChanged, OpenEvent, OpenMode, OsdEvent, OsdKind};
use ipc::events;
use paths::Paths;
use player::Player;
use video::Capabilities;

/// Shared application state, managed by Tauri.
pub struct AppState {
    pub paths: Paths,
    pub config: Arc<ConfigStore>,
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

    let (store, warnings) = ConfigStore::open(paths.config_file());
    let config = store.get();
    let level = cli
        .log_level
        .as_deref()
        .and_then(logging::parse_level)
        .or_else(|| logging::parse_level(config.general.log_level.as_str()))
        .unwrap_or(tracing::Level::WARN);
    logging::init(&paths.log_file(), level);
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        config = %paths.config.display(),
        data = %paths.data.display(),
        source = ?paths.source,
        "starting"
    );
    for warning in &warnings {
        tracing::warn!("{warning}");
    }

    let context = tauri::generate_context!();

    // A second launch only hands its arguments to the running instance. Detect that before
    // starting mpv, so the hand-off never opens (and briefly plays) the file here.
    if config.general.single_instance
        && platform::other_instance_running(&context.config().identifier)
    {
        tracing::info!("another instance is running: forwarding the arguments to it");
        return match forward_to_running_instance(context) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("vihpluh: {err}");
                ExitCode::FAILURE
            }
        };
    }

    // Must precede toolkit initialisation and any thread creation.
    platform::prepare_environment(&config);

    match launch(cli, paths, store, context) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("fatal: {err}");
            eprintln!("vihpluh: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Builds a bare app whose only job is the single-instance plugin: it sends our argv to the
/// running instance and exits the process during plugin setup.
fn forward_to_running_instance(
    context: tauri::Context<tauri::Wry>,
) -> Result<(), Box<dyn std::error::Error>> {
    let _app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
        .build(context)?;
    // Reaching this line means the other instance exited between the check and the hand-off.
    Err("the running instance closed during start-up; start vihpluh again".into())
}

fn launch(
    cli: Cli,
    paths: Paths,
    store: Arc<ConfigStore>,
    context: tauri::Context<tauri::Wry>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = store.get();
    // mpv starts first: its video output and the first file come up while Tauri and the
    // webview (the slow part of startup) are still initialising.
    let surface = video::create(&config)?;
    let capabilities = surface.capabilities();
    let player =
        Player::start(config.clone(), surface, &paths.default_screenshot_dir(), &cli.mpv_opts)?;
    if !cli.sources.is_empty() {
        player.open_cli(cli.sources.clone(), cli.start, cli.paused, OpenMode::Replace);
    }

    let state =
        AppState { paths, config: store, player, capabilities, revealed: AtomicBool::new(false) };
    let mut builder = tauri::Builder::default();
    if config.general.single_instance {
        builder = builder.plugin(tauri_plugin_single_instance::init(on_second_instance));
    }
    let app = builder
        .manage(state)
        .setup(|app| {
            setup(app)?;
            Ok(())
        })
        .invoke_handler(invoke_handler())
        .build(context)?;

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            shutdown(app);
        }
    });
    Ok(())
}

macro_rules! commands {
    ($($extra:path),* $(,)?) => {
        tauri::generate_handler![
            ipc::commands::app::app_info,
            ipc::commands::app::app_ready,
            ipc::commands::app::app_quit,
            ipc::commands::player::player_snapshot,
            ipc::commands::player::player_open,
            ipc::commands::player::player_set_paused,
            ipc::commands::player::player_toggle_pause,
            ipc::commands::player::player_seek,
            ipc::commands::player::player_frame_step,
            ipc::commands::player::player_set_speed,
            ipc::commands::player::player_set_volume,
            ipc::commands::player::player_set_muted,
            ipc::commands::player::player_set_direction,
            ipc::commands::player::player_set_loop_file,
            ipc::commands::player::player_set_ab_loop,
            ipc::commands::player::player_stop,
            ipc::commands::player::player_screenshot,
            ipc::commands::player::player_mpv_command,
            ipc::commands::tracks::tracks_select,
            ipc::commands::config::config_get,
            ipc::commands::config::config_set,
            ipc::commands::config::config_reset,
            $($extra),*
        ]
    };
}

#[cfg(debug_assertions)]
fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    commands![ipc::commands::debug::debug_mpv_get]
}

#[cfg(not(debug_assertions))]
fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    commands![]
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let state = app.state::<AppState>();
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
        .data_directory(state.paths.data.join("webview"))
        .build()?;
    state.player.attach(app.handle().clone(), &window, platform::native_window_handle(&window))?;

    // Settings changes — from the UI or from an editor saving config.toml — apply live.
    let (player, handle) = (state.player.clone(), app.handle().clone());
    state.config.on_change(move |change: &ConfigChange| {
        player.config_changed(change.config.clone());
        for (path, value) in &change.changed {
            let payload = ConfigChanged { path: path.clone(), value: value.clone() };
            let _ = handle.emit(events::CONFIG_CHANGED, &payload);
        }
        for warning in &change.new_warnings {
            let osd = OsdEvent::new(OsdKind::Warn, format!("Settings: {warning}"));
            let _ = handle.emit(events::OSD_SHOW, &osd);
        }
    });
    if let Err(err) = state.config.watch() {
        tracing::warn!("config.toml will not be reloaded automatically: {err}");
    }

    let handle = app.handle().clone();
    std::thread::spawn(move || {
        std::thread::sleep(REVEAL_FALLBACK);
        reveal_main_window(&handle);
    });
    Ok(())
}

/// A second `vihpluh` launch forwarded its command line (ARCHITECTURE §4.6).
fn on_second_instance(app: &AppHandle, argv: Vec<String>, cwd: String) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let cli = match Cli::try_parse_from(&argv) {
        Ok(cli) => cli,
        Err(err) => {
            tracing::warn!("ignoring a forwarded command line that does not parse: {err}");
            return;
        }
    };
    let sources = cli::normalize_sources(&cli.sources, std::path::Path::new(&cwd));
    tracing::info!(count = sources.len(), "second instance forwarded its command line");
    if sources.is_empty() {
        bring_to_front(app);
        return;
    }
    let mode = if cli.enqueue {
        OpenMode::Enqueue
    } else if cli.play_next {
        OpenMode::PlayNext
    } else {
        match state.config.get().general.open_behavior {
            OpenBehavior::Replace => OpenMode::Replace,
            OpenBehavior::Enqueue => OpenMode::Enqueue,
            OpenBehavior::PlayNext => OpenMode::PlayNext,
        }
    };
    state.player.open_cli(sources.clone(), cli.start, cli.paused, mode);
    let _ = app.emit(events::APP_OPEN, &OpenEvent { sources, mode });
    if mode == OpenMode::Replace {
        bring_to_front(app);
    }
}

fn bring_to_front(app: &AppHandle) {
    #[cfg(debug_assertions)]
    if std::env::var_os("VIHPLUH_QUIET_TEST").is_some() {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Shows the main window once. Called when the frontend reports its first paint, or by the
/// fallback timer.
pub fn reveal_main_window(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else { return };
    if state.revealed.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        tracing::debug!("revealing the main window");
        // Development verification runs must not take focus from the person at the keyboard.
        #[cfg(debug_assertions)]
        if std::env::var_os("VIHPLUH_QUIET_TEST").is_some()
            && platform::show_window_unobtrusively(&window)
        {
            return;
        }
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
