# F12 — Emsal kütüphanesi ve atıf kontrolü

- **Sürüm etiketi:** `v0.11.0` (hedef)
- **Durum:** Devam ediyor
- **Tarih:** 2026-10-24

| İş | Durum | Kanıt |
|---|---|---|
| F12-T01 | ✅ | emsal künyesi, etiket, konu; eksik künye `incomplete` |
| F12-T02 | ✅ | kullanım notu: benzerlik / ayrışan nokta / kullanıcı sonucu |
| F12-T03 | ✅ | atıf kontrolü: künye ve alıntı boyutları ayrı |
| F12-T04 | ✅ | “bulunamadı” ile “uydurulmuş” ayrımı korunur; sahte hükmü yok |
| F12-T05 | ✅ | karar tam metni (şema v13) + metin içi alıntı doğrulama; **tire ile bölünmüş kelime** katmanı ve gerçek kaynak aralığı (2026-10-27). **Arayüz davranış testi eklendi** (`screens/Precedents.test.tsx`, **18 test**; 3 yeni mutasyonla kanıtlandı; alıntı metni değişince **bayat hüküm + “atıf alanına taşı” düğmesi** ekranda kalıyordu, uçuştaki doğrulama yanıtı **yeni metnin** hükmü gibi yazılıyordu ve emsal değişiminde eski emsalin hükmü yeni emsalin alanında görünüyordu) — 2026-11-03 |
| F12-T06 | ✅ | toplu içe alma (CSV künye listesi, çakışma işareti) + dış bağlayıcı sicili (etkin değil) |
| F12-T07 | ✅ | alıntı çevresi + anlam incelemesi (şema v15; gerekçesiz olumsuz hüküm yok) |
| F12-T10 | ✅ | kütüphane araması dosya aramasından ayrı (şema değişikliği yok; sızıntı testi). **Alıntı kopyalama** etiketi kendi sonucuna bağlandı, geç yanıtlar yeni sonuç kümesine yazılmaz (**10 test**, 2026-11-03) |
| F12-T11 | ✅ | kaynak kullanım koşulu (şema v16) + izinsiz toplu korpus dağıtım kapısı |
| F12-T12 | ✅ | bibliyografik künye: künye + kaynak bağlantısı + erişim tarihi (eksik alan tahmin edilmez) |
| F12-T13 | ✅ | kasadaki belge sürümünden tam metin aktarma (ayrıştırılmış sayfa metni; sürümü kullanıcı seçer) |
| F12-T14 | ✅ | metin içi alıntı denetimi: iddia/özet metnindeki tırnaklı pasajlar dosya belgelerinde aranır; birebir tek konum kaynağa bağlanır; sayfa sınırını aşan alıntı bulunur ama bağlanmaz; güncel sürümde bulunamayan alıntı belgenin **eski sürümlerinde** de aranır; **satır/sayfa sonunda tire ile bölünmüş kelime** birleştirilerek aranır (en zayıf katman, bağlanmaz); **font ligatürü** (`ﬂ`) açılarak aranır (kendi katmanı, bağlanmaz). Gerçek PDF + gerçek worker ile uçtan uca (2026-10-30) |

## F12-T11 — kaynak kullanım koşulu ve korpus dağıtım kapısı

**Şema v16** (`0016_precedent_usage_terms.sql`): `precedents.usage_basis`,
`redistribution`, `terms_url`, `terms_note`, `terms_recorded_at` +
`redistribution` indeksi. Varsayılan **`unknown`**; kayıt yokluğu izin sayılmaz.

- `set_precedent_usage_terms` — dayanak ve izin **ayrı** kaydedilir. `allowed`
  yalnız `official_public` / `licence` / `permission` dayanağıyla ve yazılı
  dayanakla (bağlantı veya not) işaretlenebilir; aksi hâlde `E_TERMS`, kayıt
  yazılmaz. Bağlantı http(s) olmalı; not/bağlantı denetime **yazılmaz**.
- `precedent_corpus_coverage` — toplam, tam metinli, izinli, izinsiz,
  bilinmiyor, engelli liste (en çok 100 + toplam).
- `build_precedent_corpus(mode)` — `permitted_only` yalnız izinlileri alır ve
  dışarıda kalanları gerekçesiyle raporlar; `all` tek izinsiz metinde
  `E_CORPUS_NOT_PERMITTED` verir ve **hiçbir metin** seçmez. `contentHash`
  kanonik `items` JSON'u üzerinden SHA-256; sıra kimliğe göre sabit.
- Tauri `precedent_terms_get`, `precedent_terms_set`, `precedent_corpus_coverage`,
  `precedent_corpus_export` (kaydetme iletişim kutusu → dosya → sha256 →
  denetim; ret denemesi `export_denied`, boş korpus `E_CORPUS_EMPTY`).
- Arayüz: listede izin rozeti; **Kaynak kullanım koşulu** kartı; **Toplu korpus
  dağıtımı** kartı (sayılar, engelli kayıtlar, kip, sonuç).
- Ayrıntı ve dürüst sınırlamalar: `docs/releases/F12/USAGE-TERMS.md`.
- **Test:** storage **212** (9 yeni), Web **175** (10 yeni); clippy storage +
  tauri `-D warnings`; rustfmt iki manifest.

## F12-T06 — toplu içe alma ve dış bağlayıcı sicili

**Şema v14** (`0014_precedent_conflict.sql`): `precedent_versions.identity_conflict`
ve `conflict_with_version_id`; `(precedent_id, body_hash)` indeksi.

- `Database::import_precedent_copy(&PrecedentCopyInput)` — metin **çıkarılmış**
  olarak gelir; bu katman ağ, dosya okuma veya worker çağrısı yapmaz.
  Sonuç: `created` | `duplicate` | `conflict` (+ `needs_review`).
  Aynı künye + aynı SHA-256 yeni sürüm yazmaz; aynı künye + farklı metin
  nüshayı **ayrı sürüm** olarak ekler ve incelemeye işaretler (sessiz
  birleştirme yok). Kimlik: mahkeme + daire + esas/karar/başvuru numarası
  birebir; numara yoksa birleştirme yapılmaz, farklı mahkeme ayrı emsaldir.
- `parse_manifest` — bağımlılıksız RFC 4180'e yakın CSV: tırnaklı alan,
  kaçırılmış tırnak, alan içi satır sonu, BOM. Türkçe/İngilizce başlıklar;
  bilinmeyen sütun ve aynı dosya adının tekrarı `E_MANIFEST` ile reddedilir.
- `precedent_bulk_import` (Tauri): dosya seçimi → `dim_ingest::detect` →
  worker `extract_text` → künye (CSV satırı, yoksa varsayılan) → kasa yazımı.
  Mutlak yol, 200 dosya ve 250 MiB sınırları; görüntü dosyası reddi; boş
  metin yazılmaz. Bir dosyanın hatası diğerlerini durdurmaz.
- `connectors.rs` + `precedent_connector_list`: Yargıtay, Danıştay ve AYM için
  erişim yöntemi/kapsam/yeniden kullanım notu **belgelenir**, hiçbiri etkin
  değildir (`network_allowed=false`), sicil içerik özeti (sha256) taşır.
  Ayrıntı ve karar gerekçesi: `docs/releases/F12/CONNECTORS.md`.
- Arayüz: "Toplu içe alma" kartı (dosya seç, CSV seç, varsayılan künye, dosya
  başına sonuç rozetleri, çakışma uyarısı) ve "Dış bağlayıcılar" kartı
  (kaynak başına "etkin değil" + gerekçe + koşullar bağlantısı).
  Sürüm listesinde çakışan nüsha "aynı künye, farklı metin" rozetiyle görünür.

## F12-T05 — karar tam metni ve metin içi alıntı doğrulama

**Şema v13** (`0013_precedent_text.sql`): `precedent_versions.full_text`
(şifreli veritabanında) ve `text_char_count`; `precedent_id, acquired_at DESC`
indeksi.

- `add_precedent_text(precedent_id, {sourceKind, text, documentVersionId})`:
  kaynak türü `paste` | `document` | `file` ile sınırlı; verilen
  `document_version_id` kasadaki gerçek sürüm olmalı (`E_NOT_FOUND`). Boş metin
  ve bilinmeyen kaynak türü `E_DB_SETTING`. `body_hash` = SHA-256(tam metin),
  `verification_state='text_verified'`, denetime **metin yazılmaz** (yalnız
  hash + karakter sayısı).
- `precedent_versions` / `get_precedent_version`: metin gövdesi **dönmez**;
  yalnız `has_text`, `charCount`, `bodyHash`, `sourceKind`, tarih.
- `verify_precedent_quote(precedent_id, quote)`: kararın **en yeni** tam metnine
  karşı dört aşamalı doğrulama —
  1. birebir (`method='exact'`, kaynak metindeki **karakter** konumu),
  2. yalnız satır/boşluk farkı (`method='whitespace'`),
  3. Türkçe harf sadeleştirmesi (`method='turkish_fold'`, OCR/klavye farkı),
  4. satır sonunda tire ile bölünmüş kelime birleştirme (`method='dehyphenated'`).
  Güncel metinde bulunamazsa emsalin **eski tam metin sürümleri** yeniden eskiye
  taranır (en çok 5; sınır aşılırsa `historicalTruncated=true`, sessiz kesme yok).
  Katman sırası belge tarafıyla aynıdır: her katmanda önce güncel sürüm, sonra
  eskiler — yani eski sürümdeki birebir eşleşme günceldeki zayıf eşleşmeyi yener,
  aynı katmanda güncel kazanır. Eşleşme sayısı (`occurrences`) yalnız **eşleşen**
  sürümün metninden sayılır. Sonuç `match` | `mismatch` | `not_checked`;
  `not_checked` nedenleri `no_full_text`, `quote_too_short` (< 8 karakter — yanlış
  kesinlik üretmemek için), `mismatch` nedeni `not_found`. Yanıt
  `versionsScanned`, `historicalVersionsScanned`, `foundInOldVersion`,
  `historicalTruncated` alanlarını taşır; `foundInOldVersion=true` “alıntı güncel
  metinde geçmiyor” demektir, sessizce gizlenmez.
- `normalize_with_map`, `dim_evidence::normalize` ile **aynı** sadeleştirmeyi
  yapar ama konum eşlemesi taşır; ikisinin ayrışmasını
  `fold_matches_evidence_normalize` testi yakalar.
- Tauri: `precedent_text_add`, `precedent_version_list`,
  `precedent_quote_verify`.
- Arayüz (`Precedents.tsx` → “Tam metin ve alıntı doğrulama” kartı): sürüm
  listesi (kaynak türü, tarih, sha256 kısa, `tam metin · N karakter` /
  `tam metin yok`), tam metin yapıştırma, alıntı doğrulama; sonuç dürüst metinle
  gösterilir (“birebir bulundu (konum 1.234)” / “yalnız satır/boşluk farkıyla —
  birebir değil” / “bulunamadı … uydurulmuş demek değildir” / “tam metin yok;
  doğrulanamadı”). Sonuç, tek düğmeyle atıf kontrolü formunun `quote_match`
  alanına taşınır; künye ve anlam boyutları ayrı kalır.
