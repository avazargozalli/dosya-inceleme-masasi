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

## Windows MSI (imzasız) — F02-T08/T09

`desktop-build` → `windows-binary (x86_64-pc-windows-msvc, x64)` **gerçek üretim**
ile doğrulandı (run `40058501597`, commit `08d82c3`):

- Dosya: `Dosya İnceleme Masası_0.1.0_x64_tr-TR.msi`
- Boyut: **6.537.216** bayt
- SHA-256: `46504e297900ae942265782d38942eaa0e5f6e0f0cc948e4492c96e62c002b0b`
- Artefakt: `dim-windows-x64` (MSI + `.exe`), iş **success**.

Türkçe ürün adı/açıklaması WiX'in varsayılan kod sayfasında (1252) hata
veriyordu (`LGHT0311`); `bundle.windows.wix.language = "tr-TR"` ile kod sayfası
1254 yapıldı.

**Sınırlamalar:** paket **imzasız** (ADR-0012). WebView2 kurulum modu Tauri
varsayılanında (`downloadBootstrapper`); çevrimdışı kurulum ve kullanıcı düzeyi
kaldırma denenmedi. ARM64 paket üretilemiyor → `docs/releases/F17/README.md`.

## Doğrulama

GitHub Actions `ci`: 6/6 iş yeşil (sözleşme, arayüz, rust ×2, masaüstü, python).
`desktop-build` imzasız `.app` artefaktı üretir (F16'da imzalanacak) ve Windows
x64 MSI'yi yukarıdaki kanıtla üretir.

## Açık

- Kurulum/kaldırma deneyimi ve WebView2 yolu F16'ya bağlı.
- Windows testleri ayrı `windows-check` işinde; her push'ta koşmaz.

## Worker'ın pakete gömülmesi (F02-T07)

- `scripts/build-worker.sh`: PyInstaller **onedir** (`dim-worker.spec`), `workers/document[all]`
  bağımlılıkları venv'e kurulur, çıktı `apps/desktop/src-tauri/resources/worker/` altına kopyalanır.
- `tauri.conf.json` `bundle.resources: ["resources/worker/**/*"]` — macOS'ta
  `Contents/Resources/resources/worker/` altına düşer.
- `bundled_worker_exe()` bu çift `resources` yolunu da arar (önceki hâlde yalnız
  `Contents/Resources/worker` aranıyordu → paketli uygulamada `E_WORKER_MISSING`).
- NPM paketleri yayınlanan sürümlere sabitlendi (`@tauri-apps/api|cli 2.12.4`,
  `plugin-dialog 2.8.3`); `pnpm-lock.yaml` manifest ile eşitlendi.

### Gerçek doğrulama (macOS arm64, imzasız paket)

`dim-aarch64-apple-darwin.zip` indirildi, `.app` açıldı, paket içindeki ikili
doğrudan NDJSON ile konuşturuldu:

```
$ printf '{"v":1,"id":"1","method":"ping","params":{}}\n' | .../worker/dim-worker
{"v":1,"id":"1","ok":true,"result":{"pong":true,"workerVersion":"0.1.0"}}

$ ... capabilities
pdf 1.24.14 · docx 1.1.2 · udf 1 · image 10.4.0 · ocr eng+osd · maxPages 2000
```

### Kalan

- **Tesseract ikilisi pakete gömülmedi**; OCR için sistemde kurulu olmalı, yoksa
  `E_OCR_MISSING`. Dil verisi (`tur`) de paketlenmiyor.
- Windows paketi (`windows-binary`) kurucu üretmiyor; F16 kapsamında.
- `.app` imzasız; Gatekeeper uyarısı F16'da çözülecek.
