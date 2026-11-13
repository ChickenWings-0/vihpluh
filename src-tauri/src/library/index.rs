//! The library index (ARCHITECTURE §8.2, §9.6 `folders` / `media`): folders the user added,
//! the media rows the scanner keeps for them, and the queries behind the Library drawer.

use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::db::unix_now;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FolderKind {
    Video,
    Music,
    Mixed,
}

impl FolderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Music => "music",
            Self::Mixed => "mixed",
        }
    }

    fn parse(text: &str) -> Self {
        match text {
            "video" => Self::Video,
            "music" => Self::Music,
            _ => Self::Mixed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryFolder {
    #[ts(type = "number")]
    pub id: i64,
    pub path: String,
    pub kind: FolderKind,
    /// Shown as a shortcut in the Folders section.
    pub pinned: bool,
    /// Scanned into the library (false: a pinned shortcut that is only browsed live).
    pub indexed: bool,
    #[ts(type = "number")]
    pub added_at: i64,
    /// How many media files the index holds for it.
    #[ts(type = "number")]
    pub count: i64,
}

pub use crate::ipc::dto::MediaKind;

fn kind_str(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Video => "video",
        MediaKind::Audio => "audio",
    }
}

/// One file in the library (or in a live folder listing, where `id` is 0).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "LibraryMedia")]
pub struct MediaEntry {
    #[ts(type = "number")]
    pub id: i64,
    pub path: String,
    pub kind: MediaKind,
    pub title: String,
    pub duration: Option<f64>,
    pub series_key: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<u32>,
    /// Resume position from history, if the file was played.
    pub position: Option<f64>,
    pub finished: bool,
}

/// A series, an album, or an artist: opens to its files.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GroupEntry {
    /// What to pass back as `parent` to list the group's files.
    pub key: String,
    pub title: String,
    /// Album: the artist. Otherwise empty.
    pub subtitle: Option<String>,
    pub year: Option<u32>,
    #[ts(type = "number")]
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum LibraryItem {
    Media(MediaEntry),
    Group(GroupEntry),
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryPage {
    pub items: Vec<LibraryItem>,
    /// Rows the query matches in all, for paging.
    #[ts(type = "number")]
    pub total: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LibraryView {
    /// Every file, by title.
    All,
    /// Videos grouped into series (films stay single rows).
    Series,
    Albums,
    Artists,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryQuery {
    pub kind: MediaKind,
    pub view: LibraryView,
    /// A group's `key`, to list what is inside it.
    #[serde(default)]
    #[ts(optional)]
    pub parent: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub search: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

/// Album keys join artist and album with a character that appears in neither.
const KEY_SEPARATOR: char = '\u{1f}';

// ── Folders ─────────────────────────────────────────────────────────────────

pub fn folders(conn: &Connection) -> rusqlite::Result<Vec<LibraryFolder>> {
    let mut statement = conn.prepare(
        "SELECT f.id, f.path, f.kind, f.pinned, f.indexed, f.added_at,
                (SELECT COUNT(*) FROM media m WHERE m.folder_id = f.id)
           FROM folders f ORDER BY f.path COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(LibraryFolder {
            id: row.get(0)?,
            path: row.get(1)?,
            kind: FolderKind::parse(&row.get::<_, String>(2)?),
            pinned: row.get(3)?,
            indexed: row.get(4)?,
            added_at: row.get(5)?,
            count: row.get(6)?,
        })
    })?;
    rows.collect()
}

pub fn folder(conn: &Connection, id: i64) -> rusqlite::Result<Option<LibraryFolder>> {
    Ok(folders(conn)?.into_iter().find(|f| f.id == id))
}

/// Adds a folder, or updates the one already there. `indexed` only ever turns on here: a
/// folder that is scanned stays scanned when it is also pinned.
pub fn add_folder(
    conn: &Connection,
    path: &str,
    kind: FolderKind,
    pinned: bool,
    indexed: bool,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO folders (path, kind, pinned, indexed, added_at) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(path) DO UPDATE SET
           kind = excluded.kind, pinned = excluded.pinned, indexed = MAX(indexed, excluded.indexed)",
        params![path, kind.as_str(), pinned, indexed, unix_now()],
    )?;
    conn.query_row("SELECT id FROM folders WHERE path = ?1", params![path], |row| row.get(0))
}

/// Removes a folder and (by cascade) its media rows.
pub fn remove_folder(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM folders WHERE id = ?1", params![id])?;
    Ok(())
}

// ── Rows written by the scanner ─────────────────────────────────────────────

/// What the scanner knows about a file before it is played.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediaRow {
    pub path: String,
    pub kind: &'static str,
    pub size: i64,
    pub mtime: i64,
    pub duration: Option<f64>,
    pub title: String,
    pub series_key: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<u32>,
}

/// `(path, size, mtime)` of every indexed file of a folder, to tell what changed.
pub fn known_files(conn: &Connection, folder_id: i64) -> rusqlite::Result<Vec<(String, i64, i64)>> {
    let mut statement = conn.prepare("SELECT path, size, mtime FROM media WHERE folder_id = ?1")?;
    let rows = statement
        .query_map(params![folder_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
    rows.collect()
}

/// Inserts new files and refreshes changed ones. A changed file loses its media key and (for
/// video) its duration: both are filled in again lazily.
pub fn upsert(conn: &mut Connection, folder_id: i64, rows: &[MediaRow]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        let mut statement = tx.prepare(
            "INSERT INTO media (folder_id, path, kind, size, mtime, duration, title, series_key,
                                season, episode, artist, album_artist, album, track_no, disc_no,
                                year, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             ON CONFLICT(path) DO UPDATE SET
               folder_id = excluded.folder_id, kind = excluded.kind, size = excluded.size,
               mtime = excluded.mtime, duration = excluded.duration, title = excluded.title,
               series_key = excluded.series_key, season = excluded.season,
               episode = excluded.episode, artist = excluded.artist,
               album_artist = excluded.album_artist, album = excluded.album,
               track_no = excluded.track_no, disc_no = excluded.disc_no, year = excluded.year,
               media_key = NULL",
        )?;
        let now = unix_now();
        for row in rows {
            statement.execute(params![
                folder_id,
                row.path,
                row.kind,
                row.size,
                row.mtime,
                row.duration,
                row.title,
                row.series_key,
                row.season,
                row.episode,
                row.artist,
                row.album_artist,
                row.album,
                row.track_no,
                row.disc_no,
                row.year,
                now,
            ])?;
        }
    }
    tx.commit()
}

pub fn remove_paths(conn: &mut Connection, paths: &[String]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        let mut statement = tx.prepare("DELETE FROM media WHERE path = ?1")?;
        for path in paths {
            statement.execute(params![path])?;
        }
    }
    tx.commit()
}

/// Files still waiting for their media key or (video) duration: `(id, path, kind, has_key)`.
pub fn unfilled(
    conn: &Connection,
    limit: u32,
) -> rusqlite::Result<Vec<(i64, String, String, bool)>> {
    let mut statement = conn.prepare(
        "SELECT id, path, kind, media_key IS NOT NULL FROM media
          WHERE media_key IS NULL OR (duration IS NULL AND kind = 'video')
          ORDER BY id LIMIT ?1",
    )?;
    let rows = statement.query_map(params![limit], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })?;
    rows.collect()
}

/// Stores what the lazy pass found. A duration of −1 marks "asked, not known" so the file
/// is not probed again on every pass.
pub fn fill(
    conn: &Connection,
    id: i64,
    media_key: Option<&str>,
    duration: Option<f64>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE media SET media_key = COALESCE(?2, media_key), duration = COALESCE(?3, duration)
          WHERE id = ?1",
        params![id, media_key, duration],
    )?;
    Ok(())
}

