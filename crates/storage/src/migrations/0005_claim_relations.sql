-- 0005: iddia–iddia ilişkisi (F08-T05). Taraf yanıtı bir belgeye değil,
-- yanıtlanan iddiaya bağlanır; ilişki türü kullanıcı tarafından açıkça seçilir.
-- Kendine bağ kurulamaz; aynı (iddia, ilgili iddia, ilişki) üçlüsü tekrar
-- eklenemez.
CREATE TABLE claim_relations (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  claim_id TEXT NOT NULL REFERENCES claims(id) ON DELETE CASCADE,
  related_claim_id TEXT NOT NULL REFERENCES claims(id) ON DELETE CASCADE,
  relation TEXT NOT NULL CHECK (relation IN ('response_to', 'supports', 'contradicts', 'context')),
  rationale TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (claim_id <> related_claim_id)
);

CREATE UNIQUE INDEX idx_claim_relations_unique
  ON claim_relations(claim_id, related_claim_id, relation);
CREATE INDEX idx_claim_relations_case ON claim_relations(case_id);
CREATE INDEX idx_claim_relations_related ON claim_relations(related_claim_id);
