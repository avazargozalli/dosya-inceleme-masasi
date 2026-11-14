-- 0031: dosya kapsamı **veritabanı düzeyinde** zorlanır (INV-05, P0 sınıfı).
--
-- Neden: 2026-11-14'te çekirdek yazma yolları (`add_version_with_parent`,
-- `start_parse_run`, `store_pages`) kapsamı doğrulamaya başladı; ama savunma
-- yalnız **çekirdek sorgusundaydı**. Doğrudan SQL yazan bir yol (yeni bir komut,
-- bir betik, ileride eklenecek bir toplu içe aktarma) `case_id` sütununa başka
-- dosyanın kimliğini yazabiliyordu; sonuç: A dosyasının metni B dosyasının
-- aramasında bulunur, kapsam raporu yanlış dosyada satır sayar. Bu P0 kabul
-- engelidir.
--
-- Kural: bir satırın `case_id`'si, **üst kayıtlarının** dosyasıyla birebir aynı
-- olmalı. Üst kayıt yoksa satır yazılmaz (fail-closed). Zincir:
--
--   documents (root)              → document_versions.document_id
--   document_versions             → parse_runs.document_version_id
--   parse_runs                    → text_segments.parse_run_id
--   parse_runs + text_segments    → source_anchors.parse_run_id / segment_id
--   document_versions (+run/anchor) → source_marks
--   document_versions (+run)      → redaction_plans
--   documents                     → document_tags
--   text_segments                 → embeddings
--
-- Sayfa üyeliği de denetlenir: `text_segments.page_id` kendi koşusunun sayfası
-- olmalı; sayfa başka koşunun ise segment yanlış belgeye konumlanır (alıntı
-- geometrisi yanlış belgeyi gösterir).
--
-- Reddetme `RAISE(ABORT, 'DIM_SCOPE:<tablo>')` ile yapılır; `StorageError`
-- bu öneki `E_DB_SCOPE` koduna çevirir (mesaj metni başka bir SQLite hatasıyla
-- karışmasın diye sabit önek). Hata **işlem içinde** oluşur: kısmi satır kalmaz.
--
-- Sınır (dürüst): tetikleyiciler **yeni** yazmaları engeller; bu şema
-- sürümünden önce yazılmış aykırı satırlar silinmez/değiştirilmez. Onları
-- `Database::scope_violations()` sayar (kimlik taşımaz, yalnız sayı).
--
-- Maliyet: her satır için birincil anahtar araması (indexed). Toplu sayfa
-- yazımında ölçüldü; ölçüm `crates/storage/tests/scope_guard_cost.rs`.

