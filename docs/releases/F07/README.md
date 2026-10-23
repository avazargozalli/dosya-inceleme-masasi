# F07 — Arama ve belge okuyucu

- **Sürüm etiketi:** `v0.6.0`
- **Durum:** Devam ediyor (aşağıdaki açık işler)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F07-T01 | ✅ | FTS5 araması, dosya kapsamı zorunlu (`case_id`) |
| F07-T02 | ✅ | sonuçta sayfa numarası + ham metin; HTML çalıştırılmaz |
| F07-T03 | ✅ | vurgulama metin düğümüyle yapılır (XSS yüzeyi yok) |
| F07-T04 | 🚧 | yan yana okuyucu ve sayfaya atlama F07 devamında |

## Doğrulama

`cargo test -p dim-storage parsing::tests::search_never_crosses_case_boundary`.
CI: **bekliyor**.

## Bilinen sınırlar

- Okuyucu görünümü henüz yok; sonuç metni gösterilir.
- FTS tokenizer `unicode61 remove_diacritics 2`; Türkçe `ı/i` için özel
  tokenizer F07 devamında değerlendirilecek.
