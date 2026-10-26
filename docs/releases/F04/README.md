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

`cargo test -p dim-storage --lib` — **332** test (iş kuyruğu 22, iptal yolu 1 yeni).
`cargo test -p dim-ingest --lib` — **44** test (worker 22; ilerleme ayrıştırma/sınır 5 yeni).
`cargo test -p dim-ingest --test worker_progress` — **2** test (gerçek Python worker,
4 sayfalık kurgu PDF: `open 0/4` + `pages 1..4/4`).
`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` — **14** test.
Python `pytest workers/document/tests` — **140 passed, 1 skipped** (+7 ilerleme testi).
Web `tsc --noEmit` + `vitest run` — **359** test (+11 `parseProgress`), `vite build` yeşil.
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

**Dürüst sınırlar:**

- Tauri penceresi açılmadı: sayacın **görsel** QA'sı yok; kanıt protokol testleri +
  gerçek worker ölçümü + arayüz birim testleri.
- `store` aşaması 0 → N tek çağrıda yazıldığı için **ara değer yok**; okuma aşaması
  kadar akıcı değil.
- İlerleme yalnız **ayrıştırma** yolunda; kuyruktaki diğer iş türleri yürütücüsüz.
  `jobs.progress` sütunu bu turda **güncellenmiyor** (olay arayüze gider, kuyruk
  kartındaki sayı değişmez).
- OCR aşaması ayrı ad olarak yayımlanmaz: OCR'lı görüntüde de `pages` sayacı kullanılır.
- İlerleme çerçevesi kaybı ayrıştırmayı etkilemez ama sayaç donabilir; sonuç yine
  döner (olay bilgilendiricidir).

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

## Bilinen sınırlar

- Erişilebilirlik (klavye/ekran okuyucu) tam denetimi F15'te.
- Çoklu pencere yok; tek yazıcı kuralı korunur.
