# F03 — Kasa, veritabanı ve yedek temeli

- **Sürüm etiketi:** `v0.2.0`
- **Durum:** Devam ediyor (kuyruk 2026-10-26'da gerçek oldu); açık işler aşağıda
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F03-T01 | ✅ | `crates/vault`: rastgele ana anahtar, Argon2id, sürümlü başlık |
| F03-T02 | ✅ | parola + kurtarma yuvası ayrı sarılı; `open_with_recovery` |
| F03-T04 | ✅ | `crates/storage`: 54 tablo tanımı (şema v24), FK/CHECK, migrasyon checksum |
| F03-T05 | ✅ | rastgele `storage_id`, chunked AEAD blob, atomik rename |
| F03-T07 | ✅ | **2026-10-26 düzeltmesi:** `crates/jobs` yalnız *kuralları* taşıyordu; `jobs` tablosu salt-yazılıydı. Kuyruk artık `crates/storage/src/queue.rs`'te gerçek: idempotent ekleme, atomik sahiplenme (lease + `job_attempts`), öncelik/yaş sırası, bağımlılık kapısı, doğrulama adımı, tamamlama/kısmi, hata + artan bekleme, engel, iptal, kullanıcı yeniden denemesi. **§17.2 durum makinesi tam** (migration 0021) |
| F03-T08 | ✅ | **Açılışta kurtarma (2026-10-26, süreç öldürme 2026-10-29):** §17.6 iki adım — kesilen `running/validating/canceling` önce `interrupted`, sonra `queued` (bütçe varsa) / `failed` (bütçe bittiyse); iptal istenmiş iş `canceled`. `vault_open` **kiraya bakmadan** yapar (`recover_orphaned_jobs`; çökme sonrası kira hâlâ gelecekte olabilir — ölçüldü), orta oturumdaki "Bekleyen işleri kurtar" kiraya saygı duyar. Kurtarılan iş `queued` kalır; **"Başlat"** (`job_run`) gerçekten çalıştırır |
| F03-T10 | ✅ | `backup_to` (SQLCipher backup API), yanlış anahtarla açılmaz |
| F03-T12 | ✅ | disk/izin/yanlış parola hata kodları; **2026-11-07:** teşhis kartı sürümü **çekirdekten** okur (`system_info`; sürüm/protokol/platform, ayrışma uyarısı) |

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
  - `recover_stale_jobs` — §17.6 iki adım, **orta oturum**: yalnız kirası
    dolmuş satırlar (kurtarma raporu: `interrupted`, `requeued`, `failed`,
    `canceled`). Kiraya saygı duyar: süresi dolmamış iş canlı bir işçiye ait
    olabilir.
  - `recover_orphaned_jobs` — **kasa açılışında**: kiraya **bakmaz**, çünkü o
    anda bu süreçte hiçbir işçi yoktur. Çökme/SIGKILL sonrası hemen yeniden
    açılışta kira hâlâ gelecekte (300 sn) olur ve `recover_stale_jobs` işi hiç
    kesmezdi → satır sonsuza kadar "Yürüyor" kalırdı (ölçüldü, F15-T06).
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
  **açılış kurtarmasını** (`recover_orphaned_jobs`) çalıştırır. Komutlar:
  `job_queue_status`, `job_queue_recover` (orta oturum), `job_cancel`,
  `job_retry`, **`job_run`** (sırada bekleyen ayrıştırmayı gerçekten başlatır),
  `job_unblock`. Arayüz: Ayarlar → **İş kuyruğu** kartı (özet, dikkat satırı,
  son işler, satır başına gerçek eylem düğmeleri, "Bekleyen işleri kurtar");
  `queued` ayrıştırma satırında **"Başlat"**.
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
- `vault_gc` Tauri komutu kuru çalıştırma ve gerçek silme sunar.
- Ayarlar'da iki adımlı akış: **Tara** → **Temizle**.
- 5 storage testi (referanslı blob korunur, boş kasa no-op, türev yolu,
  kimlikle silme referanslıyı korur, aday/envanter birebir).

### Sıra düzeltmesi + disk tarafı (2026-11-12)

**Ölçülen kusur (iki parçalı):** (1) `vault_gc` önce **kaydı** siliyor, sonra
dosyayı siliyordu. Dosya silinemezse (izin, kilit, süreç ölümü) geriye
veritabanında **hiçbir izi olmayan** bir dosya kalıyordu: `blobs` tablosundan
türeyen hiçbir envanter onu göremez, ürün onu bir daha silemez. (2)
`VaultFile::write_blob` geçici dosyaya yazıp `rename` ediyor, **hata hâlinde
temizlemiyordu** → yarım `.blb.tmp` diskte kalıyordu (aynı sınıf: kaydı yok,
görünmez). Projenin kendi `dim_storage::atomic` kuralı "hata olursa geçici
dosya silinir" derken kasa yazımı bu kuralın dışında kalmıştı.

**Yazılan:**

- `gc.rs`: `orphan_blob_candidates()` (yalnız seçim) + `delete_unreferenced_blobs(ids)`
  (koşullu: iki adım arasında referans doğmuşsa satır **korunur**; tek işlem,
  denetim yalnız sayı). `garbage_collect_blobs` ikisinin sarmalayıcısı; SQL
  koşulu (`UNREFERENCED`) **tek kaynak** — aday, silme ve envanter aynı ifadeyi
  kullanır.
- `vault_gc`: **önce dosya, sonra kayıt.** Dosyası silinemeyen kayıt korunur ve
  `blocked` alanında `ad: neden` olarak raporlanır (sonraki deneme yine aday).
  Diskte dosyası olmayan kayıt `missingFiles` ile bildirilir ve kaydı düşer
  (diskte veri yok; tutmak hiçbir şeyi kurtarmaz — ama sessizce geçilmez).
- `VaultFile::write_blob_with_id` + `write_tmp_then_rename`: hata hâlinde geçici
  dosya silinir; parola değişikliğinin başlık yazımı da aynı yardımcıyı kullanır.
- Yeni `crates/storage/src/blob_disk.rs`: `blobs/` klasörünün **disk tarafı**
  envanteri — `scan_blob_dir()` ve `sweep_blob_orphans()`. Kapalı küme ad biçimi
  (`<32 küçük hex>.blb`, `.<32 küçük hex>.blb.tmp`), symlink/dizin **dokunulmaz**,
  **yaş eşiği** (varsayılan 600 sn) uçuştaki içe aktarmayı korur, silme anında
  kayıt yeniden sorulur, liste kırpılırsa `truncated` yazılır, denetim yalnız sayı.
- Yeni komut `vault_blob_disk_sweep(dry_run)`; Ayarlar → **Kullanılmayan veriler**
  kartında "Diskteki artıkları tara" → onay paneli → "Onayla ve sil". Yaş eşiği
  ve liste sınırı arayüze **çekirdekten** gelir (kopya sayı yok).
- Testler: `blob_disk` **9**, `gc` **3 → 5**, vault **+3**, masaüstü kapısı
  **46 → 51** (kaynak 56; 5 gerçek worker testi `#[ignore]`); `Settings.test.tsx`
  **57 → 60**.

**Dürüst sınırlar:** (a) disk taraması **yalnız** kasanın `blobs/` klasörünü
tarar; başka klasörlere kopyalanmış blob dosyaları kapsam dışı. (b) Yaş eşiği
sabittir (600 sn), arayüzden değiştirilemez; saat geri alınırsa dosya "yeni"
sayılıp **korunur** (temkinli taraf). (c) Silme **yalnız kullanıcı eylemiyle**
koşar; otomatik süpürme yok. (d) `missing` kayıtlar **onarılmaz** — yalnız
bildirilir; kurtarma yedeğe döner. (e) Komut yolu (`vault_gc`,
`vault_blob_disk_sweep`) `State<AppState>` aldığı için **komut olarak
koşulmadı**; kablolama `cargo check`/`clippy`, davranış `gc_vault_blobs` ve
`blob_disk_view` yardımcılarıyla gerçek dosya sistemi + bellek içi kasada
ölçüldü. (f) Gerçek Tauri penceresi + görsel QA **insan bekliyor**.

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

## Kasa kapısı (arayüz) — F03-T02'nin kullanıcıya görünen yüzü (2026-11-02)

`apps/desktop/src/screens/Unlock.tsx` kasa oluşturma, mevcut kasayı açma ve
kurtarma anahtarıyla açmayı yönetir. Bu turda ekranın **davranış testi** yazıldı
(`src/screens/Unlock.test.tsx`, 14 test + `lock-behavior.test.tsx`te 1 App
düzeyi; ayrıntı ve kusur listesi: `docs/releases/F04/README.md`).

- **Kurtarma anahtarı artık `App`te tutulur** (`pendingKey`). Anahtar bir kez
  gösterilir; `Unlock` ekranı yan menüde gezinirken yeniden bağlandığı için
  bileşen içinde tutulsa ilk ekran değişiminde **siliniyordu** (ölçüldü).
  `refresh` başarı döndürür: durum okunamazsa anahtar ekranı açık kalır.
- **"Geri" iki adımlıdır**: "Kurtarma anahtarı bir daha gösterilmez…" uyarısı +
  Vazgeç / Kaydetmeden devam et. Tek tıkla anahtarı düşürmek sessiz kayıptı.
- **İstek sürerken** kip düğmeleri ve alanlar kilitlidir; kip değişimi önceki
  kipin hatasını siler; seçili kip `aria-pressed` ile bildirilir.
- **Ölçüm:** `tsc --noEmit` temiz · `vitest` **771/771** (54 dosya) ·
  `pnpm build` temiz · CI **`41523031374` 7/7 YEŞİL**.
- **Dürüst sınırlar:** jsdom tarayıcı değil — gerçek Tauri penceresi, Argon2id
  süresi, dosya sistemi ve görsel QA yok; `api` mock'ludur. Kasa oluşturulduktan
  sonra uygulama durumu "kilitli" kalır (yan menü "Kasa kilitli" gösterir) ve
  "Kasayı aç" ile tazelenir — kullanıcı anahtarı onaylamadan çalışma alanına
  geçmez (kabul edilen akış, kayıtlı).

## Denetim zinciri okunur ve doğrulanır (2026-11-07)

**ÖLÇÜLEN KUSUR (iki parça):** `audit_events` her önemli işlemde yazılıyordu
(`append_audit`; 40 kategori / 40+ çağrı yeri — kasa açılışı kurtarması, belge
içe aktarma, iş kuyruğu, dışa aktarma, maskeleme, aktarım onayı/makbuzu, emsal,
CLI sondaları, GC…), ama:

1. **Okuma yolu hiç yoktu** — `crates/storage` içinde `list_audit_events`
   bulunmuyordu; Tauri komutu, `api.ts` bağı ve ekran kartı da yoktu
   (`git grep`: tek tüketici yok). Avukat "bu kasada hangi işlemler yapıldı"
   sorusunu yanıtlayamıyordu; şartname tablosu bu tabloyu **"yerel değişiklik
   geçmişi"** olarak tanımlar (`docs/spec/...`, `audit_events` satırı).
2. **Bütünlük doğrulaması üründe hiç koşmuyordu** — `verify_audit_chain`
   yalnız **testlerden** çağrılıyordu (8 çağrı yeri: 8'i de test). Hash zinciri
   kurcalamayı yakalayacak biçimde yazılmıştı ama çalışan uygulamada bu kapı
   hiç kapanmıyordu; kayıt bozulsa/elle değiştirilse kullanıcı bunu göremezdi.

**Yazılan (çekirdek):** `crates/storage/src/audit.rs` (yeni)

- `list_audit_events(limit, category)` — **en yeni önce** (`sequence DESC`);
  sınır **çekirdekte** kırpılır (`1..=200`, `0` → `1`, taşan → `200`). Sıra
  `timestamp`e göre değildir: aynı damgada yazılan kayıtlar bile yazılma sırasını
  korur. `category` verilirse yalnız o kategori.
- `AuditEventRow` — `sequence, id, category, entityId, operation, actorLabel,
  metadata, priorHash, eventHash, timestamp`. `metadata_allowlist` çözülemezse
  satır **düşmez**, `metadata: null` döner (bozukluk görünür, kayıt kaybolmaz).
- `audit_chain_status()` — `{ checked, intact, brokenEventId, brokenReason }`.
  `brokenReason` **kararlı kod**: `prior_hash` (önceki kayda bağ kopuk) ya da
  `event_hash` (kaydın kendi özeti tutmuyor). `checked` yalnız **doğrulanmış**
  kayıt sayısıdır (kopukluktan önceki).
- **Tek yürüyüş:** hash kuralı `audit::walk_chain` içinde; `verify_audit_chain`
  (eski hata-döndüren sözleşme, mesajları birebir korunur) ve
  `audit_chain_status` (yeni durum sözleşmesi) aynı yürüyüşü paylaşır — iki
  kopya zamanla ayrışamaz.

**Yazılan (kabuk + arayüz):** yeni `audit_event_list` / `audit_chain_check`
komutları (`lib.rs` kaydı), `api.auditEventList` / `api.auditChainCheck` +
`AuditEventRow`/`AuditChainStatus` tipleri; **Ayarlar → "Denetim zinciri"** kartı:

- Kasa açılınca son **50** kayıt okunur (düğmeyle tazelenir); kilitliyken
  **okuma yapılmaz**, doğrulama düğmesi kapalıdır ve kart nedenini yazar.
- Tablo: zaman · kategori · işlem · aktör · kayıt (kimlik kısaltılır, tam değer
  `title`da) · kayıt özeti (10 karakter + `…`, tam özet `title`da). Kategori ve
  işlem **çekirdek kodlarıdır**; bilinmeyen değer gizlenmez (40 kategori için
  uydurma çeviri yok — not satırı bunu söyler).
- "Zinciri doğrula": tutarlıysa **yeşil** "N kayıt baştan sona doğrulandı";
  kopuksa **kırmızı** (`role=alert`) "İlk N kayıt tutarlı; sonrasında kopukluk
  var: `<kayıt>`" + neden (`prior_hash` → "aradan kayıt çıkmış olabilir",
  `event_hash` → "kayıt değiştirilmiş"; **bilinmeyen neden ham değeriyle**).
- Düşen okuma **kendi satırında** (kod + mesaj); liste istekten **önce**
  temizlenir (eski satırlar "güncel" gibi kalmaz); hata hâlinde liste `null` →
  ekran **"kayıt yok" demez**. Doğrulama sonucu istek anında düşer, geç yanıt
  yenisini ezmez (`auditSeq`/`auditCheckSeq`).
- 23 i18n anahtarı (`settings.audit*`); gömülü metin yok
  (`localization.test.ts` yeşil).

**Testler:** storage `audit::tests` **7** (yeni dosya) → `dim-storage --lib`
**415 → 422**. Arayüz `Settings.test.tsx` **25 → 33** (+8) → vitest
**1201 → 1210** (60 dosya; +8'i bu tur, +1'i başka ajanın commit edilmemiş
`format.test.ts` değişikliği).

**Kapı kanıtı (gerçek koşular, betikler `scratch/audit-{ui,core}-gates.py`):**

- **Düzeltme öncesi** HEAD bileşeni + yeni 8 arayüz testi: **`8 failed | 25 passed`**
  (sekizi de kırmızı — kart hiç yok).
- **Arayüz 9/9 kırmızı:** U1 kasa açılınca okuma (1), U2 tazeleme temizliği (1),
  U3 düşen okuma sessiz (1), U4 tutarsız sonuç "tutarlı" yazar (1), U5 bilinmeyen
  neden gizlenir (1), U6 doğrulama hatası yutulur (1), U7 önceki "tutarlı" satırı
  istekte kalır (1), U8 kilitliyken doğrulama açık (1), U9 **çiftli** (hata varken
  "kayıt yok": iki kapı birlikte kaldırılınca, 1). Geri yükleme sha256 birebir
  (`8d83a8f8…4dcb`), son koşu **33/33**.
- **Çekirdek 7/7 kırmızı:** C1 sıra ters, C2 kategori filtresi yok sayılır,
  C3 bozuk metadata satırı düşürülür, C4 kopukluk "tutarlı" bildirilir, C5 neden
  sabitlenir, C6 kırılan kayıt "doğrulandı" sayılır, C7 bağ kopukluğu denetlenmez.
  Geri yükleme sha256 birebir (`27b4d60d…2cd7`), son koşu yeşil.
- **Dürüst (kendi hatam):** U9'u önce iki kapıdan **birini** kaldırarak ölçtüm —
  test **yeşil kaldı**: `auditError` görünürken boş-liste satırı zaten
  `!auditError` kapısıyla, hata hâlinde liste de `null` ile korunuyor; iki kapı
  birbirinin yedeği. Ölçüm **çiftli** mutasyona çevrildi ve kırmızı alındı; iki
  kapı da bırakıldı (biri bozuk veri, diğeri görünüm sözleşmesi).
- **İlk betik hatası (dürüst):** kapı betiği `npx`i daraltılmış `PATH`te arayıp
  düştü ve bileşen dosyasını HEAD hâlinde bıraktı; dosya yedekten geri yazıldı
  (sha256 aynı), betiğe `try/finally` + tam `PATH` eklendi. O koşu kanıt sayılmadı.

**Ölçüm (yerel, Node 22.23.1 + TZ=UTC):** `dim-storage --lib` **422/422**
(260,63 s) · clippy `-p dim-storage --all-targets -D warnings` temiz · masaüstü
clippy temiz · `cargo check --all-targets` temiz · `cargo fmt` iki manifest temiz ·
masaüstü kabuk kapısı **34 koştu · 0 düştü · 5 yoksayıldı** (kaynak 39) ·
`tsc --noEmit` temiz · vitest **1210/1210** (60 dosya) · `npm run build`
**619,48 kB** (`index-B98Cx2XO.js`, gzip **166,26 kB**; CSS 7,35 kB).

**CI (ölçüldü):** kod `84f0b74` (8 dosya, +593/−73) → `42326849144` **KIRMIZI**:
`arayüz` işi `yerelleştirme kapısı > sözlükte kullanılmayan anahtar kalmaz`
testinde düştü (`cases.parseStatusSucceeded/Partial/Failed` — **benim hatam**:
`i18n.ts`i eklerken **başka ajanın** commit edilmemiş 4 anahtarını da sahneye
aldım; onların tüketicisi `lib/format.ts`/`Cases.tsx` henüz commit edilmemişti).
Düzeltme `e8a752d` yalnız CI'nın işaret ettiği **3** anahtarı HEAD'den çıkardı
(dördüncü `cases.partialParse` kapıdan geçtiği için kaldı; çalışma ağacındaki
diğer ajan değişiklikleri korunarak) → **`42327661340` 10/10 YEŞİL**:
`dim-storage` **422** hem `macos-15` (189,62 s) hem `ubuntu-latest` (268,83 s) —
birebir; `arayüz` **60 dosya / 1209 test**, paket `index-lHY2P-yg.js`
**618,83 kB** / gzip **166,00 kB**; `masaüstü kabuk` **34 koştu · 0 düştü ·
5 yoksayıldı** + yoksayılan koşu **5 passed** (50,41 s). **Dürüst:** yerel paket
**619,48 kB** çıktı çünkü çalışma ağacımda **başka ajanın** commit edilmemiş
`format.ts`/`Cases.tsx`/`app.css` değişiklikleri vardı; CI (yalnız commit'lenen
kod) **618,83 kB**. Yerel/CI test sayısı birebir değil çünkü aynı sebeple yerelde
**1210** (bir test başka ajanın `format.test.ts`inden), CI'da **1209**.

**Dürüst sınırlar:** (a) iki komut **komut olarak koşulmadı** (`State<AppState>`
kuran test altyapısı yok) — kablolama `cargo check` + clippy, davranış storage
düzeyinde **gerçek SQLite** ile; (b) defter **salt-okunur** (arayüzden silinemez/
düzenlenemez), filtre yalnız kategori (tarih/aktör filtresi yok), sayfalama yok
(en yeni 200 sınırı); (c) doğrulama **çalışma anında** yapılır — arka planda
otomatik/kendiliğinden doğrulama yok, kullanıcı düğmeye basar; (d) kopukluk
**onarılmaz**, yalnız raporlanır (onarım ürün kararı); (e) `sequence` AUTOINCREMENT
sırasıdır, silinen kayıt boşluğu zincirde **bağ kopukluğu** olarak görünür
(beklenen davranış — ölçüldü); (f) jsdom tarayıcı değil → gerçek Tauri penceresi
ve **görsel QA insan bekliyor**; (g) ~~`system_info` komutu hâlâ arayüzden
kullanılmıyor~~ — **2026-11-07'de kapandı** (Ayarlar → Teşhis kartı çekirdek
sürümünü okur); (h) şartname bu tabloyu **"resmî zaman damgası veya
değiştirilemez delil iddiası taşımayan yerel değişiklik geçmişi"** olarak
tanımlar — kart bunu **yazar**; hash zinciri yalnız bütünlük bozukluğunu
gösterir, hukukî delil iddiası üretmez.

