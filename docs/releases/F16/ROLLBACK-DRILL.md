# F16 — Atomik güncelleme ve geri dönüş tatbikatı

- **Durum:** Tatbikat yapıldı (otomatik, gerçek dosyalarla), **uygulama içi
  akışa bağlandı** (Ayarlar → “Güncelleme öncesi yedeğe dön”) ve **çökme +
  gerçek disk dolması** senaryoları ölçüldü
- **Tarih:** 2026-10-24 (tatbikat) · 2026-10-25 (uygulama içi akış) ·
  2026-10-26 (süreç öldürme + gerçek ENOSPC)
- **Kod:** `crates/storage/src/lib.rs` (güncelleme zarfı + `rollback_vault`),
  `crates/storage/src/atomic.rs` (sabit bağlantı, geçici artık süpürme,
  tatbikat kancası), `crates/storage/src/error.rs` (`E_DB_ROLLBACK_WAL`,
  `E_DB_IO`), `crates/storage/src/pre_update.rs` (yedek keşfi +
  `rollback_and_resume`), `crates/storage/src/upgrade_tests.rs`,
  `crates/storage/tests/rollback_drill.rs`,
  `apps/desktop/src-tauri/src/commands.rs`
  (`vault_pre_update_backups`, `vault_rollback_pre_update`)

Bu belge bir **tatbikat kaydıdır**: her adım gerçek SQLCipher kasa dosyasıyla,
gerçek migrasyon dosyalarıyla ve gerçek geri dönüş koduyla çalıştırıldı.
Aşağıdaki davranışlar testle sabitlendi; hiçbiri "tasarım niyeti" değildir.

## Ne garanti ediliyor

1. **Yedeksiz güncelleme başlamaz.** Bekleyen migrasyon varsa `Database::open`
   önce `sqlcipher_export` ile şifreli kopya yazar, kopyayı açar, anahtarını ve
   `PRAGMA integrity_check` sonucunu doğrular; ancak ondan sonra migrasyonu
   uygular. Yedek yazılamaz/doğrulanamazsa hata `E_DB_MIGRATION_BACKUP` olur ve
   dosya **eski sürümde** kalır.
2. **Migrasyon tek işlemde.** Bir migrasyon dosyası yarıda düşerse (`CREATE
   INDEX` çakışması ile denendi) `schema_migrations` kaydı yazılmaz, şema eski
   sürümde kalır, yedek kullanılabilir durumda kalır.
3. **Yeni şema eski uygulamaya kapalı.** `schema_migrations` içinde daha yeni
   bir sürüm varsa `open` **hiçbir yazma yapmadan** `E_DB_SCHEMA_NEWER` döner;
   eski uygulama yeni kasayı açıp üzerine yazamaz.
4. **Geri dönüş kendi içinde atomik.** `rollback_vault` yedeği doğrular,
   mevcut dosyayı **sabit bağlantıyla** güvenlik kopyası olarak saklar
   (desteklenmiyorsa tam kopya + doğrulama), yedeği `rename` ile yerine koyar ve
   yerleşen dosyayı yeniden doğrular. Son doğrulama düşerse güvenlik kopyası
   atomik olarak geri konur. Yan dosyalar taşınmazsa eski `-wal` yeni dosyaya
   uygulanır ve kasa bozulur; bu yüzden taşınıyor.
5. **Kasa dosyası geri dönüş sırasında hiç ortadan kalkmaz.** Eski sıra dosyayı
   önce `rename` ile taşıyordu; süreç o arada ölürse kasa "yok" görünüyor ve
   `Database::open` (`SQLITE_OPEN_CREATE`) **boş bir kasa yaratıyordu**. Artık
   güvenlik kopyası bağlanır, yedek `rename` ile **üzerine** konur: `vault.db`
   her an vardır (süreç öldürme tatbikatıyla ölçüldü).
6. **Dolu yan dosya varken geri dönüş başlamaz.** Kasa yanında dolu `-wal`/
   `-shm` varsa (başka süreç/ düzgün kapanmamış kasa) `E_DB_ROLLBACK_WAL`
   döner; yan dosyalar yerinden oynatılıp içlerindeki işlemler kaybettirilmez.
7. **Disk doluysa geri dönüş başlamaz.** Yedek geçici ada kopyalanırken
   `E_DISK_FULL` döner; kasa **bayt bayt** değişmez, hedefte yarım dosya
   kalmaz. Dolu diskte kasa açılamıyorsa hata artık "yanlış anahtar" değil
   `E_DB_IO`'dur (yer açılınca açılır).
