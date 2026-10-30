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
  doğrulanır** (20/20) ve toplam sayının maliyeti ayrışır. Aynı örnekte
  `search_total` de çağrılır: dönen sayı sayfa toplamıyla **birebir aynı olmak
  zorunda** (`assert_eq!`; rapor `searchTotalDeferred.matchesPageTotal`).
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

### Ertelenmiş sayım uygulandı (2026-10-26)

Kaldıraç koda ve arayüze geçti: sayfa `include_total=false` ile istenir, sayı
**ayrı** `search_segments_total` komutuyla, ilk boyamadan sonra alınır.

- **Çekirdek:** `Database::search_total(case_id, query, filters)` → `SearchTotal`
  (`total`, `tookMs`, `resolvedQuery`, `mode`, `filters`). Sayım kuralı
  sayfayla **aynı** (`count_where`: doğrulamayı geçen satırlar); "en çok N" ya da
  pencere kırpması **yok**, sayı tamdır.
- **Arayüz:** sonuç başlığında önce "Eşleşme sayısı hesaplanıyor…", sayı gelince
  "N eşleşme · sayım X ms sürdü". Sayı **sayfayla aynı sorguya** ait değilse
  (kip/çözümlenmiş metin farklı) gösterilmez; hata varsa gerçek mesaj yazılır.
- **Kanıt (bu turda koşuldu, 2.000 segment / 20 sorgu, debug):**

| Ölçüm | p50 | p95 | max |
|---|---|---|---|
| `include_total=true` (eski yol) | 100,2 ms | 118,1 ms | 126,9 ms |
| Yalnız ilk sayfa (`include_total=false`) | **53,1 ms** | **58,1 ms** | 59,9 ms |
| Ayrı sayım (`search_total`) | 45,0 ms | 58,9 ms | 59,1 ms |

  Sayfa + sayım ≈ eski tek çağrı (53,1 + 45,0 ≈ 98 ≈ 100,2) — ayrıştırma tutarlı.
  **Sayı eşitliği 20/20 örnekte doğrulandı** (`searchTotalDeferred.matchesPageTotal=true`);
  ilk sayfa da 20/20 birebir aynı. Koşucu artık bu karşılaştırmayı her örnekte
  `assert_eq!` ile yapıyor, yani sayı ayrışırsa ölçüm **düşer**.

Bu profil B-01'in altında (2.000 segment / 20 sorgu) — hedef hükmü değildir;
yalnız ayrıştırmanın ve eşitliğin kanıtıdır. B-01 ölçeğinde beklenen kazanç
yukarıdaki tablodan: p50 1.248 → ~605 ms, p95 2.179 → ~712 ms (kullanıcının
beklediği süre). **Kuyruk değişmez**: `"numaralı reddine"` gibi sorgularda 36–54 sn
süren kısım sayım değil, aday doğrulamasının kendisidir (sonraki kaldıraç).

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

## Aday penceresi + tembel doğrulama — 2026-10-26 (debug, aynı oturumda A/B)

**Kusur:** doğrulama SQL'in `WHERE`'indeydi (`AND dim_search_verify(...)=1`).
SQLite bu yüklemi `ORDER BY bm25` sıralamasından **önce** uyguladığı için sayfa
20 sonuç istese de FTS adaylarının **tamamı** doğrulanıyordu; 2.000 segmentlik
profilde kelime sorgularında 1.700–1.980 aday, 100.000 segmentte on binlerce
aday demekti. Ölçülen kuyruk (36–54 sn) buradan geliyordu.

**Düzeltme:** sayfa sorgusu adayları `bm25, segment_id` sırasında **akıtır**
(`LIMIT` yok), doğrulamayı Rust'ta `SearchMatcher` ile 200 satırlık pencereler
hâlinde yapar ve sayfa dolduğu an (`limit+1` eşleşme) taramayı keser.
`SearchPage.verification` kaç adayın gerçekten doğrulandığını taşır; arayüz
"*N aday doğrulandı · X ms · sayfa doldu, tarama kesildi*" satırını gösterir.

**A/B (aynı makine, aynı korpus, aynı sorgu listesi `472fe8ac…`,
200 belge / 2.000 segment / 20 sorgu, debug, arka arkaya iki koşu):**

| ölçü | eski (SQL doğrulama) | yeni (aday penceresi) | kazanç |
|---|---|---|---|
| ilk sayfa p50 | 44,73 ms | **19,64 ms** | **2,28×** |
| ilk sayfa p95 | 51,70 ms | **29,29 ms** | 1,77× |
| ilk sayfa max | 52,53 ms | 32,89 ms | 1,60× |
| toplamlı arama p50 | 84,99 ms | 61,38 ms | 1,38× |
| ayrı sayım p50 | 40,08 ms | 40,95 ms | değişmedi (bilinçli) |

Sorgu grubu bazında (sayfa, p50): kelime 50–58 → **20–23 ms**; ifade 55–60 →
29–35 ms; esas/karar no 2,4–3,3 → 2,3–2,5 ms (aday kümesi zaten küçük).

**Doğrulanan aday:** ortalama **135,95**, tepe **580** (3 pencere); 20 örneğin
**15**'inde erken kesme. Eski kodda her örnekte adayların tamamı doğrulanıyordu.

**Doğruluk:** ilk sayfa 20/20 birebir aynı (aynı segment kimlikleri), toplam
sayı 20/20 eşit. Birim testleri pencere boyutunu 1, 2, 3, 4, 64, 200 vererek
sonucun, sıranın ve imlecin değişmediğini; ilk pencerede eşleşme yoksa taramanın
devam ettiğini denetler.

