# F02 — Repository, derleme ve masaüstü iskeleti

- **Sürüm etiketi:** `v0.1.0`
- **Durum:** Devam ediyor
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F02-T01 | ✅ | `apps/`, `crates/`, `workers/`, `packages/`, `fixtures/`, `tests/` |
| F02-T02 | ✅ | `packages/contracts/schemas` → TS üretimi + drift kontrolü |
| F02-T03 | ✅ | komutlar alan işlemleriyle sınırlı; genel SQL/shell yok |
| F02-T04 | ✅ | CSP + `capabilities/default.json` (core + dialog) |
| F02-T05 | ✅ | `ApiError { code, message, retryable }`, tek hata biçimi |
| F02-T06 | ✅ | CI: Linux/macOS test + macOS Tauri check + Windows ayrı iş |
| F02-T07 | 🚧 | worker paketleme manifesti ve OCR dil dosyaları F06/F16'da |
| F02-T08 | ⬜ | WebView2 kurulum yolu (Windows kurucu F16) |
| F02-T09 | ⬜ | kullanıcı düzeyi kurulum/kaldırma (F16) |
| F02-T10 | ✅ | teşhis yalnız sürüm/platform; log içerik taşımaz |
| F02-T11 | ✅ | `CONTRIBUTING.md`, `SECURITY.md`, `docs/dev-setup.md`, `AGENTS.md` |
| F02-T12 | ✅ | `tests/smoke/worker_smoke.py` gerçek PDF ile |

## Doğrulama

GitHub Actions `ci`: 6/6 iş yeşil (sözleşme, arayüz, rust ×2, masaüstü, python).
`desktop-build` imzasız `.app` artefaktı üretir (F16'da imzalanacak).

## Açık

- Kurulum/kaldırma deneyimi ve WebView2 yolu F16'ya bağlı.
- Windows testleri ayrı `windows-check` işinde; her push'ta koşmaz.
