# F15 — Kalite, güvenlik, pilot

- **Durum:** Kısmi (otomasyon var, pilot yok)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F15-T01 erişilebilirlik regresyonu | ✅ | `src/a11y.test.ts` — etiketsiz alan, altsız görsel, adsız buton |
| F15-T02 güvenlik test matrisi | ✅ | `docs/security/test-matrix.md` (T1–T15) |
| F15-T03 SBOM | 🚧 | `scripts/sbom.sh`; CI job'u |
| F15-T04 performans benchmark | ⬜ | 100k belge / 1M sayfa ölçümü yok |
| F15-T05 pilot (5–8 avukat) | ⬜ | **insan gerekir**; protokol `docs/pilot/` |
| F15-T06 VoiceOver/NVDA turu | ⬜ | statik test var, ekran okuyucu turu yapılmadı |

## Dürüst durum

Otomatik erişilebilirlik kuralları yalnız yapısal hataları yakalar; gerçek ekran
okuyucu deneyimi ve pilot geri bildirimi olmadan F15 "tamam" sayılamaz.
