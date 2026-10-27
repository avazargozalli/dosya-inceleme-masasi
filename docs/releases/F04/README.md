# F04 — UX ve dosya çalışma alanları

- **Sürüm etiketi:** `v0.3.0`
- **Durum:** Devam ediyor (aşağıdaki açık işler)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F04-T01 | ✅ | Dosyalar ekranı: liste, oluşturma, detay |
| F04-T02 | ✅ | arşivle / geri al / geri dönüşümlü silme |
| F04-T03 | ✅ | `row_version` çakışmasında `E_ROW_VERSION` |
| F04-T04 | ✅ | saklama kilidi (`retention_hold`) silmeyi engeller |
| F04-T07 | ✅ | **iptal gerçek** (süreç durdurulur, yarım çıktı yazılmaz) + ayrıştırma arayüz iş parçacığını kilitlemez + **canlı aşama/sayım** (worker ilerleme çerçevesi → `parse-progress` olayı → arayüz sayacı) |

## Doğrulama

`cargo test -p dim-storage --lib` — **338** test (iş kuyruğu 28, canlı sayaç 6 yeni).
`cargo test -p dim-ingest --lib` — **44** test (worker 22; ilerleme ayrıştırma/sınır 5).
`cargo test -p dim-ingest --test worker_progress` — **2** test (gerçek Python worker,
4 sayfalık kurgu PDF: `open 0/4` + `pages 1..4/4`).
`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` — **15** test
(+1: `JobProgressSink` gerçek bellek içi kasayla kuyruk satırına yazıyor).
Python `pytest workers/document/tests` — **140 passed, 1 skipped** (+7 ilerleme testi).
Web `tsc --noEmit` + `vitest run` — **362** test (+11 `parseProgress`, +3 kuyruk
sayaç satırı), `vite build` yeşil.
clippy: `-p dim-ingest` ve tauri `--all-targets -D warnings` temiz; rustfmt iki manifest temiz.

## Canlı aşama/sayım (2026-10-27, F04-T07 kapanışı)

**Neden:** önceki turun açık kaydı "canlı aşama/sayım yok — worker tek istek/tek yanıt
çalışır, ilerleme çerçevesi göndermez; kullanıcı 'çalışıyor' görür, sayfa sayacı görmez"di.

**Protokol (geriye uyumlu):** ilerleme **ayrı çerçevedir**, yanıt değildir:

```json
{"v":1,"id":"req-1","progress":{"stage":"pages","done":12,"total":40}}
```

- `ok`/`result` alanı yoktur; istemci bunu sonuç sanmaz. Aşamalar: `start` (Tauri),
  `open` (belge açıldı, toplam biliniyor), `pages` (her sayfa bitti), `store`
  (kasaya yazma: 0 → N).
- Çerçeve belge metni, dosya adı, yol veya kasa bilgisi taşımaz (denetim sınırıyla aynı).
- Eski worker ilerleme göndermezse istemci yalnız `start` gösterir: **sayaç uydurulmaz**.

**Yapılan:**

- `workers/document/dim_worker/protocol.py`: `progress_frame()`; `engines.extract_pdf` /
  `extract_image` sayfa başına `notify(stage, done, total)` çağırır (geri çağrı yoksa
  hiçbir şey yapılmaz — sonuç değişmez); `worker.serve` ilerlemeyi yazar, yazma düşerse
  ayrıştırma düşmez.
- `crates/ingest/src/worker.rs`: yeni `ParseProgress`, `parse_progress()` (tutarsız sayaç
  — ör. 5 / 3 — **protokol hatası**, sessizce gösterilmez), `read_response` ilerleme
  çerçevelerini ayırır (kimlik kapısı + `MAX_PROGRESS_FRAMES = 10_000` sınırı; sonsuz
  akıtan süreç isteği kilitleyemez), yeni `extract_text_with_progress()`
  (`extract_text` aynı yolu geri çağrısız kullanır).
- Tauri `document_parse`: `parse-progress` olayı yayımlar; `start` olayı **kasadan
  okuma başlamadan** gider, worker çerçeveleri aynen iletilir; olay kaybı ayrıştırmayı
  etkilemez. `ParseProgressEvent` yalnız 4 alan taşır (test bunu tutuyor).
- Arayüz (Dosyalar → belge satırı): ayrıştırma sürerken "Sayfalar okunuyor: 12 / 40 · %30"
  satırı + `role="progressbar"`; olay yalnız **etkin sürüm** için gösterilir; toplam
  yoksa yüzde yazılmaz, toplamı aşan sayaç toplamda kırpılır. `lib/parseProgress.ts`
  4 saf yardımcı + 11 test.

