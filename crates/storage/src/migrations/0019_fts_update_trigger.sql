-- 0019_fts_update_trigger: FTS tetikleyicisi yalnız **içeriği belirleyen**
-- sütunlara bağlanır.
--
-- Neden: `0002_fts`'teki tetikleyici sütun listesi olmadan **her** UPDATE'te
-- çalışıyor ve FTS5 dış içerikli tabloya `delete` + `insert` yazıyordu. Metin
-- dışı bir sütun güncellendiğinde bu ikili yazım gereksizdi; üstelik aynı
-- satırda ikinci bir tetikleyici de yazınca (F15 ölçümünde: belirteç dizisi
-- sütununun temizlenmesi) FTS indeksi bozuluyor ve arama
-- `SQLITE_CORRUPT_VTAB` ("database disk image is malformed") veriyordu.
--
-- Kural: FTS içeriği yalnız `raw_text`/`normalized_text`'e bağlıdır; başka
-- sütun değişiminde indeksin tazelenmesi gerekmez.
DROP TRIGGER IF EXISTS text_segments_au;

CREATE TRIGGER text_segments_au
AFTER UPDATE OF raw_text, normalized_text ON text_segments
BEGIN
  INSERT INTO text_segments_fts(text_segments_fts, rowid, raw_text, normalized_text)
  VALUES ('delete', old.rowid, old.raw_text, old.normalized_text);
  INSERT INTO text_segments_fts(rowid, raw_text, normalized_text)
  VALUES (new.rowid, new.raw_text, new.normalized_text);
END;
