# F11 — Kaynaklı AI inceleme araçları

- **Sürüm etiketi:** `v0.10.0`
- **Durum:** Kısmi
- **Tarih:** 2026-10-24

| İş | Durum | Kanıt |
|---|---|---|
| F11-T01 aday kaydı | ✅ | `ai_candidates`, `valid/invalid/partial`; altı aday türü şeması |
| F11-T02 aday → inceleme kuyruğu | ✅ | `promote_candidate`, `review_items` `pending`, doğrulanmış alıntı → `source_anchors` |
| F11-T03 kapsamlılık raporu | ✅ | belge/sayfa/OCR/boş sayfa/ayrıştırılmamış sayıları |
| F11-T04 gerçek model çağrısı | ✅ | `ApiProvider`/`LocalProvider` + `ai_run_execute`: onay + profil + kapsam sicili, hata run'ı `failed` kapatır |
| F11-T05 şema doğrulama + alıntı eşleştirme | ✅ | `crates/storage/src/ai_candidate.rs`, şema v17, `ai_candidate_validate` |
| F11-T06 kapsamlılık raporunun rapora gömülmesi | ✅ | JSON `coverage`, DOCX/PDF bölümü, CSV `#` bloğu, `KAPSAM.md` + `manifest.coverage` |

## Dürüst durum

AI çıktısı **adaydır**; avukat kabul etmeden hukuki kayıt oluşmaz. Model çağrısı
yapılmadığı için gerçek çıkarım kalitesi ölçülmedi.

## F11-T05 — şema doğrulama ve alıntı eşleştirme (2026-10-24)

**Doğrulama durumu artık hesaplanır.** `record_ai_candidate` çağıranın verdiği
durumu kabul etmez; yükü türüne göre doğrular, alıntıları belge metninde arar ve
`valid | partial | invalid` sonucunu kendisi yazar. Arayüzden “geçerli” demenin
yolu yoktur.

- **Altı şema** (`event`, `person`, `date`, `claim`, `summary`, `answer`;
  `*_candidate` adları eski kayıtlarla uyum için kabul edilir):
  - ortak: `schemaVersion` (yalnız `1`), `quotes[]` (en çok 20; her biri
    `documentVersionId` + `quote`, isteğe bağlı `pageIndex`/`label`);
  - `event`: `title` (1–200) zorunlu; `date`, `participants` (≤20), `description`;
  - `person`: `name` zorunlu; `role` (`davaci|davali|vekil|tanik|hakim|bilirkisi|diger|unknown`), `organization`;
  - `date`: `rawText` + `kind` (`document|event|notification|hearing|unknown`)
    zorunlu; `isoDate` **gerçek bir gün** olmalı (`YYYY-AA-GG`, artık yıl
    denetimi), `precision` isteğe bağlı — yaklaşık tarih `rawText`te kalır;
  - `claim`: `text` zorunlu; `assertedBy`, `amountText` (tutar **özgün metin**
    olarak, sayıya çevrilmez);
  - `summary`: `text` zorunlu **ve en az bir alıntı** (F11-T07 dayanağı);
  - `answer`: `text` zorunlu; `question` isteğe bağlı.
- **Sorun kodları:** `invalid_json`, `not_object`, `missing_field`,
  `unknown_field` (uyarı), `wrong_type`, `empty_text`, `too_long`,
  `too_many_items`, `bad_enum`, `bad_date`, `control_chars`,
  `unknown_candidate_type`, `schema_version_unsupported`,
  `quote_missing_field`, `quote_too_short` (< 8 karakter), `duplicate_quote`.
  `error` adayı geçersiz kılar; `warning` yalnız `partial` yapar.
