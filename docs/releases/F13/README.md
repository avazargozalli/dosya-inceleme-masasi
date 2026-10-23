# F13 — Gerçek karartma ve paylaşım kopyası

- **Sürüm etiketi:** `v0.12.0` (hedef)
- **Durum:** Kısmi (elle bölge seçimi yok)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F13-T01 | ✅ | terim listesiyle gerçek çıkarma (`apply_redactions`, görüntü pikselleri dahil) |
| F13-T02 | ✅ | çıktı yeniden açılıp terimlerin kalmadığı doğrulanır; kalmazsa kayıt yok |
| F13-T03 | ✅ | yeni sürüm `parent_version_id` ile bağlanır; özgün sürüm değişmez |
| F13-T04 | ✅ | plan + bölgeler (sayfa, quad, terim hash'i) kaydı |
| F13-T05 | ⬜ | elle bölge seçimi (görüntü üzerinde) ve insan incelemesi |
| F13-T06 | ✅ | bağımsız pypdf okuyucusu + OCR (tesseract varsa) sızıntı kontrolü; `ocrVerified` dürüst bayrağı |

## Dürüst durum

Maskeleme **terim tabanlıdır**: kullanıcı metni girer, PyMuPDF tüm sayfalarda arar,
eşleşen bölgeleri gerçekten çıkarır. Görsel bölge seçimi ve ikinci bağımsız motor
henüz yok. Doğrulama başarısızsa çıktı **hiç kaydedilmez** (`E_REDACT_VERIFY`).

## Doğrulama

`python -m pytest workers/document/tests/test_redaction.py` — 5 test (metin katmanı
temizliği, boş terim reddi, eşleşmeyen terim, bağımsız pypdf doğrulaması, OCR bayrağı).
Tesseract yoksa `ocrVerified=false` döner; "görüntü doğrulandı" denmez. CI: yeşil.
