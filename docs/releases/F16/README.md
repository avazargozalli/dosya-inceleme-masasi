# F16 — Dağıtım (imzasız)

- **Durum:** Kapsam daraltıldı (ADR-0012)
- **Tarih:** 2026-10-23

| İş | Durum | Not |
|---|---|---|
| F16-T01 kaynak etiketi + kilit dosyaları + SBOM + üçüncü taraf bildirimleri | 🚧 | **SBOM tarafı tamam (2026-10-28)**: kapsam gerçek (üretim **7** + geliştirme **185** node bileşeni, geçişli dahil; rust 159; python 5) ve ürüne bağlı (sürüm/commit/üç kilit sha256); kapı `scripts/sbom_check.py` + `--self-test` CI'da. **Üçüncü taraf lisans bildirimi tamam (2026-10-28)**: `THIRD_PARTY_NOTICES.md` üretilir — kapsam **665** bileşen (578 ölçülen + 87 lisansı ölçülemeyen; rust **485** = iki cargo çalışma alanı), **224** benzersiz lisans metni (2,0 MB), 36 bileşen metin taşımıyor, 21 ifade; kapı `scripts/third_party_notices.py --check` (10 kural, SPDX 3.29.0, SBOM iki yönlü eşleşme) + `--self-test` CI'da; gerçek dosyada 8/8 mutasyon kırmızı; CI `40756834820` 7/7 yeşil. **Kaynak etiketi/arşivi tamam (2026-11-06)**: `scripts/source_release.py` commit ağacından `git archive` ile deterministik içerik üretir, manifest sürüm+commit+etiket+dört kilit (`Cargo.lock`, `apps/desktop/src-tauri/Cargo.lock`, `pnpm-lock.yaml`, `pyproject.toml`)+bildirim özeti+SBOM `subject` bağını taşır; `check` bağımsız doğrular (dosya kümesi = `git ls-tree -r -z`, kilit özetleri arşivin içinden, sır/çöp yolu reddi). Ölçüm: 431 dosya / 2.260.254 bayt, açılmış tar özeti `b58a6136…d082` ubuntu CI ve macOS'ta **birebir**; `.tar.gz` özeti zlib sürümüne bağlı olduğu için kimlik tar özetidir. CI `kaynak` işi her push'ta üretir+doğrular, `kaynak-arsivi` artefaktı bırakır. Kapı kanıtı 13/13 mutasyon. **Kalan:** git etiketi depoda yok — etiket oluşturmak yayın adımı, `--tag` bağı yalnız fixture'da ölçüldü. Ayrıntı: `docs/releases/F16/SOURCE-RELEASE.md`; kusur #60, #63–#68 `docs/releases/F15/README.md` |
| F16-T02 güncelleme imza doğrulama | 🚧 | mekanizma var, anahtar yok; `plugins.updater.active=false` |
| F16-T03 atomik migrasyon geri dönüş | ✅ | `rollback_vault` (sabit bağlantılı güvenlik kopyası, dolu `-wal` kapısı, artık süpürme) + uygulama içi akış (`pre_update.rs`, Ayarlar kartı, onay metni); `ROLLBACK-DRILL.md` — **süreç öldürme ve gerçek disk dolması tatbikatı** (kusur #39–#41); arayüzde onay **seçilen yedeğe bağlı** (2026-11-04, aşağıdaki ek tur) |
| F16-T04 Apple Developer ID + notarization | ➖ | **kapsam dışı** (ADR-0012) |
| F16-T05 Windows kod imzası | ➖ | **kapsam dışı** (ADR-0012) |
| F16-T06 imzalı `.dmg` / `.msi` | ➖ | **kapsam dışı**; imzasız paket üretiliyor |
| F16-T07 güncelleme öncesi yedek + işlemli migrasyon | ✅ | `upgrade_with_backup`: yedek doğrulanmadan migrasyon başlamaz |
| F16-T08 eski uygulama yeni şemayı bozmasın | ✅ | `E_DB_SCHEMA_NEWER`, yazmadan reddeder (testli) |
| F16-T09 hatalı güncellemeyi reddet | 🚧 | bozuk yedek reddedilir; kesilen indirme/yanlış mimari denenmedi (updater kapalı) |

## Dürüst durum

Paketler **imzasızdır**; macOS Gatekeeper ve Windows Defender uyarı gösterir.
Kaynak açık olduğu için isteyen kendi sertifikasıyla imzalar. Bu depo hiçbir
zaman "imzalı" ya da "notarize edilmiş" sürüm iddia etmez.

## Kullanıcı kendi imzasını nasıl ekler

1. `apps/desktop/src-tauri/tauri.conf.json` → `bundle.macOS.signingIdentity`
   (veya `APPLE_SIGNING_IDENTITY` ortam değişkeni) ve Windows için
   `certificateThumbprint`.
2. Güncelleme dağıtacaksa `plugins.updater.pubkey` ve `endpoints` doldurulur,
   `active=true` yapılır.
3. `desktop-build.yml` imzalama sırlarını ortam değişkeni olarak geçirir;
   sırlar depoda tutulmaz.

## F16-T03 ek turu — geri dönüş onayı seçilen yedeğe bağlandı (2026-11-04)

Ayarlar ekranı (`apps/desktop/src/screens/Settings.tsx`, 642 satır, 17 test)
kasanın **en yıkıcı** işlemini yürütür: güncelleme öncesi yedeğe geri dönüş.
Bugüne dek ölçülenler kararlar, çizim ve bayat bildirimlerdi; **uçuş** yolları
(seçim değişirken, istek askıdayken, tazeleme düşerken) hiç ölçülmemişti.

**Ölçülen kusurlar (düzeltme öncesi bileşende 8/8 kırmızı, mevcut 17 test yeşil):**

- **Onay hedefe bağlı değildi (en ağırı):** onay metni ("GERİ DÖN") yalnız
  metin olarak doğrulanıyordu; A yedeği seçiliyken yazılan onay, B yedeğine
  geçildikten sonra da "Seçili yedeğe dön" düğmesini **açık** tutuyordu.
  Çekirdek (`vault_rollback_pre_update`) yalnız metni bilir, hedefi bilmez →
  tek tıkla **kullanıcının onaylamadığı** bir yedek geri konabiliyordu
  (yanlış sürüm + ad uyuşmazlığı uyarısı taşıyan yedek dâhil).
- **"Yedekleri tara" aynı kapıyı atlıyordu:** tarama ilk kullanılabilir yedeği
  kendiliğinden seçiyor; seçim değişse de yazılan onay kalıyordu.
- **Tazeleme düşerse bayat liste:** geri dönüş başarılı olup liste okuması
  düşünce (E_IO) eski satırlar seçilebilir kalıyordu — kullanılmış yedek
  yeniden hedef gösterilebiliyordu. Artık liste **fail-closed** boşaltılır,
  "Geri dönüş tamamlandı" paneli korunur (işlem gerçekten oldu).
- **Parola: uçuşta yazılan siliniyordu:** başarı yanıtı üç alanı koşulsuz
  temizliyordu; kullanıcı yanıt beklerken yeni parola yazıyorsa metni
  sessizce kayboluyordu. Artık yalnız **gönderilen değer** hâlâ oradaysa
  temizlenir.
- **Parola başarı satırı bayatlıyordu:** alanlar değişince "Parola
  değiştirildi." satırı güncel sonuçmuş gibi ekranda kalıyordu.
- **Durum yalanı:** kasa açıkken ve henüz yedek alınmamışken yedek kartı
  "Kasa kilitliyken yedek alınamaz" yazıyordu.
- **Sessiz seçici hatası:** klasör seçici (`@tauri-apps/plugin-dialog`)
  reddederse ekranda hiçbir şey olmuyordu (ne hata ne istek). İki seçici de
  (yedek, geri yükleme) artık `E_DIALOG` yazar.

**Düzeltme (`Settings.tsx` +72/−23, `i18n.ts` +2):** `selectBackup()` —
seçim değişince onay metni ve sonuç paneli düşer; radyo, tarama ve geri dönüş
sonrası tazeleme **aynı** yardımcıdan geçer. Geri dönüşün tazelemesi ayrı
`try` kapısında: işlem başarısı paneli korur, tazeleme düşerse liste boşaltılır.
`changePassword()` gönderilen değerleri saklar ve alanları yalnız eşitse
temizler; üç alanın `onChange`i başarı satırını siler. `pickDirectory()`
seçici hatasını `E_DIALOG` ile görünür kılar (üç çağrı yeri: yedek, geri
yükleme, otomatik yedek klasörü). Yeni sözlük anahtarları: `settings.backupNone`,
`settings.pickDirFailed`.

**Testler (`Settings.test.tsx` 17 → 25, +8):** uçuş yolları için ayrı
`describe`; `deferred<T>()` ile askıda tutulan yanıtlar. Sabitlenenler:
onay hedefe bağlı (seçim değişince düşer, yeniden yazılınca **B** için gider);
tarama seçimi değiştirirse onay düşer; başarılı geri dönüşte tazeleme düşerse
satır kalmaz + panel kalır; parola uçuşunda yazılan korunur; başarı satırı
alan değişince kalkar; açık kasada "kilitli" yazmaz; iki seçicinin hatası
görünür ve istek üretmez.

**Kapı kanıtı: 8/8 tek kapı mutasyonu kırmızı.** M1 seçimde onay temizliği
(**2**), M2 taramada onay temizliği (1), M3 tazeleme düşünce listeyi boşaltma
(1), M4 koşulsuz alan temizliği (1), M5 alan değişiminde bildirim temizliği
(1), M6 yedek kartı durum metni (1), M7 seçici hatası sessiz — yardımcı (2),
M8 geri yükleme seçicisi yardımcıyı atlar (1). **Dürüst:** M8 ilk koşuda
**yeşil kaldı** — test iki seçiciyi tek senaryoda ölçüyordu ve geri yükleme
tıklaması, yedek tıklamasının bıraktığı `E_DIALOG` bandını görüyordu; test
ayrıldı, M8 kırmızı verdi. Mutasyon betiği her koşudan sonra dosyayı yedekten
geri yükledi; geri yükleme sha256 `b7b943d3…fbd43` birebir.

**Ölçüm:** `tsc --noEmit` temiz; `vitest` **982/982** (60 dosya; 974 → +8);
`npm run build` temiz (**581,16 kB**, `index-Bu15KLCp.js`, gzip **156,71 kB**).
Rust değişmedi; rustfmt iki manifestte `--check` temiz.

**Dürüst sınırlar:**

- jsdom **tarayıcı değil**: gerçek Tauri penceresi, gerçek yerel klasör
  seçicisi ve **görsel QA yok**; `api` mock'ludur, yanıt **sırası** elle
  çözülen sözlerle denetlenir, gerçek gecikme değil.
- Onay bağlama **arayüz düzeyindedir**: çekirdek hâlâ yalnız metni doğrular.
  Arayüz dışından (doğrudan IPC) çağrı yapan biri bu bağı atlayabilir; çekirdek
  kapısı `rollback_confirmed` olarak kalır (tasarım gereği: onay metni sabit).
- `busy` tek bayrak: parola değişimi sürerken alanlar **açık** kalır (yazılan
  korunur) ama düğmeler kapalıdır; iki ağır işlemin gerçek paralelliği
  ölçülmedi (kapılar çakışmayı engeller).
- Yerel klasör seçici gerçek Tauri'de **modal**dır; çift tıklayla iki pencere
  açılması bu yüzden ölçülmedi (jsdom'da modal yok).
- `E_DIALOG` arayüzün yerel hata kodudur (çekirdekten gelmez); `E_PATH`,
  `E_PASSWORD_MISMATCH` ile aynı sınıf.
- Kimlik: yeni numara uydurulmadı — F16 listesindeki **T03** (atomik migrasyon
  geri dönüşü) kullanıldı; bu tur onun arayüz onay kapısının sertleştirilmesidir.
