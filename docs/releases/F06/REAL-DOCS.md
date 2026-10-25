# Gerçek belgelerle ayrıştırma kanıtı

- **Tarih:** 2026-10-25
- **Kaynak:** kullanıcının `~/Downloads/ekler` klasörü (2,4 MB, 15 dosya)
- **Kural:** belgeler repoya **kopyalanmadı**; yalnız ayrıştırma sonuçları ölçüldü.

## Sonuçlar

| Dosya | Motor | Sayfa | Metin (karakter) | Uyarı |
|---|---|---|---|---|
| `EK 1.pdf` | pymupdf | 8 | 7 | 8 (metin katmanı yok) |
| `EK 13.pdf` | pymupdf | 1 | 1.164 | 0 |
| `EK 7.PDF` | pymupdf | 12 | 29.874 | 0 |
| `EK 4.udf` | uyap-udf | 1 | 29.474 | 3 |
| `EK 11_03_10_2025.udf` | uyap-udf | 1 | 80.012 | 3 |
| `EK 15.jpg` | image | 1 | 0 | 2 (OCR gerekli) |

## OCR (taranmış belgeler)

- **`EK 15.jpg`** (`eng`): **4.178 karakter**, örnek: "T.C. KONYA 17. NOTERLİĞİ …
  NURCAN TUNCER …". Metin okunuyor.
- **`EK 1.pdf` sayfa 1** (300 dpi, `eng`): **1.597 karakter**, ortalama güven
  **57,9**, 32 blok. Güven düşük olduğu için kalite uyarısı üretiliyor — skor
  uydurulmuyor.

## Bulgular

1. **UDF (UYAP) gerçek dosyalarla çalışıyor**: 29 KB ve 80 KB metin çıktı;
   sayfa ayrımı doğrulanmadığı uyarısı taşınıyor (dürüst sınır).
2. **Taranmış PDF'te metin katmanı gerçekten yok** (8 sayfada 7 karakter) — OCR
   zorunlu. Bu, "metin var" varsayımının yanlış olacağını gösteriyor.
3. **`tur` dil paketi bu makinede yok.** OCR `eng` ile çalışıyor ama Türkçe
   karakterlerde hata üretiyor (`NOTERLIGI`, `Tiirk`, `Gdrev`). Gerçek kullanım
   için `tesseract-ocr-tur` kurulmalı; kurulmazsa ürün `E_OCR_MISSING`/düşük
   güven uyarısıyla dürüst kalıyor.
4. **JPG hattı** `image` motoruna düşüyor ve OCR olmadan metin üretmiyor — doğru
   davranış; sessizce boş metin yazılmıyor.

## Tekrarlama

```bash
python3 -m venv .venv && .venv/bin/pip install -e "workers/document[all]"
.venv/bin/python - <<'PY'
from pathlib import Path
from dim_worker import engines
data = (Path.home()/"Downloads/ekler"/"EK 15.jpg").read_bytes()
r = engines.extract(data, "image/jpeg", ocr=True, ocr_lang="eng")
print(len("\n".join(p.text for p in r.pages)))
PY
```