**Ölçüm (bu tur, gerçek worker):**

- 4 sayfalık kurgu fikstür (`fixtures/synthetic/progress-pages.pdf`, 1.431 bayt):
  `open 0/4` → `pages 1/4 … 4/4`, toplam **93,5 ms** (süreç başlatma + ayrıştırma).
- Yalnız worker (bellek içi, 120 sayfa): **121** ilerleme çerçevesi; `open` **0,5 ms**,
  sayfa 1 **4,1 ms**, sayfa 30 **7,8 ms**, sayfa 60 **11,6 ms**, sayfa 120 **19,3 ms**,
  yanıt **20,5 ms** — çerçeveler gerçekten **artımlı**, sonuçla birlikte toplu değil.

**CI'ın yakaladığı kusurlar (#47):** ilk koşu `40409362853` **kırmızı** (5/7) iki ayrı
sebeple:

1. `arayüz`: `parseProgress.test.ts` içinde bir beklenti hâlâ `ocr` etiketini arıyordu
   (etiket kaldırılmıştı) → 359 testte 1 kırmızı. Düzeltildi (`store` etiketi).
2. `belge worker'ı`: **duman testi** (`tests/smoke/worker_smoke.py`) tek satır okuyup
   yanıt sanıyordu; artık ilk satır `progress` çerçevesi → `KeyError: 'ok'` ve süreç
   `shutdown` alamayınca 10 sn zaman aşımına düştü. Bu gerçek bir istemci kusuruydu:
   duman testi ve `scripts/verify-worker-offline.py` ilerleme çerçevelerini **atlar**
   (yanıt değildir); duman testi ayrıca `open 0/1` + `pages 1/1` çerçevelerini doğrular.

**Dürüst sınırlar:**

- Tauri penceresi açılmadı: sayacın **görsel** QA'sı yok; kanıt protokol testleri +
  gerçek worker ölçümü + arayüz birim testleri.
- `store` aşaması 0 → N tek çağrıda yazıldığı için **ara değer yok**; okuma aşaması
  kadar akıcı değil.
- İlerleme yalnız **ayrıştırma** yolunda; kuyruktaki diğer iş türleri yürütücüsüz.
  Aynı kareler artık kuyruk satırına da yazılır (aşağıdaki 0022 bölümü) — kuyruk
  kartındaki sayaç ayrıştırma sırasında **canlı** güncellenir.
- OCR aşaması ayrı ad olarak yayımlanmaz: OCR'lı görüntüde de `pages` sayacı kullanılır.
- İlerleme çerçevesi kaybı ayrıştırmayı etkilemez ama sayaç donabilir; sonuç yine
  döner (olay bilgilendiricidir).

## Kuyruk satırında canlı sayaç (2026-10-27, migration 0022)

**Neden:** önceki turun açık kaydı "`jobs.progress` sütunu güncellenmiyor — kuyruk
kartındaki sayı değişmez"di. Ayrıştırma sırasında sayaç yalnız arayüz olayında
(`parse-progress`) yaşıyordu; kasa kapanıp açıldığında ya da başka bir ekran
kuyruğa baktığında "hangi aşamada, kaç sayfa" bilgisi **kayıptı**.

**Şema (migration 0022):** `jobs.progress_stage TEXT`, `jobs.progress_done INTEGER`,
`jobs.progress_total INTEGER`. Yol **`ALTER TABLE ADD COLUMN`** — tablo yeniden
yazılmaz, 0021'de ölçülen FK/CASCADE tuzağı tekrarlanmaz. Sütunlar arası kural kodda:
`done <= total`, `total > 0`, toplam yoksa `done = 0`.

**Çekirdek (`crates/storage/src/queue.rs`):**

- `update_job_progress(job_id, owner, stage, done, total, now)` — yalnız **kira
  sahibi** yazar (`E_JOB_LEASE`); yalnız `running|validating` iş sayar; iş bu arada
  iptal/engel/bitiş olduysa `Ok(None)` (geç gelen sayaç terminal satırı bozmaz).
  Oran (`progress`) sayaçtan **türetilir** — ayrı hesaplanmaz.
- `ProgressCounts::new` — tutarsız sayaç **reddedilir** (`E_JOB_PROGRESS`): `5/3`,
  `total=0`, toplam yokken sayı. Aşama adı protokol adıdır (küçük harf/rakam/alt
  çizgi, ≤24); serbest metin (dosya adı, yol) sütuna sızamaz.
