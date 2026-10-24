-- F07-T09: metin veya sayfa bölgesi seçiminden oluşturulan **kalıcı** işaret.
--
-- Metin işareti doğrulanmış bir kaynak anchor'ına bağlıdır: alıntı segment
-- metninde birebir durmuyorsa anchor oluşmaz, dolayısıyla işaret de oluşmaz.
-- Bölge işareti sayfanın kendi koordinat uzayında dikdörtgen(ler) taşır;
-- görünüm döndürmesi/yakınlaştırması koordinatı değiştirmez.
CREATE TABLE source_marks (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  document_version_id TEXT NOT NULL REFERENCES document_versions(id) ON DELETE CASCADE,
  parse_run_id TEXT REFERENCES parse_runs(id) ON DELETE CASCADE,
  anchor_id TEXT REFERENCES source_anchors(id) ON DELETE CASCADE,
  page_id TEXT REFERENCES pages(id) ON DELETE SET NULL,
  mark_kind TEXT NOT NULL CHECK (mark_kind IN ('text', 'region')),
  start_offset INTEGER,
  end_offset INTEGER,
  quoted_text TEXT NOT NULL DEFAULT '',
  rects_json TEXT NOT NULL DEFAULT '[]',
  label TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  deleted_at TEXT,
  CHECK (
    mark_kind <> 'text'
    OR (anchor_id IS NOT NULL AND start_offset IS NOT NULL AND end_offset IS NOT NULL
        AND start_offset >= 0 AND end_offset >= start_offset)
  ),
  CHECK (mark_kind <> 'region' OR (page_id IS NOT NULL AND rects_json <> '[]'))
);

CREATE INDEX source_marks_version ON source_marks(document_version_id, created_at);

-- Aynı anchor üzerinde tek **aktif** metin işareti: seçim birden çok kayda
-- bağlanabilir (not + işaret) ama aynı işaret iki kez oluşmaz. Silinen işaret
-- (deleted_at dolu) yeni kayda engel değildir.
CREATE UNIQUE INDEX source_marks_unique_text
  ON source_marks(anchor_id)
  WHERE mark_kind = 'text' AND deleted_at IS NULL;