## Teşhis kartı sürümü çekirdekten okur (2026-11-07)

**Ölçülen kusur (ölü yüzey):** `system_info` komutu `generate_handler!`da kayıtlıydı
ama **hiçbir yerden çağrılmıyordu** (`git grep system_info -- apps/desktop/src` boştu).
Ayarlar'daki "Teşhis" kartı yalnız sabit metindi; ekranda görünen sürüm
`package.json`dan geliyordu — yani **paketlenen arayüzün** sürümü. Çalışan ikilinin
sürümü hiç okunmadığı için, ikili ile arayüz ayrışan bir kurulumda (elle kopyalama,
yanlış paket, geri alınmış güncelleme) ekran farkında olmadan **yanlış sürüm**
yazıyordu ve bunu gösterecek bir satır yoktu. `protocol_version` da hiçbir yerde
görünmüyordu.

**Yazılan:** `DiagnosticsCard` (Ayarlar → Teşhis) + `api.systemInfo` + `SystemInfo`:

- Çekirdek sürümü (`env!("CARGO_PKG_VERSION")`), arayüz sürümü (`package.json`),
  protokol sürümü ve platform. İkisi ayrışırsa **uyarı** satırı basılır; protokol
  uyumsuzsa ayrı uyarı. Ayrışma gizlenmez.
