-- 0002_library_index.sql — the library comes into use (Phase 9).

-- Keeps the full-text index in step with `media` (an external-content FTS5 table does not
-- follow its content table by itself).
CREATE TRIGGER media_fts_insert AFTER INSERT ON media BEGIN
  INSERT INTO media_fts(rowid, title, series_key, artist, album)
  VALUES (new.id, new.title, new.series_key, new.artist, new.album);
END;
CREATE TRIGGER media_fts_delete AFTER DELETE ON media BEGIN
  INSERT INTO media_fts(media_fts, rowid, title, series_key, artist, album)
  VALUES ('delete', old.id, old.title, old.series_key, old.artist, old.album);
END;
CREATE TRIGGER media_fts_update AFTER UPDATE OF title, series_key, artist, album ON media BEGIN
  INSERT INTO media_fts(media_fts, rowid, title, series_key, artist, album)
  VALUES ('delete', old.id, old.title, old.series_key, old.artist, old.album);
  INSERT INTO media_fts(rowid, title, series_key, artist, album)
  VALUES (new.id, new.title, new.series_key, new.artist, new.album);
END;

-- Library rows show their resume progress by joining history on the path.
CREATE INDEX history_source_idx ON history(source);
CREATE INDEX media_folder_idx ON media(folder_id);
CREATE INDEX media_kind_title_idx ON media(kind, title COLLATE NOCASE);
