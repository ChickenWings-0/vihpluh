//! Bookmarks (ARCHITECTURE §9.6 `bookmarks`): the user's own marks on a piece of media, keyed
//! by `media_key` so they survive renaming or moving the file.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use crate::library::db::unix_now;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Bookmark {
    #[ts(type = "number")]
    pub id: i64,
    pub media_key: String,
    /// Seconds from the start.
    pub position: f64,
    pub label: String,
    #[ts(type = "number")]
    pub created_at: i64,
}

/// `bookmarks://changed` — the bookmarks of this media were added to, edited, or removed.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BookmarksChanged {
    pub media_key: String,
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Bookmark> {
    Ok(Bookmark {
        id: row.get(0)?,
        media_key: row.get(1)?,
        position: row.get(2)?,
        label: row.get(3)?,
        created_at: row.get(4)?,
    })
}

const COLUMNS: &str = "id, media_key, position, label, created_at";

/// In time order.
pub fn list(conn: &Connection, media_key: &str) -> rusqlite::Result<Vec<Bookmark>> {
    let mut statement = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM bookmarks WHERE media_key = ?1 ORDER BY position, id"
    ))?;
    let rows = statement.query_map(params![media_key], row)?;
    rows.collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Bookmark>> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM bookmarks WHERE id = ?1"), params![id], row)
        .optional()
}

/// "Bookmark N", where N is one more than the highest number already used on this media.
pub fn next_label(conn: &Connection, media_key: &str) -> rusqlite::Result<String> {
    let mut statement = conn.prepare("SELECT label FROM bookmarks WHERE media_key = ?1")?;
    let labels = statement.query_map(params![media_key], |row| row.get::<_, String>(0))?;
    let highest = labels
        .filter_map(Result::ok)
        .filter_map(|label| label.strip_prefix("Bookmark ")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    Ok(format!("Bookmark {}", highest + 1))
}

/// Adds a bookmark. An empty or missing label becomes the next "Bookmark N".
pub fn add(
    conn: &Connection,
    media_key: &str,
    position: f64,
    label: Option<&str>,
) -> rusqlite::Result<Bookmark> {
    let label = match label.map(str::trim).filter(|l| !l.is_empty()) {
        Some(label) => label.to_string(),
        None => next_label(conn, media_key)?,
    };
    let position = if position.is_finite() { position.max(0.0) } else { 0.0 };
    let created_at = unix_now();
    conn.execute(
        "INSERT INTO bookmarks (media_key, position, label, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![media_key, position, label, created_at],
    )?;
    Ok(Bookmark {
        id: conn.last_insert_rowid(),
        media_key: media_key.to_string(),
        position,
        label,
        created_at,
    })
}

/// Changes the label and/or position. Returns the bookmark as it now is (`None`: no such id).
pub fn update(
    conn: &Connection,
    id: i64,
    label: Option<&str>,
    position: Option<f64>,
) -> rusqlite::Result<Option<Bookmark>> {
    if let Some(label) = label.map(str::trim).filter(|l| !l.is_empty()) {
        conn.execute("UPDATE bookmarks SET label = ?2 WHERE id = ?1", params![id, label])?;
    }
    if let Some(position) = position.filter(|p| p.is_finite()) {
        conn.execute(
            "UPDATE bookmarks SET position = ?2 WHERE id = ?1",
            params![id, position.max(0.0)],
        )?;
    }
    get(conn, id)
}

/// Removes a bookmark and returns it (for undo).
pub fn remove(conn: &Connection, id: i64) -> rusqlite::Result<Option<Bookmark>> {
    let existing = get(conn, id)?;
    conn.execute("DELETE FROM bookmarks WHERE id = ?1", params![id])?;
    Ok(existing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db::Db;

    #[test]
    fn bookmarks_are_listed_in_time_order_with_auto_labels() {
        let db = Db::in_memory().unwrap();
        let (list_a, list_b) = db
            .call(|c| {
                add(c, "f:a", 1421.0, None)?;
                add(c, "f:a", 192.5, Some("  That one shot "))?;
                add(c, "f:a", 600.0, Some(""))?;
                add(c, "f:b", 10.0, None)?;
                add(c, "f:a", -5.0, None)?;
                Ok((list(c, "f:a")?, list(c, "f:b")?))
            })
            .unwrap();
        let labels: Vec<(&str, f64)> =
            list_a.iter().map(|b| (b.label.as_str(), b.position)).collect();
        assert_eq!(
            labels,
            vec![
                ("Bookmark 3", 0.0),
                ("That one shot", 192.5),
                ("Bookmark 2", 600.0),
                ("Bookmark 1", 1421.0)
            ]
        );
        assert_eq!(list_b.len(), 1);
        assert_eq!(list_b[0].label, "Bookmark 1", "numbering is per media");
    }

    #[test]
    fn rename_move_and_remove() {
        let db = Db::in_memory().unwrap();
        db.call(|c| {
            let first = add(c, "f:a", 10.0, None)?;
            let second = add(c, "f:a", 20.0, None)?;
            let renamed = update(c, first.id, Some("Opening"), None)?.unwrap();
            assert_eq!((renamed.label.as_str(), renamed.position), ("Opening", 10.0));
            let moved = update(c, first.id, Some("   "), Some(30.0))?.unwrap();
            assert_eq!((moved.label.as_str(), moved.position), ("Opening", 30.0));
            assert_eq!(update(c, 999, Some("x"), None)?, None);

            // Numbers are not reused while a higher one exists.
            assert_eq!(remove(c, first.id)?.map(|b| b.label), Some("Opening".to_string()));
            assert_eq!(next_label(c, "f:a")?, "Bookmark 3");
            assert_eq!(remove(c, second.id)?.map(|b| b.id), Some(second.id));
            assert_eq!(remove(c, second.id)?, None);
            assert_eq!(next_label(c, "f:a")?, "Bookmark 1");
            assert!(list(c, "f:a")?.is_empty());
            Ok(())
        })
        .unwrap();
    }
}