- Okuma düşerse sürüm **uydurulmaz**: satır "okunamadı" yazar, hata kodu ve ileti
  görünür. Sözleşme dışı gövde (eksik alan/yanlış tip/boş sürüm) `parseSystemInfo`
  ile reddedilir (`E_SYSTEM_INFO`) — eksik yanıtı sürüm saymak da uydurmadır.
- Geç yanıt kapısı (`seqRef`): `main.tsx` StrictMode ile açıldığı için effect iki kez
  koşar; yavaş dönen eski yanıt (ya da **geç hata**) yeni değeri ezmez.
- Kart **kasa durumuna bağlı değildir** — `system_info` kasa açmadan yanıt verir ve
  "hangi sürümü kurmuşum" sorusu tam olarak kasa açılamadığında sorulur.
- Platform kodları marka adıyla (`macos` → macOS), **tanınmayan kod ham** yazılır.
- 14 i18n anahtarı; gizlilik sınırı ekranda yazılı: yalnız sürüm/protokol/platform —
  dosya yolu, kasa kimliği, kullanıcı bilgisi yok (REQ-033).
- `lib/diagnostics.ts` + **kaynak kapısı** (`lib/diagnostics.test.ts`): çekirdek
  sürümü crate üstverisinden okunmalı, komut `generate_handler!`da kayıtlı olmalı,
  `api` bağı bulunmalı.

