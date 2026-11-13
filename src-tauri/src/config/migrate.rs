//! Schema migrations for the user's TOML files (ARCHITECTURE §9.3). `config.toml` and
//! `keybinds.toml` carry `schema = <int>`; an older file is upgraded in place, step by step,
//! after a copy of it is written next to it as `<name>.bak`. Comments and ordering survive:
//! steps edit the document, they do not rewrite it.
//!
//! Schema 1 is the first schema, so there is nothing to migrate yet — the list of steps is
//! empty and a v1 file is left alone. The machinery is here (and tested) so the first real
//! change of a key's name or meaning is one function added to a list.

use std::path::{Path, PathBuf};

use toml_edit::DocumentMut;

/// One upgrade: takes a document at schema `from` to `from + 1`.
pub struct Step {
    pub from: u32,
    pub apply: fn(&mut DocumentMut),
}

/// `config.toml` upgrades, oldest first. Empty: schema 1 is the first.
pub const CONFIG_STEPS: &[Step] = &[];
/// `keybinds.toml` upgrades, oldest first.
pub const KEYBIND_STEPS: &[Step] = &[];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// No file, or already at the current schema.
    UpToDate,
    Migrated {
        from: u32,
        to: u32,
        backup: PathBuf,
    },
    /// Written by a newer vihpluh: left untouched (unknown keys are preserved and reported
    /// by the loader as usual).
    Newer(u32),
    /// Not valid TOML: left for the loader to report.
    Unreadable,
}

pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".bak");
    path.with_file_name(name)
}

/// The schema a document declares. A file without the key predates it: schema 1.
fn declared_schema(doc: &DocumentMut) -> u32 {
    doc.get("schema").and_then(toml_edit::Item::as_integer).map_or(1, |n| n.max(0) as u32)
}

/// Brings the file at `path` up to `current` using `steps`. The original is copied to
/// `<name>.bak` before anything is changed.
pub fn migrate_file(path: &Path, current: u32, steps: &[Step]) -> std::io::Result<Outcome> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Outcome::UpToDate),
        Err(err) => return Err(err),
    };
    let Ok(mut doc) = text.parse::<DocumentMut>() else { return Ok(Outcome::Unreadable) };
    let from = declared_schema(&doc);
    if from == current {
        return Ok(Outcome::UpToDate);
    }
    if from > current {
        return Ok(Outcome::Newer(from));
    }
    let backup = backup_path(path);
    std::fs::write(&backup, &text)?;
    for version in from..current {
        if let Some(step) = steps.iter().find(|step| step.from == version) {
            (step.apply)(&mut doc);
        }
    }
    doc["schema"] = toml_edit::value(i64::from(current));
    crate::media::thumbs::write_atomic(path, doc.to_string().as_bytes())?;
    Ok(Outcome::Migrated { from, to: current, backup })
}

