# F01-T06 — DOCX/UDF yapı, ek ve değişiklik envanteri (ölçüldü, 25 Eki 2026)

**Soru:** izinli DOCX/UDF örneklerinde belge **yapısı**, **ekleri** ve
**değişiklik envanteri** gerçekten çıkarılabiliyor mu; çıkarılamayan ne var?

**Kısa cevap:** DOCX'te üçü de çıkarılıyor (gömülü nesne/altChunk **içeriği**
hariç — sayılıyor ama açılmıyor). UDF'te yapı ve paket girdileri çıkarılıyor;
UDF şemasında izlenen değişiklik kavramı **yok** ve gerçek dosyalarda tanınmayan
öğeler var. Sayılar aşağıda; hiçbiri tahmin değil.

## Yöntem

Envanter **paket düzeyinde** çıkarılır: ZIP üyeleri + XML parçaları. Metin
çıkarımından (python-docx/PyMuPDF) bağımsızdır; bu yüzden python-docx'in gövde
metnine katmadığı içerik "yok" sayılmaz, **sayılır** ve çıkarılamadığı yazılır.

- Çekirdek: `workers/document/dim_worker/inventory.py` (`docx_inventory`, `udf_inventory`).
- Protokol: worker `inventory` yöntemi (`contentBase64` + `mime`).
- Masaüstü: `document_inventory(versionId)` — blob kasadan çözülür, worker'a
  gider; **kasaya hiçbir şey yazılmaz** (ayrıştırma koşusu açmaz). Denetime
  yalnız sayılar yazılır (ek adı, metin, yol girmez).
- Arayüz: okuyucu panelinde **“Belge envanteri”** düğmesi; DOCX/UDF dışında
  kapalı ve gerekçesi yazılı.
- Dışarı verilen bilgi en az: eklerin adı/boyutu/türü, değişiklik sayısı/yazar/
  tarih, dış bağlantı **sayısı** (adres yok). Gövde metni yanıtta yok — testle
  doğrulanıyor (fikstür işaretçileri yanıtta aranıyor).

## Ölçüm 1 — kurgu DOCX fikstürü

`fixtures/synthetic/docx-inventory.docx` (4.378 bayt, bağımlılıksız üretim:
`fixtures/synthetic/generate.py`). İçerik: gövde paragrafları, tablo, izlenen
ekleme/silme, dipnot, yorum, metin kutusu, içerik denetimi, gömülü OLE nesnesi,
görsel, altChunk, dış bağlantı, XML imza parçası, belge koruması.

| Alan | Ölçülen |
|---|---|
| Yapı | 9 paragraf · 1 tablo · 1 satır · 2 hücre · 1 bölüm (`sectPr`) · 2 stil |
| Gizli katman | 1 metin kutusu · 1 içerik denetimi · 1 üstbilgi · 1 dipnot · 1 yorum |
| Görsel öğe | 1 `w:drawing` · 1 `w:pict` · 1 medya parçası |
| Ekler | **3 ek / 210 bayt**: OLE nesnesi 84 B (`ProgID=Excel.Sheet.12`), görsel 82 B, altChunk 44 B |
| Değişiklik izi | 1 ekleme · 1 silme · 1 yorum · yazarlar: `Av. Deneme`, `Karsi Taraf` · 2026-10-01 → 2026-10-02 |
| Künye | revizyon **12** · son kaydeden `Av. Karsi` · oluşturan `Av. Deneme` · `trackRevisions=true` · koruma `edit=readOnly` |
| Diğer | 1 XML imza parçası (**doğrulanmadı**) · 1 dış bağlantı (adres gösterilmez) |

Sızıntı kontrolü: fikstürdeki 8 işaretçi (`ZZ-OLE-…`, `ZZ-GORSEL-…`,
`ZZ-ALTCHUNK-…`, `ZZ-IMZA-…`, gövde/dipnot/yorum metni) yanıtın **hiçbir**
alanında yok.

## Ölçüm 2 — kurgu UDF fikstürü

`fixtures/synthetic/udf-inventory.udf` (715 bayt): 1 `page`, 5 paragraf, 1 tablo
(1 satır/1 hücre), 1 görsel, 1 üstbilgi, 1 altbilgi, 1 tanınmayan öğe, 3 paket
girdisi (görsel 108 B, gömülü PDF 19 B, imza dosyası 66 B).

Sonuç: yapı sayıları ve paket girdileri çıktı; `changes` alanı UDF için boş
(`trackRevisions=null`) ve notta “UDF'te izlenen değişiklik kavramı yok” yazıyor.
İmza dosyası **tespit edildi — kriptografik doğrulama yapılmadı**.

## Ölçüm 3 — gerçek (kullanıcıya ait, izinli) UDF dosyaları

