# F07 — Arama ve belge okuyucu

- **Sürüm etiketi:** `v0.6.0`
- **Durum:** Devam ediyor (aşağıdaki açık işler)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F07-T01 | ✅ | FTS5 araması; dosya kapsamı zorunlu (`case_id` her sorguda) |
| F07-T02 | ✅ | tam/esnek/tırnaklı ifade ve E./K. numarası kipleri; doğrulama Rust'ta tek yerde |
| F07-T03 | ✅ | belge türü, tarih rolü, etiket ve OCR durumu filtreleri |
| F07-T04 | ✅ | sonuç alıntısı `raw_text`ten; vurgu metin düğümüyle (XSS yüzeyi yok) |
| F07-T05 | ✅ | keyset sayfalama, 200 sonuç sınırı, kapsam görünürlüğü |
| F07-T06 | ✅ | okuyucuda kademeli yakınlaştırma, görünüm döndürmesi ve PDF `/PageLabels` etiketi (fiziksel sayfadan ayrı) |
| F07-T07 | ✅ | DOCX başlık/paragraf yolu ve UDF paragraf sırası ayrı "Yapı" panelinde; sayfa türev olarak etiketlenir |

## F07-T02 — arama kipleri

`SearchQuery { text, mode }`; `mode`: `auto | exact | flexible | phrase | docket`.

- **auto**: tırnaklı metin → ifade, `2024/123 E.` → esas/karar no, diğerleri → esnek.
- **exact (Tam)**: Türkçe karakterler korunur (`ı ≠ i`, `ş ≠ s`); metin bitişik
  geçmeli (noktalama araya giremez); son kelime önek olabilir (`itiraz` →
  `itirazın`).
- **flexible (Esnek)**: Türkçe karakterler sadeleşir (`ı→i, ş→s, ğ→g, ü→u,
  ö→o, ç→c`); noktalama önemsiz; son kelime önek olabilir. Varsayılan kip.
- **phrase (İfade)**: kelimeler aynı sırada, araya noktalama girebilir; son
  kelime tam olmalı.
- **docket**: `2024/123 E.` biçimi; yıl + numara (baştaki sıfırlar yok sayılır)
  + varsa E./K. rolü. Rol verilmezse ikisi birlikte gelir.

