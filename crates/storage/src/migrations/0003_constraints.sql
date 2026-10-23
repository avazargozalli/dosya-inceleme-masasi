-- 0003: aynı delil bağının tekrarını engelle. Aynı iddia–anchor–ilişki
-- üçlüsü ikinci kez eklenemez; farklı ilişki (ör. supports + contradicts)
-- ayrı kayıttır ve bilinçli olarak korunur.
CREATE UNIQUE INDEX idx_evidence_unique
  ON evidence_links(claim_id, source_anchor_id, relation);
