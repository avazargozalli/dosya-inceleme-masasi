-- 0064: **denetim defteri** — `audit_events` satırı «bu işlem şu aktörce, şu
-- anda yapıldı ve kendinden önceki kayda bağlıdır» iddiasıdır; defter
-- **eklemelidir**: doğuşta zincire bağlı ve iyi biçimli olmalı, sonrasında
-- hiçbir sütunu değişmemeli, satır silinmemelidir.
--
-- Neden: aynı satır üç yerde kanıt olarak okunur:
--   * `audit::walk_chain` / `verify_audit_chain` / `audit_chain_status` —
--     zincir yürüyüşü `prior_hash`in **önceki satırın** `event_hash`ine eşit
--     olduğunu ve `event_hash`in yeniden hesapla tuttuğunu ölçer. Kopukluk
--     «kurcalama» diye raporlanır (Settings ekranı «zincir doğrulandı /
--     kırıldı» satırını buradan basar).
--   * `ai.rs` — `supersede` sayımı (`WHERE operation='supersede' AND
--     entity_id=?`) ve sağlayıcı yanıtının kayda geçtiğinin kanıtı
--     (`metadata_allowlist` alanı) **doğrudan** bu satırlardan okunur.
--   * `blob_disk.rs` — `blob_disk_sweep` kayıtları (`category='blob_disk'`).
-- Tablo 0001'den beri **tetikleyicisizdi**: tek kapı `id` UNIQUE'iydi.
-- `append_audit` zinciri kendisi kurar (prior = `ORDER BY sequence DESC LIMIT
-- 1`), ama doğrudan SQL yazan yol bunu atlayabilir; doğmuş satır sonradan
-- **her** sütunuyla değiştirilebiliyor, silinebiliyordu.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v63; sonda
-- `scratch/dim-v64-probe/measure_v64.py`, log `measure_before.log`):
-- 41 yazımın **41'i KABUL** (39 kusur + 1 meşru ekleme + 1 no-op güncelleme).
-- 29 doğum kusuru: boş / kırpılmamış / 65 haneli
-- kimlik; boş, yalnız boşluk ve kırpılmamış kategori / işlem / aktör; boş ve
-- kırpılmamış `entity_id`; bozuk JSON, dizi, `null` ve metin
-- `metadata_allowlist`; 63 haneli / büyük harf / hex olmayan `event_hash`;
-- `dün`, yalnız gün, olmayan gün (`2026-02-30`), olmayan saat (`99:00:00`) ve
-- saniyesiz damga; **zincir kopuk** `prior_hash` (rastgele 64 hex), zincirde
-- satır varken `prior_hash NULL`, 63 haneli ve büyük harf prior; açıkça
-- **düşük `sequence`** (satır defterin başına düşer). 9 UPDATE kusuru: işlem,
-- kategori, aktör, metadata, özet, prior, damga, kimlik ve `entity_id`
-- çevrildi. 1 silme kusuru: kayıt silindi.
--
-- Ret `DIM_INTEGRITY:audit_events` → `E_DB_INTEGRITY`. Tablo `case_id`
-- taşımaz → `GUARDED_TABLES` **25 kalır**.

