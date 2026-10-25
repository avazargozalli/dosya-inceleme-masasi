# F01-T02 — PyInstaller `onedir` worker paketi: manifest + çevrimdışı kanıt

- **Durum:** Ölçüldü (macOS arm64). Windows/Linux paketi bu turda üretilmedi.
- **Kanıt:** `scripts/build-worker.sh`, `scripts/worker_manifest.py`,
  `scripts/verify-worker-offline.py`, `crates/ingest/src/packaging.rs`,
  `crates/ingest/tests/worker_package.rs`, Tauri `worker_package_report`.

## Neden bu iş "paketlendi" demekle kapanmıyor

`onedir` çıktısı tek dosya değil, **yüzlerce dosyalık bir ağaçtır** (Python
çalışma zamanı, PyMuPDF/Pillow yerel kütüphaneleri, python-docx şablonları,
`base_library.zip`). Bu ağaçtan bir dosya eksikse uygulama açılır, `ping`
yanıtlar, sonra tam olarak o motoru kullanan işlemde düşer. "Paket var" ile
"paket eksiksiz" farklı iddialardır; bu tur ikincisini **ölçülebilir** yaptı.

## Üretilen: `WORKER-MANIFEST.json`

`scripts/worker_manifest.py generate` paket dizinini tarar ve her girdi için
`path`, `role`, `kind`, `bytes`, `sha256` yazar; sembolik bağlarda `linkTarget`
ve `unsafeLink`. Üst düzey künye: `manifestVersion=1`, `workerVersion`, `mode`,
`buildPlatform`, `pythonVersion`, `pyinstallerVersion`, `entryPoint`, `totals`,
`roles`, `tesseract`.