- `claim_jobs` / `claim_job_by_key` / `retry_failed_job` / kurtarma yeniden sıraya
  alma yolları sayacı **sıfırlar**: yeni denemede "12/40" kalıntısı görünmez.
- `ProgressGate` (saf kural) — arayüz olayı her karede gider, **veritabanı yazımı
  seyrekleştirilir**: ilk kare, aşama değişimi, sayacın 0'a dönüşü ve toplamı bulma
  her zaman yazılır; aradakiler yalnız 300 ms dolunca.

**Tauri (`document_parse`):** yeni `JobProgressSink` olayı ve yazımı tek yerden
yürütür: olay **önce** yayımlanır (arayüz DB'yi beklemez), yazım düşerse ayrıştırma
**düşmez** ama sessizce yutulmaz — yazılan/atlanan/başarısız sayıları
`document_parse`/`progress` denetim kaydına geçer (yalnız sayı, metin/ad/yol yok).

**Arayüz:** Ayarlar → İş kuyruğu kartında yeni **İlerleme** sütunu
("Sayfalar okunuyor: 12 / 40 · %30" + `role="progressbar"`). Biçim ayrıştırma
satırıyla **aynı** kaynaktan gelir (`lib/parseProgress.ts`); iki yerde farklı dil
olmaz. Kart, yürüyen iş varken 1,2 sn'de bir **sessiz** tazelenir (düğmeler
kilitlenmez) ve iş bitince kendiliğinden durur. Bitmiş işte sayaç gösterilmez —
önceki denemenin kalıntısı yanıltıcı olurdu.

**Test:** storage **338** (+6: yazım/oran, terminal kapısı, sayaç doğrulama, kira
kapısı, yeniden sahiplenme sıfırlaması, kapı kuralı), tauri **15** (+1: gerçek
bellek içi kasa + gerçek kuyruk satırı ile sink), Web **362** (+3 `jobs`).

**Dürüst sınırlar:**

