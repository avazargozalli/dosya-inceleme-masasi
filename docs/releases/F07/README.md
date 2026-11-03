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
| F07-T08 | ✅ | yan yana iki panel kendi sürümünü, sayfasını, yakınlaştırmasını ve seçimini korur; sürüm başına görünüm belleği |
| F07-T09 | ✅ | metin seçimi doğrulanmış anchor'a bağlı kalıcı işaret ve kaynaklı not üretir; bölge işareti sayfa koordinatında saklanır |
| F07-T12 | ✅ | kısmi ayrıştırma rozeti; kopmuş/eski kaynak bağlantısı görünür, güncel sürümde yeniden bağlama önerisi kullanıcı onayıyla uygulanır (arayüz davranış testi: **10 test**, yeniden bağlama hedef bağa bağlandı — 2026-11-03) |
| F07-T10 | ✅ | not taslağı kasada otomatik kaydedilir; geri/ileri al gerçek revizyonları kullanır; `row_version` çakışmasında fark gösterilip seçim istenir |
| F07-T11 | ✅ | sayfa önizlemesi kasa anahtarıyla şifreli disk önbelleğinde; bellekteki çözülmüş sayfalar kilitta temizlenir |

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

## F07-T08 — yan yana iki belge; panel başına kaynak ve seçim

- **Panel durumu**: `ReaderView { page, zoom, rotation, selection }`. Her panel
  kendi görünümünü tutar; bir panelde belge, sayfa, yakınlaştırma veya döndürme
  değişimi diğerini etkilemez (ayrı durum, ayrı bellek).
- **Sürüm belleği**: panel açtığı her belge sürümünün son görünümünü hatırlar
  (`rememberView` / `recallView`, en çok 8 sürüm; sınır aşılınca en eski düşer).
  Belge değiştirip geri dönüldüğünde sayfa, yakınlaştırma, döndürme ve metin
  seçimi geri yüklenir. Bellek **yalnız panel oturumundadır**; diske veya
  `localStorage`a yazılmaz (seçim metni kasa dışına çıkmaz).
- **Seçim doğrulaması**: geri yüklenen seçim sayfa metnine karşı doğrulanır
  (`validSelection`). Alıntı kaydedildiği offset'te birebir durmuyorsa (yeniden
  ayrıştırma metni kaydırmış olabilir) seçim geri yüklenmez, arayüz nedenini
  yazar; yanlış alıntı işaretlenmez.
- **Hedef ile hatıra çatışması**: arama sonucu/bağ hedefi yeni tıklandığında
  ilgili sayfa açılır; sürüm hatırlanan görünümle geri yüklendiyse hedef
  zıplaması yapılmaz, kullanıcının bıraktığı yer korunur.
- **Boş panel**: kendiliğinden bir belge açar ama **diğer panelde açık sürümü
  seçmez** (`pickInitialVersion`); ikinci belge yoksa tek belgeye döner.
- **Klavye**: odak hangi paneldeyse `Alt`+`←`/`→`, `Alt`+`PageUp`/`PageDown`,
  `Alt`+`Home`/`End` o panelin sayfasını değiştirir (`pageForKey`). Düz ok ve
  PageDown/PageUp tuşları metin alanının kendi kaydırmasında kalır. Odaklanan
  panel "etkin panel" rozeti ve vurgulu çerçeveyle işaretlenir; diğer panelin
  sayfası ve seçimi değişmez.

## F07-T09 — metin/görsel seçiminden kaynaklı not ve kalıcı işaret

- **Şema v12** (`0012_source_marks.sql`): `source_marks`. `mark_kind` `text` |
  `region`. Metin işareti **doğrulanmış** `source_anchors` kaydına bağlanır;
  aynı anchor'da tek aktif işaret vardır (kısmi tekil indeks), silme yumuşaktır
  (`deleted_at`, satır kalır).
- **Atomik yazım**: `mark_from_selection` ve `note_from_selection` anchor ile
  işaret/notu **tek işlemde** yazar (`write_atomically`; çağıran açık işlem
  içindeyse ona katılır). Alıntı segmentte birebir bulunamazsa hiçbir kayıt
  oluşmaz — ne anchor ne işaret (test: `mark_is_not_created_when_quote_does_not_match`).
- **Notun kaynağı**: notun `source_anchor_ids` listesi çağıranın gönderdiği
  kimliklerden değil, oluşan anchor'dan gelir; doğrulanmamış kaynak kimliği
  notta kalmaz.
