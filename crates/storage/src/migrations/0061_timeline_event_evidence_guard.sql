-- 0061: **kronoloji olayı** (`events`) doğuşta kanıtlı olmalı, kökeni ve
-- kimliği sonradan değişmemeli, tarih/kesinlik/sıralama üçlüsü tutarlı kalmalı.
--
-- Neden: `events` satırı «bu olay **şu tarihte** oldu, tarihin kesinliği
-- **şu**, rolü **şu**, kaynağı **şu**» iddiasıdır (şartname §13.3, F08-T03:
-- «kesin, yaklaşık, aralık ve bilinmeyen tarihleri **gün uydurmadan** sırala»).
-- Dört okuma yolu bu satıra yaslanır:
--   * `Timeline.tsx` — tarih metni (`eventDateText`), kesinlik rozeti
--     (`PRECISION_LABEL`), tarih rolü rozeti, durum rozeti («Onaylandı» ⇔
--     `status='confirmed'`, değilse «Taslak») ve ham tarih notu **doğrudan**
--     bu sütunlardan basılır;
--   * `list_events` — kronoloji **`sortable_date`** ile sıralanır; tarihi
--     bilinmeyenler sonda ayrı grupta;
--   * `csv_export` — `event_date_text` ile aynı üçlüden üretir;
--   * `review_commit::review_target_exists` — inceleme öğesinin hedefi bu
--     tablodan çözülür.
-- `event_sources` (kaynak bağı) 0032'den beri kapılı; `claims` v32'de kapılı;
-- ama **olayın kendisi** 0001'den beri **tetikleyicisizdi**. Şemadaki tek
-- kapı dört `CHECK`ti (kesinlik, rol, köken, durum kapalı kümeleri) ve
-- `unknown ⇒ sortable_date IS NULL`. Tarih üçlüsünün tutarlılığı, sıralama
-- değerinin tarihten türetilmesi, kökenin (assertion_kind) değişmezliği,
-- durumun ileri-akışı ve satırın dosyası **şemada yoktu**.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v60; sonda
-- `scratch/dim-v61-probe/measure_v61.py`, log `measure_before.log`):
-- **66 kusur yazımının 66'sı KABUL**, 16 meşru yazma KABUL. Doğum (41):
-- boş/boşluk/kırpılmamış/65 haneli kimlik; boşluk/boş/65 haneli tür;
-- boş/boşluk başlık; kırpılmamış açıklama; **gün** kesinliğinde ay biçimi /
-- dolgusuz gün / takvimde olmayan gün (`2025-02-30`); **ay** kesinliğinde tam
-- gün taşıyan değer, ay `13` ve ay `00`; **yıl** kesinliğinde ay taşıyan
-- değer; **aralık**ta tek tarih / ters aralık / yarım aralık / ayırıcısız
-- değer (`…X…`) / takvim dışı başlangıç ve bitiş; **yaklaşık**ta `dün`;
-- `sortable_date` tarihten **başka** (kronoloji yanlış sırada), sıralama
-- değeri yokken tarih var / tarih yokken sıralama değeri var (ekran tarih
-- uydurur), `unknown` + tarih taşır; `created_at` `dün` / kırpılmamış /
-- serbest metin (`'1'`), `updated_at` boş / kırpılmamış / `created_at`ten
-- önce; doğuşta `confirmed` (onay kullanıcı eylemidir), doğuşta silinmiş,
-- **silinmiş dosyaya** doğar; `date_raw` yalnız boşluk ve boş metin;
-- `row_version` 0 (iyimser kilit ilk yazmada çakışmayı göremez).
-- UPDATE (25): satır **başka dosyaya** taşındı (kaynak bağları eski dosyada
-- kalır — INV-05), köken çevrildi (kullanıcı beyanı «belge» oldu), kimlik
-- yeniden adlandırıldı, `created_at` 2030'a çekildi, onay geri alındı
-- (`confirmed`→`draft`), onay arşive çevrildi, **taslak arşive çevrildi**,
-- sıralama değeri 1999'a çekildi, tarih çevrilip sıralama değeri bırakıldı,
-- silinmiş satır **geri getirildi**, silinmiş satırın içeriği değişti, silme
-- anında içerik değişti (sürüm artarak da), `row_version` sıfırlandı,
-- bilinmeyen tarihe gün eklendi, ay kesinliğine tam gün yazıldı, aralık ters
-- çevrildi / ayırıcısız yazıldı, başlık boşaltıldı, tür kırpılmamış yazıldı,
-- `updated_at` `dün`, tarih silinip sıralama değeri bırakıldı, içerik
-- **sürüm artmadan** değişti, yaklaşık kesinliğe `dün` yazıldı.
--
-- Yazılan kural (2 tetikleyici):
--   * `events` BI — `id` kırpılmış-dolu ≤64; `case_id` **var ve silinmemiş**
--     dosyayı gösterir; `event_kind` kırpılmış-dolu ≤64; `title`
--     kırpılmış-dolu; `description` kırpılmış (boş olabilir); `date_raw` NULL
--     ya da kırpılmış-dolu; doğuşta durum **yalnız `draft`** (onay kullanıcı
--     eylemidir; `confirmed` doğan satır hiç onaylanmamış kaydı «onaylandı»
--     diye basardı); `created_at`/`updated_at` ISO-8601 ve
--     `updated_at ≥ created_at`; `row_version ≥ 1`; doğuşta **silinmemiş**;
--     **tarih üçlüsü** kesinlikle tutarlı: `day` ⇒ `date(value) = value`
--     (kanonik biçim **ve** takvim günü birlikte: `2025-02-30`, `2025-1-5`,
--     `2025-03` düşer), `month` ⇒ `date(value || '-01') = value || '-01'`
--     (ay 01–12), `year` ⇒ dört hane, `approximate` ⇒ üç biçimden biri,
--     `range` ⇒ `YYYY-AA-GG/YYYY-AA-GG` (ayırıcı dahil) ve başlangıç ≤ bitiş,
--     `unknown` ⇒ tarih **yok** (çift kuralı yakalar); `date_value` NULL ise
--     `sortable_date` de NULL, değilse `sortable_date` = tarihin kendisi
--     (aralıkta **başlangıç**). Gün/ay için ayrıca `GLOB` **yazılmadı**:
--     `date()` eşitliği kanonik biçimi zaten zorunlu kılar (mutasyon kapısı
--     bu iki cümlenin etkisiz olduğunu gösterdi — çıkarıldı).
--   * `events` BU — `id`, `case_id`, `created_at` ve **`assertion_kind`**
--     (kaydın kökeni: kullanıcı / taraf iddiası / belge / sistem çıkarımı)
--     **donar**; durum yalnız `draft`→`confirmed` ilerler, `confirmed`
--     **terminal** (`disputed`/`archived` eski satırlarda donmuş kalır —
--     arayüz ikisini de «Taslak» bastığı için yeni geçiş açılmadı);
--     `deleted_at` yalnız NULL→damga (**geri getirme yok**) ve silme anında
--     içerik/durum değişmez; silinmiş satırda **hiçbir** alan değişmez;
--     `row_version` **azalamaz** ve içerik değişiyorsa **artmalıdır** (eşit
--     sürümle içerik yazan yol iyimser kilidi sessizce atlardı); içerik
--     yazıldığında tarih üçlüsü ve biçim kuralları **yine** geçerli; yalnız
--     durum/damga yazan geçişler eski satırı yeniden yargılamaz (onarım
--     yolu: kullanıcı düzenlemesi).
-- Ret `DIM_INTEGRITY:events` → `StorageError::Integrity` → **`E_DB_INTEGRITY`**.
--
-- Kapsam: `events` `case_id` taşır ama kapsam **referansı** (document_id /
-- document_version_id / parse_run_id / segment_id) taşımadığı için
-- `GUARDED_TABLES` taramasına girmez; `case_id` burada **donar** (taşıma
-- yok). Tarama sınıfı eklenmedi → `GUARDED_TABLES` **25 kalır**. Kural dosya
-- kapsamı değil **olay kanıtı** bütünlüğüdür.
--
-- Çekirdek hizası (küçük sıkılaşma): `create_event`/`update_event` artık
-- `date_raw`ı **kırpar** ve yalnız boşluksa `NULL` yazar (arayüz zaten
-- kırpıyor; doğrudan IPC yolunda boşluk dizisi kapıya takılırdı). Diğer
-- alanlar zaten kırpılıyor; normalizasyon (`normalize_event_date`) ve
-- türetme (`sortable_event_date`) kapının kurallarıyla **aynı** sözleşmede.

