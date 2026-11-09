# F13 — Gerçek karartma ve paylaşım kopyası

- **Sürüm etiketi:** `v0.12.0` (hedef)
- **Durum:** Tamam (elle bölge + bağımsız doğrulama)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F13-T01 | ✅ | terim listesiyle gerçek çıkarma (`apply_redactions`, görüntü pikselleri dahil) |
| F13-T02 | ✅ | çıktı yeniden açılıp terimlerin kalmadığı doğrulanır; kalmazsa kayıt yok |
| F13-T03 | ✅ | yeni sürüm `parent_version_id` ile bağlanır; özgün sürüm değişmez |
| F13-T04 | ✅ | plan + bölgeler (sayfa, quad, terim hash'i) kaydı; **sicil okunur** (plan listesi + bölge sayfa dağılımı; 2026-11-07 turu) |
| F13-T05 | ✅ | sayfa önizlemesi üzerinde dikdörtgen seçimi; punto koordinatına çevrim; bölge doğrulaması |
| F13-T06 | ✅ | bağımsız pypdf okuyucusu + OCR (tesseract varsa) sızıntı kontrolü; `ocrVerified` dürüst bayrağı |
| F13-T04b | ✅ | plan bölgeleri **gönderim maskesine** bağlandı (2026-11-09 turu; terimler çekirdekte türetilir, `E_MASK_PLAN`) |
| F13-T04c | ✅ | plan satırı **ürettiği çıktı sürümünü** taşır (şema v28; bağ doğrulanır, eski planlar "kayıtlı değil" der; 2026-11-09 turu) |

## Dürüst durum

İki mod vardır: **terimle** (PyMuPDF tüm sayfalarda arar) ve **bölge seçerek**
(sayfa önizlemesi üzerinde fareyle dikdörtgen; piksel → punto çevrimi `72/dpi`).
Her iki modda da `apply_redactions` metni ve görüntü piksellerini gerçekten
kaldırır, çıktı yeniden okunur; kalan içerik varsa dosya **hiç kaydedilmez**
(`E_REDACT_VERIFY`). Terim modunda ayrıca pypdf + OCR ile bağımsız doğrulama
yapılır. Bölge modunda OCR doğrulaması yapılmaz; bu, kayıtta dürüstçe belirtilir
(`ocrVerified=false`).

## Doğrulama

`python -m pytest workers/document/tests/test_redaction.py` — 5 test (metin katmanı
temizliği, boş terim reddi, eşleşmeyen terim, bağımsız pypdf doğrulaması, OCR bayrağı).
Tesseract yoksa `ocrVerified=false` döner; "görüntü doğrulandı" denmez. CI: yeşil.

## Tur kaydı — 2026-11-06 (ilerleme olayına çalıştırma kimliği)

- **Kapanan açık kayıt:** bu dosyanın (a) maddesi — `redact-progress` olayı yalnız
  `versionId` taşıyordu; aynı sürümde art arda iki maskelemede birincinin
  **gecikmiş** karesi ikincinin ilerleme satırına yazabiliyordu.
- **ÖLÇÜLEN KUSUR (eski kod):** arayüz kapısı yalnız `versionId`e bakıyordu.
  İkinci maskeleme başladığında birincinin gecikmiş karesi `versionId` eşleştiği
  için **yeni çalıştırmanın satırına** yazılıyordu (yeni maskeleme hiç sayfa
  işlemeden "3 / 4 · %75" gösterebiliyordu). `busy` dışında kareler zaten
  düşüyordu; kusur yalnız **çalıştırma sürerken** görünüyordu.
- **Düzeltme (çekirdek + arayüz):**
  - `ParseProgressEvent` artık `runId` taşır (5 alan: `versionId`, `runId`,
    `stage`, `done`, `total`). Kimliği **çağıran** üretir; `document_redact`,
    `document_redact_regions` ve `document_parse` komutlarına verir, çekirdek her
    karede aynı kimliği yayımlar (`progress_run_id`: kırpar, **boş kimliği
    `null`** sayar — boş dize hiçbir çalıştırmayla eşleşmezdi).
  - Kuyruktan kendiliğinden başlatılan işler (`documents_parse_pending`,
    `job_retry`, `job_run`) `runId: null` yayımlar: çağıranın bilmediği kimliği
    çekirdek **uydurmaz**; arayüz o kareleri kendi satırına yazmaz (kuyruk satırı
    `jobs.progress_*` alanlarından okunur).
  - `Redaction.tsx` ve `Cases.tsx` kapısı `(versionId, runId)` **çifti** üzerinden
    çalışır; kapanış (`finally`) yalnız kendi kimliğinde temizler, hedef değişimi
    kimliği kapatır (INV-05).
- **Testler:** Rust `parse_progress_event_carries_run_identity_and_counts`
  (+`runId: null` dürüstlüğü) ve `progress_run_id_trims_and_drops_blank` →
  masaüstü kabuk tabanı **34 koşar / 5 yoksayılır** (kaynak 39 test). Arayüz:
  `Redaction.test.tsx` **+2** (aynı sürümde gecikmiş kare; kimliksiz olay),
  `Cases.test.tsx` **+1** (aynı belgenin ikinci ayrıştırması) → toplam **1171**.
- **Kapı kanıtı (tek kapı mutasyonu, her biri kendi testini düşürdü):** R1
  Redaction kimlik kapısı kaldırıldı → *"aynı sürümde ikinci maskelemede birincinin
  gecikmiş karesi görünmez"* kırmızı; R2 → *"çalıştırma kimliği taşımayan
  (kuyruktan gelen) olay kendi satırına yazılmaz"* kırmızı; C1 Cases kimlik kapısı
  → *"aynı belgenin ikinci ayrıştırmasında birincinin gecikmiş karesi görünmez"*
  kırmızı; Rust `run_id: None` → olay testi kırmızı; normalizasyon kaldırıldı →
  kimlik testi kırmızı. Geri yükleme sha256'ları **birebir**.
- **Dürüst sınırlar:** kimlik **arayüz üretimi**; doğrudan IPC çağrısı kimlik
  vermezse olay `runId: null` taşır ve arayüz onu göstermez (kapalı yön — yanlış
  satır yazılmaz, ilerleme görünmez). `job_retry`/`job_run` kareleri kimliksiz
  kaldığı için o yollarda canlı satır yoktur. Kimlik sayaçla üretilir
  (`redact-N`/`parse-N`); pencere yeniden yüklenirse dinleyici de yeniden kurulur.
  Gerçek Tauri penceresi ve görsel QA **insan bekliyor**.

## Tur kaydı — 2026-11-04 (maskeleme ekranı uçuş yolları)

- **Önceki turun açık işi kapandı:** tur başında `HEAD == origin/main` (`c66777c`) ve CI
  **`41824375771` 7/7 YEŞİL** doğrulandı; ağaç temizdi.
- **Neden bu dilim:** sıradaki aday listesinin (1) numarası — `Redaction` (488 satır / 20 test).
  Ekranın **dosya kimliği** (INV-05), sayfa bağlama ve sonuç künyesi yolları önceki turlarda
  kapanmıştı; **hedef değişimi sırasında uçuştaki sayfa isteği** ile **maskeleme uçuşunda
  kullanıcı girdisi** hiç ölçülmemişti.
- **ÖLÇÜLEN KUSUR 1 (kilit devri yok — en ağırı):** `documentPagePreview` askıdayken dosya/
  belge değişince `pageBusy` bırakılmıyordu → yeni hedefin "Sayfayı getir" düğmesi eski isteğin
  yanıtına kadar (yanıt hiç gelmezse süresiz) **kapalı** kalıyordu. Kırmızı: yeni belge seçili
  hâlde `button[name="Sayfayı getir"]` DOM'da yok (düğme "Sayfa getiriliyor…" adıyla kapalı).
- **ÖLÇÜLEN KUSUR 2 (aynı kök, dosya yolu):** dosya değişiminde de kilit bırakılmıyordu; yeni
  dosyanın `v9` sürümü seçili olduğu hâlde sayfa alanı kilitliydi.
- **ÖLÇÜLEN KUSUR 3 (geç yanıt kilidi erken açar):** `loadPage`ın `finally`si **koşulsuz**
  `setPageBusy(false)` çağırıyordu; kilit devredildikten sonra yeni hedef için başlatılan istek
  uçuştayken eski yanıt gelince düğme yeniden "Sayfayı getir" oluyordu (ikinci istek görünmez).
- **ÖLÇÜLEN KUSUR 4 (sessiz kayıp):** `applyRegions` başarısı **bütün** bölgeleri temizliyordu →
  maskeleme sürerken çizilen yeni bölge yanıtla birlikte siliniyordu (gönderim yükü tıklama
  anındaki listeydi; kullanıcı çizdiğini kaybediyordu). Kırmızı: başarı sonrası
  `Seçimleri temizle (1)` yok, `(0)` var.
- **ÖLÇÜLEN KUSUR 5 (plan uçuşta düzenlenebilir):** "Seçimleri temizle" düğmesi maskeleme
  sürerken açıktı → ekranda plan boşalırken çekirdekte maskeleme sürüyordu (arayüz "iptal"
  izlenimi veriyordu).
- **ÖLÇÜLEN KUSUR 6 (yarım sürükleme taşınıyor):** hedef değişimi `drag` durumunu bırakmıyordu →
  yeni önizlemede eski koordinatlarda **kesikli seçim kutusu** çiziliyor, bırakılırsa bölge
  üretiyordu.
- **Düzeltme (`Redaction.tsx` +32/−22):** tek `resetTargetState()` (useCallback) — nesil artışı,
  sonuç/bölge/önizleme/sayfa/ilerleme/hata temizliği, **sayfa kilidinin devri** ve `drag`
  sıfırlama; `caseId` effect'i ve belge `onChange`i aynı yardımcıdan geçer.
  `loadPage`ın `finally`si yalnız **kendi** isteğinde kilidi bırakır (`seq === pageSeq.current`).
  `applyRegions` gönderilen listeyi (`submitted`) saklar, başarıda yalnız onu düşürür.
  "Seçimleri temizle" `busy` iken kapalı.
- **Ölü kod (dürüst):** `resetTargetState` ilk yazımda `pageSeq.current += 1` de artırıyordu;
  hedef değişiminde kapı zaten `gen` olduğu için bu satır **tek başına hiçbir testi düşürmüyordu**
  → koddan çıkarıldı, yedekli kapı bırakılmadı (yorumda kayıtlı).
- **Testler:** `Redaction.test.tsx` **20 → 27** (+7; yeni `describe` "Maskeleme ekranı uçuş
  yolları"). **Ham koşu (düzeltme öncesi bileşen, nihai 27 test): 6 failed | 21 passed** —
  altısı da gerçek kusur; "devredilen sayfa isteğinin hatası yeni belgede görünmez" testi
  düzeltme öncesinde de **yeşildi** (sabitleme).
- **Test koşum aracı düzeltmesi (dürüst):** dosyanın `beforeEach`i `vi.clearAllMocks()` idi; bu,
  bir kezlik (`mockReturnValueOnce`) kuyruğunu **bırakır** ve düşen bir testin artığı sonraki
  testin yanıtını belirler (ilk koşuda "bölge korunur" testi bu yüzden yanlış yerde düştü) →
  `vi.resetAllMocks()` (tüm uygulamalar zaten `beforeEach`te kuruluyor).
- **Kapı kanıtı (5/5 tek kapı mutasyonu kırmızı; geri yükleme sha256
  `3f52026d93182847a2c05e3350ccc378ee78d1c2b02565d1bbf2d13a7ca2217a` birebir):** M1 hedef
  değişiminde sayfa kilidini bırakma (**3** test), M2 `finally` kapısı (1), M3 yalnız gönderilen
  bölgeleri temizleme (1), M4 hedef değişiminde `drag` sıfırlama (1), M5 maskeleme uçuşunda plan
  kilidi (1). **Dürüst:** "geç yanıt kilidi erken açmaz" testi düzeltme öncesinde **kilit
  kusuru** yüzünden düşüyordu (ikinci istek hiç başlamıyordu); asıl erken-açma kapısını ölçen
  kanıt M2'dir.
- **Ölçüm:** `tsc --noEmit` temiz · `vitest` **1026/1026** (60 dosya; 1019 → +7) · `pnpm build`
  temiz (**582,82 kB**, `index-C1rf1YHi.js`, gzip **157,19 kB**; CSS `index-CPW4QqlF.css`).
  Rust değişmedi.
- **Commit/CI (ölçüldü):** kod `5aad05a` (2 dosya, +222/−23) → **`41833574226` 7/7 YEŞİL**
  (`arayüz` günlüğü **60 dosya / 1026 test** ve aynı paket `index-C1rf1YHi.js` 582,82 kB /
  gzip 157,19 kB → yerel/CI **birebir**; `rust (macos-15)`, `rust (ubuntu-latest)`,
  `masaüstü kabuk`, `belge worker'ı`, `SBOM`, `sözleşmeler`). Bu belge commit'inin kendi koşusu
  durum dosyasında (`dim-devam-state.md`) kayıtlı.
- **Dürüst sınırlar:** jsdom **tarayıcı değil** — gerçek Tauri penceresi, gerçek işaretçi
  sürüklemesi, gerçek PDF rasterleştirmesi ve **görsel QA yok**; `api` mock'lu olduğu için
  `document_page_preview`, `document_redact`, `document_redact_regions` SQL/Rust tarafı
  **koşmaz**. Kilit devri **arayüz düzeyinde**: doğrudan IPC bu kapıları atlar. Maskeleme tek
  uçuş (`busy`), hedef seçicileri uçuşta kapalı; `busy` tek bayrak (terim ↔ bölge paralel koşmaz).
- **Açık kayıt:** (a) ✅ **kapandı (2026-11-06 turu):** `redact-progress` (ve
  `parse-progress`) olayı artık çağıranın ürettiği **çalıştırma kimliğini**
  (`runId`) taşır; aynı sürümde art arda iki maskelemede birincinin gecikmiş
  karesi ikincinin satırına yazılmaz (ayrıntı: aşağıdaki 2026-11-06 tur kaydı).
  (b) başarılı maskeden sonra
  belge seçici hâlâ **özgün** sürümü gösterir (tasarım: yeniden maskeleme idempotent), yeni türev
  otomatik seçilmez; (c) uçuşta çizilen bölgenin sayfası, o anki `page` alanından farklı olabilir
  (bölge **görüntünün** sayfasına yazılır — bilinçli); (d) görsel QA + gerçek Tauri penceresi
  **insan bekliyor**.
- **Kimlik:** yeni numara uydurulmadı — şartnamede **F13-T02** (elle metin/görsel bölge seçimini
  sayfa dönüşümlerinden bağımsız koordinatlara çevirme). **Dürüst:** bu README tablosundaki
  numaralandırma şartnameden **kayık** (tabloda T05 = bölge seçimi); kayıt şartname numarasıyla.

## Tur kaydı — 2026-11-07 (maskeleme planı sicili okunur)

- **ÖLÇÜLEN KUSUR:** her maskelemede `redaction_plans` + `redaction_regions` yazılıyor
  (`commit_redaction` → `create_redaction_plan` + `add_redaction_regions`), ama
  `list_redaction_plans` **yalnız `crates/storage` içinde** tanımlıydı — Tauri komutu,
  `lib/api.ts` bağı ve ekran kartı yoktu (`git grep`: tek tüketici yok). `redaction_regions`
  için **okuma yolu hiç yoktu** (storage'da bile). Ekranda yalnız son çalıştırmanın **ham plan
  kimliği** duruyordu; ekran kapanınca kayboluyordu. Avukat "hangi belgede hangi sürümde,
  hangi sayfalarda kaç bölge maskelendi" sorusunu yanıtlayamıyordu.
- **İkinci ölçülen boşluk:** plan satırı yalnız `document_version_id` (UUID) taşıyordu;
  okunacak belge adı/sürüm numarası yoktu.
- **Yazılan:**
  - `RedactionPlanRow` artık `document_title`, `version_no`, `region_count` taşır; iki sorgu
    (`get`/`list`) `document_versions`/`documents` ile **sol birleşim** + `COUNT(*)` alt sorgusu
    kullanır. Sürüm/belge bulunamazsa satır **düşmez**, ad `null` kalır (kimlik dururken ad
    uydurulmaz). Liste sırası `created_at DESC, rowid DESC` — aynı damgaya düşen iki plan bile
    yazılma sırasını korur.
  - Yeni `list_redaction_regions(plan_id)` (storage): **yazılma sırası** (`rowid ASC`), `quads`
    JSON'u çözülür; bozuk geometri `rects: null` döner (satır listede kalır, geometri uydurulmaz).
  - Yeni `redaction_plan_list` / `redaction_plan_regions` komutları + `api.redactionPlanList` /
    `api.redactionPlanRegions` + `RedactionPlanRow`/`RedactionRegionRow` tipleri.
  - Maskeleme ekranında **"Maskeleme planları"** kartı: belge · sürüm · durum rozeti · bölge
    sayısı · kaynak özeti öneki · zaman. Tanınmayan durum **ham değeriyle** yazılır (uydurulmuş
    etiket yok). "Bölgeleri göster" planın sayfa dağılımını verir ("Sayfa N: M bölge"), geometrisi
    okunamayan bölge sayısı ayrı uyarıda. Seçim yokken **okuma yapılmaz** ("kayıt yok" denmez);
    düşen liste okuması **kendi satırında** (sessiz yutma yok); liste istekten **önce** temizlenir;
    geç yanıt `plansSeq`/`regionsSeq` ile düşer; dosya değişiminde sicil + açık bölge detayı
    birlikte düşer (INV-05).
  - Başarılı maskelemeden **hemen sonra** sicil tazelenir ve yeni plan satırı
    "bu çalıştırma" rozetiyle işaretlenir (`result.plan.id`).
  - 29 i18n anahtarı (`redaction.plans*`, `redaction.plan*`); gömülü metin yok.
- **Testler:** storage `redactions::` **6** (2 → +4): ad/sürüm/bölge sayısı birleşimi, sürüm
  kaybolsa da satır durur (ad `null`), damga eşitken `rowid` sırası, bölge sırası + bozuk `quads`
  → `rects: null`. Arayüz `Redaction.test.tsx` **29 → 40** (+11; yeni `describe` "Maskeleme sicili
  (plan okuma yolu)"). Toplam: `dim-storage --lib` **411 → 415**, vitest **1190 → 1201** (60 dosya).
- **Kapı kanıtı (gerçek koşular, betik: `scratch/redaction-plan-gates*.py`):** düzeltme öncesi
  HEAD bileşeni + yeni testler **`11 failed | 29 passed`** (on biri de kırmızı). Arayüz mutasyonları
  **9/9 kırmızı**: M1 dosya değişiminde temizlik (1), M2 hata satırı (1), M3 tanınmayan durum (1),
  M4 sayfa numarası (**4** — *cerrahi değil*), M5 geometri uyarısı (1), M6 bölge okuma hatası (1),
  M7 aç/kapa kapanışı (1), M8 gönderim sonrası tazeleme (**2** — *cerrahi değil*), M9 "bu çalıştırma"
  işareti (1). Çekirdek **5/5**: R1 sol→iç birleşim (1), R2 bölge sayısı (1), R3 sıra yedeği (1),
  R4 bölge sırası (1), R5 bozuk geometri (1). Geri yükleme sha256 birebir (arayüz `7bfbf1b2…`,
  çekirdek `c6a6410e…`).
  **Dürüst (kendi hatam):** M7 ilk ölçümde **yeşil kaldı** — test yalnız "liste boşaldı"yı
  ölçüyordu; `openPlanId` açık kalsa da liste temizlendiği için geçiyordu. Test kapanışı **düğme
  adından** (`Bölgeleri göster` geri döner) okuyacak biçimde düzeltildi, M7 yeniden ölçüldü
  (`1 failed`). R1/R2 ilk betikte "EŞLEŞMEDİ" verdi: aynı ifade **iki sorguda** geçiyor → iki
  sorguda birden mutasyona çevrildi ve yeniden ölçüldü.
- **Ölçüm (yerel, Node 22.23.1 + TZ=UTC):** `dim-storage --lib` **415/415** (247,49 s) · clippy
  `-p dim-storage --all-targets -D warnings` temiz (5,05 s) · masaüstü kabuk clippy
  `--all-targets -D warnings` temiz (1 dk 03 s) · `cargo check --all-targets` temiz (6,19 s) ·
  `cargo fmt` iki manifest temiz · masaüstü kabuk kapısı **34 koştu · 0 düştü · 5 yoksayıldı**
  (kaynak 39 test) · `tsc --noEmit` temiz · vitest **1201/1201** (60 dosya) · `npm run build`
  **613,88 kB** (`index-vkPakvwy.js`, gzip **164,74 kB**; CSS 7,28 kB).
- **Commit/CI (ölçüldü):** kod `04c2b6f` (8 dosya, +859/−12) → **`42317787444` 10/10 YEŞİL**;
  `dim-storage` **415 passed** hem `macos-15` (166,59 s) hem `ubuntu-latest` (346,11 s) — **birebir**;
  `arayüz` **60 dosya / 1201 test**, paket `index-vkPakvwy.js` 613,88 kB / gzip 164,74 kB
  (yerel/CI **birebir**); `masaüstü kabuk` **34 koştu · 0 düştü · 5 yoksayıldı**.
- **Dürüst sınırlar:** (a) yeni iki komut **komut olarak koşulmadı** (`State<AppState>` test
  altyapısı yok) — kablolama `cargo check` + clippy, davranış storage düzeyinde **gerçek SQLite**
  ile; (b) sicil salt-okunur: plan/bölge **silinemez, düzenlenemez**, filtre/sayfalama yok;
  (c) `document_title` **anlık** okunur — belge yeniden adlandırılırsa eski plan yeni adı gösterir
  (sürüm kimliği ve bölge sayısı değişmez); (d) bölge geometrisi `quads` JSON'undan çözülür; bozuk
  satır `rects: null` ile görünür ama **onarılmaz**; (e) bölge `category`/`user_decision` bugün
  sabit (`user_term`/`redact`) — kural tabanlı kategoriler (TCKN/IBAN) henüz üretilmiyor, sicil
  bunu gizlemez (ham değer okunur); (f) jsdom tarayıcı değil → gerçek Tauri penceresi ve
  **görsel QA insan bekliyor**.
- **Açık kayıt:** (a) sicil **dosya kapsamlı**; belge bazında filtre yok; (b) ~~plan→çıktı sürümü
  bağı (`parent_version_id`) sicil satırında **gösterilmiyor** (yalnız belge/sürüm no)~~ —
  **2026-11-09'da kapandı:** plan satırı `output_version_id` taşır (şema v28), çıktı belge adı +
  sürüm no ile yazılır; eski planlar "kayıtlı değil" der. (c) plan
  durumu `applied` dışına ürün akışında geçmiyor (draft→applied tek adım) — yeniden inceleme
  akışı (F13-T01'in "yerleşim değişince tekrar incelet" kısmı) hâlâ ayrı iş.

## Tur kaydı — 2026-11-09 (plan bölgeleri gönderim maskesine bağlanır)

- **Ölçülen boşluk:** F11'in gönderim maskesi yalnız kullanıcının elle girdiği terimlerle
  çalışıyordu; F13 planı ise paylaşım PDF'ine uygulanıyordu. Aynı belge AI'ya giderken
  planın kırmızı bölgelerindeki adlar **maskesiz** çıkabiliyordu. Planda terim saklanmaz
  (yalnız bölge geometrisi + `detected_text_hash`), bu yüzden terimler metinden türetilmeliydi.
- **Yazılan:** `redactions::outbound_terms_from_plan` — planın `redact` bölgeleriyle
  **kesişen** `text_segments.bbox_list` bloklarının karakter aralıkları kesilir; kırpılan
  parçalar `validate_terms` ile kanonikleştirilir (Türkçe 1:1 katlama, tekilleştirme,
  uzundan kısaya). Terimler **kasaya yazılmaz**; `ai_run_execute` `maskPlanId` alır,
  maskeyi çekirdek uygular. Plan bu onayın kapsamındaki sürüme ait değilse
  `E_MASK_PLAN_SCOPE`; denetim kaydı plan kimliği + bölge/blok/terim sayısı yazar.
- **Kapılar (`E_MASK_PLAN`, hepsi hatayla kapanır):** plan güncel sürüme ve aynı render
  özetine ait olmalı · sayfa döndürmesi 0 · sayfa koordinat uzayı `unrotated_cropbox_points`
  (plan `pdf_page_points`) · bölge geometrisi okunabilir · bölgede **en az bir** kullanılabilir
  terim var. Herhangi biri tutmazsa **hiçbir bayt gönderilmez**; sessiz eksik maske yok.
- **Arayüz:** AI ekranında "Karartma planı" seçicisi (plan listesi `redaction_plan_list`).
  Düşen liste "plan yok" demez (`maskPlanUnreadable` + hata satırı); dosya değişiminde seçim
  temizlenir ve çalıştırma plan kimliği taşımaz.
- **Testler:** storage `redactions` **8 → 17** (+9: kesişim, çakışan bölge tekilleştirme,
  kısa parça sayımı, bayat sürüm, yabancı dosya, tamper edilmiş `case_id`, render özeti,
  döndürülmüş sayfa, uzay uyuşmazlığı, metinsiz sürüm) → `dim-storage --lib` **484/484**
  (252,02 s). Arayüz `Ai.test.tsx` **98 → 101** (+3: plan seçimi → `maskPlanId`, düşen liste,
  dosya değişiminde taşınmama) → vitest **1323/1323** (64 dosya).
- **Kapı kanıtı (gerçek koşular; betik `scratch/dim-plan-mask-gates.py`, her mutasyon tek
  başına, `try/finally` + sha256 birebir):** **11/11 kırmızı** — C1 bölge kesişimi kapalı ·
  C2 plan dosya kapsamı denetlenmez · C3 bayat sürüm · C4 döndürülmüş sayfa · C5 PDF dışı uzay ·
  C6 metinsiz bölge maskesiz geçer · C7 tek karakter terime girer · C8 render özeti ·
  U1 plan kimliği geçirilmez · U2 düşen liste "plan yok" der · U3 seçim taşınır.
  **Dürüst (kendi hatam):** ilk ölçüm **8/11** idi — C2/C8 için test yoktu ve U3 testi
  DOM eşleşmesine bağlıydı (seçenek listede olmayınca kendiliğinden boşalıyordu). İki çekirdek
  testi eklendi, U3 testi **gönderilen çalıştırmanın argümanını** ölçecek biçimde düzeltildi;
  ikinci ölçüm 11/11.
- **Dürüst sınırlar:** (a) çıkarım **blok düzeyinde**: bölge bir satırın parçasını kapsasa da
  o blok bütün olarak maskelenir (fazla maskeleme; eksik maskeleme tercih edilmez);
  (b) 1 karakterlik parçalar `validate_terms` alt sınırı yüzünden maskelenemez — **sayılır**
  (`shortSkipped`) ama sessiz kalmaz; (c) yalnız `unrotated_cropbox_points` + döndürmesiz
  sayfalar; OCR/görüntü uzayı (`image_pixels`) çevrilemez, plan reddedilir; (d) plan
  `parse_run_id` boşsa **en yeni** ayrıştırma kullanılır — doluysa ve güncel değilse reddedilir;
  (e) plan seçici dosyadaki **tüm** planları listeler; belge/sürüm filtresi yok, uyumsuz seçim
  çalıştırmada açık hatayla durur; (f) terimler yalnız gönderim anında türetilir, hiçbir yere
  yazılmaz — aynı maskeyi yeniden üretmek için plan yeniden seçilmeli; (g) jsdom tarayıcı değil
  → gerçek Tauri penceresi + **görsel QA insan bekliyor**.
- **CI (ölçüldü, dürüst):** kod commit `effe881` → `42673881593`, **6/10 iş yeşil**
  (sözleşmeler, masaüstü kabuk, windows, worker, SBOM, kaynak arşivi); `arayüz`, `rust` (iki OS)
  ve `paket` işleri run'da **hiç oluşmadı** (log yok, annotation yok) ve run `failure` kapandı —
  kod hatası olarak okunmadı, ayrıca kovalanıyor. Test/belge commit'i ayrı koşuda doğrulanacak.

## Tur kaydı — 2026-11-09 (plan satırı ürettiği çıktı sürümünü taşır)

- **Ölçülen boşluk (dürüst):** `commit_redaction` her maskelemede **yeni bir belge + sürüm**
  üretir ve `parent_version_id` ile kaynak sürüme bağlar; ama `redaction_plans` satırı yalnız
  **kaynak** sürümü taşıyordu. Sicil bu yüzden "hangi maskeleme hangi çıktıyı üretti" sorusunu
  yanıtlayamıyordu: aynı kaynak sürümden iki maskeleme yapıldığında iki çıktı birbirinden ayırt
  edilemiyor, çıktıyı bulmak için belge adına ("… (maskeli)") bakmak gerekiyordu. Bu, önceki
  turun **açık kayıt (b)** maddesiydi.
- **Yazılan:** şema **v28** `0028_redaction_plan_output.sql` — `redaction_plans.output_version_id`
  (yabancı anahtar, `ON DELETE SET NULL`) + indeks. Bağ **doğrulanmadan yazılmaz**
  (`E_REDACTION_PLAN`): çıktı var olmalı, **aynı dosyaya** ait olmalı, kaynak sürümün **alt
  sürümü** olmalı ve kaynağın kendisi olamaz; tutmazsa plan **hiç yazılmaz** (yanlış iz, izsizlikten
  kötüdür). `NewRedactionPlan.output_version_id` (`serde(default)` — eski çağıranlar `None`),
  `commit_redaction` çıktı sürümünü geçirir, denetim kaydı bağın **varlığını** yazar (kimlik yazılmaz).
  `RedactionPlanRow` üç alan kazanır: `output_version_id`, `output_version_no`,
  `output_document_title` — ikisi **sol birleşimle** okunur, bulunamazsa `null` (ad/numara uydurulmaz).
  Arayüz sicil satırında **"Çıktı: <belge> · sürüm N"** yazar; sütundan önce yazılmış planlarda
  **"Çıktı sürümü kayıtlı değil (bu sütundan önce yazılmış plan)."** yazar — "çıktı yok" denmez,
  çıktı da uydurulmaz. 3 sözlük anahtarı.
- **Testler:** storage `redactions::tests` **17 → 22** (+5: çıktı bağı yazılır/okunur; bağsız plan
  dürüst kalır; alt sürüm olmayan çıktı reddedilir ve **satır yazılmaz**; başka dosyanın çıktısı
  reddedilir; kaynak==çıktı reddedilir) → `dim-storage --lib` **490 → 495** (315,46 s).
  `Redaction.test.tsx` **40 → 43** (+3: çıktı satırı yazılır ve kaynak satırı değişmez; bağsız eski
  plan çıktı uydurmaz; ad okunamazsa **kimlik** yazılır) → vitest **1327 → 1330** (64 dosya).
  Bir eski test daraltıldı: sürüm numarası artık iki satırda geçebildiği için kaynak satırında
  **belge adıyla birlikte** aranıyor.
- **Kapı kanıtı (gerçek koşular; betik `scratch/dim-plan-output-gates-1109.py`, her mutasyon tek
  başına, `try/finally` + sha256 birebir):** **9/9 kırmızı** — C1 bağ yazılmaz · C2 alt sürüm
  kapısı kapalı · C3 dosya kapsamı kapalı · C4 kaynak==çıktı kapalı · C5 liste çıktı bilgisini
  okumaz · C6 kaynak ve çıktı karıştırılır · U1 bağ yokken satır hiç yazılmaz · U2 bağlı plan
  "kayıtlı değil" gösterilir · U3 ad okunamazsa kimlik yazılmaz. C2/C5/C6 **derlendi ve davranış
  testiyle** düştü (derlenmeyen koşu kanıt sayılmadı); U1 filtresinde hedef test `1 failed` verdi.
- **Ölçüm (yerel, Node 22.23.1 + TZ=UTC):** `tsc --noEmit` temiz · vitest **1330/1330** · storage
  **495/495** · `cargo fmt` iki manifest temiz · storage + masaüstü `clippy --all-targets -D warnings`
  temiz (masaüstü 1 dk 23 s) · `upgrade_tests` yeşil (şema 28'e yükseltme dahil).
- **CI (ölçüldü):** kod `54af7e0` → run `42701027424` **10/10 YEŞİL** (`dim-storage` **495** hem
  `ubuntu-latest` (367,55 s) hem `macos-15` (259,20 s) — **birebir**; `arayüz` **64 dosya / 1329
  test** — yerel 1330 farkı ağaçtaki başka ajanın commit'siz `format.test.ts` testi; paket
  `index-CewgFzes.js` **657,06 kB** / gzip **175,70 kB** (CSS 7,28 kB / gzip 2,07); `masaüstü kabuk`,
  `windows güvenli depo`, `sözleşmeler`, `kaynak arşivi`, `paket manifesti`, `belge worker'ı` ve
  `SBOM` yeşil). Belge `6ba1269` → run `42702784226` **10/10 YEŞİL**.

- **Dürüst sınırlar:** (a) plan satırı **kaynak** belgeyi göstermeye devam eder; çıktı ayrı
  sütundadır — arayüzde çıktıya **tıklayıp gitme** yok (gezinme ayrı iş); (b) v28'den önce yazılmış
  planlar `NULL` kalır; geriye dönük **doldurma yapılmadı** (aynı kaynaktan birden çok maskeleme
  varsa ebeveyn bağı tek başına hangi çıktının hangi plana ait olduğunu kanıtlamaz — tahmin
  yazılmaz); (c) bağ yalnız `commit_redaction` yolunda yazılır; doğrudan `create_redaction_plan`
  çağıran bir yol bağsız plan üretebilir (testlerde bilinçli); (d) `document_redact` komutu
  **komut olarak koşulmadı** (`State<AppState>` altyapısı yok) — kablolama check + clippy, davranış
  storage düzeyinde gerçek SQLite; (e) plan satırı taşıyan bir **v27 kasa kopyasından** yükseltme
  ayrıca denenmedi (ALTER yolu her test DB'sinde 1→28 kurulumuyla koşuyor); (f) jsdom tarayıcı
  değil → gerçek Tauri penceresi + **görsel QA insan bekliyor**.
