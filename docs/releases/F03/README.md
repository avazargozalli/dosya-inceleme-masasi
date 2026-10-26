# F03 — Kasa, veritabanı ve yedek temeli

- **Sürüm etiketi:** `v0.2.0`
- **Durum:** Devam ediyor (kuyruk 2026-10-26'da gerçek oldu); açık işler aşağıda
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F03-T01 | ✅ | `crates/vault`: rastgele ana anahtar, Argon2id, sürümlü başlık |
| F03-T02 | ✅ | parola + kurtarma yuvası ayrı sarılı; `open_with_recovery` |
| F03-T04 | ✅ | `crates/storage`: 47 tablo, FK/CHECK, migrasyon checksum |
| F03-T05 | ✅ | rastgele `storage_id`, chunked AEAD blob, atomik rename |
| F03-T07 | ✅ | **2026-10-26 düzeltmesi:** `crates/jobs` yalnız *kuralları* taşıyordu; `jobs` tablosu salt-yazılıydı (hiçbir katman sahiplenmiyordu). Kuyruk artık `crates/storage/src/queue.rs`'te gerçek: idempotent ekleme, atomik sahiplenme (lease + `job_attempts`), öncelik/yaş sırası, bağımlılık kapısı, tamamlama, hata + artan bekleme, iptal isteği |
| F03-T08 | ✅ | **Açılışta kurtarma (2026-10-26):** kirası dolmuş `running` işler `retry_wait` + `E_JOB_INTERRUPTED` ile yeniden sıraya alınır, iptal istenmiş olanlar kapanır; `vault_open` bunu kendiliğinden yapar (sayı denetime yazılır) |
| F03-T10 | ✅ | `backup_to` (SQLCipher backup API), yanlış anahtarla açılmaz |
| F03-T12 | ✅ | disk/izin/yanlış parola hata kodları |

## Doğrulama

`cargo test -p dim-vault -p dim-storage` — birim testleri: anahtar sarma, başlık
kurcalama, truncation, FTS, FK, denetim zinciri, yedek.
CI: **bekliyor** (bu commit ile ilk koşu).

## İş kuyruğu (2026-10-26)

- **Neden:** `jobs` tablosu 0001'den beri vardı ama **salt yazılıyordu**.
  Her içe aktarma `parse_document` işi açıyor, hiçbir katman sahiplenmiyordu:
  iş sonsuza kadar `queued` kalıyor, yeniden başlatmada yarım işler
  görünmüyordu. F03-T07 "✅" işaretliydi ama kanıt yalnız kural testleriydi.
- **Şema (migration 0020):** `jobs.priority`, `jobs.completed_at` eklendi;
  eski satırlarda `next_run_at` dolduruldu; sahiplenme indeksi
  `(state, priority DESC, created_at, next_run_at)`.
- **Çekirdek (`crates/storage/src/queue.rs`):**
  - `enqueue_job_with_priority` (idempotent; `enqueue_job` bunu çağırır),
  - `claim_jobs` — tek işlemde: durum/bağımlılık kapısı, `running` + kira +
    `attempt_count+1`, `job_attempts` satırı. Durum başına tek eşitlik +
    `INDEXED BY idx_jobs_claim` (plan testi geçici B-tree kurulmadığını tutar).
  - `claim_job_by_key` — belge ayrıştırma yolunun tekil sahiplenmesi.
  - `complete_job` / `fail_job` — kira sahibi ve süresi denetlenir; hata
    tekrar sınıfına göre `retry_wait` (artan bekleme, `dim-jobs::RetryPolicy`)
    ya da `failed`. İptal isteği varsa sonuç `cancelled` olur, sessizce yutulmaz.
  - `cancel_job` — sırada bekleyen hemen iptal, yürüyen işe işaret.
  - `recover_stale_jobs` — F03-T08 kurtarma.
  - `job_queue_summary` / `list_jobs` — arayüz için sayı ve son 20 iş
    (dosya adı/metin taşımaz).
- **Tauri:** `document_parse` işi **gerçekten** sahiplenir (hata → `failed` /
  `retry_wait`; yalnız `E_WORKER_*`, `E_IO`, `E_DISK_FULL` tekrar denenir),
  `vault_open` kurtarmayı çalıştırır, yeni `job_queue_status` ve
  `job_queue_recover` komutları eklendi. Arayüz: Ayarlar → **İş kuyruğu** kartı
  (özet, dikkat satırı, son işler, "Bekleyen işleri kurtar").
- **Kanıt (ölçüm):** içe aktarma → iş kaydı → sahiplenme → bitirme uçtan uca
  (`crates/ingest/tests/queue_handoff.rs`); ikinci içe aktarma **yeni iş
  açmaz**. Kuyruk testleri **13** (durum geçişi, öncelik, bağımlılık, kira,
  iptal, kurtarma, tekrar bütçesi, plan). Gecikme ölçümü:
  `docs/releases/F15/BENCHMARK.md`.
- **Dürüst sınırlar:** `jobs.state` CHECK kümesi şartnamenin bütün durumlarını
  taşımıyor (`blocked`, `validating`, `canceling`, `interrupted`, `partial`,
  `completed` yok); `interrupted`/`canceling` **koşul** olarak taşınır
  (`error_code='E_JOB_INTERRUPTED'`, `cancel_requested_at`). Yeni durum eklemek
  tablo yeniden yazımı ister (CHECK), bu turda yapılmadı. Kuyruk yürütücüsü
  seridir (kasa başına tek yazıcı); paralel belge worker'ları (şartname §17.3
  "en fazla iki") uygulanmadı. Kuyruk gecikmesi **çekirdek** ölçümdür
  (Python worker/IPC yok; sentetik iş 2 ms). 100k belge profilinde kuyruk
  yeniden ölçülmedi.

