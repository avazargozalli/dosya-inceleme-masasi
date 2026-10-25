-- F15-T05 — sayfa okuma yolunda ölçümle bulunan kusur.
--
-- `pages_for_run` her sayfa için `text_segments` tablosunu `page_id` ile üç kez
-- arıyordu (segment kimliği, ham metin, blok geometrisi) ve `page_id` sütununda
-- indeks yoktu. Sonuç: tek bir "hazır sayfa açılışı" bile **tam tablo taraması**
-- yapıyordu; 100.000 segmentlik kasada her satır SQLCipher ile çözülüp HMAC
-- doğrulandığı için gecikme saniyelerle ölçülüyordu.
--
-- Ölçüm (macOS aarch64, release, 1.000 belge / 100.000 segment, sayfa başına 1
-- segment): indeksten önce ilk sayfa okuması 100 sayfalık bir sürüm için
-- dakikalar sürüyordu; indeksten sonra milisaniyeler. Ayrıntı:
-- `docs/releases/F15/BENCHMARK.md`.
--
-- `pages` tarafında ek indeks gerekmiyor: `UNIQUE (parse_run_id, page_index)`
-- kısıtının SQLite otomatik indeksi (`sqlite_autoindex_pages_2`) sorgu planında
-- `parse_run_id` ile arama yapıyor (test: `pages_for_run_plan_uses_indexes`).

CREATE INDEX IF NOT EXISTS idx_text_segments_page ON text_segments(page_id);
