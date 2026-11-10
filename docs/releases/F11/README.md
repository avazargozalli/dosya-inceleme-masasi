# F11 — Kaynaklı AI inceleme araçları

- **Sürüm etiketi:** `v0.10.0`
- **Durum:** Kısmi
- **Tarih:** 2026-10-24

| İş | Durum | Kanıt |
|---|---|---|
| F11-T01 aday kaydı | ✅ | `ai_candidates`, `valid/invalid/partial`; altı aday türü şeması |
| F11-T02 aday → inceleme kuyruğu | ✅ | `promote_candidate`, `review_items` `pending`, doğrulanmış alıntı → `source_anchors` |
| F11-T03 kapsamlılık raporu | ✅ | belge/sayfa/OCR/boş sayfa/ayrıştırılmamış sayıları |
| F11-T04 gerçek model çağrısı | ✅ | `ApiProvider`/`LocalProvider` + `ai_run_execute`: onay + profil + kapsam sicili, hata run'ı `failed` kapatır; onay **varış yerine** bağlı (`provider_id` → türetilmiş `adapter@politika#parmak izi`) |
| F11-T05 şema doğrulama + alıntı eşleştirme | ✅ | `crates/storage/src/ai_candidate.rs`, `ai_candidate_validate`; alıntı **önce gönderilen metinde** aranır (şema v26, `transfer_segments` + `record_transfer_segments`) |
| F11-T06 kapsamlılık raporunun rapora gömülmesi | ✅ | JSON `coverage`, DOCX/PDF bölümü, CSV `#` bloğu, `KAPSAM.md` + `manifest.coverage` |
| Şartname F11-T03 eki — gönderim önizlemesi maskeyi taşır | ✅ | `consent_preview_masked`: maskeli örnek + **maskeli bayt** (`bytesExact` bütçe bayrağı), plan kapsam kapısı `E_MASK_PLAN_SCOPE`, onaydan sonra maske değişince çalıştır kapısı; **11/11 mutasyon** (2026-11-09, aşağıdaki bölüm) |

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

> **Bu sınırlardan biri bu turda kapandı:** “dosya değişince eski istek bitene
> kadar yeni dosyanın düğmeleri kapalı kalır (kilit devri yapılmadı)” artık
> geçerli değil — aşağıdaki 2026-11-05 bölümü kilit devrini ekledi.

## F11-T04 — AI ekranı: yazma uçuşu, kilit devri ve geç yanıt kapıları (2026-11-05)

Önceki tur yanıtların **hangi dosya/çalıştırmaya** yazıldığını kapatmıştı
(`caseGen`, `runSeq`). Bu tur ekranın **yazma uçuşunu** ölçtü: `busy` tek
bayrağının devri, iki yazma yolunun (gönderim, iptal) hiç kilidi olmaması ve
bildirim/rapor/önizleme yanıtlarının **gönderilen değere** bağlanması.

`Ai.tsx` yedi yazma yolu çağırıyor: `aiConsentCreate` (onay), `aiRunExecute`
(çalıştırma), `aiCandidateValidate` + `aiCandidateRecord` (doğrula/kaydet),
`aiCandidateRevalidate`, `aiCandidatePromote` (incelemeye gönder) ve
`aiConsentRevoke` (iptal).

### Ölçülen kusurlar (11; hepsi düzeltme öncesinde kırmızı)

1. **Kilit devri yok (en ağırı):** onay/çalıştırma/doğrulama askıdayken dosya
   değişince `busy` bırakılmıyordu → yeni dosyanın “Onayla/Çalıştır” düğmeleri
   eski yanıta kadar, yanıt hiç gelmezse **süresiz** kapalı.
2. **Erken açılan kilit:** dört yolun `finally`si koşulsuz `setBusy(false)` →
   devredilen isteğin kapanışı yeni dosyada başlatılan yazmanın kilidini
   açıyordu (kilit devri eklenince gerçek yarış hâline geliyor).
3. **Gönderimde uçuş kilidi yok:** “İncelemeye gönder” düğmesi
   `disabled={busy || ...}` idi ama `promote()` `busy`yi **hiç** kurmuyordu →
   çift tık iki `aiCandidatePromote` (çekirdekte yinelenen terfi denemesi).
4. **Onay iptalinde uçuş kilidi yok:** `revokeConsent()` bayrak kurmuyor,
   düğme hiç `disabled` taşımıyordu → çift tık iki `aiConsentRevoke`; ikincisi
   `E_NOT_FOUND` alıp **başarılı** iptalin üstüne kırmızı yazıyordu.
5. **Geç bildirim (kayıt):** `same` yanıt **öncesinde** hesaplanıyordu →
   `aiCandidateRecord` veya liste tazelemesi beklerken çalıştırma değişirse
   “Aday kaydedildi” bildirimi **yeni** çalıştırmanın altına yazılıyordu.
