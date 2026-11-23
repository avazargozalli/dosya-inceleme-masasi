-- 0068: **türev ve önbellek satırları birer makbuzdur** — `render_derivatives`
-- satırı «şu belge sürümünün şu ayrıştırma koşusundan, şu blob'daki baytlarla,
-- şu işleyici sürümüyle üretilmiş **sayfa kümesi**; içerik özeti şu»,
-- `ai_cache_entries` satırı «şu dosyada şu önbellek anahtarının çıktısı **şu
-- çalıştırmadan** doğdu, şu blob'da duruyor, şu ana kadar geçerli» iddiasıdır.
--
-- Neden: iki tablo da 0001'den beri **hiçbir** tetikleyici taşımıyordu; tek
-- kapıları biçim CHECK'leri ve `ai_cache_entries`in `UNIQUE(case_id,
-- cache_key)` indeksidir. Ölçüldü (gerçek SQLite; sonda
-- `scratch/dim-v68-probe/measure_v68.py`, loglar `measure_before.log` /
-- `measure_after.log`): düzeltme öncesi (v67) **89 yazımın 86'sı KABUL** —
-- 79 saldırının **76'sı kabul** (üçü yalnız mevcut FK'dan RED) + 10 meşru;
-- düzeltmeden sonra (v68) **79 RED · 10 KABUL · sapma 0**. Kusurlar:
-- `render_derivatives.document_version_id` **var olmayan** sürümü
-- gösterebiliyor (FK yok), `parse_run_id` başka sürümün koşusunu,
-- `blob_id` **var olmayan** blob'u gösterebiliyor (v66 yalnız *silme*yi
-- engelliyor, doğuşta kanıt istemiyor), `settings_hash`/`content_hash` serbest
-- metin, `page_manifest` bozuk JSON, damga `dün`; satır sonradan her sütunuyla
-- çevrilebiliyor. `ai_cache_entries` ise `ai_run_id` ile **başka dosyanın**
-- çalıştırmasına bağlanabiliyor (dosya kapsamı sızıntısı), `output_blob_id`
-- var olmayan blob'u gösterebiliyor, `expires_at`/`validation_version`
-- serbest metin; satır her sütunuyla çevrilebiliyor.
-- Dürüst olay: ilk yazımda `page_manifest` kuralı `NEW.page_manifest IS NOT
-- json_valid(...)` diye kurulmuştu — metni 1'e eşitlediği için **her** meşru
-- satırı reddetti (üç testin ikisi ilk koşuda kırmızı); `json_valid(...) IS NOT 1`
-- olarak düzeltildi, iki test yeniden yeşil ölçüldü ve sonda iki tarafta
-- **baştan** koşuldu. İlk koşunun hükümleri kanıt sayılmadı.
--
-- Okuyucular: bugün **ürün yolu yok** (iki tabloya da yazan/okuyan Rust kodu
-- yok; yalnız GC referans kapısı ve testler). Bu bilinçli bir sınırdır: kapı
-- bugünkü akışları değil, **gelecekte bağlanacak** türev/önbellek yollarını
-- ve doğrudan SQL yazan her yolu bağlar. `ai_cache_entries`in kendi
-- `case_id`'si vardır ama kapsam kapısı listesine (`GUARDED_TABLES`) dahil
-- **edilmedi**: tablo `document_version_id`/`parse_run_id` gibi kapsam
-- sütunlarından hiçbirini taşımaz; çapraz dosya bağı bu göçün BI kuralıyla
-- (`ai_run_id`in çalıştırmasının `case_id`'si satırınkiyle aynı olmalı)
-- tek noktada kapanır. Liste **25 kalır**.
--
-- Yazılan kurallar:
--   * `render_derivatives` BI — `id` kırpılmış-dolu ≤64; `document_version_id`
--     kırpılmış-dolu ve **var + dosyası silinmemiş** sürümü gösterir;
--     `parse_run_id` NULL ya da kırpılmış-dolu ve **aynı sürümün** koşusu;
--     `blob_id` kırpılmış-dolu ve **var olan** blob; `renderer_version`
--     kırpılmış-dolu ≤64; `settings_hash` **64 küçük harf hex**;
--     `font_manifest_hash` NULL ya da 64 küçük harf hex; `page_manifest`
--     **geçerli JSON dizi**; `content_hash` 64 küçük harf hex; `created_at`
--     ISO-8601 + gerçek gün/saat (0066 deseni).
--   * `render_derivatives` BU — **on sütunun tamamı donar** (türev, üretim
--     anının makbuzudur; sonradan «başka sürümün/başka baytların» diyemez).
--     No-op takılmaz.
--   * `render_derivatives` BD — **canlı türev silinemez**: bir
--     `redaction_plans.render_derivative_id` satırı hâlâ gösteriyorsa silme
--     reddedilir (maske planının hükmü çözülemez olmasın). Referanssız türev
--     silinebilir (bugün ürün yolu yok; testler ve GC yolu kullanır).
--   * `ai_cache_entries` BI — `id` kırpılmış-dolu ≤64; `case_id` **var ve
--     silinmemiş** dosya; `cache_key` kırpılmış-dolu ≤512, CR/LF/TAB yok;
--     `ai_run_id` NULL ya da kırpılmış-dolu ve **aynı dosyanın** çalıştırması
--     (çapraz dosya sızıntısı fail-closed kapanır); `output_blob_id` NULL ya
--     da **var olan** blob; `validation_version` kırpılmış-dolu ≤64;
--     `expires_at` NULL ya da ISO-8601 + gerçek gün/saat; `created_at` ISO.
--   * `ai_cache_entries` BU — yedi künye sütunu **donar** (`id`, `case_id`,
--     `cache_key`, `ai_run_id`, `output_blob_id`, `validation_version`,
--     `created_at`); `expires_at` yalnız **ileri** gidebilir (NULL → damga ya
--     da daha sonraki damga); geri alınamaz, temizlenemez.
--   * `ai_cache_entries` BD — **yok**: önbellek satırı silinebilir olmalı
--     (süre dolumu/temizlik meşru). Bu bilinçli sınırdır; kalıcı kanıt isteyen
--     yol `transfer_receipts`/`ai_runs`tur.
-- Ret `DIM_INTEGRITY:render_derivatives` / `DIM_INTEGRITY:ai_cache_entries`
-- → `E_DB_INTEGRITY`.
--
-- Çekirdek hizası: ürün bu iki tabloya yazmadığı için davranış değişikliği
-- yok; çekirdek zaten 64 küçük hex özetler ve `now_iso()` damgaları üretiyor.
-- Test fikstürleri gerçek sürüm/koşu/blob kurar (kapı gevşetilmedi).
--
-- Dürüst sınırlar (kapanışta belgeye de yazılır): (a) özetlerin gerçekten o
-- baytların/ayarların SHA-256'sı olduğu **yeniden hesaplanmaz** (SQLite'ta
-- SHA-256 yok) — biçim + değişmezlik; (b) `page_manifest`in sayfa kümesini
-- gerçekten tarif ettiği doğrulanmaz, yalnız JSON dizi biçimi; (c)
-- `renderer_version`/`validation_version` içeriği denetlenmez (opak sürüm
-- etiketi); (d) v68 öncesi tutarsız satırlar **silinmez/düzeltilmez** ve
-- kimlik sütunları donduğu için **onarılamaz** — onarım yolu yeni türev/yeni
-- önbellek satırı; (e) tetikleyicileri anahtarı olan saldırgan düşürebilir;
-- (f) `ai_cache_entries.output_blob_id`in o dosyaya ait olduğu çapraz
-- denetlenmez (blob'lar dosya taşımaz), yalnız varlığı; (g) `ai_cache_entries`
-- kapsam kapısı listesinde değildir (gerekçe yukarıda).

CREATE TRIGGER render_derivative_receipt_guard_bi
BEFORE INSERT ON render_derivatives
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.id IS NOT trim(NEW.id)
  OR length(NEW.id) > 64
  OR length(NEW.document_version_id) > 64
  OR NOT EXISTS (
       SELECT 1 FROM document_versions v
         JOIN documents d ON d.id = v.document_id
        WHERE v.id = NEW.document_version_id
          AND d.deleted_at IS NULL
     )
  OR (
       NEW.parse_run_id IS NOT NULL
       AND (
         length(NEW.parse_run_id) > 64
         OR NOT EXISTS (
              SELECT 1 FROM parse_runs p
               WHERE p.id = NEW.parse_run_id
                 AND p.document_version_id = NEW.document_version_id
            )
       )
     )
  OR length(trim(COALESCE(NEW.blob_id, ''))) = 0
  OR NEW.blob_id IS NOT trim(NEW.blob_id)
  OR length(NEW.blob_id) > 64
  OR NOT EXISTS (SELECT 1 FROM blobs b WHERE b.id = NEW.blob_id)
  OR length(trim(COALESCE(NEW.renderer_version, ''))) = 0
  OR NEW.renderer_version IS NOT trim(NEW.renderer_version)
  OR length(NEW.renderer_version) > 64
  OR length(NEW.settings_hash) IS NOT 64
  OR NEW.settings_hash GLOB '*[^0-9a-f]*'
  OR (
       NEW.font_manifest_hash IS NOT NULL
       AND (
         length(NEW.font_manifest_hash) IS NOT 64
         OR NEW.font_manifest_hash GLOB '*[^0-9a-f]*'
       )
     )
  OR (
       CASE
         WHEN json_valid(NEW.page_manifest) = 1
         THEN json_type(NEW.page_manifest)
       END
     ) IS NOT 'array'
  OR length(NEW.content_hash) IS NOT 64
  OR NEW.content_hash GLOB '*[^0-9a-f]*'
  OR NEW.created_at IS NOT trim(NEW.created_at)
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.created_at, 1, 10)) IS NOT substr(NEW.created_at, 1, 10)
  OR time(substr(NEW.created_at, 12, 8)) IS NOT substr(NEW.created_at, 12, 8)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:render_derivatives');
END;

CREATE TRIGGER render_derivative_receipt_guard_bu
BEFORE UPDATE ON render_derivatives
WHEN NEW.id IS NOT OLD.id
  OR NEW.document_version_id IS NOT OLD.document_version_id
  OR NEW.parse_run_id IS NOT OLD.parse_run_id
  OR NEW.blob_id IS NOT OLD.blob_id
  OR NEW.renderer_version IS NOT OLD.renderer_version
  OR NEW.settings_hash IS NOT OLD.settings_hash
  OR NEW.font_manifest_hash IS NOT OLD.font_manifest_hash
  OR NEW.page_manifest IS NOT OLD.page_manifest
  OR NEW.content_hash IS NOT OLD.content_hash
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:render_derivatives');
END;

CREATE TRIGGER render_derivative_receipt_guard_bd
BEFORE DELETE ON render_derivatives
WHEN EXISTS (
       SELECT 1 FROM redaction_plans p
        WHERE p.render_derivative_id = OLD.id
     )
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:render_derivatives');
END;

CREATE TRIGGER ai_cache_receipt_guard_bi
BEFORE INSERT ON ai_cache_entries
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.id IS NOT trim(NEW.id)
  OR length(NEW.id) > 64
  OR NOT EXISTS (
       SELECT 1 FROM cases c
        WHERE c.id = NEW.case_id
          AND c.deleted_at IS NULL
     )
  OR length(trim(COALESCE(NEW.cache_key, ''))) = 0
  OR NEW.cache_key IS NOT trim(NEW.cache_key)
  OR length(NEW.cache_key) > 512
  OR instr(NEW.cache_key, char(10)) > 0
  OR instr(NEW.cache_key, char(13)) > 0
  OR instr(NEW.cache_key, char(9)) > 0
  OR (
       NEW.ai_run_id IS NOT NULL
       AND (
         length(trim(NEW.ai_run_id)) = 0
         OR NEW.ai_run_id IS NOT trim(NEW.ai_run_id)
         OR length(NEW.ai_run_id) > 64
         OR NOT EXISTS (
              SELECT 1 FROM ai_runs r
               WHERE r.id = NEW.ai_run_id
                 AND r.case_id = NEW.case_id
            )
       )
     )
  OR (
       NEW.output_blob_id IS NOT NULL
       AND (
         length(trim(NEW.output_blob_id)) = 0
         OR NEW.output_blob_id IS NOT trim(NEW.output_blob_id)
         OR length(NEW.output_blob_id) > 64
         OR NOT EXISTS (
              SELECT 1 FROM blobs b WHERE b.id = NEW.output_blob_id
            )
       )
     )
  OR length(trim(COALESCE(NEW.validation_version, ''))) = 0
  OR NEW.validation_version IS NOT trim(NEW.validation_version)
  OR length(NEW.validation_version) > 64
  OR (
       NEW.expires_at IS NOT NULL
       AND (
         NEW.expires_at IS NOT trim(NEW.expires_at)
         OR NEW.expires_at NOT LIKE '____-__-__T__:__:__%'
         OR date(substr(NEW.expires_at, 1, 10))
            IS NOT substr(NEW.expires_at, 1, 10)
         OR time(substr(NEW.expires_at, 12, 8))
            IS NOT substr(NEW.expires_at, 12, 8)
       )
     )
  OR NEW.created_at IS NOT trim(NEW.created_at)
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.created_at, 1, 10)) IS NOT substr(NEW.created_at, 1, 10)
  OR time(substr(NEW.created_at, 12, 8)) IS NOT substr(NEW.created_at, 12, 8)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:ai_cache_entries');
END;

CREATE TRIGGER ai_cache_receipt_guard_bu
BEFORE UPDATE ON ai_cache_entries
WHEN NEW.id IS NOT OLD.id
  OR NEW.case_id IS NOT OLD.case_id
  OR NEW.cache_key IS NOT OLD.cache_key
  OR NEW.ai_run_id IS NOT OLD.ai_run_id
  OR NEW.output_blob_id IS NOT OLD.output_blob_id
  OR NEW.validation_version IS NOT OLD.validation_version
  OR NEW.created_at IS NOT OLD.created_at
  OR (
       NEW.expires_at IS NOT OLD.expires_at
       AND (
         NEW.expires_at IS NULL
         OR (
              OLD.expires_at IS NOT NULL
              AND NEW.expires_at <= OLD.expires_at
            )
       )
     )
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:ai_cache_entries');
END;