**Sınır (dürüst):** kazanç **2.000 segmentte** ölçüldü; **100.000 segmentte
kuyruk yeniden ölçülmedi** (disk 2 GB — 100k ingest + release derleme
yapılmadı). Mekanizma aynı olduğu için beklenen kuyruk düşüşü, ilk 21 eşleşmenin
kaç adayda bulunduğuna bağlı: eşleşme yoğun sorgularda tarama ilk pencerelerde
kesilir; **eşleşme çok seyrekse** (ör. 50.000 adayda 5 eşleşme) tarama yine
tüm adayları gezer — kuyruk **tamamen yok olmaz**, yalnız eşleşme bulunan yerde
kısalır. Tam sayım (`search_total`) doğrulamayı hâlâ SQL'de yapar ve tüm
adayları gezmek zorundadır; bu yüzden ölçümde **değişmedi** ve arayüzde ayrı
(sonradan) istenir.

Ham rapor: `docs/releases/F15/bench-window-2026-10-26.json`.

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
- **İş kuyruğu gecikmesi ölçüldü** (2026-10-26, aşağıdaki bölüm); **OCR gecikmesi
  ölçüldü** (2026-10-29, aşağıdaki bölüm). 8 GB çekirdek profili ve 100.000
  **belge**/1 milyon sayfa kapasite testi (F15-T04) yapılmadı.
- **Bellek** yalnız süreç RSS'i; SQLite sayfa önbelleği ve işletim sistemi
  önbelleği ayrı raporlanmaz. Örnekleme 2 sn aralıkla (tepe değer yaklaşık).
- **Ertelenmiş sayım komutu senkron** (`search_segments_total`), tıpkı
  `search_segments_page` gibi: arayüz sayıyı isterken pencere kısa süre
  kilitlenebilir (p95 ~1,5 sn). Sayfa artık bu süreyi beklemez; ama sayımına
  arka plana taşınması AppState'in (`Mutex<VaultState>`) yeniden düzenlenmesini
  gerektirir — yapılmadı.
- Bu tablo bir "geçti" belgesi değildir. Hedef tutmuyorsa güvenlik veya doğrulama
  adımı kaldırılmaz; kapasite iddiası ölçülen sınırla daraltılır.

## Seyrek eşleşme kuyruğu: belirteç/konum indeksi ölçüldü — **uygulanmadı** (2026-10-26)

Önceki tur aday penceresini getirdi; geriye "eşleşme seyrekse tarama yine bütün
adayları gezer" sınırı kalmıştı. Planlanan kaldıraç **kalıcı belirteç/konum
indeksiydi**. Önce ölçüldü, sonra karar verildi.

**Deney 1 — belirteç dizisi filtresi.** Her segmente belirteçlerin 16 bitlik
özetleri (tam + ilk-üç-karakter, `flexible`/`phrase` için) yazıldı; aday, iğne
belirteçlerini **bitişik** görmüyorsa metni hiç okumadan elendi, "olabilir"
dediğinde eşleşme metinle onaylandı (özet çakışması yanlış sonuç üretmez).

Korpus: **3.030 segment** (3.000 "iki belirteç var ama bitişik değil" tuzağı +
30 gerçek eşleşme), limit 200 (sayfa erken dolmuyor → bütün adaylar geziliyor),
21 koşu, release.

| Sorgu | dizisiz p50 | dizili p50 | kazanç | elenen aday |
|---|---|---|---|---|
| `itirazın reddine` (esnek) | 17,29 ms | 16,57 ms | **1,04×** | 3.000 / 3.030 |
| `"itirazın reddine"` (ifade) | 17,06 ms | 16,19 ms | **1,05×** | 3.000 / 3.030 |

Sonuçlar birebir aynı (aynı segment kimlikleri, aynı sayılar), yanlış pozitif 0.
Buna karşılık dizinin depolama maliyeti **segment başına 71,9 bayt**, metin
boyutunun **%54'ü**. Kazanç %5, maliyet %54 → **uygulanmadı** (kod geri alındı,
ölçüm dosyası `bench-token-index-ab-negative-2026-10-26.json`).

**Ders:** aday başına maliyetin neredeyse tamamı doğrulama değil, **satır
hattı**: dört `LEFT JOIN` (sayfa, koşu, sürüm, belge) + taşınan yük. 3.030
adayda 17 ms, aday başına ~5,6 µs; doğrulamanın kendisi bunun küçük kısmı.

## İki aşamalı tarama — uygulandı (2026-10-26)

Aynı ölçümün işaret ettiği düzeltme: aday akışı **yalnız** `text_segments` + FTS
üzerinden yürür (`SELECT s.id, s.page_id, s.raw_text, s.normalized_text,
bm25(...)`), belge/sayfa üst verisi **yalnız dönen sonuçlar için** tek `IN`
sorgusuyla okunur. Filtre varsa JOIN'ler `WHERE` için zaten gerekli olduğundan o
durumda eski `FROM` kullanılır. Aynı korpus, aynı sorgular, arka arkaya iki
derleme (önce/sonra), 21 koşu, release:

| Sorgu | profil | önce (tek sorgu) | sonra (iki aşama) | kazanç |
|---|---|---|---|---|
| `itirazın reddine` | tam tarama (limit 200) | 18,03 ms | **6,88 ms** | **2,62×** |
| `itirazın reddine` | ilk sayfa (limit 20) | 16,95 ms | **5,91 ms** | **2,87×** |
| `"itirazın reddine"` | tam tarama (limit 200) | 18,14 ms | **6,58 ms** | **2,76×** |
| `"itirazın reddine"` | ilk sayfa (limit 20) | 16,96 ms | **5,55 ms** | **3,06×** |

A/B'de sonuç kimlikleri ve sayıları **assert** edilir (birebir aynı); ham
raporlar `bench-scan-before-2026-10-26.json` ve `bench-scan-2026-10-26.json`.

