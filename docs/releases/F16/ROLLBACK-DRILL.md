# F16 — Atomik güncelleme ve geri dönüş tatbikatı

- **Durum:** Tatbikat yapıldı (otomatik, gerçek dosyalarla) ve **uygulama içi
  akışa bağlandı** (Ayarlar → “Güncelleme öncesi yedeğe dön”)
- **Tarih:** 2026-10-24 (tatbikat) · 2026-10-25 (uygulama içi akış)
- **Kod:** `crates/storage/src/lib.rs` (güncelleme zarfı + `rollback_vault`),
  `crates/storage/src/pre_update.rs` (yedek keşfi + `rollback_and_resume`),
  `crates/storage/src/upgrade_tests.rs` (7 test),
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
   mevcut dosyayı (ve varsa `-wal`/`-shm` yan dosyalarını) güvenlik kopyasına
   taşır, yedeği yerine koyar ve yerleşen dosyayı yeniden doğrular. Son
   doğrulama düşerse güvenlik kopyası geri konur. Yan dosyalar taşınmazsa eski
   `-wal` yeni dosyaya uygulanır ve kasa bozulur; bu yüzden taşınıyor.
5. **Bozuk yedek reddedilir.** Yedek dosyası bozulmuşsa `rollback_vault`
   `E_DB_MIGRATION_BACKUP` verir ve **mevcut kasa dosyasına dokunmaz**.
6. **Uygulama içi akış kilitler → geri koyar → yeniden açar.**
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
  kopyalanır; en az kasa boyutu kadar boş alan gerekir. Yetersiz alanda işlem
  hata verir, kasa dosyası yerinde kalır (test edilmedi — F15-T06).
- **Tek yazıcı:** geri dönüş kasa kilidi altında çalışır; süre kasa boyutu ve
  migrasyon sayısıyla artar, bu sırada arayüzün diğer kasa komutları bekler.
- **Arayüz görsel QA'sı yok:** Tauri penceresi bu ortamda açılmadı; kart
  typecheck + birim testleri + üretim derlemesiyle doğrulandı, gerçek tıklama
  ve ekran görüntüsü doğrulaması yapılmadı.
- Disk dolması / süreç öldürme senaryoları bu tatbikatta yok (F15-T06).
