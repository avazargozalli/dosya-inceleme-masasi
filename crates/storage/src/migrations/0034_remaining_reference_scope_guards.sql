-- 0034: kalan **referans** sütunları da dosya sınırında (INV-05, P0 sınıfı).
--
-- Neden: 0032 tek sütunlu kanıt referanslarını, 0033 JSON dizi kanıt listelerini
-- kapıladı. Şema taraması (0032/0033 testlerinin türettiği kural) üç sütunu
-- **hiç görmüyordu**, çünkü hiçbiri `case_id` + bilinen kapsam sütunu kalıbına
-- girmiyor ya da tablo kendi `case_id`'sini taşımıyordu:
--
--   document_versions.parent_version_id  → document_versions.case_id (soy ağacı)
--   redaction_plans.output_version_id    → document_versions.case_id (maskeleme çıktısı)
--   ai_context_items.document_version_id → ai_runs.case_id (gönderim sicili)
--   ai_context_items.source_anchor_id    → ai_runs.case_id
--   ai_context_items.segment_ids (JSON)  → satırın sürümünün segmentleri
--   transfer_segments.document_version_id → ai_runs.case_id (giden segment sicili)
--   source_anchors.page_id            → pages.parse_run_id (sayfa üyeliği)
--   source_marks.page_id              → pages.parse_run_id → case_id (dosya sınırı)
--   review_items.ai_run_id            → ai_runs.case_id (aday kökeni)
--   note_drafts.note_id               → notes.case_id (taslağın bağlandığı not)
--
-- Sonuç (ölçüldü, düzeltme öncesi, gerçek SQLite): üç yol da kapıyı atlıyordu.
--   (1) `record_context_items` **çekirdek** kapısı yoktu: A dosyasının AI
--       çalıştırmasına B dosyasının sürümü + çapası yazıldı (`Ok(1)`) ve
--       `list_context_items` A dosyasının ekranında **B'nin belge adını**
--       gösterdi ("GIZLI diger dosya belgesi"). Gönderim sicili, cihazdan çıkan
--       içeriğin kaydıdır; yanlış dosyayı göstermesi P0 sınıfıdır.
--   (2) `redaction_plans.output_version_id` çekirdekte doğrulanıyor
--       (`create_redaction_plan`: aynı dosya + alt sürüm) ama **doğrudan SQL**
--       yazan yol kapıyı atlıyordu: A dosyasının planı B dosyasının sürümünü
--       çıktı gösterdi ve sicil (`list_redaction_plans`) B'nin belge adını yazdı.
--   (3) `document_versions.parent_version_id` çekirdekte doğrulanıyor
--       (`add_version_with_parent`) ama şema düzeyinde kapı yoktu: soy ağacı
--       başka dosyaya bağlanabiliyordu (maskeleme izi yanlış dosyayı gösterir).
--
-- Kural 0031/0032/0033 ile aynı: referansın hedefi **kendi dosyasında** olmalı;
-- yoksa satır yazılmaz (fail-closed). Reddetme `RAISE(ABORT, 'DIM_SCOPE:<tablo>')`
-- → `StorageError::ScopeViolation` → **`E_DB_SCOPE`**.
--
-- Kasıtlı ayrım — **AI gönderim sicili** (`ai_context_items`) 0033'ün taslak
-- (`note_drafts`) kuralıyla aynı gevşeklikte: sürüm/çapa **başka dosyada
-- varsa** reddedilir; ama var **olmayan** kimlik kalabilir. Neden: `ai_context_items`
-- denetim izidir — belge gönderimden sonra silinirse satır **yaşamalıdır**
-- (`context_item_without_document_keeps_the_row_without_inventing_a_name`
-- sözleşmesi; `document_version_id` bu tabloda yabancı anahtar değildir).
-- Segment listesi ise **katıdır**: her eleman satırın kendi sürümünün segmenti
-- olmalı (çekirdek `record_context_items` bunu zaten doğruluyor; uydurma kimlik
-- sicile yazılmaz).
--
-- Kapsam dışı (dürüst): `notes.undo_stack`/`redo_stack` anlık görüntüdür
-- (referans değil); `redaction_plans.output_version_ref` bir **iz** metnidir
-- (FK değil, çözümlenmez); `payload`/`usage_report` içindeki serbest JSON
-- yorumlanmaz; `redaction_regions`, `pages`, `transfer_segments` gibi tablolar
-- kendi `case_id`'sini taşımaz ve kapsamı üst kaydından miras alır (üst kayıt
-- zaten kapılı).
--
-- Sınır (dürüst): tetikleyiciler **yeni** yazmaları engeller; bu sürümden önce
-- yazılmış çapraz bağlar silinmez/değiştirilmez. `Database::scope_violations()`
-- onları sayar (kimlik/içerik taşımaz).
--
-- Maliyet: satır başına bir indeksli arama (ya da segment listesinde eleman
-- başına bir arama). Bu üç tabloya yazan hacim düşüktür (plan = kullanıcı
-- maskeleme eylemi başına 1 satır, bağlam = çalıştırma başına belge sayısı
-- kadar); toplu yol yok.

