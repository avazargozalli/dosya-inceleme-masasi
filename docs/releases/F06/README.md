# F06 — Metin, OCR, UDF ve kaynak konumları

- **Sürüm etiketi:** `v0.5.0`
- **Durum:** Devam ediyor (aşağıdaki açık işler)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F06-T01 | ✅ | sürümlü `parse_run`, sayfa ve metin segmentleri |
| F06-T02 | ✅ | PyMuPDF metin çıkarma (worker), `text_origin` |
| F06-T03 | ✅ | worker istemcisi: 1 MiB çerçeve, zaman aşımı, süreç ölümü |
| F06-T04 | ✅ | ayrıştırma durumu (`succeeded`/`partial`), uyarılar |
| F06-T05 | 🚧 | OCR (Tesseract) henüz bağlanmadı — sayfa uyarısı üretilir |
| F06-T06 | 🚧 | UDF/DOCX yapı çıkarımı F06 devamında |

## Doğrulama

`cargo test -p dim-storage parsing::` ve `cargo test -p dim-ingest worker::`
(python varsa gerçek worker; yoksa atlanır). CI: **bekliyor**.

## Bilinen sınırlar

- OCR yok: taranmış PDF'te metin boş kalır ve "OCR gerekli" uyarısı yazılır.
- Kaynak konumu (bbox/quad) üretilmiyor; segment düzeyi var, alıntı anchor'ı F07/F11.
