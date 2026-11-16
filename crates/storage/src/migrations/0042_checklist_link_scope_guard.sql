-- 0042: **kontrol listesi kanıt bağları kendi dosyasında olmalı** (INV-05, P0).
--
-- Neden: `case_checklist_items.linked_document_ids`, "bu madde **bulundu**"
-- iddiasının tek kanıtıdır (F08-T? / AI-07: incelenen kapsamda bulunan
-- belgeler). Çekirdek `set_case_checklist_state` her kimliği kendi dosyasında,
-- silinmemiş bir belge olarak doğruluyor; `present` durumu en az bir bağ
-- istiyor, `open`/`missing`/`not_applicable` bağ kabul etmiyor. Ama **şemada
-- hiçbir kapı yoktu**: sütun serbest metin, `scope_violations()` bu tabloyu
-- saymıyordu.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v41, `PRAGMA foreign_keys=ON`;
-- `scratch/schema-probe/measure_v42.py`):
--   (1) A dosyasının öğesine doğrudan SQL ile **B dosyasının** belgesi yazıldı
--       (`UPDATE … state='present', linked_document_ids='["d9"]'` → rows=1);
--   (2) A'nın listesi öğeyi `('c1','present','["d9"]')` diye döndürdü; belge
--       adı A dosyasında **çözülemiyor** (`NULL`) — ekranda `review`/kontrol
--       listesi satırında çıplak UUID görünür, "Bulundu" rozeti durur;
--   (3) `present` durumu **boş bağ listesiyle** yazılabildi: kanıtsız iddia;
--   (4) **bozuk JSON** (`oops`) yazılabildi; okuma yolu satırı çözemiyor →
--       `list_case_checklist` **tüm listeyi** hata ile düşürür (tek bozuk
--       satır ekranı kapatır — hata satır satır değil, liste düzeyinde);
--   (5) `missing` (bulunamadı) durumu **bağ listesiyle** yazılabildi (çelişki);
--   (6) v41 taraması bu tabloyu **hiç saymıyor** (alanı yok).
--
-- Yazılan kural (2 tetikleyici, INSERT + UPDATE):
--   * `linked_document_ids` **geçerli JSON dizi** olmalı, elemanları metin,
--     en çok 50 eleman (çekirdeğin `MAX_LINKS` sınırı);
--   * dizinin **her elemanı** `case_id` dosyasında **silinmemiş** bir
--     `documents` satırı göstermeli (fail-closed);
--   * durum ile liste çelişemez: `present` **en az bir** bağ ister;
--     `open`/`missing`/`not_applicable` bağ kabul etmez (çekirdeğin kuralları).
--   * `UPDATE OF case_id, state, linked_document_ids`: yalnız bu üç sütunu
--     değiştiren yazma kapıdan geçer. **Not** ve `row_version` güncellemesi
--     takılmaz (v35 dersi); çekirdek her yazmada üç sütunu birden yazdığı için
--     çağrı yolu kapıya takılmaz.
--
-- Kasıtlı gevşeklik (0033/0039 ile aynı): **sonradan silinmiş** belge kimliği
-- kapıya takılmaz (kapı yalnız yeni yazmada koşar) ve taranmaz — bağ kurulduktan
-- sonra belge silinebilir, bu tarihsel bir gerçektir. Tarama yalnız
-- **kanıtlanabilir** uyuşmazlığı sayar: başka dosyada var olan kimlik, bozuk/
-- array olmayan JSON, sınırı aşan liste, `present` + boş liste, bağ taşıyan
-- `open`/`missing`/`not_applicable` satırı.
--
-- Sınır (dürüst): kontrol listesi bağı bugün **ekranda** yalnız "bağlı
-- belgeler" listesinde ve "listede görünmeyen bağlar" uyarısında görünür;
-- kapı bugünkü görünür davranışı değiştirmez (çekirdek zaten reddediyor),
-- **duruktaki** dosya sınırını korur. Reddetme 0031–0041 ile aynı sözleşme:
-- `RAISE(ABORT,'DIM_SCOPE:case_checklist_items')` → `StorageError::ScopeViolation`
-- → **`E_DB_SCOPE`**.

CREATE TRIGGER scope_guard_case_checklist_items_bi
BEFORE INSERT ON case_checklist_items
WHEN CASE WHEN json_valid(NEW.linked_document_ids)
          THEN json_type(NEW.linked_document_ids) ELSE 'invalid' END <> 'array'
  OR (CASE WHEN json_valid(NEW.linked_document_ids)
                AND json_type(NEW.linked_document_ids) = 'array'
           THEN json_array_length(NEW.linked_document_ids) ELSE -1 END) > 50
  OR (NEW.state = 'present'
      AND (CASE WHEN json_valid(NEW.linked_document_ids)
                     AND json_type(NEW.linked_document_ids) = 'array'
                THEN json_array_length(NEW.linked_document_ids) ELSE -1 END) < 1)
  OR (NEW.state <> 'present'
      AND (CASE WHEN json_valid(NEW.linked_document_ids)
                     AND json_type(NEW.linked_document_ids) = 'array'
                THEN json_array_length(NEW.linked_document_ids) ELSE -1 END) <> 0)
  OR EXISTS (
       SELECT 1 FROM json_each(
         CASE WHEN json_valid(NEW.linked_document_ids)
              THEN NEW.linked_document_ids ELSE '[]' END) j
        WHERE j.type <> 'text'
           OR NOT EXISTS (
                SELECT 1 FROM documents d
                 WHERE d.id = j.value
                   AND d.case_id = NEW.case_id
                   AND d.deleted_at IS NULL))
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:case_checklist_items');
END;

CREATE TRIGGER scope_guard_case_checklist_items_bu
BEFORE UPDATE OF case_id, state, linked_document_ids ON case_checklist_items
WHEN CASE WHEN json_valid(NEW.linked_document_ids)
          THEN json_type(NEW.linked_document_ids) ELSE 'invalid' END <> 'array'
  OR (CASE WHEN json_valid(NEW.linked_document_ids)
                AND json_type(NEW.linked_document_ids) = 'array'
           THEN json_array_length(NEW.linked_document_ids) ELSE -1 END) > 50
  OR (NEW.state = 'present'
      AND (CASE WHEN json_valid(NEW.linked_document_ids)
                     AND json_type(NEW.linked_document_ids) = 'array'
                THEN json_array_length(NEW.linked_document_ids) ELSE -1 END) < 1)
  OR (NEW.state <> 'present'
      AND (CASE WHEN json_valid(NEW.linked_document_ids)
                     AND json_type(NEW.linked_document_ids) = 'array'
                THEN json_array_length(NEW.linked_document_ids) ELSE -1 END) <> 0)
  OR EXISTS (
       SELECT 1 FROM json_each(
         CASE WHEN json_valid(NEW.linked_document_ids)
              THEN NEW.linked_document_ids ELSE '[]' END) j
        WHERE j.type <> 'text'
           OR NOT EXISTS (
                SELECT 1 FROM documents d
                 WHERE d.id = j.value
                   AND d.case_id = NEW.case_id
                   AND d.deleted_at IS NULL))
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:case_checklist_items');
END;