- **Bölge işareti**: `create_region_mark` sayfa geometrisine karşı doğrular —
  sayfa aynı sürümün ayrıştırmasına ait olmalı (`E_NOT_FOUND`), dikdörtgen sayfa
  içinde ve sonlu olmalı (`E_DB_SETTING`), 1–20 kutu. Geometri yoksa işaret
  oluşmaz; konum uydurulmaz.
- **Arayüz (okuyucu)**:
  - Seçim çubuğu: "Seçimi kaynağa bağla" · "İşaretle" · "Seçimden not".
  - "İşaretle": seçim çekirdekte doğrulanır, sayfada sarı zeminle kalıcı işaret
    çizilir; "kalıcı işaret" listesi sayfa etiketi, sayfaya git ve sil verir.
  - "Seçimden not": başlık + gövde formu; "Notu kaydet" `note_create_from_selection`
    çağırır, not Notes ekranında kaynak kimliğiyle listelenir.
  - **Görsel seçim**: sayfa görüntüsü (F07-T11) üzerinde sürüklenen bölge
    görünen uzayda 0–1 oranıyla tutulur, kayda geçerken `rectFromDisplay` ile
    **sayfa koordinat uzayına** çevrilir. İşaretler görüntüde ve konum
    haritasında kesikli çerçeveyle gösterilir; yakınlaştırma/döndürme koordinatı
    kaydırmaz.
  - Metin işaretleri `textSegments` ile parçalanır: parçalar birleşince sayfa
    metni birebir elde edilir (seçim offsetleri kaymaz); arama/bağ hedefi vurgusu
    işaretten önceliklidir.
- **Sınırlama**: yapısal (DOCX/UDF) görünümde işaretler metin üzerinde çizilmez,
  listeden sayfaya gidilir; bölge seçimi yalnız sayfa görüntüsü açıkken yapılır.
  Tauri penceresi bu ortamda açılmadı → gerçek sürükleme/tıklama ve görsel QA yok.

## F07-T12 — kısmi ayrıştırma ve kopmuş/eski kaynak bağlantısı

- **Kısmi ayrıştırma rozeti**: `parseRun.status !== 'succeeded'` olan belgede
  "kısmi ayrıştırma · <durum>" rozeti zaten gösteriliyordu; artık kaynak
  bağlantısıyla birlikte aynı panelde okunur.
- **Eski sürüm bağlantısı**: `locate_anchor` artık `isCurrentVersion`,
  `currentVersionId`, `versionNo`, `currentVersionNo` alanlarını döndürür.
  Anchor belgenin güncel sürümünde değilse okuyucu "eski sürüm v1 · güncel v2"
  rozetini gösterir; **eski konum korunur** (anchor kendiliğinden taşınmaz).
- **Yeniden bağlama önerisi** (`rebind_suggestion`): güncel sürümün en son
  başarılı/kısmi ayrıştırmasında alıntı birebir aranır. Durumlar:
  `current` (zaten güncel), `exact` (tek eşleşme), `ambiguous` (birden fazla →
  "belirsiz", adaylar sayfa numarasıyla listelenir), `unresolved` (yok →
  "çözümlenemedi", eski konum korunur). Öneri **kullanıcı onayı olmadan
  uygulanmaz**.
- **Bağlama** (`rebind_anchor`): alıntı verilen segment/offset'te birebir
  durmuyorsa `E_QUOTE_NOT_FOUND` döner ve bağ değişmez. `verify_quote` başlangıç
  ipucunu yalnız öneri olarak kullandığı için konum ayrıca karakter karakter
  doğrulanır. Başarılı bağ `anchor_status='rebound'` yazar, denetim kaydı
  bırakır ve anchor dosya listesinde görünür kalır.
- **Arayüz**: kopmuş alıntı veya eski sürüm varsa "Güncel sürümde ara" düğmesi;
  sonuç panelinde durum metni + aday sayfaları; seçilen aday bağlanınca okuyucu
  **yeni sürümü ve yeni sayfayı** açar, üst ekran kaynak listesini tazeler.
- **Sınırlama**: yeniden bağlama yalnız metin alıntısı içindir; görüntü/OCR
  sayfasında alıntı yoksa öneri "çözümlenemedi" kalır. Otomatik toplu yeniden
  bağlama yoktur (kullanıcı eylemi gerekir).

## F07-T10 — not taslağı, geri/ileri al ve sürüm çakışması

- **Şema v11** (`0011_note_history.sql`): `notes.undo_stack` / `notes.redo_stack`
  (JSON kimlik listeleri, en çok 50), `note_revisions` (kalıcı sürüm geçmişi) ve
  `note_drafts` (kasa içi taslak). Göç, var olan notlar için 1. revizyonu
  `INSERT … SELECT` ile geriye dönük doldurur; eski notlarda geçmiş boş kalmaz.