**Neden tokenizer değişmedi:** `unicode61 remove_diacritics 2` `ş→s` gibi
eşlemeleri zaten yapar ama `ı` ile `i`yi ayırmaz ve tam arama ile esnek aramayı
aynı davranışa indirger. Bu yüzden aday kümesi FTS'te daraltılır, **kesin
eşleşme** `dim_search_match(text, needle, mode)` SQL skaler fonksiyonuyla
(Rust'ta tek yerde) doğrulanır. Sayım ve sayfa aynı doğrulamayı kullanır;
`total` ile gösterilen sonuç çelişmez.

- Esnek/tam adaylar sırasıyla `normalized_text` / `raw_text` sütunundan üretilir;
  `normalized_text` indekslenirken `dim_evidence::normalize` kullanıldığı için
  aday üretimi ile doğrulama aynı sadeleştirmeyi paylaşır.
- E./K. adayları yalnız **yıl** belirteciyle daraltılır: `2024/0123` baştaki
  sıfır yüzünden `123` belirteciyle eşleşmez; numara karşılaştırması
  kanonikleştirilmiş değerle yapılır.
- Kip/numara çözümlenemezse sessiz boş sonuç yerine `E_SEARCH_QUERY`.
- Arayüz: Arama ekranında kip seçimi, kip ipucu ("Otomatik: Esnek — …"),
  sonuç başlığında çözümlenmiş kip; kip değişince "önceki ayarlarla alındı"
  uyarısı. Vurgulama `resolvedQuery` ile yapılır (tırnaklar atılmış metin).

## F07-T03 — arama filtreleri

- `SearchFilters`: `documentTypes`, `dateRole`, `tags`, `ocrStatus`.
- **Belge türü**: `documents.document_type`; birden fazlaysa VEYA.
- **OCR durumu**: `pages.text_origin IN ('ocr','mixed')` ya da `= 'native'`.
  `ocr_used` diye ayrı bir kolon yok; sayfanın gerçek metin kaynağı kullanılır.
- **Etiket**: `document_tags` (şema v10, `0010_document_tags.sql`). Etiket
  belgeye bağlıdır, sürüme değil. Kanonik biçim `normalize_tag`: boşluk
  sadeleşir, küçük harfe iner, Türkçe `İ` küçültmesinin bıraktığı birleşik
  nokta atılır, 40 karakter sınırı. Birden fazla etiket **VE**: hepsi aynı
  belgede olmalı. Etiket ekleme idempotent; her değişiklik denetim kaydı yazar.
- **Tarih rolü**: belgeye **kaynaklı** olayın `date_role` değeri. Bağ
  `events → event_sources → source_anchors → document_versions` üzerinden
  kurulur; belgeye bağlanmamış bir olay belgeyi sonuca sokmaz.
- Bilinmeyen filtre değeri sessizce yok sayılmaz: `E_SEARCH_FILTER`.
- Arayüz: Arama ekranında filtre çubuğu, etkin filtre rozetleri ve "Filtreler
  değişti" uyarısı (eski sonuç yeni filtreyle gösterilmez). Etiketler Dosyalar
  ekranındaki belge satırından eklenir/kaldırılır.

## F07-T05 — keyset sayfalama

- `search_segments_page(case, query, filters, limit, cursor, include_total)`.
- Sıra `bm25` (küçük = daha iyi), eşitlikte `segment_id`. İmleç son sonucun
  `f64` **bitleri** + segment kimliğidir; ondalık metne çevrilmez, eşit
  skorlarda sayfa kaymaz. Bozuk/yabancı imleç `E_SEARCH_CURSOR`.
- `limit` 1–200 arasına kırpılır; bir fazla satır çekilerek `has_more` gerçek
  veriden üretilir. `total` yalnız ilk sayfada istenir.
- Her sonuç kapsam üst verisini taşır: `caseId`, `documentType`,
  `documentStatus`, `textOrigin`, `tags`. Böylece liste büyürken sonucun hangi
  dosyaya/belgeye ait olduğu görünür kalır.
- Arayüz: "Daha fazla sonuç" yalnız sunucunun imleciyle çağrılır; aynı segment
  iki kez gösterilmez.

## Doğrulama

- `cargo test -p dim-storage --lib` → **131** test (6 yeni: tam/esnek, ifade
  noktalaması, Türkçe ek öneki, tırnaklı auto, E./K. sıfır+rol, numarasız ret).
- `cargo test --workspace --all-targets` → storage 131, ingest 12 + E2E 1,
  vault 15, domain 13, jobs 5, evidence 7, policy 7, providers 5,
  cli-adapters 7.
- Web: `pnpm --filter @dim/desktop typecheck` + `vitest` **77** (4 yeni) +
  `vite build`.
- `cargo clippy --workspace --all-targets -- -D warnings` ve Tauri clippy
  temiz; iki manifestte rustfmt temiz.
- CI: `cf00acd` → `ci.yml` **40028597232**, **7/7 yeşil** (SBOM dahil).

## F07-T06 — okuyucuda yakınlaştırma, döndürme ve sayfa etiketi

- **Yakınlaştırma**: kademeli (%75, %100, %125, %150, %200) — `ZOOM_STEPS`,
  `clampZoom`, `nextZoom`. `−`/`+` düğmeleri uçlarda pasif, `%100` sıfırlar;
  metin puntosu ölçeklenir, seçim/offset hesabı değişmez (DOM ölçümü değil,
  metin düğümü kullanılır).
- **Döndürme**: belgenin kendi döndürmesi (`pages.rotation`) + panelin görünüm
  döndürmesi `rotateView` ile toplanır; 90° adımlarla sola/sağa ve sıfırlama
  var. Kayıtlı veri değişmez; konum haritası ve vurgu kutuları aynı toplamla
  döner (`mapRectForDisplay`). Belge değişince görünüm ayarı sıfırlanır.
- **Sayfa etiketi**: PDF `/PageLabels` ağacı **gerçekten tanımlıysa** PyMuPDF
  `page.get_label()` ile okunur (`iv`, `Ek-1`); ağaç yoksa `None`. PyMuPDF
  ağaç yokken fiziksel numarayı döndürdüğü için önce `doc.get_page_labels()`
  varlığına bakılır — böylece etiket uydurulmaz. Etiket kırpılır, boş etiket
  `NULL` olur, 60 karakterle sınırlanır.
- **Protokol/depolama**: worker `pageLabel` alanını yalnız tanımlıysa taşır;
  `ExtractedPage.page_label` `Option` + `#[serde(default)]` (eski worker
  yanıtı uyumlu). `pages.page_label` şema v1'de vardı; artık yazılıyor ve
  `PageRow.pageLabel` olarak okunuyor.
- **Arayüz**: "Fiziksel sayfa N" her zaman gösterilir; "basılı etiket “iv”"
  rozeti yalnız etiket tanımlıysa ve fiziksel numarayla aynı değilse çıkar.
  Sayfa düğmelerinin erişilebilir adı fiziksel sayfa + etikettir.

## F07-T07 — DOCX/UDF yapısal görünüm

- **Kaynak**: DOCX paragrafları python-docx stillerinden okunur. `Heading N` /
  `Başlık N` / `HeadingN` (yerelleşmiş `Başlık`) **gerçek** başlık sayılır ve
  yığın kurulur (`Kira › Ödeme`); başka stil adı başlık sayılmaz, bölüm adı
  uydurulmaz. Paragraf yolu `… · p/N` ile **fiziksel paragraf sırasını** taşır
  (türev sayfa numarasından bağımsız). Tablo satırları `Tablo N · satır M`,
  üstbilgi/altbilgi ve gövde dışı içerik (dipnot/sonnot/yorum) ayrı türlerdir.
- **UDF**: doğrulanmış başlık kaynağı yok; blok yolu yalnız `p/N`. Metin
  `_join_blocks` kuralıyla birebir aynı kalacak şekilde yeniden kurulur, böylece
  paragraf offsetleri kaymaz (eski tek blok davranışının yerini gerçek paragraf
  blokları alır).
- **Protokol/depolama**: blok alanları `path` + `kind` (`heading`, `paragraph`,
  `table`, `header`, `footer`, `annotation`). Worker yalnız dolu alanları taşır;
  Rust `ExtractedBlock`/`SegmentBlock` `Option` + `#[serde(default)]` (eski
  yanıt uyumlu), `bbox_list` JSON'unda saklanır, `PageRow.blocks` ile döner.
- **Arayüz**: yapısal belgede sayfa rozeti **"Türev sayfa N"** olur ve
  "yapısal belge · paragraf bazlı" rozeti eklenir. "Yapı" paneli gerçek
  yolları listeler (başlıklar kalın); tıklanınca ilgili paragrafa kaydırılır.
  Metin paragraf paragraf gösterilir; paragraflar arasında `\n\n` metin düğümü
  korunur, böylece `textContent` belge metniyle birebir kalır ve seçim/anchor
  offsetleri bozulmaz. Paragraflar metnin tamamını kaplamıyorsa tek parça
  gösterime dönülür (offset uyuşmazlığı riski alınmaz).
- Alıntı vurgusu paragraf içinde yerel aralığa çevrilir (`entrySlice`); PDF
  sayfalarında yapı üretilmez (`structureEntries` boş döner).

## Doğrulama (F07-T06/T07)

- `cargo test --workspace --all-targets` → **205** test: storage **133** (2 yeni:
  etiket aynen saklanır/boş etiket `NULL`; yapı bloğu yolu+türü geri okunur),
  ingest 12 + E2E 1, vault 15, domain 13, jobs 5, evidence 7, policy 7,
  providers 5, cli-adapters 7.
- Rust: `cargo clippy --workspace --all-targets -- -D warnings` ve Tauri clippy
  temiz; rustfmt iki manifestte temiz.
- Web: `tsc` + `vitest` **87** (9 yeni: zoom kademe/kırpma, döndürme toplamı,
  etiket ayrımı/yinelememe, yapı paragraf aralığı, PDF'te yapı üretmeme, aralık
  kırpma, paragraf içi vurgu) + `vite build` yeşil.
- Python: worker **71 passed, 1 skipped** (2 yeni DOCX başlık yolu testi + UDF
  `p/N` yol kontrolü; `/PageLabels` var/yok testleri).
- Gerçek worker IPC (elle): `/PageLabels` `Ek-1…Ek-3` PDF → `pageLabel`
  `["Ek-1","Ek-2","Ek-3"]`; başlıklı DOCX → bloklar
  `heading | Kira Sözleşmesi`, `paragraph | Kira Sözleşmesi · p/2`,
  `heading | Kira Sözleşmesi › Ödeme`, `paragraph | … · p/4`. Offsetler metinle
  birebir eşleşiyor.
- CI: `98d757a` → `ci.yml` **40034456417**, **7/7 yeşil** (SBOM dahil).

## Bilinen sınırlar

- Tauri penceresi bu ortamda açılmadı: yakınlaştırma/döndürme düğmeleri ve
  etiket rozeti typecheck + birim testleri + üretim derlemesiyle doğrulandı —
  gerçek tıklama ve görsel QA yok.
- Yakınlaştırma metin puntosunu ölçekler; konum haritası sabit boyutlu küçük
  önizleme olarak kalır (kutular haritada ayrıca büyümez).
- Basılı etiket yalnız PDF `/PageLabels` ağacından gelir; DOCX/UDF/görüntüde
  kaynak yok, arayüz fiziksel sayfa der. Ağaç tanımlı ama `get_label()` boş
  dönerse etiket gösterilmez.
- Döndürme yalnız gösterimdir; kayıtlı `pages.rotation` ve metin offsetleri
  değişmez, yeniden ayrıştırma gerekmez.
- Yapısal görünümde başlık yalnız DOCX yerleşik stillerinden gelir
  (`Heading N`/`Başlık N`); özel stilli veya numarayla biçimlendirilmiş başlıklar
  bölüm yolu üretmez — uydurulmaz. UDF'te başlık kaynağı yoktur, yol `p/N`.
- Yapı paneli ilk 200 kaydı listeler; fazlası için metin içinde gezinilir.
- Yapısal görünüm ve paragraf kaydırma gerçek Tauri penceresinde tıklanarak
  doğrulanmadı; typecheck + birim testleri + üretim derlemesi kanıtı var.

## Bilinen sınırlar (önceki işler)

- Tam arama **bitişik** metin ister (`kira, bedeli` ≠ `kira bedeli`); noktalama
  toleransı için İfade kipi kullanılmalı.
- Esnek/tam kiplerde son kelime önek olabilir; bu, "kelime başı" araması
  yapıldığı anlamına gelir (ör. `an` çok sonuç getirir). Kısa sorgularda
  sonuç sayısı bu yüzden yüksek olabilir.
- E./K. adayları yıl belirteciyle daraltılır; yıl geçmeyen ama numarası geçen
  bir metin (ör. yalnız `123 E.`) docket kipinde bulunmaz.
- Kip çözümü `auto` için sunucudadır; arayüz aynı kuralı önizler ama tek
  doğruluk kaynağı çekirdektir.
- Tarih **aralığı** filtresi yok; yalnız tarih rolü var (planda rol/aralık
  birlikte anılır, aralık açık iş).
- Filtre çubuğu belge türünde tek seçim sunar; çekirdek API çoklu türü
  destekler ve test eder.
- Tauri penceresi bu ortamda açılmadı; ekranlar typecheck, birim testleri ve
  üretim derlemesiyle doğrulandı — gerçek tıklama/görsel QA yok.
