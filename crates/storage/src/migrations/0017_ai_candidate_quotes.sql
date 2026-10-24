-- F11-T05: aday şema doğrulaması ve alıntı eşleştirmesi.
--
-- Doğrulama durumu artık çağıranın beyanı değil, hesaplanan sonuçtur.
-- `schema_state`: valid | partial | invalid
-- `quote_state`:  verified | partial | failed | no_quotes | not_checked
--                 | legacy (bu sürümden önce yazılmış aday)
-- `quote_report`: alıntı başına eşleşme raporu (JSON dizi).
--
-- Eski satırlar `legacy` olarak işaretlenir: doğrulanmış gibi görünmezler.
ALTER TABLE ai_candidates ADD COLUMN schema_state TEXT NOT NULL DEFAULT 'legacy';
ALTER TABLE ai_candidates ADD COLUMN quote_state TEXT NOT NULL DEFAULT 'legacy';
ALTER TABLE ai_candidates ADD COLUMN quote_report TEXT NOT NULL DEFAULT '[]';
ALTER TABLE ai_candidates ADD COLUMN checked_at TEXT;
