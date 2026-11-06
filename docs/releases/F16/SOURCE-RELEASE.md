# F16-T01 — Kaynak arşivi (AGPL kaynak teslimi)

- **Durum:** Arşiv üretimi ve bağımsız doğrulama **hazır**; CI'da her push'ta
  gerçek repodan üretilip doğrulanıyor. **Sürüm etiketi (git tag) depoda
  yok** — etiket oluşturmak yayın adımıdır; araç etiketi `--tag` ile zorunlu
  kılar ve commit'e baktığını doğrular.
- **Tarih:** 2026-11-06
- **Kod:** `scripts/source_release.py` (üretim + doğrulama + `--self-test`),
  `scripts/sbom.sh` ve `scripts/sbom_check.py` (dört kilit dosyası bağı),
  `.github/workflows/ci.yml` → `kaynak` işi
- **Şartname:** F16-T01 "kaynak etiketi, kilit dosyaları, SBOM ve üçüncü taraf
  bildirimlerini **aynı sürüme bağla**"

## Neden — ölçülen boşluk

Depoda **hiç git etiketi yoktu** ve `git archive` hiçbir yerde çağrılmıyordu:
ikili paketle birlikte verilecek "buna karşılık gelen kaynak" hiç üretilmiyor,
SBOM'un `subject` bloğu (sürüm + commit + kilit özetleri) hiçbir kaynak
artefaktına bağlanmıyordu. AGPL-3.0 dağıtımında kaynak teslimi opsiyonel
değildir; "depo herkese açık" tek başına sürümle eşleşen kaynağı garanti
etmez (dal ilerler, commit kayar).

## Ne garanti ediliyor

1. **Arşiv commit ağacından üretilir** (`git archive <commit>`), çalışma
   ağacından değil → commit'lenmemiş/yerel dosya arşive sızmaz.
2. **İçerik kimliği platformdan bağımsızdır.** Manifest hem `.tar.gz`
   özetini (`archive.sha256`) hem **açılmış tar** özetini
   (`archive.tarSha256`) taşır; `check` gzip'i açıp ikincisini yeniden
   hesaplar. Ölçüldü: ubuntu (Python 3.12) ve macOS (Python 3.13) aynı
   commit'te **farklı** `.tar.gz` özeti üretti (zlib sürümü), ama açılmış tar
   özeti **birebir aynı**.
3. **Sürüm tek kaynaktan ve tekildir.** `tauri.conf.json`,
   `apps/desktop/package.json`, `packages/contracts/package.json` ve Cargo
   `[workspace.package].version` aynı olmalı; ayrışırsa üretim durur.
   (Kök `package.json` monorepo sürümüdür — 0.2.0 — bilinçli olarak bu bağa
   girmez; sürüm hizalama işi ayrı açık madde.)
4. **Etiket bağı doğrulanır.** `--tag <ad>` verilirse etiketin **o commit'e**
   baktığı ölçülür; etiketsiz üretim yalnız `--allow-untagged` ile (CI) ve
   manifestte `tag: null` olarak yapılır.
5. **Kilit dosyaları arşivin içinden bağlanır.** Dört kilit: `Cargo.lock`,
   `apps/desktop/src-tauri/Cargo.lock` (ikinci cargo çalışma alanı),
   `pnpm-lock.yaml`, `workers/document/pyproject.toml`. Özetler **arşivdeki
   baytlardan** hesaplanır, çalışma ağacından değil.
6. **SBOM aynı sürüme bağlanır.** `--sbom sbom.json` verilirse `subject`
   bloğu birebir doğrulanır: `commit`, `version`/`bundleVersion`/
   `cargoVersion` ve dört kilit özeti — biri kayarsa üretim durur.
7. **Üçüncü taraf bildirimi bağlanır.** `THIRD_PARTY_NOTICES.md` zorunlu
   dosyadır ve özeti manifestte taşınır.
8. **Doğrulama bağımsızdır.** `check`: sha256 + yan dosya, gzip'i açıp içerik
   özeti, dosya kümesi = `git ls-tree -r -z <commit>` (fazla/eksik yok),
   sürümün commit ağacından **yeniden** okunması, etiket bağı, zorunlu
   lisans/kaynak dosyaları, kilit özetleri, sır/çöp yolu reddi.
9. **Sır sızıntısı kapısı.** Arşivde `.env`, `id_rsa`, `*.pem`, `*.key`,
   `.venv`, `node_modules`, `target` gibi yol varsa üretim **durur**:
   "kaynak teslimi" nedeniyle yayımlanacak ağaçta sır olmadığı ölçülür.

## Komutlar