/// Runs a file's migrations at startup and logs what happened. Never fails startup.
pub fn run(path: &Path, current: u32, steps: &[Step]) {
    match migrate_file(path, current, steps) {
        Ok(Outcome::Migrated { from, to, backup }) => tracing::info!(
            "{} upgraded from schema {from} to {to}; the old file is kept as {}",
            path.display(),
            backup.display()
        ),
        Ok(Outcome::Newer(schema)) => tracing::warn!(
            "{} was written by a newer vihpluh (schema {schema}); it is used as far as it is understood",
            path.display()
        ),
        Ok(Outcome::UpToDate | Outcome::Unreadable) => {}
        Err(err) => tracing::warn!("{} could not be upgraded: {err}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, text: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vihpluh-migrate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, text).unwrap();
        path
    }

    /// An invented history: schema 0 called the volume key `start_volume`, and schema 1
    /// stored the seek step in milliseconds.
    const TEST_STEPS: &[Step] = &[
        Step {
            from: 0,
            apply: |doc| {
                let Some(audio) = doc.get_mut("audio").and_then(toml_edit::Item::as_table_mut)
                else {
                    return;
                };
                if let Some(value) = audio.remove("start_volume") {
                    audio.insert("default_volume", value);
                }
            },
        },
        Step {
            from: 1,
            apply: |doc| {
                let Some(item) = doc.get_mut("controls").and_then(|c| c.get_mut("seek_small_ms"))
                else {
                    return;
                };
                if let Some(ms) = item.as_integer() {
                    *item = toml_edit::value(ms as f64 / 1000.0);
                }
            },
        },
    ];

    const OLD: &str = "# my settings\nschema = 0\n\n[audio]\nstart_volume = 80  # quieter\n\n[controls]\nseek_small_ms = 5000\n";

    #[test]
    fn an_old_file_is_backed_up_then_upgraded_in_place() {
        let path = temp_file("old", OLD);
        let outcome = migrate_file(&path, 2, TEST_STEPS).unwrap();
        let backup = backup_path(&path);
        assert_eq!(outcome, Outcome::Migrated { from: 0, to: 2, backup: backup.clone() });
        assert_eq!(backup.file_name().unwrap(), "config.toml.bak");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), OLD, "the backup is the old file");
        let new = std::fs::read_to_string(&path).unwrap();
        assert!(new.starts_with("# my settings\nschema = 2\n"), "{new}");
        assert!(new.contains("default_volume = 80  # quieter"), "{new}");
        assert!(!new.contains("start_volume"), "{new}");
        assert!(new.contains("seek_small_ms = 5.0"), "{new}");
        // A second run finds nothing to do and leaves the backup alone.
        assert_eq!(migrate_file(&path, 2, TEST_STEPS).unwrap(), Outcome::UpToDate);
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), OLD);
    }

    #[test]
    fn version_one_is_a_no_op_with_the_shipped_steps() {
        let text = "schema = 1\n[interface]\naccent = \"#3E8BFF\"\n";
        let path = temp_file("current", text);
        let current = crate::config::schema::SCHEMA_VERSION;
        assert_eq!(migrate_file(&path, current, CONFIG_STEPS).unwrap(), Outcome::UpToDate);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        assert!(!backup_path(&path).exists(), "nothing changed, nothing backed up");
        // A file that never had the key is schema 1 too.
        let bare = temp_file("bare", "[interface]\ntheme = \"light\"\n");
        assert_eq!(migrate_file(&bare, current, CONFIG_STEPS).unwrap(), Outcome::UpToDate);
        assert!(CONFIG_STEPS.is_empty() && KEYBIND_STEPS.is_empty());
    }

    #[test]
    fn newer_missing_and_broken_files_are_left_alone() {
        let newer = temp_file("newer", "schema = 9\n");
        assert_eq!(migrate_file(&newer, 2, TEST_STEPS).unwrap(), Outcome::Newer(9));
        assert_eq!(std::fs::read_to_string(&newer).unwrap(), "schema = 9\n");
        assert!(!backup_path(&newer).exists());

        let missing = newer.with_file_name("nothing.toml");
        assert_eq!(migrate_file(&missing, 2, TEST_STEPS).unwrap(), Outcome::UpToDate);

        let broken = temp_file("broken", "schema = \n[[[\n");
        assert_eq!(migrate_file(&broken, 2, TEST_STEPS).unwrap(), Outcome::Unreadable);
        assert!(!backup_path(&broken).exists());
    }

    #[test]
    fn a_gap_in_the_steps_still_reaches_the_current_schema() {
        let path = temp_file("gap", "schema = 0\n");
        let only_second: &[Step] = &[Step { from: 1, apply: |_| {} }];
        assert!(matches!(
            migrate_file(&path, 3, only_second).unwrap(),
            Outcome::Migrated { from: 0, to: 3, .. }
        ));
        assert!(std::fs::read_to_string(&path).unwrap().contains("schema = 3"));
    }
}
