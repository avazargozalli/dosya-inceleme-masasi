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

kurallarını denetler. Ölçüm (2026-11-06): **33 koşar, 5 yoksayılır** (38 test);
yoksayılanlar gerçek worker/tesseract ister ve `#[ignore = "gerçek worker …"]`
taşır — elle: `DIM_WORKER_PYTHON=… cargo test --lib <ad> -- --ignored --nocapture`.

Yeni test eklerken ya da bir testi bilinçli yoksayarken tabanı **tek satır**
güncelleyin (`scripts/desktop_test_gate.py`); kazara `#[ignore]` kapıyı kırmızı
yakar. Kapının kendisi `--self-test` ile 9 fixture üzerinde sınanır; CI'daki
`masaüstü kabuk (clippy + test)` işi ikisini de koşar.

## Neden ağır derleme lokalde değil

Bu projede Tauri/Rust paketlemesi GitHub Actions'ta yapılır. Gerekçe: tekrarlanabilir
ortam, disk tasarrufu ve imzalama sırlarının geliştirici makinesine kopyalanmaması.
Lokal ortamda `cargo check`/`cargo test` yeterlidir; `tauri build` gerekmez.

## Sık hatalar

- **`pnpm install --frozen-lockfile` hata veriyor:** `pnpm-lock.yaml` güncel değil;
  `pnpm install --lockfile-only` ile bilinçli güncelleyip commit edin.
- **CLI kimlik dosyası okunamıyor:** Tasarım gereği uygulama token/çerez okumaz.
  CLI kendi oturumunu yönetir (ADR-0005).
- **Worker `E_ENGINE_MISSING`:** `pip install -e "workers/document[pdf]"` eksik.
