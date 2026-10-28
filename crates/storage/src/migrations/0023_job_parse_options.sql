-- 0023_job_parse_options: ayrıştırma **ayarları** iş satırında saklanır.
--
-- Neden: `job_retry` yalnız `failed → queued` yazıp `run_parse`ı **varsayılan**
-- ayarlarla (OCR kapalı, kodlama `auto`) başlatıyordu. Kullanıcı belgeyi OCR
-- açık + `tur` ile ayrıştırmışsa, yeniden deneme sessizce **başka bir iş**
-- yapıyordu: taranmış belgede metin çıkmaz, iş yine "başarılı" görünürdü.
-- Aynı girdi + aynı ayar = aynı çıktı kuralı iş satırında tutulur.
--
-- Yol bilinçli olarak `ALTER TABLE ADD COLUMN`: `jobs` tablosu **yeniden
-- yazılmaz** (0021'de ölçülen FK/CASCADE tuzağı tekrarlanmaz).
--
-- **Parola saklanmaz.** Yalnız `parse_requires_password` bayrağı tutulur:
-- parolayı iş satırında tutmak onu kasa kilidinin dışına çıkarırdı. Bayrak
-- `1` ise yeniden deneme kendiliğinden başlatılmaz; kullanıcı parolayı belge
-- ekranından girer (sessizce farklı çıktı üretilmez).
--
-- `parse_ocr IS NULL` = "bu işte ayar kaydı yok" (0023 öncesi işler). Arayüz
-- bu durumda "varsayılanlarla koştu" der; uydurulmuş bir ayar gösterilmez.
ALTER TABLE jobs ADD COLUMN parse_ocr INTEGER
  CHECK (parse_ocr IS NULL OR parse_ocr IN (0, 1));
ALTER TABLE jobs ADD COLUMN parse_ocr_lang TEXT;
ALTER TABLE jobs ADD COLUMN parse_text_encoding TEXT;
ALTER TABLE jobs ADD COLUMN parse_requires_password INTEGER NOT NULL DEFAULT 0
  CHECK (parse_requires_password IN (0, 1));