- **Alıntı eşleştirmesi:** her alıntı, belge sürümünün **en yeni**
  `succeeded/partial` ayrıştırmasının sayfalarında `dim_evidence::verify_quote`
  ile aranır. Modelin `startHint`i **kullanılmaz** (konum güvenilir değil).
  - `exact` (tek eşleşme, sayfa + karakter konumu), `normalized` (yalnız
    normalizasyonla), `ambiguous` (birden fazla yer; en çok 5 konum),
    `page_mismatch` (bulundu ama modelin dediği sayfada değil),
    `not_found`, `unverifiable` (`version_not_found`, `case_mismatch`,
    `no_text`).
  - **Başka dosyanın belgesi** kaynak olamaz: `case_mismatch` → `failed`.
  - Ayrıştırılmamış belge “doğrulandı” sayılmaz; `no_text` → `partial`.
- **Kapılar:** `quote_state=failed` aday `E_AI_QUOTE` ile, `validation_state=invalid`
  aday `E_AI_SCHEMA` ile inceleme kuyruğundan reddedilir. Alıntısı olmayan aday
  (`no_quotes`) kuyruğa girebilir ama `partial` işaretlidir.
- **Kaynağa bağlama:** terfi sırasında `exact` alıntılar için `source_anchors`
  kaydı oluşturulur (aynı segment/offset/alıntı varsa yeniden kullanılır) ve
  `review_items.source_anchor_ids` bu kimliklerle yazılır.
- **Yeniden doğrulama:** `revalidate_ai_candidate` güncel ayrıştırmaya göre
  sonucu tazeler; belge yeniden ayrıştırıldığında eski “geçerli” damgası kalmaz.
- **Şema v17** (`0017_ai_candidate_quotes.sql`): `schema_state`, `quote_state`,
  `quote_report`, `checked_at`. Bu sürümden önceki satırlar **`legacy`** olur —
  doğrulanmış gibi görünmez.
- **Denetim:** `ai_candidate`/`record|revalidate` kaydı yalnız tür, durum ve
  alıntı sayısı taşır; **yük metni ve alıntı içeriği denetime yazılmaz**.
- **Arayüz:** “Aday doğrula ve kaydet” kartı (tür seçimi, JSON taslağı,
  “Doğrula” / “Doğrula ve kaydet”), sonuçta şema/alıntı rozetleri, alıntı
  tablosu (durum, sayfa/karakter, tekrar, metin kaynağı), sorun listesi ve
  “Doğrulamanın sınırları”; aday listesinde şema + alıntı rozetleri,
  “Yeniden doğrula” ve engel gerekçesiyle kapalı “İncelemeye gönder”.

### Sınırlamalar (dürüst)

- **Model çağrısı yok**: aday yükü kullanıcı tarafından yapıştırılır; gerçek
  sağlayıcı çıktısıyla ölçüm yapılmadı.
- Eşleştirme **kasada ayrıştırılmış metinle** sınırlı: OCR yazım hatası veya
  taranmış sayfada metin olmaması eşleşmeyi etkiler; görüntü üzerinde
  doğrulama yok.
- **Maskeli bağlamın özgün kaynağa geri eşlenmesi (F11-T03) yapılmaz**; aday
  metni gönderildiği hâliyle saklanır, alıntı bu metinde aranır.
- Şema katıdır: tanımsız alan `warning`, yanlış tür/eksik zorunlu alan `error`.
  Model serbest metin döndürürse aday geçersiz olur — bu bilinçli bir kapıdır.
- `pageIndex` 0 tabanlıdır; arayüz 1 tabanlı gösterir. `isoDate` ile `rawText`
  arasındaki tutarlılık **denetlenmez** (Türkçe tarih ayrıştırma kapsam dışı).
- Tauri penceresi açılmadı → kartların görsel QA'sı yok.

### Doğrulama kaydı

- `cargo test -p dim-storage --lib` → **231** test yeşil (ai_candidate 13 yeni).
- Web: `pnpm typecheck` temiz, `pnpm test` **259** (candidate.test.ts 13 test),
  `pnpm build` yeşil.
- Clippy: `-p dim-storage --all-targets -D warnings` ve
  `--manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -D warnings`
  temiz; rustfmt iki manifest temiz.