- Ölçüm "tek yazım maliyeti" düzeyinde; gerçek pencerede uçtan uca süre ölçülmedi.
- Çok kısa belgede (kareler 300 ms'den hızlı) sayaç 0 → %100 atlar: kapı sayaç
  yazımını zamana bağlar, "her sayfa" garantisi vermez.
- `store` aşaması hâlâ tek çağrıda yazıldığı için ara değer yok (0 → N).
- Sayaç yalnız `parse_document` yolunda yazılır; kuyruktaki diğer iş türlerinin
  yürütücüsü yok.
- Kartın görsel QA'sı hâlâ yok (Tauri penceresi açılmadı).

## İş kuyruğu iptali (2026-10-26, F04-T07)

**Ölçülen kusur (#43):** `job_cancel` yürüyen ayrıştırmaya yalnız `canceling` işareti
yazıyordu. Süreç çalışmaya devam ediyor, `document_parse` sayfaları ve
`finish_parse_run("succeeded")` kaydını yazıyor, **sonra** iş satırı `canceled`
oluyordu: kullanıcı "iptal edildi" görürken çıktı kasadaydı (şartname §17.2
`canceled`: "tamamlanmamış çıktı uygulanmadı"). Ayrıca `document_parse` senkron
komuttu; ayrıştırma boyunca arayüz kilitlendiği için **iptal düğmesine basılamıyordu**.

**Düzeltme:**

- `crates/ingest/src/worker.rs`: `WorkerKiller` — çocuk süreç `Arc<Mutex<Child>>`
  olarak paylaşılır, iptal bayrağı `AtomicBool`. `cancel()` süreci öldürür ve
  toplar (zombi bırakmaz); "iptali ben başlattım" bilgisi bayraktan gelir, çünkü
  ölmüş süreçte `kill()` de başarılı görünür. Yeni `WorkerError::Canceled`
  (`E_WORKER_CANCELED`, `retryable=false`): kopma ve zaman aşımından ayrı kod.
- `apps/desktop/src-tauri/src/state.rs`: `AppState.vault` `Arc<Mutex<…>>` oldu
  (uzun komutlar `spawn_blocking` içinde kilidi tutar); yeni `AppState.parse`
  yuvası süren ayrıştırmanın iptal tutamağını taşır; `ParseSlotGuard` yuvayı
  hata/panik yolunda da temizler.
- `commands.rs`: `document_parse` **async** + `spawn_blocking` (arayüz iş
  parçacığı beklemez). İptal, çıktı yazılmadan **önce** denetlenir; yakalanırsa
  koşu `cancelled` kapanır ve hiçbir sayfa yazılmaz. İptal yolu kuyrukta
  `running → canceling → canceled` olarak biter (`fail_job` iptal isteğini öne alır).
  `job_cancel` **önce** süreci durdurur (kasa kilidini beklemeden), sonra kuyruk
  kaydını yazar ve `workerStopped` alanıyla gerçekleşen sonucu bildirir.
- Arayüz: `cancelOutcomeLine` — süreç durdurulduysa "bekleniyor", durmadıysa
  "durduruldu" denmez; `E_WORKER_CANCELED` mesajı "yarım çıktı kasaya yazılmadı".

**Ölçüm:** iptal, 30 sn zaman aşımını beklemeden **197,8 ms**'de döndü (öldürme →
`E_WORKER_CANCELED`; `worker::tests::cancel_kills_the_running_worker_and_reports_canceled`).

**CI'ın yakaladığı kusur (#45):** aynı test **Ubuntu'da 30,0001 sn** sürdü —
`sh` torun süreç bırakınca borunun yazma ucu açık kalıyor, okuyucu EOF görmüyordu.
Bekleme **50 ms dilimlere** bölündü (`recv_frame`): iptal artık EOF'tan bağımsız
fark edilir. Yeni regresyon testi torun süreci taklit eder
(`cancel_does_not_wait_for_eof_when_a_grandchild_holds_the_pipe`, iptal < 2 sn).

**Yan bulgu (#44):** `E_DISK_FULL` otomatik tekrar listesindeydi; §17.4 "disk
yetersizliği otomatik tekrar döngüsüne girmez" — listeden çıkarıldı.

**Dürüst sınırlar:**

- Tauri penceresi açılmadı: **arayüzün kilitlenmediği görsel olarak doğrulanmadı**;
  kanıt komutun async olması + `spawn_blocking` + derleme/clippy/test.
- Canlı **aşama/sayım** yok: worker tek istek/tek yanıt çalışır, ilerleme çerçevesi
  göndermez; kullanıcı "çalışıyor" görür, sayfa sayacı görmez.
- İptal yalnız **ayrıştırma** yolunda gerçek; CLI sınama iptali ayrı bayrakla
  çalışır (F10). Kuyruktaki diğer iş türleri için yürütücü yok.
- Öldürme `SIGKILL` (çocuk süreç): worker'a graceful "dur" isteği gönderilmez;
  worker'ın bellek içi yarım çıktısı zaten kasaya yazılmadığı için veri kalıntısı
  oluşmaz, ama worker geçici dosya yazdıysa temizlenmez.
- Windows'ta iptal testleri koşmaz (`#[cfg(unix)]`, `/bin/sh` vekil süreç);
  Windows'ta süreç öldürme yolu denenmedi.

## §17.3 belge worker paralelliği (2026-10-27)

**Neden:** şartname §17.3 "aynı anda **en fazla iki** belge worker işi; büyük
dosyada **bir** iş" diyor. Kuyruk yürütücüsü seriydi ve `document_parse` kasa
kilidini **bütün** ayrıştırma boyunca tutuyordu: iki belge paralel istense bile
ikincisi kilidi beklerdi. Yani "iki worker" yalnız kâğıt üzerinde vardı.

**Yapılan (dikey dilim):**

- **`parse_pool.rs` (yeni):** eşzamanlılık kapısı. Normal iş **2** eşzamanlı,
  `LARGE_DOCUMENT_BYTES` (32 MiB) üstü iş **tek başına** koşar. Küçük iş büyük
  işi beklemez; sonsuz açlığa karşı `LARGE_FAIRNESS_WAIT` (2 sn) sonrası yeni
  küçük iş alınmaz. `QUEUE_NOTICE_WAIT` (150 ms) aşılırsa arayüze **`queued`**
  karesi gider (sessiz bekleyiş yok).
- **Üç aşamalı ayrıştırma (`run_parse`):** (A) kilit altında sürüm + kuyruk
  sahiplenmesi + blob çözümü; (B) **kilit yokken** worker süreci; (C) kilit
  altında koşu/sayfa/segment yazımı. Kasa kilidi artık ayrıştırma boyunca
  tutulmuyor → iki belge **gerçekten** paralel.
- **Tek yazıcı korunur:** ilerleme yazımları kilit dışında **kısa** kilitlerle,
  sonuç yazımı zaten kilit altında; iki ayrıştırma aynı anda yazmaz.
- **Kira sahibi tekil:** `parse-<pid>-<sıra>`; iki iş aynı sahibi taşısaydı
  kuyruk "kim kiraladı" ayrımını kaybederdi.
- **§17.5 uyumu:** `vault_lock` artık **önce** süren ayrıştırmaları durdurur
  (kilit bırakılarak çalışıldığı için gerekli oldu). `job_cancel` iş kimliğiyle
  eşleşen süreci durdurur; diğer paralel ayrıştırma etkilenmez. Yazma aşamasında
  iş "bizim + `running`" değilse **çıktı yazılmaz** (§17.2 `canceled`).
- **Toplu ayrıştırma:** `documents_pending_parse` (güncel sürümü var, başarılı
  ayrıştırması yok) + `documents_parse_pending` (en fazla 4 belge, havuz 2).
  Arayüz: **Belgeler → "Bekleyenleri ayrıştır"** düğmesi; sonuç kartı belge
  başına gerçek sonucu, sayfayı ve **gözlenen eşzamanlılık tepesini** yazar.
  `jobs` tarafında toplu iş için tek denetim satırı (yalnız sayılar).

**ÖLÇÜM (gerçek kasa + gerçek Python worker, debug derleme, Apple M2):**
`two_parses_run_concurrently_on_a_real_vault` (elle: `--ignored --nocapture`),
iki belge (`progress-pages.pdf` 4 sayfa, `redaction-hidden.pdf` 1 sayfa),
ısınma koşusundan sonra:

| Koşu | Duvar saati (1. ölçüm) | Duvar saati (2. ölçüm) | Not |
|---|---|---|---|
| Seri (2 belge sırayla) | 1,235 s | 1,233 s | belge başına ≈ 0,62 s |
| Paralel (2 worker) | **0,721 s** | **0,699 s** | **1,71× / 1,76×** |
| Çakışma | 0,720 s | 0,698 s | iki aralık neredeyse tamamen kesişiyor |

Ayrıştırma süresinin baskın kısmı **süreç başlatma + PyMuPDF importu** (≈ 0,4 s);
belge 4 sayfa olduğu için kazanç üst sınırı süreç sayısıyla sınırlı. Küçük
belgede iki worker **daha yavaş değil**, ama kazanç 2× değil: ölçülen 1,71×.

**İlk ölçüm tuzağı (kayda değer):** ilk denemede paralel koşu **önce**, seri
koşu **sonra** yapıldı ve paralel 1,816 s / seri 1,389 s çıktı — yani "paralel
daha yavaş" göründü. Sebep paralellik değil **soğuk önbellekti** (ilk iki süreç
PyMuPDF'i diskten okuyor). Isınma koşusu eklendikten sonra sıra tersine döndü.
A/B sırası ısınma olmadan ölçümü yanıltıyor.

**Test:** tauri **21** (+5 `parse_pool`: iki iş sınırı, büyük iş tek başına,
açlık önleyici, kuyruk bildirimi, boş havuz; +1 gerçek kasa/worker ölçümü
`#[ignore]`), storage **339** (+1 `documents_pending_parse`), Web **363**
(+1 `queued` etiketi). clippy tauri `--all-targets -D warnings` ve
`-p dim-storage --all-targets -D warnings` temiz; rustfmt iki manifest temiz.

**Dürüst sınırlar:**

- Ölçüm **debug** derleme ve **küçük** belgelerle; release ve büyük belgede
  kazanç daha yüksek olabilir — **ölçülmedi** (türetilmiyor).
- `LARGE_DOCUMENT_BYTES` = 32 MiB **seçilmiş** bir eşik; gerçek büyük belgeyle
  "tek başına koşma" davranışı ölçülmedi (birim testte 1 byte'lık sınırla
  doğrulanıyor).
- Toplu ayrıştırma **arayüzde görsel QA edilmedi** (Tauri penceresi açılmadı);
  komut katmanı ve havuz testlerle doğrulandı.
- Toplu ayrıştırma **kullanıcı seçimlerini** (OCR, parola, kodlama) taşımaz:
  varsayılanlarla çalışır. Parola korumalı belge `blocked` olur, kullanıcı tek
  belge akışından açar.
- Windows'ta süreç/paralellik yolu denenmedi.

## Bilinen sınırlar

- Erişilebilirlik (klavye/ekran okuyucu) tam denetimi F15'te.
- Çoklu pencere yok; tek yazıcı kuralı korunur.
