-- 0026: gönderim sicili segment düzeyinde (şartname F09/F11).
--
-- Şartname `transfer_segments` tablosunu "gönderilen metin ve özgün aralık
-- eşlemesi şifreli tutulur" diye tanımlar; alıntı doğrulaması da önce
-- **gönderilen** metinde aranır. Tablo şemanın ilk sürümünden beri var ama
-- hiçbir satır yazılmıyordu (`ai_context_items.segment_ids` de hep '[]').
--
-- `document_version_id`: gönderilen segmentin ait olduğu sürüm. Aday
-- doğrulaması "bu sürüm gerçekten gönderildi mi" sorusunu bu sütunla yanıtlar;
-- eski satırlar (yok) NULL kalır ve "gönderilmiş" sayılmaz.
ALTER TABLE transfer_segments ADD COLUMN document_version_id TEXT;

CREATE INDEX idx_transfer_segments_run ON transfer_segments(ai_run_id);
-- Aynı çalıştırmada aynı segment iki kez yazılamaz (sicil mükerrer olmaz).
CREATE UNIQUE INDEX idx_transfer_segments_unique
  ON transfer_segments(ai_run_id, outbound_segment_id);
