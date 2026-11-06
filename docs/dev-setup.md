# Geliştirme ortamı

## Gereksinimler

| Araç | Sürüm | Not |
|---|---|---|
| Node | 22.x (CI ile aynı) | `pnpm` 10.15 |
| Rust | 1.83.0 | `rust-toolchain.toml` otomatik seçer |
| Python | 3.11+ | worker testleri |
| Tauri önkoşulları | platforma özgü | macOS: Xcode CLT; Windows: WebView2 + MSVC |

## Kurulum

```bash
pnpm install --frozen-lockfile
python3 -m venv .venv && . .venv/bin/activate
pip install -e "workers/document[pdf,dev]"
```

## Günlük döngü

```bash
pnpm typecheck          # TS
pnpm test               # vitest + node:test
cargo test --workspace  # çekirdek
python3 scripts/desktop_test_gate.py   # masaüstü kabuk (Tauri) testleri + sayı kapısı
python -m pytest workers/document/tests -q
python tests/smoke/worker_smoke.py
```

## Masaüstü kabuk testleri (kapı)

`apps/desktop/src-tauri` ayrı bir cargo çalışma alanıdır; kök `cargo test
--workspace` onu **kapsamaz**. Bu yüzden testleri `scripts/desktop_test_gate.py`
koşar: `cargo test --lib` çıktısını okur, sonra

1. tek `running N tests` + tek `test result` özeti (derleme kırıldıysa yeşil görünmez),
2. `0 failed` ve `0 filtered out` (ad filtresiyle daraltılmış koşu tam sayılmaz),
3. kaynak sayımı: `#[test]` = koşan + yoksayılan,
4. **kayıtlı taban** (`BASELINE_PASSED` / `BASELINE_IGNORED`) ile birebir

kurallarını denetler. Ölçüm (2026-11-06): **34 koşar, 5 yoksayılır** (39 test).

Yoksayılan beş test gerçek worker/tesseract ister (`#[ignore = "gerçek worker …"]`)
ve **2026-11-06'dan beri CI'da gerçekten koşuyor**: `desktop` işi depo venv'ini
(`pymupdf` + `pytesseract`) ve tesseract'ı kurar, sonra

```bash
python3 scripts/desktop_test_gate.py --ignored
```

`--ignored` modu `DIM_REQUIRE_WORKER=1` ile koşar: motor bulunamazsa test
**düşer** (sessiz atlama yok — `E_SKIPPED`), `0 ignored`, `--ignored` düşmediyse
`33 filtered`, beklenen beş test adı (`E_CENSUS`) ve taban 5
(`IGNORED_BASELINE`) denetlenir. Ölçülen tuzak: motoru olmayan bir python'la
(`DIM_WORKER_PYTHON=/bin/true`) koşu cargo'ya *"5 passed; 0 failed … finished in
0.00s"* dedirtiyor — kapı bunu kırmızı yakalar.

Yerelde aynı komut venv'i **kendiliğinden** bulur; `DIM_WORKER_PYTHON` vermek
gerekmez. Yorumlayıcı sırası **tek yerde**: `DIM_WORKER_PYTHON` →
`workers/document/.venv` → PATH (`dim_ingest::worker::dev_python_for_worker`).
Ürün (`spawn_worker`) da bu sırayı kullanır — ayrı bir sıra yazmak "test venv'i
gördü, ürün görmedi" kusurunu üretir (2026-11-06'da ölçüldü: beş test ilk kez
koşuldu, **0 geçti / 5 düştü**, dördü `E_ENGINE_MISSING`).

Yeni test eklerken ya da bir testi bilinçli yoksayarken tabanı **tek satır**
güncelleyin (`scripts/desktop_test_gate.py`); kazara `#[ignore]` kapıyı kırmızı
yakar. Kapının kendisi `--self-test` (9 fixture) ve `--ignored --self-test`
(7 fixture) ile sınanır; CI'daki `masaüstü kabuk (clippy + test)` işi üçünü de
(normal koşu, öz test, gerçek worker koşusu) çalıştırır.

## Neden ağır derleme lokalde değil

Bu projede Tauri/Rust paketlemesi GitHub Actions'ta yapılır. Gerekçe: tekrarlanabilir
ortam, disk tasarrufu ve imzalama sırlarının geliştirici makinesine kopyalanmaması.
Lokal ortamda `cargo check`/`cargo test` yeterlidir; `tauri build` gerekmez.

### Windows MSI kurulum tatbikatı

`desktop-build` MSI'yi üretir; **kurulduğunu** ayrı bir iş ölçer (sessiz kurulum →
kurulan ağaçta uygulama + belge worker'ı → worker'ı NDJSON ile konuşturma → kaldırma).
Yeniden derleme beklemeden, herhangi bir koşunun MSI'sine karşı koşulabilir:

```
gh workflow run msi-install-drill.yml -f artifact_run_id=<desktop-build koşu kimliği>
```

Koşu, MSI'yi `dim-windows-x64` artefaktından indirir; bu yüzden o koşuda
`windows-binary (x86_64…)` işi yeşil olmalı. Tatbikat **worker'sız** bir MSI'de
kırmızı yanar (`kurulan pakette belge worker yok`) — 2026-11-06'da gerçekten
böyle bir paket vardı, düzeltildi: `docs/releases/F02/README.md`.

## Sık hatalar

- **`pnpm install --frozen-lockfile` hata veriyor:** `pnpm-lock.yaml` güncel değil;
  `pnpm install --lockfile-only` ile bilinçli güncelleyip commit edin.
- **CLI kimlik dosyası okunamıyor:** Tasarım gereği uygulama token/çerez okumaz.
  CLI kendi oturumunu yönetir (ADR-0005).
- **Worker `E_ENGINE_MISSING`:** `workers/document/.venv` eksik ya da motoru yok
  (`pip install -e "workers/document[all]"`). Uygulama venv'i kendiliğinden
  bulur (`DIM_WORKER_PYTHON` → venv → PATH); yine alıyorsan `DIM_WORKER_PYTHON`
  açıkça venv'e işaret etsin.