- `apps/desktop/src/lib/precedents.ts`: durum metinleri/tonları saf fonksiyon
  olarak; `precedents.test.ts` 11 test.

## Dürüst durum

Karar **tam metni** yoksa alıntı doğrulanamaz; kontrol `not_checked` olarak
kalır ve “uydurulmuş” anlamına gelmez. Emsal notu hukuki uygunluk
değerlendirmesidir; sistem hükmü değildir.

## Sınırlamalar

- Tam metin **yapıştırma**, **toplu içe alma** (dosyadan, `source_kind=file`) veya
  **kasadaki belge sürümünden aktarma** (`source_kind=document`, F12-T13) ile
  girer. Belgeden aktarımda dosya kopyalanmaz: o sürümün en yeni başarılı/kısmi
  ayrıştırmasının sayfa metinleri birleştirilir.
- Doğrulama yalnız metin eşleşmesidir; **anlam/bağlam** incelemesi ayrı boyut
  olarak kalır (F12-T07).
- Sadeleştirme Türkçe harf katlamasıyla ve **satır sonunda tire ile bölünmüş
  kelime birleştirmesiyle** sınırlı (2026-10-27); kelime değişikliği, çeviri
  veya özet metin eşleşmez — bu durumda sonuç `mismatch` (“bulunamadı”) olur,
  “uydurulmuş” değil.
- Tauri penceresi bu ortamda açılmadı: kart, yapıştırma ve doğrulama akışı
  typecheck + birim testleri + üretim derlemesiyle doğrulandı; gerçek tıklama ve
  görsel QA yok.

## Doğrulama

- Rust: `cargo test -p dim-storage precedents::` → **14/14** (8 yeni);
  `cargo test --workspace --all-targets` → **256** (storage 169); clippy
  workspace + tauri `-D warnings`; rustfmt iki manifest.
- Web: `pnpm typecheck`, `pnpm test` → **146** (11 yeni), `pnpm build` yeşil.
- CI: bu commit ile doğrulanacak (aşağıdaki kayıt güncellenir).

## F12-T07 — Alıntı çevresi ve anlam incelemesi (ek)

- **Şema v15** (`0015_citation_meaning.sql`): `meaning_note`, `meaning_reviewed_at`.
- `review_citation_meaning`: `accurate | misleading | off_context`; olumsuz
  hükümde **gerekçe zorunlu** (gerekçesiz "yanıltıcı" damgası basılamaz), denetim
  kaydı yazılır.
- `citation_context`: alıntıyı mevcut `verify_precedent_quote` ile birebir doğrular,
  sonra tam metinden `radius` karakter öncesi/sonrası döndürür. Metin yoksa
  `found=false` + neden (`no_full_text` / `no_candidate`) — uydurma bağlam yok.
- Arayüz: atıf kontrolü satırında anlam rozeti; panelde alıntı çevresi (vurgulu)
  ve üç karar + gerekçe alanı.
- Test: 3 yeni storage testi (gerekçe zorunluluğu, bağlam çevresi, dürüst
  "metin yok" durumu).

Künye doğruluğu, alıntının birebir bulunması ve **anlamın kararı yansıtması**
üç ayrı boyuttur; biri diğerinin yerine geçmez.

## F12-T12 — Bibliyografik künye, kaynak bağlantısı ve erişim tarihi

- `precedent_bibliography(precedent_id)` → `PrecedentBibliography`:
  `text`, `missing[]`, `warnings[]`, `sourceUrl`, `accessedAt`, `accessBasis`,
  `hasFullText`. Şema değişikliği yok; veri mevcut alanlardan okunur.
- **Biçim:** `{mahkeme} {daire}, {GG.AA.YYYY}, E. {esas}, K. {karar}, B. No: {başvuru}, “{başlık}”.
  Kaynak: {url} (erişim: {GG.AA.YYYY})` — yalnız **var olan** alanlar yazılır.
- **Eksik alan tahmin edilmez:** tarih/esas/karar/başlık/kaynak yoksa
  `missing` listesine girer ve uyarı üretilir; künye metnine yer tutucu
  (`-`, `[tarih]`) konmaz. Mahkeme her zaman zorunlu alandır.
- **Erişim tarihi:** en yeni **tam metin** sürümünün `acquired_at` değeri
  (`access_basis=version`); tam metin yoksa künye kaydının oluşturulma zamanı
  (`access_basis=record`) ve bu durum arayüzde açıkça yazılır. Kaynak siteye
  yeniden erişilmez; tarih kasadaki kayıttan gelir.
- **Tarih biçimi:** yalnız `YYYY-AA-GG` (saat kısmı varsa ilk 10 karakter)
  `GG.AA.YYYY`'ye çevrilir. Başka biçim olduğu gibi yazılır ve uyarı verir;
  geçersiz ay/gün (`2025-13-01`) çevrilmez.
- Tauri `precedent_bibliography`; arayüzde seçili emsalde “Bibliyografik künye”
  kartı: kopyalanabilir metin, eksik alan rozeti, erişim tarihi ve kaynağı,
  tam metin durumu, “Künyeyi kopyala”. Pano izni yoksa metni seçip elle
  kopyalama önerilir (sessiz başarısızlık yok).
- Test: 5 yeni storage testi (tam künye + sürüm erişim tarihi, eksik alan
  listesi, ISO olmayan tarih, bulunmayan emsal, tarih ayrıştırıcı sınırları);
  2 yeni web testi (eksik alan metni, erişim tarihi kaynağı).

### Dürüst sınırlama

Künye biçimi tek bir TR hukuk geleneğine göre sabittir; dipnot/atıf stili
seçimi (örn. makale stili, kısaltmalar) yoktur. Erişim tarihi kasadaki edinim
kaydıdır — kaynağın bugün hâlâ erişilebilir olduğu doğrulanmaz.

## F12-T10 — Emsal kütüphanesi kapsamının dosya aramasından ayrılması

- **Ayrı çekirdek:** `Database::search_precedents(PrecedentSearchQuery { text, mode, limit })`
  → `PrecedentSearchPage`. Kapsam alanı **her zaman** `precedent_library`;
  dosya araması (`search_page`) `scope="case"` döndürmeye devam eder. İki
  arama aynı sonuç listesini paylaşmaz, birbirinin sonucunu filtrelemez.
- **Ne taranır:** her emsal için **en yeni tam metin** sürümü
  (`precedent_versions.full_text`) ve docket kipinde künye numaraları
  (esas/karar/başvuru). Metin gövdesi arayüze çıkmaz; yalnız eşleşme
  çevresinden en çok 240 karakterlik pencere (+ en çok 60 karakter ön bağlam).
- **Kip/tırnak kuralı tek yerde:** kütüphane araması da dosya aramasının
  `SearchQuery::resolve` + `SearchMatcher` yolunu kullanır (auto → tırnaklı
  metin ifade, `2024/123 E.` docket, diğerleri esnek). `dim_evidence::normalize`
  ile Türkçe katlama aynı.
- **Kapsam sızıntısı testi:** kütüphaneye yazılan belirteç dosya aramasında
  (`search_page`) **0** sonuç verir ve `text_segments` tablosunda hiç
  bulunmaz; emsal metni segment indeksine yazılmaz.
- **Eksik bilgi gizlenmez:** tam metni olmayan emsal sayısı `withoutText` ve
  `notes` ile bildirilir; bu kayıtlar metin aramasında taranmaz. Eşleşme yoksa
  not “karar yok hükmü değildir” der.
- **Eşleşme kaynağı ayrı:** `matchSource=full_text|metadata`; künye
  eşleşmesinde karakter konumu (`matchStart`) **uydurulmaz**, `null` kalır.
  Konum yalnız birebir → boşluk sadeleştirmesi → Türkçe katlama ile
  bulunabildiğinde dolar.
- **Arayüz:** Arama ekranında iki düğmeli kapsam seçici (`aria-pressed`),
  kalıcı kapsam uyarısı, kütüphane kapsamında dosya seçimi ve belge
  filtrelerinin gizlenmesi + nedeni; sonuç kartında künye, eşleşme kaynağı,
  pencere ve “Alıntıyı kopyala” (pano reddederse açık hata). Emsal ekranı da
  kendi kapsamını yazar. Kapsam değişince öteki kapsamın sonuçları temizlenir.
- Test: 6 yeni storage testi (kapsam+sızıntı, metinsiz kayıt sayımı, Türkçe
  katlama + pencere/konum, docket künye eşleşmesi, limit/toplam, boş sorgu);
  5 yeni web testi (kapsam etiketleri, künye, eşleşme etiketi, özet, kopya
  metni).

### Dürüst sınırlama

Kütüphane araması **sayfalanmaz**: tek turda en çok 50 sonuç döner
(`truncated`, `matched` ile toplam bildirilir); imleç yok. Arama, tam metinleri
Rust tarafında tek tek tarar (emsal sayısı binlerle sınırlıyken hızlı, on
binlerde doğrusal) — FTS indeksi yok. Pencere konumu yalnız üç yöntemle
bulunur; noktalama araya girdiği ifade eşleşmelerinde konum `null` kalabilir
(snippet baştan gösterilir). Kütüphane sonucu dosyaya delil olarak eklenmez;
bunu yapan bir düğme bilinçli olarak yok.

## F12-T13 — Kasadaki belge sürümünden tam metin aktarma

`source_kind=document` yolu artık arayüzden çalışır; `add_precedent_text`'in
"verilen `document_version_id` kasadaki gerçek sürüm olmalı" kuralı korunur,
üzerine **metin üretimi** eklenir.

- `Database::precedent_text_from_document(precedent_id, document_version_id)` →
  `PrecedentDocumentText { version, parseRunId, engine, parseStatus, pages,
  ocrPages, emptyPages, charCount, written, notes }`.
  - Metin **çıkarılmış** sayfa metnidir: o sürümün **en yeni** `succeeded` /
    `partial` ayrıştırma koşusunun sayfaları çift satır sonuyla birleştirilir.
    Dosya baytları kopyalanmaz; PDF/DOCX'in kendisi saklanmaz.
  - **Hiçbir şey uydurulmaz:** ayrıştırma yoksa, yalnız `failed` koşu varsa,
    sayfalar boşsa ya da metin 16 MiB'ı aşarsa metin **yazılmaz** →
    `E_PRECEDENT_SOURCE`. Sayfa numarası/başlık metne eklenmez.
  - **Eksik bilgi gizlenmez:** `ocrPages` (`text_origin IN ('ocr','mixed')`),
    `emptyPages` (birleştirmeye girmeyen), `parseStatus=partial` uyarısı ve
    "metin dosyanın kopyası değil" notu döner.
  - **Tekrar yazım yok:** aynı `body_hash` bu emsalde zaten varsa yeni sürüm
    yazılmaz (`written=false`) ve var olan sürüm döner.
  - Denetime `text_from_document` kaydı düşer: `documentVersionId`,
    `parseRunId`, sayfa/OCR/boş sayfa sayıları, karakter sayısı — **metin
    yazılmaz**.
