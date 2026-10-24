-- F12-T05: karar **tam metni** ve metin içi alıntı doğrulaması.
--
-- Tam metin kasadaki şifreli veritabanında tutulur; kütüphane listesi yalnız
-- hash ve karakter sayısı taşır (metnin kendisi UI'ya gönderilmez).
-- `body_hash` = SHA-256(tam metin). `verification_state='text_verified'`
-- "metin elimizde ve hash kayıtlı" demektir; künyenin doğruluğu ayrı boyuttur.
-- Metin yoksa alıntı doğrulaması "kontrol edilmedi" kalır — bu "uydurulmuş"
-- anlamına gelmez.
ALTER TABLE precedent_versions ADD COLUMN full_text TEXT;
ALTER TABLE precedent_versions ADD COLUMN text_char_count INTEGER NOT NULL DEFAULT 0;

CREATE INDEX precedent_versions_by_precedent
  ON precedent_versions(precedent_id, acquired_at DESC);
