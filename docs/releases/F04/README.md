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
| F04-T07 | ✅ | **iptal gerçek** (süreç durdurulur, yarım çıktı yazılmaz) + ayrıştırma arayüz iş parçacığını kilitlemez + **canlı aşama/sayım** (worker ilerleme çerçevesi → `parse-progress` olayı → arayüz sayacı) + **maskeleme de kuyruk işi** (kasa kilidi worker boyunca tutulmaz, `redact-progress` aşamaları, iptal) + **yeniden deneme gerçekten çalışır** (2026-10-28) |
| F04-T08 | 🚧 | **merkezi sözlük + biçimlendirme katmanı** (kabuk, Kasa/Workspace/Unlock ekranları taşındı; 11 ekran açık) — 2026-10-28 |

## Merkezi sözlük ve biçimlendirme katmanı (2026-10-28, F04-T08 — kısmi)

**Neden:** şartname F04-T08 "Türkçe metinleri merkezi sözlüğe taşı; tarih/sayı
gösterimini veri türünden ayır" diyor. Ölçüm, ikinci yarısının **hiç
yapılmadığını** ve arayüzde üç ayrı biçimin yan yana yaşadığını gösterdi.

- **ÖLÇÜLEN KUSUR #54 (tutarsız biçim):** aynı bayt değeri üç ayrı yerde üç ayrı
  biçimde yazılıyordu — `providers.ts` `"1,5 KiB"` (virgül, doğru), `Settings.tsx`
  ve `Ai.tsx` `(b/1024).toFixed(1) + " KB"` (**nokta**), `reader.ts`
  `toFixed(1) + " MB"`. Türkçede ondalık ayracı virgüldür; ayrıca 1024'e bölen
  birim `KB` değil `KiB`'dir. Aynı ekranda "1.5 KB" ile "1,5 KiB" birlikte
  görünüyordu.
- **ÖLÇÜLEN KUSUR #55 (uydurma boyut):** `reader.ts` küçük önizlemeyi
  `Math.max(1, Math.round(b/1024))` ile **1 KB tabanına** yuvarlıyordu; 20 baytlık
  bir önizleme kullanıcıya "1 KB" görünüyordu. Kullanıcıya olmayan bir boyut
  söylenmez (ürün değişmezi: "ölçülmeyen değer uydurulmaz").
- **ÖLÇÜLEN KUSUR #56 (gün kayması sınıfı):** takvim tarihi `new Date("2026-01-01")`
  ile ayrıştırılırsa **UTC** gece yarısı olur; eksi saat diliminde gün bir önce
  kayar. Ölçüm (Node 22, `America/New_York`): naif ayrıştırma **31.12.2025**,
  yerel kurulum **01.01.2026**; `Europe/Istanbul`'da ikisi de 01.01.2026 (kusur
  +03'te görünmez). Hukuki tarihte bir gün kayması kabul edilemez.
- **Kapanan kusur #57 (bayat etiket):** kenar çubuğu gömülü `"v0.1.0 · F02"`
  yazıyordu; uygulama F04'te. Artık sürüm **tek kaynaktan** (`package.json`) gelir.

**Yazılanlar**

- **`src/lib/i18n.ts` (yeni, tek sözlük):** `tr` ağacı `as const`; `MessageKey`
  yaprak anahtar birleşimi → yazım hatası **derleme hatası**. `t(key, params?)`
  `{ad}` yer tutucusunu doldurur; **eksik parametre hata atar** (ekranda yarım
  cümle görünmez). `messageKeys()` denetim/test için.