/// Where a file with this media key lives now (playlists relocate moved files by it).
pub fn path_for_key(conn: &Connection, media_key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT path FROM media WHERE media_key = ?1 ORDER BY id LIMIT 1",
        params![media_key],
        |row| row.get(0),
    )
    .optional()
}

// ── Queries ─────────────────────────────────────────────────────────────────

const MEDIA_COLUMNS: &str = "m.id, m.path, m.kind, m.title, m.duration, m.series_key, m.season,
    m.episode, m.artist, m.album_artist, m.album, m.track_no, m.disc_no, m.year,
    h.position, COALESCE(h.finished, 0)";
const MEDIA_FROM: &str = "media m LEFT JOIN history h ON h.source = m.path";

fn media_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<MediaEntry> {
    let kind: String = row.get(2)?;
    Ok(MediaEntry {
        id: row.get(0)?,
        path: row.get(1)?,
        kind: if kind == "audio" { MediaKind::Audio } else { MediaKind::Video },
        title: row.get(3)?,
        // Negative: probed without a result.
        duration: row.get::<_, Option<f64>>(4)?.filter(|d| *d >= 0.0),
        series_key: row.get(5)?,
        season: row.get(6)?,
        episode: row.get(7)?,
        artist: row.get(8)?,
        album_artist: row.get(9)?,
        album: row.get(10)?,
        track_no: row.get(11)?,
        disc_no: row.get(12)?,
        year: row.get(13)?,
        position: row.get::<_, Option<f64>>(14)?.filter(|p| *p > 0.0),
        finished: row.get(15)?,
    })
}

