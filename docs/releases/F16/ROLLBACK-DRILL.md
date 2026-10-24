# F16 — Atomik güncelleme ve geri dönüş tatbikatı

- **Durum:** Tatbikat yapıldı (otomatik, gerçek dosyalarla)
- **Tarih:** 2026-10-24
- **Kod:** `crates/storage/src/lib.rs` (güncelleme zarfı + `rollback_vault`),
  `crates/storage/src/upgrade_tests.rs` (6 test)

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

## Komut

```bash
cargo test -p dim-storage upgrade_tests
```

## Dürüst sınırlamalar

- Tatbikat **kütüphane düzeyinde** yapıldı. Uygulama içinde "geri dön" düğmesi
  yok: geri dönüş kapalı dosya üzerinde çalışır, çalışan bir kasadan çağrılması
  ayrı bir akış (kilitle → kapat → geri dön → aç) gerektirir ve bu turda
  yapılmadı. Kullanıcı yedeği Ayarlar'dan alıp yeni kasaya geri yükleyebilir
  (`vault_restore`), ancak bu **aynı kasayı yerinde geri alma** değildir.
- Gerçek bir sürüm yükseltmesi (imzalı güncelleme paketi) denenmedi: updater
  `active=false` (ADR-0012, imza kapsam dışı). Tatbikat şema güncellemesini
  ölçer, ikili (binary) geri dönüşü değil.
- Disk dolması / süreç öldürme senaryoları bu tatbikatta yok (F15-T06).
- Yedek `sqlcipher_export` ile alınır; çok büyük kasalarda güncelleme anında
  ek disk alanı gerekir (kasa boyutu kadar). Bu, güncelleme öncesi boş alan
  kontrolünü zorunlu kılar — uygulama tarafında henüz ölçülmüyor.
