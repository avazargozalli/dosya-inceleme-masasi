-- 0040: **karar hedefi kendi dosyasında olmalı** (INV-05, P0).
--
-- Neden: `review_items.target_entity_id`, avukatın onayladığı önerinin dönüştüğü
-- kalıcı kaydı gösterir (`claims` / `notes` / `tasks` / `events`). Arayüz bu
-- kimliği `review.targetBadge` ile **ilk 8 karakter** olarak A dosyasının
-- öğesinde gösterir; öğe "karara bağlandı" sayılır (`target_entity_id IS NULL`
-- şartı yüzünden bir daha karara bağlanamaz). Şemada hiçbir kapı yoktu.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v39; `scratch/schema-probe/measure_v40.py`,
-- log `scratch/dim-v40-measure.log`):
--   (1) doğrudan SQL ile A dosyasının öğesine **B dosyasının** `claim9` / `n9`
--       kimliği yazıldı → **KABUL** (rows=1);
--   (2) `list_review_items('c1')` onu A'nın öğesi diye döndürdü → ekranda B
--       dosyasının kaydının kimliği A'nın kararı sanılır;
--   (3) `scope_violations()` bu tabloyu sayıyor ama `target_entity_id` alanını
--       **hiç denetlemiyordu** → toplam **0** (kör);
--   (4) karar kapısı (`… AND target_entity_id IS NULL`) yanlış hedefle
--       **kilitleniyordu** (`changed=0`): öğe bir daha karara bağlanamaz.
--
-- Yazılan kural (0033'ün iki tetikleyicisi **DROP + CREATE** ile genişletildi,
-- tek tanım): `target_entity_id` doluysa kimlik `claims` / `notes` / `tasks` /
-- `events` tablolarından **birinde var** ise satırın `case_id`'si aynı olmalı;
-- yoksa satır yazılmaz (fail-closed). `UPDATE OF case_id, source_anchor_ids,
-- target_entity_id`: yalnız bu üç sütunu değiştiren yazma kapıdan geçer —
-- durum/`row_version` güncellemesi takılmaz (v35 dersi).
--
-- Kasıtlı gevşeklik (0033/0034/0038/0039 ile aynı): **var olmayan** ya da
-- sonradan **silinmiş** hedef kimliği ne kapıya takılır ne taranır — karar
-- verildikten sonra kayıt silinmiş olabilir; bu bir denetim izidir, kanıtlanabilir
-- uyuşmazlık değildir. Tarama yalnız **başka dosyada var olan** kimliği sayar.
--
-- Sınır (dürüst): hedef kimliğin **türü** ile `item_kind` karşılaştırılmaz
-- (şemada tür bilgisi tek sütunda değil); kapı kimliğin **dosyasını** korur,
-- "bu öğe bu tür kayda dönüştü" iddiasını kanıtlamaz. Reddetme 0031–0039 ile
-- aynı sözleşme: `RAISE(ABORT,'DIM_SCOPE:review_items')` → `StorageError::ScopeViolation`
-- → **`E_DB_SCOPE`**.

DROP TRIGGER IF EXISTS scope_guard_review_items_bi;
DROP TRIGGER IF EXISTS scope_guard_review_items_bu;

-- ---- review_items: kanıt listesi (0033) + karar hedefi (0040) ----------------
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
  OR (
  NEW.target_entity_id IS NOT NULL AND EXISTS (
    SELECT 1 FROM claims x
     WHERE x.id = NEW.target_entity_id AND x.case_id <> NEW.case_id
    UNION ALL
    SELECT 1 FROM notes x
     WHERE x.id = NEW.target_entity_id AND x.case_id <> NEW.case_id
    UNION ALL
    SELECT 1 FROM tasks x
     WHERE x.id = NEW.target_entity_id AND x.case_id <> NEW.case_id
    UNION ALL
    SELECT 1 FROM events x
     WHERE x.id = NEW.target_entity_id AND x.case_id <> NEW.case_id)
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:review_items');
END;

CREATE TRIGGER scope_guard_review_items_bu
BEFORE UPDATE OF case_id, source_anchor_ids, target_entity_id ON review_items
WHEN NOT json_valid(NEW.source_anchor_ids)
  OR EXISTS (
  SELECT 1 FROM json_each(
      CASE WHEN json_valid(NEW.source_anchor_ids) THEN NEW.source_anchor_ids ELSE '[]' END) j
   WHERE j.type <> 'text'
      OR NOT EXISTS (
        SELECT 1 FROM source_anchors a
         WHERE a.id = j.value AND a.case_id = NEW.case_id)
)
  OR (
  NEW.target_entity_id IS NOT NULL AND EXISTS (
    SELECT 1 FROM claims x
     WHERE x.id = NEW.target_entity_id AND x.case_id <> NEW.case_id
    UNION ALL
    SELECT 1 FROM notes x
     WHERE x.id = NEW.target_entity_id AND x.case_id <> NEW.case_id
    UNION ALL
    SELECT 1 FROM tasks x
     WHERE x.id = NEW.target_entity_id AND x.case_id <> NEW.case_id
    UNION ALL
    SELECT 1 FROM events x
     WHERE x.id = NEW.target_entity_id AND x.case_id <> NEW.case_id)
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:review_items');
END;