- **Taslak**: 900 ms sonra `note_draft_save` ile **açık kasaya** yazılır
  (`note_drafts`, anahtar `(case_id, note_id)`, `note_id = ''` yeni not).
  `localStorage` kullanılmaz; kasa kilitliyse durum **"Taslak kaydedilemedi"**
  olur ve arayüz başarı iddia etmez. Taslak boş gövdeyi de saklar (kullanıcının
  sildiği metin geri gelmez); asıl nota dönüşürken boş gövde yine reddedilir.
- **Geri/ileri al**: `note_undo` / `note_redo` yığındaki **gerçek** revizyonu
  uygular ve yeni bir revizyon yazar (`origin = undo | redo`); geçmiş silinmez.
  Yığın boşsa `E_NOTE_HISTORY` döner, metin uydurulmaz. Yeni düzenleme ileri al
  dalını temizler (klasik geri/ileri al davranışı). Yığınlar kasa içinde olduğu
  için uygulama yeniden açıldığında geçmiş korunur (test:
  `history_survives_reopen_of_the_vault`).
- **Sürüm çakışması**: `note_update` `expected_row_version` ile yazar. Araya
  başka bir yazma girdiyse **yazmaz**, `E_ROW_VERSION` döner; arayüz
  `note_get` ile güncel kaydı okur, satır bazlı LCS farkını gösterir ve
  "Benim metnimi kaydet" / "Güncel metni koru" seçimini kullanıcıya bırakır.
  Eski görünüm sessizce üzerine yazılmaz.
- **Arayüz**: not başına "Düzenle" paneli; sürüm/revizyon bilgisi, taslak
  durumu, Geri al / İleri al (kaydedilmemiş değişiklik varken pasif —
  kullanıcının yazdığı metin sessizce kaybolmaz), Sürüm geçmişi listesi,
  kasadaki farklı taslağı "yükle / sil" şeridi. Çakışma açıkken düzenleme
  alanları kilitlenir; çakışan anlık görüntü ile yazılan metin birbirinden
  ayrışmaz.
- **İç içe işlem**: inceleme kararından (`commit_review_decision` → not) gelen
  `create_note` çağrısı artık çağıranın işlemine katılır (`note_write`
  `is_autocommit` denetimi); iç içe işlem hatası (`cannot start a transaction
  within a transaction`) giderildi.

## F07-T11 — şifreli sayfa önizlemesi ve kilitte bellek temizliği

- **Yeni crate `dim-preview`**: iki katmanlı önbellek.
  - **Disk**: PNG, kasa ana anahtarıyla AEAD şifreli
    `<kasa>/preview-cache/<sürüm>/<sayfa>_<dpi>.pv`. AAD
    `preview:v1:<sürüm>:<sayfa>:<dpi>` olduğu için kayıt başka sürüme/sayfaya
    taşınamaz; dosya düz PNG değildir (test: diskteki baytlarda PNG imzası yok).
  - **Bellek**: 32 MB sınırlı LRU, yalnız kilit açıkken. `vault_lock`
    `PreviewMemory::clear()` çağırır — çözülmüş sayfa baytları bellekte kalmaz
    (INV-06 / K0). Kasa kilitliyken komut `E_VAULT_LOCKED` döner.
- **Worker**: yeni `render_page` yöntemi (PyMuPDF → PNG). Sayfa numarası
  **fiziksel sıradır**, `/PageLabels` etiketi kullanılmaz. DPI 48–200 aralığına
  kırpılır, aralık dışı sayfa `E_PAGE_RANGE`, parola korumalı PDF
  `E_PDF_PASSWORD`, 8 MiB üstü çıktı `E_PREVIEW_TOO_LARGE` ile reddedilir —
  var olmayan sayfa için boş görüntü üretilmez.
- **Komut** `document_page_preview(versionId, pageIndex, dpi)`: bellek →
  şifreli disk → worker sırası. Bozuk/anahtarı uyuşmayan kayıt sessizce
  "yok" sayılmaz; atılır ve görüntü yeniden üretilir, yanıt `cached=false`
  olur (başarı iddiası yalnız gerçekten önbellekten geldiyse `cached=true`).
  Yalnız `application/pdf`; DOCX/UDF `E_PREVIEW_UNSUPPORTED` alır.
- **Arayüz**: okuyucuda "Sayfa görüntüsü" düğmesi (`aria-pressed`); görüntü
  fiziksel sayfa için çizilir, alt yazı kaynağı yazar
  ("şifreli önbellekten · 4 KB · 96 dpi" / "yeni üretildi"). Sayfa veya belge
  değişince eski görüntü bırakılır; istek yarışında geç gelen yanıt gösterilmez.
