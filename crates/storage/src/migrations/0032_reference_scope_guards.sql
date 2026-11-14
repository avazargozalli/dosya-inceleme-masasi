-- 0032: **kanıt ve iddia referansları** dosya sınırını aşamaz (INV-05, P0).
--
-- Neden: 0031, zincirin (belge → sürüm → koşu → segment → çapa) `case_id`
-- tutarlılığını şemada zorladı. Ama bir satırın **işaret ettiği** kayıt hâlâ
-- başka dosyadan olabiliyordu:
--
--   claims.source_anchor_id      → source_anchors.case_id
--   evidence_links.claim_id      → claims.case_id
--   evidence_links.source_anchor_id → source_anchors.case_id
--   event_sources.event_id       → events.case_id  (satırda case_id yok)
--   event_sources.source_anchor_id  → source_anchors.case_id
--   claim_relations.claim_id / related_claim_id → claims.case_id
--   tasks.source_anchor_id       → source_anchors.case_id
--
-- Sonuçları (ölçüldü, düzeltme öncesi): A dosyasının iddiası B dosyasının
-- alıntısına "kanıt" olarak bağlanabiliyordu; **B dosyasının belgesi
-- silindiğinde A'nın kanıt bağı `ON DELETE CASCADE` ile sessizce yok oluyor**,
-- iddianın çapası `SET NULL` ile düşüyor ve denetime tek satır bile
-- yazılmıyordu. `anchor_locate` gibi kimlikle okuyan yollar ise A dosyasında B
-- dosyasının belge adını/sayfasını/metnini gösterir. Bu P0 kabul engelidir.
--
-- Kural: referansın hedefi **kendi dosyasındaki** kayıt olmalı; hedef yoksa
-- satır yazılmaz (fail-closed). Reddetme sabit önekle yapılır
-- (`RAISE(ABORT,'DIM_SCOPE:<tablo>')` → `StorageError::ScopeViolation` →
-- `E_DB_SCOPE`), 0031 ile aynı sözleşme.
--
-- Kasıtlı olarak **yalnız dosya sınırı** denetlenir: çapa durumu
-- (`anchor_status` kullanılabilir mi), `deleted_at`, ilişki türü gibi ürün
-- politikaları çekirdek sorgusunda kalır. Tetikleyici politika öğrenirse eski
-- veri bir anda "yazılamaz" hâle gelir; kapı yalnız kapsamı korur.
--
-- Sınır (dürüst): tetikleyiciler **yeni** yazmaları engeller; bu şema
-- sürümünden önce yazılmış çapraz referanslar silinmez/değiştirilmez.
-- `Database::scope_violations()` onları sayar (kimlik taşımaz). JSON dizi
-- sütunları (`notes.source_anchor_ids`, `review_items.source_anchor_ids`,
-- `note_revisions.source_anchor_ids`, `note_drafts.source_anchor_ids`) bu
-- kapının dışında: çekirdek `validate_note_anchors` ile doğrular, şema düzeyinde
-- dizi elemanı denetlenmez (JSON1 sorgusu gerekir; ayrı iş).
--
-- Maliyet: satır başına bir birincil anahtar araması (indexed); iddia/kanıt
-- yazımı düşük hacimlidir. Ölçüm `crates/storage/tests/scope_guard_cost.rs`.

-- ---- claims: iddianın kanıtı kendi dosyasından ------------------------------
CREATE TRIGGER scope_guard_claims_bi
BEFORE INSERT ON claims
WHEN NEW.source_anchor_id IS NOT NULL
 AND NOT EXISTS (
  SELECT 1 FROM source_anchors a
   WHERE a.id = NEW.source_anchor_id AND a.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:claims');
END;

CREATE TRIGGER scope_guard_claims_bu
BEFORE UPDATE OF case_id, source_anchor_id ON claims
WHEN NEW.source_anchor_id IS NOT NULL
 AND NOT EXISTS (
  SELECT 1 FROM source_anchors a
   WHERE a.id = NEW.source_anchor_id AND a.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:claims');
END;

-- ---- evidence_links: iddia ve kanıt aynı dosyada ----------------------------
CREATE TRIGGER scope_guard_evidence_links_bi
BEFORE INSERT ON evidence_links
WHEN NOT EXISTS (
  SELECT 1 FROM claims c
   WHERE c.id = NEW.claim_id AND c.case_id = NEW.case_id
)
 OR NOT EXISTS (
  SELECT 1 FROM source_anchors a
   WHERE a.id = NEW.source_anchor_id AND a.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:evidence_links');
END;

CREATE TRIGGER scope_guard_evidence_links_bu
BEFORE UPDATE OF case_id, claim_id, source_anchor_id ON evidence_links
WHEN NOT EXISTS (
  SELECT 1 FROM claims c
   WHERE c.id = NEW.claim_id AND c.case_id = NEW.case_id
)
 OR NOT EXISTS (
  SELECT 1 FROM source_anchors a
   WHERE a.id = NEW.source_anchor_id AND a.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:evidence_links');
END;

-- ---- event_sources: olayın kaynağı olayın dosyasından (satırda case_id yok) --
CREATE TRIGGER scope_guard_event_sources_bi
BEFORE INSERT ON event_sources
WHEN NOT EXISTS (
  SELECT 1 FROM events e
    JOIN source_anchors a ON a.id = NEW.source_anchor_id
   WHERE e.id = NEW.event_id AND e.case_id = a.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:event_sources');
END;

CREATE TRIGGER scope_guard_event_sources_bu
BEFORE UPDATE OF event_id, source_anchor_id ON event_sources
WHEN NOT EXISTS (
  SELECT 1 FROM events e
    JOIN source_anchors a ON a.id = NEW.source_anchor_id
   WHERE e.id = NEW.event_id AND e.case_id = a.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:event_sources');
END;

-- ---- claim_relations: iki iddia aynı dosyada --------------------------------
CREATE TRIGGER scope_guard_claim_relations_bi
BEFORE INSERT ON claim_relations
WHEN NOT EXISTS (
  SELECT 1 FROM claims c
   WHERE c.id = NEW.claim_id AND c.case_id = NEW.case_id
)
 OR NOT EXISTS (
  SELECT 1 FROM claims c
   WHERE c.id = NEW.related_claim_id AND c.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:claim_relations');
END;

CREATE TRIGGER scope_guard_claim_relations_bu
BEFORE UPDATE OF case_id, claim_id, related_claim_id ON claim_relations
WHEN NOT EXISTS (
  SELECT 1 FROM claims c
   WHERE c.id = NEW.claim_id AND c.case_id = NEW.case_id
)
 OR NOT EXISTS (
  SELECT 1 FROM claims c
   WHERE c.id = NEW.related_claim_id AND c.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:claim_relations');
END;

-- ---- tasks: görevin dayanağı kendi dosyasından ------------------------------
CREATE TRIGGER scope_guard_tasks_bi
BEFORE INSERT ON tasks
WHEN NEW.source_anchor_id IS NOT NULL
 AND NOT EXISTS (
  SELECT 1 FROM source_anchors a
   WHERE a.id = NEW.source_anchor_id AND a.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:tasks');
END;

CREATE TRIGGER scope_guard_tasks_bu
BEFORE UPDATE OF case_id, source_anchor_id ON tasks
WHEN NEW.source_anchor_id IS NOT NULL
 AND NOT EXISTS (
  SELECT 1 FROM source_anchors a
   WHERE a.id = NEW.source_anchor_id AND a.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:tasks');
END;
