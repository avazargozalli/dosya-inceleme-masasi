# F15 — Kapasite ve gecikme ölçümü (ilk tur)

- **Tarih:** 2026-10-24
- **Durum:** Koşucu hazır, **ilk ölçüm alındı**; 100k belge hedefi henüz ölçülmedi
- **Koşucu:** `crates/storage/tests/bench_scale.rs` (elle, `#[ignore]`)

## Yöntem

- Kurgusal korpus, deterministik üretici (LCG): her belge `pagesPerDocument`
  sayfa, her sayfa 8 cümle × 12 kelime (46 kelimelik Türkçe hukuk sözlüğü) +
  `Esas 2024/N E.` satırı.
- Yükleme **gerçek çekirdek yolu** ile: `create_blob_record → create_document →
  add_version → start_parse_run → store_pages` (denetim kayıtları dahil).
- Arama: 200 sabit sorgu (50 tek kelime, 50 tırnaklı ifade, 50 Türkçe
  karaktersiz, 50 esas/karar numarası), her biri `auto` kip çözümüyle,
  `limit=50`, `include_total=true` (kullanıcı arayüzünde toplam sayı görünür).
- "İlk sayfa": son ayrıştırma koşusunun sayfalarının okunması (50 örnek).
- Ölçülen katman **yalnız çekirdek**; Tauri IPC ve arayüz gösterimi yok.

## Sonuçlar (bu makine: macOS 26.6.2, aarch64, 8 mantıksal CPU, SSD)

| Korpus | Yükleme | Arama p50 | Arama p95 | Arama max | İlk sayfa p95 | DB |
|---|---|---|---|---|---|---|
| 100 belge / 300 sayfa / 300 segment | 0,27 sn (366 belge/sn) | 74,4 ms | 93,8 ms | 115,1 ms | 0,57 ms | 1,5 MB |
| 1.000 belge / 3.000 sayfa / 3.000 segment | 2,77 sn (361 belge/sn) | **1.553,9 ms** | **2.099,9 ms** | 4.496,9 ms | 79,6 ms | 11,5 MB |

Sorgu grubuna göre (1.000 belge):

| Grup | p50 | p95 |
|---|---|---|
| Esas/karar numarası (`2024/N E.`) | 111,6 ms | 115,4 ms |
| Tırnaklı ifade | 603,2 ms | 654,8 ms |
| Tek kelime | **1.668,8 ms** | **4.180,5 ms** |
| Türkçe karaktersiz (esnek) | 1.691,4 ms | 4.176,3 ms |

## Bulgu: arama ölçeklenmiyor (kabul hedefinin çok altında)

B-01 hedefi **100.000 segmentte p95 ≤ 2 sn** (çekirdek + IPC + UI). Biz
**3.000 segmentte** çekirdekte p95 2,1 sn ölçtük. Aday sayısı 10 kat artınca
gecikme ~21 kat arttı; yani maliyet aday sayısıyla **doğrusal**.

Neden: `search_page` adayları FTS ile daraltıyor, ama kesin eşleşmeyi her aday
satır için `dim_search_match(s.raw_text, …)` Rust/SQL fonksiyonuyla doğruluyor.
Fonksiyon her çağrıda **tüm segment metnini yeniden normalleştiriyor** (Türkçe
harf sadeleştirme + tokenizasyon). 200 sorgu × 3.000 satır = 600.000 tam metin
normalleştirmesi. `include_total=true` sayım sorgusu da aynı doğrulamayı
yineliyor.

Kaba dış değerleme (ölçüm değil, uyarı): aynı eğri 100.000 segmentte sorgu
başına ~50–70 sn demek olurdu. Bu haliyle beta kapasite hedefi **karşılanmıyor**.

Olası düzeltme yolları (henüz uygulanmadı, sıradaki iş):

1. Doğrulamayı `s.normalized_text` üzerinde yapmak (kolon zaten var) — her satır
   için yeniden normalleştirmeyi kaldırır.
2. Normalleştirilmiş metni FTS belirteçleriyle önceden daraltmak; docket
   sorgusunda yıl yerine yıl+numara belirteçleri (baştaki sıfır varyantlarıyla).
3. `include_total` sayımını doğrulama fonksiyonundan bağımsız, yalnız FTS aday
   sayısıyla hesaplamak (sayı "yaklaşık" olarak etiketlenmeli) ya da ikinci
   sayfada hiç istememek (arayüz zaten yalnız ilk sayfada istiyor).
4. Doğrulama fonksiyonunu toplu (set tabanlı) çağırmak.

## Dürüst sınırlamalar

- 100.000 belge / 1.000.000 sayfa hedefi **ölçülmedi**: bu makinede disk boş
  alanı ~5 GB ve koşu süresi sınırlı. Koşucu `DIM_BENCH_DOCS` ile kademeli
  çalıştırılabilir; 100k için referans donanım (B-01: 16 GB RAM, 8+ CPU, SSD) ve
  boş disk gerekir.
- Ölçüm tek makine, tek koşu; sıcak/soğuk önbellek ayrımı yapılmadı.
- IPC ve arayüz gecikmesi yok; gerçek kullanıcı gecikmesi bundan **büyüktür**.
- OCR ve iş kuyruğu gecikmesi ölçülmedi (worker bu koşuda devrede değil).
- Bu tablo bir "geçti" belgesi değildir; arama ölçeklenmesi düzeltilmeden
  F15-T04/T05 kapanmaz.

## Komut

```bash
DIM_BENCH_DOCS=1000 DIM_BENCH_PAGES=3 DIM_BENCH_OUT=/tmp/bench.json \
  cargo test -p dim-storage --test bench_scale -- --ignored --nocapture
```
