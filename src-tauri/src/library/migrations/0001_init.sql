-- 0001_init.sql
CREATE TABLE folders (
  id        INTEGER PRIMARY KEY,
  path      TEXT NOT NULL UNIQUE,
  kind      TEXT NOT NULL CHECK (kind IN ('video','music','mixed')),
  pinned    INTEGER NOT NULL DEFAULT 0,
  indexed   INTEGER NOT NULL DEFAULT 1,     -- 0 = pinned shortcut only
  added_at  INTEGER NOT NULL                -- unix seconds
);

CREATE TABLE media (
  id          INTEGER PRIMARY KEY,
  folder_id   INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
  path        TEXT NOT NULL UNIQUE,
  media_key   TEXT,                         -- filled lazily (hashing is off the scan path)
  kind        TEXT NOT NULL CHECK (kind IN ('video','audio')),
  size        INTEGER NOT NULL,
  mtime       INTEGER NOT NULL,
  duration    REAL,
  title       TEXT NOT NULL,
  series_key  TEXT, season INTEGER, episode INTEGER,       -- video
  artist      TEXT, album_artist TEXT, album TEXT,         -- audio
  track_no    INTEGER, disc_no INTEGER, year INTEGER,
  width       INTEGER, height INTEGER,
  added_at    INTEGER NOT NULL
);
CREATE INDEX media_key_idx    ON media(media_key);
CREATE INDEX media_series_idx ON media(series_key, season, episode);
CREATE INDEX media_album_idx  ON media(album_artist, album, disc_no, track_no);

CREATE VIRTUAL TABLE media_fts USING fts5(
  title, series_key, artist, album, content='media', content_rowid='id'
);

-- Playback memory. Independent of `media`: covers files never added to the library.
CREATE TABLE history (
  media_key      TEXT PRIMARY KEY,
  source         TEXT NOT NULL,             -- last known path/URL
  title          TEXT NOT NULL,
  duration       REAL,
  position       REAL NOT NULL DEFAULT 0,   -- resume point; 0 when finished
  finished       INTEGER NOT NULL DEFAULT 0,
  play_count     INTEGER NOT NULL DEFAULT 0,
  last_played_at INTEGER NOT NULL,
  audio_track    TEXT,                      -- JSON TrackRule (§10.3) remembered per file
  sub_track      TEXT,
  sub_delay_ms   INTEGER,
  audio_delay_ms INTEGER
);
CREATE INDEX history_recent_idx ON history(last_played_at DESC);

CREATE TABLE bookmarks (
  id          INTEGER PRIMARY KEY,
  media_key   TEXT NOT NULL,
  position    REAL NOT NULL,
  label       TEXT NOT NULL,
  created_at  INTEGER NOT NULL
);
CREATE INDEX bookmarks_media_idx ON bookmarks(media_key, position);

-- External subtitle associations produced by auto-match or manual add.
CREATE TABLE sub_links (
  media_key   TEXT NOT NULL,
  sub_path    TEXT NOT NULL,
  lang        TEXT,
  source      TEXT NOT NULL CHECK (source IN ('auto','manual')),
  PRIMARY KEY (media_key, sub_path)
);

CREATE TABLE stream_sources (
  id           INTEGER PRIMARY KEY,
  name         TEXT NOT NULL,
  location     TEXT NOT NULL,               -- URL or file path of the .m3u
  refreshed_at INTEGER
);
CREATE TABLE streams (
  id         INTEGER PRIMARY KEY,
  source_id  INTEGER NOT NULL REFERENCES stream_sources(id) ON DELETE CASCADE,
  name       TEXT NOT NULL,
  url        TEXT NOT NULL,
  grp        TEXT,
  logo       TEXT,
  tvg_id     TEXT,
  user_agent TEXT,
  referrer   TEXT,
  position   INTEGER NOT NULL               -- order within the source
);
CREATE INDEX streams_source_idx ON streams(source_id, grp, position);
