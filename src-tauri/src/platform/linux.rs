use crate::config::Config;
use crate::config::schema::AutoOnOff;

pub fn prepare_environment(config: &Config) {
    let mut vars: Vec<(String, String)> = Vec::new();
    if config.video.linux_force_xwayland {
        vars.push(("GDK_BACKEND".into(), "x11".into()));
    }
    let disable_dmabuf = match config.performance.webkit_dmabuf {
        AutoOnOff::On => false,
        AutoOnOff::Off => true,
        // Known WebKitGTK blank/garbled-webview bug with the NVIDIA proprietary driver.
        AutoOnOff::Auto => std::path::Path::new("/proc/driver/nvidia/version").exists(),
    };
    if disable_dmabuf {
        vars.push(("WEBKIT_DISABLE_DMABUF_RENDERER".into(), "1".into()));
    }
    // The unsupported [env] passthrough goes last so it can override the above.
    vars.extend(config.env.iter().map(|(k, v)| (k.clone(), v.clone())));

    for (name, value) in vars {
        tracing::info!("env {name}={value}");
        // SAFETY: called from main() before any other thread is started.
        unsafe { std::env::set_var(name, value) };
    }
}
