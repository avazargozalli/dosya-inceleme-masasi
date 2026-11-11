# F14 — Çıktılar, paket, yedek

- **Sürüm etiketi:** `v0.13.0`
- **Durum:** Kısmi
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F14-T01 JSON dışa aktarma | ✅ | `export_json`, tam metadata, hash kaydı |
| F14-T02 CSV | ✅ | kronoloji + görevler + **belge dizini**; **güvenli hücre politikası** (`=`, `+`, `-`, `@`, tam genişlikli karşılıkları, baştaki boşluk/kontrol) — `dim_storage::build_case_csv` (2026-11-08; aşağıdaki bölüm) |
| F14-T03 DOCX rapor | ✅ | `render_report`, tablolar + notlar |
| F14-T04 PDF rapor | ✅ | `render_report_pdf`, HTML kaçışlı, `Story` ile sayfalama |
| F14-T05 dışa aktarma sicili | ✅ | `exports` tablosu + denetim kaydı; **okunur**: `exports_list` + Çıktılar ekranında defter (tür, hedef yol, zaman, özet, durum). **Kaynak sürüm izi (2026-11-10):** `export_items` artık yazılır — her çıktı hangi **belge sürümlerinden** üretildi ve o sürüm `masked` mi (`redaction_plans.output_version_id` ölçüsü); `included_ids` kalemlerden türetilir, `template_version` gerçek biçim sürümünü taşır, `E_EXPORT_SOURCE` kapıları düşerse **hiçbir satır yazılmaz**; ekranda satır başına "kaynakları göster" (aşağıdaki bölüm) |
| F14-T06 şifreli devir paketi | ✅ | klasör → zip → Argon2id + AES-256-GCM `.dimpak`; parola verilmezse açık klasör kalır |
| F14-T07 yedek zamanlaması / bildirim | ✅ | Otomatik yedek: klasör + 7 gün aralık + son tarih; App saatte bir kontrol eder, Settings kartında **son başarılı yedek · son deneme (neden koduyla) · sıradaki plan** ayrı gösterilir; başarısız deneme tazeliği ilerletmez; **başarılı yedekler kasa içi `backup_runs` geçmişine yazılır ve Ayarlar'da okunur** (2026-11-07); **plan (hedef + aralık) kasa ayarında (`settings`, kapsam `vault`) saklanır, cihaz kaydındaki eski plan bir kez göç eder** (2026-11-07); **"son başarılı yedek" tazeliği de kasadan okunur, cihaz kaydı yalnız yedektir ve çelişki gizlenmez** (2026-11-07) |
| F14-T08 PDF çıktısında Türkçe font gömme | ✅ | Gömülü **Charis SIL** (Type0/Identity-H); `test_pdf_report_embeds_unicode_font_and_keeps_turkish` |
| F14-T09 yedek saklama (retention) | ✅ | Otomatik yedek klasöründe **son 7 günlük + 4 haftalık** politika (değiştirilebilir); tarama → önizleme → **açık onay** → silme. **Yalnız bu kasanın** yedekleri silinir (manifest `vaultId`); kimliksiz eski yedek, başka kasanın yedeği, symlink ve bozuk manifest **dokunulmaz**; en yeni yedek her zaman korunur; boşalan bayt ölçülür (2026-11-11; aşağıdaki bölüm) |
| Yarım çıktı bırakma / atomik yazım (şartname F14-T11) | ✅ | Beş dışa aktarma ve devir paketi `atomic::write_atomic_verified` ile yazılır (geçici ad → geri okunan özet → `rename`); 2026-11-07 ölçüm + 8/8 mutasyon (aşağıdaki bölüm) |

## Dürüst durum

Çıktılar kullanıcı seçtiği yola yazılır ve kasa içinde kayda alınır. Devir paketi
parola verilirse **şifrelenir** (`crates/vault/src/package.rs`: Argon2id + AES-256-GCM,
magic + tuz + nonce başlığı); parola verilmezse açık klasör kalır ve arayüz bunu
söyler. Belge baytları otomatik pakete konmaz; maskeli kopyalar ayrıca eklenir.

## Dışa aktarma denetim defteri okunur (2026-11-06)

**Ölçülen boşluk:** her dışa aktarma `exports` tablosuna kayıt yazıyordu
(`record_export`; düşen pakette `fail_export` → `failed`) ama **okuma yolu yoktu**:
`list_exports` yalnız `crates/storage` içinde tanımlıydı — Tauri komutu, `api.ts`
bağı ve ekran kartı yoktu. Sonuç: avukat "bu dosyadan ne çıktı, nereye, hangi
özetle, başarılı mı" sorusunu yanıtlayamıyordu; `failed` işaretlenen kayıt hiçbir
yerde görünmüyordu (kayıt yalnız SQLite dosyasındaydı). Ekrandaki "yazıldı" satırı
bellekte tutuluyordu: dosya değişince ya da ekran yeniden açılınca kayboluyordu.

**Yazılan:**
- `exports.destination_path` (**24. şema**, `ALTER TABLE … ADD COLUMN`): kayıt
  **gerçek** hedefi taşır; boş yol `NULL`'dır (konum uydurulmaz). Eski kayıtlar
  `NULL` taşır ve arayüz "hedef yazılmamış (eski kayıt)" yazar.
- Paket yolunda parola verildiyse `seal_dir` ile **aynı** ad kuralı uygulanır
  (`<klasör>.dimpak`); kayıt `proposed` anında yazıldığı için (denetlenemeyen
  aktarım yapılmaz) hedef, paketlemeden **önce** hesaplanır.
- `exports_list` komutu + `api.exportList`; liste **en yeni önce**
  (`created_at DESC, rowid DESC` — aynı milisaniyede yazılan iki kayıt
  `created_at` ile ayrılmaz, sıra kararlı olmalı).
- Çıktılar ekranında **"Dışa aktarma kayıtları"** kartı: tür · zaman · hedef ·
  özet öneki ve durum rozeti (`yazıldı` / `başarısız`). Tanınmayan durum
  gizlenmez — ham değeriyle yazılır.
- Tazeleme: dosya değişiminde (liste **istekten önce** temizlenir), her yazma
  denemesinden sonra (başarı **ve** hata — düşen paket kaydı ancak böyle görünür)
  ve "Kayıtları tazele" düğmesiyle. Okuma hatası kendi satırında yazılır;
  "kayıt yok" **denmez** (okunamayan defter boş sanılmamalı).

**Kanıt (yerel):** storage `exports` testleri `export_destination_path_is_stored_and_blank_is_null`
ve `exports_are_listed_newest_first`; arayüz `Exports.test.tsx` **49 → 57**
(+8; yeni `describe` "dışa aktarma kayıtları (denetim defteri)"). Düzeltme öncesi
HEAD bileşeninde sekiz testin sekizi de **kırmızı** (`8 failed | 49 skipped`).
Kapı mutasyonları **8/8** (her biri tek başına kırmızı; M5 iki testi birden düşürür
— başarısız satır iki testte de görünür; geri yükleme sha256 birebir).

**Dürüst sınırlar:**
- Kayıt **silinemez** (denetim defteri) ama ekranda **filtre/sayfalama yok**;
  çok sayıda dışa aktarmada liste uzar.
- Yardımcı dosya yolu (`<hedef>.sha256` benzeri) yazılmıyor; kayıt yalnız özeti taşır.
- `output_hash` paket için **manifest** özetidir (`.dimpak` özeti değil — sözleşme kararı).
- Yol, arayüzde tam gösterilir (kısaltma `title` ile); çok uzun yollarda satır taşmaz
  ama tek satırda kesilir.
- jsdom tarayıcı değil: gerçek Tauri penceresi, gerçek dosya seçici ve **görsel QA
  insan bekliyor**.

## Otomatik yedek durumu görünür — son deneme, plan, bekleme nedeni (2026-11-07)

