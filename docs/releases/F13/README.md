# F13 — Gerçek karartma ve paylaşım kopyası

- **Sürüm etiketi:** `v0.12.0` (hedef)
- **Durum:** Tamam (elle bölge + bağımsız doğrulama)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F13-T01 | ✅ | terim listesiyle gerçek çıkarma (`apply_redactions`, görüntü pikselleri dahil) |
| F13-T02 | ✅ | çıktı yeniden açılıp terimlerin kalmadığı doğrulanır; kalmazsa kayıt yok |
| F13-T03 | ✅ | yeni sürüm `parent_version_id` ile bağlanır; özgün sürüm değişmez |
| F13-T04 | ✅ | plan + bölgeler (sayfa, quad, terim hash'i) kaydı |
| F13-T05 | ✅ | sayfa önizlemesi üzerinde dikdörtgen seçimi; punto koordinatına çevrim; bölge doğrulaması |
| F13-T06 | ✅ | bağımsız pypdf okuyucusu + OCR (tesseract varsa) sızıntı kontrolü; `ocrVerified` dürüst bayrağı |

## Dürüst durum

İki mod vardır: **terimle** (PyMuPDF tüm sayfalarda arar) ve **bölge seçerek**
(sayfa önizlemesi üzerinde fareyle dikdörtgen; piksel → punto çevrimi `72/dpi`).
Her iki modda da `apply_redactions` metni ve görüntü piksellerini gerçekten
kaldırır, çıktı yeniden okunur; kalan içerik varsa dosya **hiç kaydedilmez**
(`E_REDACT_VERIFY`). Terim modunda ayrıca pypdf + OCR ile bağımsız doğrulama
yapılır. Bölge modunda OCR doğrulaması yapılmaz; bu, kayıtta dürüstçe belirtilir
(`ocrVerified=false`).

## Doğrulama

`python -m pytest workers/document/tests/test_redaction.py` — 5 test (metin katmanı
temizliği, boş terim reddi, eşleşmeyen terim, bağımsız pypdf doğrulaması, OCR bayrağı).
Tesseract yoksa `ocrVerified=false` döner; "görüntü doğrulandı" denmez. CI: yeşil.
