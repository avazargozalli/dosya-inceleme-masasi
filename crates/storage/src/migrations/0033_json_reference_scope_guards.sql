-- 0033: **JSON dizi** kanıt referansları da dosya sınırını aşamaz (INV-05, P0).
--
-- Neden: 0032, `claims`/`evidence_links`/`event_sources`/`claim_relations`/
-- `tasks` üzerindeki **tek sütunlu** kanıt referanslarını şemada kapıladı. Ama
-- kanıt listesi taşıyan dört tablo bir **JSON dizi** sütunu kullanıyor ve bu
-- sütunlar kapının dışında kalmıştı:
--
--   notes.source_anchor_ids          → source_anchors.case_id
--   note_revisions.source_anchor_ids → üst notun (notes) case_id'si
--   review_items.source_anchor_ids   → source_anchors.case_id
--   note_drafts.source_anchor_ids    → source_anchors.case_id
--
-- Sonuç (ölçüldü, düzeltme öncesi): A dosyasının notu B dosyasının çapasını
-- "kaynağım" diye taşıyabiliyordu; notu açan kullanıcı B dosyasının belge
-- adını/sayfasını görüyor, B dosyası silinince kaynak sessizce düşüyordu.
-- Çekirdek `validate_note_anchors` bunu doğruluyor, ama **doğrudan SQL** yazan
-- bir yol o kapıyı atlar; kapsam ihlali P0 kabul engelidir.
--
-- Kural: dizinin **her elemanı** kendi dosyasındaki bir `source_anchors`
-- kaydını göstermeli; gösteremiyorsa satır yazılmaz (fail-closed). Reddetme
-- 0031/0032 ile aynı sözleşmeyi kullanır (`RAISE(ABORT,'DIM_SCOPE:<tablo>')` →
-- `StorageError::ScopeViolation` → `E_DB_SCOPE`).
--
-- JSON bozuksa ya da dizi değilse satır **yine** reddedilir: doğrulanamayan
-- referans yazılmaz. `json_each`e her zaman geçerli bir metin verilir
-- (`CASE … ELSE '[]'`), böylece SQLite'ın "malformed JSON" hatasına düşmeden
-- kapının kendi kodu döner.
--
-- Kasıtlı ayrım — **taslak** (`note_drafts`): taslak katmanı bugüne dek kaynağı
-- doğrulamaz (yarıda kalan otomatik kayıt kullanıcının metnini kaybettirmemeli,
-- `save_note_draft` sözleşmesi). Şema kapısı bu davranışı **bozmaz**: var
-- olmayan/doğrulanmamış kimlik taslakta kalabilir. Yalnız **başka dosyada var
-- olan** çapa reddedilir — taslak, başka dosyanın kanıtına işaret edemez.
--
-- Kapsam dışı (dürüst): `notes.undo_stack` / `redo_stack` birer **anlık
-- görüntü**dür (revizyon numarası yığını), referans değildir; geri/ileri alma
-- zaten `validate_note_anchors`dan geçer. `payload` içindeki serbest JSON da
-- referans sayılmaz (şema düzeyinde anlam bilinmiyor).
--
-- Sınır (dürüst): tetikleyiciler **yeni** yazmaları engeller; bu sürümden önce
-- yazılmış çapraz diziler silinmez/değiştirilmez. `Database::scope_violations()`
-- onları sayar (kimlik taşımaz).
--
-- Maliyet: satır başına bir JSON çözümlemesi + eleman başına bir indeksli
-- arama; not/inceleme yazımı düşük hacimlidir (ölçüm
-- `crates/storage/tests/scope_guard_cost.rs`).