-- ---- source_anchors: çapanın sayfası kendi koşusunun sayfası ------------------
-- `source_anchors.page_id` 0031'de kapı dışında kaldı (tetikleyici yalnız sürüm,
-- koşu ve segmenti denetliyordu). Sayfa başka koşunun ise çapa yanlış belgenin
-- geometrisini gösterir — `text_segments.page_id` için 0031'de konan kuralın
-- ikizi. NULL (sayfasız çapa) kapıya takılmaz.
CREATE TRIGGER scope_guard_source_anchors_page_bi
BEFORE INSERT ON source_anchors
WHEN NEW.page_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM pages p
   WHERE p.id = NEW.page_id AND p.parse_run_id = NEW.parse_run_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:source_anchors');
END;

CREATE TRIGGER scope_guard_source_anchors_page_bu
BEFORE UPDATE OF page_id, parse_run_id ON source_anchors
WHEN NEW.page_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM pages p
   WHERE p.id = NEW.page_id AND p.parse_run_id = NEW.parse_run_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:source_anchors');
END;

-- ---- source_marks: işaretin sayfası kendi dosyasından ------------------------
-- `source_marks.page_id` 0031'de kapı dışında kaldı. Kural burada **dosya
-- sınırı**dır (sayfanın koşusunun dosyası = işaretin dosyası); sayfa-koşu
-- üyeliği geometri politikası olarak çekirdekte kalır. NULL (sayfasız işaret)
-- kapıya takılmaz.
CREATE TRIGGER scope_guard_source_marks_page_bi
BEFORE INSERT ON source_marks
WHEN NEW.page_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM pages p
    JOIN parse_runs pr ON pr.id = p.parse_run_id
   WHERE p.id = NEW.page_id AND pr.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:source_marks');
END;

CREATE TRIGGER scope_guard_source_marks_page_bu
BEFORE UPDATE OF page_id, case_id ON source_marks
WHEN NEW.page_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM pages p
    JOIN parse_runs pr ON pr.id = p.parse_run_id
   WHERE p.id = NEW.page_id AND pr.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:source_marks');
END;

