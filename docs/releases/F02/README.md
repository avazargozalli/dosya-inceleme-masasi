# F02 — Repository, derleme ve masaüstü iskeleti

- **Sürüm etiketi:** `v0.1.0`
- **Durum:** Tamam — Windows ARM64 F17'de upstream engelli
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F02-T01 | ✅ | `apps/`, `crates/`, `workers/`, `packages/`, `fixtures/`, `tests/` |
| F02-T02 | ✅ | `packages/contracts/schemas` → TS üretimi + drift kontrolü |
| F02-T03 | ✅ | komutlar alan işlemleriyle sınırlı; genel SQL/shell yok |
| F02-T04 | ✅ | CSP + `capabilities/default.json` (core + dialog) |
| F02-T05 | ✅ | `ApiError { code, message, retryable }`, tek hata biçimi |
| F02-T06 | ✅ | CI: Linux/macOS test + macOS Tauri check + Windows ayrı iş |
| F02-T07 | ✅ | PyInstaller onedir `.app` içinde; `ping`/`capabilities` kanıtı. Tesseract ikilisi pakete gömülmedi — sistemde kurulu olmalı |
| F02-T08 | ✅ | Windows x64 MSI üretiliyor **ve belge worker'ını taşıyor** (2026-11-06: worker Windows'ta da paketleniyor; MSI 6,5 MB → 41,1 MB); WebView2'yi Tauri bootstrapper yönetir |
| F02-T09 | ✅ | **Gerçek kurulum/kaldırma tatbikatı (2026-11-06):** sessiz kurulum çıkış 0, ARP kaydı, 122 dosya / 94,4 MB, kurulan worker `pong`+`pdf 1.24.14`, kaldırma çıkış 0 ve iz bırakmıyor. Kod imzası kapsam dışı (ADR-0012) |
| F02-T10 | ✅ | teşhis yalnız sürüm/platform; log içerik taşımaz |
| F02-T11 | ✅ | `CONTRIBUTING.md`, `SECURITY.md`, `docs/dev-setup.md`, `AGENTS.md` |
| F02-T12 | ✅ | `tests/smoke/worker_smoke.py` gerçek PDF ile |

## Windows MSI (imzasız) — F02-T08/T09

`desktop-build` → `windows-binary (x86_64-pc-windows-msvc, x64)` **gerçek üretim**
ile doğrulandı (run `40058501597`, commit `08d82c3`):

- Dosya: `Dosya İnceleme Masası_0.1.0_x64_tr-TR.msi`
- Boyut: **6.537.216** bayt
- SHA-256: `46504e297900ae942265782d38942eaa0e5f6e0f0cc948e4492c96e62c002b0b`
- Artefakt: `dim-windows-x64` (MSI + `.exe`), iş **success**.

Türkçe ürün adı/açıklaması WiX'in varsayılan kod sayfasında (1252) hata
veriyordu (`LGHT0311`); `bundle.windows.wix.language = "tr-TR"` ile kod sayfası
1254 yapıldı.

**Sınırlamalar:** paket **imzasız** (ADR-0012). WebView2 kurulum modu Tauri
varsayılanında (`downloadBootstrapper`); çevrimdışı kurulum ve kullanıcı düzeyi
kaldırma denenmedi. ARM64 paket üretilemiyor → `docs/releases/F17/README.md`.

## Kurulum tatbikatı ve worker'ın Windows paketine girmesi (2026-11-06, F02-T08/T09)

**Ölçülen kusur (paket, gerçek):** `windows-binary` MSI'yi üretiyordu ama
**belge worker'ını hiç paketlemiyordu** — `scripts/build-worker.sh` yalnız macOS
`bundle` işinde çağrılıyordu. Önceki koşunun (`42181755831`) `dim-paket-msi-x64`
artefaktı indirilip MSI'nin dosya tablosuna bakıldı: `resources/worker` altında
yalnız **`.gitkeep` + `README.md`** var, `dim-worker.exe` **yok**. Kurulum
tatbikatı aynı paketi kurduğunda hedefte **4 dosya / 22.694.396 bayt** vardı —
yani kurulan uygulama hiçbir belgeyi açamazdı (`E_WORKER_MISSING`). Paket
"başarılı" görünürken ürünün çekirdeği eksikti.