/// An FTS5 query that matches every word of `text` as a prefix (`"fri"*  "0"*`).
pub fn fts_query(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

type Args = Vec<Box<dyn rusqlite::ToSql>>;

fn count(conn: &Connection, sql: &str, args: &Args) -> rusqlite::Result<i64> {
    conn.query_row(sql, params_from_iter(args.iter().map(AsRef::as_ref)), |row| row.get(0))
}

fn media_rows(
    conn: &Connection,
    filter: &str,
    order: &str,
    mut args: Args,
    query: &LibraryQuery,
) -> rusqlite::Result<LibraryPage> {
    let total = count(conn, &format!("SELECT COUNT(*) FROM media m WHERE {filter}"), &args)?;
    args.push(Box::new(query.limit));
    args.push(Box::new(query.offset));
    let sql = format!(
        "SELECT {MEDIA_COLUMNS} FROM {MEDIA_FROM} WHERE {filter} ORDER BY {order} LIMIT ? OFFSET ?"
    );
    let mut statement = conn.prepare(&sql)?;
    let items = statement
        .query_map(params_from_iter(args.iter().map(AsRef::as_ref)), media_entry)?
        .map(|row| row.map(LibraryItem::Media))
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(LibraryPage { items, total })
}

/// The Library drawer's listings (§7.3 `library_query`).
pub fn query(conn: &Connection, query: &LibraryQuery) -> rusqlite::Result<LibraryPage> {
    let kind = kind_str(query.kind);
    let search = query.search.as_deref().and_then(fts_query);
    // A search always lists files, whatever the view: the match may be a title, a series,
    // an artist, or an album.
    if let Some(fts) = search {
        let args: Args = vec![Box::new(kind), Box::new(fts)];
        return media_rows(
            conn,
            "m.kind = ? AND m.id IN (SELECT rowid FROM media_fts WHERE media_fts MATCH ?)",
            "m.title COLLATE NOCASE, m.path",
            args,
            query,
        );
    }
    match (query.view, query.parent.as_deref()) {
        (LibraryView::All, _) => media_rows(
            conn,
            "m.kind = ?",
            "m.title COLLATE NOCASE, m.path",
            vec![Box::new(kind)],
            query,
        ),
        (LibraryView::Series, Some(series)) => media_rows(
            conn,
            "m.kind = 'video' AND m.series_key = ? AND m.episode IS NOT NULL",
            "COALESCE(m.season, 0), m.episode, m.path",
            vec![Box::new(series.to_string())],
            query,
        ),
        (LibraryView::Series, None) => series_and_films(conn, query),
        (LibraryView::Albums, Some(key)) => {
            let (artist, album) = key.split_once(KEY_SEPARATOR).unwrap_or(("", key));
            media_rows(
                conn,
                "m.kind = 'audio' AND COALESCE(m.album_artist, m.artist, '') = ?
                 AND COALESCE(m.album, '') = ?",
                "COALESCE(m.disc_no, 1), COALESCE(m.track_no, 9999), m.path",
                vec![Box::new(artist.to_string()), Box::new(album.to_string())],
                query,
            )
        }
        (LibraryView::Albums, None) => groups(
            conn,
            "COALESCE(album_artist, artist, '') || char(31) || COALESCE(album, '')",
            "COALESCE(album, '')",
            "COALESCE(album_artist, artist, '')",
            query,
        ),
        (LibraryView::Artists, Some(artist)) => media_rows(
            conn,
            "m.kind = 'audio' AND COALESCE(m.album_artist, m.artist, '') = ?",
            "COALESCE(m.album, '') COLLATE NOCASE, COALESCE(m.disc_no, 1),
             COALESCE(m.track_no, 9999), m.path",
            vec![Box::new(artist.to_string())],
            query,
        ),
        (LibraryView::Artists, None) => groups(
            conn,
            "COALESCE(album_artist, artist, '')",
            "COALESCE(album_artist, artist, '')",
            "NULL",
            query,
        ),
    }
}

fn group_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<GroupEntry> {
    Ok(GroupEntry {
        key: row.get(0)?,
        title: row.get(1)?,
        subtitle: row.get::<_, Option<String>>(2)?.filter(|s| !s.is_empty()),
        year: row.get(3)?,
        count: row.get(4)?,
    })
}

/// Music grouped by an expression (album or artist).
fn groups(
    conn: &Connection,
    key: &str,
    title: &str,
    subtitle: &str,
    query: &LibraryQuery,
) -> rusqlite::Result<LibraryPage> {
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(DISTINCT {key}) FROM media WHERE kind = 'audio'"),
        [],
        |row| row.get(0),
    )?;
    let sql = format!(
        "SELECT {key}, {title}, {subtitle}, MAX(year), COUNT(*) FROM media
          WHERE kind = 'audio' GROUP BY 1 ORDER BY 2 COLLATE NOCASE, 3 COLLATE NOCASE
          LIMIT ?1 OFFSET ?2"
    );
    let mut statement = conn.prepare(&sql)?;
    let items = statement
        .query_map(params![query.limit, query.offset], group_entry)?
        .map(|row| row.map(LibraryItem::Group))
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(LibraryPage { items, total })
}

