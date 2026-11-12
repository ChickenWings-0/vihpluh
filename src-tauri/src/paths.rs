//! Where vihpluh keeps its files (ARCHITECTURE §9.1).
//!
//! Override order: `--home <DIR>` → `VIHPLUH_HOME` → a `portable` marker beside the executable
//! → (debug builds only) `<repo>/.local` → OS defaults. With any override everything lives under
//! one root: `<root>/{config,data,cache,logs}`.

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

pub const HOME_ENV: &str = "VIHPLUH_HOME";
const APP_DIR: &str = "vihpluh";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
    pub source: HomeSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeSource {
    Cli,
    Env,
    Portable,
    DevDefault,
    Os,
}

/// The four locations as strings, for `app_info`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PathsDto {
    pub config: String,
    pub data: String,
    pub cache: String,
    pub log: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PathsError {
    #[error("could not determine the user's home directories")]
    NoHome,
    #[error("could not create {path}: {source}")]
    Create { path: PathBuf, source: std::io::Error },
}

/// OS-specific base directories, before the app folder is appended.
#[derive(Debug, Clone)]
struct OsDirs {
    config: PathBuf,
    data: PathBuf,
    cache: PathBuf,
    logs: PathBuf,
}

#[derive(Debug, Clone, Default)]
struct Inputs {
    cli_home: Option<PathBuf>,
    env_home: Option<PathBuf>,
    exe_dir: Option<PathBuf>,
    portable_marker: bool,
    dev_default: Option<PathBuf>,
}

impl Paths {
    pub fn resolve(cli_home: Option<&Path>) -> Result<Self, PathsError> {
        let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        let inputs = Inputs {
            cli_home: cli_home.map(absolute),
            env_home: std::env::var_os(HOME_ENV)
                .filter(|v| !v.is_empty())
                .map(|v| absolute(Path::new(&v))),
            portable_marker: exe_dir.as_ref().is_some_and(|d| d.join("portable").is_file()),
            exe_dir,
            dev_default: dev_default(),
        };
        if let Some(paths) = resolve_override(&inputs) {
            return Ok(paths);
        }
        Ok(from_os(os_dirs()?))
    }

    /// Creates the data, cache, and log directories. The config directory is created lazily on
    /// first write so an untouched install leaves no user files behind.
    pub fn ensure(&self) -> Result<(), PathsError> {
        for dir in [&self.data, &self.cache, &self.logs] {
            std::fs::create_dir_all(dir)
                .map_err(|source| PathsError::Create { path: dir.clone(), source })?;
        }
        Ok(())
    }

    pub fn config_file(&self) -> PathBuf {
        self.config.join("config.toml")
    }

    pub fn log_file(&self) -> PathBuf {
        self.logs.join("vihpluh.log")
    }

    /// Where screenshots go when `general.screenshot_dir` is empty: the user's Pictures
    /// folder normally, but inside the home root whenever one is in use (dev builds, `--home`,
    /// portable), so those modes never write into personal folders.
    pub fn default_screenshot_dir(&self) -> PathBuf {
        match (self.source, self.data.parent()) {
            (HomeSource::Os, _) | (_, None) => directories::UserDirs::new()
                .and_then(|dirs| dirs.picture_dir().map(|p| p.join(APP_DIR)))
                .unwrap_or_else(|| self.data.join("screenshots")),
            (_, Some(root)) => root.join("screenshots"),
        }
    }

    pub fn dto(&self) -> PathsDto {
        PathsDto {
            config: self.config.display().to_string(),
            data: self.data.display().to_string(),
            cache: self.cache.display().to_string(),
            log: self.logs.display().to_string(),
        }
    }
}

fn resolve_override(inputs: &Inputs) -> Option<Paths> {
    let (root, source) = if let Some(dir) = &inputs.cli_home {
        (dir.clone(), HomeSource::Cli)
    } else if let Some(dir) = &inputs.env_home {
        (dir.clone(), HomeSource::Env)
    } else if let (true, Some(exe_dir)) = (inputs.portable_marker, &inputs.exe_dir) {
        (exe_dir.join("data"), HomeSource::Portable)
    } else if let Some(dir) = &inputs.dev_default {
        (dir.clone(), HomeSource::DevDefault)
    } else {
        return None;
    };
    Some(Paths {
        config: root.join("config"),
        data: root.join("data"),
        cache: root.join("cache"),
        logs: root.join("logs"),
        source,
    })
}

