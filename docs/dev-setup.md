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
python -m pytest workers/document/tests -q
python tests/smoke/worker_smoke.py
```

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