- `Database::list_versions(document_id)` — belgenin sürümleri en yeniden eskiye;
  arayüz seçimi bu gerçek listeden yapılır ("en yeni sürüm" varsayımı yok,
  varsayılan seçim kutuda görünür).
- Tauri: `precedent_text_from_document`, `document_version_list`.
- Arayüz (`Precedents.tsx` → "Tam metin ve alıntı doğrulama" kartı): **Kasadaki
  belgeden aktar** bölümü — dosya / belge / sürüm seçicileri, aktar düğmesi,
  sonuç kartı (sayfa, karakter, sha256, motor, koşu kimliği, uyarılar, notlar).
  Liste boşsa düğme kapalı ve **nedeni yazılır**.
- Test: 5 yeni storage testi (`precedents::document_text_transfer_*`) +
  `list_versions` sırası; 12 yeni web testi (`precedent-document.test.ts`).
- Commit/CI: `18fef37` → run `40099826596` **7/7 yeşil**.

### Dürüst sınırlama

- Aktarım **çıkarılmış metindir**: OCR sayfalarında yazım hatası, taranmış
  görüntüde hiç metin olmaması mümkündür; ürün kaynak PDF'e bakıp metni
  doğrulamaz.
- Sayfa sınırları çift satır sonudur; **basılı sayfa numarası/başlık metne
  eklenmez**, dolayısıyla alıntı konumu belge sayfasıyla birebir eşlenmez
  (alıntı doğrulaması karakter düzeyindedir).
- Kısmi (`partial`) ayrıştırma "tam" sayılmaz; eksik sayfa olabilir ve bu
  yazılır. Ayrıştırma yoksa kullanıcı önce belgeyi ayrıştırmalıdır.
- Tauri penceresi bu ortamda açılmadı: akış typecheck + birim testleri +
  üretim derlemesiyle doğrulandı; gerçek tıklama ve görsel QA yok.

## F12-T14 — Metin içi alıntı denetimi

İddia, özet veya not metninin içine gömülü alıntılar artık ayrı bir çekirdek
komutla dosya belgelerinde aranır. Girdi **serbest metindir**; kullanıcı alıntıyı
tek tek işaretlemez, metindeki tırnaklı pasajlar çıkarılır.

- **Çekirdek (`crates/storage/src/intext_quotes.rs`):**
  `Database::check_in_text_quotes(case_id, InTextQuoteInput { text, document_id })`.
  - **Çıkarım:** yalnız çift tırnak ailesi — `“…”`, `„…“`, `«…»`, `"…"`.
    Tek tırnak/apostrof sınır değildir (`İstanbul'da` alıntı sayılmaz). En az
    **8** karakter (daha kısa alıntı yanlış kesinlik üretir), en çok 1.000
    karakter; kapanmayan/kısa/uzun tırnaklar **sayılır**, alıntı üretilmez.
  - **Kapsam:** `document_id` yoksa dosyadaki bütün belgeler, varsa yalnız o
    belge. Her belgenin **güncel sürümünün en yeni** `succeeded`/`partial`
    ayrıştırması taranır; ayrıştırması olmayan belge sayılır ve yazılır. Güncel
    sürümde bulunamayan alıntı için belgenin **eski sürümleri** de taranır
    (2026-10-27 genişletmesi, aşağıda).
  - **Üç eşleşme katmanı ayrı:** `exact` (birebir) → `whitespace` (yalnız
    boşluk/satır farkı) → `turkish_fold` (harf sadeleştirmesi). Üst katman
    bulunduysa alt katman aranmaz; hangi katmanın tuttuğu konumda yazar.
  - **Durumlar:** `exact` (tek konum), `normalized` (birebir değil, tek konum),
    `ambiguous` (birden çok konum; `occurrences` ve `distinctDocuments`),
    `not_found`, `unverifiable` (`no_text`: taranacak metin yok). “Bulunamadı”
    **uydurma hükmü değildir**.
  - **Konum:** belge/sürüm/parse koşusu/segment, fiziksel sayfa, Unicode scalar
    `charStart`/`charEnd`, `textOrigin` (metin katmanı/OCR) ve **kaynak metnin
    ham hâli** (`sourceText`) döner. Normalize eşleşmede `sourceText` yazılan
    alıntıdan farklıdır ve fark kullanıcıya gösterilir.
  - **Sınırlar:** metin 50.000 karakter, alıntı 30, taranan sayfa 20.000;
    aşılırsa `E_QUOTE_TEXT` / `E_QUOTE_TOO_MANY` / `E_QUOTE_SCOPE` — sessiz
    kesme yok.
- **Arayüz (`Review.tsx` → “Metin içi alıntı denetimi” kartı):** iddia
  listesinden metin seçme veya yapıştırma, kapsam (tüm dosya / tek belge),
  “Alıntıları denetle”, özet satırı, alıntı başına rozet + konum + aday listesi.
  **“Kaynağa bağla”** yalnız `exact` tek konumda açılır; `anchor_create` ile
  gerçek `source_anchors` kaydı oluşturur ve delil bağı listesine düşer.
  Normalize/belirsiz/bulunamadı satırları bağlanmaz (nedeni yazılır).
- **Test:** storage **257** (+18: çıkarım 4, uçtan uca 14); Web **280** (+7).
  `tsc --noEmit`, `vite build`, clippy storage + tauri `-D warnings`, rustfmt
  iki manifest temiz.

### Dürüst sınırlamalar

- Denetim **metin katmanındadır**: OCR'sız taranmış sayfada görünen yazı
  bulunamaz; OCR metni varsa yazım hatası eşleşmeyi düşürebilir.
- Alıntı **sayfa sınırını aşabiliyorsa** bulunur (2026-10-27 genişletmesi, aşağıda);
  ancak sayfa sınırını aşan eşleşme **tek sayfalık anchor'a bağlanmaz** — tek
  segmentlik bir aralık olarak ifade edilemez, kaynak metin gösterilir.
- Sayfa sınırı taraması **sınırlı bir pencere** kullanır (son ~1.000 karakter):
  çok kısa sayfalar üzerinden üç ve daha fazla sayfaya yayılan çok uzun alıntı
  kaçabilir. Tek sayfa içinde bulunan eşleşme her zaman üstündür; sayfa sınırı
  yalnız alıntı başka yerde bulunamazsa aranır.
- Normalize eşleşme anchor'a **bağlanmaz**: kaynak metin farklı olduğu için
  birebir alıntı yerine geçmez; kullanıcı kaynak metni görüp karar verir.
- Eski sürüm taraması **belge başına 5 sürümle** sınırlıdır (yeniden yeniye);
  daha eskiler taranmaz ve durum notlarda yazılır. Eski sürümde bulunan alıntı
  `isCurrentVersion=false` + sürüm numarasıyla işaretlenir: güncel metinde
  geçmiyor demektir, “uydurma” hükmü değildir. Belge seçimi hâlâ güncel sürüme
  çözülür; sürüm seçici yoktur (eski sürümler yalnız arama kapsamına girer).
- Aynı metin birden çok yerde geçiyorsa tek kaynağa bağlanmaz; seçim
  kullanıcıya bırakılır (aday listesi gösterilir).
- Tauri penceresi bu ortamda açılmadı: kart typecheck + birim testleri +
  üretim derlemesiyle doğrulandı; gerçek tıklama ve görsel QA yok.

## F12-T14 genişletmesi — sayfa sınırını aşan alıntı (2026-10-27)

Önceki turun **açık kaydı** “alıntı tek sayfa içinde aranır; iki sayfaya bölünen
cümle bulunamaz (`not_found`)” kapandı: PDF sayfa sonuna denk gelen alıntı artık
bulunuyor ve hangi sayfalar arasında olduğu yazılıyor.

- **Çekirdek (`crates/storage/src/intext_quotes.rs`):** tek sayfa taramasından
  sonra, hâlâ bulunamayan alıntılar için **sayfa sınırı** taraması
  (`scan_page_break_tier`). Ardışık sayfalar tek pencerede birleştirilir (araya
  bir ayırıcı konur); **yalnız iki sayfaya da gerçekten yayılan** eşleşmeler
  kaydedilir — tek sayfa içinde kalanlar birinci taramada bulunduğu için
  yinelenmez, `occurrences` şişmez.
- **Katman sırası korunur:** her katmanda (birebir → boşluk → harf) önce tek
  sayfa, sonra sayfa sınırı taranır. Böylece birebir ama sayfa sonuna denk gelen
  eşleşme, başka yerdeki normalize eşleşmeden **üstün** kalır. Alıntı sayfa
  sonundaki satır sonunu taşıyorsa (`“Karar gerekçesi\nşöyledir”`) katman yine
  `exact` olur.
- **Konum:** `InTextQuotePosition` iki alan kazandı — `endPageIndex` ve
  `spansPages`. Sayfa sınırını aşmayan eşleşmede `endPageIndex == pageIndex` ve
  `spansPages=false`; aşanda `charStart` **ilk** sayfadaki, `charEnd` **son**
  sayfadaki konumdur.
- **Bağlanamaz:** sayfa sınırını aşan konum tek segmentlik anchor olarak ifade
  edilemez; çekirdek reddeder (`E_DB_SETTING`, aralık/uzunluk kapısı), arayüz
  “Kaynağa bağla” düğmesini **açmaz** ve nedenini yazar. “Birebir tek konumda
  bulundu; kaynak bağlanabilir” notu artık yalnız **tek sayfada** birebir
  bulunanları sayar; sayfa sınırını aşanlar ayrı notla bildirilir.
- **Arayüz (`lib/intext.ts`, `Review.tsx`):** `pageSpanLabel` (“s. 3–4”),
  `canBind` içinde `!spansPages` kapısı, `bindHint` (bağlanamama nedeni), kaynak
  metinde satır sonu `⏎` ile gösterilir ve “Sayfa sınırını aşıyor: 3. sayfa → 4.
  sayfa” satırı düşer.
- **Test (gerçek çekirdek, bellek içi kasa):** `intext_quotes` **21** test
  (önceki 18; kapanan “bulunamaz” testi yerine 4 yeni: sayfa aralığı + bağlanmama,
  satır sonu taşıyan `exact`, iki sınırda tekrar → `ambiguous`, harf katmanında
  sayfa sınırı). `cargo test -p dim-storage --lib` **342/342**; clippy
  `-p dim-storage --all-targets -D warnings` temiz; rustfmt iki manifest temiz.
  Arayüz: `tsc --noEmit` temiz, `vitest run` **373/373** (intext 8).
- **DİKKAT (dürüst):** ölçüm **bellek içi kasa + elle yazılmış sayfa metinleri**;
  gerçek PDF/worker ile uçtan uca denenmedi (sayfa sonu boşlukları, tire ile
  bölünmüş kelime kapsam dışı). Pencere ~1.000 karakter; çok kısa sayfalar
  üzerinden üç+ sayfaya yayılan çok uzun alıntı kaçabilir. Tauri penceresi
  açılmadı → yeni satırın görsel QA'si yok. macOS aarch64 + rustc 1.99.1 ile
  ölçüldü.