**Yeni ölçülen sınır:** ilk sayfa yalnız 21 aday doğrulanırken de ~5,5–5,9 ms
sürüyor. Yani kalan maliyet doğrulama değil, **FTS'in bütün eşleşmeler için
`bm25` hesaplayıp sıralaması**. Bir sonraki kaldıraç burada: sıralamayı
aday sayısıyla sınırlamak (ör. iki geçişli `rowid` ön seçimi) — bu turda
yapılmadı.

**Yan bulgu (kusur, düzeltildi):** `0002_fts`'teki `text_segments_au`
tetikleyicisi sütun listesi olmadan **her** UPDATE'te FTS'e `delete`+`insert`
yazıyordu. Aynı satırda ikinci bir tetikleyici de yazınca FTS indeksi bozuluyor
ve arama `SQLITE_CORRUPT_VTAB` ("database disk image is malformed") veriyordu;
`anchors::locate_anchor_returns_page_and_intact_quote` testi bunu yakaladı.
Düzeltme **migration 0019**: tetikleyici `AFTER UPDATE OF raw_text,
normalized_text` olarak yeniden kuruldu (FTS içeriği yalnız bu iki sütuna bağlı).

## Aday daraltma (`NEAR`) — uygulandı (2026-10-26)

Önceki tur "kalan maliyet FTS'in bütün eşleşmelere `bm25` hesaplayıp sıralaması"
diye **çıkarmıştı**; bu tur bileşenlere ayrıldı. Koşucu
`crates/storage/tests/search_cost_bench.rs` (3.030 segment = 3.000 tuzak + 30
eşleşme, 41 koşu, release, Apple M2):

| Varyant | p50 | min |
|---|---|---|
| yalnız FTS sayımı (`count(*) … MATCH`) | 0,63 ms | 0,61 ms |
| `ORDER BY bm25` + yalnız `s.id` | 4,68 ms | 4,50 ms |
| `ORDER BY bm25` + `raw_text`/`normalized_text` (üretim şekli) | 5,70 ms | 5,66 ms |
| aynı sorgu **sıralamasız** | 1.472,9 ms | 1.430,1 ms |
| `ORDER BY` + `LIMIT 200` | 3,44 ms | 3,41 ms |

Okuma: FTS numaralandırması 0,63 ms, **sıralama ≈ 4,05 ms**, metin yükü ≈ 1,02 ms.
`LIMIT` sıralamayı kısaltmıyor (3,44 ms) — sıralama tüm eşleşmeler üzerinde
kurulur. Sıralamasız sorgunun 1,5 **saniye** olması ayrı ve önemli bir gerçek:
`ORDER BY` kalkınca SQLite planı FTS'ten sürmüyor, `text_segments` üzerinde
tam taramaya düşüyor (SQLCipher her satırı çözüyor). Yani sıralama ifadesi
planın da taşıyıcısı; kaldırılamaz.

Asıl kaldıraç **aday sayısıydı**: doğrulama zaten bitişik belirteç istiyor
(`seq_matches_spans`), ama FTS ifadesi `"a" AND "b"` idi → iki belirteci uzak
taşıyan 3.000 tuzak da aday oluyor, `bm25` onlar için de hesaplanıp sıralanıyordu.
İfade bitişikliğe çevrildi:

| İfade | aday | p50 (üretim şekli) |
|---|---|---|
| geniş (`"a" AND "b"`) — eski | 3.030 | 5,70 ms |
| tam bitişik ifade (`"a b" *`) — yalnız ölçüm | 30 | 1,28 ms |
| **`NEAR("a" "b"*, 2)` — uygulanan** | **30** | **1,32 ms** |