- **`src/lib/format.ts` (tek biçimlendirme katmanı):** `formatDateTime` (anlık
  zaman), **`formatLegalDate`** (takvim tarihi: `23.10.2026`, `10.2026`, `2026` —
  eksik gün uydurulmaz, zaman bileşeni taşıyan değer "tarih biçimi
  desteklenmiyor" döner), `formatUnixSeconds` (0/negatif → "—", 1970 uydurulmaz),
  `formatNumber`, `formatPercent`, **`formatSize`** (bayt/KiB/MiB, virgül),
  `formatDuration`. `formatTimestamp` geriye uyum için aynı fonksiyondur.
- **Taşınan ekranlar (tam):** `App.tsx` (gezinme + kabuk + "hazır değil" ekranı),
  `screens/Unlock.tsx` (kasa açma/oluşturma/kurtarma, 40+ metin),
  `screens/Workspace.tsx`. Etiket haritaları (`CASE_KIND_LABEL`,
  `SENSITIVITY_LABEL`, `AI_POLICY_LABEL`) artık sözlükten **türetilir**.
- **Biçimlendirme çağrı yerleri tek katmana alındı (7 dosya):** `arch.ts`,
  `precedents.ts`, `providers.ts` (`formatBytes` = `formatSize`), `reader.ts`,
  `rollback.ts`, `Ai.tsx`, `Precedents.tsx`, `Providers.tsx`, `Settings.tsx`.
  `src` altında `toLocaleString`/`new Intl`/`"tr-TR"` **yalnız** `lib/format.ts`'te.
- **`src/lib/localization.test.ts` (yeni kapı):** (a) biçimlendirme başka dosyaya
  sızarsa, (b) sözlükte kullanılmayan anahtar kalırsa, (c) taşınmış ekranlara
  gömülü metin geri gelirse, (d) kabukta gömülü sürüm etiketi olursa **kırmızı**.

**Commit/CI:** `4d5c2da` → CI **`40605471296` 7/7 yeşil** (sözleşmeler, arayüz
tsc+vitest+build, rust macos-14 + ubuntu-latest, masaüstü kabuk, belge worker'ı,
SBOM). `HEAD == origin/main`; `git status`ta yalnız iki `Cargo.lock` (başka ajanın,
dokunulmadı).

**Test**

- Web `tsc --noEmit` temiz · `vitest run` **413/413** (30 dosya; önceki 384 → +29:
  `i18n` 6, `format` 19, `localization` 4) · `vite build` temiz (83 modül, 698 ms).
- Biçim testleri **iki saat diliminde** koşuldu: `TZ=UTC` 23/23 ve
  `TZ=America/New_York` 33/33 (takvim tarihi kaymıyor).
- Ölçüm/kanıt: 20 bayt → `"20 bayt"` (eskiden "1 KB"); 1.536 bayt →
  `"1,5 KiB"`; 0 saniye → `"—"`.

**Dürüst sınırlar**

- Sözlüğe **11 ekran taşınmadı** (Dosyalar/Cases, Arama, İnceleme, Emsal, Maskeleme,
  Çıktılar, AI, Sağlayıcılar, Ayarlar, Kronoloji, Görevler, Kontrol listesi).
  F04-T08 bu yüzden **🚧 kaldı**; kalan taşıma mekanik ama dosya dosya yapılmalı.
- Sözlük **tek dilli**: dil değiştirme düğmesi yok, ürün Türkçe sabit. Sözlüğün
  değeri çeviri değil, **tek kaynak + derleme zamanı anahtar denetimi**.
- `package.json`/`tauri.conf.json`/`Cargo.toml` sürümü hâlâ **0.1.0**, F04 belgesi
  `v0.3.0` diyor; sürüm yükseltmesi (updater/SBOM'u etkiler) bu turda **yapılmadı**
  — kenar çubuğu artık paket sürümünü gösterdiği için ekranda "v0.1.0" yazar.
- Tauri penceresi açılmadı → sözlükten gelen metinlerin **görsel QA'si yok**
  (metinler birebir aynı; yalnız kaynak değişti).
- `formatLegalDate` bugün **çağrılmıyor**: takvim tarihi alanları (F05+ belge
  tarihleri) geldiğinde kullanılacak; şimdilik altyapı + test.

## Yeniden deneme gerçekten çalışıyor (2026-10-28, F04-T07 kapanışı)

**Neden:** şartname F04-T07 "gerçek aşama, sayım, iptal ve **yeniden deneme**
denetimlerini sun" diyor. Önceki turun açık kaydı "arayüzde yeniden deneme
denetimi gerçek mi?" sorusuydu; **ölçüldü: değildi**.

- **ÖLÇÜLEN KUSUR #52 (işlevsel, kritik):** `job_retry` yalnız
  `retry_failed_job` + `append_audit` çağırıyordu (12 satır). Kuyrukta **arka
  plan yürütücüsü yok**: `claim_job_by_key` yalnız `document_parse`,
  `prepare_redaction` ve `documents_parse_pending` içinden çağrılıyor. Yani
  düğme `failed → queued` yazıyor, iş **sonsuza kadar "Bekliyor"** kalıyordu;
  arayüz mesajı "yeni deneme numarası açılacak" diyordu ama hiçbir deneme
  açılmıyordu — kullanıcının en çok istemediği "çalışıyor gibi görünen düğme".
- **Düzeltme — çekirdek:** yeni `Database::job_idempotency_key(job_id)`; iş
  satırı hangi girdiye ait olduğunu buradan çözer (arayüze **verilmez**).
- **Düzeltme — Tauri:** `job_retry` artık **async** ve iki aşamalı:
  - **A (`plan_retry`, kısa kilit):** `failed → queued`, deneme bütçesi sıfır,
    `parse:<sürüm>` anahtarından sürüm çözümü, denetim
    `{state, jobType, started}` (metin/ad/yol yok).
  - **B (kilitsiz):** `run_parse` `spawn_blocking` ile **gerçekten koşar**;
    arayüz iş parçacığı beklemez, ilerleme `parse-progress` olayı **ve**
    `jobs.progress_*` sütunlarından gelir.
  - **Erken düşüş kapısı:** `prepare_parse` artık gövdeyi sahiplenmeden
    **önce** okur; sürüm/blob yoksa iş `queued` kalır ve `finalize_failed_retry`
    satırı `failed` yazar — "Sırada"da asılı kalmaz. Satır `running` ise (başka
    bir yürütücü sahiplendi) **dokunulmaz**.
  - **Bilinçli sınır:** maskeleme işi otomatik **başlatılmaz**
    (`skipped="type_not_runnable"`): terimler/bölgeler iş satırında saklanmaz,
    uydurulmuş bir istekle yeniden maskelemek yanlış çıktı üretirdi.
- **Arayüz:** `JobRetryView` (`started`, `skipped`, `versionId`, `applied`);
  `retryOutcomeLine` "sıraya alındı" ile "başladı"yı **ayırır** ve uygulanan
  ayarı yazar ("OCR kapalı, kodlama auto"). `shouldPollQueue` sayesinde
  yeniden denemesi başlayan iş terminal olana kadar kart 1,2 sn'de bir tazelenir
  (`jobIsTerminal` eski `succeeded`/`cancelled` adlarını da tanır).
- **YAN BULGU #53 (ölçüldü, test kırığı):** `ParsePool::acquire_noticing`
  `Some(süre)`yı yalnız **bildirim eşiği (150 ms)** aşıldığında döndürüyordu;
  yani "sıraya girdi mi" sorusu zamanlamaya bağlıydı. Yüklü makinede
  `large_document_runs_alone` bu yüzden düştü (100 ms bekleyen büyük iş
  `None` aldı). Artık `Some` = **kapı en az bir kez kapalıydı**; test sabit
  100 ms uyku yerine "büyük iş beklemeye girdi" kapısıyla ilerliyor.
- **ÖLÇÜM (gerçek kasa + gerçek Python worker, debug, Apple M2;**
  `DIM_WORKER_PYTHON=workers/document/.venv/bin/python ... --ignored --nocapture`**)**: `retrying_a_failed_parse_job_really_parses_the_document`
  → **4 sayfa, 4 segment, 1,21 s**, kareler `start → open → pages ×4 → store ×2`,
  iş **`completed`**, `progress_stage=store`, `attempt_count=1` (yeniden
  deneme gerçekten yeni bir denemedir).
- **Test:** `cargo test -p dim-storage --lib queue::` **28/28**; tauri lib
  **27 geçti / 4 ignored** (+3: girdi çözümü, plan, gerçek ölçüm); Web
  `tsc --noEmit` temiz, `vitest run` **384/384** (+2).
- **Dürüst sınırlar:** yeniden deneme **varsayılan ayarlarla** koşar (OCR
  kapalı, kodlama `auto`); özgün istekteki OCR/parola iş satırında saklanmaz →
  taranmış belgede OCR'sız deneme boş metin üretebilir (uyarı olarak görünür).
  Parola korumalı belge yine `blocked` olur; parola belge ekranından girilir.
  Maskeleme yeniden denemesi otomatik başlamaz. Tauri penceresi açılmadı →
  düğme/mesaj satırının **görsel QA'si yok**. Arka plan görevi pencere kapansa
  da sürer; kasaya yazma `commit_parse` kapısından geçer (kasa değiştiyse
  yazmaz). Ölçüm tek makine + kurgu belge.


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

**2026-10-28 (yeniden deneme turu):** `cargo test -p dim-storage --lib queue::` **28/28**;
tauri lib **27 geçti / 4 ignored** (yeni: girdi çözümü, plan, gerçek worker ölçümü);
Web `tsc --noEmit` temiz + `vitest run` **384/384**; rustfmt iki manifest temiz.

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

## Maskeleme kuyruk işi olarak koşar (2026-10-28, F04-T07 genişletmesi)

**Neden:** maskeleme `state.lock()` altında çalışıyordu — uzun maskede bütün arayüz
kilitleniyor, iptal düğmesine basılamıyordu. Ayrıca kuyrukta `parse_document` dışında
yürütücüsü olan iş türü yoktu.

**Yapılan (üç aşamalı `run_redaction`):** (A) kilit altında sürüm + blob + iş satırı
(`prepare_redaction`), (B) **kilit yokken** worker süreci ve doğrulama, (C) kilit altında
çıktı/denetim yazımı (`commit_redaction`). Yeni iş türü `redaction` (idempotent anahtar
`redact:<sürüm>:<parmak izi>`, aynı niyet ikinci kez `E_REDACT_BUSY`), iptal yuvası artık
iş türü taşır (`RunningParse` → `RunningWorker` + `kind`), yeni `redact-progress` olayı;
`redaction` / `redaction_regions` komutları async (`spawn_blocking`).

### ÖLÇÜLEN KUSUR #50 — ürün zarfı OCR'ı çökertiyordu (kritik, işlevsel)

`worker_env()` yalnız `PATH/LANG/LC_ALL` veriyordu; `TMPDIR` yoktu. Bu makinede
`/tmp` bir **sembolik bağ**tır ve pytesseract geçici çıktıyı okuyamıyor:

| zarf | sonuç |
|---|---|
| `PATH/LANG/LC_ALL` (ürün) | **`UnicodeDecodeError`** (`byte 0x89`, PNG imzası) |
| `+ HOME` | aynı hata (çözüm değil) |
| `+ TMPDIR=/tmp` | **aynı hata** (sembolik bağ) |
| `+ TMPDIR=/private/tmp` | ✅ `Sayfa 2 kurgu` |
| `+ TMPDIR=<gerçek dizin>` | ✅ |

Eski kodda doğrulayıcı istisnası **sızıntı** sayılıyordu (aşağıdaki #51) → ürün
`E_REDACT_VERIFY` veriyor ve **maskeli dosya hiç kaydedilmiyordu**. Yani tesseract
kurulu bir makinede maskeleme ürün yolunda sessizce çalışmıyordu; önceki turlar bunu
görmedi çünkü testler tam ortamla (`spawn_dev`) koşuyordu.

**Düzeltme:** `worker_env()` artık `TMPDIR/TMP/TEMP` verir; değer kabuk ortamından
devralınmaz, `std::env::temp_dir()`in **çözülmüş** yoludur (`/tmp` → `/private/tmp`).
Regresyon testi: `worker_env_hands_a_real_temp_directory` (mutlak + var + sembolik bağ
değil + üç ad aynı değer).

### ÖLÇÜLEN KUSUR #51 — doğrulayıcı çökmesi "sızıntı" diye raporlanıyordu

`_verify_with_pypdf`, `_verify_with_ocr`, `_region_ocr_leaks` istisna yolunda
`list(terms)` dönüyordu: doğrulayıcı hatası "terim hâlâ var" gibi görünüyordu. Artık
üçlü döner — `(bulunan, çalıştı_mı, hata)`; çökme `remaining`e **girmez**, uyarı olarak
yazılır ("pypdf/OCR doğrulaması yapılamadı"), `engines` listesine o doğrulayıcı eklenmez
ve `ocr_verified=false` kalır. Bölge maskesinde "bölgede metin kaldı" ile "doğrulanamadı"
ayrıldı; "OCR ile doğrulandı" uyarısı yalnız OCR gerçekten koştuysa yazılır.

### ÖLÇÜM (gerçek worker + gerçek kasa, debug, Apple M2)

`redaction_runs_off_the_vault_lock_and_reports_pages` (4 sayfalık fikstür, 3 koşu):
maskeleme **5,27 / 2,91 / 3,00 s** (ilki soğuk), **en uzun kasa kilidi beklemesi
15,4 / 47,5 / 40,6 ms** (eşik 150 ms), kareler birebir
`start → redact 0..4 → pack 0..1 → verify 0..4 → scan 0..1`, iş satırı `completed`,
son aşama `scan`. `redaction_cancel_stops_the_worker_and_writes_nothing`
(120 sayfa, `fixtures/synthetic/cancel-pages.pdf`): iptal **153 / 211 ms**'de döndü,
hata `E_WORKER_CANCELED`, iş `canceled`, **hiçbir belge yazılmadı**.

**Fikstür:** `fixtures/synthetic/cancel-pages.pdf` (120 sayfa, 33.144 bayt) —
`fixtures/synthetic/generate.py` üretir (`build_cancel_pdf`). Test **süreç başlatmaz**
(ADR-05); ölçüm her koşuda aynı belgeyi kullanır.

**Test:** `cargo test -p dim-ingest --lib` **67** (+1 ortam testi, 1 ignored);
tauri `--lib` **25** (+2 iş testi, 3 ignored); ignored ölçüm **2/2**; Python
`pytest workers/document/tests` **147 passed, 1 skipped** (+3 doğrulayıcı dürüstlüğü;
pymupdf 1.24.14 / pypdf 5.1.0 / pytesseract 0.3.13 — CI pinleriyle);
Web `tsc --noEmit` temiz, `vitest run` **382/382**. clippy `-p dim-ingest` ve tauri
`--all-targets -D warnings` temiz; rustfmt iki manifest temiz.

**Dürüst sınırlar:**

- Takas bilinçli: doğrulayıcı çökerse dosya **kaydedilir** ama uyarı + `ocr_verified=false`
  ile; eskiden her şeyi reddediyordu (ürün fiilen çalışmıyordu). Uyarı arayüzde
  "Doğrulama notları" altında görünür.
- OCR doğrulaması yalnız tesseract **kuruluysa** koşar (bu makinede 5.5.3; ürün
  paketinde tesseract dağıtımı ayrı iş — F01-T02).
- Kilit beklemesi örnekleyen bir yoklamayla ölçüldü (2 ms aralık), garanti değil;
  ölçüm **debug** derleme, tek makine, 4 ve 120 sayfalık kurgu belgeler.
- `TMPDIR` düzeltmesi macOS'ta ölçüldü; Windows'ta `TMP/TEMP` verilir ama **denenmedi**.
- Tauri penceresi açılmadı → aşama satırının ve iptal düğmesinin **görsel QA'si yok**.

**Commit/CI:** `7ebc21b` → CI `40583920367` **7/7 yeşil** (sözleşmeler, arayüz
tsc+vitest+build, rust macos-14 + ubuntu-latest — `cargo clippy --workspace
--all-targets -D warnings` dahil, masaüstü kabuk — tauri clippy `-D warnings` dahil,
belge worker'ı pytest+duman, SBOM). `HEAD == origin/main`; `git status`ta yalnız iki
`Cargo.lock` (başka ajanın, **dokunulmadı**).

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

## §17.3 worker kaynak bütçeleri (2026-10-27)

**Neden:** şartname §17.3 "süreç başına CPU/bellek/çıktı bütçesi; salt bir
timeout değeri bütün belgeler için kullanılmaz; bütçe aşımı **ayrı hata kodu**
taşır" diyor. Önceki durum tek bir `DEFAULT_TIMEOUT` (30 sn) idi: render,
ayrıştırma, OCR, redaksiyon ve AI çağrısı aynı sınırı paylaşıyordu, bellek/çıktı
sınırı yoktu ve aşım `E_WORKER_TIMEOUT` (transport arızası gibi, **tekrar
denenebilir**) olarak raporlanıyordu.

**Yapılan (dikey dilim):**

- **`crates/ingest/src/budget.rs` (yeni):** beş profil — `render`, `extract`,
  `ocr`, `redaction`, `ai` — × beş sınır: CPU saniyesi, bellek, yanıt/çıktı
  baytı, açık dosya, duvar saati. Profiller **birbirinin yerine geçmez** (test
  ağırlık sırasını ve AI profilinde CPU sınırının bilinçli yokluğunu tutuyor).
- **Sınır uygulama yolu:** POSIX `sh` zarfı (`ulimit -t`, `ulimit -n` +
  `exec "$0" "$@"`). Argümanlar kabuk tarafından **yorumlanmaz** (konumsal
  parametre): `$(whoami)`, `a b`, `tek'tırnak` birebir geçiyor (test). `ulimit`
  uygulanamazsa zarf `exit 126` verir → `E_WORKER_LIMIT` ("bütçeli" görünen
  sınırsız süreç yok).
- **Ayrı hata kodları:** `WorkerError::Budget` → **`E_WORKER_BUDGET`**,
  `retryable=false` (§17.4: bütçe aşımı otomatik tekrar döngüsüne girmez);
  sınır uygulanamadı → `E_WORKER_LIMIT`. Duvar saati aşımı bütçeli işte
  `E_WORKER_TIMEOUT` **değil** `E_WORKER_BUDGET(Time)`; bütçesiz istemcide eski
  davranış korunur (protokol testleri).
- **Ölçüm yolu:** çıktı sayacı okuyucu iş parçacığında (istek başına sıfırlanır,
  parçalı yanıtta parçalar toplanır); bellek gözcüsü RSS örnekler (macOS
  `ps -o rss=`, Linux `/proc/<pid>/status`; `libproc` `unsafe` ister, kullanılmaz)
  ve sınır aşılınca süreci öldürür; `SIGXCPU`(24)/`SIGXFSZ`(25) sinyal yolu da
  ihlali rapora yazar (`BudgetUsage::mark_violation` — ilk neden korunur).
- **Tauri:** `spawn_worker(kind)`; 8 çağrı yeri gerçek türle bağlandı
  (önizleme/rapor → `render`, ayrıştırma OCR açıksa `ocr` değilse `extract`,
  redaksiyon → `redaction`, envanter/toplu içe aktarma → `extract`).
  `ParseResultView.budget` iş sonrası **uygulanan** profili ve gözlenen tepe
  değerleri taşır; yeni `worker_budgets` komutu profilleri çekirdekten okur
  (arayüz kendi sabitini yazmaz).
- **Arayüz:** Ayarlar → **"Kaynak bütçeleri"** kartı (beş profil × beş sınır,
  tablo + tek satır özet), ayrıştırma sonuç satırında gerçek tepe bellek/yanıt/
  süre. `lib/budget.ts` 10 saf yardımcı + 8 vitest.
- **Bulunan ve düzeltilen üç kusur (bu tur):**
  1. **Sinyal yolundan gelen ihlal rapora yazılmıyordu** (`violated: None` iken
     hata `Cpu`) → `mark_violation`.
  2. **Kısa işte "tepe bellek 0 MiB"** raporlanıyordu: gözcü 500 ms'de bir
     uyanıyor, 433 ms'lik iş hiç örnek almıyordu. Düzeltme: gözcü **ilk örneği
     hemen** alır + `budget_report()` rapor anında son örneği alır (`note_peak`;
     süreç yaşıyorsa, `try_wait` ile toplanmış pid'e örnek alınmaz). Regresyon
     testi: `report_samples_peak_memory_even_when_the_job_ends_before_the_watchdog_wakes`
     (bellek bütçesi kapalı → gözcü koşmaz, tek örnek rapor anındadır).
  3. **Linux'ta CPU bütçesi yanlış kodla raporlanıyordu (CI yakaladı).**
     `ulimit -t N` soft **ve** hard tavanı birlikte N yapar; Linux çekirdeği bu
     durumda `SIGXCPU` yerine **doğrudan `SIGKILL`** gönderir → süreç sinyalle
     ayırt edilemez, iş `E_WORKER_DISCONNECTED` (**retryable** — §17.4 ihlali)
     olarak kapanır. macOS'ta aynı kod `SIGXCPU` verdiği için kusur yerelde
     görünmüyordu. Düzeltme: zarf önce **hard** tavanı indirir (`ulimit -H -t
     soft+5`), sonra **soft** tavanı (`ulimit -S -t N`) — `SIGXCPU` garanti,
     SIGXCPU'yu yok sayan süreç için 5 sn sonra `SIGKILL` geri çekilmesi.
     `SIGXFSZ` (25) eşlemesi **kaldırıldı** (`RLIMIT_FSIZE` uygulanmıyor; "çıktı
     bütçesi aşıldı" demek yanlış olurdu). Kanıt: aynı zarf Ubuntu 24.04
     (dash, Docker) içinde — **eski** biçim `ulimit -t 1` → çıkış **137**
     (`SIGKILL`); **yeni** biçim → çıkış **152** (`128+24`, `SIGXCPU`). Regresyon
     testleri: `cpu_hard_limit_stays_above_the_soft_limit`,
     `a_worker_killed_by_the_cpu_signal_reports_the_cpu_budget`.

**ÖLÇÜM (gerçek Python worker + PyMuPDF, `progress-pages.pdf` 4 sayfa, debug,
Apple M2, bu turda yeniden koşuldu):** süreç süresi **115,9 ms**, tepe bellek
**57 MiB**, yanıt **1.431 B**, ihlal **yok**; `extract` profili (120 sn CPU,
1024 MiB bellek, 32 MiB çıktı, 256 dosya, 300 sn) aşılmıyor → bellek tavanının
~%5,6'sı kullanılıyor. Bütçe sınırları sınama yoluyla doğrulandı: sonsuz döngü
`SIGXCPU` ile ~1 sn'de durdu, 200 MiB ayıran süreç 64 MiB sınırında
`Budget(Memory)` verdi, 200 KB yanıt 64 KiB çıktı bütçesinde reddedildi, 1 sn
duvar saati `E_WORKER_BUDGET(Time)` döndü.

**Test:** `dim-ingest` lib **65 geçti / 1 ignored** (+9 `budget`: profil
sıralaması, sınır arama, zarf metni/hard-soft sırası, sayaç/ilk-neden, sıfırlama;
+7 worker: CPU, CPU sinyal eşlemesi, bellek, çıktı, duvar saati, rapor-anı
örneklemesi), tauri **23 geçti / 1 ignored** (+2: bütçe görünümü çekirdek
raporuyla birebir, beş profil çekirdekten gelir); arayüz `budget.ts` üzerinde
**39 gerçek assert** (vitest'siz node tip-sıyırma koşucusu; `node_modules` yok).
clippy `-p dim-ingest --all-targets -D warnings` temiz, tauri clippy
`--all-targets -D warnings` temiz, rustfmt iki manifest temiz.

**Dürüst sınırlar:**

- `sh`/`ulimit` **yalnız POSIX**: Windows'ta CPU ve açık dosya sınırı
  **uygulanmaz** (süre ve çıktı bütçesi uygulanır). Windows'ta denenmedi.
- Bellek gözcüsü **aralıklı** örnekler (500 ms): süreç örnekler arasında sınırı
  aşabilir; aşım bayrağı yazılır ama "hiç aşmadı" garantisi yoktur. macOS'ta her
  örnek bir `ps` süreci başlatır (bütçe yoksa gözcü hiç koşmaz).
- **Hard CPU tavanı "en iyi çaba"**: kısıtlı ortamda `ulimit -H` indirilemezse
  yalnız soft sınır uygulanır (`SIGXCPU` yine gelir; süreç sinyali yok sayarsa
  duvar saati bütçesi devreye girer). Süreç `SIGKILL` ile ölürse neden
  **atfedilmez** — iş `E_WORKER_DISCONNECTED` olarak raporlanır (sinyalden neden
  uydurulmaz).
- **AI profili süreç değil:** CPU/dosya sınırı yok, yalnız duvar saati ve yanıt
  boyutu ölçülür; modelin bellek kullanımı bu bütçeye girmez.
- Profil değerleri **başlangıç ayarları** (ölçümle gerekçelendirilmiş ama tek
  belge/tek makine): büyük ve taranmış belgelerde tavanlar F15 ölçümüyle
  güncellenmeli.
- Bütçe aşımı işi `failed` yapar; **kısmi çıktı uygulanmaz** (worker zaten
  kasaya yazmaz) ama yarım ayrıştırma raporlanmaz — kullanıcı yeniden başlatır.
- Arayüzün **görsel QA'si yok** (Tauri penceresi açılmadı); kartın okunabilirliği
  ve tablo taşması denenmedi.

## Bilinen sınırlar

- Erişilebilirlik (klavye/ekran okuyucu) tam denetimi F15'te.
- Çoklu pencere yok; tek yazıcı kuralı korunur.