- **Sınırlama**: önizleme yalnız PDF. DOCX/UDF yapısal görünüm kullanır;
  görüntü belgeleri (JPG/TIFF) için sayfa çizimi yok. Önbellek sürüm silinince
  otomatik temizlenmez (`PreviewStore::purge_version` hazır, silme akışına
  bağlanmadı) — diskte şifreli kaldığı için veri sızıntısı değil, yer kullanımı.

## Doğrulama (F07-T12)

- Rust: `cargo test --workspace --all-targets` → storage **160** (5 yeni: güncel
  sürümde tek eşleşme + sürüm numaraları, belirsiz/çözümlenemedi, zaten güncel,
  doğrulanmış yeniden bağlama (durum `rebound`, `locate` güncel+intact, listede
  kalır), yanlış offset reddi (`E_QUOTE_NOT_FOUND`, eski bağ korunur)). Clippy
  `-D warnings` (workspace + Tauri) ve rustfmt iki manifestte temiz.
- Web: `tsc` + `vitest` **134** (3 yeni: eski sürüm rozeti güncel/eski/bilinmeyen,
  yeniden bağlama durum metinleri) + `vite build` yeşil.
- **Sınırlama**: gerçek Tauri penceresinde tıklama ve görsel QA yapılmadı; kanıt
  birim testleri + typecheck + üretim derlemesi.

## Doğrulama (F07-T12 — yeniden bağlama akışı, 2026-11-03)

- Web: `DualReader.rebind.test.tsx` **10** davranış testi (jsdom + RTL). Okuyucunun
  **kopmuş/eski bağı yeniden bağlama** yolu (`anchorLocate` → `anchorRebindSuggestion`
  → `anchorRebind`) bugüne dek hiç ölçülmemişti; bu yol kasaya **yazar** ve hangi
  bağa hangi offset'in gideceğini belirler. `lib/api` kısmi mock (12 fonksiyon),
  `t()` ve `lib/reader` **gerçek**. Sayfa sekmeleri de "Fiziksel sayfa N" adını
  taşıdığı için aday düğmeleri **öneri kartına daraltılarak** sorgulanır.
- Sabitlenenler: kopmuş bağın uyarısı + sürüm rozeti + arama düğmesi çekirdekten;
  sağlam/güncel bağda düğme yok; öneri kendiliğinden uygulanmaz (durum metni +
  alıntı + aday sayfası, "Kapat" listeyi kaldırır); seçilen aday
  `anchorRebind(anchorId, segmentId, startOffset)` ile gider, ardından güncel konum
  okunur, panel yeni sürüme geçer, yeşil bildirim çizilir, kopma uyarısı kalkar;
  bağ bulunamazsa künye uydurulmaz (sessiz); çift tık tek yazma üretir.
- **Ölçülen kusurlar (düzeltme öncesi ham koşu: 4 failed | 6 passed):**
  1. **Uçuştaki öneri hedef bağa bağlı değildi:** kullanıcı öneri askıdayken başka
     bağa geçince **önceki bağın adayları** yeni bağın altında çiziliyor ve
     "Bağla" **yeni bağa eski adayın offset'ini** yazabiliyordu (INV-05'in
     okuyucu karşılığı). Kırmızı: `expected <div class="card tight"> to be null`.
  2. **Uçuştaki yeniden bağlama hedef bağa bağlı değildi:** geç yanıt yeni bağın
     künyesini **önceki bağın konumuyla** eziyor, panel eski sürüme zıplıyor ve
     yeşil "bağlandı" bildirimi yeni bağın altında görünüyordu. Kırmızı:
     `expected 'v2' to be 'v1'`.
  3. **Bayat hata (öneri araması):** başında `setError(null)` yoktu → düşen
     denemeden sonra başarılı aramada kırmızı hata ekranda kalıyordu.
  4. **Bayat hata (bağlama):** aynı sınıf → düşen bağlamadan sonra başarılı
     bağlamada kırmızı `E_ANCHOR` satırı kalıyordu.
- **Düzeltme (`DualReader.tsx` +34/−12):** `anchorGen` (hedef bağ değişince artar)
  + `rebindSeq` (her istekte artar); öneri ve bağlama yanıtları yalnız kendi
  nesli **ve** en son istek için yazılır, hata yolu da kapıdan geçer; iki işlem
  başında hata silinir; bağlama isteği bağın kimliğini (`anchorId`) çağrı anında
  saklar. Kilidi yalnız en son istek açar.
