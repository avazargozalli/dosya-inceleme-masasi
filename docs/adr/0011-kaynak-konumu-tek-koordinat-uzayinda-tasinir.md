# ADR-0011 — Kaynak konumu tek koordinat uzayında taşınır

- **Durum:** Kabul edildi
- **Tarih:** 2026-10-23
- **İlgili:** F06-T08, F07 okuyucu, F13 redaksiyon

## Bağlam

Metin çıkarımı üç farklı kaynaktan gelir: PDF (punto), görüntü/TIFF (piksel) ve
yapısal belgeler (DOCX/UDF/plain — sayfa koordinatı yok). Alıntının sayfadaki
yerini göstermek, redaksiyon bölgesi üretmek ve "kaynağa git" akışını kurmak
için kutuların **hangi uzayda** olduğu tartışmasız olmalıdır. Aksi hâlde
döndürülmüş bir PDF'te veya kırpma kutusu kaydırılmış bir sayfada vurgu yanlış
yere düşer.

## Karar

1. Her sayfa kaydı bir `coordinate_space` adı taşır:
   - `unrotated_cropbox_points`: PDF. Origin kırpma kutusunun (cropbox) sol
     üstüdür, y aşağı, birim punto. `pages.width`/`height` **döndürme öncesi**
     kırpma kutusu ölçüsüdür.
   - `image_pixels`: görüntü/TIFF kareleri. Origin sol üst, birim piksel.
   - `structure`: DOCX/UDF/plain. Kutu yoktur; yalnız karakter aralıkları
     vardır. Kutu uydurulmaz.
2. Döndürme ayrı alanda (`pages.rotation`, 0/90/180/270) taşınır. Koordinatlar
   **asla** döndürülmüş uzayda saklanmaz. Görüntülemek için tüketici
   `rotation_matrix` eşdeğerini uygular (saat yönünde):
   - 90°: `(x, y) → (H − y, x)`, görünen boyut `(H, W)`
   - 180°: `(x, y) → (W − x, H − y)`
   - 270°: `(x, y) → (y, W − x)`
3. OCR kutuları 300 dpi'de üretilir, `72/dpi` ile puntoya çevrilir ve
   `derotation_matrix` ile ortak uzaya geri alınır. Geri alınmazsa kutu sayfa
   dışına taşar; test bunu doğrular.
4. Segment metni ile kutular **aynı çıkarımdan** üretilir (`get_text("dict")`).
   Ayrı bir `get_text("text")` çağrısıyla offset/kutu kayması kabul edilmez.
5. Blok kaydı `{"start","end","bbox","lines"}` alanlarını taşır; `start`/`end`
   Unicode scalar offsetidir ve segmentin kendi metnine göredir. Kutu yoksa
   `bbox` yazılmaz.
6. Kutu hassasiyeti **blok düzeyindedir**; satır offsetleri saklanmadığı için
   alıntı, düştüğü blokların kutularıyla gösterilir. Bu sınır arayüzde
   "konum haritası" olarak, sayfa görüntüsü diye sunulmadan gösterilir.

## Sonuçlar

- Okuyucu ve redaksiyon aynı uzayı kullanır; döndürme tek yerde uygulanır.
- Eski kayıtlarda `coordinate_space` NULL'dur: koordinat yok sayılır, tahmin
  edilmez. Migrasyon geri doldurma yapmaz.
- Satır düzeyinde hassasiyet gerektiğinde (ör. kelime vurgusu) satır offsetleri
  ayrıca üretilmelidir; bugünkü kayıt bunu vaat etmez.