CREATE TRIGGER timeline_event_evidence_guard_bi
BEFORE INSERT ON events
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.id IS NOT trim(NEW.id)
  OR length(NEW.id) > 64
  OR NOT EXISTS (
       SELECT 1 FROM cases c
        WHERE c.id = NEW.case_id AND c.deleted_at IS NULL
     )
  OR NEW.event_kind IS NOT trim(NEW.event_kind)
  OR length(NEW.event_kind) = 0
  OR length(NEW.event_kind) > 64
  OR NEW.title IS NOT trim(NEW.title)
  OR length(NEW.title) = 0
  OR NEW.description IS NOT trim(NEW.description)
  OR (NEW.date_raw IS NOT NULL
      AND (NEW.date_raw IS NOT trim(NEW.date_raw)
           OR length(NEW.date_raw) = 0))
  OR COALESCE(NEW.status, '') <> 'draft'
  OR NEW.created_at IS NOT trim(NEW.created_at)
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
  OR NEW.updated_at IS NOT trim(NEW.updated_at)
  OR NEW.updated_at NOT LIKE '____-__-__T__:__:__%'
  OR NEW.updated_at < NEW.created_at
  OR NEW.row_version < 1
  OR NEW.deleted_at IS NOT NULL
  OR (NEW.date_value IS NULL) <> (NEW.sortable_date IS NULL)
  OR (NEW.date_value IS NOT NULL
      AND (
        CASE NEW.date_precision
          WHEN 'day' THEN
            date(NEW.date_value) IS NOT NEW.date_value
          WHEN 'month' THEN
            date(NEW.date_value || '-01') IS NOT (NEW.date_value || '-01')
          WHEN 'year' THEN
            NEW.date_value NOT GLOB '[0-9][0-9][0-9][0-9]'
          WHEN 'approximate' THEN
            NOT (
              (NEW.date_value GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
               AND date(NEW.date_value) IS NEW.date_value)
              OR (NEW.date_value GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]'
                  AND date(NEW.date_value || '-01') IS (NEW.date_value || '-01'))
              OR NEW.date_value GLOB '[0-9][0-9][0-9][0-9]'
            )
          WHEN 'range' THEN
            NEW.date_value
              NOT GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
            OR date(substr(NEW.date_value, 1, 10)) IS NOT substr(NEW.date_value, 1, 10)
            OR date(substr(NEW.date_value, 12)) IS NOT substr(NEW.date_value, 12)
            OR substr(NEW.date_value, 1, 10) > substr(NEW.date_value, 12)
        END
      ))
  OR (NEW.date_value IS NOT NULL
      AND NEW.sortable_date IS NOT NULL
      AND NEW.sortable_date <> CASE NEW.date_precision
                                 WHEN 'range' THEN substr(NEW.date_value, 1, 10)
                                 ELSE NEW.date_value
                               END)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:events');
