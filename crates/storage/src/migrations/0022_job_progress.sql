-- 0022_job_progress: canlı aşama/sayım **kuyruk satırında** tutulur.
--
-- Neden: `jobs.progress` yalnız `REAL 0..1` taşıyordu; ayrıştırma sırasında
-- sayaç arayüzde (`parse-progress` olayı) görünüyor, kuyruk kartında
-- görünmüyordu. Şartname §17.2 iş kaydında `progress_numerator` /
-- `progress_denominator` istiyor; burada `progress_done` / `progress_total`
-- adlarıyla durur, aşama da `progress_stage` ile yazılır.
--
-- Yol bilinçli olarak `ALTER TABLE ADD COLUMN`: `jobs` tablosu **yeniden
-- yazılmaz**. 0021'de ölçülen tuzak (tablo yeniden yazımı `job_attempts`
-- satırlarını CASCADE ile silebiliyordu) burada tekrarlanmaz; ekleme mevcut
-- satırları yerinde bırakır.
--
-- Sütun CHECK'leri tek sütunludur (SQLite `ADD COLUMN` başka sütuna bakan
-- kısıt kabul etmez). Sütunlar arası kural (`done <= total`, `total > 0`,
-- aşama adı biçimi) koddadır: `dim_storage::queue::update_job_progress`.
ALTER TABLE jobs ADD COLUMN progress_stage TEXT;
ALTER TABLE jobs ADD COLUMN progress_done INTEGER
  CHECK (progress_done IS NULL OR progress_done >= 0);
ALTER TABLE jobs ADD COLUMN progress_total INTEGER
  CHECK (progress_total IS NULL OR progress_total > 0);
