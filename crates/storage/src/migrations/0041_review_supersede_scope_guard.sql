-- 0041: **yerine geçen kayıt kendi dosyasında ve imlenmiş olmalı** (INV-05, P0).
--
-- Neden: `review_items.superseded_by_item_id`, "bu öğenin yerine hangi öğe geçti"
-- bağını tutar. Arayüzde gerçek bir eylem var (`review.supersedeAction` düğmesi →
-- `review_supersede` → `supersede_review_item`), çekirdek de aynı dosya, kendini
-- göstermeme ve durum kurallarını denetler — ama **şemada hiçbir kapı yoktu**:
-- doğrudan SQL yazan bir yol (göç, onarım betiği, ileride eklenecek ikinci yazıcı)
-- bu kuralları atlar. 0006 sütunu `REFERENCES review_items(id) ON DELETE SET NULL`
-- ile ekledi; yabancı anahtar **satırın varlığını** korur, **dosyasını** korumaz.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v40, `PRAGMA foreign_keys=ON`;
-- `scratch/schema-probe/measure_v41.py`, log `scratch/dim-v41-measure.log`):
--   (1) A dosyasının öğesine **B dosyasının** `j9` öğesi "yerine geçen" yazıldı
--       → **KABUL** (rows=1); yabancı anahtar aynı tabloya baktığı için geçer;
--   (2) öğe **kendini** gösterdi (`i2 → i2`) → **KABUL**; çekirdek bunu reddeder;
--   (3) **damgasız** yazım (`superseded_by_item_id` dolu, `superseded_at` NULL)
--       → **KABUL**; `list_review_items('c1')` öğeyi **hâlâ kuyrukta, karara
--       açık** gösterir (durum `superseded_at`ten türetilir) — yani satır
--       "yerine geçildi" derken ekran "bekliyor" der;
--   (4) aynı satırda çekirdeğin gerçek `supersede_review_item` UPDATE'i
--       (`… WHERE id=? AND superseded_at IS NULL`) **çalıştı** (`changed=1`) ve
--       ilk işareti sessizce ezdi — denetim izi ile satır çelişir;
--   (5) `list_review_items('c1', 'superseded')` çapraz işaretli kaydı A
--       dosyasının öğesi diye döndürdü (`i1 → j9`, j9 c2'de);
--   (6) v40 taraması **0** dedi (sınıf kör); önerilen koşul **3** buldu.
--
-- Yazılan kural (0033/0040'ın iki tetikleyicisi **DROP + CREATE** ile
-- genişletildi, tek tanım): `superseded_by_item_id` doluysa
--   (a) `superseded_at` de **dolu** olmalı (yarım yazım yok — imlenmemiş bağ
--       saklanmaz),
--   (b) satır **kendini** gösteremez,
--   (c) kimlik `review_items`ta **başka dosyada var** ise satır yazılmaz.
-- `UPDATE OF case_id, source_anchor_ids, target_entity_id, superseded_by_item_id,
-- superseded_at`: yalnız bu beş sütunu değiştiren yazma kapıdan geçer —
-- durum/`row_version` güncellemesi takılmaz (v35 dersi).
--
-- Kasıtlı gevşeklik (0033/0040 ile aynı): **var olmayan** yerine-geçen kimliği ne
-- kapıya takılır ne taranır; 0006'nın yabancı anahtarı zaten var olmayan satırı
-- yazdırmaz ve hedef silinirse işareti `NULL`'a çeker (`ON DELETE SET NULL`) —
-- damga kalır, bu yüzden (a) tek yönlüdür. Yerine geçen öğenin **durumu**
-- denetlenmez: bugün "pending" olan yerine-geçen sonradan reddedilebilir; bu
-- tarihsel bir gerçektir, kanıtlanabilir uyuşmazlık değildir (durum politikası
-- çekirdeğin işi).
--
-- Sınır (dürüst): bağ bugün **hiçbir ekranda çizilmiyor** (arayüz yalnız
-- `superseded` durum etiketini yazar); kapı bugünkü görünür davranışı
-- değiştirmez, **duruktaki** dosya sınırını korur ve ileride "yerine geçen"
-- bağını gösteren/navigate eden bir ekranın başka dosyaya sızmamasını sağlar.
-- Reddetme 0031–0040 ile aynı sözleşme: `RAISE(ABORT,'DIM_SCOPE:review_items')`
-- → `StorageError::ScopeViolation` → **`E_DB_SCOPE`**.

DROP TRIGGER IF EXISTS scope_guard_review_items_bi;
DROP TRIGGER IF EXISTS scope_guard_review_items_bu;

-- ---- review_items: kanıt listesi (0033) + karar hedefi (0040) + yerine geçen (0041)
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
  OR (
  NEW.superseded_by_item_id IS NOT NULL AND (
    NEW.superseded_at IS NULL
    OR NEW.superseded_by_item_id = NEW.id
    OR EXISTS (
      SELECT 1 FROM review_items s
       WHERE s.id = NEW.superseded_by_item_id AND s.case_id <> NEW.case_id))
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:review_items');
END;

CREATE TRIGGER scope_guard_review_items_bu
BEFORE UPDATE OF case_id, source_anchor_ids, target_entity_id, superseded_by_item_id, superseded_at
ON review_items
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
  OR (
  NEW.superseded_by_item_id IS NOT NULL AND (
    NEW.superseded_at IS NULL
    OR NEW.superseded_by_item_id = NEW.id
    OR EXISTS (
      SELECT 1 FROM review_items s
       WHERE s.id = NEW.superseded_by_item_id AND s.case_id <> NEW.case_id))
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:review_items');
END;