**Ölçülen kusur:** otomatik yedek zamanlayıcısı (`App.tsx`) düşen denemeyi
`catch {}` ile **sessizce yutuyordu**; Ayarlar kartı yalnız `dim.backup.last`
(yani son **başarılı** yedek) zamanını yazıyordu. Şartname §05.5 açık:
"Son başarılı yedek zamanı, son deneme ve sıradaki plan ayrı gösterilir;
başarısız deneme yedeğin tazeliğini ilerletmez. Hedef disk yoksa bekleme
nedeni görünürdür." Hedef disk çıkarıldığında ya da izin düştüğünde avukat
ekranda "son yedek: 3 gün önce" görüyor, yedeğin **hiç alınamadığını**
göremiyordu.

**Yazılan:**
- `lib/backup-schedule.ts`: `BackupAttempt` (`at`, `ok`, `code`) kaydı +
  doğrulayıcı (`parseBackupAttempt`): bozuk/eksik kayıt `null` ("deneme yok"
  ile karışmaz), **başarılı kayda yazılmış kod yok sayılır** (uydurma neden
  gösterilmez). `nextBackupAt` (son başarılı yedek + aralık), `readBackupStatus`,
  `writeBackupAttempt`, `clearBackupAttempt`.
- `App.tsx`: başarıda son tarih **ve** `ok` kaydı; hatada **yalnız** deneme
  kaydı (`code` ile) — `BACKUP_LAST_KEY` yazılmaz, yani tazelik ilerlemez.
  Kabukta modal/toast yok (bilinçli: saatlik yeniden deneme gürültü üretmesin);
  görünürlük Ayarlar'dadır.
- Ayarlar → Otomatik yedek kartı: **"Son deneme"** (zaman · başarılı/başarısız)
  ve **"Sıradaki plan"** (son başarılı yedekten hesaplanır; süresi dolduysa
  "sonraki kontrolde"). Başarısız denemede kırmızı uyarı + **hata kodu**
  (bekleme nedeni: "Hedef klasör erişilebilir mi — disk takılı ve izin var mı?").
- 9 sözlük anahtarı (`settings.autoAttempt*`, `settings.autoNext*`); gömülü metin yok.

**Kanıt (gerçek koşular):**
- Yerel: `tsc --noEmit` temiz · vitest **62 dosya / 1243 test** · `npm run build`
  **624,59 kB** (`index-BpKW5LTC.js`, gzip **167,56 kB**).
- **CI:** `42376626051` **10/10 YEŞİL**; `arayüz` **62 dosya / 1242 test**, paket
  `index-DEB1rJT2.js` **624,02 kB** / gzip **167,32 kB**; `masaüstü kabuk` 34/0/5 +
  yoksayılan koşu 5 passed. Yerel ile **1 test / 0,57 kB** farkı, ağaçta duran
  **başka ajanın** commit edilmemiş `format.test.ts`/`Cases.tsx` değişiklikleridir
  (commit'te yok).
- Kapı **8/8 mutasyon kırmızı** (her mutasyon temiz dosyaya, tek başına):
  M1 düşen deneme kaydı silindi (**1 failed · 17 passed**), M2 hatada tazelik
  ilerletildi (**2 · 16**), M3 "son deneme" satırı silindi (**3 · 34**),
  M4 uyarı gizlendi (1 · 36), M5 plan her zaman "süresi doldu" (1 · 36),
  M6 bozuk tarih kabul (1 · 15), M7 başarılı kayıtta kod korundu (1 · 15),
  M8 plan aralığı yok sayıldı (1 · 15). Geri yükleme sha256 birebir
  (`App.tsx 6eaf8c8c…`, `Settings.tsx a2f55ceb…`, `backup-schedule.ts 4b354a7a…`).
- **Dürüst:** M2 ve M3 cerrahi değil (birden çok test düşürüyor); M1 ve M4–M8 cerrahi.

**Dürüst sınırlar:**
- Kayıt **yerel** (localStorage, cihaz kapsamı): kasa içi `settings` tablosu
  hâlâ kullanılmıyor; uygulama verisi temizlenirse deneme geçmişi kaybolur
  (son başarılı yedek de aynı yerdeydi — bu dilim onu değiştirmedi).
- Bekleme nedeni **deneme sonrası** görünür: ön uçuşta disk/izin yoklaması yok
  (`vault_backup` hatası ne derse o yazılır; tanınmayan kod ham gösterilir).
- Bildirim yok: işletim sistemi bildirimi/rozet çizilmiyor; yalnız Ayarlar kartı.
- Zamanlayıcı yalnız kasa **açıkken** koşar; kilitliyken deneme yazılmaz
  (şartname: kasa kilitliyken yedek bekler).
- Şartname varsayılanı **günlük** yedek der; ürün **7 gün** aralığı seçmişti
  (bu dilim değiştirmedi — ürün kararı).
- jsdom tarayıcı değil: gerçek Tauri penceresi, gerçek disk çıkarma ve
  **görsel QA insan bekliyor**.

## Yedek geçmişi — `backup_runs` yazılır ve okunur (2026-11-07)

**Ölçülen kusur (yazma + okuma yok):** şema dosyasında `backup_runs` tablosu
vardı (`docs/spec/...:434`: snapshot, şema sürümü, manifest özeti, durum,
doğrulama sonucu, hedef). Koda karşı tarandı: tabloya **tek bir INSERT bile
yoktu** (`select=0 insert=0 update=0`). `vault_backup` yedeği diske yazıyor,
manifesti üretiyor ve yedeğin açıldığını (integrity check) doğruluyordu ama
kasaya yalnız `audit_events` içinde `{files:N}` düşüyordu. "Ne zaman, nereye,
hangi manifestle, doğrulandı mı" sorusunun tek kanıtı hedef klasördeki
`manifest.json` dosyasıydı; uygulamadan okunamıyordu. Ayarlar ekranı yalnız
**son** başarılı/deneme zamanını (localStorage) gösteriyordu; geçmiş yoktu.

**Yazılan:**
- `crates/storage/src/backup_runs.rs` (yeni): `record_backup_run` +
  `list_backup_runs`; `BackupRunRow` (camelCase) ve `NewBackupRun`.
  Durum kümesi **kapalı** (`running|succeeded|failed|verified`); boş
  snapshot/manifest/hedef reddedilir; hedef kırpılır; `running` dışında
  `completed_at` yazılır. Sıra **en yeni önce** (`created_at DESC, rowid DESC`
  — aynı damgada kayıt sırası korunur), `limit` 1..=50'ye kırpılır.
- `vault_backup`: yedek + arşiv başarıyla yazıldıktan ve doğrulandıktan
  **sonra** kayıt yazılır (`status="verified"`, `verification_result="integrity_check"`,
  `schema_version=db.schema_version()`). Kayıt düşerse **yedek geçersiz olmaz**
  (dosyalar hedefte durur): yanıt `historyRecorded=false` + `historyError`
  taşır, arayüz bunu kırmızı uyarı olarak yazar — sessiz yutma yok.
- `backup_run_list` komutu + `api.backupRunList` + `BackupRunRow` tipi.
- Ayarlar → **"Yedek geçmişi"** kartı: zaman · hedef (seçilen klasör) ·
  snapshot adı · manifest öneki · durum rozeti. Kilitliyken **okuma yok**;
  liste istekten önce temizlenir; düşen okuma kendi satırında yazılır
  ("kayıt yok" **denmez**); yedek başarısından sonra ve "Geçmişi tazele" ile
  okunur; `historySeq` ile geç yanıt yeni listeyi ezmez. Tanınmayan durum
  gizlenmez, **ham değeriyle** yazılır.
- 16 sözlük anahtarı (`settings.backupHistory*`); gömülü metin yok.

**Kanıt (gerçek koşular):**
- Düzeltme öncesi HEAD `Settings.tsx` + nihai test dosyası: **5 failed |
  37 passed** — beşi de gerçek kusur (kart/okuma yok).
- Kapı **13/13 mutasyon kırmızı** (her mutasyon temiz dosyaya, tek başına;
  geri yükleme sha256 birebir): M1 kilitliyken okuma (**2**), M2 tablo
  çizilmez (**3**), M3 bilinmeyen durum "doğrulandı"ya yuvarlanır (1),
  M4 hatada liste boş (**1**), M5 yedek sonrası tazeleme yok (**2**),
  M6 kayıt uyarısı gizli (1), M7 geç yanıt kapısı yok (1); R1 durum
  doğrulaması yok (1), R2 sıra ters (1), R3 limit kırpma yok (1), R4 hedef
  kırpılmaz (**2**), R5 `completed_at` boş (1).
  **Dürüst:** M1/M2/M5 ve R4 cerrahi değil (birden çok test düşürüyor);
  M4 ilk ölçümde **yeşil kaldı** (boş metin zaten `!historyError` kapısıyla
  gizliydi) → fazladan kapı kaldırıldı, boş metin artık **yalnız** listeye
  bakar, M4 yeniden ölçüldü (1 failed). R5 ilk mutasyonu **derlenmedi**
  (`let completed = None` tür çıkarımı) → `Option<String>` ile yeniden
  ölçüldü; derlenmeyen koşu kanıt sayılmadı.
- `tsc --noEmit` temiz · vitest **62 dosya / 1248 test** (yerel; CI 1247 —
  fark: ağaçta duran başka ajanın `format.test.ts` değişikliği) ·
  `npm run build` **628,50 kB** (`index-Dpi5BGKN.js`, gzip 168,28).
- `dim-storage --lib` CI'da **427/427** (macos 240,96 s · ubuntu 283,64 s — iki
  platformda **birebir**; önceki 422 + 5 yeni; yerelde yalnız `backup_runs::`
  filtresi **5/5** koştu) · storage clippy `-D warnings`
  temiz · masaüstü clippy/check temiz · `cargo fmt` iki manifest temiz.