- **Kapı kanıtı (5 mutasyon; her biri en az bir testi düşürdü; geri alma sha256
  `682230d8…` birebir):** M1 hedef bağ nesli artışı (**2** kırmızı: kusur A + B;
  aynı kökün iki yüzü), M2 konum okuma sonrası nesil kapısı (B), M3 öneride
  `setError(null)` (C), M4 bağlamada `setError(null)` (D), M5 öneri yanıt kapısı
  (A). **Dürüst:** yazma sonrasına konan nesil kapısı ölçülemedi (konum okuma
  sonrasındaki kapı zaten kararı veriyor; tek başına kaldırılınca hiçbir test
  düşmüyordu) → **koddan çıkarıldı**, yedekli kapı bırakılmadı.
- `tsc --noEmit` temiz · `vitest` **822/822** (57 dosya; 812 → **+10**) ·
  `pnpm build` temiz (**575,53 kB**, `index-D2S0cOCm.js`, gzip **155,04 kB**).
- **Dürüst sınırlar:** jsdom **tarayıcı değil** — CSS, gerçek Tauri penceresi,
  gerçek işaretçi ve **görsel QA yok**; `api` mock'lu olduğu için `anchor_rebind`in
  birebir alıntı doğrulaması (`E_QUOTE_NOT_FOUND`), `anchor_locate` SQL'i ve Rust
  **koşmaz**. Kapsam dışı: `ambiguous`/`unresolved` öneri metinleri (lib testleri
  kapsıyor), otomatik toplu yeniden bağlama, klavye, Windows.
- **Açık kayıt:** yeniden bağlama yazması çekirdekte başarılı olup kullanıcı bu
  sırada başka bağa geçerse arayüz sonucu **bilinçli olarak yazmaz** (yeni bağın
  künyesi korunur); kullanıcı eski bağa döndüğünde konum `anchor_locate` ile
  tazelenir. Bağlama başarılı olup konum okuması düşerse kırmızı hata yazılır ve
  önceki künye korunur (yazma gerçekleştiği hâlde arayüz bunu iddia etmez) —
  dürüst sonuç, kullanıcı yeniden deneyebilir.

## Doğrulama (F07-T09 — bölge işareti akışı, 2026-11-03)

- Web: `DualReader.region.test.tsx` **11** davranış testi (jsdom + RTL). Sayfa
  görüntüsü üzerinde **sürüklenen bölge** (`markCreateRegion`) bugüne dek hiç
  ölçülmemişti; `DualReader.selection.test.tsx` yalnız metin seçiminden giden üç
  komutu kapsıyordu. `lib/api` kısmi mock (`markCreateRegion` + mevcut yollar),
  `t()` ve `lib/reader` **gerçek**. jsdom `PointerEvent`/pointer capture
  sağlamadığı için olaylar `clientX/clientY` taşıyan `MouseEvent` ile, yakalama
  metotları no-op olarak kurulur; görünüm kutusu `getBoundingClientRect` ile
  ölçülendirilir.
- Sabitlenenler: seçim yokken işaret düğmesi kapalı, sürükleme açar, "temizle"
  kaldırır; %1'den küçük sürükleme bölge sayılmaz; sürükleme **görünen uzaydan
  sayfa uzayına** çevrilip tam sözleşmeyle gider (400×600 kutu → 600×800 punto,
  `rects: [[60, 80, 300, 400]]`, `label: ""`); 90° döndürülmüş görünümde
  koordinat döndürme öncesine çevrilir (`[[60, 400, 300, 720]]`); başarıda
  işaret listeye girer, seçim bırakılır; sayfa ölçüsü yoksa istek üretilmez.