## F12-T14 genişletmesi — eski sürümlerde arama (2026-10-27)

Sayfa sınırı turunun **açık kaydı** “yalnız güncel sürüm taranır; eski sürümde
kalmış alıntı `not_found` görünür” kapandı. Hukukta bir belgenin önceki nüshası
(uzman raporunun ilk hâli, düzeltilmiş sözleşme) sıkça alıntılanır; belge
güncellendiğinde alıntının sessizce “bulunamadı” olması denetimin en olası
**yanlış negatifiydi**.

- **Çekirdek (`crates/storage/src/intext_quotes.rs`):** yeni
  `resolve_historical_scopes` — her belgenin güncel sürüm **dışındaki**
  sürümleri yeniden eskiye alınır, belge başına `MAX_HISTORICAL_VERSIONS_PER_DOCUMENT`
  (**5**) ile sınırlanır; sınırın ötesinde sürüm varsa belge sayılır ve not
  düşülür (sessiz kesme yok). Kapsam `document_id` verildiğinde yalnız o belgeye
  daralır.
- **Katman sırası korunur ve genişler:** her katmanda (birebir → boşluk → harf)
  önce güncel sürümün tek sayfası, sonra sayfa sınırı, **sonra eski sürümler**
  taranır. Böylece eski sürümdeki **birebir** eşleşme, güncel sürümdeki
  **normalize** eşleşmeden üstün kalır; güncel sürümde bulunan alıntı hiçbir
  zaman eski sürüm taramasına düşmez (`historical_versions_scanned = 0`).
- **Veri modeli:** `InTextQuotePosition` + `versionNo`, `isCurrentVersion`.
  `InTextQuoteCheck` + `versionsScanned`, `historicalVersionsScanned`,
  `foundInOldVersion`. Eski sürüm kapsamları **yalnız** güncel sürümde
  bulunamayan alıntı olduğunda çözülür → sık durumda ek maliyet yok.
- **Bağlama:** `source_anchors` zaten `document_version_id` + `parse_run_id`
  taşıdığı için eski sürümdeki birebir tek konum **bağlanabilir**; arayüz bunu
  “eski sürüm” rozeti + “Kaynak v1 sürümüne bağlanır; alıntı güncel sürümde
  geçmiyor.” satırıyla yazar. Anchor uydurma değil: hangi sürümde doğrulandığı
  kayıtta durur.
- **Arayüz (`lib/intext.ts`, `Review.tsx`):** `positionLabel` eski sürümde
  “· eski sürüm (v1)” ekler; `oldVersionNote` bağlama uyarısını üretir;
  `versionsLabel` “3 sürüm tarandı · 1 eski sürüm” satırını başlıkta gösterir;
  kartta “eski sürüm” rozeti.
- **Sayfa sınırı:** eski sürüm sayfaları da `pages_scanned` sayımına girer;
  toplam 20.000 sayfa sınırını aşacaksa eski sürüm taraması **hiç yapılmaz** ve
  nedeni notlarda yazılır (yarım tarama yok).
- **Test (gerçek çekirdek, bellek içi kasa):** `intext_quotes` **27** test
  (önceki 21 + 6 yeni: güncelde yok/eski sürümde var; eski sürüm konumu
  `create_anchor` ile **gerçekten** bağlanıyor (`quote_intact`); güncel sürüm
  kazanır ve eski sürüm taranmaz; eski sürümdeki `exact` günceldeki `whitespace`i
  yener; belge kapsamı başka belgenin geçmişini taramaz; 5 sürüm sınırı aşılınca
  not düşer). `cargo test -p dim-storage --lib` **348/348**; clippy
  `-p dim-storage --all-targets -D warnings` temiz; rustfmt iki manifest temiz.
  Arayüz: `tsc --noEmit` temiz, `vitest run` **375/375** (intext 10).
- **Dürüst sınırlar:** ölçüm **bellek içi kasa + elle yazılmış sayfa metinleri**;
  gerçek PDF/worker ile uçtan uca denenmedi. Eski sürüm taraması belge başına
  5 sürümle sınırlı; daha eskisi taranmaz. Katman kuralı gereği **eski sürümdeki
  birebir** eşleşme, güncel sürümdeki normalize eşleşmeden üstündür (hangisinin
  raporlandığı `isCurrentVersion` + `versionNo` ile yazılır). Aynı alıntı hem
  güncelde hem eski sürümde geçiyorsa `occurrences` yalnız güncel sürümü sayar
  (yinelenmez); **birden çok eski sürümde** geçiyorsa da tek konum sayılır (en yeni
  sürüm seçilir — 2026-10-28 bölümü, kusur #59). Pencere ~1.064 karakter
  sınırı ve tire ile bölünmüş kelime sınırı aynen sürüyor. Tauri penceresi
  açılmadı → “eski sürüm” rozetinin görsel QA'si yok.

## F12-T14 genişletmesi — tire ile bölünmüş kelime (2026-10-27)

Sayfa sınırı turunun **açık kaydı** “sayfa sonunda tire ile bölünmüş kelime
(`bedel-\lini`) kapsam dışı” kapandı. Hukuk metinlerinde iki yana yaslı dizgi ve
OCR, kelimeyi satır sonunda tireyle böler; avukat alıntıyı doğal olarak
`bedellini` diye yazar. Bu durumda alıntı **sessizce** `not_found` oluyordu —
sayfa sınırı ve eski sürüm turlarında kapatılan sınıfın aynısı: **yanlış
negatif**.

- **Çekirdek (`crates/storage/src/precedents.rs` → `dehyphenate_with_map`):**
  harf sadeleştirmesi + iki kural.
  - **Tire + tek satır sonu + harf/rakam** → tire ve satır sonu atılır, kelime
    birleşir (`bedel-\nlini` → `bedellini`; `\r\n` de tek satır sonu sayılır).
  - **Yumuşak tire** (`U+00AD`) her yerde atılır: görünmez, yazım farkı değildir.
  - **Yalnız kanıtlı bölünme:** tire ile kelime arasında satır sonu yoksa
    (`kredi-kartı`) tire **korunur**; iki ve daha çok satır sonu (paragraf/sayfa
    ayrımı) birleştirilmez. Böylece “kredikartı” gibi uydurma birleştirme
    yapılmaz — bu kural testle tutuluyor.
- **Yeni katman `MatchTier::Dehyphenated` (`kind = "dehyphenated"`):** en
  **zayıf** katman, `Fold`'dan sonra taranır. Katman sırası korunur: birebir →
  boşluk → harf → tire birleştirme; her katmanda önce güncel sürümün tek sayfası,
  sonra sayfa sınırı, sonra eski sürümler. Yani güncel sürümdeki harf farkı
  eşleşmesi, eski sürümdeki tire birleştirmesinden üstündür.
- **Veri modeli:** `InTextQuotePosition.matchKind = "dehyphenated"`;
  `InTextQuoteCheck` + `dehyphenated` (yalnız bu katmanda bulunan alıntı sayısı).
  Notlar: “N alıntı yalnız **tire ile bölünmüş kelime** birleştirilerek bulundu
  (kaynakta `kelime-\nsatır` gibi); birebir sayılmaz ve anchor'a bağlanmaz,
  kaynak metin gösterilir.”
- **Bağlama kapısı:** birebir olmadığı için `canBind` **false**; `bindHint`
  nedeni yazar (“…tire ile bölünmüş kelime birleştirilerek bulundu; birebir
  sayılmaz, kaynağa bağlanmaz.”). Anchor `source_text`'i kaynak aralığından
  üretildiği için tire + satır sonu **gerçekten** görünür (uydurulmuş düz metin
  yok).
- **Arayüz (`lib/intext.ts`):** `MATCH_KIND_LABEL.dehyphenated = "tire
  birleştirme"`, özet satırında yalnız sayı sıfırdan büyükse “N tire birleştirme”
  (gürültü yok), `bindHint` tire nedeni, `candidateLabel` aynı etiket.
- **Test (gerçek çekirdek, bellek içi kasa):** `intext_quotes` **33** test
  (önceki 27 + 6 yeni: satır sonu tiresi bulunur + konum kaynağı tireyi gösterir;
  yumuşak tire fark değildir; **gerçek tire birleştirilmez**; sayfa sınırındaki
  tire bulunur ve `spansPages=true`; paragraf ayrımındaki tire birleştirilmez;
  birebir eşleşme en güçlü katmanda kalır). `precedents` +1 birim testi
  (`dehyphenation_joins_only_a_word_broken_at_one_line_end`: CRLF, gerçek tire,
  iki satır sonu, yumuşak tire, metin sonu tiresi).
- **Dürüst sınırlar:**
  - Ölçüm **bellek içi kasa + elle yazılmış sayfa metinleri**; gerçek PDF/worker
    ile uçtan uca denenmedi (taranmış/OCR metinde tire karakteri `-` yerine
    `–`/`—` gelirse eşleşmez — yalnız `-` tanınır).
  - Alıntı **da** tireyle yazılmışsa (`bedel-lini`) ve kaynakta satır sonu yoksa
    birleştirme yapılmaz (gerçek tire kabul edilir); iki yön aynı kurala tabidir.
  - Kelime ortasındaki tire kaynakta **gerçekten** satır sonundaysa (ör.
    `Türk-\nAlman`) birleştirilir; bu, satır sonu tiresinden ayırt edilemez
    (tasarım gereği, katman en zayıf ve bağlanmaz).
  - Tauri penceresi açılmadı → etiketin görsel QA'si yok.

## F12-T05 genişletmesi — tire ile bölünmüş kelime + gerçek kaynak aralığı (2026-10-27)

**Neden:** son dört turda **belge** alıntılarında kapatılan yanlış negatif sınıfı
(sayfa sınırı, eski sürüm, satır sonu tiresi) emsal **tam metninde** açıktı.
`check_in_text_quotes` `bedel-\nlini` yazılmış metinde `bedellini` alıntısını
bulurken aynı alıntı `verify_precedent_quote`'da `mismatch` (“bulunamadı”)
dönüyordu: iki amiral gemisi özellik aynı olguda farklı hüküm veriyordu. Karar
metinleri çoğunlukla iki yana yaslı PDF'ten geldiği için bu tipografi beklenen
durumdur.

**Ölçülen ikinci kusur (aynı turda):** `matched_length` her katmanda **alıntının**
karakter sayısıydı. Boşluk katlama ve tire birleştirmede kaynak aralığı farklıdır
(ölçüm: kaynak `ödeme    yapılmadığı` = 20 karakter, alıntı 17). `citation_context`
“sonrası” dilimini bu yanlış uzunlukla başlattığı için bağlam **kelimenin
ortasından** başlıyordu. Artık `matched_length` **kaynak aralığıdır**.

**Çekirdek (`crates/storage/src/precedents.rs`):**

- `mapped_span` — eşleşmenin kaynak metindeki (başlangıç, uzunluk) aralığı;
  `map` her çıktı karakterinin kaynak indeksini taşıdığı için tire, satır sonu ve
  katlanan boşluklar aralığa **dahil** edilir.
- Katman sırası: `exact` → `whitespace` → `turkish_fold` → **`dehyphenated`**
  (en zayıf). `dehyphenate_with_map` aynı kuralları uygular: `-` + **tek** satır
  sonu + harf birleşir; satır sonu yoksa gerçek tire korunur; iki+ satır sonu
  paragraf ayrımıdır, birleşmez; yumuşak tire (`U+00AD`) atılır.
- `citation_context` artık `matched_length`'i kullanır (yoksa alıntı uzunluğuna
  düşer) — “sonrası” bağlam eşleşmenin **bittiği** yerden başlar.