**Uçtan uca** (`apiFirstPage`, üretim API'si, limit 20): **5,95 → 1,55 ms
(3,84×)**; doğrulanan aday 21'de sabit (limit+1, erken kesme çalışıyor).
SQL sorgusunun kendisi 5,70 → 1,32 ms (**4,3×**). Daraltma **sonuç kümesini
değiştirmez**: `bm25` skoru satır bazında aynıdır, sıralama aynı kalır; aday
olmayan satırlar doğrulamadan da geçemezdi.

**Neden tam ifade değil `NEAR`:** FTS `unicode61` bazı sembolleri (`₺`, `€`, `©`,
`°`, `№`) **ayırıcı saymaz**, Rust `is_alphanumeric` sayar. Tam ifade, `itirazın ₺
reddine` gibi bir metinde (tek sembol araya girmiş) doğrulamanın kabul ettiği
eşleşmeyi kaçırırdı. `NEAR(…, belirteç sayısı)` bir belirteçlik pay bırakır ve
ölçülen maliyeti aynıdır (1,28 → 1,32 ms).

**Dürüst sınırlar (ölçüldü):**
- Pay **bir** belirteç. İki sembol belirteci yan yana gelirse (`itirazın ₺ €
  reddine`) daraltma o eşleşmeyi kaçırır. Test bunu görünür tutuyor
  (`narrowing_keeps_symbol_separated_words`); sınıf 1.506 kod noktası (Mn/So/Po/Cn
  ağırlıklı) — pratikte kelime ayırıcı boşluk olduğu için rastlanmıyor.
- Daraltma **FTS** düzeyinde; `search_total` da aynı ifadeyi kullanır (sayfa ve
  sayı aynı kümeden gelir). Tam eşleşme kuralı değişmedi.
- Ölçüm seyrek profilde: **daraltmanın kazancı aday fazlalığıyla doğru orantılı**.
  Her belirteci gerçekten bitişik taşıyan yoğun korpusta aday sayısı zaten
  eşleşme sayısına yakındır → kazanç küçülür, maliyet artmaz.
- 100k profilde yeniden ölçülmedi (disk); etki aday sayısına bağlı olduğu için
  yöne bakılmaksızın negatif değil.

**Yan bulgu (kusur #35, düzeltildi):** `phrase` kipinin adayları `raw_text`
indeksinden geliyordu; FTS `I`/`ı` ayrımını Unicode katlamasıyla yapıyor,
doğrulama ise Türkçe küçük harfle (`I`→`ı`). Sonuç: **"ıslah" sorgusu "ISLAH"
yazılmış metni bulamıyordu** (metin doğrulamadan geçebilecekken aday olmuyordu).
Aday üretimi `normalized_text`'e taşındı. Fark testi
(`narrowing_matches_full_verification_set`) beklenen kümeyi daraltmadan bağımsız
olarak doğrulayıcıdan hesaplayıp sayfayla karşılaştırır; bu kusuru o yakaladı.

Ham raporlar: `bench-cost-before-2026-10-26.json` (daraltma öncesi, 21 koşu) ve
`bench-cost-2026-10-26.json` (41 koşu, aynı korpus, aynı makine).

## `exact` kipinin aday tabanı — kusur #36 (2026-10-26)

`exact` kipi (arayüzde **Tam**) `phrase` ile aynı sınıfı taşıyordu: adaylar
`raw_text` FTS indeksinden geliyordu, doğrulama ise Türkçe küçük harfle
karşılaştırıyor. Önce **test kırmızı yazıldı**:
`exact_finds_dotless_query_in_uppercase_text` "ıslah" sorgusu için 2 sonuç
beklerken **1** döndü — `ISLAH` yazılmış metin hiç aday olmuyordu.

İkinci, daha sinsi fark: `exact` doğrulaması alt dizge (`contains`), aday
ifadesi ise FTS **öneki** (`"…"*`) idi. Kelime ortasından başlayan parça
doğrulamada geçiyor, aday üretiminde hiç görünmüyordu — daraltmanın sessizce
gizleyebileceği bir sınıf. Kural artık açık: **`candidates ⊇ verified`**.
`exact` doğrulaması belirteç başına çapalanır (`anchored_contains`); sonu
belirteç ortasında bitebilir ("itiraz" → "itirazın"), başı belirteç başında
olmak zorunda. Aday üretimi `normalized_text`e taşındı (sadeleştirme `ı`/`i`
ayrımını kaldırdığı için aday kümesi **üst küme** olur; kesin kararı doğrulama
verir).

**Ölçüm (aynı korpus: 3.030 segment = 3.000 tuzak + 30 eşleşme, 21 koşu,
release, `search_cost_bench`):**

| Ölçüm | p50 | Aday |
|---|---|---|
| `exact` ilk sayfa (`apiFirstPageExact`, limit 20) | **1,564 ms** | 21 (limit+1) |
| `auto`→`flexible` ilk sayfa (aynı sorgu) | 1,560 ms | 21 |
| FTS ifade tabanı (`ftsCountNear`, 3.030 satır) | 0,741 ms | 30 |
| daraltılmış SQL (`nearOrderedText`) | 1,343 ms | 30 |

`exact` artık `flexible` ile aynı sınıfta (fark ölçüm gürültüsünde) ve 3.000
tuzağın hiçbiri aday değil — **düzeltme maliyet getirmedi**. Ham rapor:
`bench-cost-exact-2026-10-26.json`; fark denetimi
(`narrowing_matches_full_verification_set`) artık 4 `exact` sorgusunu da
kapsıyor.

## Sıradaki kaldıraçlar (ölçümle desteklenen)

1. ~~**Toplam sayıyı geciktir**~~ **uygulandı** (2026-10-26): `search_total` +
   `search_segments_total` + arayüzde "sayılıyor…" satırı; sayı eşitliği her
   ölçüm örneğinde assert ediliyor.
2. ~~**Aday penceresi + tembel doğrulama**~~ **uygulandı** (2026-10-26): sayfa
   adayları `bm25` sırasında akıtıp yalnız ihtiyaç kadar doğruluyor; 2.000
   segmentte ilk sayfa p50 **44,7 → 19,6 ms** (2,28×), doğrulanan aday ort.
   136. 100k profilde kuyruk yeniden ölçülmedi (disk); seyrek eşleşen sorguda
   tarama yine tüm adayları gezer.
3. ~~**Kalıcı belirteç/konum indeksi**~~ **ölçüldü, uygulanmadı** (2026-10-26):
   3.030 adaylı seyrek profilde kazanç yalnız **1,04–1,05×**, depolama maliyeti
   metnin **%54'ü** → eşik altı. Ölçüm: yukarıdaki "belirteç/konum indeksi"
   bölümü ve `bench-token-index-ab-negative-2026-10-26.json`.
4. ~~**`bm25` sıralama maliyeti**~~ **uygulandı** (2026-10-26): maliyet bileşenlere
   ayrıldı (`search_cost_bench`): FTS numaralandırması 0,63 ms, sıralama ≈4,05 ms,
   metin yükü ≈1,02 ms. Sıralama kaldırılamaz (sıralamasız plan 1,5 sn'ye düşüyor),
   bu yüzden **aday sayısı** daraltıldı: ifade bitişikliğe çevrildi
   (`NEAR(…, belirteç sayısı)`), aday 3.030 → 30, uçtan uca ilk sayfa
   **5,95 → 1,55 ms (3,84×)**. Ayrıntı: yukarıdaki "Aday daraltma" bölümü.
5. ~~**Kalan taban**~~ **kapatıldı (2026-10-26, gerekçeli):** seyrek profilde
   daraltmadan sonra 1,34 ms'nin **0,74 ms**'si FTS'in ifadeyi 3.030 satırda
   değerlendirmesidir (`ftsCountNear`); gerisi satır hattı + doğrulama.
   **Belge/sayfa düzeyinde ön eleme bu tabanı kısaltmaz:** FTS `MATCH` indeksi
   ifadenin kendisiyle tarar, sayfa/belge koşulu yalnız `WHERE`'e eklenir —
   planlayıcı indeksi yine gezer. Daha kaba bir özet indeks ise
   "belirteç/konum indeksi" sınıfıdır ve o **ölçülüp reddedildi** (kazanç
   1,04×, depolama +%54). Taban segment sayısıyla doğrusal: 100k segmentte
   ~24 ms (3.030 → 0,74 ms eğrisinden **türetildi, ölçülmedi**), B-01 p95
   bütçesinin (2 sn) yanında küçük. → **Ek indeks uygulanmadı**; kalan iş
   100k profilde kapasite testi (donanım/disk).
6. ~~**İki aşamalı tarama**~~ **uygulandı** (2026-10-26): aday akışından JOIN'ler
   çıkarıldı, üst veri yalnız dönen sonuçlar için okunuyor → aynı korpusta
   **2,6–3,1×** (ölçüm yukarıda). Bu sayı daraltma **öncesi** ölçümdür; daraltma
   sonrası aynı profil 1,32 ms'dir.

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

OCR gecikmesi (aşağıdaki bölümün sayıları; yol **depo köküne** göre çözülür):

```bash
DIM_WORKER_PYTHON="$PWD/workers/document/.venv/bin/python" \
  DIM_BENCH_OCR_RUNS=10 DIM_BENCH_OCR_SINGLE_RUNS=5 \
  DIM_BENCH_OUT=docs/releases/F15/bench-ocr-2026-10-29.json \
  cargo test -p dim-ingest --test bench_ocr -- --ignored --nocapture
```

**Kanıt dosyası:** `docs/releases/F15/bench-b01-2026-10-25.json` — bu belgedeki
koşu 2'nin ham raporu (200 sorgu, sorgu bazında süreler, donanım kimliği,
`queryListSha256`). Koşu 1 aynı komutun tekrarıdır; p50/p95/max değerleri yukarıdaki
tabloda. Rapor dosyası makine tarafından üretilir, elle düzenlenmez.

## İş kuyruğu gecikmesi (F15-T05 "iş kuyruğu" maddesi)

Kuyruk 2026-10-26'ya kadar **ölçülmemişti** çünkü yürütücüsü yoktu: `jobs`
tablosuna yalnız yazılıyordu (her içe aktarma `parse_document` işi açıyor,
hiçbir katman sahiplenmiyordu). F03-T07/T08 ile sahiplenme (lease), tamamlama,
hata/tekrar, iptal ve açılışta kurtarma eklendi; ölçüm bu çekirdeğin üzerinde
yapıldı.

**Koşu (release, Apple M2, macOS 26.6.2, tek süreç):** 400 iş sıraya konur,
seri yürütücü her işi sahiplenir → 2 ms sentetik iş → bitirir; ayrıca 40 iş
4'lük parti sahiplenmeyle koşar. Ham rapor:
`docs/releases/F15/bench-queue-2026-10-26.json`.

| Ölçüm | p50 | p95 | max |
|---|---|---|---|
| Sıraya koyma (`enqueue`, 400 iş) | **0,219 ms** | 0,501 ms | 22,6 ms |
| Sahiplenme (`claim`, 400 iş) | **1,016 ms** | 4,588 ms | 10,2 ms |
| Sahiplen→bitir döngüsü (2 ms sentetik iş dahil) | **11,365 ms** | 16,654 ms | 22,8 ms |
| Kuyruk beklemesi — seri yürütücü, 400 iş dolu kuyruk | **2.238,9 ms** | 4.296,3 ms | 4.493,0 ms |
| Kuyruk beklemesi — 4'lük parti, 40 iş | **32,0 ms** | 37,5 ms | 37,9 ms |

Okuma: mekanizmanın kendi maliyeti iş başına **≈ 9,4 ms** (döngü 11,4 ms − 2 ms
sentetik iş; sahiplenme 1,0 ms + bitirme ≈ 8,4 ms). Bunun baskın kısmı
SQLCipher'ın WAL commit'i; tarama değil. Kuyruk **dolu** olduğunda bekleme
doğrusal büyür (400 iş × 11,4 ms ≈ 4,5 sn); bu yüzden yayımlanan sayı
"kuyruk beklemesi = birikmiş iş × iş süresi" olarak okunmalıdır. Parti
sahiplenme (4'lük) bekleme süresini kısaltmaz — iş sayısı az olduğu için
33 ms'de kalır; kazanç, sahiplenme çağrısı sayısındadır.

**Plan kanıtı (test):** sahiplenme sorgusu `idx_jobs_claim` indeksini kullanır ve
sıralama için geçici B-tree kurmaz (`claim_query_uses_an_index_not_a_full_scan`).
İlk yazımda sorgu `state IN (...)` ile yazılmıştı; SQLite sıralamayı indeksten
karşılayamadığı için her sahiplenmede geçici B-tree kuruyordu — sorgu durum
başına tek eşitliğe bölündü ve indeks `(state, priority DESC, created_at,
next_run_at)` olarak düzeltildi. `EXPLAIN QUERY PLAN` testi bunu kalıcı tutar.

### §17.2 durum makinesi sonrası yeniden ölçüm (2026-10-26, migration 0021)

Durum kümesi şartnamenin tamamına çıkarıldı (`blocked`, `validating`,
`canceling`, `interrupted`, `partial`, `completed`, `canceled`) ve sahiplenme
yoluna iki iş eklendi: (1) bağımlılığı çözülmüş `blocked` işleri açan tarama,
(2) `job_attempts` numarası için `MAX(attempt_no)` alt sorgusu. Aynı koşucu,
aynı profil (400 iş + 40'lık parti), release, Apple M2:

| Ölçüm | p50 | p95 | max | 0020'e göre |
|---|---|---|---|---|
| Sıraya koyma (`enqueue`) | **0,184 ms** | 0,265 ms | 5,1 ms | −0,035 ms (gürültü) |
| Sahiplenme (`claim`) | **1,929 ms** | 4,084 ms | 8,2 ms | **+0,91 ms** |
| Sahiplen→bitir döngüsü | **11,776 ms** | 16,644 ms | 101,9 ms | +0,41 ms |
| Kuyruk beklemesi — seri, 400 iş | **2.375,7 ms** | 4.776,3 ms | 4.995,4 ms | +137 ms |
| Kuyruk beklemesi — 4'lük parti, 40 iş | **19,987 ms** | 21,657 ms | 21,7 ms | −12 ms |

Ham rapor: `docs/releases/F15/bench-queue-0021-2026-10-26.json`.

Okuma: sahiplenme **iki katına yakın** (1,0 → 1,9 ms) çıktı; eklenen tarama boş
kuyrukta bile bir sorgu + hazırlık maliyeti getiriyor. Mutlak değer hâlâ
küçük (400 işte toplam ≈ 0,36 s) ve kuyruk beklemesinin baskın kısmı hâlâ
seri yürütme (400 × 11,8 ms). Bu maliyet **bilinçli kabul edildi**: engelli iş
`queued` gibi görünmüyor ve bağımlılığı bitince kendiliğinden açılıyor.
Kuyruk ölçümü hâlâ **çekirdek** (Python worker/IPC/arayüz yok); 100k profilde
yeniden koşulmadı.

**Dürüst sınırlar:**
- Ölçüm **çekirdek**: Python worker, IPC ve arayüz yok. Sentetik iş sabit 2 ms;
  gerçek ayrıştırma süresi belgeye göre değişir (B-01'de 1.000 belge 44–47 sn
  yükleme, yani belge başına ≈ 45 ms).
- Tek makine, sıcak önbellek, tek süreç. Kasa başına tek yazıcı kuralı
  gereği iki worker'ın **gerçek paralelliği** ölçülmedi; parti ölçümü yalnız
  sahiplenme çağrısı sayısını azaltır.
- Kuyruk beklemesi, iş süresi varsayımına bağlıdır; 2 ms'lik sentetik iş
  gerçek PDF ayrıştırmasını temsil etmez. Gerçek uçtan uca süre için
  B-01 yükleme ölçümü ile bu tablo birlikte okunmalıdır.
- 100.000 belge profilinde kuyruk yeniden ölçülmedi (disk).

### Canlı sayaç yazımı (2026-10-27, migration 0022)

Ayrıştırma sırasında kuyruk satırındaki ilerleme (`jobs.progress_stage/done/total`)
her karede değil, `ProgressGate` kuralıyla yazılır: **ilk kare, aşama değişimi,
sayacın 0'a dönüşü ve toplamı bulma her zaman**, aradakiler yalnız 300 ms dolunca.
Aynı koşucu (`queue_latency`, ayrı kasa), release, Apple M2:

| Ölçüm | p50 | p95 | max | n |
|---|---|---|---|---|
| Tek `update_job_progress` (UPDATE + WAL commit) | **0,255 ms** | 0,271 ms | 0,476 ms | 120 |
| 120 kare → yazım sayısı, hızlı belge (50 µs/sayfa) | **2** | — | — | kural sayımı |
| 120 kare → yazım sayısı, yavaş/taranmış (5 ms/sayfa) | **3** | — | — | kural sayımı |

Ham rapor: `docs/releases/F15/bench-queue-0022-2026-10-27.json` (`progress` alanı).

**Okuma:** yazım başına ≈ 0,26 ms; 120 karede **sayfa başına commit yapılsaydı**
≈ 31 ms ek yük olurdu (worker'ın 120 sayfalık çıktısı ≈ 20 ms — yani ayrıştırmayı
1,5× yavaşlatırdı). Kapı bunu 2–3 yazıma (≈ 0,8 ms) indiriyor. Bedeli açık:
**kısa belgede sayaç kabalaşır** (0 → %100 atlar); uzun/taranmış belgede (saniyeler
süren aşama) 300 ms'de bir yazım görünür.

Aynı koşuda sahiplenme p50 **2,107 ms** (0021: 1,929 ms) — sahiplenme `UPDATE`'i
dört sayaç sütununu da sıfırladığı için +0,18 ms; mutlak değer hâlâ küçük ve
bilinçli kabul (yanlış deneme sayacı kalıntısı göstermemek için).

**Dürüst sınırlar:** ölçüm çekirdek (Python worker/IPC/arayüz yok) ve tek makine;
"yazım sayısı" alanları **ölçüm değil**, kapı kuralının aynı kare dizisinde
sayımıdır. Gerçek belgede kare aralıkları worker hızına bağlıdır; ağ biriminde
`fsync` davranışı denenmedi. 100k profilde yeniden koşulmadı.

### Belge worker paralelliği (2026-10-27, §17.3)

Şartname §17.3 "aynı anda en fazla iki belge worker işi; büyük dosyada bir iş"
diyor. Bu ölçüm **gerçek kasa + gerçek Python worker** ile yapıldı (debug
derleme, Apple M2); koşucu Tauri testi
`commands::tests::two_parses_run_concurrently_on_a_real_vault`
(`#[ignore]`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
--lib two_parses -- --ignored --nocapture`).

İki belge: `progress-pages.pdf` (4 sayfa) ve `redaction-hidden.pdf` (1 sayfa).
Isınma koşusundan sonra:

| Koşu | 1. ölçüm | 2. ölçüm | Not |
|---|---|---|---|
| Seri (2 belge sırayla) | 1,235 s | 1,233 s | belge başına ≈ 0,62 s |
| Paralel (2 worker, havuz tavanı 2) | **0,721 s** | **0,699 s** | **1,71× / 1,76×** |
| İki ayrıştırmanın çakışması | 0,720 s | 0,698 s | aralıklar neredeyse tamamen kesişiyor |

**Okuma:** kazanç gerçek ama 2× değil. Baskın maliyet **süreç başlatma +
PyMuPDF importu** (≈ 0,4 s); 4 sayfalık belgede çıkarım ≈ 20 ms. Yani paralellik
bu profilde süreç sayısıyla sınırlı; büyük belgede (çıkarımın baskın olduğu
yerde) kazanç **türetilmemeli, ölçülmelidir** — ölçülmedi.

**Ölçüm tuzağı (kayıtlı):** ısınma olmadan, paralel koşu önce yapıldığında
1,816 s / seri 1,389 s çıktı — "paralel daha yavaş" yanıltıcısı. Sebep soğuk
sayfa önbelleğiydi (ilk iki süreç PyMuPDF'i diskten okur). A/B sırası ısınma
koşusu olmadan bu ölçümde yanıltıcıdır.

**Sınırlar:** debug derleme; küçük belgeler; tek makine; iş parçacığı/veritabanı
yazımı ölçümün parçası değil (yazımlar kasa kilidi altında seri — kural, ölçüm
değil). Windows/Linux ve release derleme ölçülmedi.

## OCR gecikmesi (F15-T05 "OCR" maddesi) — 2026-10-29

**Neden bu bölüm var:** yukarıdaki dürüst sınırlar listesi "OCR ve iş kuyruğu
gecikmesi ölçülmedi (worker devrede değil)" diyordu. İş kuyruğu 2026-10-26'da
ölçüldü; OCR maddesi **açık kaldı**. OCR, ürünün en pahalı yerel işi (sayfa
başına 300 dpi rasterleştirme + Tesseract) ve tek sayfalık fikstür toplam
maliyeti göstermiyordu.

**Koşucu:** `crates/ingest/tests/bench_ocr.rs` (elle, `#[ignore]`).
**Fikstür:** `fixtures/synthetic/scanned-pages.pdf` — 6 sayfa, **244.129 bayt**,
sha256 `fe71877b9579…cb3b2`, **metin katmanı yok**; her sayfa farklı metinle
(`MADDE 1` … `MADDE 6`, tutar 1000→2250 TL) 300 dpi gri raster olarak gömülü.
Farklı metin bilinçli: koşucu **her sayfanın kendi başlığını** okuduğunu
doğrular (`assert`), yoksa "hızlı ama boş" rapor üretilmez.

**Yöntem**

- Her koşu **yeni worker süreci** (`python -m dim_worker.worker`) → süreç
  başlatma + PyMuPDF importu ölçüme **dahil** (kullanıcının gördüğü süre).
- Gerçek OCR bütçesi (`WorkerKind::Ocr`: 600 s CPU, 2048 MiB, 48 MiB çıktı,
  900 s duvar saati); bütçe ihlali koşuyu **düşürür**.
- 10 koşu çok sayfalı belge, 5 koşu tek sayfa fikstürü
  (`scanned-page.pdf`), 1 koşu **OCR kapalı** taban çizgisi.
- Taban çizgisi kapısı: OCR kapalıyken aynı taramadan **0 karakter** gelmeli —
  yani "OCR pahalı" iddiası, OCR'ın gerçekten çalıştığı ölçülmüş halde söylenir.

**Sonuç (Apple M2, macOS 26.6.2, Tesseract 5.5.3 `tur`, dil paketi kurulu)**

| Ölçüm | p50 | p95 | max | min | Not |
|---|---|---|---|---|---|
| 6 sayfalık tarama (n=10) | **6.398 ms** | **8.140 ms** | 8.290 ms | 5.705 ms | süreç başlatma dahil |
| sayfa başına (p50/6) | **1.066 ms** | — | — | — | türetilmiş |
| tek sayfa (n=5) | **1.054 ms** | 1.062 ms | 1.063 ms | 1.048 ms | ayrı ölçüm |
| OCR kapalı taban (6 sayfa) | 425 ms | — | — | — | metin **0 karakter** |

- **Bellek:** çok sayfalı koşuda tepe **224,3 MiB** (bütçe 2048 MiB → %11),
  tek sayfada 171,0 MiB. Fark, PyMuPDF/Tesseract'ın sayfa başına ek yükü.
- **Yanıt:** 3.495 bayt/koşu; sayfa başına OCR güven notu (6 uyarı), güven
  **83–94/100**.
- **Ayrıştırma:** marjinal sayfa maliyeti ≈ (6.398 − 425) / 6 ≈ **995 ms**;
  sabit maliyet ≈ 425 ms (süreç + import). 100 sayfalık bir tarama, tek
  worker'da sayfa sıralı koştuğu için ≈ **0,43 + 100 × 1,0 ≈ 100 s** olur
  (türetilmiş, ölçülmedi — aşağıdaki sınırlar).

**Dürüst sınırlar**

- Ölçülen katman **çekirdek worker**: Tauri IPC, kasa yazımı ve arayüz
  gösterimi yok → raporlanan süre **alt sınırdır**.
- Fikstür metni **ASCII** ve her sayfa aynı düzende; gerçek taramalarda
  eğim, gürültü, gölge ve yoğun metin OCR'ı **hem yavaşlatır hem kaliteyi
  düşürür**. Kalite (dil modeli) bu koşucuda **ölçülmez**.
- Rust koşucusu **debug** derleme; ölçülen iş Python/Tesseract sürecinde
  olduğu için Rust profilinin etkisi ihmal edilebilir — yine de release
  karşılaştırması yapılmadı.
- Tek makine, tek Tesseract sürümü, 300 dpi sabit. DPI artışı maliyeti
  yaklaşık doğrusal büyütür; **ölçülmedi**.
- Belleğin sayfa sayısıyla büyümesi ölçülmedi (6 sayfa → 224 MiB). 100
  sayfalık belgede 2048 MiB tavanının payı bilinmiyor.
- Sayfalar sıralı koşar; iki belgenin paralel OCR'ında (havuz tavanı 2)
  gecikme davranışı ölçülmedi.
- Ham rapor: `docs/releases/F15/bench-ocr-2026-10-29.json` (makine üretir,
  elle düzenlenmez).

## OCR **dil modeli** kalitesi — 2026-10-29 (Tesseract 5.5.3)

Önceki bölüm OCR'ın **gecikmesini** ölçüyordu; fikstür ASCII olduğu için dil
modelinin **kaliteye** etkisini göremezdi (İngilizce modeli ASCII metni zaten
doğru okur). Ürün Türkçe hukuk belgeleri için yapıldığı hâlde varsayılan dil
yolun birinde `eng` kalmıştı; bu bölüm o kararın **ölçülmüş** gerekçesidir.

- **Koşucu:** `scripts/ocr_lang_bench.py` (`--self-test` 15 kontrol; CI'da
  saf yardımcılar sınanır, ölçüm elle koşar) — **gerçek worker süreci**, her
  dil için **yeni süreç** (başlatma dahil), gerçek OCR bütçesi.
- **Fikstür:** `fixtures/synthetic/turkish-scan.pdf` — 2 sayfa, **146.289
  bayt**, **metin katmanı yok**, 300 dpi gri raster, ş ğ ı İ ç ö ü yoğun
  hukuki cümleler (79 Türkçe harf). Üretici `fixtures/synthetic/generate.py`
  (`build_turkish_scan_pdf`); Türkçe glifli TTF yoksa **açık hata** verir,
  ASCII'ye düşmez. Gövde metin kutusuna yazılır: uzun cümle sayfa kenarında
  kırpılırsa karşılaştırma haksız olur (ilk üretimde **ölçüldü**: cümleler
  "keşide e" gibi ortadan kesiliyordu; düzeltildi).
- **Ölçüm (Apple M2, macOS 26.6.2, Tesseract 5.5.3, n=5/dil):**

| Dil | Karakter doğruluğu (medyan) | Türkçe harf korundu | Güven | Süre (medyan) |
|---|---|---|---|---|
| `eng` | **%82,78** (5/5 koşuda aynı) | **0 / 79** | 78,0 | **2.462 ms** |
| `tur` | **%100** | **79 / 79** | 94,0 | **2.326 ms** |
| `tur+eng` | **%100** | **79 / 79** | 94,5 | **2.509 ms** |

- **Okunan metin farkı (gerçek çıktı, `eng`):**
  `Supheli, israrla One surdugu iddiay! ispatlayamadi.` ·
  `Sirketin odeme guglugu cektigi ddnemde cek keside edilmisti.` ·
  `Uctincii kisi, 6demenin islak imzayla yapildigint soyledi.`
  Aynı sayfa `tur` ile birebir: `Şüpheli, ısrarla öne sürdüğü iddiayı
  ıspatlayamadı.` Yani hata yalnız diyakritik değil: `İ→I`, `ı→i`, `ş→s`,
  `ğ→g`, ayrıca `ü→u`/`ö→o` karışmaları ve noktalama kaybı.
- **Karar:** varsayılan **`tur+eng`** (tek kaynak:
  `dim_storage::queue::DEFAULT_OCR_LANG`; aynası `dim_worker.engines.
  DEFAULT_OCR_LANG` ve `apps/desktop/src/lib/ocr.ts`, üçü testle bağlı).
  Saf Türkçe metinde `tur` ile **aynı kalite**, `tur+eng` 2 sayfada
  **+183 ms (~%8)**; karışık (Türkçe + İngilizce terim) belgeler için
  birleşik model seçildi. **Karışık dil ölçülmedi** — bu bir tercih, ölçüm
  değil.

**Dürüst sınırlar**

- Tek makine, tek Tesseract (5.5.3), tek yazı tipi (Arial), 300 dpi, temiz
  raster: eğim/gürültü/çok sütun yok. Gerçek taramada her iki modelin de
  doğruluğu düşer; **farkın yönü** ölçüldü, büyüklüğü bu fikstüre özgüdür.
- Karakter doğruluğu **düzenleme mesafesi** ile hesaplanır (boşluk-duyarsız,
  noktalama duyarlı); kelime düzeyinde F1 veya hukuki anlam doğruluğu
  ölçülmedi.
- `tur`/`tur+eng` bu fikstürde **%100** çıktı; bu "her Türkçe taramada
  %100" demek değildir — tavan etkisi (fikstür kendi ürettiğimiz temiz
  raster). Ölçüm **hedef değil rapor**.
- **Dil paketi kurulu olmalı:** Tesseract ve `tur` verisi uygulamayla
  paketlenmiyor (F02). Eksikse OCR artık **`E_OCR_LANG_MISSING`** ile eksik
  kodun adını ve kurulu dilleri yazar (eskiden `E_OCR_FAILED` +
  "dil paketi eksik olabilir" **tahminiydi**).
- Ham rapor: `docs/releases/F15/bench-ocr-lang-2026-10-29.json`.
