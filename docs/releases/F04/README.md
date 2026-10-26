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
| F04-T07 | 🚧 | **iptal gerçek** (süreç durdurulur, yarım çıktı yazılmaz) + ayrıştırma arayüz iş parçacığını kilitlemez; **canlı aşama/sayım akışı yok** (worker tek yanıt döndürür, ilerleme çerçevesi göndermez) |

## Doğrulama

`cargo test -p dim-storage --lib` — **332** test (iş kuyruğu 22, iptal yolu 1 yeni).
`cargo test -p dim-ingest --lib` — **39** test (worker 17, iptal 3 yeni).
`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` — **13** test.
clippy: `-p dim-storage`, `-p dim-ingest` ve tauri `--all-targets -D warnings` temiz;
rustfmt iki manifest temiz. CI `40397926418` **7/7 yeşil** (arayüz tsc+vitest+build,
rust macos-14 + ubuntu-latest, masaüstü kabuk, belge worker'ı, sözleşmeler, SBOM).

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
