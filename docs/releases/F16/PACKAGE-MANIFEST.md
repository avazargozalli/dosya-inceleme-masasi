# Sürüm paketi manifesti ve indirilen paket doğrulaması (F16)

- **Durum:** Manifest üretimi ve indirme doğrulaması var; **gerçek paketlerle henüz
  üretilmedi** (paketler elle, imzasız derlenir).
- **Araç:** `scripts/release_manifest.py` (`build`, `verify`, `--self-test`)
- **CI:** `paket` işi (`--self-test` + gerçek repodan kaynak manifesti + sentetik
  artefaktlarla manifest/doğrulama + kesik/kurcali paketin reddi)

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
python3 scripts/release_manifest.py verify \
  --manifest release-manifest.json --file "Dosya İnceleme Masası_0.1.0_x64_tr-TR.msi" \
  --platform windows --arch x86_64 --version 0.1.0 --require-free-disk
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
| Boş disk yetersiz | `E_DISK` | `--require-free-disk` ve boş alan < paket boyutu |

Hiçbir adım "uyarı" değildir: ya geçer ya reddeder (tek satır gerekçe, `stderr`).

## Ölçüm (2026-11-06)

- **Öz test 25 kural yeşil** (`--self-test`; fixture deposu geçici klasörde).
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

1. **Gerçek paket yok.** Depoda `.dmg`/`.msi` bulunmaz; yukarıdaki ölçümler
   sentetik artefaktlarla ve gerçek **kaynak** manifestiyle yapıldı. Gerçek
   sürümde manifest, `dist/release` klasörü dolduğunda **aynı komutla** üretilir;
   bu ana kadar "gerçek paketler doğrulandı" denemez.
2. **İmza ve notarization kapsam dışı** (ADR-0012). Manifest `signed:false`
   yazar; kimse "imzalı" diye pazarlayamaz.
3. **Uygulama içi indirme yolu** (Tauri updater) bu kapıyı **çağırmıyor**:
   `plugins.updater.active=false`, imza anahtarı yok. Yani kapı bugün yayın/indirme
   *aracında* var, güncelleme *çalışma zamanında* değil.
4. **`verify` tek dosya doğrular**; paket kümesinin tamlığını (üçü de indi mi)
   manifestin zorunlu üçlüsü + `build` kapısı garanti eder, indirme aracı değil.
5. **Mimari kümesi kapalıdır**: yeni bir mimari (ör. Windows arm64 msi) eklenirse
   `ARCH_TOKENS` güncellenmelidir; aksi hâlde dosya `E_UNKNOWN_ARTIFACT` ile düşer
   (sessiz kabul yok — bilinçli).
6. **Yerel ölçüm macOS/Python 3.13**; CI ubuntu/Python 3.12 üzerinde aynı öz testi
   koşar. `generatedAt` dışındaki alanlar dile bağlı değildir (kanonik JSON), ancak
   `contentSha256` yine de **aynı klasör içeriğinde** karşılaştırılmalıdır.
