-- 0025: kişi/kurum birleştirmesinin izi (şartname F08-T01).
--
-- Şartname birleştirmeyi "inceleme ve geri alma işlemine" bağlar. Geri alma
-- için hangi katılımcı satırlarının taşındığı, kaynağın birleştirme öncesi
-- inceleme durumu ve geri alma zamanı tutulur. `moved_participants` JSON
-- dizisidir: [{"id": <case_participants.id>, "caseId": <case id>}].
CREATE TABLE actor_merges (
  id TEXT PRIMARY KEY,
  source_actor_id TEXT NOT NULL REFERENCES actors(id) ON DELETE RESTRICT,
  target_actor_id TEXT NOT NULL REFERENCES actors(id) ON DELETE RESTRICT,
  source_previous_state TEXT NOT NULL,
  moved_participants TEXT NOT NULL DEFAULT '[]',
  reviewer_label TEXT NOT NULL,
  created_at TEXT NOT NULL,
  undone_at TEXT
);

CREATE INDEX idx_actor_merges_source ON actor_merges(source_actor_id, created_at DESC);
CREATE INDEX idx_case_participants_actor ON case_participants(actor_id);
