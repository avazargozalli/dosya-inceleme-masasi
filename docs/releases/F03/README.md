# F03 — Kasa, veritabanı ve yedek temeli

- **Sürüm etiketi:** `v0.2.0`
- **Durum:** Devam ediyor (aşağıdaki açık işler)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F03-T01 | ✅ | `crates/vault`: rastgele ana anahtar, Argon2id, sürümlü başlık |
| F03-T02 | ✅ | parola + kurtarma yuvası ayrı sarılı; `open_with_recovery` |
| F03-T04 | ✅ | `crates/storage`: 47 tablo, FK/CHECK, migrasyon checksum |
| F03-T05 | ✅ | rastgele `storage_id`, chunked AEAD blob, atomik rename |
| F03-T07 | ✅ | `jobs` idempotency + lease (crates/jobs) |
| F03-T10 | ✅ | `backup_to` (SQLCipher backup API), yanlış anahtarla açılmaz |
| F03-T12 | ✅ | disk/izin/yanlış parola hata kodları |

## Doğrulama

`cargo test -p dim-vault -p dim-storage` — birim testleri: anahtar sarma, başlık
kurcalama, truncation, FTS, FK, denetim zinciri, yedek.
CI: **bekliyor** (bu commit ile ilk koşu).

## Bilinen sınırlar

- Kasa başlığı yedekleme paketi F14'te; şu an DB yedeği alınır.
- `blobs` GC (referanssız artık temizliği) F14'te.
- Windows'ta SQLCipher derlemesi uzun; ayrı `windows-check` işinde.