fn from_os(dirs: OsDirs) -> Paths {
    Paths {
        config: dirs.config,
        data: dirs.data,
        cache: dirs.cache,
        logs: dirs.logs,
        source: HomeSource::Os,
    }
}

#[cfg(windows)]
fn os_dirs() -> Result<OsDirs, PathsError> {
    let base = directories::BaseDirs::new().ok_or(PathsError::NoHome)?;
    let local = base.data_local_dir().join(APP_DIR);
    Ok(OsDirs {
        config: base.config_dir().join(APP_DIR),
        data: local.join("data"),
        cache: local.join("cache"),
        logs: local.join("logs"),
    })
}

#[cfg(not(windows))]
fn os_dirs() -> Result<OsDirs, PathsError> {
    let base = directories::BaseDirs::new().ok_or(PathsError::NoHome)?;
    let state = base
        .state_dir()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| base.home_dir().join(".local").join("state"));
    Ok(OsDirs {
        config: base.config_dir().join(APP_DIR),
        data: base.data_dir().join(APP_DIR),
        cache: base.cache_dir().join(APP_DIR),
        logs: state.join(APP_DIR),
    })
}

/// Debug builds never touch personal config: without an explicit override they use the
/// repository's gitignored `.local/` directory.
fn dev_default() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(|repo| repo.join(".local"))
    } else {
        None
    }
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> Inputs {
        Inputs {
            cli_home: None,
            env_home: None,
            exe_dir: Some(PathBuf::from("/opt/vihpluh")),
            portable_marker: false,
            dev_default: None,
        }
    }

    #[test]
    fn no_override_falls_through_to_os_defaults() {
        assert_eq!(resolve_override(&inputs()), None);
    }

    #[test]
    fn cli_home_wins_over_everything() {
        let paths = resolve_override(&Inputs {
            cli_home: Some(PathBuf::from("/a")),
            env_home: Some(PathBuf::from("/b")),
            portable_marker: true,
            dev_default: Some(PathBuf::from("/d")),
            ..inputs()
        })
        .unwrap();
        assert_eq!(paths.source, HomeSource::Cli);
        assert_eq!(paths.config, PathBuf::from("/a/config"));
        assert_eq!(paths.data, PathBuf::from("/a/data"));
        assert_eq!(paths.cache, PathBuf::from("/a/cache"));
        assert_eq!(paths.logs, PathBuf::from("/a/logs"));
    }

    #[test]
    fn env_home_wins_over_portable_and_dev() {
        let paths = resolve_override(&Inputs {
            env_home: Some(PathBuf::from("/b")),
            portable_marker: true,
            dev_default: Some(PathBuf::from("/d")),
            ..inputs()
        })
        .unwrap();
        assert_eq!(paths.source, HomeSource::Env);
        assert_eq!(paths.config, PathBuf::from("/b/config"));
    }

    #[test]
    fn portable_marker_uses_data_dir_beside_the_executable() {
        let paths = resolve_override(&Inputs {
            portable_marker: true,
            dev_default: Some(PathBuf::from("/d")),
            ..inputs()
        })
        .unwrap();
        assert_eq!(paths.source, HomeSource::Portable);
        assert_eq!(paths.config, PathBuf::from("/opt/vihpluh/data/config"));
        assert_eq!(paths.logs, PathBuf::from("/opt/vihpluh/data/logs"));
    }

    #[test]
    fn dev_default_is_the_last_override() {
        let paths =
            resolve_override(&Inputs { dev_default: Some(PathBuf::from("/d")), ..inputs() })
                .unwrap();
        assert_eq!(paths.source, HomeSource::DevDefault);
        assert_eq!(paths.data, PathBuf::from("/d/data"));
    }

    #[test]
    fn screenshots_stay_inside_an_override_root() {
        let paths = resolve_override(&Inputs {
            dev_default: Some(PathBuf::from("/repo/.local")),
            ..inputs()
        })
        .unwrap();
        assert_eq!(paths.default_screenshot_dir(), PathBuf::from("/repo/.local/screenshots"));
        let os = from_os(os_dirs().unwrap());
        assert!(os.default_screenshot_dir().ends_with(APP_DIR));
    }

    #[test]
    fn os_defaults_keep_data_cache_and_logs_apart() {
        let paths = from_os(os_dirs().unwrap());
        assert_eq!(paths.source, HomeSource::Os);
        assert!(paths.config.ends_with(APP_DIR));
        assert_ne!(paths.data, paths.cache);
        assert_ne!(paths.data, paths.logs);
        assert_ne!(paths.cache, paths.logs);
    }
}