/// Series as groups, then everything that is not an episode (films), in one paged list.
fn series_and_films(conn: &Connection, query: &LibraryQuery) -> rusqlite::Result<LibraryPage> {
    const EPISODE: &str = "kind = 'video' AND series_key IS NOT NULL AND episode IS NOT NULL";
    let series_total: i64 = conn.query_row(
        &format!("SELECT COUNT(DISTINCT series_key) FROM media WHERE {EPISODE}"),
        [],
        |row| row.get(0),
    )?;
    let film_filter = "m.kind = 'video' AND (m.series_key IS NULL OR m.episode IS NULL)";
    let film_total =
        count(conn, &format!("SELECT COUNT(*) FROM media m WHERE {film_filter}"), &Vec::new())?;

    let mut items = Vec::new();
    let (limit, offset) = (i64::from(query.limit), i64::from(query.offset));
    if offset < series_total {
        let mut statement = conn.prepare(&format!(
            "SELECT series_key, series_key, NULL, NULL, COUNT(*) FROM media WHERE {EPISODE}
              GROUP BY series_key ORDER BY series_key COLLATE NOCASE LIMIT ?1 OFFSET ?2"
        ))?;
        for row in statement.query_map(params![limit, offset], group_entry)? {
            items.push(LibraryItem::Group(row?));
        }
    }
    let room = limit - items.len() as i64;
    if room > 0 {
        let film_offset = (offset - series_total).max(0);
        let films =
            LibraryQuery { limit: room as u32, offset: film_offset as u32, ..query.clone() };
        let page =
            media_rows(conn, film_filter, "m.title COLLATE NOCASE, m.path", Vec::new(), &films)?;
        items.extend(page.items);
    }
    Ok(LibraryPage { items, total: series_total + film_total })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db::Db;

    fn video(path: &str, title: &str, series: Option<(&str, u32, u32)>) -> MediaRow {
        MediaRow {
            path: path.into(),
            kind: "video",
            size: 1,
            mtime: 1,
            title: title.into(),
            series_key: series.map(|s| s.0.to_string()),
            season: series.map(|s| s.1),
            episode: series.map(|s| s.2),
            ..MediaRow::default()
        }
    }

    fn song(path: &str, title: &str, artist: &str, album: &str, disc: u32, track: u32) -> MediaRow {
        MediaRow {
            path: path.into(),
            kind: "audio",
            size: 1,
            mtime: 1,
            duration: Some(200.0),
            title: title.into(),
            artist: Some(artist.into()),
            album_artist: Some(artist.into()),
            album: Some(album.into()),
            track_no: Some(track),
            disc_no: Some(disc),
            year: Some(1961),
            ..MediaRow::default()
        }
    }

    fn q(kind: MediaKind, view: LibraryView) -> LibraryQuery {
        LibraryQuery { kind, view, parent: None, search: None, limit: 100, offset: 0 }
    }

    fn titles(page: &LibraryPage) -> Vec<String> {
        page.items
            .iter()
            .map(|item| match item {
                LibraryItem::Media(m) => m.title.clone(),
                LibraryItem::Group(g) => format!("[{}] ×{}", g.title, g.count),
            })
            .collect()
    }

    fn seeded() -> Db {
        let db = Db::in_memory().unwrap();
        db.call(|c| {
            let folder = add_folder(c, "D:/Media", FolderKind::Mixed, false, true)?;
            upsert(
                c,
                folder,
                &[
                    video("D:/Media/Show S01E10.mkv", "Show S01E10", Some(("show", 1, 10))),
                    video("D:/Media/Show S01E2.mkv", "Show S01E2", Some(("show", 1, 2))),
                    video("D:/Media/Show S02E1.mkv", "Show S02E1", Some(("show", 2, 1))),
                    video("D:/Media/Other - 01.mkv", "Other - 01", Some(("other", 0, 1))),
                    video("D:/Media/Blade Runner 2049.mkv", "Blade Runner 2049", None),
                    video("D:/Media/Alien.mkv", "Alien", None),
                    song(
                        "D:/Media/w/02.flac",
                        "Waltz for Debby",
                        "Bill Evans",
                        "Waltz for Debby",
                        1,
                        2,
                    ),
                    song(
                        "D:/Media/w/01.flac",
                        "My Foolish Heart",
                        "Bill Evans",
                        "Waltz for Debby",
                        1,
                        1,
                    ),
                    song("D:/Media/k/01.flac", "So What", "Miles Davis", "Kind of Blue", 1, 1),
                    song("D:/Media/k/d2.flac", "Bonus", "Miles Davis", "Kind of Blue", 2, 1),
                ],
            )
        })
        .unwrap();
        db
    }

    #[test]
    fn series_are_grouped_and_episodes_come_in_order() {
        let db = seeded();
        let page = db.call(|c| query(c, &q(MediaKind::Video, LibraryView::Series))).unwrap();
        assert_eq!(titles(&page), vec!["[other] ×1", "[show] ×3", "Alien", "Blade Runner 2049"]);
        assert_eq!(page.total, 4);
        let episodes = db
            .call(|c| {
                let mut query_ = q(MediaKind::Video, LibraryView::Series);
                query_.parent = Some("show".into());
                query(c, &query_)
            })
            .unwrap();
        assert_eq!(titles(&episodes), vec!["Show S01E2", "Show S01E10", "Show S02E1"]);
    }

    #[test]
    fn paging_runs_across_series_and_films() {
        let db = seeded();
        let page = |offset: u32| {
            db.call(move |c| {
                let mut query_ = q(MediaKind::Video, LibraryView::Series);
                query_.limit = 3;
                query_.offset = offset;
                query(c, &query_)
            })
            .unwrap()
        };
        assert_eq!(titles(&page(0)), vec!["[other] ×1", "[show] ×3", "Alien"]);
        assert_eq!(titles(&page(3)), vec!["Blade Runner 2049"]);
        assert_eq!(page(3).total, 4);
    }

    #[test]
    fn albums_and_artists() {
        let db = seeded();
        let albums = db.call(|c| query(c, &q(MediaKind::Audio, LibraryView::Albums))).unwrap();
        assert_eq!(titles(&albums), vec!["[Kind of Blue] ×2", "[Waltz for Debby] ×2"]);
        let LibraryItem::Group(waltz) = albums.items[1].clone() else { panic!("a group") };
        assert_eq!(waltz.subtitle.as_deref(), Some("Bill Evans"));
        assert_eq!(waltz.year, Some(1961));
        let tracks = db
            .call(move |c| {
                let mut query_ = q(MediaKind::Audio, LibraryView::Albums);
                query_.parent = Some(waltz.key);
                query(c, &query_)
            })
            .unwrap();
        assert_eq!(titles(&tracks), vec!["My Foolish Heart", "Waltz for Debby"], "track order");

        let artists = db.call(|c| query(c, &q(MediaKind::Audio, LibraryView::Artists))).unwrap();
        assert_eq!(titles(&artists), vec!["[Bill Evans] ×2", "[Miles Davis] ×2"]);
        let davis = db
            .call(|c| {
                let mut query_ = q(MediaKind::Audio, LibraryView::Artists);
                query_.parent = Some("Miles Davis".into());
                query(c, &query_)
            })
            .unwrap();
        assert_eq!(titles(&davis), vec!["So What", "Bonus"], "disc 1 before disc 2");
    }

    #[test]
    fn search_finds_by_title_series_artist_and_album_prefix() {
        let db = seeded();
        let find = |kind: MediaKind, text: &str| {
            let text = text.to_string();
            titles(
                &db.call(move |c| {
                    let mut query_ = q(kind, LibraryView::All);
                    query_.search = Some(text);
                    query(c, &query_)
                })
                .unwrap(),
            )
        };
        assert_eq!(find(MediaKind::Video, "blade run"), vec!["Blade Runner 2049"]);
        assert_eq!(find(MediaKind::Video, "sho"), vec!["Show S01E10", "Show S01E2", "Show S02E1"]);
        assert_eq!(find(MediaKind::Audio, "miles"), vec!["Bonus", "So What"]);
        assert_eq!(find(MediaKind::Audio, "kind of"), vec!["Bonus", "So What"]);
        assert_eq!(find(MediaKind::Audio, "zzz"), Vec::<String>::new());
        assert_eq!(
            find(MediaKind::Video, "\" OR 1=1 --"),
            Vec::<String>::new(),
            "quotes are inert"
        );
        assert_eq!(fts_query("  ..  "), None);
    }

    #[test]
    fn rescans_update_rows_and_the_search_index_follows() {
        let db = seeded();
        db.call(|c| {
            let folder: i64 = c.query_row("SELECT id FROM folders", [], |r| r.get(0))?;
            // The file was retagged: same path, new title.
            upsert(c, folder, &[video("D:/Media/Alien.mkv", "Aliens", None)])?;
            remove_paths(c, &["D:/Media/Blade Runner 2049.mkv".to_string()])?;
            let mut search = q(MediaKind::Video, LibraryView::All);
            search.search = Some("alien".into());
            assert_eq!(titles(&query(c, &search)?), vec!["Aliens"]);
            search.search = Some("blade".into());
            assert!(query(c, &search)?.items.is_empty(), "a removed file is not found");
            assert_eq!(known_files(c, folder)?.len(), 9);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn history_shows_as_progress() {
        let db = seeded();
        db.call(|c| {
            crate::library::history::record_open(c, "f:1", "D:/Media/Alien.mkv", "Alien", None)?;
            crate::library::history::save_position(c, "f:1", 600.0, Some(7000.0), 95)?;
            let page = query(c, &q(MediaKind::Video, LibraryView::All))?;
            let LibraryItem::Media(alien) = &page.items[0] else { panic!("media") };
            assert_eq!(
                (alien.title.as_str(), alien.position, alien.finished),
                ("Alien", Some(600.0), false)
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn folders_upsert_and_cascade() {
        let db = seeded();
        db.call(|c| {
            let pinned = add_folder(c, "E:/Downloads", FolderKind::Video, true, false)?;
            let again = add_folder(c, "E:/Downloads", FolderKind::Video, false, false)?;
            assert_eq!(pinned, again);
            let list = folders(c)?;
            assert_eq!(list.len(), 2);
            assert_eq!(
                (list[0].path.as_str(), list[0].count, list[0].indexed),
                ("D:/Media", 10, true)
            );
            assert!(!list[1].pinned && !list[1].indexed);
            // Pinning a scanned folder keeps it scanned.
            add_folder(c, "D:/Media", FolderKind::Mixed, true, false)?;
            assert!(folders(c)?[0].indexed && folders(c)?[0].pinned);
            remove_folder(c, list[0].id)?;
            assert_eq!(c.query_row("SELECT COUNT(*) FROM media", [], |r| r.get::<_, i64>(0))?, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn lazy_fill_and_relocation_by_key() {
        let db = seeded();
        db.call(|c| {
            let waiting = unfilled(c, 100)?;
            assert_eq!(waiting.len(), 10, "every file lacks a key at first");
            let (id, path, _, _) = waiting[0].clone();
            fill(c, id, Some("f:abc"), Some(-1.0))?;
            assert_eq!(path_for_key(c, "f:abc")?, Some(path));
            assert_eq!(path_for_key(c, "f:none")?, None);
            assert_eq!(unfilled(c, 100)?.len(), 9);
            Ok(())
        })
        .unwrap();
    }
}