**Düzeltme (`scripts/build-worker.sh`, `scripts/worker_manifest.py`, `desktop-build.yml`):**

- `build-worker.sh` artık Windows'ta da koşar: venv düzeni (`Scripts/python.exe`),
  çıktı adı (`dim-worker.exe`), `-m PyInstaller`, ve **yerel Python'a geçen her yol
  `cygpath -w` ile çevrilir** (git-bash `/d/a/...` yolunu Windows Python çözemez).
- Betik "üretildi" ile "çalışıyor"u ayırır: kopyalanan ikiliye **gerçek `ping`**
  gönderir; `pong` gelmezse düşer (içi boş onedir yeşil görünmesin).
- `worker_manifest.py` platform etiketini tek yerden üretir (`windows-AMD64` →
  `windows-x86_64`) ve **konsolu UTF-8'e sabitler**: Windows konsolu cp1252
  olduğu için ilk Windows koşusu tam bu satırda `UnicodeEncodeError` ile düştü
  (ölçüldü, düzeltildi — eski sürüm cp1252'de çöküyor, yenisi yazıyor).
- `windows-binary` işi: `setup-python` + `bash scripts/build-worker.sh` + paket
  içi worker kapısı (dosya sayısı > 50 ve gerçek `ping`).

**Ölçüm (koşu `42191355696`, Windows x64):** worker manifesti **117 dosya /
71.698.329 bayt / 0 bağ**, `verify` → *"doğrulanan 117 · sorun 0 · fazla 0"*;
paket dizininde **120 dosya**; MSI **41.115.800 bayt** (39,21 MiB), sha256
`549fb7b2…cda01e` (worker'sız hâli 8.290.304 bayt / `d4fe2445…`).

**Kurulum tatbikatı (`msi-install-drill.yml`, koşu `42193217878`):**

- Sessiz kurulum (`msiexec /i … /qn /norestart`) **çıkış 0**.
- ARP: `Dosya İnceleme Masası 0.1.0 → C:\Program Files\Dosya İnceleme Masası\`.
- Kurulan ağaç: **122 dosya / 94.421.532 bayt**.
- `resources\worker\dim-worker.exe` **5.945.161 bayt** — kurulum dizininden
  konuşturuldu: `{"pong":true,"workerVersion":"0.1.0"}` ve `capabilities`:
  `pdf 1.24.14 · docx 1.1.2 · udf 1 · image 10.4.0 · text` (`ocr`: **yok**,
  çalıştırıcıda tesseract kurulu değil).
- Kaldırma **çıkış 0**; uygulama ikilisi ve ARP kaydı kalmıyor.
- **Negatif kontrol:** aynı tatbikat **eski** MSI'ye (`42181755831`) karşı
  koşuldu → kurulum yine çıkış 0, ama 4 dosya ve **`kurulan pakette belge worker
  yok`** ile **kırmızı** (`42193554800`). Yani kapı gerçekten kusuru yakalıyor.
- **Tam hat yeşil:** `desktop-build` `42194554840` — `bundle` (macOS arm64 + x86_64),
  `windows-binary (x64)`, `manifest` ve tatbikat (`workflow_call` ile) **success**;
  yalnız Windows **ARM64** deneysel işi kırmızı (`continue-on-error`, F17).

**Ölçülen nüans (iddia edilmiyor):** kurulan `dim-desktop.exe` **aynı boyutta**
(22.692.864 bayt) ama sha256 **farklı** (`d7501397…` ≠ derlenen `f985cd88…`) —
Tauri paketleme sırasında ikiliyi yeniden yazıyor. Bu yüzden kimlik iddiası
**sürüm bilgisi** (`ProductVersion`/`FileVersion` = `0.1.0`) **+ MSI özeti**
üzerinden kurulur; "kurulan ikili derlenenle bayt bayt aynı" **denmez**.

**Koşular arası tekrarlanabilirlik (ölçüldü, iddia edilmiyor):** ikinci tam hat
koşusu (`42194554840`) aynı commit'te MSI'yi **41.107.608 bayt** / `9aeb78bf…`
üretti (ilki 41.115.800 / `549fb7b2…`); derlenen `.exe` özeti de koşular arasında
değişiyor (`2d467745…` ↔ `f985cd88…`). Yani **paket bit-tekrarlanabilir değil**
(`.dmg`de de aynı durum). Buna karşılık **kurulan ağaç iki koşuda da birebir**:
122 dosya / 94.421.532 bayt, worker 5.945.161 bayt. Karşılaştırma manifest
üzerinden yapılır, koşular arası dosya özeti üzerinden değil.

**Tatbikatın kendisi yeniden koşulabilir:** `msi-install-drill.yml` ayrı bir
yeniden kullanılabilir iş akışı; `desktop-build` onu `workflow_call` ile çağırır,
elle `artifact_run_id` verilerek **25 dakikalık yeniden derleme beklemeden**
herhangi bir koşunun MSI'sine karşı koşulabilir. (Ders: çağıran iş
`permissions: actions: read` vermezse GitHub iş akışını `startup_failure` ile
reddediyor — ölçüldü ve düzeltildi.)

**Dürüst sınırlar:** (a) OCR Windows paketinde **yok** (tesseract gömülmedi —
F02-T07 kararı); (b) kurulum **GUI açmıyor**: gerçek Tauri penceresi, WebView2
indirme yolu ve görsel QA ölçülmedi; (c) paket **imzasız** (ADR-0012);
(d) ARM64 MSI üretilemiyor (F17); (e) tatbikat `windows-latest` üzerinde
yönetici oturumunda koşar — son kullanıcı makinesindeki UAC/elevation deneyimi
ölçülmedi; (f) kaldırma ürün verisini (`%APPDATA%`) silmez (kasıtlı).

## Doğrulama

GitHub Actions `ci`: 6/6 iş yeşil (sözleşme, arayüz, rust ×2, masaüstü, python).
`desktop-build` imzasız `.app` artefaktı üretir (F16'da imzalanacak) ve Windows
x64 MSI'yi yukarıdaki kanıtla üretir.

## Açık

- WebView2 **indirme yolu** (çevrimdışı kurulum, UAC/elevation) ölçülmedi; kurulum/kaldırma
  tatbikatı artık CI'da (yukarıdaki bölüm) ama gerçek Tauri penceresi açılmıyor.
- Windows testleri ayrı işte (`windows güvenli depo`); her push'ta koşmaz.

## Worker'ın pakete gömülmesi (F02-T07)

- `scripts/build-worker.sh`: PyInstaller **onedir** (`dim-worker.spec`), `workers/document[all]`
  bağımlılıkları venv'e kurulur, çıktı `apps/desktop/src-tauri/resources/worker/` altına kopyalanır.
- `tauri.conf.json` `bundle.resources: ["resources/worker/**/*"]` — macOS'ta
  `Contents/Resources/resources/worker/` altına düşer.
- `bundled_worker_exe()` bu çift `resources` yolunu da arar (önceki hâlde yalnız
  `Contents/Resources/worker` aranıyordu → paketli uygulamada `E_WORKER_MISSING`).
- NPM paketleri yayınlanan sürümlere sabitlendi (`@tauri-apps/api|cli 2.12.4`,
  `plugin-dialog 2.8.3`); `pnpm-lock.yaml` manifest ile eşitlendi.

### Gerçek doğrulama (macOS arm64, imzasız paket)

`dim-aarch64-apple-darwin.zip` indirildi, `.app` açıldı, paket içindeki ikili
doğrudan NDJSON ile konuşturuldu:

```
$ printf '{"v":1,"id":"1","method":"ping","params":{}}\n' | .../worker/dim-worker
{"v":1,"id":"1","ok":true,"result":{"pong":true,"workerVersion":"0.1.0"}}

$ ... capabilities
pdf 1.24.14 · docx 1.1.2 · udf 1 · image 10.4.0 · ocr eng+osd · maxPages 2000
```

### Kalan

- **Tesseract ikilisi pakete gömülmedi**; OCR için sistemde kurulu olmalı, yoksa
  `E_OCR_MISSING`. Dil verisi (`tur`) de paketlenmiyor. (Windows kurulumunda da
  aynı: kurulan worker `ocr.available=false` bildirir — ölçüldü 2026-11-06.)
- Windows paketi **worker'ı taşıyor** (2026-11-06); MSI 41,1 MB ve kurulum tatbikatı CI'da
  (`msi-install-drill.yml`).
- `.app` imzasız; Gatekeeper uyarısı F16'da çözülecek.
