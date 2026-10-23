-- 0011: not taslağı, sürüm geçmişi ve geri/ileri al (F07-T10).
--
-- Not düzenlemesi iyimser eşzamanlılıkla korunur (`notes.row_version`).
-- Geri/ileri al yığınları kasa içinde tutulur: frontend localStorage'a yazılmaz
-- ve uygulama yeniden açıldığında aynı geçmiş kalır. Her yazma ayrıca kalıcı
-- bir revizyon bırakır; böylece "geri al" gerçekten saklanmış bir sürümü
-- geri getirir, bellekteki metni uydurmaz.
ALTER TABLE notes ADD COLUMN undo_stack TEXT NOT NULL DEFAULT '[]';
ALTER TABLE notes ADD COLUMN redo_stack TEXT NOT NULL DEFAULT '[]';

CREATE TABLE note_revisions (
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  revision_no INTEGER NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  body_ast TEXT NOT NULL,
  source_anchor_ids TEXT NOT NULL DEFAULT '[]',
  origin TEXT NOT NULL CHECK (origin IN ('create', 'edit', 'undo', 'redo')),
  created_at TEXT NOT NULL,
  PRIMARY KEY (note_id, revision_no)
);

-- Var olan notlar için 1. revizyon geriye dönük doldurulur; aksi hâlde eski
-- notlarda geçmiş boş görünür ve geri al hiçbir şey yapamaz.
INSERT INTO note_revisions (note_id, revision_no, title, body_ast, source_anchor_ids, origin, created_at)
SELECT id, 1, title, body_ast, source_anchor_ids, 'create', created_at FROM notes;

-- Taslak: kasa açıkken otomatik kaydedilir. `note_id = ''` yeni not taslağıdır
-- (SQLite'ta NULL tekil sayılmadığı için boş metin göndericisi kullanılır).
CREATE TABLE note_drafts (
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  note_id TEXT NOT NULL DEFAULT '',
  title TEXT NOT NULL DEFAULT '',
  body TEXT NOT NULL DEFAULT '',
  source_anchor_ids TEXT NOT NULL DEFAULT '[]',
  base_row_version INTEGER,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (case_id, note_id)
);
