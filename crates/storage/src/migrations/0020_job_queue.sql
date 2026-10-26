-- 0021_job_queue: kuyruk yürütücüsünün ihtiyaç duyduğu alanlar.
--
-- `jobs` tablosu 0001'den beri yalnız *yazılıyordu* (içe aktarma iş kaydı
-- açıyor, hiçbir katman sahiplenmiyordu). Bu migrasyon yeni durum eklemez;
-- sahiplenme sırası, öncelik ve bitiş zamanı için alan ekler.
--
-- `priority`: yüksek sayı önce alınır (kullanıcı işi > arka plan türevi).
-- `completed_at`: terminal duruma geçiş zamanı; `updated_at` yeniden
--   kullanılmıyor çünkü denetim/gecikme ölçümü bitiş anını ayrı ister.
ALTER TABLE jobs ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;
ALTER TABLE jobs ADD COLUMN completed_at TEXT;

-- 0001'de `next_run_at` varsayılanı NULL'dı; sahiplenme sorgusu artık NULL
-- taşımıyor, eski satırlar oluşturulma zamanıyla doldurulur (hemen hazır).
UPDATE jobs SET next_run_at=created_at WHERE next_run_at IS NULL;

-- Sahiplenme sorgusu: durum eşitliği + öncelik/yaş sırası. `next_run_at`
-- süzgeci indeksin sonunda kalır (kalan koşul); sıralamayı indeks karşılar,
-- böylece SQLite geçici B-tree kurmaz.
CREATE INDEX idx_jobs_claim ON jobs(state, priority DESC, created_at, next_run_at);