CREATE TRIGGER audit_events_evidence_birth
BEFORE INSERT ON audit_events
BEGIN
  -- Kimlik ve metin sütunları: kırpılmış, dolu, sınırlı.
  SELECT CASE
    WHEN NEW.id IS NULL OR length(trim(NEW.id))=0 OR NEW.id<>trim(NEW.id)
      OR length(NEW.id)>64
    THEN RAISE(ABORT,'DIM_INTEGRITY:audit_events')
  END;
  SELECT CASE
    WHEN NEW.category IS NULL OR length(trim(NEW.category))=0
      OR NEW.category<>trim(NEW.category) OR length(NEW.category)>64
    THEN RAISE(ABORT,'DIM_INTEGRITY:audit_events')
  END;
  SELECT CASE
    WHEN NEW.operation IS NULL OR length(trim(NEW.operation))=0
      OR NEW.operation<>trim(NEW.operation) OR length(NEW.operation)>64
    THEN RAISE(ABORT,'DIM_INTEGRITY:audit_events')
  END;
  SELECT CASE
    WHEN NEW.actor_label IS NULL OR length(trim(NEW.actor_label))=0
      OR NEW.actor_label<>trim(NEW.actor_label) OR length(NEW.actor_label)>64
    THEN RAISE(ABORT,'DIM_INTEGRITY:audit_events')
  END;
  SELECT CASE
    WHEN NEW.entity_id IS NOT NULL AND (length(trim(NEW.entity_id))=0
      OR NEW.entity_id<>trim(NEW.entity_id) OR length(NEW.entity_id)>64)
    THEN RAISE(ABORT,'DIM_INTEGRITY:audit_events')
  END;
  -- `metadata_allowlist` bir **nesne** olmalı: izin listesi anahtar/değer
  -- taşır. Dizi, `null`, metin ve bozuk JSON kanıt değildir.
  SELECT CASE
    WHEN NEW.metadata_allowlist IS NULL
      OR NOT json_valid(NEW.metadata_allowlist)
      OR json_type(NEW.metadata_allowlist)<>'object'
    THEN RAISE(ABORT,'DIM_INTEGRITY:audit_events')
  END;
  -- Damga: `YYYY-MM-DDTHH:MM:SS.uuuuuuZ` — sabit genişlik ve **gerçek**
  -- takvim günü/saati (`date()` eşitliği hem olmayan günü hem olmayan saati
  -- yakalar: `2026-02-30`, `10:00:60` → `date()` NULL).
  SELECT CASE
    WHEN NEW.timestamp IS NULL OR length(NEW.timestamp)<>27
      OR NEW.timestamp NOT GLOB
        '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9][0-9][0-9][0-9]Z'
      OR date(NEW.timestamp) IS NOT substr(NEW.timestamp,1,10)
    THEN RAISE(ABORT,'DIM_INTEGRITY:audit_events')
  END;
  -- Özetler: 64 küçük harf hex.
  SELECT CASE
    WHEN NEW.event_hash IS NULL OR length(NEW.event_hash)<>64
      OR NEW.event_hash GLOB '*[^0-9a-f]*'
    THEN RAISE(ABORT,'DIM_INTEGRITY:audit_events')
  END;
  SELECT CASE
    WHEN NEW.prior_hash IS NOT NULL AND (length(NEW.prior_hash)<>64
      OR NEW.prior_hash GLOB '*[^0-9a-f]*')
    THEN RAISE(ABORT,'DIM_INTEGRITY:audit_events')
  END;
  -- **Zincir bağı doğuşta kurulur**: prior, defterin son satırının özetidir;
  -- defter boşsa NULL'dır. (`append_audit` aynı kuralı uygular.)
  SELECT CASE
    WHEN NEW.prior_hash IS NOT
      (SELECT event_hash FROM audit_events ORDER BY sequence DESC LIMIT 1)
    THEN RAISE(ABORT,'DIM_INTEGRITY:audit_events')
  END;
  -- Satır defterin **sonuna** eklenir: açıkça verilen düşük `sequence`
  -- geçmişin önüne düşer. (AUTOINCREMENT değeri BEFORE tetikleyicide
  -- tanımsızdır; sıra kuralı AFTER tetikleyicide ölçülür.)
END;

CREATE TRIGGER audit_events_evidence_appended
AFTER INSERT ON audit_events
WHEN EXISTS (SELECT 1 FROM audit_events WHERE sequence > NEW.sequence)
BEGIN
  SELECT RAISE(ABORT,'DIM_INTEGRITY:audit_events');
END;

-- Defter eklemelidir: doğmuş satırın hiçbir sütunu değişmez. No-op yazma
-- (aynı değerler) takılmaz.
CREATE TRIGGER audit_events_evidence_frozen
BEFORE UPDATE ON audit_events
WHEN NEW.sequence IS NOT OLD.sequence
  OR NEW.id IS NOT OLD.id
  OR NEW.category IS NOT OLD.category
  OR NEW.entity_id IS NOT OLD.entity_id
  OR NEW.operation IS NOT OLD.operation
  OR NEW.actor_label IS NOT OLD.actor_label
  OR NEW.metadata_allowlist IS NOT OLD.metadata_allowlist
  OR NEW.prior_hash IS NOT OLD.prior_hash
  OR NEW.event_hash IS NOT OLD.event_hash
  OR NEW.timestamp IS NOT OLD.timestamp
BEGIN
  SELECT RAISE(ABORT,'DIM_INTEGRITY:audit_events');
END;

-- Kayıt silinemez: silinen satır zinciri koparır ve «yapıldı» kanıtı yok eder.
-- (Tabloda CASCADE bağı yok; dosya silme ürün yolu bu defteri etkilemez.)
CREATE TRIGGER audit_events_evidence_no_delete
BEFORE DELETE ON audit_events
BEGIN
  SELECT RAISE(ABORT,'DIM_INTEGRITY:audit_events');
END;
