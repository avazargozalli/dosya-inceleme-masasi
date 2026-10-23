# F01 — Teknik doğrulamalar ve mimari kararlar

- **Sürüm etiketi:** `v0.0.1-spike` (hedef)
- **Durum:** Devam ediyor

## Spike matrisi

| İş | Deney | Durum | Kanıt |
|---|---|---|---|
| F01-T01 | Tauri/Rust/React üç hedefte derleme | 🚧 CI matrisi kuruldu, ilk koşu bekleniyor | `.github/workflows/ci.yml` |
| F01-T02 | PyInstaller onedir + PyMuPDF + Tesseract çevrimdışı | ⬜ | — |
| F01-T03 | SQLCipher/FTS5/WAL açık metin işaretçi taraması | ⬜ | — |
| F01-T04 | Streaming AEAD + truncation + kurtarma | 🚧 `crates/vault` prototipi | `cargo test -p dim-vault` |
| F01-T05 | PDF redaksiyon bağımsız doğrulama | ⬜ | — |
| F01-T06 | UDF/DOCX yapı ve ek envanteri | ⬜ | — |
| F01-T07 | Worker izolasyonu (kapsam dışı okuma/yazma/ağ) | ⬜ | — |
| F01-T08 | CLI araçsız/izinli profil kanıtı | ⬜ | — |
| F01-T09 | Yerel model RAM/ilk yanıt/iptal | ⬜ | — |
| F01-T10 | Worker protokolü bozuk frame/süreç ölümü | 🚧 | `workers/document/tests` |
| F01-T11 | Sürüm kilitleme ve ADR üretimi | ✅ | `Cargo.toml`, `package.json`, `docs/adr/` |
| F01-T12 | Başarısız deneme kaydı | ⬜ | bu dosya |

> Not: F01'in platform spike'ları gerçek cihaz/CI koşusu olmadan "geçti" sayılmaz.
> Bu dosya yalnız gerçekten çalıştırılan deneylerin sonucunu taşır.
