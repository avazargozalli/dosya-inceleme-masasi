-- 0066: **şifreli bayt kimliği** — `blobs` satırı «**şu baytlar** şu dosyada,
-- şu boyutta, şu anahtarla duruyor» iddiasıdır ve doğuşta kanıtlı olmalı,
-- sonrasında **değişmemeli**.
--
-- Neden: `blobs.storage_id` **dosya adıdır** — `gc.rs` yolu
-- `dir.join(format!("{}.blb", blob.storage_id))` diye kurar. Bugüne dek
-- `storage_id` **hiçbir yerde** biçim denetiminden geçmiyordu: şemada tek
-- kapı `UNIQUE` ve iki CHECK'ti (`cipher_version = 1`, `encrypted_size >= 0`,
-- `reference_count >= 0`); `blob_disk::is_storage_id` (32 küçük hex) yalnız
-- **disk taramasında** çalışır, DB satırını korumaz. Yol ayırıcı, uzantı,
-- CR/LF/TAB, büyük harf (macOS'ta varsayılan **büyük-küçük harf duyarsız**
-- dosya sistemi: `AB` ile `ab` **aynı dosyadır**) ve 64 haneden uzun kimlik
-- doğabiliyordu — GC `../../../../tmp/evil.blb` yolunu **kasanın dışında**
-- arar. Ayrıca `create_blob_record` doğuşta **kanıtsız `integrity_status =
-- 'verified'`** yazıyordu (sütunu bugün hiçbir okuyucu okumuyor: yalan damga)
-- ve `encrypted_size = 0`, boş `wrapped_key_ref`, doğuşta `reference_count =
-- 5` serbestti; satır sonradan her sütunuyla çevrilebiliyor, **sayaçlı** blob
-- makbuzsuz silinebiliyordu.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v65; sonda
-- `scratch/dim-v66-probe/measure_v66.py`, log `measure_before.log`):
-- **45 yazımın 40'ı KABUL** — 34 kusur (24 doğum + 8 UPDATE + 2 DELETE) +
-- 6 meşru yol; 5 RED yalnız mevcut CHECK/FK. Doğum: boş/boşluk/kırpılmamış
-- kimlik; boş/boşluk/kırpılmamış `storage_id`, **`../../../../tmp/evil`**,
-- uzantılı (`aa11.blb`), 31 hane, **büyük harf**, TAB; `encrypted_size = 0`;
-- `wrapped_key_ref` boş/boşluk/CR-LF-TAB/kırpılmamış; doğuşta
-- `reference_count = 5`; doğuşta kanıtsız `verified` ve `corrupt`; damga
-- `dün`/boş/kırpılmamış. UPDATE: `storage_id` çevirme ve **yol kaçışı**,
-- `encrypted_size` 0/7, `wrapped_key_ref`, `integrity_status`, `created_at`
-- 2030, `id`. DELETE: referanssız ve **sayaçlı** blob makbuzsuz silinebiliyor.
--
-- Yazılan kural (1 + 1 + 1 tetikleyici + yeni sütun):
--   * `integrity_checked_at` sütunu eklenir: `verified`/`corrupt` hükmü ancak
--     **damgalı** olabilir (kanıtsız damga yasak).
--   * BI — `id` kırpılmış-dolu ≤64; `storage_id` **dosya adı belirteci**:
--     1..=64 hane ve yalnız `[0-9a-z_-]` (yol ayırıcı, nokta, uzantı, CR/LF/
--     TAB, boşluk ve **büyük harf** imkânsız — ürün yolu `random_storage_id()`
--     zaten 32 küçük hex üretir, yani kural ürünün yazdığı biçimi **kapsar**,
--     testlerdeki `s1`/`st-claim`/UUID gibi belirteçleri de kapsar; «tam 32
--     hex» daraltması 75 meşru çağrı yerini kırardı ve dosya adı güvenliğine
--     ek bir şey katmazdı — aşağıdaki «dürüst sınırlar»); `encrypted_size ≥ 1`
--     (AEAD'in etiketi/noncesi her zaman vardır, sıfır bayt imkânsız);
--     `wrapped_key_ref` kırpılmış-dolu ≤128 ve CR/LF/TAB taşımaz; doğuşta
--     `reference_count = 0`; doğuşta **`integrity_status = 'unverified'`** ve
--     `integrity_checked_at` **NULL** (kanıtsız `verified`/`corrupt` doğamaz);
--     `created_at` ISO-8601 + gerçek gün/saat (0065 deseni).
--   * BU — **altı sütun donar** (`id`, `storage_id`, `cipher_version`,
--     `encrypted_size`, `wrapped_key_ref`, `created_at`): bayt kimliği
--     yazıldıktan sonra değişmez. `reference_count` yalnız **±1** oynar
--     (`add_version` yolu +1; geri alma −1) — sıfırdan beşe atlama, çevirme
--     yok. `integrity_status` değişiyorsa yeni hüküm `verified`/`corrupt`
--     olmalı ve **yeni** bir ISO damga taşımalı (`unverified`a geri dönüş
--     yok: kanıt silinemez); durum değişmeden damga yalnız `verified`/
--     `corrupt` satırda **tazelenebilir**, `unverified` satırda damga
--     yazılamaz. No-op takılmaz, eski tutarsız satır kilitlenmez (damga
--     kuralı yalnız **değişen** değere bakar).
--   * BD — **canlı blob silinemez**: `document_versions`, `pages`,
--     `render_derivatives` ya da `ai_cache_entries` hâlâ gösteriyorsa satır
--     silinemez (ilk ikisi FK'lı, son ikisi **FK'sız** — sessiz NULL/yetim
--     kalma yolu kapanır). Referanssız blob GC tarafından silinebilir.
-- Ret `DIM_INTEGRITY:blobs` → `E_DB_INTEGRITY`; `blobs` `case_id` taşımaz →
-- `GUARDED_TABLES` **25 kalır** (kapsam `document_versions` üzerinden).
--
-- Dürüst sınırlar (kapanışta belgeye de yazılır): (a) `storage_id`in diskte
-- gerçekten o adda dosya olduğu doğrulanmaz; `encrypted_size`in dosya boyutu
-- olduğu yeniden ölçülmez. (b) `wrapped_key_ref` **hedefinin** var olduğu
-- denetlenmez (etiket). (c) `integrity_status` yalnız **iç tutarlılık**
-- yönünden kapılıdır — bütünlük taramasının gerçekten koştuğu kanıtlanmaz;
-- sütunun bugün okuyucusu yok. (d) v66 öncesi tutarsız satırlar silinmez/
-- düzeltilmez, kimlik sütunları donduğu için **onarılamaz** — onarım yolu yeni
-- içe aktarma. (e) BD dört sütunu bilir; gelecekte blob'a işaret eden yeni bir
-- sütun eklenirse `gc.rs`teki `every_blob_column_is_covered_by_the_reference_gate`
-- testi düşer (kapı listesi tek yerde). (f) Tetikleyicileri anahtarı olan
-- saldırgan düşürebilir.

ALTER TABLE blobs ADD COLUMN integrity_checked_at TEXT;

CREATE TRIGGER blob_bytes_identity_guard_bi
BEFORE INSERT ON blobs
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.id IS NOT trim(NEW.id)
  OR length(NEW.id) > 64
  OR length(NEW.storage_id) = 0
  OR length(NEW.storage_id) > 64
  OR NEW.storage_id GLOB '*[^0-9a-z_-]*'
  OR NEW.encrypted_size < 1
  OR length(trim(COALESCE(NEW.wrapped_key_ref, ''))) = 0
  OR NEW.wrapped_key_ref IS NOT trim(NEW.wrapped_key_ref)
  OR length(NEW.wrapped_key_ref) > 128
  OR instr(NEW.wrapped_key_ref, char(10)) > 0
  OR instr(NEW.wrapped_key_ref, char(13)) > 0
  OR instr(NEW.wrapped_key_ref, char(9)) > 0
  OR NEW.reference_count <> 0
  OR NEW.integrity_status IS NOT 'unverified'
  OR NEW.integrity_checked_at IS NOT NULL
  OR NEW.created_at IS NOT trim(NEW.created_at)
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.created_at, 1, 10)) IS NOT substr(NEW.created_at, 1, 10)
  OR time(substr(NEW.created_at, 12, 8)) IS NOT substr(NEW.created_at, 12, 8)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:blobs');
