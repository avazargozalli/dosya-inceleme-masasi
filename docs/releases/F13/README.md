# F13 — Gerçek karartma ve paylaşım kopyası

- **Sürüm etiketi:** `v0.12.0` (hedef)
- **Durum:** Tamam (elle bölge + bağımsız doğrulama)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F13-T01 | ✅ | terim listesiyle gerçek çıkarma (`apply_redactions`, görüntü pikselleri dahil) |
| F13-T02 | ✅ | çıktı yeniden açılıp terimlerin kalmadığı doğrulanır; kalmazsa kayıt yok |
| F13-T03 | ✅ | yeni sürüm `parent_version_id` ile bağlanır; özgün sürüm değişmez |
| F13-T04 | ✅ | plan + bölgeler (sayfa, quad, terim hash'i) kaydı |
| F13-T05 | ✅ | sayfa önizlemesi üzerinde dikdörtgen seçimi; punto koordinatına çevrim; bölge doğrulaması |
| F13-T06 | ✅ | bağımsız pypdf okuyucusu + OCR (tesseract varsa) sızıntı kontrolü; `ocrVerified` dürüst bayrağı |

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
- **Açık kayıt:** (a) `redact-progress` olayı yalnız `versionId` taşır (çalıştırma kimliği yok) →
  aynı sürümde art arda iki maskelemede birincinin **gecikmiş** olayı ikincinin ilerleme satırına
  yazabilir (çekirdek sözleşmesi değişmeden kapatılamaz; ölçülmedi); (b) başarılı maskeden sonra
  belge seçici hâlâ **özgün** sürümü gösterir (tasarım: yeniden maskeleme idempotent), yeni türev
  otomatik seçilmez; (c) uçuşta çizilen bölgenin sayfası, o anki `page` alanından farklı olabilir
  (bölge **görüntünün** sayfasına yazılır — bilinçli); (d) görsel QA + gerçek Tauri penceresi
  **insan bekliyor**.
- **Kimlik:** yeni numara uydurulmadı — şartnamede **F13-T02** (elle metin/görsel bölge seçimini
  sayfa dönüşümlerinden bağımsız koordinatlara çevirme). **Dürüst:** bu README tablosundaki
  numaralandırma şartnameden **kayık** (tabloda T05 = bölge seçimi); kayıt şartname numarasıyla.