**Arayüz:** `QuoteVerificationRow.method` birleşimine `"dehyphenated"`;
`quoteStateText` “yalnız satır sonunda tire ile bölünmüş kelime birleştirilerek
bulundu … Birebir değil, gözle doğrulayın.”; ton `warn` (birebir sayılmaz).

**Test (gerçek çekirdek, bellek içi kasa):** `precedents` **39** (34 + 5 yeni:
satır sonu tiresi bulunur + kaynak aralığı 32; gerçek tire birleştirilmez;
yumuşak tire fark değildir; birebir eşleşme en güçlü katmanda kalır;
`citation_context` kaynak aralığını kullanır — boşluk katlamalı ve tireli iki
vaka). Önce **3 test kırmızı** koştu (dehyphenated ve yumuşak tire `mismatch`;
aralık 17).

**Dürüst sınırlar:**

- Yalnız ASCII `-` tanınır; OCR `–`/`—` üretirse birleşmez.
- Alıntı **da** tireyle yazılmışsa ve kaynakta satır sonu yoksa birleştirme
  yapılmaz (gerçek tire sayılır; iki yön aynı kural).
- Yalnız **en yeni** tam metin sürümü taranır; eski sürümde kalmış alıntı hâlâ
  `mismatch` — belge tarafında kapatılan sürüm sınıfı emsal tarafında **açık
  kayıt**.
- Tire birleştirme `method=dehyphenated` ile raporlanır ve `quote_match`
  boyutuna `match` olarak taşınır (bugünkü `whitespace`/`turkish_fold` gibi);
  birebir olmadığı arayüzde yazılır.
- Ölçüm bellek içi kasa + elle yazılmış metinler; gerçek PDF/worker uçtan uca
  denenmedi. Tauri penceresi açılmadı → etiketin görsel QA'si yok.

## F12-T05 genişletmesi — emsal tam metninde **eski sürüm** taraması (2026-10-27, ikinci tur)

**Neden:** önceki turun açık kaydı: “yalnız **en yeni** tam metin sürümü taranır;
eski sürümde kalmış alıntı hâlâ `mismatch` — belge tarafında kapatılan sürüm sınıfı
emsal tarafında açık”. Aynı sınıf `check_in_text_quotes`'ta 25 Eki'de kapatılmıştı
(belge güncellenince eski nüshadan alınan alıntı “bulunamadı” görünüyordu). Emsalde
nüsha değişimi daha sık: aynı kararın düzeltilmiş/uzun nüshası yüklenir, kullanıcı
alıntıyı önceki nüshadan almış olabilir. “Bulunamadı” hükmü bu durumda yanlış negatifti.

**Çekirdek (`crates/storage/src/precedents.rs`):**

- `quote_scopes(precedent_id)` — en yeni **metinli** sürüm `is_current=true`,
  ardından eski sürümler yeniden eskiye `MAX_HISTORICAL_VERSIONS_PER_PRECEDENT`
  (**5**) ile sınırlı; sınır aşılırsa ikinci dönüş `true` (`historicalTruncated`).
  Metinsiz sürümler kapsama girmez (güncel seçimi de metne göre yapılır).
- Eşleşme döngüsü **katman-major**: `exact → whitespace → turkish_fold →
  dehyphenated`; her katmanda önce güncel sürüm, sonra eskiler. Yani eski sürümdeki
  **birebir** eşleşme, günceldeki zayıf (boşluk/harf/tire) eşleşmesini yener; aynı
  katmanda güncel sürüm kazanır. Sıra belge tarafıyla birebir aynı.
- `find_in_scope` tek sürümde tek katmanı arar; konum/uzunluk **kaynak metnin**
  karakter cinsindendir (`mapped_span`), `occurrences` yalnız **eşleşen** sürümün
  metninden sayılır. `version_id` eşleşen sürümü gösterir → `citation_context`
  bağlamı **o sürümün** metninden alır.
- Tembel: güncel sürümde birebir bulunan alıntı için eski sürüm hiç taranmaz
  (`historicalVersionsScanned=0`); normalizasyon katman başına sürüm başına bir kez.

**Veri modeli:** `QuoteVerification` + `versions_scanned` (bakılan **farklı** sürüm
sayısı: güncel + eşleşmeden öncekiler), `historical_versions_scanned`,
`found_in_old_version`, `historical_truncated`. `mismatch` artık kaç sürümün
tarandığını söyler; kesme varsa sonuç **kesin değildir** ve bu yazılır.

**Arayüz:** `quoteStateText` eski sürüm eşleşmesini “**eski bir sürümde** birebir
bulundu … Güncel tam metinde geçmiyor; karar metni güncellenmiş olabilir.” diye
yazar; `quoteStateTone` bu durumda **birebir olsa da** `warn` döner. Yeni
`quoteScanNote` “3 sürüm tarandı · 2 eski sürüm [· daha eski sürümler taranmadı]”
satırını üretir; `mismatch` metni taranan sürüm sayısını ve kesmeyi taşır.
“Alıntıyı doğrula” düğmesi artık `latestHasText` yerine `withText > 0` ile açılır:
en yeni sürüm metinsiz olsa da eski metinli sürüm doğrulanabilir.

**Test (gerçek çekirdek, bellek içi kasa):** `precedents` **45** (39 + 6 yeni:
yalnız eski sürümde bulunur + sürüm kimliği; eski sürümdeki `exact` günceldeki
`whitespace`i yener; alıntı iki sürümde de varsa güncel kazanır ve `occurrences`
güncelden sayılır; 5 sürüm sınırı + `historical_truncated` + sınır içindeki alıntı
yine bulunur; `citation_context` **eşleşen** sürümün metninden bağlam verir;
taranmayan durumlarda sayaç 0). Önce **3 test kırmızı** koştu (`mismatch` 2,
`whitespace` 1). `cargo test -p dim-storage --lib` **366/366** (178 sn); clippy
`-p dim-storage --all-targets -D warnings` temiz; rustfmt iki manifest temiz.
Arayüz: `tsc --noEmit` temiz (node 22.23.1), `vitest run` **379/379** (28 dosya),
`vite build` temiz. `precedents.test.ts` 11 → 13 test.

**Dürüst sınırlar:**

- Tarama emsal başına **5 eski sürümle** sınırlı; daha eskisi taranmaz
  (`historicalTruncated=true` ile bildirilir) — sessiz kesme yok ama sonuç kesin
  değildir.
- Eski sürümdeki eşleşme **bugünkü metni doğrulamaz**: `quote_match` boyutuna
  `match` taşınır (belge tarafındaki sürüm davranışıyla aynı), arayüz ayrıca uyarır.
- Katman-major sıra bilinçli: eski nüshadaki birebir eşleşme günceldeki zayıf
  eşleşmeden **daha güçlü kanıt** sayılır. Tersi tercih edilirse sıra değişmeli.
- Tüm sürümler belleğe alınır (en çok 6 metin); emsal tam metni üst sınırı 16 MiB
  olduğundan teorik tepe ~96 MiB — ölçülmedi.
- Ölçüm bellek içi kasa + elle yazılmış metinler; gerçek PDF/worker uçtan uca
  denenmedi. Tauri penceresi açılmadı → yeni satırın görsel QA'si yok.

## Alıntı sayımında sürüm kopyası birleştirmesi (2026-10-28, kusur #59)

**Ölçülen kusur (işlevsel, yanlış hüküm):** belge tarafı (`check_in_text_quotes`)
aynı pasajı bir belgenin **birden çok sürümünde** bulunca her nüshayı ayrı konum
sayıyordu. Ölçüm (bellek içi kasa, önce kırmızı koşan test): v1 ve v2 birebir aynı
cümleyi taşıyor, güncel v3'te cümle yok → `ambiguous` **1** · `occurrences` **2** ·
`position` **null**. Avukat “alıntı birden fazla konumda geçiyor, seçin” görüyordu;
oysa tek bir pasajın iki nüshası vardı ve **hiçbiri güncel sürümde değildi**. Dahası
“yalnız eski sürümde” uyarısı hiç çıkmıyordu (`found_in_old_version=0`), çünkü o
sayaç yalnız **tek konum** varken artıyordu — yani en önemli olgu (alıntı güncel
metinde geçmiyor) tam da belirsiz durumda kayboluyordu. Emsal tarafı
(`verify_precedent_quote`) aynı olguda **tek** konum ve `occurrences=1` döndürüyordu:
iki amiral gemisi aynı olguda farklı hüküm veriyordu.

**Düzeltme (`crates/storage/src/intext_quotes.rs`):**

- `QuoteAccum` artık (belge, sürüm) başına eşleşme sayısı tutar (`VersionHit`).
- `resolve_version_copies()` belge başına **en yeni** sürümü seçer (güncel varsa
  güncel, yoksa en yüksek `version_no`); `occurrences` yalnız seçilen sürümün
  **içindeki** konumları sayar, aday listesi kazanan sürümün konumlarından gelir.
  Ham sayı yalnız “bulundu mu” kapısında (`pending_quotes`) kalır; katman sırası ve
  tembel eski-sürüm taraması **değişmedi**.
- Eşleşme yalnız eski sürümdeyse `found_in_old_version` artık **belirsiz eşleşmede
  de** artar; sürüm kopyası birleştirildiğinde not düşülür (“N alıntı aynı belgenin
  birden çok sürümünde geçiyordu; sürüm kopyaları ayrı konum sayılmadı, **en yeni**
  sürümdeki konum kullanıldı”). “Yalnız eski sürümde” notu artık “belirsiz eşleşme
  bağlanmaz” diye tamamlanır.

**Arayüz (`lib/intext.ts` + `Review.tsx`):** `oldVersionNote` konum yokken de çalışır —
adayların **hepsi** eski sürümdeyse “Alıntı güncel sürümde geçmiyor; yalnız v2
sürümünde 2 konumda bulundu.” yazar; adaylardan biri güncel sürümdeyse susar. Önceden
bu olgu yalnız katlanmış “Denetimin sınırları” notunda kalıyordu. `Review.tsx` satırı
da `position?.isCurrentVersion === false` koşulu yerine **notun kendisine** bakar;
yoksa belirsiz eşleşmede uyarı hiç görünmezdi (ölçülen ikinci kusur: yeni metin
yazılmış ama render kapısı eski koşulda kalmıştı).