Kullanıcının kendi klasöründeki **8 gerçek UDF** üzerinde yalnız **sayılar**
ölçüldü; dosyalar repoya kopyalanmadı, adları ve metinleri bu belgeye yazılmadı.

| Ölçüm | Sonuç (8 dosya) |
|---|---|
| Yapı çıkarımı | **8/8 başarılı** |
| Paragraf | 25 – 238 |
| Tablo / hücre | 5 dosyada tablo yok; en çok 2 tablo / 19 hücre |
| Alan (`field`) | 4 dosyada 6–30 |
| Üstbilgi/altbilgi | 7/8 dosyada en az biri var |
| Paket girdisi (content.xml dışı) | her dosyada **1–2** |
| İmza girdisi | **0/8** (imza dosyası yok) |
| `page` öğesi | **0/8** → UDF sayfa sayısı bildirilmiyor |
| Tanınmayan öğe | **8/8** dosyada 1–16 arası |

**Bulgu:** gerçek UDF'lerde her dosyada tanınmayan öğe var (en çok 16). Envanter
bunları **gizlemiyor**, `unknown` olarak sayıyor ve notta yazıyor. Bu, UDF şema
kapsamının eksik olduğunu gösterir: o öğelerin taşıdığı içerik (ör. üstveri,
damga, özel alan kutuları) metne katılmıyor. Ayrıca gerçek dosyalarda `page`
öğesi bulunmadığı için **sayfa ayrımı hiçbir gerçek UDF'te doğrulanamıyor** —
`docs/releases/F06/REAL-DOCS.md`'deki uyarı ölçümle doğrulandı.

## Dürüst sınırlar

- **Sayım XML işaretlerine dayanır.** `w:ins`/`w:del` sayısı Word'ün değişiklik
  panelindeki gruplamayla birebir aynı olmayabilir (aynı düzenleme birden çok
  işaret taşıyabilir).
- **İçerik çıkarılmıyor:** gömülü nesne (OLE), gömülü paket ve altChunk yalnız
  ad/boyut/tür olarak listelenir; metnine erişilmez, yeniden kullanılmaz.
- **Metin kutusu metni** gövde metnine katılmıyor (python-docx); envanter
  sayısını verir, içeriğini vermez.
- **İmza doğrulanmaz.** DOCX `_xmlsignatures/*` ve UDF imza girdisi yalnız
  “tespit edildi” olarak bildirilir; kriptografik doğrulama yapılmadı.
- **UDF'te değişiklik izi yok** — kavram yok, alan boş bırakılır (sıfır
  uydurulmaz, `null` döner).
- **Dış bağlantı adresleri gösterilmez**; ürün ağ isteği göndermez.
- **Yalnız DOCX/UDF.** PDF/görüntü/düz metin için düğme kapalı ve gerekçeli
  (`E_INVENTORY_UNSUPPORTED`).
- XLSX/PPTX envanteri yok (kapsam dışı; aynı OOXML altyapısı kullanılabilir).
- Tauri penceresi bu ortamda açılmadı → kartın **görsel QA'sı yok**; alanlar tip
  denetimi, birim testleri ve üretim derlemesiyle doğrulandı.

## Kanıt

| Katman | Test |
|---|---|
| Python | `workers/document/tests/test_inventory.py` — **14 test** (yapı, ekler, sızıntı, DTD, zip kaçışı, şifreli giriş, liste sınırı, UDF) |
| Rust (birim) | `crates/ingest/src/inventory.rs` — **7 test** (tam yanıt, eski worker toleransı, tür kapısı, yol benzeri ek adı reddi, denetim sayıları) |
| Rust (gerçek worker) | `crates/ingest/tests/document_inventory.rs` — **3 test**: fikstür → gerçek worker, iki yönlü kanıt (işaretçi kaynakta var, yanıtta yok) |
| Masaüstü komutu | `commands.rs` — MIME kapısı testi (`docx`/`udf` açık, `pdf`/`jpeg`/`text`/`xlsx` kapalı) |
| Arayüz | `apps/desktop/src/lib/inventory.test.ts` — **14 test** (sıralama, sıfır gizleme, bilinmeyen anahtar, ek özeti, değişiklik künyesi, uyarılar, kapı) |

## Tekrarlama

```bash
cd workers/document && .venv/bin/python -m pytest tests/test_inventory.py -q

# Fikstürleri yeniden üret
python3 fixtures/synthetic/generate.py

# Gerçek worker ile envanter (Rust)
cargo test -p dim-ingest --test document_inventory

# Tek dosya için sayıları gör (kurgu fikstür)
cd workers/document && .venv/bin/python - <<'PY'
from pathlib import Path
from dim_worker import inventory
data = Path("../../fixtures/synthetic/docx-inventory.docx").read_bytes()
print(inventory.docx_inventory(data).to_dict())
PY
```
