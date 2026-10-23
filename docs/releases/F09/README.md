# F09 — AI sağlayıcı çerçevesi, yerel model ve API

- **Sürüm etiketi:** `v0.8.0` (hedef)
- **Durum:** İskelet — gerçek sağlayıcı bağlantısı yok
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F09-T01 | ✅ | `crates/providers`: tek `Provider` sözleşmesi, `OffProvider` |
| F09-T02 | ✅ | yetenek kapısı (`structured_output`, `context_limit`) |
| F09-T03 | 🚧 | `ai_runs`/`transfer_*` tabloları hazır; kayıt akışı F09 devamında |
| F09-T04 | ⬜ | yerel model indirme/doğrulama |
| F09-T05 | ⬜ | doğrudan API profili |

## Dürüst durum

Bu fazda **hiçbir sağlayıcıya istek gönderilmez**. `OffProvider` her isteği
`E_AI_OFF` ile reddeder. Yerel model ve API seçenekleri arayüzde bağlantı
kartı olarak görünmez; "çalışıyor" izlenimi verilmez.

## Doğrulama

`cargo test -p dim-providers` — 5 test: kapalı sağlayıcı, yetenek kapısı,
bağlam sınırı, bayt sayımı, kararlı hata kodları. CI: **bekliyor**.
