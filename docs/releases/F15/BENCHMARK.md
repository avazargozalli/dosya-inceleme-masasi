# F15 — Kapasite ve gecikme ölçümü

- **Tarih:** 2026-10-24 (düzeltme öncesi/sonrası, debug) · **2026-10-25** (B-01 tam
  profil ölçümü, release)
- **Durum:** B-01 profili **artık ölçüldü** (1.000 belge / 100.000 segment / 200
  sabit sorgu). Sonuç: **p50 1,25 sn**, **p95 iki koşuda 1,53 sn ve 2,18 sn** → 2 sn
  hedefi **sınırda ve tutarlı biçimde karşılanmıyor**; asıl sorun kuyruk (tek
  sorguda 36–54 sn). 100.000 **belge** / 1 milyon sayfa kapasite testi (F15-T04)
  hâlâ yapılmadı.
- **Koşucu:** `crates/storage/tests/bench_scale.rs` (elle, `#[ignore]`)

## Yöntem

- Kurgusal korpus, deterministik üretici (LCG): her belge `pagesPerDocument`
  sayfa, her sayfa 8 cümle × 12 kelime (46 kelimelik Türkçe hukuk sözlüğü) +
  `Esas 2024/N E.` satırı. Üretici kimliği ve **sorgu listesinin sha256'sı**
  rapora yazılır (`corpusGenerator`, `queryListSha256`) — aynı girdi aynı
  korpusu ve aynı sorguları verir.
- Yükleme **gerçek çekirdek yolu** ile: `create_blob_record → create_document →
  add_version → start_parse_run → store_pages` (denetim kayıtları dahil).
  Faz süreleri ayrı raporlanır (`ingest.phaseMs`).
- Arama: 4 grupta 50'şer sabit sorgu (**toplam 200**): tek kelime (esnek),
  iki kelimeli ifade (ifade kipi), Türkçe karaktersiz (esnek), esas/karar
  numarası. Gruplar **dönüşümlü** koşar; her sorgu `auto` kip çözümüyle,
  `limit=50`, `include_total=true` (kullanıcı arayüzünde toplam sayı görünür).
- Aynı sorguların bir alt kümesi (`DIM_BENCH_PAGE_ONLY_EVERY`, varsayılan her
  10.) `include_total=false` ile de koşar; **ilk sayfanın birebir aynı olduğu
  doğrulanır** (20/20) ve toplam sayının maliyeti ayrışır.
- "İlk sayfa": son ayrıştırma koşusunun sayfalarının okunması (50 örnek).
- Bellek: süreç RSS'i 2 sn aralıkla örneklenir (`ps` ile; `unsafe` yok), tepe
  değer raporlanır.
- Ölçülen katman **çekirdek**; Tauri IPC ve arayüz gösterimi yok.

## Donanım ve derleme

| Alan | Değer |
|---|---|
| Makine | Apple **M2** (Mac14,7), 8 mantıksal CPU, **16 GB** RAM, SSD |
| OS | macOS 26.6.2 (aarch64) |
| Derleme | `cargo test --release` (optimize — ürün yapılandırması) |
| DB | SQLCipher (sayfa başına AES + HMAC), 280,8 MB / 100.000 segment (2.944 B/segment) |

> 2026-10-24 tabloları **optimize edilmemiş** (`cargo test`, debug) derlemeyle
> alınmıştı; 2026-10-25 satırları release. İki profil **birbiriyle
> karşılaştırılmaz**; ayrı başlıklarda verilir.

## B-01 tam profil — 2026-10-25 (release, 1.000 belge / 100.000 segment / 200 sorgu)

Aynı korpus ve aynı sorgu listesiyle **iki koşu** (tekrar oynaklığı kanıtı):

| Koşu | Yükleme | Arama p50 | Arama p95 | Arama max | Sorgu süresi |
|---|---|---|---|---|---|
| 1 | 46,9 sn (21,3 belge/sn · 2.132 sayfa/sn) | **1.240,9 ms** | **1.529,6 ms** | 53.880 ms | 268,9 sn |
| 2 | 44,3 sn (22,6 belge/sn · 2.259 sayfa/sn) | **1.248,4 ms** | **2.179,3 ms** | 35.988 ms | 277,2 sn |

- **p50 kararlı** (1.241 / 1.248 ms). **p95 kararsız** (1,53 / 2,18 sn): 2 sn hedefi
  bir koşuda sınırın altında, diğerinde üstünde → **hedef tutarlı biçimde
  karşılanmıyor**. Rapor `b01.coreP95Met` alanını buna göre yazar (`false`).
