# F14 — Çıktılar, paket, yedek

- **Sürüm etiketi:** `v0.13.0`
- **Durum:** Kısmi
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F14-T01 JSON dışa aktarma | ✅ | `export_json`, tam metadata, hash kaydı |
| F14-T02 CSV | ✅ | kronoloji + görevler, kaçışlı CSV |
| F14-T03 DOCX rapor | ✅ | `render_report`, tablolar + notlar |
| F14-T04 PDF rapor | ✅ | `render_report_pdf`, HTML kaçışlı, `Story` ile sayfalama |
| F14-T05 dışa aktarma sicili | ✅ | `exports` tablosu + denetim kaydı |
| F14-T06 devir paketi | 🚧 | `case.json` + `manifest.json` + `README.txt`; **şifreli değil** |
| F14-T07 yedek zamanlaması / bildirim | ⬜ | elle yedek var, otomatik yok |
| F14-T08 PDF çıktısında Türkçe font gömme | ⬜ | varsayılan font; gömülü font doğrulanmadı |

## Dürüst durum

Çıktılar kullanıcı seçtiği yola yazılır ve kasa içinde kayda alınır. Devir paketi
**açık metindir**; belge baytları otomatik pakete konmaz, maskeli kopyalar ayrıca
eklenir. Şifreli paket F16 kapsamındadır.
