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
| F06-T05 | ✅ | OCR: Tesseract (`tur+eng`), 300 dpi, dil paketi keşfi; yoksa açık hata |
| F06-T06 | ✅ | DOCX (python-docx): paragraf, tablo, üst/alt bilgi; UDF hâlâ açık |
| F06-T08 | ✅ | blok/satır kutuları, ortak koordinat uzayı, döndürme + kırpma kutusu, okuyucuda konum haritası |

## Doğrulama

`cargo test -p dim-storage parsing::` ve `cargo test -p dim-ingest worker::`
(python varsa gerçek worker; yoksa atlanır). CI: **bekliyor**.

## Bilinen sınırlar

- OCR isteğe bağlıdır: kullanıcı “OCR” demezse taranmış sayfa boş kalır ve
  “OCR gerekli” uyarısı yazılır. OCR kalitesi tarama kalitesine bağlıdır; sonuç
  özgün metnin yerine geçmez.
- DOCX'te sayfa düzeni doğrulanmaz; kaynak paragraf/tablo bazlıdır ve bu uyarı
  kayıtta taşınır.
- UDF (UYAP belge paketi) sayfa ayrımı doğrulanmamıştır; kaynak paragraf bazlıdır.
- Kaynak konumu **blok düzeyindedir**: kutu, alıntının düştüğü metin bloğudur;
  satır/kelime offsetleri saklanmadığı için daha ince vurgu yapılmaz. Kutu
  üretilmeyen belgelerde (DOCX/UDF/plain) konum haritası çizilmez.
- Sayfa başına 300 blok, blok başına 60 satır kutusu saklanır. Worker yanıtı
  700 KiB geometri bütçesini aşarsa önce satır kutuları, sonra blok kutuları
  düşer ve durum `warnings` + `partial` olarak raporlanır (sessiz kayıp yok).
