-- 0027: gönderim dönüşüm sicili (şartname §737 — maskeli bağlam).
--
-- Şartname her gönderim segmentinin «gönderilen metin/hash, **maskeleme
-- dönüşüm sürümü** ve özgün Unicode aralıklarına eşleme» taşımasını ister.
-- `transfer_segments.transform_manifest_id` sütunu şemanın ilk sürümünden beri
-- var ama bugüne kadar **hiçbir satır** yazılmadı (dönüşüm kimlikti).
--
-- Bu tablo dönüşümün **sürümünü** tanımlar; terimlerin kendisi saklanmaz
-- (maskenin amacı terimi kasadan çıkarmamaktır). Kayıt: tür, yer tutucu,
-- terim sayısı ve terim listesinin özeti. `settings_hash` içerik adreslidir:
-- aynı ayarlar aynı satırı kullanır.
CREATE TABLE outbound_transforms (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('identity', 'mask_terms')),
  settings_hash TEXT NOT NULL,
  term_count INTEGER NOT NULL,
  placeholder TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_outbound_transforms_hash
  ON outbound_transforms(settings_hash);

-- Özgün metnin karakter uzunluğu: aralık haritasının bütünlük kapısı
-- (`runs_are_sound`) gönderilen ve özgün uzunlukların ikisini de ister.
ALTER TABLE transfer_segments ADD COLUMN source_char_count INTEGER;
