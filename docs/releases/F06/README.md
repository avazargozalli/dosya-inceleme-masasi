# F06 — Metin, OCR, UDF ve kaynak konumları

- **Sürüm etiketi:** `v0.5.0`
- **Durum:** T01–T11 kapalı; T12 gerçek veri bekliyor
- **Tarih:** 2026-10-24

| İş | Durum | Kanıt |
|---|---|---|
| F06-T01 | ✅ | sürümlü `parse_run`, sayfa ve metin segmentleri |
| F06-T02 | ✅ | PyMuPDF metin çıkarma (worker), `text_origin` |
| F06-T03 | ✅ | worker istemcisi: 1 MiB çerçeve, parçalı yanıt (`part`/`parts`) birleştirme, zaman aşımı, süreç ölümü |
| F06-T04 | ✅ | ayrıştırma durumu (`succeeded`/`partial`), uyarılar |
| F06-T05 | ✅ | OCR: Tesseract (`tur+eng` — tek kaynak `DEFAULT_OCR_LANG`), 300 dpi; dil keşfi + eksik pakette **adıyla** `E_OCR_LANG_MISSING` |
| F06-T06 | ✅ | DOCX (python-docx): paragraf, tablo, üst/alt bilgi; UDF ayrı işte (`c1c9603`) |
| F06-T07 | ✅ | çok kareli TIFF + piksel bütçesi (mevcut); **düz metin kodlaması**: BOM + katı UTF-8 + Türkçe tek baytlık otomatik seçim, açık seçim |
| F06-T08 | ✅ | blok/satır kutuları, ortak koordinat uzayı, döndürme + kırpma kutusu, okuyucuda konum haritası |

## Doğrulama

`cargo test -p dim-storage parsing::` ve `cargo test -p dim-ingest worker::`
(python varsa gerçek worker; yoksa atlanır). 2026-10-24: Python **90 passed,
1 skipped** (11 yeni kodlama testi), Rust `dim-ingest` gerçek IPC kodlama turu
yeşil. CI: bu turda doğrulanır (commit sonrası `ci.yml`).

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
- Metin yanıtı 900 KiB'ı aşarsa sayfalar birden çok çerçevede (`part`/`parts`)
  gönderilir; istemci sırayı doğrulayıp birleştirir. **Tek bir sayfa** çerçeve
  sınırını aşıyorsa `E_FRAME_TOO_LARGE` döner: metin parçalanıp offsetleri
  bozulmaz, iş açık hatayla durur.

## T02 / T05 / T06 (ek)

- **T02 DOCX gizli katman envanteri**: `footnotes.xml`, `endnotes.xml`,
  `comments.xml`, `w:ins`/`w:del` sayılır ve metne ayrı blok olarak eklenir;
  python-docx'in gövdeye katmadığı içerik kaybolmaz. Test:
  `tests/test_docx_annotations.py` (2).
- **T05 karma PDF**: metin katmanı olan sayfa yeniden OCR edilmez; yalnız
  metinsiz sayfa OCR'a gider. Çağrı sayısı ölçülerek kanıtlandı.
- **T05 OCR dili (2026-10-29, ölçülen kusur):** dil **yolun birinde** `eng`
  kalıyordu: tek belge ayrıştırması `tur+eng` gönderiyordu ama çekirdek
  varsayılan (`run_parse` fallback), eski iş yeniden denemesi ve kasa okuması
  (`job_parse_options`) `eng` yazıyordu. Ölçüm (`scripts/ocr_lang_bench.py`,
  `fixtures/synthetic/turkish-scan.pdf`, 2 sayfa, Tesseract 5.5.3):
  `eng` → karakter doğruluğu **%82,78**, Türkçe harf **0/79**;
  `tur` / `tur+eng` → **%100**, **79/79**. Düzeltme: tek kaynak
  `dim_storage::queue::DEFAULT_OCR_LANG = "tur+eng"`; Rust, worker
  (`dim_worker.engines.DEFAULT_OCR_LANG`) ve arayüz
  (`apps/desktop/src/lib/ocr.ts`) **testle** birbirine bağlı.
