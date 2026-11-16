-- 0047: **gönderim sicilinin metni ve özeti kanıtlanabilir olmalı** (INV-05 /
-- şartname §737 «gönderilen metin sicile yazılır»).
--
-- Neden: `transfer_segments` satırı «modelin **gördüğü** metin» iddiasıdır.
--   * `list_transfer_segments` metnin ilk 120 karakterini ve `outbound_hash`in
--     ilk 12'sini ekrana basar (Ai.tsx gönderim sicili tablosu),
--   * `sent_segments` aday doğrulamasında alıntıyı **bu metinde** arar,
--   * `source_range_map` alıntının özgün karakter aralıklarına geri eşlenmesini
--     sağlar (`quote_scopes`, maske sonrası bağlama).
--
-- v45 bu tabloda **dönüşüm bağını** kapıladı (`transform_manifest_id` yalnız
-- onaylanan maskeyi gösterir, `id`/`ai_run_id` değişmez) ve kapanış notunda
-- sınırını açıkça yazdı: «Sicil metni (`outbound_text`/`outbound_hash`) bu
-- dilimin dışında: UPDATE ile değiştirilebilir kalır (sıradaki aday)».
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v46; betik
-- `~/.hermes/cache/scratch/dim-v47-probe/measure_v47.py`, log
-- `measure_before.log`): 14 kusur yazımının **12'si KABUL** (2 RED yalnız
-- 0038'in sürüm taşıması kapısı):
--   (1) yazılmış satırın `outbound_text`i UPDATE ile değiştirildi — okuyucu
--       yeni metni gösterdi, `sent_segments` alıntıyı sahte metinde arayacaktı;
--   (2) `outbound_hash` metinden bağımsız çevrildi (çift tutarsız, ekranda
--       yeni özet);
--   (3) metin + hash **birlikte** çevrildi (tutarlı görünen sahtecilik);
--   (4) `source_range_map` başka aralıklara çevrildi (alıntı yanlış özgün
--       aralığa bağlanır);
--   (5) `source_char_count` değiştirildi (harita genişliği kayar);
--   (6) `outbound_segment_id` aynı sürümde başka segmente çevrildi (satır
--       «s1» der, metin s2'nindir; alıntı yanlış çapaya bağlanır);
--   (7) `source_segment_id` çevrildi;
--   (8) boş metinle satır yazıldı;
--   (9) yalnız boşluktan oluşan metinle satır yazıldı;
--  (10) `outbound_hash='hash'` (64 haneli değil) yazıldı — ekran onu özet diye basar;
--  (11) `outbound_hash='Z'*64` (hex değil) yazıldı;
--  (12) `source_char_count=0` yazıldı.
-- `document_version_id` taşıması 0034/0038 kapsam kapısında zaten duruyor
-- (ölçümde 2 RED); bu göç onu **tekrar** kapılamaz (aynı sütunda iki değil,
-- aynı **satırda** iki tetikleyici olurdu — v46 dersi).
--
-- Yazılan kural (tek BI + tek BU, v45 adları korunur):
--   * BI — v45'in dönüşüm koşulları **aynen** durur; ek olarak satır
--     **doğarken** iyi-biçimli olmalı: metin boş/yalnız-boşluk değil,
--     `outbound_hash` **64 küçük-harf hex** hanesi, `source_char_count`
--     doluysa **pozitif**. Böylece ekranda «özet» diye basılan sütun biçim
--     olarak da hash'tir.
--   * BU — v45'in dondurduğu üç sütun (`id`, `ai_run_id`,
--     `transform_manifest_id`) **aynen** donmaya devam eder; ek olarak satırın
--     **içeriği** de donar: `outbound_text`, `outbound_hash`,
--     `source_range_map`, `source_char_count` değişmez. Gönderim sicili «ne
--     gönderildi»nin kaydıdır; sonradan ne metni ne eşlemesi düzeltilebilir
--     (yeniden ölçüm **yeni satır** ister). Bu, v45'in (d) sınırını kapatır.
--   * `scope_guard_transfer_segments_bu` **DROP+CREATE**: özne sütunları
--     (`outbound_segment_id`, `source_segment_id`) da **donar** — v46'da aynı
--     sürüm içinde başka segmente takas kabul ediliyordu (satır «s1» der,
--     metin s2'nindir; alıntı yanlış çapaya bağlanır). Sütun zaten 0038'in
--     kapsam tetikleyicisinde olduğu için **aynı** tetikleyici genişletildi
--     (v46 dersi: aynı sütunda iki BEFORE tetikleyici hata kodunu
--     belirsizleştirir); önek/numara korunur → `DIM_SCOPE:transfer_segments`.
--     `document_version_id` 0038'de kalır (NULL → sürüm **tamamlamak** meşru
--     onarım yoludur).
--     Gelecekte eklenecek bir sütun (ör. doğrulama durumu) `UPDATE OF`
--     listesinde olmadığı için takılmaz (v35 dersi).
--
-- Bilinçli sınırlar (dürüst):
--   (a) Kapı «özet **değişmedi**» der, «özet **doğru**» demez: SQLite'ta
--       SHA-256 yok, `outbound_hash` metinden yeniden hesaplanmaz. 64 hex
--       uyduran bir yazıcı yine tutarsız çift üretebilir — çekirdek doğru yazar.
--   (b) Kimlik satırında (`transform_manifest_id IS NULL`) metnin
--       `text_segments.raw_text`e **eşitliği** bu göçte kapılanmaz (sonraki
--       aday); bugün çekirdek `record_transfer_segments` beyanı kasadaki
--       metinle karşılaştırır.
--   (c) v47 öncesi bozuk satırlar **silinmez ve sayılmaz** (tablo `case_id`
--       taşımaz, tarama alanı yok); BU onların UPDATE ile onarılmasını da
--       engeller — temizlik/onarım ürün kararı.
--   (d) `transfer_segments` satırları hiçbir yol silmez (çalıştırma silinirse
--       `ON DELETE CASCADE`); bu yüzden «var olmayan segment kimliği» burada
--       denetim izi değil **kanıtlanabilir yalan** sayıldı (v45 ile aynı sınıf).
--   (e) Aynı satır hem kapsam hem içerik kuralını ihlal ediyorsa SQLite'ın
--       BEFORE tetikleyici sırası garanti değildir → hata kodu iki sınıftan
--       biri olabilir; testler satırları tek ihlalle kurar.
--   (f) Tetikleyicileri anahtarı olan saldırgan düşürebilir (uygulama hatasına
--       karşı).
--
-- Ret `RAISE(ABORT,'DIM_INTEGRITY:transfer_segments')` → `StorageError::Integrity`
-- → **`E_DB_INTEGRITY`** (0044/0045/0046 ile aynı önek).