END;

CREATE TRIGGER timeline_event_evidence_guard_bu
BEFORE UPDATE ON events
WHEN NEW.id IS NOT OLD.id
  OR NEW.case_id IS NOT OLD.case_id
  OR NEW.created_at IS NOT OLD.created_at
  OR NEW.assertion_kind IS NOT OLD.assertion_kind
  OR NEW.row_version < OLD.row_version
  OR (NEW.row_version = OLD.row_version
      AND (NEW.title IS NOT OLD.title
           OR NEW.description IS NOT OLD.description
           OR NEW.event_kind IS NOT OLD.event_kind
           OR NEW.date_value IS NOT OLD.date_value
           OR NEW.date_precision IS NOT OLD.date_precision
           OR NEW.date_role IS NOT OLD.date_role
           OR NEW.date_raw IS NOT OLD.date_raw
           OR NEW.sortable_date IS NOT OLD.sortable_date))
  OR (NEW.status IS NOT OLD.status
      AND NOT (OLD.status = 'draft' AND NEW.status = 'confirmed'))
  OR (OLD.deleted_at IS NULL
      AND NEW.deleted_at IS NOT NULL
      AND (NEW.status IS NOT OLD.status
           OR NEW.title IS NOT OLD.title
           OR NEW.description IS NOT OLD.description
           OR NEW.event_kind IS NOT OLD.event_kind
           OR NEW.date_value IS NOT OLD.date_value
           OR NEW.date_precision IS NOT OLD.date_precision
           OR NEW.date_role IS NOT OLD.date_role
           OR NEW.date_raw IS NOT OLD.date_raw
           OR NEW.sortable_date IS NOT OLD.sortable_date))
  OR (OLD.deleted_at IS NOT NULL
      AND (NEW.deleted_at IS NOT OLD.deleted_at
           OR NEW.status IS NOT OLD.status
           OR NEW.title IS NOT OLD.title
           OR NEW.description IS NOT OLD.description
           OR NEW.event_kind IS NOT OLD.event_kind
           OR NEW.date_value IS NOT OLD.date_value
           OR NEW.date_precision IS NOT OLD.date_precision
           OR NEW.date_role IS NOT OLD.date_role
           OR NEW.date_raw IS NOT OLD.date_raw
           OR NEW.sortable_date IS NOT OLD.sortable_date))
  OR (NEW.date_precision IS NOT OLD.date_precision
      OR NEW.date_value IS NOT OLD.date_value
      OR NEW.sortable_date IS NOT OLD.sortable_date
      OR NEW.title IS NOT OLD.title
      OR NEW.description IS NOT OLD.description
      OR NEW.event_kind IS NOT OLD.event_kind
      OR NEW.date_raw IS NOT OLD.date_raw
      OR NEW.updated_at IS NOT OLD.updated_at)
     AND (NEW.event_kind IS NOT trim(NEW.event_kind)
          OR length(NEW.event_kind) = 0
          OR length(NEW.event_kind) > 64
          OR NEW.title IS NOT trim(NEW.title)
          OR length(NEW.title) = 0
          OR NEW.description IS NOT trim(NEW.description)
          OR (NEW.date_raw IS NOT NULL
              AND (NEW.date_raw IS NOT trim(NEW.date_raw)
                   OR length(NEW.date_raw) = 0))
          OR NEW.updated_at IS NOT trim(NEW.updated_at)
          OR NEW.updated_at NOT LIKE '____-__-__T__:__:__%'
          OR NEW.updated_at < NEW.created_at
          OR NEW.row_version < 1
          OR (NEW.date_value IS NULL) <> (NEW.sortable_date IS NULL)
          OR (NEW.date_value IS NOT NULL
              AND (
                CASE NEW.date_precision
                  WHEN 'day' THEN
                    date(NEW.date_value) IS NOT NEW.date_value
                  WHEN 'month' THEN
                    date(NEW.date_value || '-01') IS NOT (NEW.date_value || '-01')
                  WHEN 'year' THEN
                    NEW.date_value NOT GLOB '[0-9][0-9][0-9][0-9]'
                  WHEN 'approximate' THEN
                    NOT (
                      (NEW.date_value GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                       AND date(NEW.date_value) IS NEW.date_value)
                      OR (NEW.date_value GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]'
                          AND date(NEW.date_value || '-01') IS (NEW.date_value || '-01'))
                      OR NEW.date_value GLOB '[0-9][0-9][0-9][0-9]'
                    )
                  WHEN 'range' THEN
                    NEW.date_value
                      NOT GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                    OR date(substr(NEW.date_value, 1, 10)) IS NOT substr(NEW.date_value, 1, 10)
                    OR date(substr(NEW.date_value, 12)) IS NOT substr(NEW.date_value, 12)
                    OR substr(NEW.date_value, 1, 10) > substr(NEW.date_value, 12)
                        END
              ))
          OR (NEW.date_value IS NOT NULL
              AND NEW.sortable_date IS NOT NULL
              AND NEW.sortable_date <> CASE NEW.date_precision
                                         WHEN 'range' THEN substr(NEW.date_value, 1, 10)
                                         ELSE NEW.date_value
                                       END))
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:events');
END;