8. **Bozuk yedek reddedilir.** Yedek dosyası bozulmuşsa `rollback_vault`
   `E_DB_MIGRATION_BACKUP` verir ve **mevcut kasa dosyasına dokunmaz**.
9. **Uygulama içi akış kilitler → geri koyar → yeniden açar.**
   `rollback_and_resume` yedeği geri koyduktan sonra kasayı açar ve **güncel
   şemaya ilerletir**; dönen bağlantı kullanıma hazırdır. Geri dönüş düşerse
   `Err` döner ve çağıran kasayı eski hâlinde yeniden açar (kullanıcı kilitli
   kalmaz). Geri dönüş başarılı olup yeniden açılış düşerse
   `E_DB_ROLLBACK_REOPEN` **hangi sürümün geri konduğunu ve güvenlik
   kopyasının yolunu** taşır; sessizce yutulmaz.

## Yedek dosyası nerede

`<kasa>/vault.db.v<eski sürüm>-pre-update.bak`

Örnek: `vault.db.v12-pre-update.bak`. Aynı sürüm için yedek zaten varsa
üzerine yazılmaz; ilk sağlam yedek korunur (yeniden doğrulanır).
Geri dönüşün güvenlik kopyası: `vault.db.rollback-<unix>.safety`.

## Tatbikat adımları (testlerin yaptığı)

1. v12 şeması, **gerçek** `0001…0012` migrasyon dosyalarıyla kurulur; içine bir
   dosya kaydı (`Eski dosya`) yazılır.
2. `Database::open` çağrılır → v13'e güncellenir. Kontrol: `applied = [13]`,
   `pre_migration_backup()` dolu, yedek dosyası v12 şemasında ve açılıyor.
3. Ayrı senaryoda yedek yolu bir klasörle kapatılır → `open` `E_DB_MIGRATION_BACKUP`
   verir, dosya v12'de kalır.
4. Ayrı senaryoda v13'ün oluşturacağı indeks adı önceden yaratılır → migrasyon
   yarıda düşer (`E_DB_MIGRATION`), dosya v12'de kalır, yedek hâlâ açılıyor.
5. Ayrı senaryoda `schema_migrations`'a v14 satırı eklenir → `E_DB_SCHEMA_NEWER`;
   v13 kaydı ve `precedent_versions.full_text` sütunu **yok** (yazma olmadı).
6. Güncelleme sonrası yeni kayıt (`Yeni dosya`) eklenir, sonra
   `rollback_vault` çalıştırılır: ana dosya v12'ye döner, yalnız `Eski dosya`
   görünür; güvenlik kopyasında iki kayıt da durur.
7. Yedek bozulup `rollback_vault` çağrılır → hata; ana kasa v13'te ve verisi
   yerinde.

## Uygulama içi akış (Ayarlar → “Güncelleme öncesi yedeğe dön”)

Arayüzdeki düğme gerçek çekirdeği çalıştırır; local state ile “geri döndü”
görüntüsü üretilmez.

1. **Liste — `vault_pre_update_backups`:** kasanın yanındaki
   `<kasa>.db.v<N>-pre-update.bak` dosyaları bulunur. Her dosya **salt okunur**
   açılıp anahtarı ve `PRAGMA integrity_check` sonucu doğrulanır; addan okunan
   sürüm ile dosyanın bildirdiği sürüm ayrı alanlardır. Doğrulama yedeği
   değiştirmez; kendi bıraktığı boş `-wal`/`-shm` yan dosyalarını temizler,
   **dolu** bir `-wal` görürse bunu sorun olarak bildirir. Bozuk ya da başka
   kasanın yedeği listede **görünür** ama `usable=false` olur ve düğme kapalı
   kalır; neden yazılır.
2. **Onay:** düğme, kullanıcı onay alanına birebir `GERİ DÖN` yazmadan
   açılmaz (`E_ROLLBACK_CONFIRM`). Tek tıkla veri geri alma yok.
3. **Seçim:** istemci **yol göndermez**, listedeki dosya adını gönderir; ad
   yeniden doğrulanır, yol ayırıcısı/`..` reddedilir ve ad listede bulunanla
   birebir eşleşmek zorundadır (`E_ROLLBACK_BACKUP`).