## Bilinen sınırlar

- Kasa başlığı yedekleme paketi F14'te; şu an DB yedeği alınır.
- `blobs` GC (referanssız artık temizliği) F14'te.
- Windows'ta SQLCipher derlemesi uzun; ayrı `windows-check` işinde.

## Otomatik kilit (ek)

- `src/lib/autolock.ts`: saf `shouldAutoLock(lastActivity, now, limit)`; varsayılan
  10 dakika, 30 saniyede bir kontrol.
- `App.tsx`: `pointerdown`/`keydown`/`wheel` hareketi sayacı sıfırlar; kasa açıkken
  süre dolunca `lockVault()` çağrılır ve durum yenilenir.
- Saat geri alınırsa (uyku, manuel değişiklik) güvenli tarafta kalınır: kilitlenir.
- 5 birim testi (`src/lib/autolock.test.ts`).

## Referanssız blob temizliği (ek)

- `crates/storage/src/gc.rs`: `garbage_collect_blobs(dry_run)` yalnız hiçbir
  `document_versions.blob_id` ve `pages.derivative_blob_id` tarafından
  gösterilmeyen kayıtları seçer; `document_versions` FK'si `RESTRICT` olduğu
  için yanlış silme veritabanı tarafından da engellenir.
- `blob_inventory()` toplam/referanssız/boyut özeti verir.
- `vault_gc` Tauri komutu kuru çalıştırma ve gerçek silme sunar; dosya
  silinemezse `missingFiles` ile dürüstçe raporlar.
- Ayarlar'da iki adımlı akış: **Tara** → **Temizle**.
- 3 storage testi (referanslı blob korunur, boş kasa no-op, türev yolu).

## Parola değiştirme (ek)

- `VaultFile::change_password`: yalnız **parola yuvası** yeni parolayla yeniden
  sarılır; ana anahtar ve DB anahtarı değişmez, açık oturum ve kurtarma anahtarı
  geçerli kalır. Başlık önce geçici dosyaya yazılıp `fsync` edilir, sonra atomik
  `rename` — çökme eski başlığı bozmaz. 10 karakterden kısa parola `E_PASSWORD_WEAK`.
- **Kritik düzeltme:** `open_with_recovery` KDF'e base32 **metnini** veriyordu;
  anahtar oluştururken ise **32 ham bayt** kullanılıyordu. Kurtarma anahtarı bu
  yüzden hiçbir zaman çalışmıyordu. Artık base32 çözülüp ham baytlar veriliyor;
  regresyon testi eklendi (`crates/vault/tests/password_change.rs`, 3 test).
- Ayarlar'da parola değiştirme formu (mevcut / yeni / tekrar).

## Hâlâ eksik

- OS düzeyinde kilit (ekran kilidi) algılama.

