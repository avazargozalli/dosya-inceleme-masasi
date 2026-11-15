-- 0035: belgenin **güncel sürüm işaretçisi** kendi belgesini gösterir
-- (INV-05, P0 sınıfı).
--
-- Neden: v31–v34 satırların `case_id`'sini, kanıt referanslarını ve JSON dizi
-- referanslarını kapıladı; ama `documents.current_version_id` **kendi belgesine**
-- bağlı değildi ve şemada FK da yoktu. Tek bir doğrudan
-- `UPDATE documents SET current_version_id=…` belgeyi başka bir belgenin
-- (başka dosyada olabilir) sürümüne çevirebiliyordu. Her okuma yolu bu
-- işaretçiden geçer:
--
--   * `coverage_report` “güncel sürüm”ün ayrıştırma koşusunu ve sayfalarını
--     sayar → yanlış işaretçide A dosyasının kapsamına B dosyasının
--     sayfa/segment sayıları girer;
--   * `check_in_text_quotes` alıntıyı o sürümün koşusunun sayfalarında arar →
--     A dosyasının belgesinde B dosyasının metni “bulunur” (INV-05: delil
--     ekrandaki belgeye işaret etmez);
--   * AI kapsamı gövdeyi o sürümden toplar → B dosyasının metni A dosyasının
--     onayıyla cihazdan çıkabilir.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite, v34; geçici sonda sonra silindi):
-- `UPDATE documents SET current_version_id='pv9' WHERE id='pd1'` **kabul
-- edildi** (rows=1); `coverage_report('p1c')` B dosyasının iki sayfasını /
-- iki segmentini A dosyasında saydı (2/2); A dosyasının belgesinde
-- “gizli metin” alıntısı önce `not_found` iken sonra **`exact`** döndü
-- (pages_scanned 1 → 2).
--
-- Kural: `current_version_id` NULL olabilir; doluysa **var olan** ve **aynı
-- belgeye** ait bir sürümü göstermeli. Eski/bozuk satır (sürümü silinmiş,
-- işaretçi boşta kalmış) başka alanların güncellenmesini **engellemez** —
-- yalnız işaretçiyi **değiştiren** yazma denetlenir; aksi hâlde eski bir kasada
-- belge yeniden adlandırılamaz hâle gelirdi.
--
-- Aynı değişmez **ters yönden** de korunur: bir belgenin güncel sürümünü
-- gösteren sürüm satırı başka belgeye taşınamaz (`document_versions.document_id`
-- UPDATE). Aksi hâlde kapı, sürüm satırı taşınarak atlatılırdı.
--
-- Reddetme `RAISE(ABORT, 'DIM_SCOPE:documents')` → `StorageError::ScopeViolation`
-- → **`E_DB_SCOPE`** (mesaj metni başka SQLite hatasıyla karışmaz).

CREATE TRIGGER scope_guard_documents_bi
BEFORE INSERT ON documents
WHEN NEW.current_version_id IS NOT NULL
 AND NOT EXISTS (
  SELECT 1 FROM document_versions v
   WHERE v.id = NEW.current_version_id AND v.document_id = NEW.id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:documents');
END;

CREATE TRIGGER scope_guard_documents_bu
BEFORE UPDATE OF current_version_id ON documents
WHEN NEW.current_version_id IS NOT OLD.current_version_id
 AND NEW.current_version_id IS NOT NULL
 AND NOT EXISTS (
  SELECT 1 FROM document_versions v
   WHERE v.id = NEW.current_version_id AND v.document_id = NEW.id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:documents');
END;

-- Ters yön: güncel sürüm işaretçisi olan satır başka belgeye taşınamaz.
-- (Taşımak isteyen önce işaretçiyi NULL'a çeker; sessiz taşıma yok.)
CREATE TRIGGER scope_guard_document_versions_current_ref_bu
BEFORE UPDATE OF document_id ON document_versions
WHEN NEW.document_id IS NOT OLD.document_id
 AND EXISTS (
  SELECT 1 FROM documents d
   WHERE d.current_version_id = OLD.id AND d.id IS NOT NEW.document_id
)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:documents');
END;