**Test:** önce **2 test kırmızı** koştu (`ambiguous` 1 → 0 bekleniyordu;
`found_in_old_version` 0 → 1). `intext_quotes` **35** (33 + 2: “iki eski sürüm tek
konum”, “belirsiz ama yalnız eski sürümde”), `precedents` **46** (+1 eşlik testi: aynı
olguda emsal tarafı en yeni eşleşen sürümü seçer, `occurrences=1`),
`cargo test -p dim-storage --lib` **372/372** (184 sn); clippy
`-p dim-storage --all-targets -D warnings` temiz; rustfmt iki manifest temiz.
Arayüz: `tsc --noEmit` temiz (node 22.23.1), `vitest run` **414/414** (30 dosya;
`intext.test.ts` +1), `vite build` temiz (83 modül, 486,04 kB).

**Dürüst sınırlar:** seçim **en yeni** sürümle sınırlı — daha eski nüshada kalan
farklı bir metin ayrı konum olarak raporlanmaz (aynı pasajın nüshası sayılır);
kullanıcı eski nüshayı görmek isterse belge geçmişini açmalı. Sayım seçilen sürümün
**içindeki** tekrarları sayar (aynı sayfada 3 kez → 3, `ambiguous` korunur). Konum
kapağı 8; `occurrences` kapağa takılmaz. Ölçüm bellek içi kasa + elle yazılmış
sayfa metinleri; gerçek PDF/worker uçtan uca denenmedi. Tauri penceresi açılmadı →
yeni uyarı satırının görsel QA'si yok.

