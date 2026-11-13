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

/// Family names of every installed font, as fontconfig lists them (unsorted, duplicated).
pub fn installed_font_faces() -> Vec<String> {
    let output =
        std::process::Command::new("fc-list").args(["--format", "%{family[0]}\n"]).output();
    match output {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect()
        }
        _ => {
            tracing::warn!("fc-list is unavailable: the font list is empty");
            Vec::new()
        }
    }
}