- **CI:** `42383692666` **10/10 YEŞİL**; `arayüz` **62 dosya / 1247 test**,
  paket `index-126ILiYd.js` **627,93 kB** / gzip **168,02 kB**;
  `masaüstü kabuk` **34 koştu · 0 düştü · 5 yoksayıldı**; `rust` iki
  platformda **427 passed**.
  **Dürüst:** `94fefec` commit mesajındaki kod parçaları kabuk tarafından
  yutuldu (geri yazma cron'da engelli); içerik ve ağaç doğru.

**Dürüst sınırlar:**
- Yalnız **başarılı** yedekler yazılır. Düşen deneme `backup_runs`'a
  `failed` olarak **yazılmaz** (nedeni Ayarlar'daki "son deneme" satırı ve
  hata kodu taşır); `running` durumu bugün hiçbir yol tarafından yazılmıyor.
- Arşiv (`.dimbak`) yazımı düşerse klasör yedeği geçerli kalır ama **geçmiş
  satırı yazılmaz** (kayıt tam başarıdan sonra). Bilinçli: yarım işlem
  "yedeğim hazır" gibi görünmesin.
- Geçmiş **kasa içindedir**: yedeğin kendisi alınırken snapshot, o ana kadar
  yazılmış satırları taşır; en yeni satır kendi yedeğinin içinde yoktur.
  Eski bir yedekten geri yükleme, o andaki (daha eski) geçmişi geri getirir —
  nokta-zaman doğruluğu budur, kayıp değildir.
- `destination_label` kullanıcı seçiminin **kök** klasörüdür; snapshot klasör
  adı ayrı alandır. Yol, kasa şifreli olduğu için cihaz dışına çıkmaz.
- Kart **salt-okunur**: silme/düzenleme, filtre ve sayfalama yok (en yeni 20,
  üst sınır 50). Kayıt sınırı aşılırsa eski satırlar listede görünmez.
- `backup_run_list` **komut olarak koşulmadı** (`State<AppState>` kuran test
  altyapısı yok); kablolama `cargo check` + clippy, davranış storage
  düzeyinde **gerçek SQLite** ile. Gerçek Tauri penceresi, gerçek harici disk
  ve **görsel QA insan bekliyor**.

## Dışa aktarma atomik ve doğrulanmış yazılır (2026-11-07)

**Ölçülen kusur.** Beş dışa aktarma komutu (`export_json`/`csv`/`docx`/`pdf`
ve emsal korpusu) dosyayı `std::fs::write` ile **doğrudan son ada** yazıyordu.
Şartname §1098 "dosya önce uygulamanın kontrol ettiği geçici hedefte
tamamlanır; doğrulama sonrası atomik taşıma mümkünse kullanılır" ve F14-T11
"iptal/hata hâlinde yarım çıktıyı geçerli dosya adıyla bırakma" diyor; çekirdek
bu kuralı `crates/storage/src/atomic.rs` içinde **zaten uyguluyordu** (kasa,
yedek, arşiv) — dışa aktarma yolu onu **kullanmıyordu**.

Gerçek disk dolması ölçümü (küçük bir birim: 12 MB yazım, ~6 MB boş):

| Yol | Sonuç |
|---|---|
| `std::fs::write`, yeni hedef | hedef **var**: **0 bayt** — geçerli adla boş dosya |
| `std::fs::write`, var olan hedef | 512.000 baytlık eski rapor **yok oldu** (0 bayt) |
| parçalı (1 MB) yazım | hedefte **10.485.760 bayt** kesik dosya kaldı |
| atomik (geçici ad → `sync_all` → `rename`) | hedef **hiç oluşmadı**; geçici silindi |
| atomik, var olan hedef | eski dosya **bozulmadı** (204.800 bayt yerinde) |

İkinci kusur: kayda geçen `sha256` **bellekteki** tamponun özetiydi
(`Sha256::digest(&bytes)`), dosyadan geri okunmuyordu. Üçüncü: devir paketi dört
dosyayı doğrudan **son adda** açılan klasöre yazıyordu; yazmalardan biri
düşerse `DimDevir-…` klasörü yarım kalıyor, ikinci deneme `E_PACKAGE_EXISTS`
ile kilitleniyordu (kayıt ise hiç yazılmıyordu).

**Yazılan.**

- `crates/storage/src/atomic.rs`: `write_atomic_verified` /
  `write_atomic_verified_with` — aynı dizinde geçici ada yaz → `sync_all` →
  **geri okuyup sha256** → beklenen özetle karşılaştır → ancak eşitse `rename`.
  Dönen `VerifiedWrite { sha256, bytes, replaced_existing }`; `sha256` **diskten
  okunan** özettir, `bytes` ölçülen boyuttur. Doğrulama tutmazsa hedef
  **değişmez**, geçici silinir, yeni kod `E_WRITE_VERIFY`.
- `write_export` (masaüstü kabuğu) artık bu yardımcıyı çağırır ve sonucu
  döndürür; beş çağrı yeri kayda **diskten okunan** özeti yazar. `ExportView` ve
  `CorpusExportView` `replaced_existing` alanı taşır (denetim kaydı da yazar).
- Devir paketi: içerik yanındaki **hazırlık** dizinine yazılır (`StagingGuard`),
  dört dosya tamamlanınca tek `rename` ile son ada geçer; hata hâlinde `Drop`
  hazırlık dizinini siler. `manifest.json` doğrulanmış yazımla yazılır ve kayda
  geçen özet dosyadan okunur.
- Çıktılar ekranı: sonuç künyesi "yazma doğrulandı" satırını ve (varsa) "var
  olan dosyanın üzerine yazıldı" uyarısını yazar; emsal korpusu sonuç metni de
  üzerine yazmayı söyler. 3 yeni sözlük anahtarı.

**Testler.** `atomic::tests` **7 → 11** (dönen özet = diskteki özet · üzerine
yazma ölçümü · doğrulama tutmazsa hedef korunur + `E_WRITE_VERIFY` · ENOSPC'de
hedef korunur ve geçici kalmaz) → `dim-storage --lib` **427 → 431**. Masaüstü
kabuk **5** yeni test (dosyadan okunan özet · üzerine yazma + eksik klasör ·
boş konum `E_PATH` · yazma düşünce eski dosya bozulmaz · hazırlık dizini
temizliği/atomik yerleştirme) + **yapısal kapı** (komut yüzeyinde ham
`std::fs::write(` kalmamalı) → taban **34 → 40 koşan** (kaynak 45).
Arayüz: `Exports.test.tsx` **57 → 59**, `precedent-terms.test.ts` **+1** → yerel
**62 dosya / 1251 test** (ağaçta başka ajanın commit'siz değişiklikleri vardı).

**Ölçüm (yerel, Node 22.23.1 + TZ=UTC).** `tsc --noEmit` temiz · vitest
**1251/1251** (62 dosya) · `vite build` **628,95 kB** (`index-DM3LG6R3.js`) ·
`dim-storage --lib` **431/431** (206,25 s) · storage + masaüstü clippy
`-D warnings` temiz · `cargo fmt` iki manifest temiz · masaüstü kabuk kapısı
**40 koştu · 0 düştü · 5 yoksayıldı (kaynak 45)**.

**Commit/CI (ölçüldü).** Kod `ea0ffb4` (13 dosya, +505/−33) → **`42398509896`
10/10 YEŞİL** (`dim-storage` **431** hem `macos-15` 192,88 s hem `ubuntu-latest`
358,32 s — birebir; `arayüz` **62 dosya / 1250 test**, paket
`index-DsI79o3X.js` **628,38 kB** / gzip **168,15 kB**; `masaüstü kabuk`
`40 passed · 5 ignored` + yoksayılan koşu `5 passed · 40 filtered` (41,16 s)).
Yerel 1251 / 628,95 farkı ağaçtaki **başka ajanın** commit'siz değişiklikleri
(commit'te yok). **i18n hunk ayrımı:** `git diff` üç hunk'tı; yalnız benim
`exports.verifiedReadback/replacedExisting` + `terms.resultReplaced`
hunk'larım `git apply --cached --recount` ile sahneye alındı, başka ajanın
`cases.parseStatus*` üç anahtarı commit'e **girmedi**
(`git show :…i18n.ts | grep -c parseStatus` → **0**).

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-export-atomic-gates.py`).**
**8/8 mutasyon kırmızı**, her biri tek başına, geri yükleme sha256 birebir:
S1 geri okunan özet doğrulaması kaldırıldı · S2 üzerine yazma ölçümü kaldırıldı ·
S3 hata hâlinde geçici dosya silinmiyor · D1 ham `std::fs::write`'a geri dönüş
(yapısal kapı) · D2 hazırlık dizini `Drop`'ta silinmiyor · U1 "yazma doğrulandı"
satırı gizlendi · U2 "üzerine yazıldı" uyarısı gizlendi · U3 korpus sonuç
metninden üzerine yazma bilgisi kaldırıldı.
**Dürüst:** D3 (yerleştirmeden sonra "tamamlandı" bayrağı kaldırıldı) ilk ölçümde
**yeşil kaldı** — bayrak yük taşımıyordu (rename sonrası hazırlık yolu zaten
yok); bayrak **kaldırıldı**, D3 senaryosu düşürüldü, 8/8 yeniden ölçüldü.

**Dürüst sınırlar.**
- "Üzerine yazma ayrı seçimdir" kapısı hâlâ **kaydet diyaloğunun** onayıdır;
  çekirdek `overwrite` bayrağı istemez. Doğrudan IPC çağrısı var olan dosyayı
  değiştirebilir — artık **atomik** değiştirir ve kayıt/arayüz bunu söyler, ama
  ikinci bir onay sorulmaz.
- Doğrulama **geri okuma**dır: yazma sonrası bozulmayı (bit çürümesi) yakalar;
  yazma sırasında diskin yalan söylemesi (`fsync` sonrası) bu kapının dışında.
- Ürün yolunda **hermetik disk-dolması testi yok** (ENOSPC enjeksiyonu yalnız
  storage sözleşmesinde); ürün yolu gerçek birim ölçümüyle karşılaştırıldı,
  CI'da tetiklenmiyor.
- Paket akışı (`export_package`) **komut olarak koşulmadı** (`State<AppState>`
  altyapısı yok): hazırlık/yerleştirme mekanizması birim testle ölçüldü,
  kablolama `cargo check` + clippy.
- Emsal korpusu dışa aktarımı için arayüzde ayrı uyarı satırı yok; bilgi sonuç
  metninde ve denetim kaydında.
- `std::fs::rename` hedef üzerine yazarken **atomik** yer değiştirir; ağ
  birimlerinde (SMB/NFS) bu garanti dosya sistemine bağlıdır.
- Gerçek Tauri penceresi, gerçek kaydet diyaloğu ve **görsel QA insan bekliyor**.

## Otomatik yedek planı kasa ayarında — `settings` tablosu ilk kez kullanılır (2026-11-07)

**Ölçülen boşluk:** `settings` (`scope`, `key`, `typed_value`, `version`) şemanın ilk
sürümünden beri var ve şartname "kasa ayarı ile cihaz/UI ayarı ayrı depolanır" der;
ama tabloya **tek SQL** yoktu (`git grep`: `INSERT/SELECT/UPDATE/DELETE` sayısı **0**).
Otomatik yedek planı (hedef klasör + aralık) yalnız WebView `localStorage`'ındaydı:
aynı kasa başka bir OS profilinde açıldığında plan görünmüyor, uygulama verisi
silinince yedek **sessizce** kapanıyordu; kasa kendi yedek politikasını taşımıyordu.

**Yazılan:**
- `crates/storage/src/settings.rs`: `get_setting` / `set_setting` / `delete_setting` /
  `list_settings` + `SettingRow` (camelCase). Kapalı kapsam kümesi (`vault|device|ui`,
  tablo CHECK'iyle aynı), anahtar kırpılır ve `1..=64` + yalnız `[a-z0-9._-]`, değer
  JSON ve `≤ 8 KiB`, `null` reddedilir; aynı anahtar ikinci yazımda `version` artar.
  Okunan **bozuk JSON hatadır** (`SettingInvalid`, kod `E_DB_SETTING`) — "kayıt yok"
  denmez; liste bozuk satırı **atlamaz**, `ORDER BY key ASC`.
- Üç Tauri komutu (`vault_setting_get/set/delete`): kapsam **sabit `vault`** — cihaz/UI
  ayarı kasanın içine yazılmaz; `api.ts` bağları + `SettingRow` tipi.
- `lib/backup-config.ts`: plan kasadan okunur; kasa ayarı yoksa cihaz kaydındaki eski
  plan **bir kez** kasaya taşınır (yazım **geri okunarak** doğrulanır; eski anahtarlar
  ancak doğrulanmış göçten sonra silinir). Göç yazılamazsa plan kullanılmaya devam eder
  ama kaynak `legacy` + hata kodu döner. Kasa ayarı **bozuksa** plan yok sayılır ve
  cihaz kaydıyla **ezilmez**. `save` yazımı geri okunan değerle karşılaştırır
  (`E_BACKUP_SETTING_VERIFY`); doğrulanmamış yazım önbelleğe işlenmez.
- `App.tsx` zamanlayıcısı planı kasadan okur (`source: invalid` → yedek **alınmaz**);
  `Settings.tsx` "Otomatik yedek" kartı planı kasadan okur, seç/kapat yollarını kasaya
  yazar, göç/okunamama durumlarını ayrı satırda yazar ("seçilmedi" ile "okunamadı"
  karışmaz). 6 sözlük anahtarı.
- **Kasa ayarı önbelleği:** doğrulanmış yazımdan sonra cihaz kaydına ayna yazılır ve
  **yalnız kasa okunamadığında** kullanılır; kaynak yine kasadır.

**Testler:** storage `settings::tests` **+6** → `dim-storage --lib` **431 → 437**;
`lib/backup-config.test.ts` **15** (yeni dosya); `Settings.test.tsx` **42 → 46** (+4,
biri yeniden yazıldı); `app-shell.test.tsx` **18 → 20** → yerel **63 dosya / 1272 test**.

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-settings-gates.py`).** **15/15
mutasyon kırmızı**, her biri tek başına, geri yükleme sha256 birebir:
U1 kasa ayarı yok sayılır (7 test) · U2 bozuk ayar cihaz kaydıyla ezilir (3) ·
U3 göç yazılmadan eski anahtarlar silinir (2) · U4 yazım doğrulanmaz (1) ·
U5 önbellek doğrulamadan önce yazılır (1) · U6 seçim kasaya yazılmaz/sahte başarı (2) ·
U7 "kapat" kasa ayarını silmez (1) · U8 bozuk ayar yedeği durdurmaz (3) ·
U9 plan hatası gizlenir (1) · R1 kapsam doğrulaması kapalı · R2 anahtar biçimi ·
R3 sürüm artmaz · R4 bozuk değer "kayıt yok" sayılır · R5 silme "satır vardı" der ·
R6 liste sırası ters (her R mutasyonu **tam 1** storage testi — cerrahi).
**Dürüst:** U8'in ilk mutasyonu App'te **import edilmeyen** bir ad kullandı → koşu
çökmeyle kırmızı oldu (81 passed + exit 1) ve **kanıt sayılmadı**; mutasyon sabit
hedefe (`"/yedek"`) çevrildi, 3 testin düştüğü yeniden ölçüldü. U1/U2/U3/U6/U8
cerrahi değil (birden çok test düşürüyor).

**Ölçüm (yerel, Node 22.23.1 + TZ=UTC):** `tsc --noEmit` temiz · vitest **1272/1272**
(63 dosya) · `dim-storage --lib` **437/437** (245,86 s) · storage + masaüstü clippy
`--all-targets -D warnings` temiz · masaüstü `cargo check --all-targets` temiz ·
`cargo fmt` iki manifest temiz. **Dürüst:** yerel `vite build` **koşulmadı** — disk tur
ortasında 4,4 G'ye indi (kural: eşik altında ağır derleme başlatılmaz); paket ölçümü CI'dan.

**CI (ölçüldü):** `be6d8e0` (14 dosya, +1168/−27) → **`42406317633` 10/10 YEŞİL**:
`dim-storage` **437** hem `macos-15` (193,15 s) hem `ubuntu-latest` (473,02 s) —
**birebir**; `arayüz` **63 dosya / 1271 test**, paket `index-DSM126Si.js` **632,01 kB** /
gzip **169,46 kB** (+ `index-CPW4QqlF.css` 7,28 kB); `masaüstü kabuk`
`40 passed · 5 ignored` + yoksayılan koşu `5 passed` (38,63 s); diğer yedi iş yeşil.
Yerel 1272 / CI 1271 farkı ağaçtaki **başka ajanın** commit'siz `format.test.ts`
testidir. **i18n hunk ayrımı:** `git diff` iki hunk'tı; yalnız benim `autoSetting*` /
`autoSchedule*` hunk'ım `git apply --cached --recount` ile sahneye alındı, başka ajanın
`cases.parseStatus*` üç anahtarı commit'e **girmedi** (`git show :…i18n.ts | grep -c
parseStatus` → **0**).

**Dürüst sınırlar.**
- Plan artık kasada; **"son başarılı yedek" ve "son deneme" hâlâ cihaz kaydında**
  (`localStorage`), kasa geçmişi `backup_runs`ta. Uygulama verisi silinirse tazelik
  kaybolur → sonraki açılışta **erken** yedek alınır (veri kaybı yönünde değil).
  **2026-11-07: "son başarılı yedek" artık kasadan okunur** (aşağıdaki bölüm); "son
  deneme" cihaz kaydında kalır (başarısız deneme kasaya yazılmaz).
- Kasa ayarı yalnız **kasa açıkken** okunur/yazılır; `settings`'in `device`/`ui`
  kapsamları bugün ürün tarafından **kullanılmıyor** (cihaz/UI ayarları localStorage'da).
- Kasa ayarı bozuksa otomatik yedek **durur** ve neden yazılır; onarım kullanıcı
  seçimiyle (klasörü yeniden seç) olur — otomatik onarım yok.
- Göç **ilk okumada bir kez** koşar; iki pencere aynı anda açılırsa ikisi de yazmayı
  deneyebilir (sürüm artar, son yazan kazanır) — eşzamanlılık kilidi yok.
- `vault_setting_*` komutları **komut olarak koşulmadı** (`State<AppState>` altyapısı
  yok): kablolama `cargo check` + clippy, davranış storage düzeyinde **gerçek SQLite** ile.
- Değer üst sınırı **8 KiB** ve anahtar biçimi `[a-z0-9._-]`: gelecekte büyük/farklı
  anahtar gerekirse sözleşme genişletilmeli.
- jsdom tarayıcı değil → gerçek Tauri penceresi, gerçek klasör seçici ve
  **görsel QA insan bekliyor**.

## Son yedek tazeliği kasadan okunur — tek kanıt, iki ekran aynı gerçeği yazar (2026-11-07)

**Ölçülen kusur (iki kaynak):** aynı ekranda iki farklı "son yedek" vardı:
"Otomatik yedek" kartındaki satır cihaz kaydından (`localStorage`, `dim.backup.last`),
"Yedek geçmişi" kartı ise kasa içi `backup_runs`tan okunuyordu. WebView verisi
temizlenince (veya aynı kasa başka bir OS profilinde açılınca) kart **"Son yedek: hiç"**
yazarken hemen altındaki geçmiş **N doğrulanmış yedek** listeliyordu. Ters yön daha
kötüydü: eski bir kasa yedeği geri yüklendiğinde cihaz kaydı "dün yedeklendi" diyor,
kasanın kendi geçmişi günler öncesini gösteriyor ve zamanlayıcı **yedeği erteliyordu**
(tazelik yanlış tarafta). `App.tsx` zamanlayıcısı da yalnız cihaz kaydına bakıyordu.

**Yazılan:**
- `lib/backup-schedule.ts`: `latestBackupSuccess(runs)` — yalnız `verified`/`succeeded`
  satırlar kanıttır; en yenisi seçilir, `completedAt` yoksa `createdAt`a düşer, tarihi
  çözülemeyen satır **atlanır**, tanınmayan durum tazelik ilerletmez (uydurma tarih yok).
  `dim.backup.force` işareti (`read/write/clearBackupForce`): "Sıfırla" kasa kaydını
  silemediği için bir sonraki kontrolü **zorunlu** yedeğe çevirir; başarılı yedekte temizlenir.
- `lib/backup-config.ts`: `loadBackupFreshness()` → `{ iso, source: vault|device|none,
  deviceIso, error }`. Kasa okunur ve başarılı satır varsa kaynak `vault`; kasa okunur ama
  satır yoksa **`none`** (cihaz kaydı olsa da tazelik iddiası kurulmaz — o kayıt başka
  kasanın olabilir); kasa **okunamazsa** cihaz kaydına düşer ve hata kodunu taşır
  (fail-open: okuma hatası yedeği durdurmaz, ama ekran "kasadan okundu" demez).
- `App.tsx`: tazelik `loadBackupFreshness()`ten; oturum içi `lastSuccessRef` başarılı
  yedekten sonra tazeliği ilerletir (saatlik kontrol aynı yedeği tekrarlamaz); kasa
  kilitlenince sıfırlanır. `dim.backup.force` varsa süre aranmaz; başarıda temizlenir.
- `Settings.tsx`: "Son yedek" satırı **kasa kaydından** yazılır ve kaynağını söyler
  (`… · kasa kaydı`); cihaz kaydı farklıysa **ayrı satırda** yazılır (çelişki gizlenmez).
  Kasa geçmişi okunamazsa "hiç" **denmez**: cihaz kaydı + hata kodu yazılır. Tazelik ve
  geçmiş **aynı istekten** beslenir (ayrı okuma yok, iki görünüm ayrışmaz). "Sıfırla"
  artık kasa kaydını silmeye çalışmaz: cihaz önbelleğini temizler, zorunlu yedek işaretler,
  "Sıradaki plan" `süresi doldu` yazar. 6 sözlük anahtarı.

**Testler:** `lib/backup-schedule.test.ts` **16 → 21** (+5) ·
`lib/backup-config.test.ts` **15 → 19** (+4) · `app-shell.test.tsx` **20 → 23** (+3; biri
"cihaz kaydı tazeyse" testinden "kasa kaydı tazeyse"ye yeniden yazıldı) ·
`Settings.test.tsx` **46 → 49** (+3; iki test yeni sözleşmeye göre güncellendi) →
yerel **63 dosya / 1287 test**.

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-freshness-gates.py`).** **12/12
mutasyon kırmızı**, her biri tek başına, geri yükleme sha256 birebir:
U1 App kasa tazeliğini yok sayar (2) · U2 zorunlu işaret yok sayılır (1) · U3 başarıda
işaret temizlenmez (1) · R1 tazelik cihaz kaydından okunur/eski davranış (2) ·
R2 okuma hatası gizlenir (2) · R3 her durum kanıt sayılır (3) · R4 en eski satır seçilir (1) ·
U4 okunamayınca "hiç" denir (1) · U5 kasa yerine cihaz yazılır (3) · U6 çelişki satırı
gizlenir (3) · U7 sıfırla zorunlu yedek işaretlemez (2) · U8 sıradaki plan işareti yok
sayar (1). **Dürüst (kendi hatam):** ilk betikte R2 iki eşleşme (paylaşılan `error:`
satırı), R4 girinti uyuşmazlığı, U8 ise **yeşil** kaldı — çünkü U8 testinde kasa geçmişi
boştu ve "süresi doldu" zaten `iso=null`dan geliyordu; test **taze kasa kaydıyla**
yeniden yazıldı, üçü de yeniden ölçüldü. U4–U8 cerrahi; U1/U2/U3/R1/R3/U5/U6 çoklu.

**Ölçüm (yerel, Node 22.23.1 + TZ=UTC):** `tsc --noEmit` temiz · vitest **1287/1287**
(63 dosya). **Dürüst:** yerel `vite build` **koşulmadı** — disk tur başında **1,3 G**'ye
inmişti (eşik altı); temizlikle 4,8 G'ye çıktı, ağır derleme yine başlatılmadı → paket
ölçümü CI'dan. Rust kodu değişmedi.

**CI (ölçüldü):** `24f9f95` (9 dosya, +579/−26) → **`42414030035` 10/10 YEŞİL**;
`arayüz` **63 dosya / 1286 test**, paket `index-5IZQZrtT.js` **634,09 kB** / gzip
**170,16 kB** (+ `index-CPW4QqlF.css` 7,28 kB). Yerel 1287 / CI 1286 farkı ağaçtaki
**başka ajanın** commit'siz `format.test.ts` testidir; `dim-storage` 437 (değişmedi).

**Dürüst sınırlar.**
- `backup_runs` yalnız **başarılı** yedekleri taşır (düşen deneme yazılmaz); tazelik en
  yeni **başarılı** yedeğe göre hesaplanır, "son deneme" cihaz kaydında kalır.
- Tazelik `backupRunList(20)` penceresinden okunur: en yeni 20 satırın hiçbiri başarılı
  değilse daha eski bir başarı görünmez (bugün yalnız `verified` yazıldığı için pratikte
  boşluk yok; satır sayısı büyürse sınır genişletilmeli).
- Kasa okunamazsa cihaz kaydına düşülür (fail-open): bu durumda **erken** yedek alınabilir
  (veri kaybı yönünde değil) ve ekran kaynağı `cihaz kaydı` diye yazar.
- `dim.backup.force` cihaz kaydındadır; uygulama verisi silinirse "sıfırla" isteği de
  kaybolur (kasa kaydı yine geçerli).
- Ayarlar ekranı tazeliği açılışta ve geçmiş tazelemesinde okur; **otomatik** yedek
  ekran açıkken çalışırsa "son yedek" satırı bir sonraki tazelemeye kadar eski kalabilir
  (canlı abonelik yok).
- jsdom tarayıcı değil → gerçek Tauri penceresi ve **görsel QA insan bekliyor**.

## CSV — güvenli hücre politikası ve belge dizini (2026-11-08)

**Ölçülen kusur 1 (formül enjeksiyonu):** `export_csv` hücreleri `csv_escape` ile
üretiyordu; fonksiyon **yalnız** `,`, `"` ve `\n` için tırnaklıyordu. `=`, `+`,
`@` ile başlayan (ya da baştaki boşluk/kontrol karakterinden sonra bunlar gelen)
hücreler **ham** yazılıyordu. Şartname §"Dışa aktarım" güvenli hücre politikasını
açıkça sayar: `=`, `+`, `-`, `@`, **tam genişlikli karşılıkları**, sekme ve satır
başı. Kronoloji başlığı `=cmd|' /C calc'!A0` olduğunda satır **satır başında
çıplak formülle** başlıyordu; tablo uygulaması bunu formül olarak yorumlayabilir.
Tek `\r` taşıyan başlık da tırnaklanmıyordu (satır yapısı bozulur).

**Ölçülen kusur 2 (eksik bölüm):** şartname CSV'nin "kronoloji, **belge dizini**
ve görev" aktardığını söyler; çıktı yalnız kronoloji + görev taşıyordu — dosyadaki
belgeler (ad, tür, durum, belge tarihi, eklenme) CSV'de **yoktu**. Biçim de
(ayraç/tarih/kodlama) yalnız belgede yazılıydı, çıktının kendisinde değil.

**Yazılan (`crates/storage/src/csv_export.rs`, yeni modül):**

- `safe_cell(value)` — hücrenin **ilk anlamlı** karakteri riskliyse başına `'`
  konur: `=`, `+`, `@`, tam genişlikli `＝ ＋ ＠`; `-`/`－` yalnız **ardından boşluk
  gelmiyorsa** (`-1+1`, `-A1`, `-cmd|…`). Baştaki boşluk/kontrol karakterleri
  (sekme, satır başı, NBSP, BOM) **atlanarak** bakılır — tablo uygulamaları onları
  kırpar. Ardından RFC 4180: `,`, `"`, `\r`, `\n` içeren hücre çift tırnaklanır,
  içteki tırnaklar ikilenir.
- `build_case_csv(coverage, events, tasks, documents)` — kapsam bloğu + **biçim
  notu** (`# biçim: ayraç=, · tarih=YYYY-AA-GG · kodlama=UTF-8 · riskli hücreler '
  ile metne sabitlenir`) + `# kronoloji` (olaylar ve görevler aynı beş sütunda) +
  `# belge dizini` (belge · tür · durum · belge tarihi · eklenme). Belge dizini
  **en sonda**: `comment='#'` ile okuyan araç ilk tabloyu kesintisiz görür.
- `export_csv` artık ham metni kendisi yazmaz; `dim_storage::build_case_csv`
  çağırır ve belge listesini de kasadan okur (`list_documents`).

**Testler:** `csv_export::tests` **4** (yeni) → `dim-storage --lib` **438 → 442**.

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-csv-gates.py`, her mutasyon tek
başına, geri yükleme sha256 `f6cde77a03c013e6…` birebir):** **8/8 kırmızı** —
M0 eski `csv_escape` davranışı (1 passed · 3 failed) · M1 baş koruması kapalı
(2·2) · M2 tam genişlikli tetikleyiciler tanınmaz (2·2) · M3 tire kuralı
daraltılmaz (3·1) · M4 `\r` tırnaklanmaz (3·1) · M5 baştaki boşluk atlanmaz (3·1) ·
M6 belge dizini yazılmaz (2·2) · M7 içteki çift tırnak ikilenmez (2·2).

**Ölçüm (yerel):** `cargo fmt --all -- --check` temiz · `clippy -p dim-storage
--all-targets -- -D warnings` temiz · `cargo test -p dim-storage --lib` **442/442**
(212,06 s). Ağır masaüstü derlemesi yerelde koşulmadı (disk 5,0 G — eşik) →
kablolama CI'da derlendi.

**CI (ölçüldü):** `d8daf54` → **`42433650618` 10/10 YEŞİL**: `dim-storage` **442**
hem `ubuntu-latest` (295,20 s) hem `macos-15` (198,49 s) — birebir; `masaüstü kabuk`
`40 passed · 5 ignored` + kapı "40 koştu · 0 düştü · 5 yoksayıldı (kaynak 45)" +
yoksayılan koşu `5 passed`; `arayüz` **1296 test** (değişmedi), diğer altı iş yeşil.

**Dürüst sınırlar:** (a) **Excel/LibreOffice aç–kaydet–yeniden aç turu
yapılmadı** — bu makinede Excel yok; şartname F14-T04'ün bu yarısı **insan/ortam
bekliyor**, kod tarafı testle sabit; (b) `'` öneki bazı okuyucularda **görünür**
(hücre metni bir karakter değişir) — veri kaybı yok, amaç formülün
çalışmaması; (c) tire kuralı bilinçli daraltıldı: `- zamanaşımı` bozulmaz,
`-1+1`/`-A1`/`-cmd|` korunur; `-` **ardından boşluk** gelen bir formül varsa
korunmaz (Excel böyle bir hücreyi formül saymaz); (d) belge dizini **kimlik/özet
taşımaz** (bunlar JSON/DOCX/PDF'te); (e) **UTF-8 BOM eklenmedi** — Windows
Excel'de Türkçe karakterlerin bozulması riski sürüyor; BOM, `comment='#'`
okuyucularını `utf-8-sig`'e mecbur bırakır → **ürün kararı** olarak açık
bırakıldı; (f) `export_csv` **komut olarak koşulmadı** (`State<AppState>`
altyapısı yok) — kablolama CI'da derlenir, davranış storage düzeyinde gerçek
nesnelerle test edildi; (g) gerçek Tauri penceresi ve **görsel QA insan bekliyor**.

## Dışa aktarma kaynağı: hangi belge sürümlerinden üretildi (2026-11-10)

**Ölçülen kusur (şartname §16.3, §433).** Şartname "`exports` ve `export_items`
kullanılan şablonu, **kaynak sürümlerini**, çıktının hash'ini ve **maskeleme
durumunu** kaydeder" der. Ölçüm: `record_export` **beş** yolun hepsinde
(`json`, `csv`, `docx`, `pdf`, `package`) `included_ids` için boş dizi (`[]`)
yazıyordu, `export_items` tablosuna **tek satır** yazılmıyordu ve
`exports.template_version` NULL kalıyordu. Sonuç: avukat "bu rapor hangi belge
sürümlerinden üretildi, o sürüm maskeli kopya mıydı" sorusunu yanıtlayamıyordu;
sicil yalnız "bir dosya çıktı" diyordu.

**Yazılan (şema değişikliği yok — tablo ilk şemadan beri var):**

- `record_export(case_id, type, template_version, items, …)`: kaynak kalemleri
  **yazmadan önce doğrulanır**, sonra tek işlemde yazılır. Kapılar: sürüm var
  olmalı, **aynı dosyaya** ait olmalı, mükerrer olmamalı, `redaction_state`
  kapalı kümede (`none|masked`), meta alanları (sayfa aralığı ≤32, atıf etiketi
  ≤64, şablon sürümü ≤64) kontrol karakteri taşımamalı ve sınırı aşmamalı.
  Düşerse **`E_EXPORT_SOURCE`** ve **hiçbir satır yazılmaz** (yarım iz kalmaz).
- `included_ids` artık **kalemlerden türetilir** (iki liste ayrışamaz);
  `template_version` gerçek biçim sürümünü taşır (`dim-case-export@1`,
  `case-csv@1`, `case-report@1`, `transfer-package@1`).
- `export_sources` (komut katmanı): her belgenin **güncel sürümü**; sürüm
  `redaction_plans.output_version_id` ile işaretliyse `masked` — **belge adına
  bakılmaz**. `masked_output_version_ids(case_id)` dosya kapsamlıdır.
- Okuma: `export_items_list` + `ExportItemRow` (belge adı, sürüm no, maskeleme);
  `ExportRow.item_count` alt sorguyla **tek istekte** gelir (ayrı sayaç isteği
  yok — aynı olgu iki istekten okunmaz).
- Arayüz: sicil satırında "Kaynak: N belge sürümü", kalemsiz eski kayıtta
  "Kaynak sürüm yazılmamış (eski kayıt)"; "Kaynakları göster" **istek üzerine**
  okur (`itemsSeq` geç yanıt kapısı), düşen okuma "kaynak yok" **demez**, dosya
  değişiminde panel kimliği düşer (aynı dosyaya geri dönüşte kendiliğinden
  açılmaz).

**Testler:** `exports::tests` **6 → 12** (+6: kalem yazımı + `included_ids`
türetimi, yabancı/bilinmeyen sürüm reddi (satırsız), mükerrer + bilinmeyen durum,
meta sınırı/kontrol karakteri, boş meta → NULL, kalemsiz eski kayıt + `item_count`);
`redactions::tests` **+1** (`masked_output_version_ids` dosya kapsamı) →
`dim-storage --lib` **510 → 517**. `Exports.test.tsx` **59 → 64** (+5) → yerel
arayüz **65 dosya / 1351 test**.

**Kapı kanıtı (gerçek koşular; betik `dim-export-source-gates.py`, her mutasyon
tek başına, `try/finally` + sha256 birebir geri yükleme):** **17/17 KIRMIZI** —
S1 `included_ids` kalemlerden türetilmez · S2 sürüm sahipliği denetimi kapalı ·
S3 maskeleme durumu kümesi kapalı · S4 `item_count` 0 · S5 şablon sürümü
yazılmaz · S6 maske izi dosya kapsamını yok sayar · S7 kalem sırası ters ·
S8 `export_items` hiç yazılmaz · S9 kontrol karakteri denetimi kapalı ·
S10 boş meta NULL yerine boş dize · U1 kaynak sayısı satırı yok · U2 eski kayıt
"kaynak yazılmamış" demez · U3 kaynaklar çekirdekten okunmaz · U4 maskeli sürüm
maskesiz yazılır · U5 düşen okuma yutulur · U6 geç yanıt kapısı yok ·
U7 dosya değişiminde panel kapanmaz. **Dürüst (kendi hatam, iki tanesi):** ilk
ölçümde **S4 EŞLEŞMEDİ** (aynı alt sorgu `get_export` ve `list_exports`ta iki kez
geçiyor; betik tek eşleşme bekliyordu → `expected=2` ile yeniden koşuldu) ve
**U7 YEŞİL kaldı** (senaryo p1→p2 geçişinde paneli ölçüyordu; yeni listede eski
kimlik olmadığı için panel zaten görünmüyordu). Test **p1→p2→p1 geri dönüşüyle**
güçlendirildi; `setItemsFor(null)` dışındaki temizlik satırları yük taşımadığı
için **koddan çıkarıldı**; iki kapı yeniden ölçüldü → kırmızı.

**Dürüst sınırlar:** (a) `output_page_range` ve `citation_label` **NULL**:
raporun hangi sayfasına hangi belgenin düştüğü worker çıktısında eşlenmiyor —
uydurma numara/etiket yazılmadı; (b) kalemler belgenin **güncel** sürümünü
gösterir; çıktı üretilirken kullanılan sürüm ayrıca sabitlenmiyor (export ile
kayıt arasında yeni sürüm eklenirse iz o anki sürümü gösterir — sıkıştırma yok);
(c) `masked` ölçüsü `redaction_plans.output_version_id`dir: planı olmayan ama
elle eklenmiş maskeli kopya `none` görünür; (d) eski kayıtlar (kalemsiz) "kaynak
yok" cümlesiyle ayrılır, geriye dönük **doldurma yapılmadı** (hangi sürümün
kullanıldığı kanıtlanamaz); (e) komutlar **komut olarak koşulmadı**
(`State<AppState>` altyapısı yok) — kablolama check+clippy, davranış storage
düzeyinde gerçek SQLite; (f) jsdom tarayıcı değil → gerçek Tauri penceresi ve
**görsel QA insan bekliyor**.

**CI (ölçüldü):** kod `d160521` (10 dosya, **+962/−22**) → run `42823128994`
**10/10 YEŞİL**: `dim-storage` **517** hem `ubuntu-latest` (368,36 s) hem
`macos-15` (203,23 s) — **birebir**; `arayüz` **65 dosya / 1350 test** (yerel
1351 farkı ağaçtaki başka ajanın commit'siz `format.test.ts` testi), paket
`index-CbO_Ee7q.js` **665,39 kB** / gzip **177,97 kB** (CSS 7,28 / 2,07);
`masaüstü kabuk` `40 koştu · 0 düştü · 5 yoksayıldı` + yoksayılan koşu
`5 passed · 40 filtered`; diğer altı iş yeşil.

## Yedek saklama — son N günlük + M haftalık, yalnız kendi yedeklerimiz (2026-11-11)

**Ölçülen kusur:** her otomatik yedek hedef klasörde `DimYedek-…` klasörü **ve**
varsa `.dimbak` arşivi bırakıyordu; **hiçbir temizlik yolu yoktu**. Şartname
(Bölüm 05.5, F14-T09) "son 7 günlük ve 4 haftalık yedek saklama önerisi
değiştirilebilir; disk doluluğu görünürdür" der. Sekiz gigabaytlık hedef cihazda
haftalık yedekler zamanla diski doldurur — **dolan disk yeni yedeği de engeller**
(yedekleme kendi kendini durdurur). Dahası: yedek manifestinde **kasa kimliği
yoktu**, yani temizlik "hangi yedek benim" sorusunu kanıtlayamazdı; yanlış
kurgulanmış bir süpürme **başka kasanın** yedeğini silerdi.

**Yazılan (çekirdek):**
- `backup.rs`: manifeste **`vaultId` aidiyet izi** (`#[serde(default)]` — eski
  yedekler kimliksizdir ve **silinmez**); `write_backup_manifest_with_vault`
  (üretim yolu `vault_backup` kasa kimliğini geçirir, eski imza `None` ile
  korunur); `read_backup_manifest` **biçim/sürüm kapısı** (tanınmayan biçim ve
  ileri sürüm yedek sayılmaz, silinmez); `NAME_PREFIX` / `ARCHIVE_EXT` sabitleri
  (komut ad biçimini iki yerde ayrı yazmaz).
- Yeni `backup_retention.rs`: `RetentionPolicy` (**günlük 1–365, haftalık
  0–52**; "hiç yedek tutma" yoktur — yanlış yapılandırma tüm yedekleri
  silemez), `plan_retention` (tarama + plan; **hiçbir dosya silinmez**) ve
  `apply_retention` (yalnız `dry_run=false`).
- **Güvenlik kuralları (kaldırılamaz):** yalnız manifestteki `vaultId` bu
  kasanın kimliğiyle **birebir** eşleşen yedek silinir; kimliksiz/başka kasanın
  yedeği **asla**; symlink, klasör olmayan girdi ve manifesti bozuk/ileri sürüm
  klasör yedek sayılmaz (`ignored` — sessizce yutulmaz); **en yeni yedek her
  zaman korunur**; **önce `.dimbak`, sonra klasör** silinir (arşiv silinemezse
  klasöre dokunulmaz); silme anında manifest **yeniden** okunur (tarama ile
  silme arasında kimlik değiştiyse hiçbir şey silinmez); silinemeyenler
  `errors`ta adı+nedeniyle raporlanır; `freedBytes` yalnız **gerçekten** silinen
  yedeklerin ölçülen baytıdır.
- Komut `vault_backup_retention(dryRun, keepDaily, keepWeekly)`: hedef **kasadaki
  `backup.schedule` ayarından** okunur — komut keyfi bir yol üzerinde dosya
  silmez; hedef yoksa `E_BACKUP_NO_TARGET`; denetim kaydı yalnız sayı taşır
  (`count`, `freedBytes`, politika), **dosya adı/yolu yazılmaz**.

**Yazılan (masaüstü + arayüz):** `api.vaultBackupRetention` +
`BackupRetentionReport` tipleri; Ayarlar'da **"Yedek saklama"** kartı: cihaz
tercihi (localStorage; sınırlar çekirdekle **aynı**), "Yedekleri tara" (dry run)
→ önizleme tablosu (satır durumları: **silindi / silinecek / korunuyor /
dokunulmadı**; korunan sayısı ve yedek sayılmayan girdiler ayrı cümleyle) →
"Silinecekleri sil" → **onay paneli** ("N yedek ve arşivleri kalıcı olarak
silinecek… geri alınamaz") → "Onayla ve sil". `retentionSeq` geç yanıt kapısı;
**politika veya hedef değişince ve kasa kilitlenince rapor düşer** (dosya
adları/boyutları kilitli ekranda kalmaz); bozuk sayı **çekirdeğe gitmez**;
çekirdeğin kabul ettiği sayılar cihaz tercihine **geri yazılır**.

**Testler:** `backup_retention::tests` **9** (ölçüm/arşiv · gün+hafta planı · en
yeni koruması · kimliksiz/yabancı · gerçek silme + boşalan bayt + idempotent ·
kimlik değişince ret · geçersiz politika/kök · symlink · **arşiv dosya değilse
klasör silinmez**) → `dim-storage --lib` **535 → 544**; `backup-retention.test.ts`
**5**; `Settings.test.tsx` **49 → 57** (+8) → yerel **66 dosya / 1383 test**.

**Kapı kanıtı (gerçek koşular; betikler `dim-retention-gates-core.py` ve
`-ui.py`, her mutasyon tek başına, `try/finally` + sha256 birebir —
`backup_retention.rs f9919dd8…`, `Settings.tsx a402a2d8…`):** **17/17 KIRMIZI** —
S1 kimliksiz eski yedek "own" sayılır · S2 başka kasanın yedeği "own" · S3 günlük
bütçe bir fazla tutar · S4 haftalık koruma işaretlenmez · S5 silme anında kimlik
yeniden doğrulanmaz · S6 symlink yedek sayılır · S7 arşiv bayta sayılmaz ·
S8 arşiv dosya değilse klasör yine silinir; U1 onay paneli atlanır (tek tıkla
siler) · U2 geçersiz günlük sayı sessizce 1'e tamir edilir · U3 düşen tarama
eski raporu bırakır · U4 politika değişince rapor düşmez · U5 hedef değişince
rapor düşmez · U6 kilitte rapor düşmez · U7 yabancı yedek "korunuyor" yazar ·
U8 silinen satır "silindi" yazmaz · U9 hedef yokken tarama açık.

**CI (ölçüldü):** kod `ae50498` (11 dosya, **+1697/−3**) → run `43027650809`
**10/10 YEŞİL**: `dim-storage` **544** hem `macos-15` (278,69 s) hem
`ubuntu-latest` (504,71 s) — **birebir**; `arayüz` **66 dosya / 1375 test** (yerel
1383 farkı ağaçtaki başka ajanın commit'siz testleri), paket
`index-DmhY8TBs.js` **676,72 kB** / gzip **180,77 kB**; `masaüstü kabuk`
`40 koştu · 0 düştü · 5 yoksayıldı` + yoksayılan koşu `5 passed · 40 filtered`;
diğer altı iş yeşil.

**Dürüst sınırlar:** (a) temizlik **yalnız kullanıcı eylemiyle** koşar —
otomatik zamanlanmış süpürme yok (ürün kararı); (b) politika **cihaz tercihidir**
(localStorage), kasa politikası değildir; aynı kasa başka cihazda farklı sayılarla
taranabilir; (c) **kimliksiz eski yedekler hiç silinmez** — raporda "dokunulmadı"
sayılır; üründe elle yedek silme yolu olmadığı için bu yedekler Finder'dan
silinmeli (tahminle silmek başka kasanın yedeğini silmek olabilir); (d)
"haftalık" **ISO takvim haftasıdır**; yıl sınırında aynı hafta iki gruba
düşebilir → **fazla** tutar, fazla silmez; (e) disk doluluğu ölçülür ve yazılır
(taranan toplam, boşalan bayt) ama **eşik uyarısı/tahmini yok**; (f) önce arşiv
silinir: ikisi arasında süreç ölürse klasör arşivsiz kalır, sonraki tarama onu
yine bu kasanın yedeği sayar; (g) yalnız `backup.schedule` hedefi taranır — elle
seçilen başka klasörlerdeki yedekler kapsam dışıdır; (h) manifest zamanı
okunamazsa ad damgası kullanılır (uydurma değil, adın taşıdığı zaman); (i) komut
**komut olarak koşulmadı** (`State<AppState>` altyapısı yok) — kablolama
check+clippy, davranış storage düzeyinde gerçek dosya sistemi; (j) jsdom tarayıcı
değil → gerçek Tauri penceresi + **görsel QA insan bekliyor**.