4. **Geri dönüş:** kasa kilitlenir (bağlantı + önizleme belleği kapanır —
   Windows'ta açık dosya yerine konmaz), `rollback_and_resume` yedeği geri
   koyar, kasayı yeniden açar ve güncel şemaya ilerletir. Geri dönüş öncesi
   dosya `vault.db.rollback-<unix>.safety` olarak saklanır.
5. **Denetim:** istek **eski** kasaya (`vault_rollback`/`requested`, şema
   sürümleri), tamamlanma **geri konan** kasaya
   (`vault_rollback`/`completed`, `restoredVersion`, `resumedVersion`,
   `safetyCopy=true`) yazılır. Denetime dosya yolu yazılmaz.

Testler: `pre_update` 4 birim testi (kapsam/bozuk yedek/yanlış anahtar/ad
uyuşmazlığı/yan dosya temizliği), `upgrade_tests` 1 yeni uçtan uca test
(`in_app_rollback_restores_old_data_and_resumes_current_schema`: v12 kasa →
güncelleme → yeni kayıt → liste → geri dönüş → yalnız eski kayıt, şema güncel,
güvenlik kopyasında iki kayıt, denetim kaydı), komut testleri (ad kapısı, onay
metni).

## Çökme ve disk dolması tatbikatı (2026-10-26)

İki senaryo **gerçek** dosyalarla ölçüldü; hiçbiri taklit değil
(`crates/storage/tests/rollback_drill.rs`).

### 1) Geri dönüşün ortasında süreç öldürme

Çocuk süreç gerçek `rollback_vault` çağrısını yapar ve yedek **yerine
konmadan önce** bekler (`before_install` noktası; `atomic::drill_pause` yalnız
hata ayıklama derlemesinde etkin). Ebeveyn güvenlik kopyası görününce
`SIGKILL` gönderir.

| Ölçüm | Sonuç |
|---|---|
| Öldürme noktası | güvenlik kopyası hazır, `rename` **yapılmadı** |
| `vault.db` dosyası | **yerinde** (eski sırada *yoktu* — aşağıdaki kırmızı kanıt) |
| Açılış | açılır; kayıtlar `Eski dosya` + `Yeni dosya` (güncelleme sonrası veri) |
| Şema | güncel sürüm |
| Çökme artığı | 1 geçici dosya (`.vault.db.rollback-<pid>-<sıra>`) + güvenlik kopyası |
| Tekrar deneme | `sweptTemps=1` ile artık silinir; sonuç yalnız `Eski dosya` |

**Kırmızı kanıt:** eski sıra (kasayı önce `rename` ile taşıyan kod) geçici
olarak geri konup aynı tatbikat koşturuldu; `kasa dosyası yerinde kalmalı`
iddiası **düştü**. Yani test gerçekten kusuru yakalıyor, süs değil.

### 2) Gerçek disk dolması (8 MB APFS imajı, `#[ignore]`)

| Ölçüm | Sonuç |
|---|---|
| Kasa / yedek | 876.544 / 876.544 bayt (300 eski + 1 yeni kayıt) |
| Doldurucu | 5.177.344 bayt · **48 KiB** boşluk |
| `rollback_vault` | **`E_DISK_FULL`** ("No space left on device (os error 28)") |
| Kasa baytları | **birebir aynı**; geçici artık yok, güvenlik kopyası oluşmadı |
| Dolu diskte `Database::open` | **`E_DB_IO`** ("disk I/O error"; SQLCipher `SystemIoFailure`/`SHMCLOSE`) — *eskiden `E_DB_KEY` "anahtar yanlış veya veritabanı bozuk" diyordu* |
| Yer açıldıktan sonra | kasa açılır (**301** kayıt), geri dönüş başarılı (**300** kayıt), güvenlik kopyası 876.544 bayt |

### Bulunan kusurlar (defter #39–#41)

- **#39 (kritik): geri dönüşte "kasa yok" penceresi.** Eski sıra kasayı önce
  `rename` ile güvenlik kopyasına taşıyordu; süreç o arada ölürse dosya
  ortadan kalkıyor ve `Database::open` (`SQLITE_OPEN_CREATE`) **boş kasa**
  yaratıyordu — kullanıcı için "bütün dosyalar kayboldu". Düzeltme: sabit
  bağlantı (`hard_link_or_copy`; desteklenmiyorsa doğrulanmış tam kopya) +
  yedeğin `rename` ile **üzerine** konması.
- **#40 (işlevsel): her geri dönüş 32 KiB `-shm` artığı bırakıyordu.**
  `remove_empty_sidecars` yalnız **boş** dosyaları siliyordu; SQLite'ın
  bıraktığı `-shm` 32 KiB olduğu için kalıyordu. Ayrıca yedek doğrulaması
  kullanıcının klasöründe `-wal`/`-shm` bırakabiliyordu. Düzeltme:
  `remove_temp_sidecars` (yalnız bizim geçici kopyalarımızda) ve yedek
  doğrulaması `pre_update::verify_without_residue` ile.
- **#41 (işlevsel): disk dolu, "yanlış anahtar" gibi görünüyordu.** Tam dolu
  APFS'te ilk okuma `SystemIoFailure`/`SHMCLOSE` veriyor; kod bunu `E_DB_KEY`
  ("anahtar yanlış veya veritabanı bozuk") olarak bildiriyordu. Düzeltme:
  `classify_first_read_failure` → `E_DB_IO` (G/Ç sınıfı) / `E_DISK_FULL` /
  `E_DB_KEY` ayrımı; `retryable=true`.

### Bu turda eklenen davranış

- `rollback_vault` güvenlik kopyasını **bağlar** (kopyalamaz): kasa dosyası
  hiçbir an yok olmaz, ek yer maliyeti yok.
- Dolu `-wal`/`-shm` varken geri dönüş **başlamaz** (`E_DB_ROLLBACK_WAL`).
- Sahipsiz `.vault.db.rollback-*` artıkları geri dönüş başında süpürülür
  (`RollbackOutcome.swept_temps` sayısı döner).
- Aynı saniyedeki ikinci deneme önceki güvenlik kopyasının üzerine yazmaz
  (`-1`, `-2` sırası).

## Komut

```bash
cargo test -p dim-storage upgrade_tests
cargo test -p dim-storage pre_update
```

## Dürüst sınırlamalar

- **İkili (uygulama) geri dönüşü yok.** Uygulama içi düğme **veriyi**
  güncelleme öncesi hâline döndürür ve kasayı bu uygulamanın şema sürümüne
  yeniden yükseltir. Eski bir uygulama sürümüne dönmek kurulumla yapılır; bu
  akış onu yapmaz ve yaptığını iddia etmez. Gerçek bir imzalı sürüm
  yükseltmesi denenmedi (updater `active=false`, ADR-0012).
- **Ayrı kasaya kopyalama bu kartta yok.** Plan “güvenli dönüş” olarak
  doğrulanmış snapshot'ın ayrı kasaya yüklenmesini de sayar; bunun için
  Ayarlar'daki “Yedek al / Yedekten geri yükle” akışı kullanılır (tam yedek
  klasörü). Güncelleme öncesi `.bak` dosyasını tek başına yeni kasaya açma
  düğmesi **yok**.
