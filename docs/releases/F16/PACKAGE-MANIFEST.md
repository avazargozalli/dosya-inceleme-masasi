# Sürüm paketi manifesti ve indirilen paket doğrulaması (F16)

- **Durum:** Manifest üretimi ve indirme doğrulaması var; **gerçek paketlerle üretildi
  ve doğrulandı** (2026-11-06, `desktop-build` koşusu `42170709416`): iki `.dmg` + bir
  `.msi`, üçü de `verify`den geçti; kesik ve kurcalanmış **gerçek** paket reddedildi.
- **Araç:** `scripts/release_manifest.py` (`build`, `verify`, `--self-test`)
- **CI:** `paket` işi (`ci.yml` — biçim kapıları, sentetik artefakt) + `manifest` işi
  (`desktop-build.yml` — `tauri build`in ürettiği **gerçek** `.dmg`/`.msi`)

## Neden

İkili paketler (`.dmg`, `.msi`, `.AppImage`, `.deb`) depoda tutulmaz ve **elle**
üretilir. Bu turdan önce yayımlanan bir paketin yanında **hiçbir manifest veya
checksum yoktu**: indiren kişi dosyanın doğru sürüm, doğru mimari ve eksiksiz
olduğunu doğrulayamıyordu; kaynak arşivi (`SOURCE-RELEASE.md`) üretiliyordu ama
ikili taraf ona bağlı değildi.

## `build` — manifest üretimi

```
python3 scripts/release_manifest.py build \
  --artifacts dist/release --version 0.1.0 --commit "$(git rev-parse HEAD)" \
  --source-manifest dist/kaynak/*.manifest.json \
  --out dist/release-manifest.json
```

Kapılar:

1. **Ad biçimi:** `<ürün>_<sürüm>_<mimari>[_<yerel>].<tür>`. Tanınmayan dosya
   reddedilir (`E_UNKNOWN_ARTIFACT`) — sessizce atlanmaz. Yalnız `.sha256`,
   `.json`, `.md`, `.txt` yan dosyaları serbesttir.
2. **Sürüm:** dosya adındaki sürüm `--version` ile birebir aynı olmalı (`E_VERSION`).
3. **Mimari kapalı küme:** `aarch64`/`arm64` → `aarch64`; `x64`/`x86_64`/`amd64` →
   `x86_64`; `universal`. `x86_64` alt çizgisi yerel ayırıcısıyla karıştırılmaz
   (regex `[A-Za-z0-9]+` bunu `x86` + `_64` diye bölerdi).
4. **Tür → platform:** `dmg`→macOS, `msi`→Windows, `AppImage`/`deb`→Linux.
5. **Yinelenen paket:** aynı platform+mimari+tür+yerel iki kez olamaz
   (`E_DUPLICATE`); `x64` ile `x86_64` aynı mimariye çözüldüğü için ikinci dosya
   yinelenmiş sayılır.
6. **Zorunlu üçlü:** macOS arm64 dmg, macOS x86_64 dmg, Windows x64 msi. Eksikse
   manifest **yazılmaz** (`E_MISSING_PLATFORM`). `--require platform/mimari/tür`
   ile değiştirilir (ör. Linux eklemek veya yalnız bir platformu zorunlu kılmak).
7. **Boş dosya** artefakt değildir (`E_EMPTY`).
8. **Kaynak bağı:** `--source-manifest` verilirse `version` ve `commit` birebir
   olmalı (`E_SOURCE_LINK`); kaynak arşivinin **tar** özeti, `.tar.gz` özeti ve
   manifest özeti paket manifestine taşınır.
9. **İmza iddiası yazılamaz:** `signed`/`notarized` varsayılan `false`. `--signed`
   yalnız `--attestation <dosya>` ile yazılır ve attestation'ın sha256'sı kaydedilir
   (`E_SIGNATURE_CLAIM`). Bu depo imzalı sürüm **iddia etmez** (ADR-0012).
10. **Kimlik `contentSha256`:** `generatedAt` ve `contentSha256` dışındaki alanların
    kanonik JSON özeti. İki koşu aynı klasörde aynı kimliği verir; `generatedAt`
    tek değişken alandır.
11. **Ürün adı NFKC'ye normalize edilir:** macOS dosya adlarını ayrıştırılmış (NFD)
    yazabilir; `Dosya İ̇nceleme Masası` ile `Dosya İnceleme Masası` aynı üründür.