- Commit `323a662` → CI run `40132668277` **7/7 yeşil** (arayüz, SBOM, masaüstü
  kabuk, rust ubuntu+macos, belge worker'ı, sözleşmeler).

## F11-T06 — kapsamlılık raporunun çıktıya gömülmesi (2026-10-25)

Rapor kasadan çıktıktan sonra “her şey incelendi” sanılmamalı. Kapsam artık
**her dışa aktarma çıktısının içinde** yazılı; arayüzün kendi cümlesi değil,
kasadan gelen sayılar.

- **Yeni çekirdek modül `crates/storage/src/export_coverage.rs`:** `ExportCoverage`
  (`documents`, `versions`, `parseRuns`, `pages`, `textPages`, `ocrPages`,
  `segments`, `emptyPages`, `unparsedDocuments`, `failedParseRuns`, `complete`,
  `warnings[]`, `statement`, `generatedAt`). Sayılar `Database::coverage_report`
  çıktısından **olduğu gibi** taşınır; modül yalnız okunur biçime çevirir.
- **Tek dil, dört çıktı.** Uyarı cümleleri tek yerde üretilir:
  - ayrıştırılmamış belge, metinsiz (boş) sayfa, başarısız ayrıştırma koşusu,
    OCR ile okunan sayfa ve belgesiz dosya için ayrı uyarı;
  - sabit sınır cümlesi `COVERAGE_STATEMENT` = “İncelenmedi ≠ temiz: …”;
  - `complete` yalnız “her belge ayrıştırıldı, her sayfada metin var” demektir;
    OCR kullanımı kapsamı eksik yapmaz, güvenilirlik uyarısı üretir.
- **JSON (`export_json`)** — üst düzeyde `coverage` bloğu. **Devir paketi** —
  `case.json` içinde aynı blok, ayrıca `KAPSAM.md` (sayım tablosu + uyarılar +
  sınır + sayım yöntemi notu) ve `manifest.json` içinde `coverage`; `KAPSAM.md`
  manifest dosya listesinde sha256 ile yer alır. README “KAPSAM.md zorunlu
  okumadır” der.
- **DOCX/PDF** — “İncelenen kapsam” bölümü **ilk** sırada (okuyucu içeriğe
  geçmeden sınırı görür): ölçü/değer tablosu + uyarı ve sınır satırları; künye
  meta satırına tek satırlık kapsam özeti eklenir.
- **CSV** — tablo satırlarına karışmasın diye başta `#` yorum bloğu (özet,
  kapsam durumu, üretim zamanı, uyarılar, sınır). `#` satırlarını yorum sayan
  okuyucular (pandas `comment='#'`) tabloyu temiz okur.
- **Tauri:** yeni `case_coverage(caseId)` komutu (dışa aktarmadan önce arayüz
  kapsamı gösterir). `ExportView` ve `PackageView` artık **yazılan** kapsam
  bloğunu döner — arayüz “gömüldü” iddiasını yanıttan gösterir, local state’ten
  değil. `PackageView.files` manifest dosya sayısından hesaplanır (önceden sabit
  `3` yazıyordu, gerçekte 2 dosya vardı).
- **Arayüz (Çıktılar):** “İncelenen kapsam” kartı — durum rozeti (tam/kısmi/
  kapsam yok), tek satır özet, on ölçü kutusu (boş sayfa/OCR/ayrıştırılmamış
  kırmızı), uyarı listesi, sınır cümlesi, “Kapsamı tazele”. Dışa aktarma
  sonucunda “Kapsam çıktıya gömüldü (…): N belge · …” satırı; devir paketinde
  `KAPSAM.md` satırı.
- **Test:** storage **239** (+8 `export_coverage`), Tauri komut testi
  `case_payload_embeds_coverage_block`, Web **273** (+14
  `export-coverage.test.ts`); `tsc --noEmit` + `vite build` yeşil.
- **Commit/CI:** `14d5994` → run `40138828188` **7/7 yeşil** (arayüz, SBOM,
  masaüstü kabuk, rust ubuntu+macos, belge worker'ı, sözleşmeler).

### Sınırlamalar (F11-T06)

- Sayımlar **ayrıştırma koşusu satırları** üzerinden alınır: aynı belge iki kez
  ayrıştırıldıysa sayfa sayısı iki katına çıkar. Belge sayısı silinmemiş
  kayıtları sayar. Bu, `KAPSAM.md` içinde yöntem notu olarak yazılıdır.
- Kapsam **kasadaki ayrıştırılmış metnin** kapsamıdır; OCR kalitesi, yanlış
  eşleme veya eksik sayfa tespiti yapılmaz. “Tam kapsam” hukuki bütünlük
  kanıtı değildir.
- CSV’de kapsam **yorum satırıdır**; yorumları atmayan bir okuyucu bu satırları
  veri sanabilir. Bu bilinçli: sessizce gizlemek yerine görünür kılındı.
- DOCX/PDF kapsam bölümü worker’ın `sections` şemasıyla üretilir; Tauri
  penceresi açılmadı → **görsel QA yok**, yalnız birim testleri + üretim
  derlemesi.
- **Devir paketinin klasör yazımı** (`case.json`, `KAPSAM.md`, `manifest.json`)
  Tauri komutu içinde ve `AppState`e bağlı; **uçtan uca birim testi yok**. Test
  edilenler: `case_payload` içindeki `coverage` bloğu, `ExportCoverage::markdown`
  çıktısı ve CSV/JSON biçimleri. Paket yolu kod incelemesi + CI derlemesiyle
  doğrulandı, gerçek paket üretilerek denenmedi.

## F11-T04 — AI ekranı: uçuştaki yanıtlar dosya/çalıştırma bağlamına bağlandı (2026-11-04)

AI ekranı (`apps/desktop/src/screens/Ai.tsx`, 623 satır) çekirdeğin onay,
çalıştırma ve aday yollarını kullanır; hiçbir yanıt **hangi dosya/hangi
çalıştırma** için istendiğini taşımıyordu. Aynı ekranın 20 testi yalnız çizimi
ve tek adımlı akışları ölçüyordu; uçuşta dosya/çalıştırma değişimi hiç
ölçülmemişti.

**Ölçülen kusurlar (düzeltme öncesi, 11 test kırmızı):**

- **Onay (en ağırı):** `approve()` p1 için `aiConsentCreate` uçuşta iken p2'ye
  geçilirse p1'in onay satırı p2'de çiziliyor, “Çalıştır” **p1'in onay
  kimliğiyle** açılıyordu → yeni dosyanın künyesiyle eski dosyanın onayı
  birlikte görünüyordu (INV-05). Onay önizlemesi uçuşta kalırsa önizleme de
  yeni dosyaya yazılıyordu.
- **Çalıştırma:** `start()` sonrası özet tazelemesi p2 seçiliyken p1'in
  sağlayıcı/kapsam künyesini yazıyordu (4 belge/132 sayfa, p2'nin 1/7'si
  yerine).
- **Aday listesi:** iki çalıştırma hızlı seçilince r1'in geç yanıtı r2'nin
  listesini eziyor, seçili çalıştırma r2 görünürken r1'in adayları
  çiziliyordu.
- **Doğrulama:** `validateDraft()` yanıtı dosya değişse de yeni dosyaya rapor
  yazıyordu; “Doğrula ve kaydet”te kayıt çekirdeğe gittikten sonra liste ve
  bildirim de yeni dosyaya yazılıyordu.
- **Geç onay iptali:** iptal uçuşta iken alınan **yeni** onay, geç dönen
  iptalin `setConsent(null)`ı ile siliniyordu; “Çalıştır” nedensiz kapanıyordu.
- **İncelemeye gönderim / yeniden doğrulama / önizleme:** geç başarı ve geç
  hata yolları yeni dosyaya yazılıyordu (bildirim, `E_` bandı, önizleme
  kartı).
- **Hızlı dosya değişimi:** p2'nin özet isteği askıda iken p1'e dönülünce
  p2'nin geç yanıtı p1'in künyesini eziyordu (başarı ve hata yolu).

**Düzeltme (`Ai.tsx` +112/−37):** iki sayaç — `caseGen` (dosya değişiminde
artar) ve `runSeq` (her çalıştırma seçiminde artar). Her yanıt istendiği andaki
nesli/seçimi saklar ve yalnız **aynı bağlamda** yazar; hata yolu da aynı
kapıdan geçer. Onay akışı önizleme ile onay kaydı arasında da kapı taşır (dosya
değiştiyse onay **oluşturulmaz**). Onay iptali `consent.id` ile karşılaştırır:
geç yanıt yeni onayı silmez. `aiCandidateRecord` bilinçli istisna: kullanıcının
açık “kaydet” isteği bağlam değişse de çekirdeğe gider; yalnız liste ve bildirim
yazılmaz.

**Testler (`Ai.test.tsx` 20 → 38):** 18 yeni uçuş testi (11 başarı, 7 hata
yolu). Düzeltme öncesi ilk 11 testin **11'i kırmızı**; hata yolu testlerinin
düzeltme öncesi karşılığı tek-kapı mutasyonlarıyla ölçüldü (M14–M19, her biri
`1 failed`).

**Kapı kanıtı:** 20 tek-kapı mutasyonu, 20'si kırmızı. **Dürüst:** M1 (onay
önizleme kapısı) ilk turda **yeşil kaldı** — mevcut senaryo önizlemeyi dosya
değişmeden önce yazıyordu; eksik senaryo için ayrı test eklendi (önizleme
askıda + dosya değişimi) ve M1 yeniden ölçüldü → `1 failed`. Mutasyon betiği
her koşudan sonra dosyayı yedekten geri yükledi; geri yükleme sha256
`f7b4a2c613aacf9b975bd622bdcebe1b7a3910cba7b1e66db6e2d958ef4aafb6` birebir.

**Ölçüm:** `tsc --noEmit` temiz; `vitest` **974/974** (60 dosya; 956 → +18);
`vite build` temiz (580,67 kB, `index-B2A0bn76.js`, gzip 156,48 kB). Rust
değişmedi; rustfmt iki manifestte `--check` temiz.

**Dürüst sınırlar:**

- jsdom **tarayıcı değil**: gerçek Tauri penceresi, gerçek çekirdek komutları
  (SQL/Argon2/CLI) ve **görsel QA yok**; `api` mock'ludur, yanıt **sırası**
  elle çözülen sözlerle denetlenir, gerçek gecikme değil.
- `busy` tek bayrak: dosya değişince eski istek bitene kadar yeni dosyanın
  “Onayla/Doğrula” düğmeleri kapalı kalır (kilit devri yapılmadı; önceki
  ekranlardaki tek-bayrak kararıyla aynı).
- Aday kaydı (`aiCandidateRecord`) bağlam değişse de gider — bilinçli; “gönderim
  yarıda kaldı” durumu arayüzde gösterilmez, kullanıcı yeniden doğrular.
- `AiRunExecute` / `aiConsentCreate` çekirdek yazıları **geri alınmaz**: arayüz
  yalnız yazmaz; istek zaten çekirdeğe iletilmiştir (yarıda kalan işlem
  temizliği kapsam dışı).
- Eşzamanlı iki işlem (ör. onay + doğrulama) aynı `busy` bayrağını paylaşır;
  kapılar çakışmayı engeller, gerçek paralellik ölçülmedi.
- Kimlik: yeni numara uydurulmadı — F11 README'nin mevcut iş listesindeki
  **F11-T04** (gerçek model çağrısı: onay + çalıştırma yolu) kullanıldı; AI
  ekranının aday/onay yolları bu işin arayüzüdür.
