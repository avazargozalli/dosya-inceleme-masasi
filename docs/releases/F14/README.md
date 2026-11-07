# F14 — Çıktılar, paket, yedek

- **Sürüm etiketi:** `v0.13.0`
- **Durum:** Kısmi
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F14-T01 JSON dışa aktarma | ✅ | `export_json`, tam metadata, hash kaydı |
| F14-T02 CSV | ✅ | kronoloji + görevler, kaçışlı CSV |
| F14-T03 DOCX rapor | ✅ | `render_report`, tablolar + notlar |
| F14-T04 PDF rapor | ✅ | `render_report_pdf`, HTML kaçışlı, `Story` ile sayfalama |
| F14-T05 dışa aktarma sicili | ✅ | `exports` tablosu + denetim kaydı; **okunur**: `exports_list` + Çıktılar ekranında defter (tür, hedef yol, zaman, özet, durum) |
| F14-T06 şifreli devir paketi | ✅ | klasör → zip → Argon2id + AES-256-GCM `.dimpak`; parola verilmezse açık klasör kalır |
| F14-T07 yedek zamanlaması / bildirim | ✅ | Otomatik yedek: klasör + 7 gün aralık + son tarih; App saatte bir kontrol eder, Settings kartında **son başarılı yedek · son deneme (neden koduyla) · sıradaki plan** ayrı gösterilir; başarısız deneme tazeliği ilerletmez |
| F14-T08 PDF çıktısında Türkçe font gömme | ✅ | Gömülü **Charis SIL** (Type0/Identity-H); `test_pdf_report_embeds_unicode_font_and_keeps_turkish` |

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
