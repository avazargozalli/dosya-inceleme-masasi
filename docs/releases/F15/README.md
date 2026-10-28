# F15 — Kalite, güvenlik, pilot

- **Durum:** Kısmi (B-01 ölçüldü; pilot ve ekran okuyucu turu yok)
- **Tarih:** 2026-10-25 (B-01 tam profil ölçümü)

| İş | Durum | Kanıt |
|---|---|---|
| F15-T01 erişilebilirlik regresyonu | ✅ | `src/a11y.test.ts` — etiketsiz alan, altsız görsel, adsız buton |
| F15-T02 güvenlik test matrisi | ✅ | `docs/security/test-matrix.md` (T1–T15) |
| SBOM — şartname kimliği **F16-T01** ("F15-T03" yanlıştı: şartname F15-T03 = 8 GB cihaz / 16 GB SSD ölçümü) | ✅ **kapsam tamam** | `scripts/sbom.sh` + kapı `scripts/sbom_check.py` (CI `sbom` işi). 2026-10-28: node listesi **5 → 192** bileşen (üretim 7 + geliştirme 185), geçişli bağımlılıklar dahil; `subject` bloğu sürüm/commit/kilit sha256 taşıyor; eski kusurlu SBOM kapıda **14 nedenle kırmızı**. Eksik (F16-T01 içinde): kaynak etiketi, üçüncü taraf lisans bildirimleri. Ayrıntı: aşağıdaki "SBOM kapsamı" bölümü |
| F15-T04 performans benchmark | 🚧 | **B-01 profili ölçüldü** (1.000 belge / 100.000 segment / 200 sorgu, release): p50 **1,25 sn**, p95 **1,53 / 2,18 sn** → 2 sn hedefi sınırda ve tutarlı değil; kuyruk **36–54 sn**. Ölçümle bulunan indeks kusuru düzeltildi (ilk sayfa **0,314 ms**). **Ertelenmiş toplam sayım** uygulandı: arayüz yalnız ilk sayfayı bekler (ölçülen sayfa p50 **605 ms**, p95 **712 ms**), sayı sonradan ayrı komutla gelir ve sayfayla **birebir aynı** sayıyı verir. **Aday penceresi + tembel doğrulama** (ilk sayfa 2,28×). **Belirteç/konum indeksi ölçüldü ve uygulanmadı** (kazanç %5, depolama +%54 — gerekçe `BENCHMARK.md`). **İki aşamalı tarama** uygulandı: seyrek profilde aynı korpusta **2,6–3,1×**. **Aday daraltma** (`NEAR`) uygulandı: seyrek profilde aday 3.030 → 30, uçtan uca ilk sayfa **5,95 → 1,55 ms (3,84×)**; yan bulgu **kusur #35** (`phrase` kipinde "ıslah" sorgusu "ISLAH" metnini bulamıyordu) düzeltildi. **Kusur #36** (`exact`/Tam kipi aynı sınıfı taşıyordu; ayrıca doğrulama alt dizge, aday ifadesi FTS önekiydi) düzeltildi: adaylar `normalized_text`ten, doğrulama belirteç başına çapalı; ölçülen `exact` ilk sayfa **1,56 ms**, aday 21 (düzeltme maliyet getirmedi). **Kalan FTS tabanı (0,71 ms / 3.030 satır) kapatıldı**: belge/sayfa ön elemesi `MATCH` taramasını kısaltmaz, kaba özet indeksi sınıfı ölçülüp reddedilmişti. 100.000 **belge** / 1 milyon sayfa kapasite testi **yapılmadı**. Ayrıntı: `BENCHMARK.md` |
| F15-T05 pilot (5–8 avukat) | ⬜ | **insan gerekir**; protokol `docs/pilot/` |
| F15-T05 iş kuyruğu ölçümü | ✅ | **Ölçüldü (2026-10-26)**: 400 iş, release, Apple M2 — sahiplenme p50 **1,016 ms** / p95 4,588; sıraya koyma p50 **0,219 ms**; sahiplen→bitir döngüsü p50 **11,365 ms** (2 ms sentetik iş dahil); dolu kuyrukta bekleme p50 **2.238,9 ms** / p95 4.296,3 (400 × 11,4 ms). Ham rapor `bench-queue-2026-10-26.json`, ayrıntı `BENCHMARK.md`. Kuyruk yürütücüsü bu turda gerçek oldu (F03-T07/T08) |
| F15-T06 VoiceOver/NVDA turu | ⬜ | statik test var, ekran okuyucu turu yapılmadı |
| Disk dolması (spec F15-T06 senaryosu) | 🚧 **kısmi** | **Disk dolması** ölçüldü ve düzeltildi (bu tur): gerçek 8 MB APFS imajı dolduruldu, `backup_to`/`write_atomic`/`copy_atomic` `E_DISK_FULL` verdi, **hedefte kalıntı 0**, kaynak kasa `integrity_check` temiz; geri yükleme artık **önce manifesti doğruluyor** (boyut+sha256) ve hazırlık dizininden tek `rename` ile yerleşiyor. Ölçüm: `crates/storage/tests/disk_full.rs` (`#[ignore]`, elle). **Kalan:** süreç öldürme (kısmen F01-T10'da), uyku/uyanma, harici disk kopması — donanım/insan gerekir. Defter #37/#38 |

## Ölçüm yayını (spec F15-T05)

Spec'teki "arama, ilk sayfa, OCR, bellek ve iş kuyruğu p50/p95" maddesinin
durumu:

- **Arama** — yayımlandı (200 sorgu, p50/p95/max + grup kırılımı).
- **İlk sayfa** — yayımlandı (100 sayfalık sürüm, p50 0,314 ms).
- **Bellek** — yayımlandı (tepe RSS 10,3 MB, arama sonrası 21,1 MB).
- **OCR** — **yayımlanmadı** (worker bu koşuda devrede değil; Tesseract bu
  makinede kurulu değil).
- **İş kuyruğu** — **yayımlandı (2026-10-26)**: sahiplenme, sıraya koyma ve
  dolu kuyrukta bekleme p50/p95 ölçüldü (`BENCHMARK.md` → "İş kuyruğu
  gecikmesi"). Ölçüm **çekirdek**; Python worker ve IPC yok, sentetik iş
  sabit 2 ms.

Ayrıca 8 GB çekirdek profili ve anti-virüs/uyku gibi senaryolar (spec F15-T03/
T06) bu makinede denenmedi; Windows/Linux ölçümü yok.

## Dürüst durum

Otomatik erişilebilirlik kuralları yalnız yapısal hataları yakalar; gerçek ekran
okuyucu deneyimi ve pilot geri bildirimi olmadan F15 "tamam" sayılamaz. Performans
tarafında B-01 profili artık **ölçülmüş** durumda: p50 hedefin altında, p95
sınırda, kuyruk (tek sorguda 36–54 sn) hedefin çok üstünde. Bu yüzden kapasite
iddiası "100.000 segmentte ilk 50 sonuç tipik olarak ~1,25 sn, en kötü tek
sorguda dakikalar" biçiminde daraltılmalıdır; hedefi tutturmak için güvenlik veya
doğrulama adımı kaldırılmaz.

## SBOM kapsamı (2026-10-28, kusur #60)

**Ölçülen kusur (işlevsel, sessiz eksik çıktı):** `scripts/sbom.sh` `pnpm list -r
--depth 0` çağırıyor ve yalnız `dependencies` anahtarını okuyordu → **geçişli**
bağımlılıklar ve **bütün `devDependencies`** SBOM'a hiç girmiyordu. Ölçüm (kurulu
ağaç, node 22.23.1): SBOM'da node bileşeni **5**; gerçek üretim kapanışı **8**,
geliştirme kapanışı **186**, birleşim **193** → kapsam **%2,6**. CI kapısı
`counts.node > 0` dediği için kusur **yeşil** geçiyordu. Aynı sınıftan iki küçük
kusur: `@dim/contracts@link:../../packages/contracts` "sürümler sabittir"
iddiasına aykırı bir sözde sürümdü ve 10 çalışma alanı crate'i üçüncü taraf
bileşen gibi listeleniyordu.

**Düzeltme — üretim (`scripts/sbom.sh`):**

- `pnpm list -r --depth Infinity`; ağaç özyinelemeli yürünür, `dependencies` ve
  `devDependencies` **ayrı** listelenir (`node`, `nodeDev`); `link:`/`file:`
  sözde sürümleri sürüm listesine girmez (`nodeWorkspace`).
- Rust: kayıt defteri bileşenleri (`rust`) ile çalışma alanı üyeleri
  (`rustWorkspace`) ayrıldı. Python: çalışma zamanı pinleri (`python`) ile `dev`
  grubu (`pythonDev`) ayrıldı.
- **Ürüne bağlama:** `subject` bloğu uygulama sürümü, paket (`tauri.conf.json`)
  sürümü, Cargo çalışma alanı sürümü, git commit'i ve üç kilit dosyasının
  (`Cargo.lock`, `pnpm-lock.yaml`, `pyproject.toml`) sha256 özetini taşır
  (F16-T01'in "SBOM'ı sürüme bağla" maddesi için zemin).
- Çıktı deterministik: listeler sıralı, `sort_keys=True`; tek değişken alan
  `generatedAt` (iki koşu bunun dışında **birebir** aynı — ölçüldü).

**Düzeltme — kapı (`scripts/sbom_check.py`, yeni; CI'da iki adım):**

1. `counts` ile liste uzunlukları birebir (sayaç şişmesi/sessiz kırpma yok).
2. Her bileşen **sabit semver**; `latest`, `*`, `^`, `~`, aralık reddedilir.
3. Çalışma alanındaki her `package.json`'un bildirdiği bağımlılık (üretim +
   geliştirme) SBOM'da **adıyla** bulunur → `devDependencies` atlanamaz.
4. Geçişli bağımlılık kanıtı: node bileşen sayısı bildirilen bağımlılık
   sayısından **büyük** olmalı → `--depth 0` kusuru geri gelirse kırmızı.
5. `subject.version = bundleVersion = cargoVersion` (semver), commit 40 hane,
   üç kilit özeti sha256.
6. Python pinleri `ad==sürüm` biçiminde.

`--self-test` (CI'da koşar) altı fixture üretir: tam SBOM yeşil; **eksik
devDependency**, **`latest`**, **`--depth 0`**, **sürüm kayması** ve **sayaç
uyuşmazlığı** kırmızı. Eski (kusurlu) SBOM aynı kapıda **14 nedenle** kırmızı
döndü — kapı gerçekten yakalıyor.

**Ölçüm (yeni çıktı):** rust **159** (+10 çalışma alanı), node **7** üretim +
**185** geliştirme (+1 bağ), python **5** (+1 geliştirme); `subject` sürümü
`0.1.0` (üç kaynakta aynı), commit 40 hane, üç kilit özeti dolu.

**Dürüst sınırlar:** node listesi **kurulu ağaçtan** gelir → platforma özgü
opsiyonel paketler yalnız o platformda görünür (CI linux üretir; macOS listesi
farklı ve bu yüzden SBOM platforma göre değişir, platform etiketi yok).
`pnpm install` yapılmadan üretilen SBOM boş node listesi verir ve kapı **kırmızı**
olur (sessiz geçmez). Python tarafı yalnız **bildirilen** pinlerdir; geçişli
Python bağımlılıkları SBOM'da yok (paketlenmiş worker içeriği
`scripts/worker_manifest.py` ile ayrı doğrulanır — release'te birleştirilmeli).
Lisans/üçüncü taraf bildirimi ve kaynak etiketi yok (F16-T01'in kalanı). Kök
`package.json` sürümü **0.2.0**, uygulama **0.1.0**: subject yalnız uygulama/paket/
Cargo sürümünü taşır, monorepo sürümü hizalanmadı (sürüm hizalama işi açık).
SBOM imzalanmıyor, attestation yok.

## Kimlik eşlemesi (2026-10-28, kusur #61)

Bu defterdeki `F15-Txx` kimlikleri şartname §20.17 listesiyle **birebir
eşleşmiyordu**; faz kapısının istediği "iş kimliği + REQ eşlemesi" bu yüzden
kanıtı yanlış işe bağlıyordu. Şartname karşılıkları (ölçüldü, tahmin değil):

- **F15-T01** altın korpus sürümleme → **karşılık yok** (`fixtures/synthetic/`
  kurgu belgeler var, altın kayıt yok). Tablodaki "F15-T01 erişilebilirlik"
  satırı aslında **F15-T09**'un statik kısmıdır.
- **F15-T02** üç mimaride işletim sistemi matrisi → **yok** (donanım/insan).
  Tablodaki "F15-T02 güvenlik test matrisi" bir **iç** test matrisidir; şartnamenin
  **F15-T07** (bağımsız güvenlik incelemesi) yerine geçmez.
- **F15-T03** 8 GB cihaz / 16 GB SSD ölçümü → **yok** (donanım).
- **F15-T04** 100.000 belge / 1 milyon sayfa kapasite → **yok** (donanım/disk).
- **F15-T05** p50/p95 yayımla → "performans benchmark" + "iş kuyruğu ölçümü"
  satırları. Arama/ilk sayfa/bellek/kuyruk yayımlandı; **OCR yayımlanmadı**.
- **F15-T06** süreç öldürme, disk dolması, uyku/uyanma, harici disk → disk
  dolması **ölçüldü**; süreç öldürme kısmi (F01-T10 + geri dönüş tatbikatı);
  uyku/uyanma ve harici disk **yok**.
- **F15-T07** bağımsız güvenlik incelemesi → **yok** (insan).
- **F15-T08** prompt injection, dosyalar arası sızıntı, gizli log işaretleyicisi →
  **yok**: yalnız CLI araç-yalıtımı canary'leri var (F10), dolaylı prompt
  injection ölçülmedi.
- **F15-T09** VoiceOver/NVDA + %200 ölçek + yalnız klavye → statik a11y testi var,
  **gerçek tur yok** (insan).
- **F15-T10** 5–8 avukat pilotu, **F15-T11** yardım ihtiyacı/ikinci kullanım
  ölçümü → **yok** (insan, pilota bağlı).
- **SBOM** şartnamede F15'te değil, **F16-T01**'de.

Eski satırlar kanıt kaybolmasın diye silinmedi; düzeltilen satırda doğru kimlik
yazılı. Sıradaki tur: tabloyu bu eşlemeye göre kanıt taşıyarak yeniden yaz.