- **Kuyruk asıl sorun:** en yavaş sorgu `"numaralı reddine"` (ifade) **36,0 sn**
  (koşu 1'de 53,9 sn); `"takip talep"` 10,1 sn; `haciz` (karaktersiz) 6,4 sn;
  `sure` 5,6 sn; `esas` 5,1 sn. Ortak özellik: eşleşen aday sayısı 100.000'e
  yakın (`esas` sorgusunda toplam **100.000**).
- **İlk sayfa (100 sayfalık sürüm):** p50 **0,314 ms**, p95 **0,344 ms** (50 örnek).
- **Bellek:** tepe RSS **10,3 MB** (yükleme), arama sonrası **21,1 MB**.
- **Depolama:** 280,8 MB DB + 10,3 MB WAL.

### Toplam sayının maliyeti (kaldıraç kanıtı)

| Ölçüm | p50 | p95 |
|---|---|---|
| `include_total=true` (arayüzün bugünkü yolu) | 1.248 ms | 2.179 ms |
| `include_total=false` (yalnız ilk sayfa) | **605 ms** | **712 ms** |
| **Toplam sayının payı** | **+643 ms (~%52)** | +1.467 ms |

İlk sayfa 20/20 örnekte **birebir aynı** döndü (aynı segment kimlikleri ve sıra;
`searchPageOnly.identicalFirstPageChecked`). Yani toplam sayı geciktirilirse ilk
sayfa süresi yarıya iner — arayüz "N sonuç" alanını sayfa geldikten sonra
doldurabilir.

### Sorgu grubuna göre (koşu 2, p50 / p95 / max)

| Grup | p50 | p95 | max |
|---|---|---|---|
| Esas/karar numarası (`2024/N E.`) | **26,5 ms** | 33,5 ms | 106,1 ms |
| Tek kelime (esnek) | 1.259,9 ms | 1.425,9 ms | 4.373,4 ms |
| Tırnaklı ifade | 1.302,6 ms | 3.903,3 ms | **35.988,3 ms** |
| Türkçe karaktersiz (esnek) | 1.268,5 ms | 5.139,0 ms | 6.449,3 ms |

## Kademeli profiller (release)

| Korpus | Yükleme | Arama p50 | Arama p95 | DB |
|---|---|---|---|---|
| 100 belge / 3.000 segment | 1,25 sn | 40,3 ms | 54,0 ms | 8,5 MB |
| 1.000 belge / 100.000 segment | 44,3 sn | **1.248 ms** | **2.179 ms** | 280,8 MB |

Eğri doğrusala yakın (segment 33,3× → p50 31×; p95 40×, kuyruk nedeniyle).
p50 için segment başına maliyet ≈ **0,0124 ms**; 100.000 segmentte ölçülen
1,25 sn bu katsayıyla uyumlu.

## Arama düzeltmesinin geçmişi (2026-10-24, debug profil)

| Korpus | Arama p50 | Arama p95 | Not |
|---|---|---|---|
| 300 segment | 74 ms | 94 ms | başlangıç |
| 3.000 segment (**önce**) | **1.554 ms** | **2.100 ms** | `dim_search_match` satır başına yeniden normalleştiriyordu |
| 3.000 segment (**sonra**) | **140 ms** | **170 ms** | 11× / 12,4× kazanç |
| 9.999 segment (sonra) | 425 ms | 517 ms | doğrusal |

Grup tablosu (3.000 segment, önce → sonra): esas/karar 112/115 → **3,7/3,9 ms**;
ifade 603/655 → **166/179 ms**; tek kelime 1.669/4.181 → **138/146 ms**; Türkçe
karaktersiz 1.691/4.176 → **141/156 ms**.

Düzeltmenin içeriği (ayrıntı: `crates/storage/src/parsing.rs` yorumları):
matcher sorgu başına bir kez hazırlanır; `flexible` doğrulaması
`normalized_text` üzerinde; esas/karar numarasında yıl + numara daraltması;
sayım `s.rowid IN (SELECT rowid … MATCH …)` planıyla (<1 ms); sayım yalnız
gerekliyse. **Denenip vazgeçilen:** `count(*) OVER ()` (bm25 bağlamı kayboluyor)
ve `LIMIT -1` materyalizasyonu (p50 2.620 ms — daha yavaş).

## Ölçümle bulunan kusur: indekssiz `text_segments.page_id` (2026-10-25)

`pages_for_run` her sayfa için `text_segments` tablosunu `page_id` ile **üç kez**
arıyordu ve bu sütunda indeks yoktu → her sayfa okuması **tam tablo taraması**.
SQLCipher her satırı çözüp HMAC doğruladığı için maliyet satır sayısıyla
(kasada 100.000) çarpılıyordu.

**A/B ölçümü** (aynı kasa, tek sayfalık koşu, `DIM_BENCH_PAGE_AB=1`):

| Durum | Sayfa başına p50 | 100 sayfalık belge (hesaplanan) |
|---|---|---|
| İndeks **yok** | **914,9 ms** | **~91,5 sn** |
| İndeks **var** | **0,098 ms** | **~9,8 ms** |
| Kazanç | **~9.336×** | ~9.300× |

- Düzeltme: migration **0018** (`idx_text_segments_page`). `pages` tarafında ek
  indeks gerekmedi: `UNIQUE (parse_run_id, page_index)` otomatik indeksi sorgu
  planında kullanılıyor (plan testi: `pages_for_run_plan_uses_indexes`).
- Düzeltme sonrası ölçülen **ilk sayfa** gecikmesi: p50 **0,314 ms** (100
  sayfalık sürüm). Düzeltmeden önce aynı döngü (50 örnek) **12+ dakikada
  bitmedi** ve koşu durduruldu; bu yüzden "önce" değeri A/B ile ayrı ölçüldü.
- Regresyon testleri: `pages_for_run_plan_uses_indexes`,
  `pages_for_run_reads_only_its_own_run`,
  `pages_for_run_returns_first_segment_of_a_page` (indeks eklendikten sonra da
  sayfanın **ilk** segmenti dönüyor).

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
- Toplamsız arama ilk sayfayı **değiştirmez**: her 10. sorguda iki yolun döndürdüğü
  segment kimlikleri birebir karşılaştırılır (20/20 eşit).
- Mevcut F07 arama testleri (kip semantiği, filtreler, keyset sayfalama, kapsam
  sınırı) değişmeden geçiyor: storage **267 test**.

## Dürüst sınırlamalar

- **B-01 toplam süreyi kapsar** (çekirdek + IPC + arayüz); buradaki sayılar
  çekirdek alt sınırıdır, gerçek kullanıcı gecikmesi **daha büyüktür**.
- **Tek makine** (Apple M2, 16 GB). B-01 referans cihaz tanımına uyar; ama
  Windows/Linux ve 8 GB profil **ölçülmedi**. Sonuç başka cihazlara genellenmez.
- **Kuyruk hedefi tutmuyor:** p95 sınırda, tek sorguda **36–54 sn**. "p95 hedefi
  tuttu" demek için yeterli değil; kullanıcı tek bir ifade aramasında dakika
  bekleyebilir.
- **Kurgusal korpus.** Gerçek hukuk metinlerinde kelime dağılımı, sayfa uzunluğu
  ve segment sayısı farklı; burada sayfa başına **1 segment** var, gerçek
  belgelerde birden fazla olabilir.
- **Sıcak önbellek**, iki koşu; soğuk başlangıç ve boş disk önbelleği ölçülmedi.
  İki koşu arasındaki p95 farkı (1,53 → 2,18 sn) tekrarların ne kadar oynadığını
  gösterir.
- **OCR ve iş kuyruğu gecikmesi ölçülmedi** (worker devrede değil); 8 GB çekirdek
  profili ve 100.000 **belge**/1 milyon sayfa kapasite testi (F15-T04) yapılmadı.
- **Bellek** yalnız süreç RSS'i; SQLite sayfa önbelleği ve işletim sistemi
  önbelleği ayrı raporlanmaz. Örnekleme 2 sn aralıkla (tepe değer yaklaşık).
- Bu tablo bir "geçti" belgesi değildir. Hedef tutmuyorsa güvenlik veya doğrulama
  adımı kaldırılmaz; kapasite iddiası ölçülen sınırla daraltılır.

## Sıradaki kaldıraçlar (ölçümle desteklenen)

1. **Toplam sayıyı geciktir** — ölçüldü: yalnız ilk sayfa 605 ms, toplam sayıyla
   1.248 ms (~%52). Arayüz ilk sayfayı hemen gösterip sayıyı sonra isteyebilir.
2. **Aday penceresi + tembel doğrulama** — kuyruk, aday sayısı 100.000'e yakın
   sorgularda oluşuyor (`esas`, `haciz`, `sure`). İlk 50 sonuç için tüm adayları
   doğrulamak zorunlu değil; sıralama `bm25` olduğu için pencere büyütülerek
   kesinlik korunabilir (tasarım değişikliği, ayrı iş).
3. **Kalıcı belirteç/konum indeksi** (`normalized_text` için) — doğrulamayı satır
   taramasından çıkarır; en büyük kazanç, en pahalı iş.

## Komutlar

B-01 tam profil (bu belgedeki sayılar):

```bash
DIM_BENCH_DOCS=1000 DIM_BENCH_PAGES=100 DIM_BENCH_QUERIES_PER_GROUP=50 \
  DIM_BENCH_PAGE_AB=1 DIM_BENCH_OUT=/tmp/bench-b01.json \
  cargo test --release -p dim-storage --test bench_scale -- --ignored --nocapture
```

Küçük profil:

```bash
DIM_BENCH_DOCS=100 DIM_BENCH_PAGES=30 DIM_BENCH_QUERIES_PER_GROUP=5 \
  DIM_BENCH_PAGE_ONLY_EVERY=0 DIM_BENCH_OUT=/tmp/bench-small.json \
  cargo test --release -p dim-storage --test bench_scale -- --ignored --nocapture
```

Rapor JSON'u: `machine` (CPU modeli, RAM, çekirdek), `corpus`,
`queryList` + `queryListSha256`, `perQuery` (sorgu bazında süre/hit/toplam),
`ingest.phaseMs`, `search`, `searchPageOnly`, `searchByGroup`, `firstPage`,
`pageLookupAb`, `memory`, `b01`, `storage`.
