-- Belge etiketleri (F07-T03): arama filtresi ve belge listesi.
--
-- Etiket belgeye bağlıdır; `case_id` kopyası, etiket filtresinin FTS sorgusunda
-- dosya kapsamıyla **aynı** WHERE içinde uygulanabilmesi içindir. Silinen belge
-- etiketleri de gider (CASCADE), böylece yetim etiket aramaya sızmaz.

CREATE TABLE document_tags (
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  tag TEXT NOT NULL CHECK (length(tag) BETWEEN 1 AND 40),
  created_at TEXT NOT NULL,
  PRIMARY KEY (document_id, tag)
);

CREATE INDEX idx_document_tags_case_tag ON document_tags(case_id, tag);