6. **Sessiz çapraz liste:** aynı senaryoda `loadCandidates(forRun)` eski
   çalıştırmanın adaylarını yeni çalıştırmanın tablosuna yazıyordu (çağrı
   bağlam değiştikten **sonra** yapıldığı için kendi iç kapısı işlemiyordu).
   *Ölçüm:* testin bildirim assertion'ları kaldırılıp yalnız liste assertion'ı
   koşuldu → düzeltme öncesi `1 failed` (`expected <td …> to be null`, r2'nin
   tablosunda r1'in adayı); düzeltmeyle yeşil.
7. **Geç bildirim (gönderim):** `promote()` yalnız `caseGen`e bakıyordu →
   uçuşta başka çalıştırmaya geçilince “İncelemeye gönderildi.” bildirimi yeni
   çalıştırmanın ekranına düşüyordu.
8. **Önizleme sırası yok:** “İçeriği göster” bir **okuma**; iki tık iki istek
   açıyor, **eskisinin** geç hatası yeni önizlemeyi siliyor ve kırmızı yazıyordu
   (`documentList` + `aiConsentPreview` iki kez koşuyordu).
9. **Bayat kırmızı:** başarılı önizleme önceki isteğin kırmızısını silmiyordu →
   ekranda taze önizleme + eski hata yan yana.
10. **Eski isteğin geç başarısı:** yeni istek düştüğünde eskisinin geç başarısı
    önizlemeyi çizip hatayı eziyordu (sıra kapısı yalnız hata yolundaydı).
11. **Rapor yanlış türün altında:** doğrulama askıdayken aday türü değişince
    geç gelen rapor **yeni** türün taslağının altına “güncel” gibi yazılıyordu.

### Düzeltme (`Ai.tsx` +94/−17)

- `busySeq` + `beginWrite()/endWrite(seq)`: kapanış yalnız kendi neslinde
  kilidi bırakır; dosya değişimi `busy`yi **devreder** (nesil artar).
- `promoteSeq`/`revokeSeq` + ayrı `promoteBusy`/`revokeBusy` bayrakları: iki
  yazma yolu **kendi** tek-uçuş kilidini taşır (biri askıdayken diğeri açık
  kalır — onay iptali yeni onayı engellemez), dosya değişimi ikisini de
  devreder; düğmeler bu bayraklarla kapalı.
- `validateDraft`: `fresh()` (dosya + çalıştırma) kayıt sonrası **ve** liste
  tazelemesi sonrası yeniden sorulur; rapor yalnız `kindRef` hâlâ **gönderilen**
  türdeyse yazılır (`kind` değişimi `setReport(null)` yapıyordu; geç yanıt onu
  geri getiriyordu).
- `promote`: başarı/hata `caseGen` **ve** `runSeq` kapılı.
- `showContent`: `previewSeq` — son istek kazanır; başarı kırmızıyı siler.
- Rust değişmedi; yeni i18n anahtarı yok.

### Testler ve kapı kanıtı

- `Ai.test.tsx` **38 → 50** (+12; yeni `describe` “yazma uçuşu, kilit devri ve
  geç yanıt kapıları”). Ham koşu (düzeltme öncesi bileşen, nihai 50 test):
  **11 failed | 39 passed** — on biri de gerçek kusur; on ikinci test
  (gönderim/iptal kilidinin devri) düzeltme öncesinde de yeşil (**sabitleme**),
  kapısı M9 ile ölçüldü.
- **Kapı kanıtı 11/11** tek kapı mutasyonu kırmızı: M1 gönderim düğmesi kilidi
  (1), M2 iptal düğmesi kilidi (1), M3 dosya değişiminde `busy` devri (**2**),
  M4 `endWrite` nesil kapısı (1), M5a kayıt sonrası tazelik (2), M5b tazeleme
  sonrası tazelik (1), M6 gönderim çalıştırma kapısı (1), M7 önizleme sıra
  kapısı (1), M8 rapor tür kapısı (1), M9 gönderim/iptal devri (1), M10
  önizlemede kırmızı temizliği (1). Geri yükleme sha256
  `dddbbb9be363474df3c109dc6f9f76e70abaf6eed28738d8ca1b210f6a547a46` birebir.
  **Dürüst:** M7 ilk turda **yeşildi** (senaryo yoktu: eski isteğin geç
  *başarısı* ölçülmüyordu) → sıra kuralını ölçen test eklendi (11. kusur),
  M7 yeniden ölçüldü → `1 failed`.
- **Ölçüm:** `tsc --noEmit` temiz · `vitest` **1092/1092** (60 dosya;
  1080 → +12) · `vite build` temiz (**586,43 kB**, `index-BJVGyxJU.js`, gzip
  **158,27 kB**; CSS `index-CPW4QqlF.css` 7,28 kB).

### Sınırlamalar (dürüst)

- jsdom **tarayıcı değil**: gerçek Tauri penceresi, gerçek işaretçi çift
  tıklaması ve **görsel QA yok**; `api` mock'lu olduğu için `ai_consent_*`,
  `ai_run_execute`, `ai_candidate_*` SQL'i ve Rust **koşmaz**.
- Kapılar **arayüz düzeyinde**: doğrudan IPC bunları atlar. `busy` tek bayrak
  (kuyruk değil): uçuştaki ikinci yazma düğmede kapalı; `promoteBusy` ve
  `revokeBusy` ayrı bayraklar (düğme metni değişmez).
- Çift tık `disabled` üzerinden ölçülür; gerçek işaretçi zamanlaması ölçülmedi.
- **Açık kayıt — onay iptali ↔ çalıştırma:** iptal uçuşta iken “Çalıştır”
  **açıktır**; iptal edilmekte olan onayla çalıştırma başlatılabilir (çekirdek
  yarışı; ölçülmedi, kapatılmadı — `revokeBusy` çalıştırma kapısına eklenmedi,
  çünkü yeni onay alındığında eski iptalin uçuşu yeni onayı engellememeli).
- **Açık kayıt — rapor ↔ taslak metni:** tür değişimi raporu temizler; taslak
  **metni** düzenlenince temizlemez (rapor gönderilen taslağın sorunlarını
  gösterir, kullanıcı düzeltirken okumaya devam edebilsin — bilinçli).
- **Açık kayıt:** `revalidate` başarısı bağlam kapısı taşımaz (aday kimlikleri
  çakışmadığı için görünür etkisi yok — bilinçli); görsel QA **insan bekliyor**.
- **Kimlik:** commit mesajı `F11-T05` ile atıldı; ekranın defter kimliği
  **F11-T04** (T05 = şema doğrulama paneli). Bu turun kilit katmanı T05
  panelinin yazma yollarını da kapsar; yeni numara uydurulmadı.

## F11-T04 — sağlayıcı-snapshot çekirdek denetimi: onay varış yerine bağlandı (2026-11-05)

Önceki tur onayın **amaca** bağlılığını arayüzde kapatmıştı; defterde açık kalan
çekirdek kaydı duruyordu: `consent_scope_segments` onayın `provider_snapshot`ını
çalıştırılan sağlayıcıyla karşılaştırmıyordu ve `ai_run_execute` varış yerini
“ilk api profili”nden **tahmin ediyordu**.

### Ölçülen kusurlar (4)

1. **Onay sağlayıcıya bağlı değildi (en ağırı):** `NewTransferConsent.provider_snapshot`
   **çağıranın yazdığı** bir dizeydi; `start_ai_run` yalnız onayın etkinliğine, dosya
   kapsamına ve belge canlılığına bakıyordu. Doğrudan IPC ile A profilinin onayı B
   profiliyle çalıştırılabiliyordu — izin kendiliğinden genişliyordu.
2. **Varış yeri seçili profilden okunmuyordu:** `ai_run_execute`
   `api_profile_endpoint()` ile **ilk** api profilini alıyordu; onay başka profil
   için alınmış olsa da içerik o profile gidiyordu.
3. **Uç nokta/model değişimi onayı geçersizleştirmiyordu:** snapshot yalnız
   `adapter@politika`ydı; aynı adapter sürümüyle **başka** uç noktaya/modelle
   gönderim eski onayla yapılabiliyordu.
4. **API profili güncellemesi sessizce yok sayılıyordu:** `register_api_profile`
   var olan api profilini bulunca yeni değerleri **hiç yazmıyordu**; arayüz
   “Kaydedildi” yazarken gönderim **eski** uç noktaya gidiyordu.

### Düzeltme

`crates/storage/src/ai.rs` (+301/−36), `apps/desktop/src-tauri/src/commands.rs`
(+21/−3), `crates/storage/src/error.rs` (+3), `apps/desktop/src/lib/api.ts`
(+2/−2), `apps/desktop/src/screens/Ai.tsx` (+4/−6), `Ai.test.tsx` (+3/−3).

- `NewTransferConsent.provider_snapshot` → **`provider_id`**: kimlik dizesini çağıran
  yazmaz; çekirdek `provider_snapshot(adapter, policy, capabilities)` ile türetir.
  API profilinde kayıtlı **uç nokta + model**in kısa sha256 parmak izi dizeye girer
  (`openai-compatible@1#a1b2c3d4e5f6`).
- `create_transfer_consent` profili **okur**: bilinmeyen profil `E_AI_PROVIDER`,
  `off` profili yeni `E_AI_OFF` (`StorageError::AiOff`) — “hiçbir yere gönderme”
  onayı bir yetki değildir.
- `start_ai_run` onayın özetini **çalıştırılan profilinkiyle** karşılaştırır;
  uç nokta ya da model değiştiyse `E_AI_CONSENT`.
- `ai_run_execute` varış yerini **seçili** profilden okur
  (`api_destination(provider_id)`); API profili değilse sessizce başka yere
  göndermek yerine `E_AI_PROVIDER` ile reddeder.
- `register_api_profile`: aynı değerler → idempotent; **farklı** değerler → `UPDATE`
  (+ denetim kaydı `update`, `row_version` artar).

### Testler ve kapı kanıtı

- `ai.rs` **+6 test** (17 → 23): `api_profile_update_replaces_destination`,
  `api_endpoint_change_invalidates_old_consent`,
  `api_model_change_invalidates_old_consent`, `consent_requires_a_known_provider`,
  `consent_for_off_provider_is_rejected`, `api_destination_is_profile_scoped`.
- **Kapı kanıtı 6/6** (her koşu orijinal metinden; geri yükleme sha256
  `cc80346cae31120e5b83061e3ecb7966cf7e07a792755aec4910167bc9856d17` birebir):
  M1 parmak izi kapalı (**2** — uç nokta + model), M2 `off` kapısı (1), M3 bilinmeyen
  profil kabul (1), M4 çalıştırmada karşılaştırma (**3** — iki geçersizleşme +
  `consent_is_provider_scoped`), M5 `api_destination` profil kapsamı (1), M6 güncelleme
  yok sayılır (**3** — iki geçersizleşme testi de güncelleme yoluna bağlı).
- **Ölçüm:** `cargo test -p dim-storage` tüm paket yeşil (lib 391 test; ai 23/23) ·
  `cargo check --all-targets` masaüstü temiz (2 dk 51 sn) · `tsc --noEmit` temiz ·
  `vitest` **1120/1120** (60 dosya) · `vite build` temiz (**588,78 kB**,
  `index-DTcoOR6K.js`, gzip **158,94 kB**).

### Sınırlamalar (dürüst)

- `commands.rs`teki “API profili değil” kapısı **derlendi, koşulmadı**:
  `State<AppState>` kuran test altyapısı yok; `commands_tests.rs` CI'da koşmuyordu (2026-11-06: `masaüstü kabuk` işi bu testleri artık koşuyor).
- Parmak izi **uç nokta + model** metninden türetilir; kimlik doğrulama modu,
  başlıklar ve TLS ayarları dizeye girmez.
- Bu değişiklikten **önce** alınmış onayların özeti parmak izi taşımaz → güncelleme
  sonrası **yeniden onay** gerekir (kapalı yönde güvenli; sessizce genişlemez).
- Aynı `adapter@politika` + aynı `capabilities` taşıyan iki profil aynı varış yeri
  sayılır (özet `provider_type` taşımaz); arayüzde böyle bir çakışma üretilemez.
- Gerçek ağ aktarımı, gerçek Tauri penceresi ve görsel QA **insan bekliyor**.

## Gönderim sicili segment düzeyinde — alıntı önce gönderilen metinde (2026-11-08)

**ÖLÇÜLEN KUSUR (üç katman, tek sınıf):** şartname `transfer_segments` tablosunu
“gönderilen metin ve özgün aralık eşlemesi şifreli tutulur” diye tanımlar, doğrulama
sırasını da “şema → kapsam → **gönderilen alıntı eşleşmesi** → yerel kaynak eşlemesi”
diye bağlar. Üründe tablo şemanın ilk sürümünden beri vardı ama **tek satır
yazılmıyordu**; `ai_context_items.segment_ids` her çalıştırmada `'[]'` yazılıyordu
(hangi blokların gittiği kayıtsızdı) ve alıntı doğrulaması **en yeni ayrıştırmada**
arıyordu. Sonuç: belge yeniden ayrıştırıldıktan sonra model **hiç görmediği** metinden
“doğrulanmış” alıntı üretebiliyor, gönderilmeyen bir belgeden gelen alıntı da
`exact` görünebiliyordu — kaynak izlenebilirliğinin tam ortasında sessiz boşluk.

### Yazılan

- **Şema v26** (`0026_transfer_segments.sql`): `transfer_segments.document_version_id`
  sütunu (hangi sürümün segmenti olduğu; eski satırlar `NULL` kalır ve “gönderilmiş”
  sayılmaz), `ai_run_id` indeksi, `(ai_run_id, outbound_segment_id)` **UNIQUE**.
  `CURRENT_SCHEMA_VERSION = 26`.
- **`crates/storage/src/ai.rs`:** `NewTransferSegment`, `TransferSegmentRow`, `SentSegment`;
  `record_transfer_segments` metni **kasadaki** `text_segments.raw_text` ile birebir
  doğrular (beyan edilen metin yazılamaz), karakter/bayt sayısı + `sha256` + kimlik
  aralık haritası (`source_range_map`) çekirdekte hesaplanır; mükerrer satır ve yanlış
  metin `E_DB_SETTING`, bilinmeyen/başka sürüme ait segment `E_NOT_FOUND`; **hata
  hâlinde hiçbir satır yazılmaz** (tek işlem). `list_transfer_segments` kayıt sırası
  (`rowid ASC`), tam metin yerine ilk 120 karakter. `sent_segments` doğrulama yolu.
  `record_context_items` artık **gerçek** `segment_ids` yazar (her kimlik kasadan
  doğrulanır, uydurma kimlik `E_NOT_FOUND`); `AiContextItemRow.segment_count` okunur.
- **`crates/storage/src/ai_candidate.rs`:** alıntı başına `outbound_state` + rapor
  düzeyinde `outbound_state`. **Gönderilen metin kapısı:** sicil varsa alıntı önce
  gönderilen segmentlerde aranır — yoksa `not_found`/`not_in_outbound`; belge hiç
  gönderilmediyse `unverifiable`/`document_not_sent` (**failed**); gönderilen metinde
  olup güncel metinde kalmayan alıntı `unverifiable`/`source_changed` — **uydurma
  değil**, kısmi. Yeni sorun kodları: `quote_not_in_outbound`,
  `quote_document_not_sent`, `quote_source_changed`.
- **`commands.rs` + `lib.rs`:** `ai_run_execute` gönderimden **önce** context ve
  segment sicilini yazar; ikisi de yazılamazsa **hiçbir bayt dışarı çıkmaz**. Yeni
  `ai_transfer_segments` komutu kayıtlı.
- **Arayüz:** “Gönderilen içerik” kartına **segment** sütunu (`segment_count: null` →
  “okunamadı”, “0” denmez); yeni **“Gönderilen segmentler”** kartı (sıra · segment
  kimliği · karakter · sha256 öneki · metin başı) kendi `segmentsSeq` geç-yanıt
  kapısıyla, düşen okuma **“sicil yok” demez**; raporda gönderim rozeti + alıntı
  tablosunda “Gönderim” sütunu. Bilinmeyen durum **ham** yazılır.

### Testler ve kapı kanıtı

- `ai::tests` +3 (`transfer_segments_record_identity_map_and_hash`,
  `transfer_segment_text_must_match_the_vault`, `context_items_record_real_segment_ids`)
  ve `transfer_records_never_store_document_text` →
  `transfer_metadata_never_stores_document_text` (metnin **yalnız** kasa segmenti ve
  gönderim sicilinde olduğunu kanıtlar) → `dim-storage --lib` **452 → 458**.
- `ai_candidate::tests` +3 (`quotes_are_checked_against_the_sent_text_not_the_latest_parse`,
  `quote_from_a_document_outside_the_sent_scope_is_not_verified`,
  `runs_without_a_sent_ledger_keep_document_checking`).
- `Ai.test.tsx` +6 (sicil okunur · düşen okuma “sicil yok” demez · dosya değişiminde
  düşer · geç yanıt yazılmaz · rapor rozeti `recorded` · rozet `not_recorded`) →
  yerel `108/108` (dosya), tam arayüz **1314/1314** (64 dosya).
- **Kapı kanıtı 12/12 kırmızı** (her mutasyon tek başına, kaynak `try/finally` ile
  geri yüklendi, sha256 birebir; çekirdek `ai.rs c30d395b…`, `ai_candidate.rs 423636c7…`,
  arayüz `Ai.tsx 59b3238c…`): C1 metin beyanı kasadan doğrulanmaz · C2 mükerrer sicil
  reddi kapalı · C3 `segment_ids` hep `'[]'` · C4 hash metinden değil kimlikten ·
  C5 aralık eşlemesi `outEnd=0` · C6 gönderilen metin kapısı kapalı · C7 gönderilmeyen
  belge kapısı kapalı · C8 `source_changed` kapısı kapalı · U1 sicil hiç okunmaz ·
  U2 geç yanıt kapısı kaldırılır · U3 düşen okuma hata yutulur · U4 rozet her zaman
  “sicil var” der.
- **Ölçüm (yerel, Node 22.23.1 + TZ=UTC):** `tsc --noEmit` temiz · vitest **1314/1314**
  (64 dosya; `Ai.test.tsx` 92) · `dim-storage --lib` **458/458** (224,20 s) ·
  `cargo fmt` iki manifest temiz · `clippy -p dim-storage --all-targets -D warnings`
  temiz · masaüstü `clippy --all-targets -D warnings` temiz (46,05 s) · masaüstü kabuk
  kapısı **40 koştu · 0 düştü · 5 yoksayıldı (kaynak 45)**.
- **Commit/CI (ölçüldü):** kod `0722d5c` (11 dosya, **+1252/−30**) → **`42498863213`
  10/10 YEŞİL**: `dim-storage` **458** hem `macos-15` (182,06 s) hem `ubuntu-latest`
  (512,88 s) — birebir; `arayüz` **64 dosya / 1313 test**, paket `index-fgDGwFXL.js`
  **651,62 kB** / gzip **174,17 kB** (CSS 7,28 kB / gzip 2,07); `masaüstü kabuk`
  `40 passed · 5 ignored` + yoksayılan koşu `5 passed · 40 filtered`; diğer altı iş yeşil.
  **i18n hunk ayrımı:** üç hunk'tan yalnız benim iki hunk'ım `git apply --cached
  --recount` ile sahneye alındı; ağaçtaki başka ajanın `cases.parseStatus*` üç anahtarı
  commit'e **girmedi** (`git show :…i18n.ts | grep -c parseStatus` → **0**). Yerel 1314 /
  CI 1313 farkı ağaçtaki onun commit'siz `format.test.ts` testi.

### Sınırlamalar (dürüst)

- Dönüşüm bugün **kimliktir**: maskeleme planının gönderime uygulanması ve
  maskeli→özgün aralık eşlemesi **yok** (sütunlar ve `source_range_map` bunun için hazır;
  `transform_manifest_id` NULL → `transform_kind=identity`).
- Gönderilen metin kasada **ikinci kez** durur (segment + sicil) → kasa büyür; sicil
  satırı denetim gereği **silinemez** ve arayüzden düzenlenemez.
- Sicil yalnız `ai_run_execute` yolunda yazılır; elle kaydedilen adaylarda
  `not_recorded` (eski davranış korunur, ekran bunu **yazar**).
- Yeni `ai_transfer_segments` komutu **komut olarak koşulmadı** (`State<AppState>`
  altyapısı yok): kablolama check + clippy, davranış storage düzeyinde gerçek SQLite.
- jsdom tarayıcı değil → gerçek Tauri penceresi ve **görsel QA insan bekliyor**.

## F11-T03 — gönderim maskesi ve maskeli bağlamın geri eşlenmesi (2026-11-08)

**Ölçülen boşluk.** Şartname §737 maskeli bağlamın **iki aşamalı** doğrulamasını
ister: «Her gönderim segmenti, gönderilen metin/hash, **maskeleme dönüşüm sürümü**
ve özgün Unicode aralıklarına eşleme taşır. Modelin alıntısı önce gördüğü maskeli
kopyada doğrulanır. Sonra yalnız kasada bulunan eşleme, ilgili özgün segment/sürüme
bağlanır. Maske hiç uygulanmadıysa eşleme özdeşliktir. (…) geri eşlenemeyen alıntı
doğrulanmış sayılmaz.» Önceki tur birinci aşamayı yazmıştı (alıntı **gönderilen**
metinde aranır); dönüşüm ise her satırda **kimlikti**: maske uygulanmıyor,
`transform_manifest_id` hiç yazılmıyor, `source_range_map` tek bir `[0..n → 0..n]`
parçasıydı. Yani ürün «maskeli bağlam» diye bir yol **sunmuyordu**; maske yalnız
PDF çıktısında (F13) vardı ve modele giden metne hiç dokunmuyordu.

**Yazılan.**

- **Şema v27** (`0027_outbound_transforms.sql`): `outbound_transforms`
  (`kind`, içerik adresli `settings_hash`, `term_count`, `placeholder`; **terimlerin
  kendisi saklanmaz**) + `transfer_segments.source_char_count` (harita bütünlük
  kapısı). `CURRENT_SCHEMA_VERSION = 27`.
- **`crates/storage/src/outbound_mask.rs` (yeni, 505 satır):** Türkçe kurallı
  **1:1** harf katlaması (`İ`→`i`, `I`→`ı`; `to_lowercase`'in iki karakter üretip
  aralığı kaydırması engellenir), çakışan ve **bitişik** terimlerin tek yer
  tutucuda birleşmesi, `MaskedText` + kesintisiz `MaskRun` haritası,
  `map_span_in_runs` (maske parçasına değen aralık → `None`; uydurma konum yok),
  `parse_runs` (kesintili/bozuk harita reddedilir), terim doğrulama (2–200
  karakter, ≤500 terim, kontrol karakteri yok, yer tutucu çakışması reddi).
- **`ai.rs`:** `OutboundMask`, `OutboundSegmentRef`, `OutboundSegment`,
  `OutboundRecord`. `record_outbound_segments` metni **çağırandan almaz**: kasadaki
  `text_segments.raw_text`i okur, maskeyi **çekirdekte** uygular, sicile maskeli
  metni + `sha256` + haritayı + dönüşüm kimliğini yazar ve **gönderilecek metni
  geri döndürür** — kayıt ile gönderim ayrışamaz. Hata hâlinde hiçbir satır
  yazılmaz. `record_transfer_segments` beyan kapısını (metin kasadakiyle birebir)
  koruyarak bu yola delege eder. `sent_segments` haritayı ve dönüşüm türünü taşır;
  `list_transfer_segments` `outbound_transforms` ile birleşir (`identity` ·
  `mask_terms` · `unknown` + terim sayısı).
- **`ai_candidate.rs`:** maskeli satırda alıntı yalnız **tek birebir konumdan**
  geri eşlenir. Maske aralığına düşen alıntı `quote_masked`, maskeli metinde
  yalnız zayıf katmanda kalan alıntı `quote_unmapped`, okunamayan harita
  `map_unreadable` — üçü de **failed** ve sorun koduyla raporlanır. Eşleme
  başarılıysa rapor `masked`, `transformKind`, `sourceCharStart/End` taşır; kimlik
  satırlarında davranış değişmez.
- **`commands.rs`:** `ai_run_execute` `maskTerms` alır; bağlam satırındaki bayt ve
  makbuz artık **maskeli** metinden sayılır (`bytes_sent` = kasadan çıkan gerçek
  boyut). **Arayüz:** "Gönderim maskesi" alanı (satır başına terim; ölçü çekirdekle
  aynı — kod noktası, 2–200, ≤500), geçersiz liste çalıştırmayı **kapatır**;
  sicilde maske türü/terim sayısı, raporda geri eşlenen özgün aralık; `limits`
  metni yeni sözleşmeye göre düzeltildi.

**Testler ve kapı kanıtı.**

- `outbound_mask::tests` **9** (kimlik, tek terim + harita, Türkçe katlama,
  çakışan/bitişik birleştirme, sınırı aşan aralık, yer tutucu çakışması, terim
  doğrulama, bozuk harita) + `ai::tests` **+3** (maskeli sicil + harita + terim
  sızıntısı yok; reddedilen maske satır bırakmaz; kimlik haritası) +
  `ai_candidate::tests` **+3** (maskeli alıntı özgün aralığa döner; maske içine
  düşen alıntı doğrulanmaz; zayıf katman `quote_unmapped`) → `dim-storage --lib`
  **458 → 473**.
- `Ai.test.tsx` **+6** (terimler çalıştırmaya gider · maske yoksa dört argüman ·
  geçersiz terim düğmeyi kapatır · dosya değişince alan temizlenir · sicilde maske
  türü/terim sayısı · raporda özgün aralık) → `Ai.test.tsx` **98**, tam arayüz
  **1320/1320** (64 dosya).
- **Kapı kanıtı 13/13 kırmızı** (betik `scratch/dim-mask-gates.py`; her mutasyon
  tek başına, `try/finally` + sha256 birebir geri yükleme — `outbound_mask.rs
  90898777…`, `ai.rs 3dbddfb4…`, `ai_candidate.rs 951486a5…`, `Ai.tsx 9706a390…`):
  C1 maske hiç uygulanmaz · C2 terim alt sınırı 2→1 · C3 yer tutucu çakışması
  denetlenmez · C4 maske parçası haritada «kopya» sayılır · C5 sicile ham metin
  yazılır · C6 dönüşüm kimliği yazılmaz · C7 okunan harita yok sayılır ·
  C8 maske içine düşen alıntı yine doğrulanır · U1 maske çalıştırmaya geçirilmez ·
  U2 geçersiz maske düğmeyi kapatmaz · U3 dosya değişiminde alan temizlenmez ·
  U4 maske türü etiketi yazılmaz · U5 geri eşlenen aralık yazılmaz.
- **Ölçüm (yerel, Node 22.23.1 + TZ=UTC):** `tsc --noEmit` temiz · vitest
  **1320/1320** (64 dosya) · `dim-storage --lib` **473/473** (236,46 s) ·
  `cargo fmt` iki manifest temiz · storage + masaüstü `clippy --all-targets
  -D warnings` temiz. Yerel `vite build` koşulmadı → paket CI'dan.
- **Commit/CI (ölçüldü):** kod `cac62cd` (11 dosya, **+1602/−113**) →
  **`42507035630` 10/10 YEŞİL**: `dim-storage` **473** hem `macos-15`
  (196,20 s) hem `ubuntu-latest` (412,94 s) — **birebir**; `arayüz` **64 dosya /
  1319 test**, paket `index-Ci6W12PB.js` **653,29 kB** / gzip **174,75 kB**
  (CSS 7,28 kB / gzip 2,07); `masaüstü kabuk` **40 koştu · 0 düştü ·
  5 yoksayıldı (kaynak 45)** + yoksayılan koşu **5 passed · 40 filtered**;
  diğer altı iş yeşil. **i18n hunk ayrımı:** üç hunk'tan yalnız benim iki
  hunk'ım `git apply --cached --recount` ile sahneye alındı; ağaçtaki başka
  ajanın `cases.parseStatus*` anahtarları commit'e **girmedi**
  (`git show :…i18n.ts | grep -c parseStatus` → **0**). Yerel 1320 / CI 1319
  farkı ağaçtaki onun commit'siz `format.test.ts` testi.

**Sınırlamalar (dürüst).**

- Maske terim listesi **kullanıcıdan** gelir. F13 maskeleme **planı** (bölge/quad)
  gönderim metnine **hâlâ uygulanmaz**: plan yalnız bölge geometrisi ve terim
  **hash'i** saklar, bölge→karakter aralığı eşlemesi yoktur. Bu tur metin düzeyinde
  maske + harita altyapısını kurdu; plan→gönderim köprüsü ayrı iştir.
- Eşleşme **birebir** (harf katlamalı) alt dizedir: çekim eki alan adlar
  (`Ahmet Yılmaz'a`) gövdeyi maskeler, **eki metinde bırakır**; bitişik yazım ve
  kısaltma varyantları maskelenmez (liste elde tutulmalı).
- Yer tutucu sabittir (`[MASKED]`); metin bu dizgeyi zaten içeriyorsa maske
  **reddedilir** (belirsizlik yaratılmaz).
- Terimlerin kendisi sicile yazılmaz (özet + sayı). Aynı maskeyi yeniden üretmek
  için kullanıcı listeyi yeniden girmelidir; maskeli metin ve harita kasadadır.
- Maske yalnız `ai_run_execute` yolunda; elle kaydedilen adaylarda `not_recorded`
  (eski davranış korunur ve ekran yazar).
- `ai_context_items.bytes_sent` artık **gönderilen** (maskeli) bayt: maskesiz eski
  satırlarla birebir karşılaştırılmamalı.
- Yeni komut yolu **komut olarak koşulmadı** (`State<AppState>` test altyapısı
  yok): kablolama check + clippy, davranış storage düzeyinde gerçek SQLite ve
  arayüz testleriyle ölçüldü.
- jsdom tarayıcı değil → gerçek Tauri penceresi ve **görsel QA insan bekliyor**.

## F11-T03 eki — gönderim önizlemesi **maskeyi taşır** (2026-11-09)

**Ölçülen kusur (şartname §11.7).** Onay/önizleme ekranı "seçili dosya ve
belgeler, gönderilecek metin, **maskeleme durumu**, tahmini kapsam" göstermek
zorundadır. Ölçüm: `consent_preview` maskeyi **hiç bilmiyordu** — örnek metin
hamdı ve bayt sayısı maskesizdi; oysa `ai_run_execute` maskeyi uygulayıp
**maskeli** metni gönderiyordu. Yani aynı kod yolunun kendi yorumu ("önizleme
ile gönderilen içerik ayrışamaz") maske seçildiğinde **kırılıyordu**: kullanıcı
ham adı görüp onay veriyor, dışarı maskeli metin çıkıyordu (ters yönde de
yanıltıcı: bayt sayısı gönderilenden büyük görünüyordu).

**Yazılan.**

- `crates/storage/src/ai.rs`: `consent_preview_masked(case_id, ids, terms, plan)`
  — maske **doğrulanır** (`validate_terms`), örnek segment maskeli gösterilir ve
  bayt sayısı **maskeli metinden** sayılır. `ConsentPreview` artık
  `bytesUnmasked` ve `mask: Option<ConsentMaskPreview>` taşır
  (`termCount`, `maskedRuns`, `maskedChars`, `sampleMasked`, `bytesExact`,
  `planId`) — **terim listesi yine taşınmaz**.
- **Kapsam bütçesi:** `mask_terms` maliyeti metin × terim olduğundan
  `PREVIEW_MASK_CHAR_BUDGET` (1.000.000 karakter) üstünde yalnız **örnek**
  maskelenir; `bytesExact=false` yazılır ve bayt sayısı "maskesiz üst sınır"
  olarak sunulur. Sessizce "maskeli" denmez.
- **Plan köprüsü:** plan kimliği verilirse terimler çekirdekte türetilir ve plan
  bu kapsamdaki bir belgeye ait değilse önizleme, gönderimle **aynı** hatayı
  verir: `E_MASK_PLAN_SCOPE` (onay oluşmaz).
- `apps/desktop/src-tauri/src/commands.rs`: `ai_consent_preview` `maskTerms` +
  `maskPlanId` alır (plan → `outbound_terms_from_plan`, aynı kapı).
- `Ai.tsx`: önizleme çağrıları (onay + "Gönderilecek içeriği göster") maskeyi
  taşır; kartta maske satırı (terim/eşleşme sayısı, maskeli bayt ya da üst sınır,
  plan rozeti). **Maske değişirse "Çalıştır" kapanır** ve nedeni yazılır
  (`previewMaskSig`): onaylanan metin ile çıkan metin ayrışmaz; yeniden "Onayla"
  önizlemeyi maskeyle tazeler. 7 sözlük anahtarı.

**Testler.** `ai::tests` **+6** (maskeli önizleme = gönderilen bayt/örnek birebir;
maske yokken ham; eşleşme yokken "maskelendi" denmez; plan kapsam dışı
`E_MASK_PLAN_SCOPE`; bütçe üst sınırı işaretler; geçersiz terim reddi) →
`dim-storage --lib` **484 → 490**. `Ai.test.tsx` **101 → 105** (+4: maskeyi
çekirdeğe taşır + maskeli satır, onaydan sonra maske değişince çalıştır kapanır,
plan kimliği önizlemeye gider, kapsam dışı planda onay oluşmaz) — bir eski test
yeni sözleşmeye göre yeniden yazıldı (maske artık onaydan **önce** girilir).

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-preview-mask-gates.py`, her
mutasyon tek başına, `try/finally` + sha256 birebir):** **11/11 kırmızı** —
C1 maske baytı sayılmaz · C2 örnek maskelenmez · C3 plan kapsam kapısı kapalı ·
C4 terim doğrulaması kapalı · C5 bütçe yok sayılır · C6 eşleşme yokken
"maskelendi" denir · U1 önizleme maskeyi taşımaz · U2 maske imzası yazılmaz ·
U3 çalıştır kapısı maskeyi yok sayar · U4 maske satırı yazılmaz · U5 plan kimliği
önizlemeye gitmez.

**Dürüst sınırlar.**

- Bütçe (1M karakter) üstünde bayt sayısı **maskesiz üst sınırdır**
  (`bytesExact=false`); kullanıcıya böyle yazılır. Tam sayı için daha hızlı bir
  eşleştirici (Aho–Corasick) gerekir — bu turda yapılmadı.
- Maske **onaydan sonra** değişirse çalıştırma kapanır ve yeniden onay ister;
  bu bilinçli bir sürtünmedir (izin kendiliğinden değişmez). Onay kaydı
  (`transfer_consents`) belge kümesine bağlıdır; **maske imzası onay satırına
  yazılmaz** — çekirdek sözleşmesi değişmedi, kapı arayüz + önizleme düzeyinde.
- Önizleme çağrısı yine tüm kapsamın metnini okur (mevcut davranış); maske
  maliyeti bütçeyle sınırlandı, okuma maliyeti sınırlandırılmadı.
- Komut yolu **komut olarak koşulmadı** (`State<AppState>` altyapısı yok):
  kablolama check + clippy, davranış storage düzeyinde gerçek SQLite.
- jsdom tarayıcı değil → gerçek Tauri penceresi ve **görsel QA insan bekliyor**.

**Commit / CI (ölçüldü).** Kod `28f0dd1` (8 dosya, **+549/−25**) → run `42687029029`
**10/10 YEŞİL**: `dim-storage` **490** hem `ubuntu-latest` (322,26 s) hem
`macos-15` (251,61 s) — **birebir**; `arayüz` **64 dosya / 1326 test** (yerel 1327
farkı ağaçtaki başka ajanın commit'siz `format.test.ts` testi), paket
`index-B5i3FPyB.js` **656,52 kB** / gzip **175,59 kB** (CSS 7,28 kB / 2,07);
`masaüstü kabuk`, `windows güvenli depo`, `sözleşmeler`, `kaynak arşivi`,
`paket manifesti`, `belge worker'ı` ve `SBOM` yeşil. **i18n hunk ayrımı:** iki
hunk'tan yalnız benim `previewMask*`/`maskChangedSincePreview` hunk'ım
`git apply --cached --recount` ile sahneye alındı; başka ajanın
`cases.parseStatus*` üç anahtarı commit'e **girmedi**
(`git show HEAD:…i18n.ts | grep -c parseStatus` → **0**).

## Aktarım makbuzu açılış uzlaştırması — kesilen deneme «oturum kesildi» yazar (2026-11-11)

**Ölçülen kusur.** `transfer_receipts` satırı gönderimden **önce** `attempted`
olarak açılır (`begin_transfer_receipt`) ve `ai_run_execute` dönerken
`dispatched|failed` olarak kapanır. Ama süreç gönderim sırasında ölürse (uygulama
kapatıldı, çökme, SIGKILL) satır **sonsuza kadar** `attempted` kalıyordu: arayüz
aylar sonra da "denendi · sonuç yazılmadı" yazıyor, satır hâlâ uçuşta olabilecek
bir deneme gibi duruyordu — oysa sonucu yazacak çağrı artık yok. İş kuyruğu için
tam bu gerekçeyle bir açılış uzlaştırması vardı (`recover_orphaned_jobs` /
`recover_at_open`); gönderim makbuzlarının eşdeğeri **yoktu**.

**Yazılan.**

- `crates/storage/src/ai.rs`: `reconcile_attempted_receipts()` — tek koşullu
  `UPDATE … WHERE status='attempted'` → `interrupted`. Sonuçlanmış satıra
  dokunmaz; geçiş **kesindir** (`finish_transfer_receipt` yalnız `attempted`
  satırı sonuçlandırır, kesilen satır artık sonuçlandırılamaz). Kaç satırın
  kapandığını döndürür ve sayıyı denetime yazar (`transfer_receipt` /
  `interrupt`); **makbuz kimliği, hedef ve özet denetime girmez**. İkinci açılış
  yapacak bir şey bulmaz (idempotent). Denetim yazılamazsa kasa açılışı düşmez
  (yükseltme göçünün `let _ =` deseniyle aynı gerekçe).
- Gerekçe (kodda da yazılı): açılış anında hiçbir gönderim uçuşta olamaz —
  `ai_run_execute` kasa kilidini gönderim boyunca tutar ve komut dönerken makbuz
  sonuçlandırılır; sonuçlandırma yazılamamış olsa bile o çağrı geri gelmez
  (`partial` raporlanır). Yani açılışta `attempted` kalan satırın sonucunu
  yazacak **canlı bir çağrı yoktur**: süreç çökmüş ya da final yazımı düşmüştür.
- `apps/desktop/src-tauri/src/commands.rs` `remember()`: iş kurtarmasının yanında
  uzlaştırma çağrılır.
- `Ai.tsx` + `i18n.ts`: dördüncü durum **«oturum kesildi · sonuç yazılmadı»**
  (`warn` rozet; «gönderildi» **demez**). Kart alt metni iki durumu ayırır:
  «denendi» sonucun yazılmadığını, «oturum kesildi» sonucu yazacak çağrının
  artık olmadığını söyler — içerik çıkmış olabilir.

**Testler.** `ai::tests` **+1** (`reconcile_closes_orphaned_attempts_as_interrupted`:
yalnız yarım kalan satır kapanır; `dispatched`/`failed` değişmez; kesilen satır
sonradan sonuçlandırılamaz `E_AI_RECEIPT`; ikinci çağrı **0**; denetimde sayı
var — kimlik/hedef yok) → `dim-storage --lib` **524 → 525**. `Ai.test.tsx` **+1**
(kesilen makbuz «oturum kesildi» yazar, `warn`, «gönderildi» yazmaz) → yerel
**65 dosya / 1357 test**; `tsc --noEmit` temiz; `cargo fmt` iki manifest temiz;
storage + masaüstü `clippy --all-targets -D warnings` temiz.

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-receipt-recovery-gates.py`, her
mutasyon tek başına, `try/finally` + sha256 birebir):** **9/9 KIRMIZI** —
S1 uzlaştırma hiç kapatmaz · S2 sonuçlanmış satır da ezilir · S3 denetim
yazılmaz · S4 denetim sayıyı taşımaz · S5 kesilen makbuz yeniden
sonuçlandırılabilir · S6 uzlaştırma idempotent değil (kesinleri de sayar) ·
U1 etiket dalı yok · U2 kesilen makbuz yeşil rozet · U3 «gönderildi» yazar.

**Dürüst sınırlar.**

- Uzlaştırma **yalnız kasa açılışında** koşar; açık oturumda gönderim sırasında
  süreç ölürse satır bir sonraki açılışa kadar `attempted` görünür (doğru: o
  oturumda sonuç gelmemişti).
- `interrupted`, "içerik çıkmadı" **demek değildir** — "sonuç yazılmadı" der;
  içerik çıkmış olabilir, çıkıp çıkmadığı sicilden kanıtlanamaz.
- Kesinleşen satır **geri alınamaz** (denetim izi); yeniden deneme yeni bir
  çalıştırma açar.
- Komut yolu (`remember`) **komut olarak koşulmadı** (`State<AppState>` altyapısı
  yok): kablolama check + clippy, davranış storage düzeyinde gerçek SQLite.
- jsdom tarayıcı değil → gerçek Tauri penceresi ve **görsel QA insan bekliyor**.

**Commit / CI (ölçüldü).** Kod `4c1d03b` (5 dosya, **+146/−5**) → run
`42917762357` **10/10 YEŞİL**: `dim-storage` **525** hem `macos-15` (237,68 s)
hem `ubuntu-latest` (479,66 s) — **birebir**; `arayüz` **65 dosya / 1356 test**
(yerel 1357 farkı ağaçtaki başka ajanın commit'siz `format.test.ts` testi), paket
`index-CP778o7x.js` **667,94 kB** / gzip **178,58 kB**; `masaüstü kabuk`
**40 koştu · 0 düştü · 5 yoksayıldı**; diğer altı iş yeşil. **i18n hunk ayrımı:**
üç hunk'tan yalnız benim iki hunk'ım sahneye alındı (`git update-index
--cacheinfo` ile HEAD içeriği + benim değişikliklerim blob olarak yazıldı); başka
ajanın `parseStatus*` üç anahtarı commit'e **girmedi**
(`git show HEAD:…i18n.ts | grep -c parseStatus` → **0**; ağaçta 3).
