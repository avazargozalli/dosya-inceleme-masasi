# F06 — Metin, OCR, UDF ve kaynak konumları

- **Sürüm etiketi:** `v0.5.0`
- **Durum:** T01–T11 kapalı; T12 gerçek veri bekliyor
- **Tarih:** 2026-10-24

| İş | Durum | Kanıt |
|---|---|---|
| F06-T01 | ✅ | sürümlü `parse_run`, sayfa ve metin segmentleri |
| F06-T02 | ✅ | PyMuPDF metin çıkarma (worker), `text_origin`. **F06-T02b (2026-11-14):** koşu ve segmentler dosya sınırını aşmaz — `start_parse_run` sürümün dosyasını, `store_pages` koşunun dosyasını doğrular; ölçülen sızıntı + 3 mutasyon kapısı (aşağıdaki bölüm) |
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

## F06-T02b — koşu ve segmentler dosya sınırını aşmaz (2026-11-14)

**Ölçülen sızıntı (asıl kusur).** `store_pages(parse_run_id, case_id, pages)`
`text_segments.case_id` sütununu **çağıranın** verdiği dosyayla yazıyordu ve
arama tam olarak bu sütunla kapsam daraltıyor (`s.case_id = ?`). Yanlış
`case_id` kabul edilirse bir dosyanın metni **başka dosyanın** aramasında
görünür, kendi dosyasında görünmez — INV-05 ("başka dosyanın verisi örtük olarak
eklenmez") ihlali.

**Düzeltme öncesi gerçek koşu (geçici sonda, silindi):** A dosyasının sürümü
için `start_parse_run(..., case_id=B)` → `Ok`; `store_pages(run, B, [...])` →
`Ok(1)`; arama: **A=0 sonuç, B=1 sonuç**. Yani metin yalnız yanlış dosyada
bulunuyordu.

**Yazılan (çekirdek).**

- `start_parse_run` sürümün `case_id`'sini okur, çağıranınkiyle karşılaştırır;
  tutmazsa `E_NOT_FOUND` — koşu satırı yazılmaz.
- `store_pages` koşunun `case_id`'sini okur; tutmazsa `E_NOT_FOUND` — sayfa ve
  segment yazılmaz (transaction açılmadan önce).
- İki kapı da **tek kaynak** sorgudur; kapsam arayüzden/çağırandan bağımsızdır.

**Testler.** `parsing::tests::parse_run_is_opened_only_for_its_own_case`,
`parsing::tests::store_pages_refuses_a_foreign_case_and_leaks_nothing` (+2).
İkincisi reddedilen yazmanın **hiçbir iz bırakmadığını** (0 sayfa, 0 segment) ve
metnin iki dosyada da bulunmadığını; doğru dosyayla yazınca yalnız kendi
dosyasında bulunduğunu **ölçer**.

**Kapı kanıtı.** F05-T03b bölümündeki 5/5 mutasyon turunun parçası: M3 (koşu
kapısı düşer) ve M4 (sayfa kapısı düşer) kırmızı; taban yeşil, geri yazım
sha256 birebir.

**Dürüst sınırlar.** (a) Bugün ürün yolu (`commit_parse`) `case_id`'yi sürüm
satırından okur → canlı sızıntı yok; kapı sınıfı kapatır. (b) `text_segments.
case_id` ile `pages.parse_run_id` arasında DB düzeyinde bileşik kısıt yok;
savunma çekirdek sorgusundadır. (c) Doğrudan SQL yazan bir yol (bugün üründe
yok) bu kapıyı atlar. (d) Ayrıştırma koşusu komut olarak koşulmadı; davranış
gerçek SQLite ile storage düzeyinde ölçüldü.

**CI (ölçüldü).** Kod `0465eb6` → run `43616435408` **10/10 YEŞİL**;
`parsing::tests` yeni iki test dâhil `dim-storage` **607** iki OS'ta birebir
(macos-15 231,73 s / ubuntu-latest 580,79 s). Tauri komutu + gerçek pencere
**insan bekliyor**.

## F06-T02c — koşu/sayfa kapısı artık şemada (2026-11-14)

T02b'nin dürüst sınırları (b) ve (c) kapandı: kapsam kuralı **şema v31**
tetikleyicileriyle veritabanında durur (ayrıntı: F05-T03c bölümü).

- `parse_runs`: `case_id`, `document_version_id`'nin dosyasıyla aynı olmalı.
- `text_segments`: `case_id`, `parse_run_id`'nin dosyasıyla aynı olmalı **ve**
  `page_id` kendi koşusunun sayfası olmalı (yanlış sayfa, alıntı geometrisini
  yanlış belgeye bağlardı).
- Reddedilen yazma `E_DB_SCOPE` verir; işlem içinde durur, kısmi sayfa/segment
  kalmaz. Doğrudan SQL yazan bir yol artık kapıyı **atlayamaz**.

**Testler.** `scope_guard::tests::a_parse_run_cannot_be_written_into_another_case`,
`…a_segment_cannot_be_written_into_another_case_or_onto_a_foreign_page` ve
şemadan türetilen kapı testi (`every_case_scoped_table_has_guard_triggers`) →
`dim-storage --lib` **619**. **Maliyet (ölçüldü, debug):** segment yazımında
**+5,5 µs/satır (%5)** (`tests/scope_guard_cost.rs`). Mutasyon kanıtı: F05-T03c
9/9 (M2 koşu, M3 segment/sayfa tetikleyicisi düşünce ilgili testler kırmızı).

**Dürüst sınırlar.** Tetikleyiciler yalnız yeni yazmayı engeller; v31 öncesi
yazılmış çapraz satırlar sayılır (`scope_violations()`), düzeltilmez.

**CI (ölçüldü).** Kod `20d7525` → run `43626252354` **10/10 YEŞİL**;
`dim-storage` **619** iki OS'ta birebir (macos-15 266,87 s / ubuntu-latest
301,26 s), `arayüz` 1428 test.
