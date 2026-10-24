# F17 — Windows ARM64 paketleme

- **Durum:** **ENGELLİ (upstream)** — kod tarafı hazır, derleme SQLCipher/OpenSSL
  yüzünden aarch64 Windows'ta tamamlanmıyor
- **Tarih:** 2026-10-24

## Ne yapıldı

`desktop-build` işi matrise ayrıldı: `x86_64-pc-windows-msvc` (x64) ve
`aarch64-pc-windows-msvc` (arm64). Her hedef kendi `--target`'ıyla
`tauri build --bundles msi` çalıştırır, MSI'nin varlığını **boyut + sha256** ile
kanıtlar ve artefakta koyar (`if-no-files-found: error`).

## Neden ARM64 üretilemiyor

Run `40058501597` (commit `08d82c3`):

- **x64: BAŞARILI** — `Dosya İnceleme Masası_0.1.0_x64_tr-TR.msi`,
  6.537.216 bayt,
  sha256 `46504e297900ae942265782d38942eaa0e5f6e0f0cc948e4492c96e62c002b0b`.
- **arm64: BAŞARISIZ** — `openssl-sys v0.9.117` / `openssl-src` vendored OpenSSL
  derlemesi `HostX64\arm64\nmake.exe` altında düşüyor:
  `fatal error C1083: Cannot open compiler generated file: ''` →
  `NMAKE : fatal error U1077` → `error: failed to build custom build command for openssl-sys`.

Sebep bizim kodumuz değil: `rusqlite` `bundled-sqlcipher-vendored-openssl`
özelliği SQLCipher'ı OpenSSL ile statik derliyor; `openssl-src` Windows ARM64
çapraz derlemesini desteklemiyor.

## Karar

- ARM64 işi **deneysel** (`continue-on-error`) olarak matriste kalır: kırılınca
  ana hattı kırmızıya çevirmez ama durum görünür kalır.
- ARM64 MSI **üretilmedi**; "destekleniyor" denmez.
- Çözüm yolları (upstream bekliyor): `openssl-src` ARM64 düzeltmesi, SQLCipher'ı
  sistem OpenSSL'iyle bağlamak veya Windows ARM64'te farklı şifreleme sağlayıcısı.
  Üçü de mimari karar gerektirir; insan onayı olmadan değiştirilmedi.

## Doğrulama

- `gh run view 40058501597` → `windows-binary (x86_64…, x64)` **success**,
  `(aarch64…, arm64)` **failure** (yukarıdaki OpenSSL hatası).
- macOS `bundle` işleri (aarch64 + x86_64 .app) aynı koşuda **success**.
