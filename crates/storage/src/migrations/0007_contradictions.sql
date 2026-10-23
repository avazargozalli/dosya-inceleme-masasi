-- 0007: çelişki değerlendirmesi (F08-T06). İki kaynak yan yana gösterilir ve
-- kullanıcı kendi değerlendirmesini kaydeder. Değerlendirme, inceleme kararından
-- ayrıdır: kullanıcı önce farkı değerlendirir, sonra kabul/ret kararını verir.
-- Kayıtlar eklemelidir; aynı öğe için birden çok değerlendirme tutulabilir ve
-- en yenisi gösterilir.
CREATE TABLE contradiction_assessments (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  review_item_id TEXT NOT NULL REFERENCES review_items(id) ON DELETE CASCADE,
  verdict TEXT NOT NULL CHECK (verdict IN ('conflict', 'explained', 'not_conflict', 'needs_more_evidence')),
  rationale TEXT NOT NULL DEFAULT '',
  reviewer_label TEXT NOT NULL,
  assessed_at TEXT NOT NULL
);

CREATE INDEX idx_contradiction_assessments_item
  ON contradiction_assessments(review_item_id, assessed_at);
