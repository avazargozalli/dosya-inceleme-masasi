# F11 — Kaynaklı AI inceleme araçları

- **Sürüm etiketi:** `v0.10.0`
- **Durum:** Kısmi
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F11-T01 aday kaydı | ✅ | `ai_candidates`, `valid/invalid/partial` |
| F11-T02 aday → inceleme kuyruğu | ✅ | `promote_candidate`, `review_items` `pending` |
| F11-T03 kapsamlılık raporu | ✅ | belge/sayfa/OCR/boş sayfa/ayrıştırılmamış sayıları |
| F11-T04 gerçek model çağrısı | ⬜ | sağlayıcı yok |
| F11-T05 şema doğrulama + alıntı eşleştirme | 🚧 | aday doğrulama durumu kaydediliyor; otomatik alıntı eşleştirme yok |
| F11-T06 kapsamlılık raporunun rapora gömülmesi | ⬜ | — |

## Dürüst durum

AI çıktısı **adaydır**; avukat kabul etmeden hukuki kayıt oluşmaz. Model çağrısı
yapılmadığı için gerçek çıkarım kalitesi ölçülmedi.
