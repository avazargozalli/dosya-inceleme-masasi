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
| F12-T05 | ✅ | karar tam metni (şema v13) + metin içi alıntı doğrulama; **tire ile bölünmüş kelime** katmanı ve gerçek kaynak aralığı (2026-10-27) |
| F12-T06 | ✅ | toplu içe alma (CSV künye listesi, çakışma işareti) + dış bağlayıcı sicili (etkin değil) |
| F12-T07 | ✅ | alıntı çevresi + anlam incelemesi (şema v15; gerekçesiz olumsuz hüküm yok) |
| F12-T10 | ✅ | kütüphane araması dosya aramasından ayrı (şema değişikliği yok; sızıntı testi) |
| F12-T11 | ✅ | kaynak kullanım koşulu (şema v16) + izinsiz toplu korpus dağıtım kapısı |
| F12-T12 | ✅ | bibliyografik künye: künye + kaynak bağlantısı + erişim tarihi (eksik alan tahmin edilmez) |
| F12-T13 | ✅ | kasadaki belge sürümünden tam metin aktarma (ayrıştırılmış sayfa metni; sürümü kullanıcı seçer) |
| F12-T14 | ✅ | metin içi alıntı denetimi: iddia/özet metnindeki tırnaklı pasajlar dosya belgelerinde aranır; birebir tek konum kaynağa bağlanır; sayfa sınırını aşan alıntı bulunur ama bağlanmaz; güncel sürümde bulunamayan alıntı belgenin **eski sürümlerinde** de aranır; **satır/sayfa sonunda tire ile bölünmüş kelime** birleştirilerek aranır (en zayıf katman, bağlanmaz) |

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

**Commit/CI:** `bad747c` → CI **`40730120844` 7/7 yeşil** (sözleşmeler, arayüz
tsc+vitest+build, rust macos-14 + ubuntu-latest, masaüstü kabuk, belge worker'ı, SBOM).