CREATE TRIGGER scope_guard_document_versions_bi
BEFORE INSERT ON document_versions
WHEN NOT EXISTS (
  SELECT 1 FROM documents d
   WHERE d.id = NEW.document_id AND d.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:document_versions');
END;

CREATE TRIGGER scope_guard_document_versions_bu
BEFORE UPDATE OF document_id, case_id ON document_versions
WHEN NOT EXISTS (
  SELECT 1 FROM documents d
   WHERE d.id = NEW.document_id AND d.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:document_versions');
END;

CREATE TRIGGER scope_guard_parse_runs_bi
BEFORE INSERT ON parse_runs
WHEN NOT EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.document_version_id AND dv.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:parse_runs');
END;

CREATE TRIGGER scope_guard_parse_runs_bu
BEFORE UPDATE OF document_version_id, case_id ON parse_runs
WHEN NOT EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.document_version_id AND dv.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:parse_runs');
END;

CREATE TRIGGER scope_guard_text_segments_bi
BEFORE INSERT ON text_segments
WHEN NOT EXISTS (
  SELECT 1 FROM parse_runs pr
   WHERE pr.id = NEW.parse_run_id AND pr.case_id = NEW.case_id
)
OR (
  NEW.page_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM pages p
     WHERE p.id = NEW.page_id AND p.parse_run_id = NEW.parse_run_id
  )
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:text_segments');
END;

CREATE TRIGGER scope_guard_text_segments_bu
BEFORE UPDATE OF parse_run_id, page_id, case_id ON text_segments
WHEN NOT EXISTS (
  SELECT 1 FROM parse_runs pr
   WHERE pr.id = NEW.parse_run_id AND pr.case_id = NEW.case_id
)
OR (
  NEW.page_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM pages p
     WHERE p.id = NEW.page_id AND p.parse_run_id = NEW.parse_run_id
  )
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:text_segments');
END;

CREATE TRIGGER scope_guard_source_anchors_bi
BEFORE INSERT ON source_anchors
WHEN NOT EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.document_version_id AND dv.case_id = NEW.case_id
)
OR NOT EXISTS (
  SELECT 1 FROM parse_runs pr
   WHERE pr.id = NEW.parse_run_id AND pr.case_id = NEW.case_id
)
OR NOT EXISTS (
  SELECT 1 FROM text_segments s
   WHERE s.id = NEW.segment_id
     AND s.parse_run_id = NEW.parse_run_id
     AND s.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:source_anchors');
END;

CREATE TRIGGER scope_guard_source_anchors_bu
BEFORE UPDATE OF document_version_id, parse_run_id, segment_id, case_id ON source_anchors
WHEN NOT EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.document_version_id AND dv.case_id = NEW.case_id
)
OR NOT EXISTS (
  SELECT 1 FROM parse_runs pr
   WHERE pr.id = NEW.parse_run_id AND pr.case_id = NEW.case_id
)
OR NOT EXISTS (
  SELECT 1 FROM text_segments s
   WHERE s.id = NEW.segment_id
     AND s.parse_run_id = NEW.parse_run_id
     AND s.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:source_anchors');
END;

CREATE TRIGGER scope_guard_source_marks_bi
BEFORE INSERT ON source_marks
WHEN NOT EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.document_version_id AND dv.case_id = NEW.case_id
)
OR (
  NEW.parse_run_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM parse_runs pr
     WHERE pr.id = NEW.parse_run_id AND pr.case_id = NEW.case_id
  )
)
OR (
  NEW.anchor_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM source_anchors a
     WHERE a.id = NEW.anchor_id AND a.case_id = NEW.case_id
  )
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:source_marks');
END;

CREATE TRIGGER scope_guard_source_marks_bu
BEFORE UPDATE OF document_version_id, parse_run_id, anchor_id, case_id ON source_marks
WHEN NOT EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.document_version_id AND dv.case_id = NEW.case_id
)
OR (
  NEW.parse_run_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM parse_runs pr
     WHERE pr.id = NEW.parse_run_id AND pr.case_id = NEW.case_id
  )
)
OR (
  NEW.anchor_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM source_anchors a
     WHERE a.id = NEW.anchor_id AND a.case_id = NEW.case_id
  )
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:source_marks');
END;

CREATE TRIGGER scope_guard_redaction_plans_bi
BEFORE INSERT ON redaction_plans
WHEN NOT EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.document_version_id AND dv.case_id = NEW.case_id
)
OR (
  NEW.parse_run_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM parse_runs pr
     WHERE pr.id = NEW.parse_run_id AND pr.case_id = NEW.case_id
  )
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:redaction_plans');
END;

CREATE TRIGGER scope_guard_redaction_plans_bu
BEFORE UPDATE OF document_version_id, parse_run_id, case_id ON redaction_plans
WHEN NOT EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.document_version_id AND dv.case_id = NEW.case_id
)
OR (
  NEW.parse_run_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM parse_runs pr
     WHERE pr.id = NEW.parse_run_id AND pr.case_id = NEW.case_id
  )
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:redaction_plans');
END;

CREATE TRIGGER scope_guard_document_tags_bi
BEFORE INSERT ON document_tags
WHEN NOT EXISTS (
  SELECT 1 FROM documents d
   WHERE d.id = NEW.document_id AND d.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:document_tags');
END;

CREATE TRIGGER scope_guard_document_tags_bu
BEFORE UPDATE OF document_id, case_id ON document_tags
WHEN NOT EXISTS (
  SELECT 1 FROM documents d
   WHERE d.id = NEW.document_id AND d.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:document_tags');
END;

CREATE TRIGGER scope_guard_embeddings_bi
BEFORE INSERT ON embeddings
WHEN NOT EXISTS (
  SELECT 1 FROM text_segments s
   WHERE s.id = NEW.segment_id AND s.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:embeddings');
END;

CREATE TRIGGER scope_guard_embeddings_bu
BEFORE UPDATE OF segment_id, case_id ON embeddings
WHEN NOT EXISTS (
  SELECT 1 FROM text_segments s
   WHERE s.id = NEW.segment_id AND s.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:embeddings');
END;
