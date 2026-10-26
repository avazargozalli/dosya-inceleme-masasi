# F16 — Dağıtım (imzasız)

- **Durum:** Kapsam daraltıldı (ADR-0012)
- **Tarih:** 2026-10-23

| İş | Durum | Not |
|---|---|---|
| F16-T01 SBOM | ✅ | `scripts/sbom.sh`, CI `sbom` job'u, artefakt |
| F16-T02 güncelleme imza doğrulama | 🚧 | mekanizma var, anahtar yok; `plugins.updater.active=false` |
| F16-T03 atomik migrasyon geri dönüş | ✅ | `rollback_vault` (sabit bağlantılı güvenlik kopyası, dolu `-wal` kapısı, artık süpürme) + uygulama içi akış (`pre_update.rs`, Ayarlar kartı, onay metni); `ROLLBACK-DRILL.md` — **süreç öldürme ve gerçek disk dolması tatbikatı** (kusur #39–#41) |
| F16-T04 Apple Developer ID + notarization | ➖ | **kapsam dışı** (ADR-0012) |
| F16-T05 Windows kod imzası | ➖ | **kapsam dışı** (ADR-0012) |
| F16-T06 imzalı `.dmg` / `.msi` | ➖ | **kapsam dışı**; imzasız paket üretiliyor |
| F16-T07 güncelleme öncesi yedek + işlemli migrasyon | ✅ | `upgrade_with_backup`: yedek doğrulanmadan migrasyon başlamaz |
| F16-T08 eski uygulama yeni şemayı bozmasın | ✅ | `E_DB_SCHEMA_NEWER`, yazmadan reddeder (testli) |
| F16-T09 hatalı güncellemeyi reddet | 🚧 | bozuk yedek reddedilir; kesilen indirme/yanlış mimari denenmedi (updater kapalı) |

## Dürüst durum

Paketler **imzasızdır**; macOS Gatekeeper ve Windows Defender uyarı gösterir.
Kaynak açık olduğu için isteyen kendi sertifikasıyla imzalar. Bu depo hiçbir
zaman "imzalı" ya da "notarize edilmiş" sürüm iddia etmez.

## Kullanıcı kendi imzasını nasıl ekler

1. `apps/desktop/src-tauri/tauri.conf.json` → `bundle.macOS.signingIdentity`
   (veya `APPLE_SIGNING_IDENTITY` ortam değişkeni) ve Windows için
   `certificateThumbprint`.
2. Güncelleme dağıtacaksa `plugins.updater.pubkey` ve `endpoints` doldurulur,
   `active=true` yapılır.
3. `desktop-build.yml` imzalama sırlarını ortam değişkeni olarak geçirir;
   sırlar depoda tutulmaz.