**Kanıt (gerçek koşular):**

- **Düzeltme öncesi:** HEAD `Settings.tsx` + yeni testler → **`1 failed | 33 passed`**
  (düşen test: "sürüm çekirdekten okunur…"); geri yükleme sha256 birebir
  (`5635ea26…f8e0`).
- **Mutasyon 11/11 kırmızı:** M1 geç yanıt kapısı, M2 sürüm ayrışma uyarısı, M3 hata
  satırı, M4 sözleşme dışı künye kapısı, M5 bilinmeyen platform ham, M6 protokol
  uyumsuzluğu, M7 "Yenile" yeniden okuma, M8 çekirdek sürümü kaynağı, M9 geç hata
  kapısı, M10 çekirdek sürümü gömülü sabite çevrildi (kaynak kapısı), M11 komut kaydı
  silindi (kaynak kapısı).
- **Dürüst (kendi ölçüm hatam):** M1 ve M9 ilk koşuda **yeşil kaldı** — iki test de
  geç yanıtı **beklemiyordu** (`waitFor` eski değer yazılmadan geçiyordu). Testlere
  açık bekleme eklendi, mutasyon yeniden ölçüldü (`1 failed`); kapı ölçülmeden
  bırakılmadı.
- Yerel: `tsc --noEmit` temiz · vitest **62 dosya / 1228 test** · `npm run build`
  **622,72 kB** (`index-CwDOuVA5.js`, gzip **167,08 kB**).
- **CI:** `42334337774` **10/10 YEŞİL**; `arayüz` **62 dosya / 1227 test**, paket
  `index-G68jn6fB.js` **622,07 kB** / gzip **166,80 kB**. Yerel 1228 ile aradaki
  **1 test farkı** ağaçta duran **başka ajanın** commit edilmemiş
  `format.test.ts`/`Cases.test.tsx` değişiklikleridir (commit'te yok).

**Dürüst sınırlar:** (a) `system_info` **komut olarak koşulmadı** (`State<AppState>`
kuran test altyapısı yok; komut zaten state istemez) — kablolama derleme + kaynak
kapısı, davranış arayüz düzeyinde `api` mock'lu; (b) ayrışma **gösterilir**, otomatik
onarılmaz; (c) kart yalnız Ayarlar'da — kabuk başlığındaki sürüm hâlâ `package.json`dan
(tek kaynak yapılması ayrı iş); (d) jsdom tarayıcı değil → gerçek Tauri penceresi ve
**görsel QA insan bekliyor**.

## Hâlâ eksik

- OS düzeyinde kilit (ekran kilidi) algılama.
- Denetim zinciri: onarım/uzlaştırma akışı, tarih/aktör filtresi, sayfalama.