- **Ölçülen kusurlar (düzeltme öncesi ham koşu: 5 failed | 5 passed; on birinci
  test sonradan eklendi ve o da düzeltme öncesinde kırmızıydı):**
  1. **Sayfa değişince bölge taşınıyordu:** `dragRegion` sayfa durumundan ayrı
     tutulduğu için 1. sayfada çizilen kutu 2. sayfada duruyor ve "Seçili bölgeyi
     işaretle" **önceki sayfanın koordinatlarını yeni sayfaya** yazabiliyordu.
  2. **Sürüm değişince bölge taşınıyordu:** aynı sınıf — başka belgenin sayfasına
     eski koordinatlar işaret olarak yazılabiliyordu (§13.5'in okuyucu karşılığı).
  3. **Uçuştaki yanıt sürüm kapısı taşımıyordu:** `markCreateRegion` askıdayken
     belge değişince eski sürümün işareti **yeni sürümün** listesine ekleniyordu.
  4. **Bayat hata:** işlem başında `setError(null)` yoktu → düşen denemeden sonra
     başarılı bölge işaretinde kırmızı hata ekranda kalıyordu.
  5. **Uçuştaki yanıt yeni seçimi siliyordu:** istek sürerken çizilen bölge, geç
     yanıtın koşulsuz `setDragRegion(null)` çağrısıyla sessizce kayboluyordu.
  6. **Sürüm değiştikten sonra düşen istek** yeni sürüme hata yazıyordu (hata
     yolu da sürüm kapısından geçmiyordu).
- **Düzeltme (`DualReader.tsx` +19/−3):** sürüm effect'i ve sayfa değişimi
  `dragRegion`/`dragStart`ı temizler; `createRegionMark` isteğin sürümünü ve
  seçim nesnesini saklar; yanıt yalnız aynı sürümde yazılır, seçim yalnız hâlâ
  **aynı** nesneyse bırakılır, liste kimlikle tekilleştirilir; işlem başında hata
  silinir.
- **Kapı kanıtı (6 mutasyon; her biri kendi testini düşürdü, `1 failed | 10
  passed`; geri alma sha256 `7b6789c4…` birebir):** M1 sürüm temizliği,
  M2 sayfa temizliği, M3 başarı yolundaki sürüm kapısı, M4 `setError(null)`,
  M5 koşullu seçim temizliği, M6 hata yolundaki sürüm kapısı. **Dürüst:**
  düğme kilidi (`markBusy`) tek başına çift tık testini yeşil tutuyordu
  (seçim turundaki M1/M2 gibi yedekli kapı) — ayrı mutasyon sayılmadı,
  `writeGate` eklenmedi.
- `tsc --noEmit` temiz · `vitest` **812/812** (56 dosya; 801 → +11) ·
  `pnpm build` temiz (**575,18 kB**, `index-CDKjIZEz.js`, gzip **154,94 kB**).
- **Sınırlama:** jsdom tarayıcı değil — CSS, gerçek Tauri penceresi, gerçek
  işaretçiyle sürükleme ve **görsel QA yok**; jsdom pointer capture'ı
  sağlamadığı için testte no-op kuruldu. `api` mock'lu olduğu için çekirdeğin
  gerçek yolu (`mark_create_region` geometri doğrulaması, SQL, Rust) **koşmaz**.
  Kapsam dışı: dokunmatik, klavye, çok kutulu birikimli seçim, Windows.

## Doğrulama (F07-T09)

- Rust: `cargo test --workspace --all-targets` → storage **155** (8 yeni: doğrulanmış
  anchor'dan işaret, uydurma alıntıda hiç kayıt yok, aynı anchor tek işaret, bölge
  geometri doğrulaması (içeride/dışarıda/boş), yabancı sayfa `E_NOT_FOUND`, yumuşak
  silme, notun kaynağı anchor'dan gelir, boş gövde anchor yazmaz). Clippy
  `-D warnings` (workspace + Tauri) ve rustfmt iki manifestte temiz.
- Web: `tsc` + `vitest` **131** (8 yeni: işaret parçalama/birleştirme/hedef önceliği/
  kırpma, sayfa filtresi, `mapRectForDisplay` tersi (0/90/180/270), döndürülmüş
  dikdörtgen, rotasyonsuz dönüşüm) + `vite build` yeşil.
- Python worker değişmedi (bu adımda yeni worker yöntemi yok).
- **Sınırlama**: gerçek Tauri penceresinde sürükleme, tıklama ve görsel QA
  yapılmadı; kanıt birim testleri + typecheck + üretim derlemesi.

## Doğrulama (F07-T09 — seçim akışı, 2026-11-03)

- Web: `DualReader.selection.test.tsx` **16** davranış testi (jsdom + RTL). Seçim
  gerçek `window.getSelection()` aralığıyla kurulur, `mouseup` tetiklenir;
  `lib/api` kısmi mock (`anchorCreate`, `markCreateFromSelection`,
  `noteCreateFromSelection`, `markDelete`), `t()` ve `lib/reader` **gerçek**.
  `DualReader.test.tsx` yalnız panel/sayfa/yakınlaştırma davranışını ölçüyordu;
  metinden **çekirdeğe giden** üç komut (kaynak, işaret, kaynaklı not) bugüne
  dek hiç ölçülmemişti.
- Sabitlenenler: seçim yokken üç düğme kapalı; aralık **skaler** offsetle yazılır
  (emoji tek karakter: UTF-16 3 → skaler 2); kaynak kaydı tam sözleşmeyle gider,
  başarıda seçim bırakılır ve `onAnchor` çağrılır; düşen kayıt seçimi korur;
  çift tık tek istek üretir; işaret belgenin listesine girer, düşen işaret satır
  eklemez; not taslağı alıntı + sayfa etiketiyle açılır; kayıt çapa ve gövdeyi
  tek istekte gönderir; düşen kayıt yazılanı korur; boş gövde gönderilmez;
  taslak açıkken başka metin seçmek ya da sayfa değiştirmek hedefi kaydırmaz;
  başarılı işlem bayat hatayı siler; ikinci kaynak denemesi düşünce önceki yeşil
  satır kalkar; sürüm değişince geç gelen işaret yanıtı yeni sürüme yazılmaz.
- **Ölçülen kusurlar (düzeltme öncesi ham koşu: 5 failed | 10 passed; ilk ham
  koşu 8 failed | 7 passed ve 3'ü testin kendi sorgu hatasıydı):**
  1. `saveAnchor` uçuş kilidi taşımıyordu → hızlı çift tık **iki kaynak** yazıyordu.
  2. Not taslağı hedefini dondurmuyordu: gövde ilk alıntıdan, çapa **yeni**
     seçimden gidiyordu (`note_create_from_selection` yanlış alıntıya bağlanır).
  3. Kaynak satırı canlı seçimden okunuyordu: sayfa değişince "Fiziksel sayfa 2 ·
     alıntı –" yazıyor ve "Notu kaydet" **sessizce hiçbir şey yapmıyordu**.
  4. Üç yazma yolu başında `setError(null)` yoktu → başarılı işaretten sonra
     önceki `E_QUOTE` kırmızısı ekranda kalıyordu.
  5. `createMark` geç gelen yanıtı sürüm kapısından geçirmiyordu → eski sürümün
     işareti **yeni sürümün** listesine ekleniyordu.
  6. `saveAnchor` önceki yeşil "Kaynak kaydedildi" satırını silmiyordu.
- **Yerelleştirme sızıntısı (harf kapısının kör noktası):** not başlığı
  `` `${… ?? "Sayfa"} notu` `` idi; iki kelime de Türkçeye özgü harf taşımadığı
  için mevcut kapı görmedi. `readerUi.noteTitle` sözlüğe eklendi;
  `localization.test.ts`e **ASCII Türkçe kelime** kapısı (dize/template literal,
  canary dahil) eklendi — kapı eski hâli yakalıyor.
- **Kapı kanıtı (9 mutasyon):** **M1** (ref kapısı) ve **M2** (düğme kilidi) tek
  tek kaldırıldığında test **yeşil kaldı** — iki kapı birbirini yedekliyor;
  **M1b** ikisi birlikte kaldırılınca kırmızı (dürüst kayıt: tek başına hiçbiri
  zorunlu değil, ikisi birlikte zorunlu). M3 (taslak dondurma), M4 (kaynak
  satırı), M5 (bayat hata), M6 (sürüm kapısı), M7 (bayat yeşil satır) kendi
  testlerini düşürdü; M8 (gömülü başlık geri kondu) **iki** testi düşürdü (yeni
  kapı + kullanılmayan anahtar). Geri alma sha256 `11240d8d…` birebir.
- `tsc --noEmit` temiz · `vitest` **788/788** (55 dosya; 771 → +17) ·
  `pnpm build` temiz (**574,05 kB**, `index-3NzDNOKe.js`, gzip **154,60 kB**).
- CI: `b548a27` → `ci.yml` **41536355799**, **7/7 yeşil** (`arayüz` **55 dosya /
  788 test**, aynı paket `index-3NzDNOKe.js` 574,05 kB — yerel/CI **birebir**;
  `rust (macos-15)` 5 dk 12 s, `rust (ubuntu-latest)` 5 dk 56 s, `masaüstü kabuk`
  35 s, `belge worker'ı` 44 s, `SBOM` 20 s, `sözleşmeler` 7 s).
- **Sınırlama:** jsdom tarayıcı değil — CSS, gerçek Tauri penceresi, gerçek
  işaretçiyle metin seçimi ve **görsel QA yok**; `api` mock'lu olduğu için
  çekirdeğin gerçek yolu (`anchor_create` doğrulaması, `note_create_from_selection`
  SQL'i, `row_version`, Rust) **koşmaz**. jsdom `Selection` API'si gerçek tarayıcı
  seçiminin yerine geçmez (sürükleme, çift tık kelime seçimi, dokunmatik ölçülmedi).

## Doğrulama (F07-T08)

- Web: `tsc` + `vitest` **123** (16 yeni) + `vite build` yeşil. Yeni testler:
  görünüm belleği (kaydet/geri ver, aynı sürümü güncelleme, sınır, boş kimlik,
  kopya izolasyonu), seçim doğrulama (eşleşme, kaymış offset, aralık dışı/ters/
  tam sayı olmayan), klavye gezinme (Alt ile ileri-geri, uçlarda durma,
  Home/End, değiştirici reddi, boş belge), boş panelde belge seçimi (diğer
  panelin sürümünü atlama, tek belgeye dönme, sürümsüz belge).
- Rust ve Python değişmedi: bu adım yalnız arayüz görünüm durumunu ele alır;
  çekirdek komut, şema veya worker protokolü değişmedi.
- **Sınırlama**: Tauri penceresi bu ortamda açılmadı. Panel belleği, seçim geri
  yükleme ve klavye gezinmesi saf fonksiyon testleri + typecheck + üretim
  derlemesiyle doğrulandı; gerçek tıklama, odak sırası ve görsel QA yapılmadı.

## Doğrulama (F07-T11)

- Rust: `cargo test --workspace --all-targets` → **222** test (yeni `dim-preview`
  6: şifreli disk yuvarlak yolu + PNG imzası yokluğu, AAD bağlaması, sürüm
  temizliği, LRU sınırı, kilitte temizleme, büyük kayıt belleğe alınmaz).
  Clippy `-D warnings` workspace + Tauri, rustfmt iki manifest temiz.
- Python: worker **77 passed, 1 skipped** (6 yeni: PNG + sayfa sayısı, aralık
  dışı ret, DPI kırpma, protokolde `render_page`, tam sayı olmayan sayfa
  indeksi `E_PARAMS`, aralık dışı `E_PAGE_RANGE`).
- Gerçek worker IPC (elle): 2 sayfalı PDF → `render_page` `pageIndex=1`,
  `dpi=72` → `pageCount=2`, PNG `\x89PNG\r\n\x1a\n`, 4600 bayt.
- Web: `tsc` + `vitest` **107** (4 yeni: data URL, önbellek/yeni üretim
  alt yazısı, MB/KB biçimi, 1 KB alt sınırı) + `vite build` yeşil.
- **Not**: bu adımın kodu eşzamanlı çalışan F16 ajanının `77fcc76` numaralı
  commit'ine karıştı; commit mesajı F16 ADR'sini anlatıyor. İşlevsel içerik
  yukarıdaki gibidir, kanıt bu sürüm notunda tutulur. Bu commit yalnız
  `crates/ingest/src/worker.rs` rustfmt düzeltmesini ve bu belgeyi ekler.
- Sınırlama: Tauri penceresi bu ortamda açılmadı; görüntü düğmesi ve önbellek
  yolu yalnız typecheck + birim testleri + üretim derlemesiyle doğrulandı.
  Gerçek tıklama ve görsel QA yapılmadı.

## Doğrulama (F07-T10)

- Rust: `cargo test --workspace --all-targets` → **216** test (storage **144**;
  not modülü 16, 11 yeni: taslak yuvarlak yol/temizleme, yabancı dosya notuna
  taslak reddi, sürüm artışı + revizyon, bayat yazmanın reddi, boş gövde,
  geri/ileri al, boş yığın `E_NOTE_HISTORY`, geri aldıktan sonra düzenlemenin
  ileri dalı temizlemesi, iki adım geri, yeniden açılışta geçmiş, göç
  doldurma ifadesi). Clippy `-D warnings` (workspace + Tauri) ve rustfmt iki
  manifestte temiz.
- Web: `tsc` + `vitest` **103** (16 yeni: taslak durum etiketleri, bayat taslak,
  revizyon kaynağı etiketleri, LCS farkı/silme/ekleme/Türkçe karakter/boş
  metin/uzun metin sınırı, kaydedilmemiş değişiklik) + `vite build` yeşil.
- Sınırlama: Tauri penceresi bu ortamda açılmadı; düzenleme paneli, geri/ileri
  al düğmeleri ve çakışma ekranı yalnız typecheck + birim testleri + üretim
  derlemesiyle doğrulandı. Gerçek tıklama ve görsel QA yapılmadı.
- Sınırlama: geri/ileri al kayıt düzeyinde çalışır (son kayıt ↔ önceki kayıt);
  tuş vuruşu düzeyinde metin geçmişi tarayıcı textarea davranışına bırakılmıştır.
  Taslak otomatik kaydı 900 ms gecikmelidir; uygulama bu süre içinde kapanırsa
  son değişiklikler taslağa yazılmamış olabilir.

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