```bash
# Yayın anında (etiket zorunlu; SBOM aynı commit'te üretilmiş olmalı)
bash scripts/sbom.sh
git tag -a v0.1.0 -m "kaynak sürümü 0.1.0"
python3 scripts/source_release.py build --tag v0.1.0 --sbom sbom.json --out dist
python3 scripts/source_release.py check dist/dim-source-0.1.0-<kısa>.tar.gz --sbom sbom.json

# CI her push'ta (henüz etiket yok): gerçek repodan üretir ve doğrular
python3 scripts/source_release.py --self-test
python3 scripts/source_release.py build --ref HEAD --out "$RUNNER_TEMP/kaynak" --allow-untagged
python3 scripts/source_release.py check "$RUNNER_TEMP/kaynak"/*.tar.gz
```

Çıktı üçlüsü: `dim-source-<sürüm>-<kısa>.tar.gz`, `.sha256` yan dosyası,
`.manifest.json` (sürüm, commit, etiket, arşiv/içerik özeti, kilit özetleri,
bildirim özeti, SBOM bağı, `generatedAt` — manifestin tek değişken alanı).

## Ölçüm (gerçek repo, commit `07c5df3`)

| Ölçüm | Değer |
|---|---|
| Arşiv | `dim-source-0.1.0-07c5df3e.tar.gz` · **431 dosya** · **2.260.254 bayt** (açılmış **10.147.840**) |
| İçerik (tar) sha256 | `b58a61363d2d35a6f3d1d149449c1c51d378ab7a2198d0ab61a04d5bae90d082` — **ubuntu CI ve macOS birebir** |
| `.tar.gz` sha256 | macOS `209e016d…`, ubuntu CI `930c6a3b…` — zlib sürümüne bağlı, bu yüzden kimlik **tar** özetidir |
| Determinizm (aynı ortam) | art arda üretimler aynı `.tar.gz` özetini verdi: `02c63da` üç koşu, `07c5df3` iki koşu |
| CI işi | `kaynak` (ubuntu, Python 3.12): self-test + üretim + doğrulama **yeşil**, artefakt `kaynak-arsivi` |
| SBOM bağı | `bash scripts/sbom.sh` → `subject.commit = 07c5df3…`, dört kilit özeti dolu; `sbom_check.py` yeşil (rust 488, node 7+255, python 5+1) |

**Kapı kanıtı: 13/13 tek satır mutasyonu self-test'i kırmızıya düşürdü**
(kilit özeti, içerik özeti, zorunlu dosya — üretim ve doğrulama, ağaç
eşitliği, bozuk arşiv, kayan etiket, sürüm ayrışması, izlenen sır dosyası,
SBOM commit kayması, deterministik gzip, manifest etiket alanı, `ls-tree -z`);
geri yükleme sha256 birebir, son self-test yeşil.

Self-test fixture'ı **Türkçe dosya adı** taşır (`docs/adr/türkçe-ad.md`):
`git ls-tree` varsayılan tırnaklaması (`core.quotePath`) bu depoda gerçek bir
kusur üretmişti — kapı artık o tuzağı kendisi ölçer.

## Dürüst sınırlar

- **Etiket yok.** Depoda hiç git etiketi yok; `--tag` yolu yalnız fixture'da
  (self-test) ölçüldü, gerçek bir etiketle **denenmedi**. Etiket oluşturmak
  yayın adımıdır; oluşturulduğunda `build --tag` ve `check` bağı iki yönlü
  doğrular.
- **`.tar.gz` özeti ortamlar arasında değişir** (zlib sürümü). Kimlik olarak
  `tarSha256` kullanılmalı; `.sha256` yan dosyası taşıma kabuğunun özetidir.
- **İçerik taraması yok:** sır kapısı **yol/ad deseniyle** çalışır; zararsız
  adlı bir dosyanın içine gömülü sır yakalanmaz. İmza/attestation yok
  (ADR-0012: dağıtım imzasız).
- **Yalnız izlenen dosyalar** arşivdedir; `sbom.json` gibi üretim çıktıları
  depoda izlenmediği için arşive **girmez** — yayında `--sbom` ile ayrıca
  bağlanır.
- **Zorunlu dosya listesi bir tabandır**; tamlık, dosya kümesinin commit
  ağacına birebir eşitliğiyle sağlanır. Alt modül yok; `.gitattributes`
  `export-ignore` kullanmıyor (kullanılsaydı ağaç eşitliği kırmızı verirdi).
- Dosya izinleri (`chmod`) ve mtime **commit'ten** gelir (git archive
  semantiği); arşiv yeniden paketlenirse bu alanlar korunmaz — `check` bunu
  ağaç eşitliğiyle değil, yalnız içerikle ölçer.
- Gerçek ikili paket (imzasız `.dmg`/`.msi`) üretimi ayrı hattır; bu belge
  yalnız **kaynak** teslimini kapsar.