-- ---- review_items: adayın kaynağı çalıştırma kendi dosyasından ----------------
-- `review_items.ai_run_id` (yabancı anahtar değil) hangi çalıştırmanın ürettiğini
-- söyler; başka dosyanın çalıştırmasını göstermesi kökeni yanlış dosyaya yazar.
-- Var olmayan çalıştırma kimliği eski satırlarda kalabilir (kapı yalnız **başka
-- dosyada var olan** kimliği reddeder).
CREATE TRIGGER scope_guard_review_items_run_bi
BEFORE INSERT ON review_items
WHEN NEW.ai_run_id IS NOT NULL AND EXISTS (
  SELECT 1 FROM ai_runs r
   WHERE r.id = NEW.ai_run_id AND r.case_id <> NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:review_items');
END;

CREATE TRIGGER scope_guard_review_items_run_bu
BEFORE UPDATE OF ai_run_id, case_id ON review_items
WHEN NEW.ai_run_id IS NOT NULL AND EXISTS (
  SELECT 1 FROM ai_runs r
   WHERE r.id = NEW.ai_run_id AND r.case_id <> NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:review_items');
END;

-- ---- note_drafts: taslağın bağlandığı not kendi dosyasından -------------------
-- `note_drafts.note_id` bir **anahtar parçasıdır** (`case_id, note_id`), yabancı
-- anahtar değildir; boş dize yeni taslaktır. Başka dosyanın notuna bağlanmak
-- taslağı yanlış dosyaya yazar (okuma yolu çifti birlikte arar). Var olmayan
-- kimlik taslakta meşru kalır (0033 taslak sözleşmesi).
CREATE TRIGGER scope_guard_note_drafts_note_bi
BEFORE INSERT ON note_drafts
WHEN NEW.note_id <> '' AND EXISTS (
  SELECT 1 FROM notes n WHERE n.id = NEW.note_id AND n.case_id <> NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:note_drafts');
END;

CREATE TRIGGER scope_guard_note_drafts_note_bu
BEFORE UPDATE OF note_id, case_id ON note_drafts
WHEN NEW.note_id <> '' AND EXISTS (
  SELECT 1 FROM notes n WHERE n.id = NEW.note_id AND n.case_id <> NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:note_drafts');
END;

-- ---- document_versions: soy ağacı dosya sınırını aşmaz ----------------------
CREATE TRIGGER scope_guard_document_versions_parent_bi
BEFORE INSERT ON document_versions
WHEN NEW.parent_version_id IS NOT NULL
 AND NOT EXISTS (
  SELECT 1 FROM document_versions p
   WHERE p.id = NEW.parent_version_id AND p.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:document_versions');
END;

CREATE TRIGGER scope_guard_document_versions_parent_bu
BEFORE UPDATE OF parent_version_id, case_id ON document_versions
WHEN NEW.parent_version_id IS NOT NULL
 AND NOT EXISTS (
  SELECT 1 FROM document_versions p
   WHERE p.id = NEW.parent_version_id AND p.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:document_versions');
END;

-- ---- redaction_plans: çıktı sürümü kendi dosyasından -----------------------
CREATE TRIGGER scope_guard_redaction_plans_output_bi
BEFORE INSERT ON redaction_plans
WHEN NEW.output_version_id IS NOT NULL
 AND NOT EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.output_version_id AND dv.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:redaction_plans');
END;

CREATE TRIGGER scope_guard_redaction_plans_output_bu
BEFORE UPDATE OF output_version_id, case_id ON redaction_plans
WHEN NEW.output_version_id IS NOT NULL
 AND NOT EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.output_version_id AND dv.case_id = NEW.case_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:redaction_plans');
END;

-- ---- ai_context_items: gönderim sicili kendi dosyasından -------------------
-- Tablonun `case_id`'si yoktur: dosya, üst kaydın (`ai_runs.case_id`) dosyasıdır.
CREATE TRIGGER scope_guard_ai_context_items_bi
BEFORE INSERT ON ai_context_items
WHEN EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.document_version_id
     AND dv.case_id <> (SELECT r.case_id FROM ai_runs r WHERE r.id = NEW.ai_run_id)
)
OR (
  NEW.source_anchor_id IS NOT NULL AND EXISTS (
    SELECT 1 FROM source_anchors a
     WHERE a.id = NEW.source_anchor_id
       AND a.case_id <> (SELECT r.case_id FROM ai_runs r WHERE r.id = NEW.ai_run_id))
)
OR NOT json_valid(NEW.segment_ids)
OR EXISTS (
  SELECT 1 FROM json_each(
      CASE WHEN json_valid(NEW.segment_ids) THEN NEW.segment_ids ELSE '[]' END) j
   WHERE j.type <> 'text'
      OR NOT EXISTS (
        SELECT 1 FROM text_segments s
          JOIN parse_runs pr ON pr.id = s.parse_run_id
         WHERE s.id = j.value AND pr.document_version_id = NEW.document_version_id)
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:ai_context_items');
END;

CREATE TRIGGER scope_guard_ai_context_items_bu
BEFORE UPDATE OF ai_run_id, document_version_id, source_anchor_id, segment_ids ON ai_context_items
WHEN EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.document_version_id
     AND dv.case_id <> (SELECT r.case_id FROM ai_runs r WHERE r.id = NEW.ai_run_id)
)
OR (
  NEW.source_anchor_id IS NOT NULL AND EXISTS (
    SELECT 1 FROM source_anchors a
     WHERE a.id = NEW.source_anchor_id
       AND a.case_id <> (SELECT r.case_id FROM ai_runs r WHERE r.id = NEW.ai_run_id))
)
OR NOT json_valid(NEW.segment_ids)
OR EXISTS (
  SELECT 1 FROM json_each(
      CASE WHEN json_valid(NEW.segment_ids) THEN NEW.segment_ids ELSE '[]' END) j
   WHERE j.type <> 'text'
      OR NOT EXISTS (
        SELECT 1 FROM text_segments s
          JOIN parse_runs pr ON pr.id = s.parse_run_id
         WHERE s.id = j.value AND pr.document_version_id = NEW.document_version_id)
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:ai_context_items');
END;

-- ---- transfer_segments: giden segment sicili kendi dosyasından --------------
-- Giden metnin kendisi bu tabloda durur (maskelenmiş hâli). Sürüm başka
-- dosyadan ise A dosyasının gönderim sicili B dosyasının belgesini gösterir.
-- `document_version_id` 0026'da eklendi ve NULL olabilir (eski satırlar);
-- NULL kapıya takılmaz.
CREATE TRIGGER scope_guard_transfer_segments_bi
BEFORE INSERT ON transfer_segments
WHEN NEW.document_version_id IS NOT NULL AND EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.document_version_id
     AND dv.case_id <> (SELECT r.case_id FROM ai_runs r WHERE r.id = NEW.ai_run_id)
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:transfer_segments');
END;

CREATE TRIGGER scope_guard_transfer_segments_bu
BEFORE UPDATE OF ai_run_id, document_version_id ON transfer_segments
WHEN NEW.document_version_id IS NOT NULL AND EXISTS (
  SELECT 1 FROM document_versions dv
   WHERE dv.id = NEW.document_version_id
     AND dv.case_id <> (SELECT r.case_id FROM ai_runs r WHERE r.id = NEW.ai_run_id)
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:transfer_segments');
END;
