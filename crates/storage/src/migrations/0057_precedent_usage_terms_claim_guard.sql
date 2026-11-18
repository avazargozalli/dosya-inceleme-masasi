-- 0057: **kullanım koşulu iddiası** (`precedents.usage_basis`,
-- `redistribution`, `terms_url`, `terms_note`, `terms_recorded_at`) kendi
-- içinde tutarlı olmalı: «izinli» ancak dayanağıyla birlikte doğar ve yazılır.
--
-- Neden: bu beş sütun «bu metni **yeniden dağıtabilirim**» iddiasıdır ve
-- iddianın tek okuyucusu onu **yeniden denetlemez**:
--   * `build_precedent_corpus` / `precedent_corpus_coverage` yalnız
--     `redistribution` sütununa bakar; `allowed` ise metni **dosyaya yazar**
--     (`CorpusItem.text`) ve `all` kipinde kapıyı açar;
--   * `screens/Precedents.tsx` liste rozeti (`termsBadge`) `redistribution`
--     sütunundan «izinli» basar; koşul paneli damgayı ve dayanağı gösterir;
--   * `Precedents.tsx` kaydetmeden önce yalnız `needsWrittenBasis` uyarısını
--     gösterir — asıl kapı Rust'tadır (`set_precedent_usage_terms`, `E_TERMS`).
-- 0016 sütunları eklerken yalnız iki kapalı küme CHECK'i koydu; kuralın
-- kendisi (izin ⇒ dağıtılabilir dayanak + yazılı dayanak + damga; url biçimi)
-- **şemada yoktu**. Tabloda 0056 yalnız **künye**yi kapılıyor, bu beş sütunu
-- bilinçli olarak dışarıda bırakmıştı (0056 kapanış sınırı (b)).
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v56; sonda
-- `scratch/dim-v57-probe/measure_v57.py`, log `measure_before.log`):
-- 55 yazımın **55'i KABUL** — 37 kusur + 14 meşru + 4 eski satır yolu:
--   doğum (22): `allowed` + dayanak `unknown` / `internal_only`; `allowed` +
--     yazılı dayanak yok; not yalnız boşluk; `allowed` + damga yok / `dün` /
--     boş metin; url `ftp://`, şemasız, kırpılmamış (sondaki boşluk), boş
--     metin, yalnız boşluk, 2001 karakter, satır sonu / satır başı / sekme
--     taşıyan; not kırpılmamış, 4001 karakter; damga `dün` / boş /
--     `01.06.2026`.
--   UPDATE (15): dayanak `unknown`a / `internal_only`a çevrildi; url ve not
--     birlikte silindi; damga NULL'a çekildi / `dün` yazıldı; url `ftp://`
--     yapıldı; not yalnız boşluğa çevrildi (url zaten yok); not kırpılmamış
--     yazıldı / 4001 karaktere çıkarıldı; url kırpılmamış (sondaki boşluk)
--     yazıldı / satır sonu / satır başı / sekme eklendi / 2001 karaktere
--     çıkarıldı; yazılı dayanak yokken izin sonradan **açıldı** (yalnız
--     `redistribution` değişti).
--
-- Yazılan kural (2 tetikleyici):
--   * INSERT — «izinli» iddiası **desteklenmiş** olmalı: `usage_basis`
--     dağıtılabilir üçlüden (`official_public|licence|permission`;
--     `internal_only` ve `unknown` **dağıtılamaz**), yazılı dayanak var
--     (`terms_url` ya da `terms_note` kırpılmış-boş değil) ve
--     `terms_recorded_at` ISO-8601 damgası taşıyor. `terms_url` NULL ya da
--     kırpılmış `http://`|`https://` bağlantısı (≤ 2000 karakter, CR/LF/TAB
--     yok); `terms_note` kırpılmış (≤ 4000 karakter); damga NULL ya da ISO.
--     `unknown`/`not_allowed` serbest: **izin vermemek** kanıt istemez.
--   * UPDATE — beş sütundan **biri değişiyorsa** yeni satır aynı kurala uymalı.
--     Sütunlara dokunmayan yazma (künye no-op'u) takılmaz: kapı yeni bir
--     tutarsızlık doğmasını durdurur, eski satırı kilitlemez. Böylece v57
--     öncesi kanıtsız `allowed` satırı **onarılabilir** (dayanak + yazılı
--     kanıt + damga yazılır) — 0056'nın aksine burada onarım yolu **açık**.
-- Ret `DIM_INTEGRITY:precedents` → `StorageError::Integrity` →
-- **`E_DB_INTEGRITY`**.
--
-- Kapsam taraması: tablo `case_id` taşımaz → yeni sınıf eklenmedi,
-- `GUARDED_TABLES` **25 kalır** (kural dosya kapsamı değil **iddia
-- bütünlüğü**dür).
--
-- Dürüst sınırlar: (a) v57 öncesi kanıtsız `allowed` satırları **silinmez** ve
-- kendiliğinden düzelmez; kapı yalnız yeni yazmayı durdurur — onarım elle
-- (arayüz/Rust yolu) yapılır. (b) Kapı iddianın **doğruluğunu** kanıtlamaz:
-- yazılı dayanak metninin gerçekten o izni verdiği, url'nin yaşadığı ve
-- kararın o kaynaktan alındığı doğrulanmaz — yalnız iddianın **iç tutarlılığı**
-- ve biçimi. (c) SQLite `trim()` yalnız ASCII boşluk kırpar (U+00A0 artığı
-- geçer); Rust `str::trim` U+00A0'ı da kırpar. (d) Kontrol karakterlerinden
-- yalnız CR/LF/TAB denetlenir (gömülü NUL ve diğer C0 karakterleri SQL
-- metninde temsil edilemez; Rust hepsini reddeder). (e) Damga **gelecekte**
-- olabilir ve damga eskisinden **geriye** alınabilir (saat/karşılaştırma
-- güvenilmez); yalnız biçimi ve `allowed` için **varlığı** denetlenir.
-- (f) Notun **içeriği** (gerçekten dayanak mı) ve url'nin **hedefi** denetlenmez.
-- (g) Tetikleyicileri anahtarı olan saldırgan düşürebilir; çekirdek bu yüzden
-- aynı kuralı **dışa aktarma anında** ayrıca uygular (bkz. `precedent_terms.rs`).
-- (h) Arayüz davranışı değişmedi; gerçek pencere + görsel QA **insan bekliyor**.

CREATE TRIGGER precedent_usage_terms_guard_bi
BEFORE INSERT ON precedents
WHEN (NEW.redistribution = 'allowed'
      AND (NEW.usage_basis NOT IN ('official_public', 'licence', 'permission')
           OR (NEW.terms_url IS NULL
               AND length(trim(COALESCE(NEW.terms_note, ''))) = 0)
           OR NEW.terms_recorded_at IS NULL))
  OR (NEW.terms_url IS NOT NULL
      AND (NEW.terms_url <> trim(NEW.terms_url)
           OR (NEW.terms_url NOT LIKE 'http://%'
               AND NEW.terms_url NOT LIKE 'https://%')
           OR length(NEW.terms_url) > 2000
           OR instr(NEW.terms_url, char(10)) > 0
           OR instr(NEW.terms_url, char(13)) > 0
           OR instr(NEW.terms_url, char(9)) > 0))
  OR NEW.terms_note <> trim(NEW.terms_note)
  OR length(NEW.terms_note) > 4000
  OR (NEW.terms_recorded_at IS NOT NULL
      AND NEW.terms_recorded_at NOT LIKE '____-__-__T__:__:__%')
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:precedents');
END;

CREATE TRIGGER precedent_usage_terms_guard_bu
BEFORE UPDATE ON precedents
WHEN (NEW.usage_basis IS NOT OLD.usage_basis
      OR NEW.redistribution IS NOT OLD.redistribution
      OR NEW.terms_url IS NOT OLD.terms_url
      OR NEW.terms_note IS NOT OLD.terms_note
      OR NEW.terms_recorded_at IS NOT OLD.terms_recorded_at)
 AND ((NEW.redistribution = 'allowed'
       AND (NEW.usage_basis NOT IN ('official_public', 'licence', 'permission')
            OR (NEW.terms_url IS NULL
                AND length(trim(COALESCE(NEW.terms_note, ''))) = 0)
            OR NEW.terms_recorded_at IS NULL))
   OR (NEW.terms_url IS NOT NULL
       AND (NEW.terms_url <> trim(NEW.terms_url)
            OR (NEW.terms_url NOT LIKE 'http://%'
                AND NEW.terms_url NOT LIKE 'https://%')
            OR length(NEW.terms_url) > 2000
            OR instr(NEW.terms_url, char(10)) > 0
            OR instr(NEW.terms_url, char(13)) > 0
            OR instr(NEW.terms_url, char(9)) > 0))
   OR NEW.terms_note <> trim(NEW.terms_note)
   OR length(NEW.terms_note) > 4000
   OR (NEW.terms_recorded_at IS NOT NULL
       AND NEW.terms_recorded_at NOT LIKE '____-__-__T__:__:__%'))
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:precedents');
END;