- **Blob'lar geri alınmaz.** Güncelleme öncesi yedek yalnız veritabanıdır
  (`sqlcipher_export`); şifreli blob dosyaları şema güncellemesinden
  etkilenmediği için yerinde kalır. Blob'ları da geri almak isteyen tam yedek
  (`vault_backup`) kullanmalıdır.
- **Disk:** geri dönüş sırasında kasa dosyası bir kez güvenlik kopyası olarak
  saklanır; sabit bağlantı kurulabildiğinde **ek yer gerekmez**, kurulamayan
  dosya sisteminde kasa boyutu kadar boş alan gerekir. Yetersiz alanda işlem
  `E_DISK_FULL` verir ve kasa dosyası yerinde kalır (ölçüldü, 2026-10-26).
- **Dolu diskte kasa açılamaz.** SQLite WAL için yer bulamıyor; hata `E_DB_IO`
  (eskiden yanıltıcı biçimde `E_DB_KEY` idi). Yer açılınca kasa normal açılır
  (ölçüldü: 301 kayıt).
- **Tek yazıcı:** geri dönüş kasa kilidi altında çalışır; süre kasa boyutu ve
  migrasyon sayısıyla artar, bu sırada arayüzün diğer kasa komutları bekler.
  Dolu `-wal` görülürse geri dönüş **başlamaz** (`E_DB_ROLLBACK_WAL`).
- **Arayüz görsel QA'sı yok:** Tauri penceresi bu ortamda açılmadı; kart
  typecheck + birim testleri + üretim derlemesiyle doğrulandı, gerçek tıklama
  ve ekran görüntüsü doğrulaması yapılmadı. Bu turda karta iki sınır satırı
  eklendi (çökme + disk dolması); `vitest` lokalde koşulmadı (`node_modules`
  yok) — CI `arayüz` işi koşuyor.
- **Tatbikat sınırları:** süreç öldürme yalnız **macOS/APFS** ve hata ayıklama
  derlemesinde `drill_pause` kancasıyla; ürün derlemesinde kanca yok, gerçek
  çökme zamanlaması rastgele. Disk dolması tatbikatı 48 KiB boşlukla ve
  **876.544 baytlık** kasayla yapıldı. `hard_link` desteklemeyen dosya
  sistemindeki **kopya yolu** kodda var ama gerçek ortamda denenmedi. Ağ
  biriminde `rename`/bağlantı semantiği denenmedi. Uyku/uyanma ve harici disk
  kopması hâlâ denenmedi.
