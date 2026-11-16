-- 0048: **emsal sürümünün metni ve özeti iyi-biçimli doğar ve donar**.
--
-- Neden: `precedent_versions` satırı «metin elimizde ve doğrulandı» iddiasıdır
-- (`verification_state='text_verified'`). `full_text` metin içi alıntı
-- doğrulamasının (F12) **tek** kaynağıdır; `text_char_count` ekranda basılır
-- (`screens/Precedents.tsx`); `body_hash` nüsha kimliğidir — v44 çakışma rozeti
-- «aynı künye, **farklı metin**» iddiasını bununla kanıtlar ve özetin ilk 12
-- hanesi UI'da görünür. v44 yalnız **çakışma işaretini** ve `body_hash`in
-- değişmezliğini kapıladı; içerik ve künye sütunları serbest kaldı.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v47; sonda
-- `scratch/dim-v48-probe/measure_v48.py`, log `measure_before.log`):
-- **14 kusur yazımının 14'ü KABUL**:
--   (1) `body_hash='hash'` (64 hane değil) — ekran onu özet diye bastı;
--   (2) `body_hash` **büyük** harf hex — aynı metin iki farklı kimlik olur;
--   (3) 63 haneli hash;
--   (4) `full_text=''` + `verification_state='text_verified'`;
--   (5) yalnız boşluktan oluşan metin + `text_verified`;
--   (6) `full_text=NULL` + `text_verified` — «doğrulandı» ama metin yok;
--   (7) `text_char_count` metinle uyuşmuyor (15 karakterlik metne 99);
--   (8) `source_kind='uydurma'` (kapalı küme dışı);
--   (9) `acquired_at=''` — sürüm sıralaması bozulur;
--  (10) yazılmış satırın `full_text`i UPDATE ile çevrildi — alıntı doğrulaması
--       başka metne bakar, «doğrulandı» iddiası yerinde kalır;
--  (11) `text_char_count` UPDATE ile değiştirildi;
--  (12) `acquired_at` UPDATE ile geriye çekildi;
--  (13) `source_kind` UPDATE ile değiştirildi (köken iddiası çevrildi);
--  (14) `document_version_id` başka sürüme çevrildi (köken çaprazlandı).
--
-- Yazılan kural (2 tetikleyici; v44'ün çakışma kapıları **aynen** durur —
-- ayrı tetikleyici, farklı sütunlar, aynı önek):
--   * BI (`precedent_versions_content_guard_bi`): `body_hash` **64 küçük harf
--     hex**; `full_text` NULL değil, boş/yalnız-boşluk değil ve **kırpılmış**
--     (çekirdek `trim()` sonrası yazar — kenar boşluğu kanonik değil);
--     `text_char_count` = `length(full_text)` (SQLite `length()` metin için
--     karakter sayar; çekirdek `chars().count()` yazar — aynı ölçü);
--     `acquired_at` boş değil; `source_kind` kapalı küme
--     (`paste`/`document`/`file` — `import_precedent_copy`in kabul ettiği üç
--     değer).
--   * BU (`precedent_versions_content_guard_bu`): `full_text`,
--     `text_char_count`, `acquired_at`, `source_kind` **değişmez** (yeniden
--     içe alma **yeni satır** ister; nüsha tekrarı `body_hash` ile yakalanır);
--     `document_version_id` yalnız **NULL'a çekilebilir** — belge sürümü
--     silinebildiği için bağın koparılması meşru onarımdır; başka sürüme
--     **çevrilemez** (köken iddiası sonradan taşınamaz).
--     `verification_state` serbest kalır (v35 dersi: durum ilerler).
--
-- Bilinçli: `precedents` silinirse sürümler `CASCADE` ile gider — kapı silmeyi
-- engellemez. Tablo `case_id` taşımaz; kapsam taramasına sınıf eklenmez
-- (`GUARDED_TABLES` 25 kalır).
--
-- Ret `RAISE(ABORT,'DIM_INTEGRITY:precedent_versions')` → `StorageError::Integrity`
-- → **`E_DB_INTEGRITY`**.

CREATE TRIGGER precedent_versions_content_guard_bi
BEFORE INSERT ON precedent_versions
WHEN NEW.body_hash IS NULL
  OR length(NEW.body_hash) <> 64
  OR NEW.body_hash GLOB '*[^0-9a-f]*'
  OR NEW.full_text IS NULL
  OR trim(NEW.full_text) = ''
  OR NEW.full_text <> trim(NEW.full_text)
  OR NEW.text_char_count IS NULL
  OR NEW.text_char_count <> length(NEW.full_text)
  OR NEW.acquired_at IS NULL
  OR trim(NEW.acquired_at) = ''
  OR NEW.source_kind IS NULL
  OR NEW.source_kind NOT IN ('paste', 'document', 'file')
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:precedent_versions');
END;

CREATE TRIGGER precedent_versions_content_guard_bu
BEFORE UPDATE OF full_text, text_char_count, acquired_at, source_kind, document_version_id
ON precedent_versions
WHEN NEW.full_text IS NOT OLD.full_text
  OR NEW.text_char_count IS NOT OLD.text_char_count
  OR NEW.acquired_at IS NOT OLD.acquired_at
  OR NEW.source_kind IS NOT OLD.source_kind
  OR (NEW.document_version_id IS NOT NULL
      AND NEW.document_version_id IS NOT OLD.document_version_id)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:precedent_versions');
END;
