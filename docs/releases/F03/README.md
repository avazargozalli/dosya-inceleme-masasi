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

## Otomatik kilit (ek)

- `src/lib/autolock.ts`: saf `shouldAutoLock(lastActivity, now, limit)`; varsayılan
  10 dakika, 30 saniyede bir kontrol.
- `App.tsx`: `pointerdown`/`keydown`/`wheel` hareketi sayacı sıfırlar; kasa açıkken
  süre dolunca `lockVault()` çağrılır ve durum yenilenir.
- Saat geri alınırsa (uyku, manuel değişiklik) güvenli tarafta kalınır: kilitlenir.
- 5 birim testi (`src/lib/autolock.test.ts`).

## Referanssız blob temizliği (ek)

- `crates/storage/src/gc.rs`: `garbage_collect_blobs(dry_run)` yalnız hiçbir
  `document_versions.blob_id` ve `pages.derivative_blob_id` tarafından
  gösterilmeyen kayıtları seçer; `document_versions` FK'si `RESTRICT` olduğu
  için yanlış silme veritabanı tarafından da engellenir.
- `blob_inventory()` toplam/referanssız/boyut özeti verir.
- `vault_gc` Tauri komutu kuru çalıştırma ve gerçek silme sunar; dosya
  silinemezse `missingFiles` ile dürüstçe raporlar.
- Ayarlar'da iki adımlı akış: **Tara** → **Temizle**.
- 3 storage testi (referanslı blob korunur, boş kasa no-op, türev yolu).

## Hâlâ eksik

- OS düzeyinde kilit (ekran kilidi) algılama.
- Parola değiştirme (anahtar rewrap) akışı.