DROP TRIGGER transfer_segments_transform_guard_bi;
CREATE TRIGGER transfer_segments_transform_guard_bi
BEFORE INSERT ON transfer_segments
WHEN NEW.outbound_text IS NULL
   OR trim(NEW.outbound_text) = ''
   OR NEW.outbound_hash IS NULL
   OR length(NEW.outbound_hash) <> 64
   OR NEW.outbound_hash GLOB '*[^0-9a-f]*'
   OR (NEW.source_char_count IS NOT NULL AND NEW.source_char_count <= 0)
   OR (NEW.transform_manifest_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM outbound_transforms x
         WHERE x.id = NEW.transform_manifest_id AND x.kind = 'mask_terms'
      ))
   OR EXISTS (
        SELECT 1
          FROM ai_runs r
          JOIN transfer_consents c ON c.id = r.consent_id
          JOIN outbound_transforms x ON x.id = c.outbound_transform_id
         WHERE r.id = NEW.ai_run_id
           AND ((x.term_count > 0
                 AND (NEW.transform_manifest_id IS NULL
                      OR NEW.transform_manifest_id <> c.outbound_transform_id))
                OR (x.term_count = 0 AND NEW.transform_manifest_id IS NOT NULL))
      )
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:transfer_segments');
END;

DROP TRIGGER transfer_segments_transform_guard_bu;
CREATE TRIGGER transfer_segments_transform_guard_bu
BEFORE UPDATE OF id, ai_run_id, outbound_text, outbound_hash, transform_manifest_id,
                 source_range_map, source_char_count
ON transfer_segments
WHEN NEW.id IS NOT OLD.id
  OR NEW.ai_run_id IS NOT OLD.ai_run_id
  OR NEW.outbound_text IS NOT OLD.outbound_text
  OR NEW.outbound_hash IS NOT OLD.outbound_hash
  OR NEW.transform_manifest_id IS NOT OLD.transform_manifest_id
  OR NEW.source_range_map IS NOT OLD.source_range_map
  OR NEW.source_char_count IS NOT OLD.source_char_count
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:transfer_segments');
END;

-- 0038'in kapsam tetikleyicisi genişletilir: **özne** de donar. Eski
-- koşullar (sürüm/dosya uyumu) aynen durur; ek olarak segment kimlikleri
-- değişemez. Aynı sütunları ayrı bir DIM_INTEGRITY tetikleyicisine koymak
-- hata kodunu SQLite'ın tetikleyici sırasına bırakırdı (v46 dersi).
DROP TRIGGER scope_guard_transfer_segments_bu;
CREATE TRIGGER scope_guard_transfer_segments_bu
BEFORE UPDATE OF ai_run_id, document_version_id, outbound_segment_id, source_segment_id
ON transfer_segments
WHEN NEW.outbound_segment_id IS NOT OLD.outbound_segment_id
   OR NEW.source_segment_id IS NOT OLD.source_segment_id
   OR (NEW.document_version_id IS NOT NULL AND EXISTS (
        SELECT 1 FROM document_versions dv
         WHERE dv.id = NEW.document_version_id
           AND dv.case_id <> (SELECT r.case_id FROM ai_runs r WHERE r.id = NEW.ai_run_id)
      ))
   OR (NEW.document_version_id IS NOT NULL AND EXISTS (
        SELECT 1
          FROM text_segments s
          JOIN parse_runs pr ON pr.id = s.parse_run_id
         WHERE s.id IN (NEW.outbound_segment_id, NEW.source_segment_id)
           AND pr.document_version_id IS NOT NEW.document_version_id
      ))
   OR (NEW.document_version_id IS NULL AND EXISTS (
        SELECT 1
          FROM text_segments s
          JOIN parse_runs pr ON pr.id = s.parse_run_id
         WHERE s.id IN (NEW.outbound_segment_id, NEW.source_segment_id)
           AND pr.case_id IS NOT (SELECT r.case_id FROM ai_runs r WHERE r.id = NEW.ai_run_id)
      ))
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:transfer_segments');
END;
