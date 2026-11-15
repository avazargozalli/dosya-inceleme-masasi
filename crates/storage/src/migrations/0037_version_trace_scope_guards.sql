-- 0037: **sürüm izi taşıyan siciller** kendi dosyasına bağlıdır (INV-05, P0).
--
-- Neden: 0031–0036 zinciri, kanıt referanslarını, JSON dizilerini, soy ağacını,
-- maskeleme çıktısını, gönderim sicilini ve "güncel sürüm" işaretçisini kapıladı.
-- Şema taraması iki sicili **hiç görmüyordu**: ikisi de `case_id` taşımıyor,
-- kapsamı üst kaydından miras alıyor ve gösterdikleri sürüm kimliği serbest metin:
--
--   citation_checks.input_document_version_id → documents.case_id (sürümün belgesi)
--   export_items.source_version_id            → exports.case_id (sahibi üst satır)
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v36; geçici sonda sonra silindi):
--
--   (1) `record_citation_check` **hiçbir kapı taşımıyordu**: A dosyası adına B
--       dosyasının sürümüne atıf kontrolü yazıldı (`Ok(<id>)`) ve
--       `list_citation_checks('v9')` **B dosyasının ekranında** A'nın
--       belgesinden çıkarılmış metni gösterdi («gizli tutanak metni») — INV-05.
--       Var olmayan sürüm kimliği de kabul ediliyordu (satır sessizce kayıp).
--   (2) `record_export` **çekirdek** kapısı doğru çalışıyor (çapraz sürüm
--       `E_EXPORT_SOURCE` ile reddedildi) ama şemada kapı yoktu: doğrudan SQL
--       ile A'nın dışa aktarmasına B'nin sürümü yazıldı ve `list_export_items`
--       A'nın kayıt defterinde **B'nin belge adını** («Diger dosya belgesi»)
--       gösterdi.
--   Her iki durumda `scope_violations()` **0** dedi (kör).
--
-- Yazılan:
--
--   * `citation_checks.case_id` — satır **hangi dosyada** yazıldı. Göç: eski
--     satırlar başvurdukları sürümün dosyasına atfedilir; başka bilgi yok.
--     **Dürüst sınır:** v37 öncesi yazılmış çapraz satır bu göçle ayırt
--     **edilemez** (satırda yazan dosya bilgisi hiç yoktu); bu satırların
--     temizliği ürün kararıdır. Kapı yeni yazmayı kapatır.
--   * **4 tetikleyici** (2 tablo × BEFORE INSERT/UPDATE): satırın gösterdiği
--     sürüm **başka dosyada var ise** yazma reddedilir. `citation_checks`
--     kendi `case_id`'sini taşır (zorunlu); `export_items` dosyayı `exports`
--     satırından okur (kendi `case_id`'si yok — 0034'ün `ai_context_items`
--     kuralıyla aynı: kapsam üst kayıttan miras alınır).
--   * `Database::scope_violations()` iki tabloyu da sayar.
--
-- Kasıtlı gevşeklik (0033 `note_drafts` / 0034 `ai_context_items` ile aynı):
-- **var olmayan** sürüm kimliği sicilde kalabilir — bu iki tabloda
-- `source_version_id`/`input_document_version_id` yabancı anahtar değildir
-- (denetim izi; belge/sürüm sonradan silinebilir, satır yaşamalıdır). Kapı
-- yalnız **başka dosyada var olan** sürümü reddeder. Bu yüzden tarama da
-- yalnız "var olan ama çapraz" satırı sayar.
--
-- Kural 0031–0036 ile aynı: hedef **kendi dosyasında** değilse satır yazılmaz
-- (fail-closed). Reddetme `RAISE(ABORT, 'DIM_SCOPE:<tablo>')` →
-- `StorageError::ScopeViolation` → **`E_DB_SCOPE`**.

ALTER TABLE citation_checks ADD COLUMN case_id TEXT;

-- Göç: eski satırın dosyası, başvurduğu sürümün belgesinden okunur. Sürümü
-- silinmiş satırda `NULL` kalır (uydurulmaz) — tarama onu "aidiyeti yazılmamış"
-- diye sayar, liste göstermez.
UPDATE citation_checks
   SET case_id = (
        SELECT d.case_id
          FROM document_versions v
          JOIN documents d ON d.id = v.document_id
         WHERE v.id = citation_checks.input_document_version_id
   );

-- Atıf kontrolü: satır kendi dosyasını **taşır** (NULL olamaz) ve gösterdiği
-- sürüm başka dosyada **var ise** reddedilir.
CREATE TRIGGER scope_guard_citation_checks_bi
BEFORE INSERT ON citation_checks
WHEN NEW.case_id IS NULL
   OR EXISTS (
        SELECT 1
          FROM document_versions v
          JOIN documents d ON d.id = v.document_id
         WHERE v.id = NEW.input_document_version_id
           AND d.case_id <> NEW.case_id
   )
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:citation_checks');
END;

CREATE TRIGGER scope_guard_citation_checks_bu
BEFORE UPDATE OF input_document_version_id, case_id ON citation_checks
WHEN NEW.case_id IS NULL
   OR EXISTS (
        SELECT 1
          FROM document_versions v
          JOIN documents d ON d.id = v.document_id
         WHERE v.id = NEW.input_document_version_id
           AND d.case_id <> NEW.case_id
   )
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:citation_checks');
END;

-- Dışa aktarma kalemi: dosya, kalemin **üst kaydından** (`exports`) okunur.
CREATE TRIGGER scope_guard_export_items_bi
BEFORE INSERT ON export_items
WHEN EXISTS (
  SELECT 1
    FROM exports e
    JOIN document_versions v ON v.id = NEW.source_version_id
    JOIN documents d ON d.id = v.document_id
   WHERE e.id = NEW.export_id
     AND e.case_id <> d.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:export_items');
END;

CREATE TRIGGER scope_guard_export_items_bu
BEFORE UPDATE OF export_id, source_version_id ON export_items
WHEN EXISTS (
  SELECT 1
    FROM exports e
    JOIN document_versions v ON v.id = NEW.source_version_id
    JOIN documents d ON d.id = v.document_id
   WHERE e.id = NEW.export_id
     AND e.case_id <> d.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:export_items');
END;