-- ---- notes: notun kaynak listesi kendi dosyasından --------------------------
CREATE TRIGGER scope_guard_notes_bi
BEFORE INSERT ON notes
WHEN NOT json_valid(NEW.source_anchor_ids)
  OR EXISTS (
  SELECT 1 FROM json_each(
      CASE WHEN json_valid(NEW.source_anchor_ids) THEN NEW.source_anchor_ids ELSE '[]' END) j
   WHERE j.type <> 'text'
      OR NOT EXISTS (
        SELECT 1 FROM source_anchors a
         WHERE a.id = j.value AND a.case_id = NEW.case_id)
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:notes');
END;

CREATE TRIGGER scope_guard_notes_bu
BEFORE UPDATE OF case_id, source_anchor_ids ON notes
WHEN NOT json_valid(NEW.source_anchor_ids)
  OR EXISTS (
  SELECT 1 FROM json_each(
      CASE WHEN json_valid(NEW.source_anchor_ids) THEN NEW.source_anchor_ids ELSE '[]' END) j
   WHERE j.type <> 'text'
      OR NOT EXISTS (
        SELECT 1 FROM source_anchors a
         WHERE a.id = j.value AND a.case_id = NEW.case_id)
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:notes');
END;

-- ---- note_revisions: revizyonun dosyası üst notun dosyasıdır ---------------
-- Satırda `case_id` yok; dosya **üst nottan** okunur. Üst not yoksa yazma
-- reddedilir (fail-closed) — FK zaten engeller, kapı niyeti açık tutar.
CREATE TRIGGER scope_guard_note_revisions_bi
BEFORE INSERT ON note_revisions
WHEN NOT json_valid(NEW.source_anchor_ids)
  OR NOT EXISTS (SELECT 1 FROM notes n WHERE n.id = NEW.note_id)
  OR EXISTS (
  SELECT 1 FROM json_each(
      CASE WHEN json_valid(NEW.source_anchor_ids) THEN NEW.source_anchor_ids ELSE '[]' END) j
   WHERE j.type <> 'text'
      OR NOT EXISTS (
        SELECT 1 FROM source_anchors a
          JOIN notes n ON n.id = NEW.note_id
         WHERE a.id = j.value AND a.case_id = n.case_id)
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:note_revisions');
END;

CREATE TRIGGER scope_guard_note_revisions_bu
BEFORE UPDATE OF note_id, source_anchor_ids ON note_revisions
WHEN NOT json_valid(NEW.source_anchor_ids)
  OR NOT EXISTS (SELECT 1 FROM notes n WHERE n.id = NEW.note_id)
  OR EXISTS (
  SELECT 1 FROM json_each(
      CASE WHEN json_valid(NEW.source_anchor_ids) THEN NEW.source_anchor_ids ELSE '[]' END) j
   WHERE j.type <> 'text'
      OR NOT EXISTS (
        SELECT 1 FROM source_anchors a
          JOIN notes n ON n.id = NEW.note_id
         WHERE a.id = j.value AND a.case_id = n.case_id)
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:note_revisions');
END;

-- ---- review_items: önerinin kanıtı kendi dosyasından ------------------------
CREATE TRIGGER scope_guard_review_items_bi
BEFORE INSERT ON review_items
WHEN NOT json_valid(NEW.source_anchor_ids)
  OR EXISTS (
  SELECT 1 FROM json_each(
      CASE WHEN json_valid(NEW.source_anchor_ids) THEN NEW.source_anchor_ids ELSE '[]' END) j
   WHERE j.type <> 'text'
      OR NOT EXISTS (
        SELECT 1 FROM source_anchors a
         WHERE a.id = j.value AND a.case_id = NEW.case_id)
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:review_items');
END;

CREATE TRIGGER scope_guard_review_items_bu
BEFORE UPDATE OF case_id, source_anchor_ids ON review_items
WHEN NOT json_valid(NEW.source_anchor_ids)
  OR EXISTS (
  SELECT 1 FROM json_each(
      CASE WHEN json_valid(NEW.source_anchor_ids) THEN NEW.source_anchor_ids ELSE '[]' END) j
   WHERE j.type <> 'text'
      OR NOT EXISTS (
        SELECT 1 FROM source_anchors a
         WHERE a.id = j.value AND a.case_id = NEW.case_id)
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:review_items');
END;

-- ---- note_drafts: taslak metni kurtarır, başka dosyanın kanıtını taşımaz ----
-- Bilinçli olarak **daha gevşek**: var olmayan ya da doğrulanmamış kimlik
-- taslakta kalabilir (otomatik kayıt kullanıcının metnini kaybettirmemeli).
-- Ama **başka dosyada var olan** çapa reddedilir: taslak, A dosyasının
-- ekranında B dosyasının kanıtına işaret edemez.
CREATE TRIGGER scope_guard_note_drafts_bi
BEFORE INSERT ON note_drafts
WHEN NOT json_valid(NEW.source_anchor_ids)
  OR EXISTS (
  SELECT 1 FROM json_each(
      CASE WHEN json_valid(NEW.source_anchor_ids) THEN NEW.source_anchor_ids ELSE '[]' END) j
    JOIN source_anchors a ON a.id = j.value
   WHERE a.case_id <> NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:note_drafts');
END;

CREATE TRIGGER scope_guard_note_drafts_bu
BEFORE UPDATE OF case_id, source_anchor_ids ON note_drafts
WHEN NOT json_valid(NEW.source_anchor_ids)
  OR EXISTS (
  SELECT 1 FROM json_each(
      CASE WHEN json_valid(NEW.source_anchor_ids) THEN NEW.source_anchor_ids ELSE '[]' END) j
    JOIN source_anchors a ON a.id = j.value
   WHERE a.case_id <> NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:note_drafts');
END;
