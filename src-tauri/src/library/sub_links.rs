//! External subtitle associations (ARCHITECTURE §9.6 `sub_links`, §10.5): which subtitle
//! files belong to a piece of media, set by auto-match or by adding a file by hand. They are
//! attached again on every later load of that media.

use rusqlite::{Connection, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubLink {
    pub path: String,
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSource {
    Auto,
    Manual,
}

impl LinkSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }
}

pub fn for_media(conn: &Connection, media_key: &str) -> rusqlite::Result<Vec<SubLink>> {
    let mut statement =
        conn.prepare("SELECT sub_path, lang FROM sub_links WHERE media_key = ?1 ORDER BY rowid")?;
    let rows = statement.query_map(params![media_key], |row| {
        Ok(SubLink { path: row.get(0)?, lang: row.get(1)? })
    })?;
    rows.collect()
}

/// Adds or refreshes a link. A manual link is never downgraded to an automatic one.
pub fn add(
    conn: &Connection,
    media_key: &str,
    link: &SubLink,
    source: LinkSource,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO sub_links (media_key, sub_path, lang, source) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(media_key, sub_path) DO UPDATE SET
           lang = COALESCE(excluded.lang, lang),
           source = CASE WHEN source = 'manual' THEN 'manual' ELSE excluded.source END",
        params![media_key, link.path, link.lang, source.as_str()],
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, media_key: &str, sub_path: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM sub_links WHERE media_key = ?1 AND sub_path = ?2",
        params![media_key, sub_path],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db::Db;

    #[test]
    fn links_are_kept_per_media_and_removable() {
        let db = Db::in_memory().unwrap();
        let links = db
            .call(|c| {
                let en = SubLink { path: "D:/s/Subs/e1.en.srt".into(), lang: Some("eng".into()) };
                let es = SubLink { path: "D:/s/Subs/e1.es.srt".into(), lang: None };
                add(c, "f:1", &en, LinkSource::Manual)?;
                add(c, "f:1", &es, LinkSource::Auto)?;
                // Re-running auto-match neither duplicates nor downgrades.
                add(c, "f:1", &SubLink { lang: None, ..en.clone() }, LinkSource::Auto)?;
                add(c, "f:2", &en, LinkSource::Auto)?;
                remove(c, "f:1", "D:/s/Subs/e1.es.srt")?;
                let source: String = c.query_row(
                    "SELECT source FROM sub_links WHERE media_key = 'f:1'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(source, "manual");
                Ok((for_media(c, "f:1")?, for_media(c, "f:2")?, for_media(c, "f:3")?))
            })
            .unwrap();
        assert_eq!(
            links.0,
            vec![SubLink { path: "D:/s/Subs/e1.en.srt".into(), lang: Some("eng".into()) }]
        );
        assert_eq!(links.1.len(), 1);
        assert_eq!(links.2, vec![]);
    }
}
