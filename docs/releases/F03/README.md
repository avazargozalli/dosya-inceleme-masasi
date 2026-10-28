# F03 — Kasa, veritabanı ve yedek temeli

- **Sürüm etiketi:** `v0.2.0`
- **Durum:** Devam ediyor (kuyruk 2026-10-26'da gerçek oldu); açık işler aşağıda
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F03-T01 | ✅ | `crates/vault`: rastgele ana anahtar, Argon2id, sürümlü başlık |
| F03-T02 | ✅ | parola + kurtarma yuvası ayrı sarılı; `open_with_recovery` |
| F03-T04 | ✅ | `crates/storage`: 54 tablo tanımı (şema v23), FK/CHECK, migrasyon checksum |
| F03-T05 | ✅ | rastgele `storage_id`, chunked AEAD blob, atomik rename |
| F03-T07 | ✅ | **2026-10-26 düzeltmesi:** `crates/jobs` yalnız *kuralları* taşıyordu; `jobs` tablosu salt-yazılıydı. Kuyruk artık `crates/storage/src/queue.rs`'te gerçek: idempotent ekleme, atomik sahiplenme (lease + `job_attempts`), öncelik/yaş sırası, bağımlılık kapısı, doğrulama adımı, tamamlama/kısmi, hata + artan bekleme, engel, iptal, kullanıcı yeniden denemesi. **§17.2 durum makinesi tam** (migration 0021) |
| F03-T08 | ✅ | **Açılışta kurtarma (2026-10-26):** §17.6 iki adım — süresi dolmuş `running/validating/canceling` önce `interrupted`, sonra `queued` (bütçe varsa) / `failed` (bütçe bittiyse); iptal istenmiş iş `canceled`. `vault_open` kendiliğinden yapar (sayı denetime yazılır) |
| F03-T10 | ✅ | `backup_to` (SQLCipher backup API), yanlış anahtarla açılmaz |
| F03-T12 | ✅ | disk/izin/yanlış parola hata kodları |

## Doğrulama

`cargo test -p dim-vault -p dim-storage` — birim testleri: anahtar sarma, başlık
kurcalama, truncation, FTS, FK, denetim zinciri, yedek, iş kuyruğu (durum
makinesi, engel, bağımlılık, doğrulama/kısmi, kurtarma).
CI: `7640c9d` + `4f013a7` → run **`40381503652` 7/7 yeşil** (sözleşmeler,
arayüz tsc+vitest+build, rust macos-14 + ubuntu-latest, masaüstü kabuk, belge
worker'ı, SBOM). Önceki run `40381133165` aynı eşzamanlılık grubunda **iptal**
edildi (yeni push), kırmızı değildi.

## İş kuyruğu (2026-10-26)

- **Neden:** `jobs` tablosu 0001'den beri vardı ama **salt yazılıyordu**.
  Her içe aktarma `parse_document` işi açıyor, hiçbir katman sahiplenmiyordu:
  iş sonsuza kadar `queued` kalıyor, yeniden başlatmada yarım işler
  görünmüyordu. F03-T07 "✅" işaretliydi ama kanıt yalnız kural testleriydi.
- **Şema (migration 0020):** `jobs.priority`, `jobs.completed_at` eklendi;
  eski satırlarda `next_run_at` dolduruldu; sahiplenme indeksi
  `(state, priority DESC, created_at, next_run_at)`.
- **Canlı sayaç (migration 0022, 2026-10-27):** `jobs.progress_stage`,
  `progress_done`, `progress_total` — `update_job_progress` yalnız **kira sahibi**
  ve `running|validating` iş için yazar; tutarsız sayaç `E_JOB_PROGRESS`,
  terminal işe geç yazım `Ok(None)` (uygulanmaz). Sahiplenme/yeniden deneme
  sayacı sıfırlar. Ayrıntı ve ölçüm: `docs/releases/F04/README.md`.
- **Çekirdek (`crates/storage/src/queue.rs`):**
  - `enqueue_job_with_priority` (idempotent; `enqueue_job` bunu çağırır),
  - `claim_jobs` — tek işlemde: durum/bağımlılık kapısı, `running` + kira +
    `attempt_count+1`, `job_attempts` satırı. Durum başına tek eşitlik +
    `INDEXED BY idx_jobs_claim` (plan testi geçici B-tree kurulmadığını tutar).
  - `claim_job_by_key` — belge ayrıştırma yolunun tekil sahiplenmesi.
  - `begin_validation` / `complete_job` / `complete_job_partial` / `fail_job` —
    `running → validating → completed|partial`, hata sınıfına göre
    `retry_wait` (artan bekleme) ya da `failed`; iptal isteği her sonucun
    önüne geçer (`→ canceling → canceled`).
  - `block_job` / `unblock_job` — `running → blocked` (neden `blocked_reason`a
    yazılır) ve `blocked → queued`.
  - `cancel_job` — sırada bekleyen/engelli hemen `canceled`, yürüyen/doğrulanan
    `canceling`.
  - `retry_failed_job` — `failed → queued`, deneme bütçesi sıfırlanır,
    `job_attempts` numaralandırması `MAX(attempt_no)+1` ile sürer.
  - `recover_stale_jobs` — §17.6 iki adım (kurtarma raporu: `interrupted`,
    `requeued`, `failed`, `canceled`).
  - `job_queue_summary` / `list_jobs` — arayüz için sayı ve son 20 iş
    (dosya adı/metin taşımaz).
- **Durum makinesi tek kaynakta:** `crates/job_state_machine.tsv` (§17.2'nin
  birebir tablosu, 11 durum). `crates/jobs` (kural crate'i) ve
  `crates/storage/src/job_state.rs` **aynı dosyayı** `include_str!` ile okur;
  iki kopya tablo yok, ayrışma testle değil yapısal olarak imkânsız. Her geçiş
  SQL'den önce `job_state::allowed` ile denetlenir (`E_JOB_STATE`).
- **Tauri:** `document_parse` işi **gerçekten** sahiplenir; başarıda
  `begin_validation` → uyarısız `completed`, uyarılı `partial`; parola
  korumalı belge **hata değil engel** (`running → blocked`, neden `password`);
  yalnız `E_WORKER_*`, `E_IO`, `E_DISK_FULL` tekrar denenir. `vault_open`
  kurtarmayı çalıştırır. Komutlar: `job_queue_status`, `job_queue_recover`,
  `job_cancel`, `job_retry`, `job_unblock`. Arayüz: Ayarlar → **İş kuyruğu**
  kartı (özet, dikkat satırı, son işler, satır başına gerçek eylem düğmeleri,
  "Bekleyen işleri kurtar").
- **Kanıt (ölçüm):** içe aktarma → iş kaydı → sahiplenme → bitirme uçtan uca
  (`crates/ingest/tests/queue_handoff.rs`); ikinci içe aktarma **yeni iş
  açmaz**. Kuyruk testleri **21** (durum geçişi, öncelik, engel, bağımlılık,
  kira, doğrulama/kısmi, iptal, kurtarma, kullanıcı denemesi, tekrar bütçesi,
  plan, şema CHECK uyumu). Gecikme ölçümü: `docs/releases/F15/BENCHMARK.md`.
- **Migrasyon tuzağı (ölçüldü):** `jobs` tablosunu yeniden yazmak (CHECK
  değişmez) `foreign_keys` AÇIKken `job_attempts` satırlarını `ON DELETE
  CASCADE` ile **siliyordu** (1 → 0). Bu yüzden `Database::migrate` artık
  migrasyonları `PRAGMA foreign_keys=OFF` ile koşar ve her migrasyondan sonra
  `PRAGMA foreign_key_check` ile denetler; ihlal varsa migrasyon geri alınır
  (`E_DB_MIGRATION_FK`). Kanıt:
  `upgrade_tests::job_state_rewrite_keeps_attempts_and_remaps_legacy_states`.
- **Dürüst sınırlar:** Kuyruk yürütücüsü seridir (kasa başına tek yazıcı);
  paralel belge worker'ları (şartname §17.3 "en fazla iki") **F04'te** uygulandı
  (`parse_pool.rs`, ölçüm **1,71×**) ve worker kaynak bütçeleri (CPU/bellek/
  çıktı/dosya/süre profilleri, `E_WORKER_BUDGET`) **F04/§17.3** turunda eklendi.
  `completed`/`canceled` durumundaki iş **yeniden açılamaz** (kullanıcı yeni
  bir ayrıştırma başlatır; yeni satır açılır). `interrupted → queued` "yeniden
  doğrulama" adımı bugün girdi parmak izinin değişmediği varsayımıyla yapılır
  (yerel deterministik iş); uzak (AI) işlerinde otomatik yeniden gönderim yok.
  `partial` bu satır için terminaldir; şartnamenin "yeni alt iş" yolu ayrı bir
  satırdır. Engel nedeni `password`/`dependency` dışında bugün üretilmiyor
  (kota/izin bekleyen iş yolu yok). Kuyruk gecikmesi **çekirdek** ölçümdür
  (Python worker/IPC yok; sentetik iş 2 ms); 100k belge profilinde yeniden
  ölçülmedi. Tauri penceresi açılmadı → kartın görsel QA'sı yok.

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