## `verify` — indirilen paketin doğrulanması

```
# 1) manifesti üret (yanına release-manifest.json.sha256 da yazılır)
python3 scripts/release_manifest.py build --artifacts dist/release \
  --version 0.1.0 --commit "$(git rev-parse HEAD)" --out dist/release/release-manifest.json

# 2) indirilen paketi doğrula (özet ayrı kanaldan, ör. sürüm duyurusu)
python3 scripts/release_manifest.py verify \
  --manifest release-manifest.json --file "Dosya İnceleme Masası_0.1.0_x64_tr-TR.msi" \
  --platform windows --arch x86_64 --version 0.1.0 --require-free-disk \
  --manifest-sha256 <duyurudaki-64-haneli-özet> --require-manifest-digest
```

| Kapı | Kod | Ne zaman |
|---|---|---|
| Dosya manifestte yok | `E_ARTIFACT_UNLISTED` | ad birebir eşleşmiyorsa |
| Boyut farklı | `E_SIZE` | kesilen/eksik indirme (aynı ad, eksik bayt) |
| sha256 farklı | `E_HASH` | kurcalanmış dosya (aynı boyut, değişmiş bayt) |
| Platform farklı | `E_PLATFORM` | yanlış işletim sistemi paketi |
| Mimari farklı | `E_ARCH` | yanlış mimari (ör. arm64 dmg'yi x86_64 diye kurmak) |
| Sürüm farklı | `E_VERSION` | manifest sürümü beklenenle uyuşmuyorsa |
| Manifest düzenlenmiş | `E_MANIFEST_TAMPER` | `contentSha256` yeniden hesaplanınca tutmuyorsa |
| Manifest dış özeti tutmuyor | `E_MANIFEST_DIGEST` | `--manifest-sha256` verilmiş ve dosya baytları başka |
| Yan dosya tutmuyor | `E_MANIFEST_SIDECAR` | `release-manifest.json.sha256` manifestin baytlarıyla uyuşmuyorsa |
| Boş disk yetersiz | `E_DISK` | `--require-free-disk` ve boş alan < paket boyutu |

Hiçbir adım "uyarı" değildir: ya geçer ya reddeder (tek satır gerekçe, `stderr`).
Tek istisna **bilinçli**: hiçbir özet kaynağı yoksa (`--manifest-sha256` verilmemiş,
`.sha256` yan dosyası da yok) doğrulama geçer ama `stderr`'a **UYARI** yazar —
"manifest özeti doğrulanmadı, paket ikamesi yakalanmaz". Sessiz kabul yok.

## Manifestin kendisi nasıl doğrulanır (dış özet)

Manifestin `contentSha256` alanı **dosyanın içindedir**. Bir saldırgan paketi
değiştirip manifestteki `bytes`/`sha256` alanlarını yeni dosyaya göre günceller ve
`contentSha256`ı **yeniden hesaplarsa** `E_MANIFEST_TAMPER` tetiklenmez: manifest
kendi kendisiyle tutarlıdır. Ölçüldü (2026-11-06, eski sürüm): ikame edilmiş MSI
`verify`den **geçti**.

Bu yüzden:

1. `build` manifesti yazarken yanına `release-manifest.json.sha256`
   (`<özet>  <dosya adı>`) bırakır — kaynak arşivindeki `.sha256` ile aynı biçim.
2. `verify` manifesti **ayrıştırmadan önce** dosya özetini denetler: sıra
   `--manifest-sha256` (ayrı kanaldan gelen) → yan dosya → `--require-manifest-digest`
   ile "özet yok" reddi.
3. Sürüm işi (`desktop-build.yml` → `manifest`) özeti **iş özetine** (job summary)
   yazar; duyuruya giden özet budur. Aynı iş, ikame edilmiş gerçek paketi bu özetle
   reddeder (`E_MANIFEST_DIGEST`) — kapı gerçek dosyalarla kanıtlanır.

## Ölçüm (2026-11-06)

### Gerçek paketlerle (ilk kez)

İki `desktop-build` koşusu — `tauri build`in ürettiği dosyalar, sentetik değil.
İlki (`42170709416`) manifesti üretti; ikincisi (`42181755831`) **dış özet kapısıyla**
birlikte koştu:

| platform/mimari/tür | boyut (`42181755831`) | not |
|---|---|---|
| `macos/aarch64/dmg` | 66.697.588 bayt | `Dosya İnceleme Masası_0.1.0_aarch64.dmg` |
| `macos/x86_64/dmg` | 66.643.461 bayt | `Dosya İnceleme Masası_0.1.0_x64.dmg` |
| `windows/x86_64/msi` | 8.290.304 bayt | `Dosya İnceleme Masası_0.1.0_x64_tr-TR.msi` |

- Sürüm `0.1.0`, etiket **yok** (`--allow-untagged`); kaynak arşivi **435 dosya**
  (2.286.554 / 2.289.801 bayt — commit'e bağlı), manifest `source.commit` ile bağlı.
- İkinci koşuda manifest kimliği `99934c6c…058e72`, **dış özet** `d43148d4…4721e958`
  (yan dosya + iş özeti); **üç paket de** dış özetle `verify`den geçti.
- `.dmg` boyutları iki koşuda ~10 bayt farklı: **dmg bit-tekrarlanabilir değil**; bu
  yüzden karşılaştırma manifest üzerinden yapılır, koşular arası dosya özeti değil.
- **Kesik gerçek `.dmg`** (`head -c 102400`) → `E_SIZE`; **tek baytı bozulmuş gerçek `.msi`**
  → `E_HASH`. Artefakt `dim-surum-paketleri` (140.481.702 bayt) yüklendi.
- **Windows ARM64 işi kırmızı** (`continue-on-error`, deneysel — F17: `openssl-src`
  aarch64-windows derlemiyor) → manifestte Windows **arm64 yok**, `windows/x86_64` var.

### Dış özet kapısı (bu tur eklendi)

- **Ölçülen kusur:** eski `verify`, ikame edilmiş paketi **kabul etti** — manifestin
  `contentSha256`ı geri hesaplandığında kendi kendisiyle tutarlı görünüyor
  (`E_MANIFEST_TAMPER` tetiklenmiyor). Aynı saldırı yeni sürümde dış özetle
  `E_MANIFEST_DIGEST` ile düşüyor (yerel ölçüm + CI adımı).
- **Gerçek CI (ölçüldü, `42181755831`):** `manifest` işi özeti ilan etti
  (`d43148d4…`), olumlu yolu `--manifest-sha256` + `--require-manifest-digest` ile koştu
  ve **gerçek `.msi` ikamesini** (içerik değişti, `bytes`/`sha256`/`contentSha256`
  yeniden hesaplandı) `E_MANIFEST_DIGEST` ile reddetti. İş günlüğü dürüst ölçümü de
  yazıyor: *"ölçü: ikame manifest kendi iç özetiyle KABUL ediliyor (bilinen sınır)"* —
  yani eski kapı bunu gerçekten kaçırıyordu.
- **Yerel prova:** adımın birebir kabuğu sentetik üç paketle koşuldu (özet ilanı,
  olumlu yol, ikame reddi) — CI'dan önce.
- **Öz test 25 → 31 kural.** Yeni kurallar: `G21` yan dosya yazımı, `G22` kurcalanmış +
  geri hesaplanmış manifest (`E_MANIFEST_SIDECAR`), `G23` dış özet uyuşmazlığı
  (`E_MANIFEST_DIGEST`), `G24` özet yokluğu, `G25` bozuk özet biçimi (`E_USAGE`),
  `G26` özetsiz doğrulamanın **uyarı** yazması.
- **Kapı kanıtı 6/6:** altı mutasyonun her biri **tek başına** yalnız kendi kuralını
  düşürdü (G21…G26); geri yükleme sha256 `916f84ff…` birebir, son koşu yeşil.
- **CI sayıları:** `ci.yml` `42181706803` **10/10 yeşil** — `paket` işi günlüğü:
  `öz testi yeşil (31 kural)`, üç **sentetik** paket için `manifest özeti doğrulandı:
  3e04b411…`, `ok: kesik → E_SIZE`, `ok: kurcali → E_HASH`. `desktop-build`
  `42181755831` **yeşil** (macOS iki mimari + Windows x64; ARM64 deneysel iş kırmızı
  ama `continue-on-error`) — orada ilan edilen özet `d43148d4…`.

### Biçim kapıları (önceki tur)

- **Kapı kanıtı 15/15 tek satır mutasyonu kırmızı**: boyut, sha256, mimari, platform,
  sürüm, listede olmayan dosya, manifest bütünlüğü, boş dosya, tanınmayan dosya,
  dosya sürümü, yinelenen paket, eksik platform, kanıtsız imza, kaynak commit bağı,
  kimliğin `generatedAt`e bağlanması. **Dürüst:** 12 mutasyon cerrahi (yalnız kendi
  kuralı düşüyor), 3'ünde (listede olmayan dosya, tanınmayan dosya, kanıtsız imza)
  kapı kaldırılınca öz test çöküyor — o üçünde kural yine kırmızı ama ölçüm
  "yanlış çökme" biçiminde. Geri yükleme sha256 `7537082…cc1c9f` birebir.
- **Gerçek repoyla bağ:** `source_release.py` ile üretilen gerçek kaynak manifesti
  (`dim-source-0.1.0-a5d450d3.tar.gz.manifest.json`, commit `a5d450d3…`,
  tar özeti `d4a0d3ba…cbeb2`) paket manifestine bağlandı; sentetik üç artefaktla
  manifest üretildi ve `verify` üçünü de doğruladı.
- **CI:** `paket` işi yukarıdakini her push'ta koşar; kesik (`head -c 100`) ve
  kurcalanmış (`dd` ile ilk bayt) kopyalar **aynı adla** ayrı klasörlerde
  doğrulanır ve `E_SIZE`/`E_HASH` ile düştüğü `grep` ile kanıtlanır. İlk koşu
  `42143791175` **10/10 yeşil** (iş günlüğü: `öz testi yeşil (25 kural)`,
  `ok: kesik → E_SIZE`, `ok: kurcali → E_HASH`).

## Dürüst sınırlar

1. **Gerçek paketler ölçüldü, ama yayımlanmadı.** Paketler CI'da üretildi ve
   `verify`den geçti (`42170709416`); depoda **tutulmaz** (imzasız, `.gitignore`) ve
   sürüm etiketi/yayın adımı **yapılmadı** (insan bekliyor). Bu yüzden "indirilebilir
   sürüm var" denemez — yalnız "gerçek paketlerle manifest üretildi ve doğrulandı".
2. **Dış özet ayrı kanaldan gelmezse koruma yok.** `--manifest-sha256` verilmez ve
   `.sha256` yan dosyası indirilmezse `verify` eski davranışta kalır (yalnız **uyarı**
   yazar): paket ikamesi yakalanmaz. Özet bugün `desktop-build` iş özetine yazılıyor;
   sürüm duyurusuna/README'ye **insan** taşımalı. Sidecar'ı manifestle aynı yerden
   indiren biri, ikisini birlikte değiştirebilen bir saldırgana karşı korunmaz.
   Manifest hâlâ **imzasız** (ADR-0012), yani köken (origin) kanıtı yok — yalnız
   bütünlük + ikame tespiti var.
3. **İmza ve notarization kapsam dışı** (ADR-0012). Manifest `signed:false`
   yazar; kimse "imzalı" diye pazarlayamaz.
4. **Uygulama içi indirme yolu** (Tauri updater) bu kapıyı **çağırmıyor**:
   `plugins.updater.active=false`, imza anahtarı yok. Yani kapı bugün yayın/indirme
   *aracında* var, güncelleme *çalışma zamanında* değil.
5. **`verify` tek dosya doğrular**; paket kümesinin tamlığını (üçü de indi mi)
   manifestin zorunlu üçlüsü + `build` kapısı garanti eder, indirme aracı değil.
6. **Gerçek koşu yalnız macOS/Windows x64 üretti**: ARM64 Windows işi deneysel ve
   upstream'de kırmızı (F17) → Windows arm64 msi manifestte **yok**; "dört platform"
   iddiası yok. `desktop-build` **elle/etiketle** tetiklenir, her push'ta koşmaz.
7. **Mimari kümesi kapalıdır**: yeni bir mimari (ör. Windows arm64 msi) eklenirse
   `ARCH_TOKENS` güncellenmelidir; aksi hâlde dosya `E_UNKNOWN_ARTIFACT` ile düşer
   (sessiz kabul yok — bilinçli).
8. **Yerel ölçüm macOS/Python 3.13**; CI ubuntu/Python 3.12 üzerinde aynı öz testi
   koşar. `generatedAt` dışındaki alanlar dile bağlı değildir (kanonik JSON), ancak
   `contentSha256` yine de **aynı klasör içeriğinde** karşılaştırılmalıdır.