Sınırlar kodda: en çok **20.000** girdi / **8 GiB**; aşılırsa `truncated` ve
"sonuç eksik" uyarısı (sessiz kesme yok). Paket dizininde depoda izlenen
`.gitkeep` ve `README.md` **paket sayılmaz** (Tauri glob'u için dururlar).

## Gerçek ölçüm (bu makinede, 25 Eki 2026)

| Ölçü | Değer |
|---|---|
| Paket | `apps/desktop/src-tauri/resources/worker/` |
| Dosya + bağ | **118 dosya + 18 sembolik bağ** (136 girdi, 136/136 doğrulandı) |
| Toplam boyut | **79.294.775 bayt** (75,6 MiB) |
| Yürütülebilir | `dim-worker` — **5.773.936 bayt** |
| Python | 3.12.12 · PyInstaller **6.11.1** · `macos-arm64` |
| PyInstaller derleme süresi | **16,6 sn** (`COLLECT` sonuna kadar, gerçek log) |
| Roller | yerel kitaplık 74 · docx motoru 23 · görüntü kitaplığı 18 · veri dosyası 8 · python çalışma zamanı 6 · pdf motoru 4 · başlatıcı 1 · stdlib arşivi 1 · diğer 1 |
| Doğrulama | 0 sorun · 0 fazla dosya · `ok=true` |
| Tesseract | **pakete gömülmedi** (`bundled=false`); sistemde PATH'te yok → `capabilities` `ocr.available=false` |

Rol dağılımı paketin ne taşıdığını gösterir: 74 yerel kitaplık + 23 docx şablon
dosyası, tek bir "python" klasörü değil.

## Çevrimdışı kanıt (`scripts/verify-worker-offline.py`)

Koşu, paketi **kendi kendine yeter** hâlde dener:

1. Ortam asgaridir: `PATH=/usr/bin:/bin`, `HOME`/`TMPDIR` boş geçici dizin.
   `PYTHONPATH` ve `PYTHONHOME` **kasıtlı olarak bozuk** yola ayarlanır — paket
   bunları yok saymalı.
2. Ağ, macOS `sandbox-exec` ile kapatılır: `(version 1)(allow default)(deny network*)`.
3. Worker gerçek iş yapar: sentetik PDF'ten metin, sentetik DOCX'ten envanter.
4. **Dedektör kanıtı:** aynı profil altında bir TCP bağlantısı denenir; reddedilmesi
   beklenir. Reddedilmezse "çevrimdışı" iddiası geçersiz sayılır (bu kontrol ilk
   koşuda kendi kusurunu yakaladı — defter #30).

Ölçülen koşu (438 ms):

| Kontrol | Sonuç |
|---|---|
| worker çıkış kodu | 0 |
| `ping` | `workerVersion=0.1.0` |
| pdf motoru pakette | `1.24.14` |
| pdf metni | **1 sayfa · 46 karakter** · `engine=pymupdf` |
| docx envanteri | `kind=docx` · **9 paragraf · 3 ek** |
| `shutdown` | temiz |
| sandbox ağı gerçekten kapattı | **TCP bağlantısı reddedildi** |

## Uygulama içi doğrulama (yazmaz)

- `crates/ingest/src/packaging.rs`: manifesti **ayrıştırır** (sürüm kapısı, yol
  güvenliği: `..`/mutlak/`\`/denetim karakteri reddedilir, yinelenen yol reddedilir)
  ve dizine karşı **yeniden hesaplayarak** doğrular. Boyut → sha256 → bağ hedefi
  sırası; boyut tutmazsa hash hesaplanmaz (ucuz kapı). Fazla dosyalar ayrı sayılır.
- `WorkerClient::spawn_bundled` artık `PYTHONPATH`/`PYTHONHOME`/`PYTHONSTARTUP`/
  `PYTHONEXECUTABLE` değişkenlerini **kaldırır**: paketlenmiş ikili depo kaynağını
  yüklerse paket kusuru gizlenir.
- Tauri `worker_package_report()` paket dizinini bulur, doğrular ve döner; denetime
  yalnız **sayı/durum** yazılır (dosya adı, tam yol yazılmaz). Paket yokluğu hata
  değil, gerekçedir (geliştirme kipi).
- Arayüz (AI bağlantıları → “Yerel worker paketi”): rozet, 3 sayı kutusu
  (doğrulanan / uyuşmayan / fazla), rol listesi, sorun+madde listesi, Tesseract
  durumu, uyarılar ve “Doğrulamanın sınırları”.

## Dürüst sınırlar

- **Bütünlük kanıtı, imza değildir.** Manifesti üreten süreçle doğrulayan süreç
  aynı güven sınırındadır; kurcalayan manifesti de güncelleyebilir. Kod imzası
  ADR-0012 gereği kapsam dışı.
- **Yalnız macOS arm64 ölçüldü.** Windows MSI (F02-T08) ve Linux paketi bu turda
  üretilmedi; aynı betikler farklı platformda farklı sonuç verebilir (sembolik bağ
  davranışı Windows'ta farklıdır — testler o dalda atlanır).
- **Tesseract pakete gömülmez** (bilinçli): OCR için sistemde kurulu olmalı, dil
  verisi (`tur`) de paketlenmez. Bu makinede kurulu değil → `ocr.available=false`.
  Arayüz bunu “bulunamadı” olarak yazar, “OCR çalışıyor” demez.
- **Python sürümü paketi üreten sürümdür** (3.12.12). Depo geliştirme venv'i farklı
  bir sürümse iki ortam ayrışabilir; manifest bunu yazdığı için görünür.
- Paket **yeniden üretilebilir değil**: aynı kaynakla ikinci derleme bayt bayt aynı
  çıkmaz (PyInstaller zaman damgaları). Bu yüzden manifest bayt kimliği değil,
  **o derlemenin** bütünlük kaydıdır.
- Doğrulama **dosya düzeyinde**: yüklenebilirlik, dylib bağımlılıkları (`otool -L`)
  ve macOS Gatekeeper/quarantine davranışı ölçülmedi. Çevrimdışı koşu bunların
  yerine geçmez.
- `.app` içindeki çift `resources` yolu bu turda **Tauri penceresi açılmadan**
  doğrulandı; kartın görsel QA'sı yok (arayüz tip denetimi + 320 birim testi +
  üretim derlemesi ile sınırlı).
