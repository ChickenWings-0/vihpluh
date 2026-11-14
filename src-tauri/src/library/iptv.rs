//! Network stream lists (ARCHITECTURE §8.3): the IPTV flavour of M3U — channels with
//! attributes, groups, and per-channel HTTP headers — and the `stream_sources` / `streams`
//! tables that hold them. No programme guide; channel lists only.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::db::unix_now;
use super::index::fts_query;

/// One channel of an M3U list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Channel {
    pub name: String,
    pub url: String,
    pub group: Option<String>,
    pub logo: Option<String>,
    pub tvg_id: Option<String>,
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
}

/// The `key="value"` attributes of an `#EXTINF` line, and the display name after the comma.
fn parse_extinf(info: &str) -> (Vec<(String, String)>, String) {
    let mut attributes = Vec::new();
    let chars: Vec<char> = info.chars().collect();
    let mut i = 0;
    let mut name_at = None;
    while i < chars.len() {
        match chars[i] {
            ',' => {
                name_at = Some(i + 1);
                break;
            }
            c if c.is_alphabetic() => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '-') {
                    i += 1;
                }
                if chars.get(i) == Some(&'=') && chars.get(i + 1) == Some(&'"') {
                    let key: String = chars[start..i].iter().collect();
                    i += 2;
                    let value_start = i;
                    while i < chars.len() && chars[i] != '"' {
                        i += 1;
                    }
                    let value: String = chars[value_start..i.min(chars.len())].iter().collect();
                    attributes.push((key.to_lowercase(), value.trim().to_string()));
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    let name = name_at.map(|at| chars[at..].iter().collect::<String>()).unwrap_or_default();
    (attributes, name.trim().to_string())
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// Parses an IPTV M3U. Lines it cannot make sense of are skipped; a URL without an `#EXTINF`
/// is named after itself.
pub fn parse(text: &str) -> Vec<Channel> {
    let mut channels = Vec::new();
    let mut pending = Channel::default();
    let mut have_info = false;
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            continue;
        }
        if let Some(info) = line.strip_prefix("#EXTINF:") {
            let (attributes, name) = parse_extinf(info);
            pending = Channel { name, ..Channel::default() };
            for (key, value) in attributes {
                match key.as_str() {
                    "group-title" => pending.group = non_empty(&value),
                    "tvg-logo" => pending.logo = non_empty(&value),
                    "tvg-id" => pending.tvg_id = non_empty(&value),
                    "tvg-name" if pending.name.is_empty() => pending.name = value,
                    "user-agent" => pending.user_agent = non_empty(&value),
                    _ => {}
                }
            }
            have_info = true;
        } else if let Some(option) = line.strip_prefix("#EXTVLCOPT:") {
            if let Some((key, value)) = option.split_once('=') {
                match key.trim().to_ascii_lowercase().as_str() {
                    "http-user-agent" => pending.user_agent = non_empty(value),
                    "http-referrer" | "http-referer" => pending.referrer = non_empty(value),
                    _ => {}
                }
            }
        } else if let Some(group) = line.strip_prefix("#EXTGRP:") {
            pending.group = non_empty(group);
        } else if line.starts_with('#') {
            continue;
        } else {
            // Anything that is not a URL (or a path) with a scheme-like start is noise.
            let mut channel = std::mem::take(&mut pending);
            if !have_info {
                // Options before a bare URL still apply to it.
                channel.name = String::new();
            }
            have_info = false;
            if !crate::cli::is_url(line) {
                continue;
            }
            channel.url = line.to_string();
            if channel.name.is_empty() {
                channel.name = line.to_string();
            }
            channels.push(channel);
        }
    }
    channels
}

// ── Storage ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StreamSource {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    /// URL or path of the `.m3u`.
    pub location: String,
    #[ts(type = "number | null")]
    pub refreshed_at: Option<i64>,
    #[ts(type = "number")]
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StreamEntry {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub source_id: i64,
    pub name: String,
    pub url: String,
    pub group: Option<String>,
    /// A remote image: shown only with `network.load_remote_artwork`.
    pub logo: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StreamGroup {
    /// `None`: channels without a group.
    pub name: Option<String>,
    #[ts(type = "number")]
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StreamPage {
    pub items: Vec<StreamEntry>,
    #[ts(type = "number")]
    pub total: i64,
}

pub fn sources(conn: &Connection) -> rusqlite::Result<Vec<StreamSource>> {
    let mut statement = conn.prepare(
        "SELECT s.id, s.name, s.location, s.refreshed_at,
                (SELECT COUNT(*) FROM streams t WHERE t.source_id = s.id)
           FROM stream_sources s ORDER BY s.name COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(StreamSource {
            id: row.get(0)?,
            name: row.get(1)?,
            location: row.get(2)?,
            refreshed_at: row.get(3)?,
            count: row.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn source(conn: &Connection, id: i64) -> rusqlite::Result<Option<StreamSource>> {
    Ok(sources(conn)?.into_iter().find(|s| s.id == id))
}

pub fn add_source(conn: &Connection, name: &str, location: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO stream_sources (name, location) VALUES (?1, ?2)",
        params![name, location],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn remove_source(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM stream_sources WHERE id = ?1", params![id])?;
    Ok(())
}

/// Replaces a source's channels with a freshly parsed list.
pub fn replace_channels(
    conn: &mut Connection,
    source_id: i64,
    channels: &[Channel],
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM streams WHERE source_id = ?1", params![source_id])?;
    {
        let mut statement = tx.prepare(
            "INSERT INTO streams (source_id, name, url, grp, logo, tvg_id, user_agent, referrer, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for (position, channel) in channels.iter().enumerate() {
            statement.execute(params![
                source_id,
                channel.name,
                channel.url,
                channel.group,
                channel.logo,
                channel.tvg_id,
                channel.user_agent,
                channel.referrer,
                position as i64,
            ])?;
        }
    }
    tx.execute(
        "UPDATE stream_sources SET refreshed_at = ?2 WHERE id = ?1",
        params![source_id, unix_now()],
    )?;
    tx.commit()
}

pub fn groups(conn: &Connection, source_id: i64) -> rusqlite::Result<Vec<StreamGroup>> {
    let mut statement = conn.prepare(
        "SELECT grp, COUNT(*) FROM streams WHERE source_id = ?1
          GROUP BY grp ORDER BY grp IS NULL, MIN(position)",
    )?;
    let rows = statement.query_map(params![source_id], |row| {
        Ok(StreamGroup { name: row.get(0)?, count: row.get(1)? })
    })?;
    rows.collect()
}

/// Channels of a source (and group), or every channel whose name matches `search`.
pub fn list(
    conn: &Connection,
    source_id: Option<i64>,
    group: Option<&str>,
    ungrouped: bool,
    search: Option<&str>,
    limit: u32,
    offset: u32,
) -> rusqlite::Result<StreamPage> {
    let mut filter = String::from("1 = 1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(id) = source_id {
        filter.push_str(" AND source_id = ?");
        args.push(Box::new(id));
    }
    if let Some(group) = group {
        filter.push_str(" AND grp = ?");
        args.push(Box::new(group.to_string()));
    } else if ungrouped {
        filter.push_str(" AND grp IS NULL");
    }
    // Channel lists are small next to the library: a LIKE per word is plenty.
    let words: Vec<String> = search
        .filter(|s| fts_query(s).is_some())
        .map(|s| s.split_whitespace().map(|w| format!("%{}%", w.replace(['%', '_'], ""))).collect())
        .unwrap_or_default();
    for word in words {
        filter.push_str(" AND name LIKE ?");
        args.push(Box::new(word));
    }
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM streams WHERE {filter}"),
        rusqlite::params_from_iter(args.iter().map(AsRef::as_ref)),
        |row| row.get(0),
    )?;
    args.push(Box::new(limit));
    args.push(Box::new(offset));
    let mut statement = conn.prepare(&format!(
        "SELECT id, source_id, name, url, grp, logo FROM streams WHERE {filter}
          ORDER BY source_id, position LIMIT ? OFFSET ?"
    ))?;
    let items = statement
        .query_map(rusqlite::params_from_iter(args.iter().map(AsRef::as_ref)), |row| {
            Ok(StreamEntry {
                id: row.get(0)?,
                source_id: row.get(1)?,
                name: row.get(2)?,
                url: row.get(3)?,
                group: row.get(4)?,
                logo: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(StreamPage { items, total })
}

/// The HTTP headers a stream URL was listed with: `(user_agent, referrer)`.
pub fn headers_for(
    conn: &Connection,
    url: &str,
) -> rusqlite::Result<Option<(Option<String>, Option<String>)>> {
    conn.query_row(
        "SELECT user_agent, referrer FROM streams
          WHERE url = ?1 AND (user_agent IS NOT NULL OR referrer IS NOT NULL) LIMIT 1",
        params![url],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
}

/// The names stream lists give these URLs, for those that are listed channels.
pub fn names_for(
    conn: &Connection,
    urls: &[String],
) -> rusqlite::Result<std::collections::HashMap<String, String>> {
    let mut names = std::collections::HashMap::new();
    let mut stmt = conn.prepare_cached(
        "SELECT name FROM streams WHERE url = ?1 AND name <> '' ORDER BY id LIMIT 1",
    )?;
    for url in urls {
        if let Some(name) = stmt.query_row(params![url], |row| row.get(0)).optional()? {
            names.insert(url.clone(), name);
        }
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db::Db;

    const LIST: &str = "#EXTM3U url-tvg=\"http://example.com/epg.xml\"\n\
#EXTINF:-1 tvg-id=\"news.one\" tvg-name=\"News One HD\" tvg-logo=\"http://img/news.png\" group-title=\"News\",News One\n\
#EXTVLCOPT:http-user-agent=TestAgent/1.0 (Linux; x)\n\
#EXTVLCOPT:http-referrer=http://portal.example/\n\
http://example.com/news1.m3u8\n\
\n\
#EXTINF:-1 group-title=\"Sport\" tvg-logo=\"\",Sport, Live & More\n\
https://example.com/sport.ts\n\
#EXTINF:-1,No Group\n\
#EXTGRP:Music\n\
http://example.com/music.m3u8\n\
#EXTINF:-1 tvg-name=\"Only tvg name\",\n\
http://example.com/tvg.m3u8\n\
this line is noise\n\
#EXTINF:-1,Orphan with no url\n\
# just a comment\n\
http://example.com/bare.m3u8\n\
rtmp://example.com/live/stream\n";

    #[test]
    fn attributes_groups_and_vlc_options() {
        let channels = parse(LIST);
        let names: Vec<&str> = channels.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "News One",
                "Sport, Live & More",
                "No Group",
                "Only tvg name",
                "Orphan with no url",
                "rtmp://example.com/live/stream"
            ]
        );
        let news = &channels[0];
        assert_eq!(news.url, "http://example.com/news1.m3u8");
        assert_eq!(news.group.as_deref(), Some("News"));
        assert_eq!(news.logo.as_deref(), Some("http://img/news.png"));
        assert_eq!(news.tvg_id.as_deref(), Some("news.one"));
        assert_eq!(news.user_agent.as_deref(), Some("TestAgent/1.0 (Linux; x)"));
        assert_eq!(news.referrer.as_deref(), Some("http://portal.example/"));
        assert_eq!(channels[1].group.as_deref(), Some("Sport"));
        assert_eq!(channels[1].logo, None, "an empty attribute is no logo");
        assert_eq!(channels[1].user_agent, None, "options do not leak to the next channel");
        assert_eq!(channels[2].group.as_deref(), Some("Music"), "#EXTGRP");
        // The info line before the noise line is used up by it; the next URL keeps the one
        // closest to it.
        assert_eq!(channels[4].url, "http://example.com/bare.m3u8");
    }

    #[test]
    fn bad_input_gives_what_it_can() {
        assert!(parse("").is_empty());
        assert!(parse("#EXTM3U\n#EXTINF:\n#EXTINF:-1 tvg-id=\"unterminated,Name\n").is_empty());
        let odd = parse(
            "#EXTINF:-1 group-title=\"A\" bogus=nothing,Name\nhttp://x/y\n#EXTVLCOPT:broken\nhttp://x/z\n",
        );
        assert_eq!(odd.len(), 2);
        assert_eq!((odd[0].name.as_str(), odd[0].group.as_deref()), ("Name", Some("A")));
        assert_eq!(odd[1].name, "http://x/z");
        let crlf = parse("\u{feff}#EXTM3U\r\n#EXTINF:-1,One\r\nhttp://x/1\r\n");
        assert_eq!(crlf[0].url, "http://x/1");
    }

    #[test]
    fn sources_groups_search_and_refresh() {
        let db = Db::in_memory().unwrap();
        db.call(|c| {
            let id = add_source(c, "Test list", "D:/lists/test.m3u")?;
            replace_channels(c, id, &parse(LIST))?;
            let source = source(c, id)?.unwrap();
            assert_eq!(source.count, 6);
            assert!(source.refreshed_at.is_some());
            let names: Vec<(Option<String>, i64)> =
                groups(c, id)?.into_iter().map(|g| (g.name, g.count)).collect();
            assert_eq!(
                names,
                vec![
                    (Some("News".into()), 1),
                    (Some("Sport".into()), 1),
                    (Some("Music".into()), 1),
                    (None, 3)
                ]
            );
            assert_eq!(
                list(c, Some(id), Some("News"), false, None, 50, 0)?.items[0].name,
                "News One"
            );
            assert_eq!(list(c, Some(id), None, true, None, 50, 0)?.total, 3);
            assert_eq!(list(c, None, None, false, Some("sport live"), 50, 0)?.total, 1);
            assert_eq!(list(c, None, None, false, Some("%"), 50, 0)?.total, 6, "no usable words");
            let page = list(c, Some(id), None, false, None, 2, 4)?;
            assert_eq!((page.items.len(), page.total), (2, 6));

            assert_eq!(
                headers_for(c, "http://example.com/news1.m3u8")?,
                Some((
                    Some("TestAgent/1.0 (Linux; x)".into()),
                    Some("http://portal.example/".into())
                ))
            );
            assert_eq!(headers_for(c, "https://example.com/sport.ts")?, None);
            let urls = ["https://example.com/sport.ts".to_string(), "http://elsewhere/".into()];
            let names = names_for(c, &urls)?;
            assert_eq!(names.get(&urls[0]).map(String::as_str), Some("Sport, Live & More"));
            assert_eq!(names.len(), 1, "an address no list names has no name");

            // A refresh replaces the entries.
            replace_channels(c, id, &parse("#EXTINF:-1,Only\nhttp://x/only\n"))?;
            assert_eq!(list(c, Some(id), None, false, None, 50, 0)?.total, 1);
            remove_source(c, id)?;
            assert_eq!(list(c, None, None, false, None, 50, 0)?.total, 0, "channels go with it");
            Ok(())
        })
        .unwrap();
    }
}
