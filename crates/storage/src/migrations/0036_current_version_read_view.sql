-- 0036: "güncel sürüm" **okunurken** kendi belgesine ait olmalı (INV-05).
--
-- Neden: v35 (`0035_document_current_version_guard.sql`) yalnız **yeni**
-- yazmayı kapıladı. O turun dürüst sınırı şuydu: "v35 öncesi çapraz
-- işaretçiler sayılır, düzeltilmez — veri temizliğine kadar yanlış okuma
-- riski sürer; okuma sorgularına `dv.document_id=d.id` eklenmedi."
--
-- Ölçüldü (v35 kasası, tetikleyiciler bilinçli düşürülüp eski satır taklit
-- edilerek; geçici sonda sonra silindi): `d1.current_version_id='v9'` (başka
-- dosyanın sürümü) iken okuma yolları **kendi belgesine bakmıyordu**:
--
--   * `coverage_report('c1')` yabancı sürümün ayrıştırma koşusunu/sayfalarını
--     bu dosyanın kapsamına sayıyordu (1 sayfa / 1 segment);
--   * `check_in_text_quotes` yabancı metinde alıntıyı `exact` buluyordu —
--     alıntı ekrandaki belgede geçmediği hâlde "bulundu" (INV-05);
--   * `consent_preview_masked` / `resolve_transfer_scope` yabancı metni
--     gönderilecek gövdeye koyuyordu (onaylanan içerik ≠ gönderilen içerik);
--   * `documents_pending_parse` belgeyi yabancı sürüm kimliğiyle listeliyor,
--     `get_document`/`list_documents` yabancı sürümü "güncel" diye döndürüyor,
--     plan/anchor okumaları yabancı sürüm numarasını yazıyordu.
--
-- Kural: bir belgenin güncel sürümü **yalnız** o belgeye ait bir sürüm
-- olabilir. İşaretçi çaprazsa (ya da sürümü silinmişse) belgenin güncel
-- sürümü **yoktur** — tahmin edilmez, başka sürüme düşülmez (fail-closed).
-- Böylece eski bozuk satır "yanlış veri" değil "veri yok" üretir; bozukluk
-- `Database::scope_violations()` tarafından sayılmaya devam eder.
--
-- Tek tanım: bütün okuma yolları bu görünümden geçer. Kopya `JOIN`/alt sorgu
-- yazılmaz — kopyalar kayar (0035 öncesi durum tam olarak buydu).
--
-- `deleted_at` süzgeci **burada değil**: görünüm yalnız işaretçinin
-- geçerliliğini söyler; silme politikası çağıranın süzgecinde kalır (mevcut
-- davranış değişmesin).

CREATE VIEW current_document_versions AS
SELECT d.id         AS document_id,
       d.case_id    AS case_id,
       v.id         AS version_id,
       v.version_no AS version_no,
       v.byte_size  AS byte_size
  FROM documents d
  JOIN document_versions v
    ON v.id = d.current_version_id
   AND v.document_id = d.id;
