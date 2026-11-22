# F11 — Kaynaklı AI inceleme araçları

- **Sürüm etiketi:** `v0.10.0`
- **Durum:** Kısmi
- **Tarih:** 2026-10-24

| İş | Durum | Kanıt |
|---|---|---|
| F11-T01 aday kaydı | ✅ | `ai_candidates`, `valid/invalid/partial`; altı aday türü şeması |
| F11-T02 aday → inceleme kuyruğu | ✅ | `promote_candidate`, `review_items` `pending`, doğrulanmış alıntı → `source_anchors` |
| F11-T03 kapsamlılık raporu | ✅ | belge/sayfa/OCR/boş sayfa/ayrıştırılmamış sayıları; **sayımlar gönderim kapsamıyla tek kuraldan** (güncel sürüm + en yeni sayfa üreten koşu) ve "metinsiz belge" ayrı sayılır — 2026-11-13, aşağıdaki bölüm |
| F11-T04 gerçek model çağrısı | ✅ | `ApiProvider`/`LocalProvider` + `ai_run_execute`: onay + profil + kapsam sicili, hata run'ı `failed` kapatır; onay **varış yerine** bağlı (`provider_id` → türetilmiş `adapter@politika#parmak izi`) |
| F11-T04b gerçek yanıtın aday türü ve şeması | ✅ | `task_candidate_kind` + `candidate_output_instructions`; `ai_run_execute` artık `record_ai_run_candidate` — her gerçek aday `invalid` doğmuyor, istem somut şemayı ve alıntı sözleşmesini taşıyor (2026-11-21, aşağıdaki bölüm) |
| F11-T05 şema doğrulama + alıntı eşleştirme | ✅ | `crates/storage/src/ai_candidate.rs`, `ai_candidate_validate`; alıntı **önce gönderilen metinde** aranır (şema v26, `transfer_segments` + `record_transfer_segments`) |
| F11-T06 kapsamlılık raporunun rapora gömülmesi | ✅ | JSON `coverage`, DOCX/PDF bölümü, CSV `#` bloğu, `KAPSAM.md` + `manifest.coverage` |
| F11-T07 özet cümlesi → dayanak bağı | ✅ | `quotes[].sentences` (birebir cümle kopyası) + `SentenceAttribution` — cümle başına `backed`/`weak`/`unsourced`; kaynaksız yorum ayrı blokta, bağ yoksa «cümle bağı yok» (uydurma damga yok); kayıtlı satırda her okumada yeniden hesaplanır (2026-11-22, aşağıdaki bölüm) |
| F11-T08 soru-cevap: incelenen kapsam + sınırlı yanıt | ✅ | `answer_basis` — kapsam **fiilen gönderilen** segmentlerden, dayanak alıntı raporundan; `sufficient`/`limited` + kararlı neden kodları, `E_AI_ANSWER_LIMITED` terfi kapısı; amaç «soru», 3–500 karakter soru metni `ai_messages` **user** satırı + istemde `Soru:`; arayüzde kapsam satırı ve «sınırlı yanıt» rozeti (2026-11-21, aşağıdaki bölüm) |
| F11-T09 çelişki adayı iki kaynakla | ✅ | `CandidateKind::Contradiction` — `sides` **tam iki ayrı alıntı**, `dateKind` kapalı küme; okuma anında `contradiction_basis` (`presentable`/`role_difference`/`sources_unverified`/`unlinked`), uyarı `validation_state`i `partial`a çeker; `E_AI_CONTRADICTION` terfi kapısı + kuyruk türü normalizasyonu + **tam iki anchor**; arayüzde iki taraf yan yana + rol farkı uyarısı (2026-11-22, aşağıdaki bölüm) |
| F11-T11 talimat saldırısı, sahte kaynak, şema taşması | ✅ | `instruction_risk` — 29 iğneli, 5 kararlı kodlu deterministik tarama (Türkçe 1:1 katlama, satır başı rol işaretçisi, segment/kod başına tek satır, 2.000.000 karakter bütçesi → `truncated`); **onaydan önce** `ConsentPreview.instruction` (maske tam uygulandıysa `basis="sent"`, yetmediyse dürüst `pre_mask`); `ai_attack_assessment(run_id)` üç değerlendirmeyi **okuma anında** kanıtlarından sayar (gönderilen `transfer_segments`, `quote_report`, `too_long`/`too_many_items`); sahte kaynak hükmü yalnız `not_found`; hiçbiri engel değil (2026-11-22, aşağıdaki bölüm) |
| F11-T12 model/istem sürümü kalite ölçümü | ✅ | `ai_quality_report(case_id)` — model · istem sürümü başına çalıştırma/aday/kaynak doğruluğu/inceleme; üç kanıt ailesi **okuma anında** sayılır, kalıcı damga yok; `unverifiable` payda dışı, payda 0 → «ölçülemedi» (`%0` değil); yerine geçmiş öğe isabet dışı, son karar kazanır; düzeltme emeği vekil gösterge (2026-11-22, aşağıdaki bölüm) |
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

## F11-T03 eki — kapsam raporu **gönderilebilir korpusu** anlatır (2026-11-13)

**Ölçülen kusur (üç yüzü; düzeltme öncesi sayımla yeniden üretildi).** Kapsam raporu
(`Database::coverage_report`) her dışa aktarmaya gömülen **kanıt** bloğudur (`KAPSAM.md`,
JSON `coverage`, DOCX/PDF bölümü, CSV `#` bloğu) ve "İncelenmedi ≠ temiz" cümlesini taşır.
Oysa sayıları gönderim kapsamıyla **aynı dili konuşmuyordu**:

1. `pages`/`segments` vakadaki **bütün** ayrıştırma koşularını sayıyordu: aynı sürüm ikinci
   kez ayrıştırıldığında sayfa sayısı ikiye katlanıyor (ölçüm: 3 sayfa → **5**), eski
   sürümlerin sayfaları da ekleniyordu.
2. `unparsed_documents` "belgenin **herhangi** bir sürümü ayrıştırıldı mı"ya bakıyordu.
   Güncel sürümü hiç ayrıştırılmamış belge (v1 ayrıştırıldı, v2 geldi) "ayrıştırıldı"
   sayılıyordu; oysa `resolve_transfer_scope` aynı belgeyi `not_parsed` ile **atlıyor** →
   ihraç "Kapsam durumu: **tam**" derken AI çalıştırması belgeyi sessizce göndermiyordu
   (ölçüm: rapor 2 gönderilebilir belge, gönderim **1**).
3. "Metinli sayfa" **ikinci** bir tanımla (`text_segments` satırı var mı) ölçülüyordu;
   alıntı denetimi ise `trim(...) <> ''` diyor (F12-T14i). Yalnız boşluk taşıyan segment
   kapsamda "metinli", alıntı taramasında "taranmadı" olurdu.

**Yazılan.**

- `coverage_report` **tek kural** kullanır: her belgenin **güncel sürümü + o sürümün sayfa
  üreten en yeni ayrıştırma koşusu** (CTE `current_runs`; `resolve_transfer_scope` ile
  birebir aynı seçim, aynı `ORDER BY`). `pages`/`segments`/`empty_pages`/`ocr_pages`
  yalnız bu korpustan sayılır; `versions`/`parse_runs`/`failed_parse_runs` **tarihçedir**.
- `unparsed_documents` = güncel sürümü olmayan **ya da** güncel sürümü hiç ayrıştırılmamış
  belge (gönderimin `no_current_version`/`not_parsed` atlamalarıyla aynı).
