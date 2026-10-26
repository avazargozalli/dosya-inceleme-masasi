# F15 — Kalite, güvenlik, pilot

- **Durum:** Kısmi (B-01 ölçüldü; pilot ve ekran okuyucu turu yok)
- **Tarih:** 2026-10-25 (B-01 tam profil ölçümü)

| İş | Durum | Kanıt |
|---|---|---|
| F15-T01 erişilebilirlik regresyonu | ✅ | `src/a11y.test.ts` — etiketsiz alan, altsız görsel, adsız buton |
| F15-T02 güvenlik test matrisi | ✅ | `docs/security/test-matrix.md` (T1–T15) |
| F15-T03 SBOM | 🚧 | `scripts/sbom.sh`; CI job'u |
| F15-T04 performans benchmark | 🚧 | **B-01 profili ölçüldü** (1.000 belge / 100.000 segment / 200 sorgu, release): p50 **1,25 sn**, p95 **1,53 / 2,18 sn** → 2 sn hedefi sınırda ve tutarlı değil; kuyruk **36–54 sn**. Ölçümle bulunan indeks kusuru düzeltildi (ilk sayfa **0,314 ms**). **Ertelenmiş toplam sayım** uygulandı: arayüz yalnız ilk sayfayı bekler (ölçülen sayfa p50 **605 ms**, p95 **712 ms**), sayı sonradan ayrı komutla gelir ve sayfayla **birebir aynı** sayıyı verir. **Aday penceresi + tembel doğrulama** (ilk sayfa 2,28×). **Belirteç/konum indeksi ölçüldü ve uygulanmadı** (kazanç %5, depolama +%54 — gerekçe `BENCHMARK.md`). **İki aşamalı tarama** uygulandı: seyrek profilde aynı korpusta **2,6–3,1×**. **Aday daraltma** (`NEAR`) uygulandı: seyrek profilde aday 3.030 → 30, uçtan uca ilk sayfa **5,95 → 1,55 ms (3,84×)**; yan bulgu **kusur #35** (`phrase` kipinde "ıslah" sorgusu "ISLAH" metnini bulamıyordu) düzeltildi. 100.000 **belge** / 1 milyon sayfa kapasite testi **yapılmadı**. Ayrıntı: `BENCHMARK.md` |
| F15-T05 pilot (5–8 avukat) | ⬜ | **insan gerekir**; protokol `docs/pilot/` |
| F15-T06 VoiceOver/NVDA turu | ⬜ | statik test var, ekran okuyucu turu yapılmadı |

## Ölçüm yayını (spec F15-T05)

Spec'teki "arama, ilk sayfa, OCR, bellek ve iş kuyruğu p50/p95" maddesinin
durumu:

- **Arama** — yayımlandı (200 sorgu, p50/p95/max + grup kırılımı).
- **İlk sayfa** — yayımlandı (100 sayfalık sürüm, p50 0,314 ms).
- **Bellek** — yayımlandı (tepe RSS 10,3 MB, arama sonrası 21,1 MB).
- **OCR** — **yayımlanmadı** (worker bu koşuda devrede değil; Tesseract bu
  makinede kurulu değil).
- **İş kuyruğu** — **yayımlanmadı** (kuyruk gecikmesi ölçülmedi).

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
