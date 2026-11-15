-- 0039: **aktarım onayı kendi dosyasının belgelerini gösterir** (INV-05, P0).
--
-- Neden: `transfer_consents` satırı "şu belgeler için aktarım izni verildi"
-- der — cihazdan **ne** çıkabileceğinin tek yetki kaydıdır. Çekirdek
-- `create_transfer_consent` bu listeyi doğruluyor (her kimlik dosyada,
-- silinmemiş bir belge olmalı; boş seçim hiçbir şeyi yetkilendirmez), ama
-- **şemada hiçbir kapı yoktu**: sütun serbest metin, tarama da bu tabloyu
-- saymıyordu (`scope_violations()` kör).
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v38; geçici sonda sonra silindi —
-- `crates/storage/tests/probe_consent_scope.rs`):
--   (1) A dosyasının onayına doğrudan SQL ile **B dosyasının belgesi** yazıldı
--       (`UPDATE … selected_document_ids='["d9"]'` → `Ok(1)`);
--   (2) `list_transfer_consents('c1')` A dosyasının onay envanterinde
--       `belge_sayisi=1 kimlikler=["d9"]` gösterdi — ekranın "onaylanan
--       belgeler" sütunu B dosyasının belgesini A'nın onaylanmış kapsamı diye
--       sayar;
--   (3) boş liste (`[]`) ve **bozuk JSON** (`bozuk-json`) onay satırı olarak
--       yazılabildi; bozuk satır `serde_json::from_str(...).unwrap_or_default()`
--       ile **sessizce** "0 belge" okunur (yetki kaydı çöker, kimse görmez);
--   (4) silinmiş belgenin kimliği de yazılabildi;
--   (5) `scope_violations()` bu tabloyu hiç saymıyordu (toplam **0**).
--
-- Dürüst sınır (bugünkü davranış): gönderim yolu çapraz kimliği **zaten**
-- düşürüyor — `consent_scope_segments` → `resolve_transfer_scope` dosya
-- süzgeciyle belgeyi `missing` diye atlar ve `ai_run_execute` gönderilecek
-- segment yoksa `E_AI_UNSUPPORTED` verir; yani çapraz onay satırı **bayt
-- çıkarmaz**. Kusur yetki **kaydında**: kayıt yanlış kapsam gösterir, envanter
-- yanlış sayar, tarama görmez. 0031–0038 ile aynı sınıf: doğrudan SQL yazan
-- bir yol çekirdek kapısını atlar.
--
-- Yazılan kural (2 tetikleyici, INSERT + UPDATE):
--   * `selected_document_ids` **geçerli JSON dizi** olmalı, elemanları metin,
--     listesi **boş olamaz** (boş onay hiçbir şeyi yetkilendirmez);
--   * dizinin **her elemanı** `case_scope` dosyasında **silinmemiş** bir
--     `documents` satırı göstermeli (yoksa satır yazılmaz — fail-closed).
--     Bu, çekirdeğin `create_transfer_consent` kapısının **aynısıdır**.
--   * `UPDATE OF case_scope, selected_document_ids`: yalnız bu iki sütunu
--     değiştiren yazma kapıdan geçer. **İptal** (`revoked_at`) ve süre
--     güncellemesi takılmaz — v35 dersi: boşta kalmış/eskimiş referans başka
--     alanın güncellemesini engellememeli.
--
-- Kasıtlı gevşeklik (0033 `note_drafts` / 0034 `ai_context_items` /
-- 0038 `transfer_segments` ile aynı): **var olmayan** ya da sonradan
-- **silinmiş** belge kimliği kapıya takılmaz ve taranmaz — onay verildikten
-- sonra belge silinmiş olabilir; bu bir denetim izidir, kanıtlanabilir bir
-- uyuşmazlık değildir (gönderim yolu onu yine atlar). Tarama yalnız
-- **başka dosyada var olan** kimliği, bozuk/boş listeyi sayar.
--
-- Kural 0031–0038 ile aynı: reddetme `RAISE(ABORT,'DIM_SCOPE:<tablo>')` →
-- `StorageError::ScopeViolation` → **`E_DB_SCOPE`**. `json_each`e her zaman
-- geçerli metin verilir (`CASE … ELSE '[]'`), `json_array_length` yalnız dizi
-- olduğu **doğrulanmış** değerde çağrılır — bozuk JSON'da SQLite'ın
-- "malformed JSON" hatası değil kapının kendi kodu döner.

CREATE TRIGGER scope_guard_transfer_consents_bi
BEFORE INSERT ON transfer_consents
WHEN NOT json_valid(NEW.selected_document_ids)
  OR json_type(NEW.selected_document_ids) <> 'array'
  OR (CASE WHEN json_valid(NEW.selected_document_ids)
                AND json_type(NEW.selected_document_ids) = 'array'
           THEN json_array_length(NEW.selected_document_ids) ELSE -1 END) = 0
  OR EXISTS (
       SELECT 1 FROM json_each(
         CASE WHEN json_valid(NEW.selected_document_ids)
              THEN NEW.selected_document_ids ELSE '[]' END) j
        WHERE j.type <> 'text'
           OR NOT EXISTS (
                SELECT 1 FROM documents d
                 WHERE d.id = j.value
                   AND d.case_id = NEW.case_scope
                   AND d.deleted_at IS NULL))
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:transfer_consents');
END;

CREATE TRIGGER scope_guard_transfer_consents_bu
BEFORE UPDATE OF case_scope, selected_document_ids ON transfer_consents
WHEN NOT json_valid(NEW.selected_document_ids)
  OR json_type(NEW.selected_document_ids) <> 'array'
  OR (CASE WHEN json_valid(NEW.selected_document_ids)
                AND json_type(NEW.selected_document_ids) = 'array'
           THEN json_array_length(NEW.selected_document_ids) ELSE -1 END) = 0
  OR EXISTS (
       SELECT 1 FROM json_each(
         CASE WHEN json_valid(NEW.selected_document_ids)
              THEN NEW.selected_document_ids ELSE '[]' END) j
        WHERE j.type <> 'text'
           OR NOT EXISTS (
                SELECT 1 FROM documents d
                 WHERE d.id = j.value
                   AND d.case_id = NEW.case_scope
                   AND d.deleted_at IS NULL))
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:transfer_consents');
END;
