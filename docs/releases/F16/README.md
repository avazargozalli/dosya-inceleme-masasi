# F16 — İmzalı genel sürüm

- **Durum:** İskelet
- **Tarih:** 2026-10-23

| İş | Durum | Not |
|---|---|---|
| F16-T01 SBOM | 🚧 | `scripts/sbom.sh` + CI `sbom` job'u; artefakt yayımlanıyor |
| F16-T02 güncelleme imza doğrulama | ⬜ | updater anahtarı yok; `plugins.updater` iskeleti |
| F16-T03 atomik migrasyon geri dönüş | 🚧 | `vault_restore` var; gerçek tatbikat yapılmadı |
| F16-T04 Apple Developer ID + notarization | ⬜ | **hesap/sertifika gerekir** |
| F16-T05 Windows kod imzası | ⬜ | **sertifika gerekir** |
| F16-T06 imzalı `.dmg` / `.msi` | ⬜ | yukarıdakilere bağlı |

## Dürüst durum

Paketler **imzasız**. Gatekeeper/Defender uyarısı beklenir. Bu fazın geri kalanı
kodla değil, sertifika ve hesap sağlanmasıyla kapanır; o gelene kadar "imzalı
sürüm" denmez.
