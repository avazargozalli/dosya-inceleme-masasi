-- 0009: lisanslı kontrol listeleri (F08-T11).
--
-- Amaç: bir kontrol listesi dosyaya kopyalandığında şablonun sürümü, yazarı,
-- lisansı ve içerik hash'i kaybolmasın. Bu yüzden şablon **içeriği** de
-- veritabanında tutulur: yıllar sonra dosyadaki listenin hangi sürümden
-- geldiği, uygulamanın o günkü sürümünden bağımsız olarak kanıtlanabilir.
--
-- `checklist_templates` tablosu 0001'de vardı; kaynak ve lisans URL'si ile
-- içerik tablosu burada eklenir. `case_checklist_items` kopyalama anındaki
-- sıra ve kategoriyi taşır: şablon sonradan silinse bile dosyadaki liste
-- okunabilir kalır.
ALTER TABLE checklist_templates ADD COLUMN source_url TEXT;
ALTER TABLE checklist_templates ADD COLUMN license_url TEXT;

CREATE TABLE checklist_template_items (
  id TEXT PRIMARY KEY,
  template_id TEXT NOT NULL REFERENCES checklist_templates(id) ON DELETE CASCADE,
  ordinal INTEGER NOT NULL,
  category TEXT NOT NULL DEFAULT '',
  item_text TEXT NOT NULL,
  required INTEGER NOT NULL DEFAULT 0 CHECK (required IN (0, 1)),
  UNIQUE (template_id, ordinal)
);

CREATE INDEX idx_checklist_template_items_template
  ON checklist_template_items(template_id, ordinal);

-- Kopyalama anındaki sıra/kategori; `template_version_id` silinse de kalır.
ALTER TABLE case_checklist_items ADD COLUMN ordinal INTEGER NOT NULL DEFAULT 0;
ALTER TABLE case_checklist_items ADD COLUMN category TEXT NOT NULL DEFAULT '';

-- Durum değişikliği iyimser eşzamanlılıkla yazılır: iki oturum aynı öğeyi
-- farklı durumlara çekerse ikincisi sessizce kazanmaz.
ALTER TABLE case_checklist_items ADD COLUMN row_version INTEGER NOT NULL DEFAULT 1;

CREATE INDEX idx_case_checklist_items_case
  ON case_checklist_items(case_id, template_version_id, ordinal);