- Yeni **`documents_without_text`**: ayrıştırma koşusu var ama hiçbir sayfasında okunabilir
  metin yok (OCR'sız tarama) — gönderim bu belgeyi `empty_text` ile atlar; `complete`
  ("Kapsam tam") artık bunu da ister (bugün `empty_pages > 0` tarafından da kapsanır;
  belge düzeyindeki kapı savunma olarak durur).
- "Sayfada kullanılabilir metin" **tek tanım**: `intext_quotes::PAGE_TEXT_EXPR` `pub(crate)`
  oldu, kapsam raporu aynı ifadeyi kullanır; `ORDER BY s.rowid` eklendi (`LIMIT 1` artık
  deterministik, tarayıcıyla aynı satır).
- Çıktı yüzeyleri: `rows()` "Okunabilir metni olmayan belge", yeni uyarı ("… bu belgeler
  incelemeye alınamaz"), tek satır özet "· N metinsiz belge"; sayım notu artık **gönderim
  kuralını** yazar (eskiden "koşu sayısıyla birlikte artar" diyordu). Arayüz: Çıktılar
  tablosu satırı + AI ekranı kapsam satırı (`noText`); 2 sözlük anahtarı.

**Ham koşu (ölçüldü).** Yeni testler düzeltme öncesi sayımla **3 kırmızı** (`pages` 5≠3 ·
`unparsed_documents` 0≠1 · kapsam↔gönderim paritesi 2≠1); düzeltmeyle **5/5 yeşil**
(`coverage_report_counts_the_corpus_an_ai_run_would_see`,
`…_a_document_whose_current_version_is_unparsed`, `…_documents_without_usable_text`,
`…_treats_a_whitespace_only_segment_as_no_text`, `coverage_report_agrees_with_the_transfer_scope`).

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-coverage-gates.py`, her mutasyon tek
başına, `try/finally` + `os.utime` damgası + sha256 birebir).** **9/9 KIRMIZI, taban
yeşilken** (taban: çekirdek 15, dışa aktarma 9, arayüz 14, AI ekranı 114 test) —
M1 sayfa/segment bütün koşulardan sayılır (3 test) · M2 `unparsed_documents` eski kurala
döner (2) · M3 metinsiz belge "segment satırı yok" ile tanımlanır (1) · M4 metinsiz belge
uyarısı yazılmaz (2) · M5 raporda satır çizilmez (1) · M6 tek satır özet yazmaz (1) ·
U1 arayüz satırı çizilmez (2) · U2 özete `noText` geçirilmez (3) · U3 AI ekranı `noText`
geçirmez (**114 test düştü**, i18n eksik-parametre). **Dürüst (kendi hatam, iki ölçüm
hatası):** (1) betiğin ilk sürümünde vitest çökme biçimi ("Tests 114 failed (114)")
tanınmadı → U3 "bilinmiyor" çıktı ve 8/9 sayıldı; U3 **ayrı betikle yeniden ölçüldü**
(114/114 düştü, `Error: i18n: "ai.coverageLine" için eksik parametre: noText`, geri yazım
sha `15851d62…` birebir). (2) İlk koşuda `cargo fmt` sonrası üç kapı metni birebir
eşleşmedi ("EŞLEŞMEDİ") → metinler son biçime göre yazıldı, **tüm kapılar son hâl üzerinde**
yeniden koşuldu; yalnız bu koşu kanıt sayıldı, her mutasyonun geri yazımı birebir.

**Testler (güncel kodla).** `ai` kapsam testleri 4 → 9 (+5), `export_coverage` 9 → 10 (+1)
→ `dim-storage --lib` **587 → 593** (298,11 s); tam vitest **66 dosya / 1430** yerel;
`tsc --noEmit` temiz; iki manifest `cargo fmt --check` temiz;
`clippy -p dim-storage --all-targets -D warnings` temiz.

**Dürüst sınırlar.**

- "Metin var" ölçütü **ilk segmente** bakar; üretim yolu (`store_pages`) sayfa başına tek
  segment yazar, çok segmentli sayfada ikinci segment taranmaz — alıntı denetimiyle aynı
  sınır (tek tanım bilinçli).
- `documents` silinmemiş belgeleri sayar; `versions`/`parse_runs`/`failed_parse_runs`
  tarihçedir ve silinmiş belgelerin kayıtlarını da içerir.
- `complete` geçmişteki **başarısız** koşu yüzünden "kısmi" kalır (fail-closed: kanıt
  yokken temiz demez); başarısız koşunun hangi sürüme ait olduğu ayrıca raporlanmaz.
- Sayfa düzeyi ayrım yok (hibrit sayfa): tek sayfada hem metin hem OCR'sız görüntü varsa
  sayfa "metinli" sayılır.
- Tauri komutları (`case_coverage`, dört dışa aktarma, devir paketi) **komut olarak
  koşulmadı** (`State<AppState>` altyapısı yok): kablolama `cargo check`/`clippy`,
  davranış storage düzeyinde gerçek SQLite (`open_in_memory`).
- jsdom tarayıcı değil → gerçek Tauri penceresi ve **görsel QA insan bekliyor**.

**Commit / CI (ölçüldü).** Kod `abc4ccc` (12 dosya, **+314/−34**) → run `43436689213`
**10/10 YEŞİL** (ilk koşu): `dim-storage` **593** iki OS'ta birebir (macos-15 286,86 s /
ubuntu-latest 705,05 s), `arayüz` 66 dosya / **1422** test (yerel 1430 farkı ağaçtaki
başka ajanın commit'siz testleri), paket `index-WZ8frWYD.js` 692,02 kB / gzip 184,65 kB,
`masaüstü kabuk` 51/0/5 + yoksayılan 5 passed; diğer altı iş yeşil.
**i18n hunk ayrımı:** 5 hunk'tan yalnız benim `noText` hunk'larım `git apply --cached
--recount` ile sahneye alındı (`git show :…i18n.ts | grep -c parseStatus` → **0**).

## F11-T04b — gerçek model yanıtı doğru aday türü ve somut şemayla kaydedilir (2026-11-21)

**Ölçülen kusur.** AI ekranındaki «Çalıştır» düğmesinin arkasındaki gerçek yol
(`ai_run_execute`) model yanıtını `record_ai_candidate(&run.id, &run.task_type, …)` ile
kaydediyordu. `run.task_type` bir **sağlayıcı görevi**dir (`summarize` / `extract_events` /
`extract_claims`), aday türü değil; doğrulayıcı bu adları tanımadığı için **her gerçek model
adayı `invalid` (`unknown_candidate_type`) doğuyordu** — çıktı üretilse de incelenemiyor,
terfi edilemiyordu (kusur `git log -S` ile F09-T09c öncesine kadar izlendi). Aynı istem
yalnız `Şema: candidate.v1` etiketi taşıyordu: model `schemaVersion`ı, zorunlu alanları ve
`quotes[].documentVersionId` sözleşmesini bilmiyordu; segment satırlarında belge sürümü
kimliği de yoktu — alıntı hiçbir zaman bağlanamazdı.

**Yazılan.**

- `task_candidate_kind` (tek kaynak): `summarize→summary_candidate`,
  `extract_events→event_candidate`, `extract_claims→claim_candidate`; `task_type` zaten bir
  aday türü adı taşıyorsa (`event`, `claim_candidate`, …) **aynen** kullanılır, tanınmayan →
  `None` (**tahmin yok** — çağıran bilinmeyen türü olduğu gibi kaydeder).
- `candidate_output_instructions`: doğrulayıcının **aynı** `field_specs` tablosundan üretilen
  somut şema — her alan için ad, zorunluluk, metin uzunluğu / enum değerleri / liste sınırı /
  `YYYY-AA-GG` gerçek gün — ve alıntı sözleşmesi (`quotes[]` en çok 20; her öğe
  `{"documentVersionId": "…", "quote": "…"}`, `documentVersionId` segment satırındaki `|`
  sonrası değer, `quote` segment metninden **birebir**, 8–1000 karakter; özet adayı en az bir
  alıntı taşır). Metin doğrulayıcının tablosundan doğduğu için iki kopya ayrışamaz.
- `AiRequest.instructions` ve `ContextSegment.document_version_id`; sağlayıcı istemi tek
  `AiRequest::user_prompt()` işlevinden kurulur (`Görev / Şema / Kurallar / Segmentler`),
  segment satırı `[segmentId | documentVersionId] metin`. **İki sağlayıcı da** (API + yerel)
  aynı işlevi kullanır.
- `Database::record_ai_run_candidate`: aday türünü çalıştırmanın görev türünden türetir ve
  **aynı** `record_ai_candidate` doğrulamasını koşar; `ai_run_execute` artık bunu çağırıyor.
  Tanınmayan görev türünde yanıt **kanıtıyla** (`validation_errors`) bilinmeyen türde
  kaydedilir; depo katmanı tür tahmin etmez.

**Testler (güncel kodla).** `ai_candidate` +6: eşleme tablosu (üç görev + eski adlar +
tanınmayan), şema üretimi (olay/özet kuralları), **eski hatanın kanıtı** (`summarize` →
`invalid` + `unknown_candidate_type`), gerçek yol → `summary_candidate` + `valid` +
`verified` + terfi + anchor, `extract_events` → `event_candidate` + `valid`, tanınmayan tür
aynen + `invalid`. `dim-providers` +1: istem somut şemayı ve `[seg-1 | dv-1]` kaynak
kimliğini taşır. Yerel: `dim-storage --lib` **735/735 yeşil, 590,90 s** (log
`scratch/dim-libtest-t04b.log`); `dim-providers` **28/28**; iki manifest `cargo fmt`;
`clippy -p dim-storage -p dim-providers --all-targets -D warnings`; masaüstü
`check --all-targets` temiz.

**Kapı kanıtı (`scratch/dim-t04b-gates.py`, log `dim-t04b-gates.log`; her mutasyon tek
başına, kaynak sha256 **birebir** geri yazıldı).** **4/4 KIRMIZI**, taban yeşilken —
M1 eşleme bozuldu (`summarize→claim_candidate`) · M2 eski hata geri geldi
(`record_ai_run_candidate` doğrudan `run.task_type` geçirir) · M3 «özet en az bir alıntı»
kuralı istemden düşürüldü · M4 istemdeki kaynak kimliği bozuldu (`dim-providers` testi
düştü). **Dürüst:** M2'nin ilk mutant deseni kaynakta **eşleşmedi** («EŞLEŞMEDİ», 0 test
düştü) → desen düzeltildi ve M2 yeniden ölçüldü; ilk denemenin hükmü kanıt sayılmadı.

**Dürüst sınırlar.**

- **Gerçek API çağrısı yapılmadı** (anahtar yok): modelin istemdeki şemaya **uyacağı**
  ölçülmedi. Uymazsa aday yine kanıtıyla `invalid` kalır — bu dilim adayın
  **doğrulanabilir** doğmasını ve incelenebilmesini sağlar; modelin uyumunu garanti etmez.
- `document_version_id` **opak** kimliktir; modele içerik değil, alıntının bağlanacağı
  anahtar verilir. Belge adı/metni istemde ayrıca taşınmaz.
- Görev türü tanınmıyorsa istem kuralları **boş** kalır (tahmin yok) ve aday aynen
  kaydedilir; ürün bugünkü üç görev türünü kullanır.
- İstem metni doğrulayıcının alan tablosundan üretilir; tabloda olmayan bir kural (ör.
  alanlar arası çapraz tutarlılık) isteme de yazılmaz.
- Yerel ölçümler hata ayıklama derlemesinde; CI iki OS'ta koşar. Arayüz değişmedi — gerçek
  Tauri penceresi + görsel QA **insan bekliyor**.

**Commit / CI (ölçüldü).** Kod `4000166` (7 dosya, **+365/−28**) → run `44939941625`
**10/10 YEŞİL**: `dim-storage` **735** iki OS'ta (macos-15 **554,58 s** / ubuntu-latest
**504,78 s**), `arayüz` 66 dosya / **1449** test, paket `index-BP_FWRRw.js` **699,98 kB** /
gzip **186,63 kB**, `masaüstü kabuk` 52 + yoksayılan 5 passed (ikinci adımda), `windows
güvenli depo`, sözleşmeler, belge worker'ı, SBOM, sürüm paketi manifesti ve kaynak arşivi
yeşil. İlk `git push` bu turda yapıldı (`272bd48..4000166`); önceki koşuda commit atılmış
ama **push edilmemişti** — CI kanıtı bu turun ölçümüdür.

## F11-T09 — çelişki adayı **iki kaynağın** alıntısıyla sunulur (2026-11-22)

**Ölçülen kusur (düzeltme öncesi).** İnceleme kuyruğu `contradiction_candidate` türünü
tanıyor, `create_review_item` bu tür için **tam iki** anchor istiyor ve `Review.tsx` elle
ekleme yolunda iki ayrı anchor'ı zorunlu tutuyordu. Ama AI tarafında **çelişki adayı türü
yoktu**: `CandidateKind` altı tür tanıyordu, amaç listesinde «çelişki» yoktu. Dahası
`promote_candidate` kuyruğa **doğrudan INSERT** ettiği için iki-anchor kuralı bu yolda
atlanıyordu; iki taraf **aynı alıntıyı** gösterebiliyor, tarafların tarihleri **farklı
rollerde** olsa bile (belge tarihi ↔ tebliğ tarihi) bu ayırt edilemiyordu — ekran ve kuyruk
için ikisi de «çelişki» görünürdü.

**Yazılan.**

- **Çekirdek tür:** `CandidateKind::Contradiction` (`contradiction` | `contradiction_candidate`);
  `field_specs`: `text` 1–2000 zorunlu, `sides` ayrı doğrulanır. `parse_sides`: **tam iki**
  nesne (`sides_not_two`), her nesne `quoteIndex` (kabul edilmiş alıntı; **aynı indeks iki
  taraf olamaz** → `side_quote_duplicate`; kabul edilmemiş indeks → `side_quote_unknown`),
  isteğe bağlı `dateKind` kapalı küme (`document|event|notification|hearing|unknown`) ve
  isteğe bağlı `dateText`; tanımsız alan `unknown_field` uyarısı.
- **`contradiction_basis(payload, quotes)`** — **okuma anında** hesaplanan hüküm (kalıcı
  sütun değil): `presentable` (iki taraf var, ikisi de `exact`), `role_difference` (iki taraf
  da rol bildirdi ve **roller farklı** → otomatik çelişki değil), `sources_unverified` (en az
  bir taraf `exact` değil; neden `side_missing`/`side_not_exact`), `unlinked` (`sides` yok ya
  da iki taraf değil — eski kayıt). `distinct_documents` ayrıca sayılır.
- **Doğrulama hükmü:** `unlinked`/`sources_unverified`/`role_difference` uyarı üretir ve
  `validation_state`i **`partial`a** çeker — iki kaynak ön koşulu tutmayan aday «geçerli»
  görünmez.
- **Kapı:** `promote_candidate` çelişki adayını `presentable` değilse `E_AI_CONTRADICTION`
  ile reddeder; terfi anında kuyruk türü **`contradiction_candidate`** olarak normalize edilir
  (çelişki değerlendirmesi bu adı arar) ve bağlanan **anchor sayısı tam iki** olmalıdır.
  Kapı çekirdekte: arayüz düğmeyi kapatsa da doğrudan IPC çağıran geçemez.
- **Görev/amaç:** `extract_contradictions` ↔ `contradiction_candidate`;
  `purpose_task_type("celiski") = "extract_contradictions"`. `candidate_output_instructions`
  modele iki taraf kuralını ve tarih rolü uyarısını yazar («farklı tarih rollerindeki iki tarih
  çelişki değildir»).
- **Okuma yolu:** Tauri aday yüküne `contradictionBasis` (`#[serde(default)]`); `api.ts`
  `ContradictionBasis` / `ContradictionSideReport` tipleri.
- **Arayüz:** amaç seçicide «Çelişki çıkarımı»; aday kartında iki taraf **yan yana** (A/B,
  alıntı durumu rozeti, sayfa, etiket, alıntı metni, tarih rolü + metni), `role_difference`
  uyarısı ve engel gerekçeleri. `presentable` rozeti «çelişki var» demek **değildir**: iki
  kaynağın birebir doğrulandığını söyler, hüküm avukatındır.

**Testler (güncel kodla).** `ai_candidate` +2:
`contradiction_candidate_schema_requires_two_distinct_sides` (şema saldırıları: iki taraftan
az/çok, aynı alıntı iki taraf, bilinmeyen alıntı indeksi, kapalı küme dışı `dateKind`, tanımsız
alan) ve `contradiction_needs_two_verified_sources_and_ignores_date_roles` (iki doğrulanmış
kaynak → `presentable`; rol farkı → `role_difference` + `validation_state` `partial`; biri
doğrulanmamış → `sources_unverified`; `sides`sız eski satır → `unlinked`). Arayüz +8
(`candidate` +5: yedi tür listesi, durum etiketleri, tarih rolü, taraf durumu, terfi kapısı;
`Ai` +3: iki taraf + konum + rol gösterimi, rol farkında düğme kapalı + gerekçe, doğrulanmamış/
iki-tarafsız adayın gönderilmemesi). Yerel: `dim-storage --lib` **744/744 yeşil, 600,08 s**
(log `scratch/dim-t09/libtest_t09.log`); arayüz 66 dosya / **1466** test yeşil (ağaçtaki başka
ajanın 3 testi dahil), `tsc --noEmit` temiz; iki manifest `cargo fmt`; `clippy -p dim-storage
--all-targets -D warnings`; masaüstü `clippy --all-targets -D warnings` temiz.

**Kapı kanıtı (`scratch/dim-t09/t09_gates.py`, log `t09_gates_run2.log`).** Her mutasyon tek
başına uygulandı, hedefli test koşuldu, kaynak sha256 **birebir** geri yazıldı
(`ai_candidate.rs 228e5a6af9752860…`, `ai.rs f5429371b5d63278…`, `candidate.ts
dc23b14c8c9417be…`, `Ai.tsx 3691501c16fface3…`). **11/11 KIRMIZI · YEŞİL 0 · ÖLÇÜLEMEDİ 0**
(82,2 s): tam iki taraf, aynı alıntının iki taraf olması, tarih rolü farkı, `exact` doğrulama,
uyarının `partial`a çekmesi, terfi kapısı, kuyruk türü normalizasyonu (çekirdek); rol farkı
engeli, `presentable` tonu, tarih rolü etiketi, çelişki başlığı (arayüz). **Dürüst (önceki
koşu):** ilk kapı koşusu araç tur sınırında kesildi (M1–M5 okunmuş, M6–U4 okunamamıştı); o
koşunun eksik hükümleri kanıt sayılmadı, kapı bu turda **baştan** koşuldu.

**Dürüst sınırlar.**

- **Gerçek API çağrısı yapılmadı** (anahtar yok): modelin `sides` şemasına uyacağı ve çelişki
  önerisinin niteliği ölçülmedi; uymazsa aday kanıtıyla `invalid`/`partial` kalır.
- «İki doğrulanmış kaynak» ölçüsü **alıntı eşleşmesidir** (iki pasaj da metinde tek konumda
  birebir bulundu). Tarafların **gerçekten çeliştiği** hukuken doğrulanmaz; `presentable`
  «çelişki var» demek değildir — yorum avukatındır.
- İki taraf **aynı belgeden** olabilir (iki ayrı pasaj); `distinct_documents` yalnız bilgi
  olarak sayılır, kapı değildir (aynı belgedeki iki beyan da çelişebilir).
- `dateKind` modelin **beyanıdır**; metinden doğrulanmaz. Yanlış rol yazılırsa rol farkı yanlış
  hesaplanır (kapı bunu bilemez).
- `contradiction_basis` **kalıcı damga değildir**; her okumada yeniden hesaplanır. Yeniden
  doğrulama alıntı raporunu değiştirirse hüküm de değişir; `sides` taşımayan eski satırlar
  `unlinked` görünür (uydurma damga yok).
- Kapı çekirdekte; yine de `ai_candidates` satırını doğrudan SQL ile yazan saldırgan
  `quote_report`/`validation_state`i tahrif edebilir (bu tabloda tetikleyici yok — bilinçli
  sınır).
- Arayüz jsdom'da ölçüldü; gerçek Tauri penceresi + görsel QA **insan bekliyor**.

**Commit / CI (ölçüldü).** Kod `d061fc3` (10 dosya, **+1436/−12**) → run `44977775851`
**10/10 YEŞİL**: `dim-storage` **744** iki OS'ta (macos-15 **497,63 s** / ubuntu-latest
**1.055,59 s**), `arayüz` 66 dosya / **1463** test (yerel 1466 farkı ağaçtaki başka ajanın
commit'siz 3 testi), paket `index-ThMfSHF8.js` **714,35 kB** / gzip **189,91 kB**,
`masaüstü kabuk` (clippy + test), `windows güvenli depo`, SBOM, sözleşmeler, belge worker'ı,
sürüm paketi manifesti ve kaynak arşivi yeşil.

## F11-T08 — soru-cevapta incelenen kapsam ve «sınırlı yanıt» işareti (2026-11-22)

**Ölçülen kusur (düzeltme öncesi).** Üründe **soru amacı yoktu**: `purpose_task_type`
yalnız `ozet`/`olay_cikarimi`/`iddia_cikarimi` tanıyordu, `ai_run_execute` soru metni
almıyordu. `answer_candidate` şeması (`text` + isteğe bağlı `question`) vardı ama yanıtın
**hangi belgeleri görerek** verildiği ve **dayanağı** ölçülmüyordu: `record_ai_candidate`
alıntı bulunamazsa `partial/failed` diyor, "bu yanıt güvenilir mi" sorusunu hiçbir katman
yanıtlamıyordu; avukat yanıtı inceleme kuyruğuna gönderdiğinde satır "kaynaklı bulgu" gibi
okunuyordu.

**Yazılan.**

- **Amaç ve soru metni:** `purpose_task_type("soru") = "answer"`; `AI_ANSWER_TASK`,
  `AI_QUESTION_MIN/MAX_CHARS` (3–500), `normalize_ai_question(task_type, question)` —
  `answer` görevi soru **ister** (kırpılmış; kontrol karakteri yasak, satır sonu/sekme
  serbest), başka görev soru **kabul etmez** (`E_AI_QUESTION`). `ai_run_execute` yeni
  `question` parametresi alır; görev türü yine **onayın amacından** türetilir. Soru,
  çalıştırmanın `ai_messages` **user** satırı olarak yazılır (yazılamazsa çalıştırma
  `failed` kapanır ve hiçbir bayt çıkmaz).
- **İstem:** `AiRequest.question` + tek `user_prompt()`: `Görev / Soru / Şema / Kurallar /
  Segmentler`; soru satırı yalnız alan doluyken yazılır (iki sağlayıcı aynı işlevi kullanır).
  `answer` çıktı kuralları da somutlaştı: «yanıtı destekleyen pasajı `quotes` içinde birebir
  ver; dayanak yoksa text alanında bunu yaz ve `quotes` boş bırak».
- **`Database::answer_basis(run_id) → AnswerBasis`** — **okuma anında** hesaplanır, sütuna
  yazılmaz:
  - **kapsam** `transfer_segments`ten (fiilen gönderilen): belge sürümü başına segment +
    **bayt**, toplam segment/bayt;
  - **soru** koşunun `user` mesajından (yoksa `null`, uydurulmaz);
  - **dayanak** yanıt adaylarının alıntı raporundan: `quotesTotal/quotesFound/
    quotesVerified/quotesNotFound` — `exact|normalized` doğrulanmış, `ambiguous|page_mismatch`
    metinde **bulunmuş**, `not_found|unverifiable` ve bilinmeyen durum **bulunamadı**
    (fail-closed);
  - `state = sufficient` yalnız kapsam boş değil **ve** en az bir alıntı metinde bulunmuşsa;
    değilse `limited` + kararlı neden kodları: `no_scope`, `no_answer`, `answer_invalid`,
    `no_quotes`, `quotes_not_found`.
- **Kapı:** `promote_candidate`, `limited` yanıt adayını `E_AI_ANSWER_LIMITED` ile reddeder
  (neden kodları mesajda). Kapı **çekirdekte**: doğrudan IPC çağıran da geçemez; arayüz
  yalnız düğmeyi kapatır ve nedenini yazar.
- **Okuma yolu:** Tauri `ai_answer_basis`; `api.ts` `AiAnswerBasisRow` +
  `aiRunExecute(..., question)`.
- **Arayüz:** amaç listesinde «Soruyu yanıtla» + soru alanı (geçersiz soruda onay/çalıştır
  kapalı; ölçü çekirdekle aynı: kırpılmış kod noktası 3–500). Yanıt kartında soru satırı,
  `İncelenen kapsam: N belge · M segment · X` (kapsam boşsa ayrı cümle), dayanak rozeti
  (`dayanak doğrulandı` / `sınırlı yanıt — dayanak doğrulanmadı`), sınır nedenleri ve alıntı
  sayıları. Kullanıcının sorusu «model yanıtı» listesinde tekrarlanmaz. Dosya değişiminde
  soru ve ölçüm taşınmaz (INV-05); geç okuma nesil sayacıyla kesilir; düşen ölçüm «dayanak
  yok» demez, hata kendi satırında yazılır.

**Testler (güncel kodla).** `ai` +1 (`soru`→`answer`; soru zorunluluğu, 3–500 sınırı, kontrol
karakteri, başka görevde ret), `ai_candidate` +1 (altı senaryo: kapsam yok, yanıt yok, bulunan
alıntı + terfi, alıntısız + `E_AI_ANSWER_LIMITED`, bulunamayan alıntı, şeması geçersiz yanıt;
kapsam baytı **gönderilen** metinden), `dim-providers` +1 (soru satırı yalnız doluyken),
arayüz +4 (Ai ekranı: soru alanı/kapılar, kapsam + dayanak gösterimi, sınırlı yanıtta terfi
kapalı, düşen ölçüm). Yerel: `dim-storage --lib` **737/737 yeşil, 681,89 s** (log
`scratch/dim-libtest-t08-run2.log`); `dim-providers` **29 passed + 1 yoksayılan**; arayüz 66 dosya / **1453**
test (yerelde ağaçtaki başka ajanın 3 testiyle 1456); `tsc --noEmit`; iki manifest
`cargo fmt`; `clippy -p dim-storage -p dim-providers --all-targets -D warnings`; masaüstü
`clippy --all-targets -D warnings` temiz.

**Kapı kanıtı (`scratch/dim-t08-gates.py`, `dim-t08-ui-gates.py`; her mutasyon tek başına,
kaynak sha256 birebir geri yazıldı: `ai.rs 793fd7c3…`, `ai_candidate.rs 4aff90ca…`,
`providers/lib.rs 0db3ff58…`, `Ai.tsx 9399db68…`).** Taban yeşil (storage 6 test, providers 1,
arayüz 124); **13/13 çekirdek KIRMIZI** — amaç eşlemesi, soru kapısı, uzunluk/kontrol
karakteri, terfi kapıları (2), kapsam sorgusu, `sufficient` kararı, `no_answer`/`no_quotes`
nedenleri, kullanılabilirlik, soru rolü, istem satırı — ve **5/5 arayüz KIRMIZI** — soru
kapıları, sorunun isteğe girmesi, soru alanı, kapsam kartı, sınırlı yanıtta terfi düğmesi.
**Dürüst (kapı aracı):** ilk arayüz koşusu özet ayrıştırıcısı «Failed Tests N» satırını
yakaladığı için 5/5 **ÖLÇÜLEMEDİ** verdi → ayrıştırma düzeltildi, 5/5 yeniden ölçüldü. Ayrıca
ilk çekirdek kapısı, geri yazımda `copy2`'nin **eski mtime**'ını bırakıyordu; cargo son mutant
derlemesini «güncel» sanıp yeniden kullandı ve ardından koşulan tam pakette
`answer_basis…` testi **yanlış kırmızı** verdi. Kaynak sha256 doğruydu (ürün kusuru değil);
kapı betiği **şimdi+2** damgasıyla düzeltildi, taban + 13/13 + 5/5 yeniden ölçüldü, tam paket
temiz kaynaktan yeniden koşuldu. İlk (bozuk araçlı) koşuların hükümleri kanıt sayılmadı.

**Dürüst sınırlar.**

- **Gerçek API çağrısı yapılmadı** (anahtar yok): modelin soru + şemaya uyacağı ve yanıt
  kalitesi ölçülmedi; uymazsa aday kanıtıyla `invalid`/`partial` kalır.
- «Dayanak» ölçüsü **alıntı eşleşmesidir** (pasaj belge metninde bulundu mu). Yanıtın hukuken
  doğru olduğu, alıntının yorumu desteklediği veya yanıtın soruya tam yanıt verdiği
  doğrulanmaz — yorum ayrı incelenir (F11-T04 kuralı).
- Kapsam, **gönderilen** segmentlerdir; modelin bu metni gerçekten "okuduğu" ayrıca
  kanıtlanamaz. `scope_bytes` maskeli gönderimde **çıkan** baytı sayar.
- **Sınırlı yanıt terfi edilemez**: «gönderilen belgelerde dayanak yok» sonucunu inceleme
  kuyruğuna taşımanın yolu bugün yok; avukat bunu not olarak elle yazar. Bu bilinçli bir
  ürün sınırıdır (INV-02: kaynaksız iddia kaynaklı bulgu gibi görünmez).
- `limited`/`sufficient` **kalıcı damga değildir**; her okumada yeniden hesaplanır — belge
  yeniden ayrıştırılırsa ölçüm değişebilir (bayat «dayanak doğrulandı» yazılmaz).
- `question` yalnız yeni çalıştırmalarda yazılır; eski koşularda `null` («soru kaydı yok»
  denir, uydurulmaz). Soru metni `ai_messages`ta **görünür içerik** olarak durur; denetim
  kaydına (audit) yazılmaz.
- Arayüz jsdom'da ölçüldü; gerçek Tauri penceresi + görsel QA **insan bekliyor**.

**Commit / CI (ölçüldü).** Kod `46d986d` (14 dosya, **+1086/−22**) → run `44954882906`
**10/10 YEŞİL**: `dim-storage` **737** iki OS'ta (macos-15 **469,54 s** / ubuntu-latest
**911,03 s**), `arayüz` 66 dosya / **1453** test (yerel 1456 farkı ağaçtaki başka ajanın
commit'siz 3 testi), paket `index-D1rV_NXL.js` **704,49 kB** / gzip **187,85 kB**,
`masaüstü kabuk` 52 + yoksayılan 5 passed (ikinci adımda 5), `windows güvenli depo` 30,
sözleşmeler, belge worker'ı, SBOM, sürüm paketi manifesti ve kaynak arşivi yeşil. Bu
commit önceki koşuda atılmış ama **push edilmemişti**; push ve CI ölçümü bu turun işidir.

## F11-T07 — özet cümlesi dayanak bağı ve kaynaksız yorum ayrımı (2026-11-22)

**Ölçülen kusur (düzeltme öncesi).** `summary` adayı `text` + en az bir alıntı
istiyordu (F11-T05), ama **hangi cümlenin hangi alıntıya dayandığı** hiçbir katmanda
yoktu: `quote_report` yalnız alıntı başına «belge metninde bulundu mu» diyor, `text`
tek parça saklanıyordu. Alıntısı doğrulanmış bir özet, alıntıyla desteklenmeyen
**yorum cümleleri** taşısa bile bütünüyle «alıntılar birebir doğrulandı» rozetiyle
görünüyordu; şartnamenin kabul testi («alıntı doğru olsa bile yorum ayrı incelenir»)
karşılanmıyordu.

**Yazılan.**

- **Bağ modelin bildirimidir, çekirdeğin çıkarımı değil:** `quotes[].sentences` —
  alıntının desteklediği cümlelerin **metinden birebir kopyaları** (alıntı başına en
  çok 20, cümle ≤1.000 karakter). Çekirdek cümleye anlam atfetmez; model bildirmezse
  bağ **kurulmaz**.
- **Deterministik cümle bölme** (`split_sentences`): `. ! ? …` + kapanış tırnağı /
  parantezi; kısaltma listesi (`Av.`, `Dr.`, `TBK`, `m.`…), tek harf (`T.C.`, `A.`),
  iç noktalı baş harf grubu ve salt sayı (`5.`, `12.`) bölmez; sonraki harf küçükse
  bölünmez; `...` tek sonlandırıcıdır.
- **Şema kapıları:** özet alıntısı cümle bildirmezse `sentence_links_missing`
  (uyarı → `partial`); bildirilen cümle metinde birebir yoksa `sentence_not_found`
  (uyarı; bağ kurulmaz); özet dışı adayda `sentences` → `sentences_unsupported`
  (uyarı; yok sayılır). `candidate_output_instructions` özet için cümle bağını ve
  kaynaksız yorum kuralını modele yazar.
- **`SentenceAttribution`** — yükten ve alıntı raporundan **her okumada** yeniden
  hesaplanır (sütun/damga yok): cümle başına `quoteIndexes` + `backed`
  (birebir/normalize), `weak` (belirsiz/başka sayfa: varlık doğrulandı, konum tek
  değil), `unsourced` (gösteren alıntıların hiçbiri bulunamadı ya da hiçbir alıntı
  göstermedi = **kaynaksız yorum**); toplam `backed/weak/unsourced/linkedQuotes/
  unlinkedQuotes/unmatched`, durum `linked`/`partial`/`unlinked`. Hiçbir alıntı
  cümle bildirmediyse `unlinked` + **boş** cümle listesi — «kaynaksız» damgası
  uydurulmaz.
- **İki okuma, tek hesap:** `validate_ai_candidate` raporu `sentenceAttribution`
  taşır; kayıtlı aday satırı (`AiCandidateRow`) aynı işlevi `map_candidate`te çağırır
  — doğrulama ekranı ile aday listesi ayrışamaz.
- **Arayüz:** doğrulama kartında «Cümle dayanakları» — durum rozeti + sayım;
  **kaynaksız cümleler ayrı blokta** (kırmızı sol çizgi, «Kaynaksız yorum»),
  dayanaklı/zayıf cümleler alıntı numarasıyla; aday tablosunda özet satırında sayım
  + açılır ayrıntı. Bağ yoksa «Cümle bağı yok» ve ayrımın yapılamadığı yazılır
  (alıntılar doğrulanmış olsa da).

**Testler (güncel kodla).** `ai_candidate` +5 (bölme: kısaltma / baş harf / sıra
sayısı / `...` / küçük harf; ayrım: kaynaksız, zayıf, `unlinked`, metinde olmayan
bildirim, şema uyarıları) + DB satır testi (`record` + `validate` + `list` aynı bağı
taşır; özet dışı aday `null`) → `dim-storage --lib` **737 → 742**; yerel
**742/742 yeşil, 682,62 s** (log `scratch/t07_libtest.log`). Arayüz +3 (Ai:
kaynaksız blok/sayım, `unlinked` uyarısı, kayıtlı aday satırı) → 66 dosya /
**1459** test (yerel; ağaçtaki başka ajanın commit'siz 3 testi dahil),
`tsc --noEmit`; iki manifest `cargo fmt`; `clippy -p dim-storage --all-targets
-D warnings`; masaüstü `cargo check --all-targets` (2 dk 02 s) temiz.

**Kapı kanıtı (`scratch/t07_gates.py`, log `t07_gates_run2.log`; her mutasyon tek
başına, kaynak sha256 birebir geri yazıldı: `ai_candidate.rs cd4f2b5d…`,
`ai.rs 6fba82fe…`, `Ai.tsx 7a9da554…`, `candidate.ts 0e653071…`).** Taban yeşil
(çekirdek 32 test, arayüz 3); **15/15 çekirdek KIRMIZI** — kısaltma listesi, tek
harf, sayı sırası, küçük harf kuralı, üç nokta, belirsiz→zayıf, `linked` eşiği,
`unmatched` sayımı, `unlinked` dalı, bağ-zorunlu uyarısı, cümle-bulunamadı uyarısı,
özet-dışı uyarısı, rapor bağı, satır bağı (`ai.rs`), istem cümlesi — ve **5/5
arayüz KIRMIZI** — `unlinked` kapısı, kaynaksız blok, cümle etiketi, aday satırı,
sayım. **Dürüst (ilk koşu 16/20):** dört mutant YEŞİL kaldı; ikisi yedekli
kurallardı (tek harf kuralı `m.` için kısaltma listesiyle, salt-sayı kuralı tek
hanede tek-harf kuralıyla örtüşüyordu), biri koşu filtresine takılmayan istem
testiydi (`output_instructions…` adında `summary` yok), biri de sayımı **aynı
yardımcıyla** bekleyen arayüz testiydi (mutasyon iki tarafı birlikte kaydırdı).
Testlere baş harf (`A. Yılmaz`), çok haneli sıra (`12. Asliye`) ve bağımsız sayım
beklentisi eklendi; çekirdek filtresi `ai_candidate::tests` yapıldı; **20/20**
yeniden ölçüldü. `Ai.tsx`'e sonradan eklenen `compact` başlık düzenlemesinden sonra
arayüz kapısı **5/5** ile yeniden koşuldu (`t07_gates_ui_final.log`). İlk koşunun
hükümleri kanıt sayılmadı.

**Dürüst sınırlar.**

- **Bağ modelin bildirimidir:** cümlenin gerçekten o alıntıyla desteklendiği
  **doğrulanmaz**; yalnız alıntının belge metninde bulunduğu (ve konumu) ölçülür.
  Model yanlış cümleyi gösterirse rapor yanlış bağ gösterir — hüküm avukatındır.
- Cümle bölme **dilbilimsel değil**: yıl/sayı ile biten cümle (`Ödeme 2024.`) ve
  küçük harfle başlayan cümle bölünmez; bildirim bölünen cümleyle eşleşmezse bağ
  kurulmaz (`unmatched` yazılır), uydurulmaz. Kısaltma listesi kapalı kümedir;
  listede olmayan kısaltma cümleyi bölebilir (ör. `Yrd. Doç.` → `Yrd.` böler).
- Model `sentences` bildirmezse özet `partial` doğar ve `unlinked` gösterilir; eski
  kayıtlar **silinmez/düzeltilmez**, yeniden doğrulanınca aynı kurala tabidir.
- **Kaynaksız cümle terfi engeli değildir**: şartname «görünür ayır» der, engelleme
  istemez. İnceleme kuyruğuna giden aday yine avukat onayı ister (INV-02).
- `quoteIndexes` yük sırasıdır; geçersiz (çok kısa) alıntının cümlesi bağ sayılmaz,
  o cümlenin durumu `unsourced` kalır.
- Arayüz jsdom'da ölçüldü; gerçek Tauri penceresi + görsel QA **insan bekliyor**.

**Commit / CI (ölçüldü).** Kod `c609d07` (8 dosya, **+1131/−12**). İlk koşu
`44965619174` **9/10** (o sırada `arayüz` KIRMIZI; rust işleri sürerken düzeltme
push edilince koşu **cancelled** oldu): paylaşılan `i18n.ts` **bütün dosya**
sahnelendiği için başka ajanın commit'siz `cases.parseStatusSucceeded` /
`cases.parseStatusPartial` / `cases.parseStatusFailed` anahtarları commit'e
girmişti; tüketicileri (`format.ts`/`Cases.tsx`)
commit'siz olduğundan yerelleştirme kapısı haklı olarak düştü (**1 failed |
1455 passed**, 66 dosya). Düzeltme `33691c6` yalnız o üç satırı HEAD'den çıkardı;
anahtarlar çalışma ağacında sahibine **aynen geri bırakıldı** (commit'siz hunk
olarak duruyor). Düzeltme CI'ı run `44966154972` **10/10 YEŞİL**: `dim-storage`
**742** iki OS'ta (macos-15 **476,27 s** / ubuntu-latest **751,43 s**), `arayüz`
66 dosya / **1456** test (yerel 1459 farkı ağaçtaki başka ajanın commit'siz 3
testi), paket `index-CrMnywpN.js` **707,86 kB** / gzip **188,62 kB**,
`masaüstü kabuk` 52 + yoksayılan 5 passed (gerçek-worker adımında 5), `windows
güvenli depo` 30, sözleşmeler, belge worker'ı, SBOM, sürüm paketi manifesti ve
kaynak arşivi yeşil.

## F11-T11 — belge içi talimat saldırısı, sahte kaynak ve şema taşması değerlendirmesi (2026-11-22)

**Ölçülen kusur (düzeltme öncesi).** Şartname üç değerlendirme istiyordu; üründe
üçünün durumu farklıydı:

- **Belge içi talimat saldırısı hiç yoktu**: gönderilen belge metni yalnız maskeleme
  ve kapsam için işleniyordu; «önceki talimatları yoksay», satır başında `system:`
  ya da «tüm dosyaları gönder» gibi ifadeler ne onaydan önce ne çalıştırma kaydında
  görünüyordu.
- **Sahte kaynak** yalnız aday doğrulamasında **dağınık** sayılıyordu: `quote_report`
  her adayın satırında duruyor ama «bu çalıştırmada kaç alıntı gönderilen metinde
  bulunamadı» sorusu tek yerden yanıtlanamıyordu; `not_found` (bulunamadı) ile
  `unverifiable` (doğrulanamadı) ve `ambiguous` (belirsiz) ayrımı ekranda yoktu.
- **Şema taşması ihlalleri** (`too_long`/`too_many_items`) adayı `invalid` yapıyor
  ama hangi sınırın kaç kez aşıldığı raporlanmıyordu.

**Yazılan.**

- **Çekirdek tarama (`instruction_risk.rs`, yeni modül):** 29 iğneli, **5 kararlı
  kodlu** (`ignore_instructions`, `role_marker`, `data_exfiltration`,
  `schema_override`, `credential_request`) deterministik tarama. İğneler **katlanmış**
  metinde aranır: `outbound_mask::fold_char` üzerine **1:1 Türkçe→ASCII** indirme
  (ı→i, ş→s, ğ→g, ü→u, ö→o, ç→c); katlama karakter sayısını değiştirmediği için
  önizleme **özgün metinden** kesilir (ekrana katlanmış metin basılmaz). Satır başı
  rol işaretçileri yalnız **satır başında** eşleşir (`system:`, `assistant:`,
  `### instruction`…). Sonuç segment başına kod başına **tek satır**: tekrar sayısı +
  tek satırlık önizleme (belgenin tamamı taşınmaz). Toplam **2.000.000 karakter**
  bütçesi; aşılırsa `truncated=true` ve kalan segmentler **taranmadı** — sessiz
  «temiz» yok.
- **Onaydan önce görünürlük:** `ConsentPreview.instruction` — gönderim onayı ekranı,
  **gönderilecek** metni onaydan önce tarar. Maske bütün kapsama uygulandıysa
  **maskeli** metin taranır (`basis="sent"`); maske bütçesi yalnız örneğe yettiyse
  tarama **maskesiz** metinde yapılır ve dürüstçe `basis="pre_mask"` işaretlenir
  (gönderimde maske terimleri bu ifadeleri değiştirebilir).
- **Çalıştırma raporu:** `ai_attack_assessment(run_id)` → `AttackAssessment`; üç bölüm
  de **okuma anında** kanıtlarından yeniden sayılır, kalıcı damga yoktur:
  - `instruction` — **gönderilen** `transfer_segments` metninden (v47 kapısı satırları
    dondurur; modelin gördüğü metnin kaydı budur),
  - `fakeSource` — adayların `quote_report`undan: `verified` (`exact|normalized`),
    `notFound`, `ambiguous`, `unverifiable`, `other` **ayrı kovalarda**; **«sahte»
    hükmü yalnız `not_found`** için verilir; alıntısı olup hiçbiri doğrulanmamış aday
    sayısı ayrıca sayılır,
  - `schemaOverflow` — aday `validation_errors`undan `too_long`/`too_many_items`
    ihlalleri: en az bir ihlalli aday sayısı + kod başına adet.
  - **Hiçbiri engel değildir**: rapor avukatın görmesi içindir; gönderimi/terfiyi
    durdurmaz (INV-02 ruhu — hüküm avukatındır).
- **Arayüz (`Ai.tsx`):** onay önizlemesinde talimat satırı — toplam + taranan segment,
  kod etiketleri, önizleme; «sezgisel işarettir; saldırı kanıtı değildir» uyarısı;
  `truncated` ve `pre_mask` ayrıca yazılır. Çalıştırma seçilince «Saldırı
  değerlendirmesi» kartı: talimat sinyalleri (segment + tekrar + alıntı önizlemesi),
  sahte kaynak rozeti (bulunamadı / doğrulandı / doğrulanamadı), şema taşması rozeti
  (ihlal kodları). Nesil sayaçlarıyla bayat yanıt koruması; dosya/çalıştırma
  değişiminde kart temizlenir.
- **Tauri okuma yolu:** `ai_attack_assessment` komutu; `api.ts` `AttackAssessment` /
  `InstructionScan` / `FakeSourceScan` / `SchemaOverflowScan` tipleri; `candidate.ts`
  `instructionCodeLabel` (kararlı kod → Türkçe etiket).

**Testler (güncel kodla).** `dim-storage --lib` **754/754 yeşil, 618,41 s**
(744 → **+10**: `instruction_risk` 6 birim [beş kod, satır başı kuralı, Türkçe
katlama, segment başına tekrar, bütçe/`truncated`, masum hukuki metin] + `ai` 4
[onay öncesi `sent`, maske bütçesi `pre_mask`, üç bölümlü değerlendirme, kanıtsız
çalıştırma]). Arayüz 66 dosya / **1469** test yeşil (node **22.23.1**; node 25'te
jsdom `localStorage` kırılıyor — ortam farkı, kod değil), `tsc --noEmit` temiz. İki
manifest `cargo fmt`; `clippy -p dim-storage --all-targets -D warnings` temiz (bir
`cloned_ref_to_slice_refs` düzeltildi); masaüstü `cargo check --all-targets` temiz
(2 dk 21 s).

**Kapı kanıtı (`scratch/dim-t11-gates.py`, log `t11_gates_run2.log`).** 8/8 mutant
**KIRMIZI** · YEŞİL 0; kaynak sha256 **birebir** geri yazıldı (`instruction_risk.rs
3daaeb215f3050b8…`, `ai.rs e1a9f62179aa698b…`): rol işaretçisi satır başı koşulu,
kod başına sayım, bütçe sınırı, Türkçe noktasız ı katlaması, maske bütçesi `sent`
damgası, bulunamayan alıntının doğrulanmış sayılması, şema taşması sayımı,
doğrulanmamış alıntılı aday sayımı. **Dürüst (önceki koşu):** ilk koşuda M1 **HEDEF
YOK** çıktı (fmt sonrası desen kaydı kaymıştı); desen düzeltildi, kapı **baştan**
koşuldu; ilk koşunun hükümleri kanıt sayılmadı.

**Dürüst sınırlar.**

- Tarama **sezgiseldir**: sabit ifade listesi; eşanlamlı, başka dilde yazılmış ya da
  bölünmüş (`önceki ta-limatları`) saldırıları kaçırabilir.
- Bir eşleşme **saldırı kanıtı değildir** — belge o ifadeyi alıntılıyor olabilir;
  ekran «talimat benzeri ifade» der, «saldırı» demez.
- Yalnız **gönderilen metin** taranır; istem, sağlayıcı şablonu ve modelin gördüğü
  tam bağlam bu modülün dışındadır. Modelin talimata **uyduğu/uymadığı** ölçülmez.
- `pre_mask` taraması maske **öncesidir**: gönderimde maske terimleri ifadeyi
  bozabilir; işaret bunu söyler.
- Sahte kaynak hükmü **alıntı eşleşmesidir**, kaynağın gerçekten uydurma olduğunun
  kanıtı değil; `not_found` «gönderilen metinde bulunamadı» demektir.
- Rapor **kalıcı değildir**: aday silinirse sayı düşer; değerlendirme her okumada
  yeniden hesaplanır.
- Arayüz jsdom'da ölçüldü; gerçek Tauri penceresi + görsel QA **insan bekliyor**.

**Commit / CI (ölçüldü).** Kod `83ef5d8` (10 dosya, **+1260/−6**) → run `45021213972`
**10/10 YEŞİL**: `dim-storage` **754** iki OS'ta (macos-15 **554,80 s** / ubuntu-latest
**1.316,79 s**), `arayüz` 66 dosya / **1466** test (yerel 1469 farkı ağaçtaki başka
ajanın commit'siz 3 testi), paket `index-Dx7W7QXY.js` **719,82 kB** / gzip
**191,32 kB**, `masaüstü kabuk` 52 + yoksayılan 5 passed (ikinci adımda 5), `windows
güvenli depo` 30, SBOM, sözleşmeler, belge worker'ı, sürüm paketi manifesti ve kaynak
arşivi yeşil.

## F11-T12 — model ve istem sürümü başına kalite ölçümü (2026-11-22)

**Neden.** Aday, alıntı ve karar sicilleri bugüne dek **tek tek** okunuyordu;
«bu model bu istem sürümüyle ne kadar güvenilir öneri üretti» sorusunun tek
dürüst cevabı kayıtlı kanıtlardan **okuma anında** türetilebilir. Kalıcı bir
«kalite puanı» sütunu yazmak, kanıt değişince yalan olurdu.

**Yazılan (`ai_quality.rs`, okuma yolu — şema değişmedi):** `ai_quality_report(case_id)`
üç kanıt ailesini dosya kapsamında birleştirir:

- `ai_runs` → model snapshot + istem sürümü + durum (başarılı/başarısız/sonuçsuz),
- `ai_candidates` → doğrulama durumu (valid/partial/invalid) ve `quote_report`taki
  **alıntı başına** hüküm (exact/normalized/ambiguous/page_mismatch/not_found/
  unverifiable); rapor çözülemezse aday «alıntı yok» değil **ölçülemedi** sayılır,
- `review_items` + `review_decisions` → son karar (eskiden yeniye sıralanır, son
  kazanır); yerine geçmiş öğe isabet oranına girmez, ayrı sayılır.

**Oranlar arayüzde kurulur (`lib/ai-quality.ts`, saf):** kaynak doğruluğu =
(birebir + normalize) / **ölçülen** alıntı; `unverifiable` (sürüm silinmiş,
maske çözülememiş) paydaya girmez — modelin uydurduğunun kanıtı değildir.
Payda 0 ise oran `null` → «ölçülemedi», «%0» yazılmaz. Öneri isabeti yalnız
hüküm verilmiş (kabul+düzeltme+ret) öğeler üzerinden; `ek kanıt` hüküm değil,
öğeyi yeniden karara açar, orana girmez. Düzeltme emeği = düzenleme / hüküm
(süre değil, vekil gösterge). Aday dağılımı paydası toplam aday; bilinmeyen
doğrulama durumu ayrı kovada.

**Arayüz.** AI ekranında «Model kalite ölçümü» kartı: model · istem sürümü
başına çalıştırma/aday/kaynak doğruluğu/inceleme sütunları, satır altında
zayıf/bulunamadı/doğrulanamayan ayrımı ve sınır notu (alıntı eşleşmesi ≠
hukuki doğruluk; düzeltme emeği süre ölçmez). Nesil sayaçlı bayat yanıt
koruması; dosya değişince tablo temizlenir, düşen okuma «kayıt yok» sayılmaz
(kendi hata satırı).

**Testler.** `ai_quality` +5 (alıntı başına sayım ve `unverifiable` payda dışı;
ölçülemedi ≠ alıntı yok; yerine geçmiş öğe isabet dışı + son karar kazanır;
model/istem grupları deterministik; uçtan uca kasa okuması — başka dosya
sızmaz). `lib/ai-quality.test.ts` +8 (oranlar, payda 0 → null, pending
hesabı), `Ai.test.tsx` +3 (kart sayımları, dosya değişiminde bayat yanıt
kapısı). Yerel: `dim-storage --lib` **761/761 yeşil, 581,89 s** (v63 dilimiyle
birlikte; T12 tek başına 5 test ekler), arayüz 67 dosya / **1480** test,
`tsc --noEmit` temiz; iki manifest fmt, `clippy -p dim-storage --all-targets
-- -D warnings`, masaüstü `check --all-targets` temiz.

**Kapı kanıtı (`scratch/dim-t12-gates.py`, log `t12_gates_run1.log`).** Sekiz
çekirdek + altı saf aritmetik + bir ekran mutantı = **15/15 KIRMIZI**, YEŞİL 0,
ÖLÇÜLEMEDİ 0; kaynak sha256 **birebir** geri yazıldı (`ai_quality.rs
e3def7ba6bca50dd…`, `ai-quality.ts deca5b1db44ee088…`, `Ai.tsx
86713833fe362919…`): doğrulanamayanın paydaya girmesi, okunamayan raporun
ölçülemedi sayılmaması, ilk kararın kazanması, yerine geçmişin isabete sızması,
alıntısızın çift sayılması, birebir hükmün paydadan düşmesi, başarısızın
sonuçsuz kovaya düşmesi, karar bekleyenin kabul sayılması; oran paydasına
`unverifiable` eklenmesi, 0/0 → NaN, «başka sayfa»nın zayıf sayılmaması,
ek-kanıtın pending'den düşülmemesi, düzenlemenin kullanılabilir orana
girmemesi, ek-kanıtın düzeltme paydasına girmesi; kalite kartının hiç
çizilmemesi.

**Dürüst sınırlar.**

- **Model çağrısı yapılmadı** — kalite ölçümü kayıtlı kanıtların sayımıdır;
  modelin gerçek çıkarım kalitesini ölçmez.
- «Kaynak doğruluğu» **alıntının metinde bulunmasıdır**, hukuki doğruluk değil.
- `unverifiable` dışlanır; payda bu yüzden küçülebilir ve oran iyimser görünebilir
  — sayı ayrıca yazılır.
- Eski (`legacy`) adaylarda alıntı hükmü yok → ölçülemedi; geriye dönük hesaplanmaz.
- Düzeltme emeği **süre ölçmez**; kaç önerinin düzeltme istediğini sayar.
- Yerine geçmiş öğenin kararı isabete girmez; kararın **doğruluğu** değil
  **son karar** sayılır.
- Rapor **kalıcı değildir** — aday/karar silinirse sayı düşer; her okumada
  yeniden türetilir.
- `unattributed_items` (çalıştırması çözülemeyen kuyruk öğesi) **savunma
  yoludur** ve test edilmedi; normal yolda kapsam kapıları satırı bağlı tutar.
- Arayüz jsdom'da ölçüldü; gerçek Tauri penceresi + görsel QA **insan bekliyor**.

**Commit / CI (ölçüldü).** Kod `455da12` (11 dosya, **+1439/−2**) → run
`45059098496` **10/10 YEŞİL**: `dim-storage` **761** iki OS'ta (macos-15
**498,93 s** / ubuntu-latest **1.032,67 s**), `arayüz` 67 dosya / **1477** test
(yerel 1480 farkı ağaçtaki başka ajanın commit'siz 3 testi), paket
`index-Do2JXiZK.js` **726,53 kB** / gzip **193,19 kB**, `masaüstü kabuk` 52 +
yoksayılan 5, `windows güvenli depo`, SBOM, sözleşmeler, belge worker'ı, sürüm
paketi manifesti ve kaynak arşivi yeşil. Dürüst not: bu koşu `8623e09`
F08-T04ak'ı da kapsar — iki commit **tek push** olduğu için tek CI koşusu ikisini
birlikte ölçtü.