END;

CREATE TRIGGER blob_bytes_identity_guard_bu
BEFORE UPDATE ON blobs
WHEN NEW.id IS NOT OLD.id
  OR NEW.storage_id IS NOT OLD.storage_id
  OR NEW.cipher_version IS NOT OLD.cipher_version
  OR NEW.encrypted_size IS NOT OLD.encrypted_size
  OR NEW.wrapped_key_ref IS NOT OLD.wrapped_key_ref
  OR NEW.created_at IS NOT OLD.created_at
  OR NEW.reference_count > OLD.reference_count + 1
  OR NEW.reference_count < OLD.reference_count - 1
  OR (
       NEW.integrity_status IS NOT OLD.integrity_status
       AND (
         NEW.integrity_status = 'unverified'
         OR NEW.integrity_checked_at IS NULL
         OR NEW.integrity_checked_at IS OLD.integrity_checked_at
         OR NEW.integrity_checked_at IS NOT trim(NEW.integrity_checked_at)
         OR NEW.integrity_checked_at NOT LIKE '____-__-__T__:__:__%'
         OR date(substr(NEW.integrity_checked_at, 1, 10))
            IS NOT substr(NEW.integrity_checked_at, 1, 10)
         OR time(substr(NEW.integrity_checked_at, 12, 8))
            IS NOT substr(NEW.integrity_checked_at, 12, 8)
       )
     )
  OR (
       NEW.integrity_status IS OLD.integrity_status
       AND NEW.integrity_checked_at IS NOT OLD.integrity_checked_at
       AND (
         NEW.integrity_status = 'unverified'
         OR NEW.integrity_checked_at IS NULL
         OR NEW.integrity_checked_at IS NOT trim(NEW.integrity_checked_at)
         OR NEW.integrity_checked_at NOT LIKE '____-__-__T__:__:__%'
         OR date(substr(NEW.integrity_checked_at, 1, 10))
            IS NOT substr(NEW.integrity_checked_at, 1, 10)
         OR time(substr(NEW.integrity_checked_at, 12, 8))
            IS NOT substr(NEW.integrity_checked_at, 12, 8)
       )
     )
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:blobs');
END;

CREATE TRIGGER blob_bytes_identity_guard_bd
BEFORE DELETE ON blobs
WHEN EXISTS (SELECT 1 FROM document_versions v WHERE v.blob_id = OLD.id)
  OR EXISTS (SELECT 1 FROM pages p WHERE p.derivative_blob_id = OLD.id)
  OR EXISTS (SELECT 1 FROM render_derivatives d WHERE d.blob_id = OLD.id)
  OR EXISTS (SELECT 1 FROM ai_cache_entries c WHERE c.output_blob_id = OLD.id)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:blobs');
END;
