# F15 — Kapasite ve gecikme ölçümü

- **Tarih:** 2026-10-24 (2. tur: düzeltme sonrası ölçüm)
- **Durum:** Koşucu hazır; arama ölçeklenmesi düzeltildi (**p95 2.100 ms → 170 ms**
  @3.000 segment). 100k hedefi hâlâ **ölçülmedi** (donanım/disk); doğrusal
  eğriyle dış değerleme ~5 sn → beta hedefi (p95 ≤ 2 sn) bu makinede
  **karşılanmıyor**.
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
| 100 belge / 300 segment | 0,27 sn | 74 ms | 94 ms | 115 ms | 0,6 ms | 1,5 MB |
| 1.000 belge / 3.000 segment (**önce**) | 2,77 sn | **1.554 ms** | **2.100 ms** | 4.497 ms | 80 ms | 11,5 MB |
| 1.000 belge / 3.000 segment (**sonra**) | 2,66 sn | **140 ms** | **170 ms** | 192 ms | 86 ms | 11,5 MB |
| 3.333 belge / 9.999 segment (**sonra**) | 15,0 sn | **425 ms** | **517 ms** | 579 ms | 265 ms | 37 MB |

Sorgu grubuna göre (p50 / p95):

| Grup | 3.000 segment (önce) | 3.000 segment (sonra) | 10.000 segment (sonra) |
|---|---|---|---|
| Esas/karar numarası (`2024/N E.`) | 112 / 115 ms | **3,7 / 3,9 ms** | 6,6 / 6,7 ms |
| Tırnaklı ifade | 603 / 655 ms | **166 / 179 ms** | 510 / 557 ms |
| Tek kelime (esnek) | 1.669 / 4.181 ms | **138 / 146 ms** | 424 / 445 ms |
| Türkçe karaktersiz (esnek) | 1.691 / 4.176 ms | **141 / 156 ms** | 425 / 448 ms |
| **Genel** | 1.554 / 2.100 ms | **140 / 170 ms** | 425 / 517 ms |

**Kazanç: 3.000 segmentte p50 11×, p95 12,4×.** Ölçeklenme hâlâ **doğrusal**:
segment sayısı 3,3× artınca p95 3,0× arttı.

## Düzeltmenin içeriği (ölçümle bulundu, ölçümle doğrulandı)

1. **Doğrulama artık sorgu başına bir kez hazırlanıyor.** Eskiden
   `dim_search_match(s.raw_text, iğne, kip)` her aday satır için iğneyi yeniden
   tokenize ediyor ve segment metnini yeniden normalleştiriyordu. Şimdi
   `SearchMatcher` iğneyi bir kez hazırlar, satır başına yalnız samanlık taranır
   ve ayırma yapılmaz (`token_spans` bayt aralıkları + yeniden kullanılan
   tamponlar).
2. **`flexible` doğrulaması `normalized_text` üzerinde.** Sütun zaten
   `dim_evidence::normalize` çıktısı; her satırda yeniden normalleştirme yok.
   `exact`/`phrase`/`docket` ham metin üzerinde çalışır (Türkçe `ı`/`i` ayrımı
   ve rol harfi korunur).
3. **Esas/karar numarasında aday daraltma:** eskiden yalnız **yıl** aranıyordu;
   şimdi yıl + numara bitişik ifade olarak, baştaki sıfır varyantlarıyla ve son
   belirteç önek olarak (`2024/0123`, `2024/123E`). Kesin kural değişmedi.
4. **Sayımın planı değişti — en büyük tek kazanç.** `SELECT count(*) … FROM
   text_segments_fts JOIN …` planında SQLite her aday için **rastgele** birincil
   anahtar araması yapıyordu: 3.000 adayda **286 ms**. Aynı kümeyi
   `s.rowid IN (SELECT rowid FROM text_segments_fts WHERE … MATCH …)` ile veren
   plan `text_segments` üzerinde sıralı taramaya geçiyor: **<1 ms**. Kapsam
   aynı (aynı filtreler, aynı doğrulama); yalnız plan değişti.
5. **Sayım yalnız gerektiğinde:** sayfa bütün eşleşmeleri döndürdüyse
   (`has_more` yok) toplam dönen sayıdır; ikinci geçiş atlanır.

**Denenip vazgeçilen:** `count(*) OVER ()` pencere fonksiyonu. `bm25` FTS
yardımcı fonksiyonu alt sorgu düzleştirmesinde bağlamını kaybettiği için
`LIMIT -1` ile materyalizasyon gerekti; ölçümde **daha yavaş** çıktı
(3.000 segmentte p50 2.620 ms, p95 3.477 ms). Kaldırıldı, gerekçesi kodda yazılı.

## Doğruluk güvencesi

- `fast_matcher_agrees_with_reference_implementation`: hızlı doğrulayıcı,
  referans `search_match` ile 4 kip × 10 iğne × 12 zor metin (Türkçe büyük/küçük
  harf, noktalama, `2024-123`, `00123`, yapışık `123E`, boşluklu) üzerinde
  **birebir** aynı sonucu verir.
- `search_docket_narrowing_keeps_real_matches`: sıfırlı (`2024/0123`), boşluklu
  (`2024 / 123`) ve harfe yapışık (`2024/123E`) yazımlar eşleşir; yanlış yıl
  (`2023/123`) ve yanlış numara (`2024/12345`) eşleşmez.
- `search_total_from_single_pass_matches_verified_hits`: sayım doğrulanmış
  eşleşmeleri sayar (FTS'in bulup doğrulamanın reddettiği aday sayılmaz).
- Mevcut F07 arama testleri (kip semantiği, filtreler, keyset sayfalama, kapsam
  sınırı) değişmeden geçiyor: storage **178 test**.

## Dürüst sınırlamalar

- **100.000 belge / 300.000 sayfa hedefi ölçülmedi.** Bu makinede boş disk
  ~4,5 GB; 100k korpus ~370 MB DB üretir ama koşu süresi ve disk güvenlik
  eşiği buna izin vermiyor. Referans donanım (B-01: 16 GB RAM, 8+ CPU, SSD)
  gerekir.
- Ölçülen eğri doğrusal: 10.000 segmentte p95 517 ms → 100.000 segmentte
  ~5 sn dış değerlemesi. **Beta hedefi (p95 ≤ 2 sn) bu eğriyle karşılanmıyor.**
  Sıradaki kaldıraçlar: (a) sayfa sorgusunda da tüm adayları sıralama öncesi
  doğrulamak yerine "aday penceresi + tembel doğrulama", (b) toplam sayıyı
  arayüzde geciktirmeli/önbellekli göstermek, (c) `normalized_text` için
  kalıcı belirteç/konum indeksi.
- Ölçüm tek makine, tek koşu; sıcak/soğuk önbellek ayrımı yapılmadı.
- IPC ve arayüz gecikmesi yok; gerçek kullanıcı gecikmesi bundan **büyüktür**.
- OCR ve iş kuyruğu gecikmesi ölçülmedi (worker bu koşuda devrede değil).
- Bu tablo bir "geçti" belgesi değildir; 100k hedefi ölçülene kadar F15-T04
  **kısmi** kalır.

## Komut

```bash
DIM_BENCH_DOCS=1000 DIM_BENCH_PAGES=3 DIM_BENCH_OUT=/tmp/bench.json \
  cargo test -p dim-storage --test bench_scale -- --ignored --nocapture
```