**Commit/CI:** `bad747c` (çekirdek + test + arayüz + belge) → CI **`40730120844` 7/7
yeşil**; `6628f23` (render kapısı + belge) → CI **`40731762280` 7/7 yeşil**
(sözleşmeler, arayüz tsc+vitest+build, rust macos-14 + ubuntu-latest, masaüstü kabuk,
belge worker'ı, SBOM). `774a650` (yalnız belge) koşusu yeni push'un eşzamanlılık
grubuyla **cancelled** oldu — yeşil olan koşu HEAD'i kapsar.

## Gerçek PDF düzeniyle uçtan uca alıntı doğrulaması (2026-10-30, kusur #74–#75)

**Neden:** F12'nin her ölçümü **elle yazılmış sayfa metinleriyle** koşuyordu; dürüst
sınır olarak “gerçek PDF/worker ile uçtan uca denenmedi” yazılıydı. Bu tur o boşluk
kapatıldı ve **iki gerçek yanlış negatif** ölçüldü.

**Fikstür ve kayıt.** `fixtures/synthetic/intext-layout.pdf` — 3 sayfa, **74.295 bayt**,
gömülü Unicode font (MuPDF `Story` hattı; Türkçe diyakritik ve tipografik tırnak
korunur). Vakalar: sayfa içi satır sonu tiresi (`bedel-\nlini`), sayfa sonunda tire
(`karar ve-` → `rilmiştir`), tiresiz sayfa sınırı (`gerekçesi` → `şöyledir`) ve font
ligatürü (`taraﬂar`, U+FB02). Üretici `fixtures/synthetic/generate_intext.py`; aynı
komut ürün worker'ının çıktısını `fixtures/synthetic/intext-layout.extracted.json`
olarak yazar (PyMuPDF 1.24.14). Rust testi CI'da bu **kaydı** yükler; canlı çıkarımla
birebirliğini `#[ignore]` testi ölçer.

**ÖLÇÜLEN KUSUR #74 (yanlış negatif, sayfa sonu tiresi).** Sayfa metni `… karar ve-\n`
ile bitiyordu; sayfa-sınırı penceresi araya **bir `\n` daha** koyuyordu → tire kuralı
iki satır sonunu “paragraf ayrımı” sayıp birleştirmiyordu. Ölçüm (gerçek PDF + gerçek
worker): `“reddine karar verilmiştir”` → **`not_found`** (alıntı metinde duruyor).
Düzeltme: ayırıcı yalnız taşınan metin satır sonuyla **bitmiyorsa** eklenir; sınır,
yeni sayfanın ilk karakterinin indeksidir (`end > boundary`).

**ÖLÇÜLEN KUSUR #75 (yanlış negatif, ligatür).** Font `fl` çiftini tek glif yazıyor;
PyMuPDF `U+FB02` döndürüyor. `normalize` ligatürü açmadığı için avukatın `taraflar`
yazdığı alıntı **hiçbir katmanda** bulunmuyordu. Ölçüm: `“taraflar arasındaki
sözleşme”` → **`not_found`**. Düzeltme: (a) `dim_evidence::normalize` ligatürleri
açar (`U+FB00–U+FB06`), (b) belge tarafında **`ligature`** katmanı eklendi
(`exact → whitespace → ligature → turkish_fold → dehyphenated`), (c) emsal tarafında
aynı katman (`QuoteTier::Ligature`), (d) arayüz etiketi + sayaç + neden satırı.

**Kanıt (gerçek çıkarım, 7 alıntı — `crates/storage/tests/intext_real_layout.rs`):**

| Alıntı | Hüküm | Katman | Sayfa |
|---|---|---|---|
| `KARAR GEREKÇESİ` | exact | exact | 1 |
| `bedellini ödemediğini` | normalized | dehyphenated | 1 |
| `reddine karar verilmiştir` | normalized | dehyphenated | 1→2 (sınırı aşıyor) |
| `gerekçesi şöyledir` | normalized | whitespace | 2→3 (sınırı aşıyor) |
| `ödeme yapılmadığı sabit olup` | normalized | whitespace | 2 |
| `taraflar arasındaki sözleşme` | normalized | **ligature** | 3 |
| `faiz işletilmesine yer vardır` | not_found | — | — |

Sayaçlar: `exact=1 · normalized=5 · ambiguous=0 · not_found=1 · dehyphenated=2 ·
ligature=1`. Birebir tek konum gerçekten bağlanıyor (`create_anchor` →
`locate_anchor`, `quote_intact`, s. 1); ligatür ve tire eşleşmeleri bağlanmıyor
(kaynak metin ham hâli gösterir: `bedel-\nlini ödemediğini`, `taraﬂar…`).

**Testler:** `intext_quotes` **37** (+2: ligatür katmanı, sayfa sonu tiresi),
`precedents` **47** (+1: karar metninde ligatür, kaynak aralık ligatürü **tek
karakter** kapsar), `dim-evidence` **8** (+1: `normalize` ligatür açma),
`crates/storage/tests/intext_real_layout.rs` **1/1** (CI'da kayıtla koşar).
Canlı ölçüm (elle): `DIM_WORKER_PYTHON=… cargo test -p dim-ingest --test
intext_layout_live -- --ignored --nocapture` → canlı çıkarım kayıtla **birebir**.
Arayüz: `tsc --noEmit` temiz, `vitest run` **437/437** (34 dosya; +2 test).

**Dürüst sınırlar:** tek font (MuPDF'in gömülü serif'i) ve tek düzen; gerçek
tarama/eğim/çok kolon denenmedi. Ligatür listesi `U+FB00–U+FB06` ile sınırlı
(başka sunum karakterleri — ör. `U+2019` tırnak, `U+00A0` boşluk — kapsam dışı;
boşluk katmanı NBSP'yi zaten tekler). Tire kuralı **iki satır sonu**nu hâlâ paragraf
ayrımı sayar: gerçek paragraf sonunda tireyle bölünmüş kelime birleşmez (bilinçli,
yanlış pozitif kapısı). Sayfa-sınırı penceresi ~1.064 karakter taşır. Canlı test
Python/PyMuPDF ister (CI'da `#[ignore]`); kayıt PyMuPDF sürümüne bağlıdır — sürüm
değişince üretici yeniden koşmalıdır, aksi hâlde canlı test **düşer**. Tauri
penceresi açılmadı → yeni etiketin görsel QA'si yok; Windows denenmedi.

## Alıntı hükmü metne bağlanır (2026-11-03)

Emsal ekranının alıntı doğrulama kartı 2026-11-01'de davranış testine alınmıştı;
açık kayıt **`quoteInput` değişince önceki doğrulama sonucunun silinmemesiydi**.
Bu turda kayıt kapatıldı ve aynı sınıfın iki yarışı ölçüldü.

**Yazılanlar — `src/screens/Precedents.test.tsx` (+3 test; 15 → 18):** mevcut
harness (jsdom + RTL, `lib/api` kısmi mock) korundu. Sabitlenenler:
(1) başarılı doğrulamadan sonra alıntı metni değişince hüküm **ve** “atıf alanına
taşı” düğmesi kalkar; düzenleme kendiliğinden yeni istek üretmez; alan
boşaltılınca hüküm geri gelmez ve “Doğrula” kapalıdır; (2) doğrulama uçuşta
iken metin değişirse geç yanıt yeni metnin hükmü gibi yazılmaz, kilit açılır ve
yeni metin kendi isteğiyle doğrulanır; (3) doğrulama uçuşta iken başka emsal
seçilirse eski emsalin hükmü yeni emsalin (boş) alanında görünmez.

**Ölçülen kusurlar — ham koşu (düzeltme öncesi bileşen): 3 failed | 15 passed.**
- **Bayat hüküm + yanlış atıf (asıl kayıt):** alıntı alanı her değiştiğinde
  `verification` silinmiyordu. Kullanıcı yeni bir cümle yazdığında ekranda
  **önceki cümlenin yeşil “birebir bulundu”** satırı ve “Atıf alanına taşı”
  düğmesi kalıyordu; düğme, **doğrulanmamış** yeni metnin hükmünü atıf
  kontrolünün `quote_match` alanına yazabiliyordu.
- **Geç yanıt yeni metne yazılıyordu:** `verifyQuote` yanıtı istek anındaki
  metne bağlamıyordu. İstek askıdayken metin değişince dönen hüküm yeni metnin
  altında “güncel” gibi görünüyordu (yanlış alıntıya atıf taşıma riski).
- **Emsal değişiminde eski hüküm (INV-05):** seçim değişince `verification`
  sıfırlanıyordu ama uçuştaki yanıt sonradan yazıldığı için **önceki emsalin
  hükmü** yeni emsalin alıntı alanında beliriyordu (alan boşken bile).

**Düzeltme (`Precedents.tsx` +24/−5):** `quoteEdit` (her metin/emsal
değişiminde artar) ve `quoteReq` (her doğrulama isteğinde artar) sayaçları;
metin `onChange`’i `setVerification(null)` + sayaç artışı yapar; emsal değişimi
de sayacı artırır; yanıt yalnız **en son istek** ve **aynı metin** için yazılır;
kilidi yalnız en son istek açar. Hata bildirimi metin düzenlenince silinmez
(gerçekleşmiş bir denemenin bildirimi; sonraki doğrulama siler).

**Kapı kanıtı (3 mutasyon; her biri kendi testini düşürdü; geri alma sha256
`921fcb75…d33b0` birebir):** M1 `onChange`’deki `setVerification(null)` →
1 kırmızı; M2 yanıt yazımındaki `edit` kapısı → 2 kırmızı (metin + emsal
mutasyonu); M3 emsal değişimindeki `quoteEdit` artışı → 1 kırmızı. **Dürüst:**
ilk denemede M3 yeşil kaldı — test 3 yeni emsalde alanı doldurduğu için
`onChange` sayacı zaten artıyordu; test, alanı **boş bırakıp** yanıtı `act` ile
akıtacak şekilde düzeltildi, M3 de kırmızı verdi.

**Ölçüm:** `tsc --noEmit` temiz · `vitest` **801/801** (55 dosya; 798 → +3) ·
`pnpm build` temiz (**575,01 kB**, `index-N8N1uwex.js`, gzip **154,88 kB**).

**Dürüst sınırlar:** jsdom **tarayıcı değil** — CSS, gerçek Tauri penceresi,
gerçek `invoke`, yazma hızı ve **görsel QA yok**; `api` mock’lu olduğu için
`verify_precedent_quote` SQL’i ve dört katmanlı eşleştirme **koşmaz**. Hüküm
metinle (sayaçlarla) bağlanır, karakter aralığıyla değil; aynı metni yeniden
yazmak hükmü tazeler (yeniden “Doğrula” gerekir). Kapsam dışı: kopyala/yapıştır
ile metin değişimi (aynı `onChange`), IME, klavye, Windows.

## Türetilmiş okumalar seçim nesline bağlanır (2026-11-03)

Emsal ekranı 1497 satırdı ve 18 testi yalnız **kütüphane/künye/izin/atıf**
kararlarını ölçüyordu; türetilmiş okumaların **hangi emsale/dosyaya/sürüme**
yazıldığı ölçülmemişti. Yeni `describe` 15 test ekledi (18 → 33).

**Ölçülen kusurlar — ham koşu (düzeltme öncesi bileşen): 10 failed | 18 passed.**

- **Geç yanıt yeni seçimin verisini eziyordu (dört okuma, tek kök):** notlar,
  sürümler, künye ve **koşul kaydı** istekleri yalnız `selected` üzerinden
  yazıyordu, nesil kapısı yoktu. A seçilip B'ye geçildiğinde A'nın geç yanıtı
  B'nin altında çiziliyor; **koşul formu A'nın izin kaydıyla doluyor** ve
  “Koşulu kaydet” o kaydı **B'ye** yazabiliyordu (rozet/kapsam yalanı).
- **Belge listesi (dosya):** geç yanıt `docs` + `versionId`'yi eziyor →
  “Kontrolü kaydet” **eski dosyanın** belge sürümüne yazıyordu (INV-05,
  yalnız görsel değil **kapsam** ihlali).
- **Kasadan aktarım zinciri (iki halka):** geç gelen kaynak dosya listesi
  `srcDocId`'yi, geç gelen sürüm listesi `srcVersionId`'yi eziyor →
  `precedentTextFromDocument` **başka dosyanın belgesinden** metin alabiliyordu
  (kaynak izlenebilirliği — F12'nin çekirdek iddiası).
- **Atıf kontrol listesi:** eski sürümün kontrolleri yeni sürümün altında
  görünüyordu.
- **Anlam bağlamı:** A kontrolünün bağlamı B'nin panelinde “B'nin alıntısı”
  gibi çiziliyordu (hüküm yanlış metne bakılarak verilir).
- **Yazma sonrası tazelemeler (beş yol):** not, tam metin, aktarım, koşul ve
  anlam hükmü yanıtları seçim değiştikten sonra yeni emsalin kartlarını
  yazıyordu; not yazımı ayrıca **uçuşta yazılan yeni metni** siliyordu.

**Düzeltme (`Precedents.tsx` +160/−58):** altı kimlik ref'i (`selectedRef`,
`caseRef`, `srcCaseRef`, `srcDocRef`, `versionRef`, `meaningForRef`) + iki yeni
sayaç (`meaningSeq`, `citationRef`; `quoteReq` zaten vardı); her okuma yanıtı
**yalnız kendi kimliği hâlâ seçiliyse** yazar (hata yolu dâhil), her yazma
sonrası tazeleme aynı kapıdan geçer; anlam paneli sıra + açık kimlik kapısı
taşır; `addCheck` yalnız kullanıcının yazdığı metin **değişmediyse** alanı
temizler.

**Kapı kanıtı (16 mutasyon; her biri `1 failed | 32 passed`, geri alma sha256
`0b7a07f69e93…9cca43` birebir):** M1–M4 dört seçim okuması, M5 belge listesi,
M6–M7 aktarım zinciri, M8 kontrol listesi, M9 anlam bağlamı, M10–M11 not
tazeleme + taslak koruma, M12 atıf metni koruma, M13 anlam hükmü tazeleme,
M14 aktarım sonucu, M15 koşul formu, M16 tam metin yanıtı.

**Ölçüm:** `tsc --noEmit` temiz · `vitest` **856/856** (57 dosya; 841 → +15) ·
`npm run build` temiz (**577,71 kB**, `index-Bdwv-vK5.js`, gzip **155,74 kB**).

**Dürüst sınırlar:** jsdom **tarayıcı değil** — CSS, gerçek Tauri penceresi,
gerçek `invoke` ve **görsel QA yok**; `api` mock'lu olduğu için
`precedent_note_list`, `precedent_terms_get/set`, `document_list`,
`precedent_text_from_document`, `citation_check_list`, `citation_context` SQL'i
ve Rust **koşmaz**. Kapılar **kimlik** karşılaştırır (nesil sayacı değil): aynı
emset yeniden seçilirse eski istek ayırt edilmez (pratikte aynı veri döner).
Not besteci alanları (`n-sim`/`n-dist`/`n-conc`) emsal değişiminde
**temizlenmez** — kullanıcı girdisi korunur; A için yazılan metin B'de duruyorsa
kullanıcı görür (sessiz yazım yok), ama bu bir **açık kayıt**tır.
`addFullText` uçuşta aynı emsalde yazılan ek metni temizler (ölçülmedi).
Kapsam dışı: klavye, Windows, gerçek zamanlama.

## Emsal alıntısı kopyalama kendi sonucuna bağlanır (2026-11-03)

Kütüphane panelindeki “Alıntıyı kopyala” yolu bugüne dek **hiç ölçülmemişti**:
`copySnippet` panoya yazıyor, düğme “Kopyalandı” oluyordu; etiketin **hangi**
sonuca ait olduğu, yeni aramada taşınıp taşınmadığı ve uçuştaki yanıtların
nereye yazıldığı ölçülmemişti. Lib testleri (`search-scope`,
`i18n-bindings`) yalnız kopyalanacak **metni** doğruluyordu.

**Ölçülen kusurlar — ham koşu (düzeltme öncesi bileşen): 7 failed | 23 passed
(30 test).**

- **Bayat etiket:** `copiedHit` yalnız kapsam değişiminde temizleniyordu; yeni
  kütüphane aramasında aynı sürüm yine “Kopyalandı” görünüyordu — oysa o kopya
  **önceki** sonuç kümesinindi.
- **Geç yanıt:** kopya uçuşta iken yeni arama başlarsa, dönen kopya **yeni**
  sonuç kümesinin etiketini yazıyordu.
- **Sıra:** iki sonuca hızlı kopyada geç çözülen **ilk** kopya, son tıklananın
  etiketini eziyordu (pano ikinci metni taşırken etiket birincide kalıyordu).
- **Bayat yeşil:** düşen kopya önceki “Kopyalandı” satırını bırakıyordu; kırmızı
  `E_CLIPBOARD` ile yeşil etiket aynı anda ekranda duruyordu.
- **Geç hata (iki bağlam):** kapsam değişince ya da yeni arama başlayınca düşen
  kopya, o bağlama `E_CLIPBOARD` yazıyordu (dosya kapsamında kopyalama
  yapılmadığı hâlde clipboard hatası).

**Düzeltme (`Search.tsx` +16/−3):** tek `copySeq` sayacı; kopya denemesi
başında etiket kaldırılır (`setCopiedHit("")`), yalnız **en son** deneme yazar
(başarı ve hata yolu); yeni kütüphane araması ve kapsam değişimi sayacı artırır —
geç yanıt ne etiketi ne de aramanın hatasını yazar. Arama başında `copiedHit`
temizliği ayrıca yapılır.

**Test (`screens/Search.test.tsx` +283; yeni `describe`: 10 test; 20 → 30):**
`navigator.clipboard.writeText` testten kurulur (jsdom panosu yok). Sabitlenen:
panoya sonucun **tam** alıntısı gider (`precedentSnippetForCopy`) ve etiket
yalnız o karta konur; pano reddederse `E_CLIPBOARD` görünür, “Kopyalandı”
yazılmaz; başarılı kopya bayat hatayı siler; yeni arama eski etiketi bırakmaz;
uçuştaki kopyanın **başarısı** ve **hatası** yeni sonuç kümesine yazılmaz;
kapsam değişiminde geç hata öteki kapsama yazılmaz; iki hızlı kopyada son
tıklanan kazanır; düşen kopya önceki yeşili kaldırır; kopya sürerken düşen arama
hatası kopya başarısıyla silinmez.

**Kapı kanıtı (6 mutasyon; her biri en az bir testi düşürdü; geri alma sha256
`493ede78601f…7153d` birebir):** M1 arama başında etiket/kopya nesli temizliği
(**4** test), M2 başarı yolundaki sıra kapısı (**3**), M3 deneme başında eski
etiketi kaldırma (1), M4 başarılı kopyanın bayat hatayı silmesi (1), M5 hata
yolundaki sıra kapısı (**2**), M6 kapsam değişiminde kopya nesli artışı (1).
**Dürüst:** ilk yazımda ayrıca bir `searchAt === searchSeq.current` kapısı
denendi; mutasyonla **ölçülemedi** (her arama `copySeq`yi zaten artırdığı için
ölü koddu) → koddan çıkarıldı, yedekli kapı bırakılmadı.

**Ölçüm:** `tsc --noEmit` temiz · `vitest` **904/904** (59 dosya; 894 → +10) ·
`pnpm build` temiz (**578,40 kB**, `index-B3CTekXV.js`, gzip **155,97 kB**).
rustfmt iki manifestte fark üretmedi (Rust değişmedi).

**Dürüst sınırlar:** jsdom **tarayıcı değil** — gerçek pano izni, gerçek Tauri
penceresi ve **görsel QA yok**; pano sözleşmesi mock'ludur. Etiket bir
**bildirim** gibi kalıcıdır: zamanlayıcıyla kendiliğinden silinmez, yalnız yeni
deneme/arama/kapsam değişimi temizler. Snippet'i boş (tam metni olmayan) kayıtta
“Alıntıyı kopyala” künye + boş alıntı satırı yazar; kayıt **açık** bırakıldı
(metinsiz kayıt aramada künyeyle görünebilir). Kapsam dışı: klavye, Windows,
gerçek zamanlama.


## Korpus aktarımı ve anlam hükmü: kalan ölçülmemiş yollar (2026-11-04)

`Precedents` ekranının son iki ölçülmemiş yolu kapatıldı: **korpus dışa
aktarma** (izin kapsamına bağlı, dosyaya yazan işlem) ve **anlam hükmü**
(kasaya yazılan hukuki karar). Mevcut 33 test korpusun *kapısını* ve hükmün
*bağlamını* ölçüyordu; ikisinin de **uçuş** davranışı boştu.

**Ölçülen kusurlar — ham koşu (düzeltme öncesi bileşen): 4 failed | 2 passed |
33 skipped (7 yeni test).**

- **Bayat yeşil sonuç (izin kapsamı yalanı):** çekirdeğin
  `CorpusExportResult`u hangi kiple yazıldığını **taşımıyor** (kip alanı yok);
  arayüz de sonucu kip değişiminde temizlemiyordu. “İzinli” kiple yapılan
  aktarımın yeşil satırı (“N emsal dışa aktarıldı · sha256 …”) “Tüm korpus”
  kipine geçildikten sonra ekranda kalıyor, **tüm korpusun sayısı** sanılıyordu
  — izin/dağıtım kapsamı hakkında yanlış hüküm.
- **Uçuşta kaydırılabilen kapsam:** kip seçici `corpusBusy` ile kilitli
  değildi; dışa aktarma sürerken kapsam değiştirilebiliyordu.
- **Hukuki kararda çift yazma:** anlam hükmünün üç düğmesinde **hiç** uçuş
  kilidi yoktu (`disabled` yok). Çift tık iki `citationMeaningReview` yazısı
  gönderiyordu; iki farklı karar hızlı tıklanırsa sıra çekirdeğin kabul ettiği
  son sözü belirliyordu.
- **İki yerel pencere:** toplu içe alma dosya seçicisi kendi uçuşunda kilitli
  değildi (`bulkBusy` yalnız *içe aktarma* uçuşunu tutuyor); çift tık iki
  pencere açıyor, ikinci seçim birincinin seçimini eziyordu.

**Düzeltme (`Precedents.tsx` +35/−6):** kip `onChange`inde `setCorpusResult(null)`
ve seçicide `disabled={corpusBusy}`; `reviewBusy` tek uçuş bayrağı + üç karar
düğmesinde `disabled`; `pickBusy` bayrağı iki seçici düğmede `disabled` ve
`finally`de bırakılır. Dışa aktarmanın kipi **render değerinden** okunur
(pencere açıkken kip kaydırılsa bile dosya, düğmeye basıldığı anda görünen
kiple yazılır — bu bir kapanış garantisi, ayrı `ref` gerekmedi).

**Test (`screens/Precedents.test.tsx` +194; yeni `describe`: 7 test; 33 → 40):**
kip değişiminde sonuç satırı kalkar; uçuşta seçici + düğme kapalı ve istek ilk
kiple gider; pencere açıkken kip değişse de istek tıklama anındaki kiple gider;
düşen aktarım önceki yeşili bırakmaz (gerçek `E_IO` yazılır); hükümde çift tık
tek yazma + üç düğme uçuşta kapalı; düşen hüküm panel + gerekçe metnini korur,
düğmeler yeniden açılır; seçicide çift tık tek pencere açar.

**Kapı kanıtı (8 mutasyon; her biri en az bir testi düşürdü — 1 kırmızı; geri
alma sha256 `842aabcf0635…889985c` birebir):** M1 kip değişiminde sonuç
temizliği, M2 uçuşta seçici kilidi, M3 üç karar düğmesinin kilidi, M4 hüküm
uçuş bayrağı, M5 seçici düğme kilidi, M6 seçici uçuş bayrağı, M7 aktarma
başında sonuç temizliği, M8 hüküm sonrası kilit bırakma. **Dürüst:** ilk
mutasyon koşusu mutasyonları **biriktirdi** (6/9 okundu, son iki ölçüm
kirlendi) → betik her mutasyonu temiz dosyaya uygulayacak şekilde düzeltildi
ve 8/8 yeniden ölçüldü. Ayrıca üç aday kapı (pencere açıkken kipin `const`a
alınması, hüküm ve seçici için `ref` yeniden giriş kapıları) **tek başına
mutasyonla ölçülemedi** (kapanış zaten render değerini, React da `disabled`
düğmede tıklamayı tutuyor) → **koddan çıkarıldı**, yedekli kapı bırakılmadı.

**Ölçüm:** `tsc --noEmit` temiz · `vitest` **951/951** (60 dosya; 944 → +7) ·
`npm run build` temiz (**579,69 kB**, `index-DKjN0MM4.js`, gzip **156,27 kB**).
Rust değişmedi; rustfmt iki manifestte `--check` temiz.

**Dürüst sınırlar:** jsdom **tarayıcı değil** — gerçek Tauri penceresi, gerçek
yerel dosya pencereleri ve **görsel QA yok**; `api` ve `@tauri-apps/plugin-dialog`
mock'ludur, `precedent_corpus_export` / `citation_meaning_review` SQL'i ve Rust
**koşmaz**. Çift tık kapısı `disabled` üzerinden ölçülür (React disabled
düğmede `onClick` üretmez); gerçek işaretçi çift tıklaması ve zamanlaması
ölçülmedi. `reviewBusy` tek bayraktır: tek panel açık olduğu için sayaç
gerekmedi. Korpus sonucu hâlâ **kip taşımıyor** — kip değişimi satırı temizler,
ama aynı kip içinde iki farklı aktarım ayırt edilmez (açık kayıt).
`exportCorpus` sonrası `loadCoverage()` düşerse yeşil satır **ve** kırmızı hata
birlikte görünür (ikisi de doğru: dosya yazıldı, kapsam okunamadı) — kabul
edildi.

## F12-T15 genişletmesi — emsal yazma yolları: taslak koruması, geç hata ve kopya etiketi (2026-11-04)

`Precedents` ekranının (1686 satır, 50 test) kalan **yazma uçuşları** ölçüldü:
emsal oluşturma, kullanım notu, tam metin, kullanım koşulu, atıf kontrolü ve
anlam hükmü. Önceki turlar alıntı doğrulamayı, korpus aktarımını ve seçim
neslini kapatmıştı; bu turda **yanıt beklerken forma yazılan metin** ve **eski
sürümün geç hatası** hiç ölçülmemişti.

**Ölçülen kusurlar (düzeltme öncesi bileşende 9/9 kırmızı, mevcut 40 test yeşil):**

- **Sessiz kayıp (dört yol):** oluşturma başarısı `setDraft(emptyDraft())` ile
  formu kapatıp uçuşta yazılan yeni künyeyi siliyordu; not kaydı üç alanı,
  tam metin kaydı `setFullText("")` ile alanı, koşul kaydı sunucudan gelen
  değerlerle dört alanı koşulsuz eziyordu.
- **Geç hata (iki yol):** `citationCheckCreate` ve `citationMeaningReview`
  hataları sürüm kapısı taşımıyordu → eski belge sürümünün `E_DB`'si yeni
  sürümün/dosyanın altında görünüyordu.
- **Kopya etiketi (üç yol):** künye kopyası seçim nesli taşımıyordu (başka
  emsal seçilince geç başarı yeni künyeye "Kopyalandı" yazıyordu); düşen kopya
  önceki yeşil etiketi bırakıyordu; künye tazelenince (tam metin ekleme) etiket
  ekranda kalıyordu.

**Düzeltme (`Precedents.tsx` +85/−21):** modül düzeyinde `sameDraft()`; dört
yazma yanıtı gönderilen değere bağlandı (`setDraft`, üç not alanı,
`setFullText`, dört koşul alanı yalnız değişmemişse yazılır);
`citationCheckCreate`/`citationMeaningReview` hata yolları `versionRef`
kapısına alındı; `copySeq` nesil sayacı + `bibliography` effect'i (kopya
etiketi istek anındaki künyeye bağlı, künye tazelenince düşer).

**Testler (`Precedents.test.tsx` 40 → 50, +10):** yeni describe;
`deferredWithReject<T>()`, `BIB2`. Sabitlenenler: dört taslak koruması, iki geç
hata kapısı, üç kopya etiketi kapısı + düşen kopyanın yeşili kaldırması.

**Ham koşu (düzeltme öncesi bileşen, o günkü 49 test): 9 failed | 40 passed** —
dokuzu da gerçek kusur; mevcut 40 test düzeltme öncesinde de yeşildi. Onuncu
test (uçuştaki kopya **hatası** yeni emsale yazılmaz) düzeltmeden **sonra**
eklendi; kapısı yalnız M10 mutasyonuyla ölçüldü (düzeltme öncesi koşuda yoktu).

**Kapı kanıtı (10/10 tek kapı mutasyonu kırmızı; geri yükleme sha256
`b567866b7f980fc29e8dbc2f9a5159e4b255c04f2149f77700b5f122f1cf4732` birebir):**
M1 oluşturma taslağı (1), M2 not alanları (1), M3 tam metin (1), M4 koşul
alanları (1), M5 atıf kontrolü hatası (1), M6 anlam hükmü hatası (1), M7 kopya
başarı nesli (1), M8 kopya başında etiket temizliği (1), M9 künye tazeleme
effect'i (**3**), M10 kopya hata nesli (1).

**Ölçüm:** `tsc --noEmit` temiz · `vitest` **1043/1043** (60 dosya; 1033 → +10) ·
`npm run build` temiz (**584,30 kB**, `index-C5DATuH7.js`, gzip **157,65 kB**;
CSS `index-CPW4QqlF.css`). Rust değişmedi; rustfmt gerekmedi (Rust dosyası yok).

**Commit/CI (ölçüldü):** kod `fa84763` (2 dosya, +388/−21) → **`41881781105`
7/7 YEŞİL** (`arayüz` **60 dosya / 1043 test**, aynı paket `index-C5DATuH7.js`
584,30 kB / gzip 157,65 kB → yerel/CI **birebir**). `HEAD == origin/main`,
ağaç temiz.

**Dürüst sınırlar:** jsdom **tarayıcı değil** — gerçek Tauri penceresi, gerçek
pano izni, gerçek işaretçi tıklaması ve **görsel QA yok**; `api` mock'lu olduğu
için `precedent_*` ve `citation_*` SQL'i/Rust tarafı **koşmaz**. Taslak koruması
**arayüz sezgisi**: alan metni gönderilen değerle karşılaştırılır; kullanıcı aynı
metni yeniden yazdıysa temizlenir (anlam farkı yok). Kopya nesli **arayüz
düzeyinde**; doğrudan IPC bu kapıyı atlar. `busy` tek bayrak: bu ekranda farklı
yazmalar paralel başlamaz (düğmeler kapalı).

**Açık kayıt:** (a) not/tam metin/koşul yanıtı düşerse uçuşta yazılan metin
korunur ama kaydedilmemiş olabilir (hata görünür, kullanıcı yeniden dener);
(b) aynı künye metni yeniden yazıldıysa "değişmedi" sayılır ve alan temizlenir
(bilinçli); (c) görsel QA + gerçek Tauri penceresi **insan bekliyor**.