- **T05 dil paketi kapısı:** eksik dilde eskiden `E_OCR_FAILED` +
  "dil paketi eksik olabilir" **tahmini** dönüyordu (her OCR arızası aynı
  cümleye düşüyordu). Artık `_ensure_ocr_languages()` çağrıdan **önce**
  kurulu listeyi (`tesseract --list-langs`, süreç başına bir kez) okur ve
  `E_OCR_LANG_MISSING` ile **eksik kodu + kurulu dilleri** yazar. Liste hiç
  alınamıyorsa suçlama yapılmaz; **sessiz `eng`e düşme yok** (düşmek tam da
  ölçülen diyakritik kaybını üretirdi).
- **T06 ayar özeti**: `settings_hash` artık MIME + OCR bayrağı + dil + motor
  sürümü + `tessdataHash` taşır. Tesseract 5 `--print-tessdata-dir`, Tesseract 4
  `--list-langs` başlığı (tırnaklı yol) ile bulunur; bulunamazsa `null`.
  `capabilities().ocr.tessdataHash` 64 hex veya `null` — uydurma sürüm yok.

## F06-T07 (ek) — düz metin kodlaması

Türkçe hukuk metinleri sık sık Windows-1254 ile üretilir; katı UTF-8 çözümü
`E_TEXT_ENCODING` ile duruyordu. Artık düz metin (`text/*`) şu sırayla çözülür:

1. **BOM**: UTF-32 / UTF-16 / UTF-8 BOM'u varsa o kodlama kullanılır, BOM metne
   girmez.
2. **Katı UTF-8**: geçerliyse hiç uyarı üretilmeden kullanılır.
3. **Tek baytlık adaylar** (`windows-1254`, `iso-8859-9`, `windows-1252`,
   `iso-8859-1`): her aday çözülür ve **bozulma puanı** hesaplanır (UTF-8'in
   Latin-1 gibi okunmasından kalan `Ã Â Ä Å` çiftleri + kontrol karakterleri).
   En düşük puanlı aday seçilir; puan > 0 ise uyarı "bozuk karakter olasılığı
   var" der.
4. Hiçbir aday yoksa ya da içerik ikili görünüyorsa (NUL / yoğun kontrol baytı)
   `E_TEXT_ENCODING` döner — bozuk metin sessizce kaydedilmez.

Kullanılan kodlama **her zaman** `warnings` içinde yazılır (`metin kodlaması
otomatik belirlendi: …`) ve `parse_runs.engine` sürümü `plain/2`'dir; davranış
değiştiği için eski `plain/1` kayıtlarıyla karışmaz.

Açık seçim: arayüzde "TXT kodlaması" (Otomatik / UTF-8 / Windows-1254 /
ISO-8859-9) seçilir; değer Tauri `document_parse` → worker `encoding`
parametresine gider ve yalnız düz metinde ayar özetine (`txt-encoding=…`)
eklenir. Yanlış kodlama açıkça seçilirse `E_TEXT_ENCODING` döner, metin
kaydedilmez.

**Doğrulama**: `tests/test_text_encoding.py` 11 test (cp1254 Türkçe birebir,
UTF-8/UTF-16 BOM, ASCII gürültüsüz, açık UTF-8 reddi, bilinmeyen kodlama adı,
ikili içerik reddi, protokol varsayılanı ve açık parametre); Rust
`WorkerClient` ile gerçek IPC turu (`text_encoding_auto_detection_roundtrip`)
Türkçe baytları birebir çözer ve yanlış kodlamada `E_TEXT_ENCODING` alır.

**Sınırlama**: BOM'suz UTF-16 sezgisel olarak çözülmez (açıkça seçilmeli);
otomatik seçim istatistiksel bir sezgidir, hukuki metinde kesinlik iddiası
yoktur — yanlış görünürse kodlama elle seçilir.

## Kalan

- **T12**: 200 elle işaretli sayfada OCR/kaynak doğruluğu ölçümü — gerçek veri
  ve insan işaretlemesi gerektirir; CI'da üretilemez.
- **T06**: `tur` dil paketi bu ortamda ve CI'da yok; hash yalnız `eng` için
  doğrulanabilir, `tur` uyarı olarak kalır.
