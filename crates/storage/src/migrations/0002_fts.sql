-- 0002_fts: tam metin arama. FTS tablosu `text_segments` ile tetikleyicilerle
-- senkron tutulur; silinen segment indeksten de düşer. İçerik dış tablodadır,
-- böylece metin iki kez saklanmaz (disk ve şifreleme maliyeti).
CREATE VIRTUAL TABLE text_segments_fts USING fts5(
  raw_text,
  normalized_text,
  content = 'text_segments',
  content_rowid = 'rowid',
  tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER text_segments_ai AFTER INSERT ON text_segments BEGIN
  INSERT INTO text_segments_fts(rowid, raw_text, normalized_text)
  VALUES (new.rowid, new.raw_text, new.normalized_text);
END;

CREATE TRIGGER text_segments_ad AFTER DELETE ON text_segments BEGIN
  INSERT INTO text_segments_fts(text_segments_fts, rowid, raw_text, normalized_text)
  VALUES ('delete', old.rowid, old.raw_text, old.normalized_text);
END;

CREATE TRIGGER text_segments_au AFTER UPDATE ON text_segments BEGIN
  INSERT INTO text_segments_fts(text_segments_fts, rowid, raw_text, normalized_text)
  VALUES ('delete', old.rowid, old.raw_text, old.normalized_text);
  INSERT INTO text_segments_fts(rowid, raw_text, normalized_text)
  VALUES (new.rowid, new.raw_text, new.normalized_text);
END;
