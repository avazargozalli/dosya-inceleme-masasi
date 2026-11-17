-- 0052: **belge sürümünün kimlik kanıtı** (document_versions) doğuşta tutarlı
-- olmalı ve sonrasında değişmemeli.
--
-- Neden: sürüm satırı **kanıt zincirinin köküdür** — hangi baytlar (`blob_id`),
-- o baytların özeti (`original_sha256`), boyutu (`byte_size`), türü
-- (`detected_mime`) ve ne zaman alındığı (`imported_at`). Üstündeki bütün
-- kanıt kapıları bu sütuna yaslanır:
--   * `redaction_plans.render_hash` doğuşta **bu özetle eşit** olmak zorunda
--     (0051) — «geometri bu render'a aittir» iddiasının dayanağı budur;
--   * yeniden inceleme nedeni `render_changed`
--     (`v.original_sha256 <> p.render_hash`) yalnız bu sütunla hesaplanır
--     (`get_redaction_plan`/`list_redaction_plans`, `screens/Redaction.tsx`);
--   * `documents.find_version_by_sha` aynı baytın yeniden içe aktarılmasını
--     bu sütunla tanır — özet çevrilebilirse aynı dosya iki kez içe alınır;
--   * `parsing` blob'u sürümün künyesiyle (`detected_mime`, `byte_size`) açar.
-- Şemada bu tablonun kapısı yalnız **kapsam**tı (0031/0034/0035: sürüm kendi
-- dosyasının, ebeveyn aynı dosyanın, güncel işaretçi kendi belgesinin) ve
-- `original_sha256` için yalnız `CHECK (length = 64)` vardı.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v51; sonda
-- `scratch/dim-v52-probe/measure_v52.py`, log `measure_before.log`):
-- 17 kusur yazımının **17'si KABUL** (taban 23/23; 1 yazım zaten RED —
-- yalnız `length` CHECK'i — ve 5 meşru yazma KABUL):
--   (1) doğuşta özet **64 hex değil** (`'Z'*64`), (2) **büyük harf** hex
--       (`'A'*64`) — `find_version_by_sha` eşleşmez, ekranda özet büyük
--       harfle basılır; (3) algılanan mime boş, (4) yalnız boşluk,
--       (5) mime **kırpılmamış** (`' application/pdf'`), (6) içe aktarma
--       damgası boş, (7) damga **ISO değil** (`'dün'`), (8) **boş sürüm
--       kimliği**;
--   (9) özet UPDATE ile çevrildi (maskeleme planının bağlandığı render
--       sessizce başkalaştı), (10) `blob_id` **başka baytlara** çevrildi
--       (özet ile dosya ayrıştı), (11) `byte_size` çevrildi, (12) mime
--       çevrildi (`application/pdf` → `text/plain`), (13) `imported_at`
--       çevrildi, (14) `is_original` 1→0, (15) `version_no` çevrildi
--       (sürüm sırası bozuldu), (16) `original_filename_encrypted`
--       sonradan eklendi, (17) `id` çevrildi (satır kimliği takas edildi).
--
-- Yazılan kural (2 tetikleyici):
--   * INSERT — `id` kırpılmış-dolu; `original_sha256` **64 küçük harf hex**
--     (`length` CHECK'i 64'ü zorlar, burada karakter kümesi denetlenir —
--     0051'in (c) sınırı); `detected_mime` kırpılmış ve dolu;
--     `imported_at` ISO-8601 damgası (`YYYY-MM-DDTHH:MM:SS…`, 0049 ile aynı
--     biçim). `byte_size ≥ 0`, `version_no ≥ 1` ve `is_original ∈ {0,1}`
--     zaten CHECK'te.
--   * UPDATE — `id`, `version_no`, `blob_id`, `original_sha256`,
--     `byte_size`, `detected_mime`, `imported_at`, `is_original`,
--     `original_filename_encrypted` **donar**: sürüm doğduğu künyeyle
--     yaşar (çekirdekte bu tabloya UPDATE yolu yok; tek meşru yazma yeni
--     sürüm doğurmaktır). Değişmeyen yazma takılmaz.
-- Ret `DIM_INTEGRITY:document_versions` → `StorageError::Integrity` →
-- **`E_DB_INTEGRITY`**.
--
-- Kapsam taraması: tablo `case_id` taşıyor ve **zaten** taranıyor → yeni
-- sınıf eklenmedi, `GUARDED_TABLES` **25 kalır** (0044–0051 ile aynı sınıf —
-- kural dosya kapsamı değil **kimlik kanıtı bütünlüğü**dür).
--
-- Dürüst sınırlar: (a) v52 öncesi tutarsız sürümler **silinmez** ve artık
-- **onarılamaz** (donmuş) — temizlik ürün kararı. (b) Kapı özetin gerçekten
-- **baytların** SHA-256'sı olduğunu doğrulamaz: `blobs` özet taşımaz ve
-- SQLite'ta SHA-256 yok; ölçülen **biçim + değişmezlik**tir, metin/bayt
-- eşleşmesi değil. (c) `document_id` ve `case_id` bu kapının dışında:
-- sütunlar **kapsam** tetikleyicilerinin (0031/0035) ve sürümün aynı dosyada
-- başka belgeye taşınması orada **bilinçli serbest** (`scope_guard.rs`
-- testi bunu sabitler). (d) `parent_version_id` de kapsamın (0034) ve
-- `ON DELETE SET NULL`ın sütunu: burada doldurma/çevirme durdurulmaz.
-- (e) `byte_size`ın `blobs.encrypted_size` ile ilişkisi denetlenmez
-- (şifreleme zarfı farkı ölçülmez). (f) SQLite `trim()` yalnız ASCII boşluk
-- kırpar (U+00A0 artığı geçer). (g) Doğuşta `imported_at` **gelecekte**
-- olabilir (sıra/zaman denetimi yok). (h) Tetikleyicileri anahtarı olan
-- saldırgan düşürebilir. (i) Arayüz davranışı değişmedi; gerçek pencere +
-- görsel QA **insan bekliyor**.

CREATE TRIGGER document_version_identity_guard_bi
BEFORE INSERT ON document_versions
WHEN length(trim(NEW.id)) = 0
  OR NEW.original_sha256 GLOB '*[^0-9a-f]*'
  OR length(trim(NEW.detected_mime)) = 0
  OR NEW.detected_mime <> trim(NEW.detected_mime)
  OR NEW.imported_at NOT LIKE '____-__-__T__:__:__%'
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:document_versions');
END;

CREATE TRIGGER document_version_identity_guard_bu
BEFORE UPDATE ON document_versions
WHEN NEW.id IS NOT OLD.id
  OR NEW.version_no IS NOT OLD.version_no
  OR NEW.blob_id IS NOT OLD.blob_id
  OR NEW.original_sha256 IS NOT OLD.original_sha256
  OR NEW.byte_size IS NOT OLD.byte_size
  OR NEW.detected_mime IS NOT OLD.detected_mime
  OR NEW.imported_at IS NOT OLD.imported_at
  OR NEW.is_original IS NOT OLD.is_original
  OR NEW.original_filename_encrypted IS NOT OLD.original_filename_encrypted
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:document_versions');
END;
