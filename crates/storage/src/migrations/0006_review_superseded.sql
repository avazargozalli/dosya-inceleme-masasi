-- 0006: inceleme kuyruğunda `superseded` durumu (F08-T07).
--
-- Tablo yeniden kurulmaz: `review_items.state` CHECK kısıtı tarihsel karar
-- durumunu tutar, "yerine geçildi" bilgisi ayrı kolonlarda saklanır. Böylece
-- `review_decisions` yabancı anahtarı ve karar geçmişi bozulmadan korunur;
-- API katmanı `superseded_at` dolu olan öğeyi `superseded` olarak sunar.
ALTER TABLE review_items ADD COLUMN superseded_by_item_id TEXT
  REFERENCES review_items(id) ON DELETE SET NULL;
ALTER TABLE review_items ADD COLUMN superseded_at TEXT;

CREATE INDEX idx_review_items_superseded ON review_items(case_id, superseded_at);
