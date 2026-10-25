# F01-T12 — Başarısız deneme defteri

Her başarısız deneme için **düzeltme maliyeti**, **kapsam etkisi** ve **yeniden
test kararı** yazılır. Kural: bu dosyaya yalnız gerçekten yaşanmış olaylar girer;
kanıtı (commit, test adı, CI koşusu, ölçüm) olmayan hiçbir satır yazılmaz.

Kapsam etkisi ölçeği: **kritik** = ürün iddiası çöker (kasa açılmaz, veri
sızabilir, dağıtım yapılamaz) · **işlevsel** = bir akış çalışmaz ya da yanlış
sonuç verir · **süreç** = ürün davranışı doğru, üretim/denetim izi etkilenir ·
**test** = yalnız test iddiası yanlış.

## Özet

| # | Deneme | Belirti / kanıt | Kök neden | Düzeltme maliyeti | Kapsam | Yeniden test kararı |
|---|---|---|---|---|---|---|
| 1 | Kurtarma anahtarını KDF'e doğrudan vermek | Kurtarma anahtarı kasayı hiç açmıyordu (`a32939f`) | base32 **metni** KDF'e girdi, ham 32 bayta çevrilmedi | 1 tur, 3 test | **kritik** — parolasını unutan kullanıcı kasayı açamazdı | `cargo test -p dim-vault` (kurtarma roundtrip + yanlış anahtar) |
| 2 | `PackageView.files` sabit `3` | Manifest 2 dosya listelerken arayüz 3 diyordu (F11-T06) | Alan, gerçek listeye bağlanmamış | 1 satır (`files.len()`) | **işlevsel** — pakette olmayan dosya var sanılır | Tauri komut testi + `KAPSAM.md` ile 3 dosya |
| 3 | `upgrade_tests` "gelecek sürüm" olarak sabit `14` | v14 gerçek migrasyon olunca `E_DB_MIGRATION_CHECKSUM` | Sabit sayı, şema sürümünden bağımsız yazılmış | 1 satır (`CURRENT_SCHEMA_VERSION + 1`) | **test** — yanlış kırmızı; güvenlik kapısı gevşetilmedi | v14 sütununun yazılmadığı ayrıca doğrulanır |
| 4 | `create_note` içinde kendi işlemini açmak | `commit_review_decision` → "cannot start a transaction within a transaction" | İç içe `unchecked_transaction` | `is_autocommit` deseni (tek yardımcı) | **işlevsel** — karar + not birlikte yazılamıyordu (atomiklik çöker) | Not modülü 16 test + inceleme kararı yolu |
| 5 | Argüman kapısında yalnız `allowed_args` denetimi | CLI sınama düğmesi `E_CLI_ARGUMENT` veriyordu (F10 arayüz turu) | Kapı, sicilin kendi `--version` argümanlarını (`version_args`) tanımıyordu | İkinci listeyi kapıya eklemek | **işlevsel** — düğme tamamen çalışmıyordu | `declared_capability_flags_pass_the_argument_gate` + gerçek `--version` yoklaması |
| 6 | `count(*) OVER ()` ile tek geçişte toplam | Toplam yanlış/boş; `bm25` alt sorgu düzleştirmesinde bağlam kayboldu | SQLite alt sorgu düzleştirmesi | 1 ölçüm turu (reddedildi) | **işlevsel** olurdu (yanlış sayı) → kullanılmıyor | `search_total_from_single_pass_matches_verified_hits` |
| 7 | `LIMIT -1` materyalizasyonuyla sayım | p50 **2.620 ms** / p95 3.477 ms — mevcut 1.554/2.100'den yavaş | Tüm sonuç kümesinin materyalizasyonu | 1 ölçüm turu (reddedildi) | **işlevsel** (performans hedefi) → kullanılmıyor | `docs/releases/F15/BENCHMARK.md` tabloları |
| 8 | Süre tabanlı süreç testi (400 ms / 1,5 sn) | Paralel koşuda "önce çıktı, sonra öldürme" sırası kayboldu (flaky) | Zamanlama yarışı | Çocuk 30 sn uyku, sınır 5 sn | **test** — yalancı kırmızı | `timeout_kills_child…` 3 kez üst üste yeşil |
| 9 | Kum havuzu dizinine betik yazıp aynı dizini iş dizini vermek | `E_CLI_WORKDIR_NOT_EMPTY` | Kural doğru çalışıyor: iş dizini boş olmalı | Testte `iso` alt dizini | **test** — kurgu hatası | `crates/cli-adapters` sandbox testleri |
| 10 | WiX MSI derlemesi varsayılan kod sayfasıyla | `LGHT0311` (kod sayfası 1252); Windows kurulumu üretilemiyordu | Türkçe karakterler varsayılan kod sayfasına sığmıyor | `bundle.windows.wix.language = "tr-TR"` | **kritik** — Windows dağıtımı bloke | Windows CI `40058501597`: x64 MSI 6.537.216 bayt |
| 11 | Windows **ARM64** MSI | `openssl-src` derlenmiyor (`C1083`/`U1077`) | Upstream: aarch64-pc-windows-msvc desteği yok | Bilinmiyor (upstream) | **kritik** — ARM64 paketi yok | İş `continue-on-error`; ARM64 **yeşil sayılmaz**, upstream bekliyor |
| 12 | `page.get_label()` ile sayfa etiketi | Etiket ağacı yokken **fiziksel numara** döndü | PyMuPDF davranışı | Önce `doc.get_page_labels()` varlık kontrolü | **işlevsel** — uydurma etiket (hukuki atıf hatası) | `/PageLabels` olan ve olmayan PDF testleri |
| 13 | Sayımı yalnız DB dosyasında yapmak (F01-T03 ilk hâli) | `-wal`/`-shm` ve FTS gölge tabloları hiç taranmıyordu | Test, kapanış sonrası tek dosyaya bakıyordu | 1 tur (6 test + worker testi) | **test** — kanıt eksikti, ürün doğruydu | `crates/storage/tests/plaintext.rs` + `crates/ingest/tests/worker_temp_plaintext.rs` |
| 14 | "FTS silme indeksi boşaltır" varsayımı (F01-T03) | Silme sonrası `text_segments_fts_data` **4** satır kaldı (beklenen 0) | FTS5 harici içerikli tabloda silme yalnız işaret yazar; birleştirme sonraya kalır | 10 dk (iddia düzeltildi) | **test** — yanlış iddia; ürün davranışı doğru | Kalıntı **4 → 2** (`optimize`) raporlanır; arama boşluğu ve bayt temizliği ayrı doğrulanır |
| 15 | `git add -A` ile commit | Başka ajanın kodu benim commit'ime karıştı (`3a3f352`, `77fcc76`) | Ortak çalışma alanında toplu ekleme | Her commit'te `git status` + `git add <path>` | **süreç** — atıf/denetim izi kaybı | Tek ajan kuralı; commit öncesi durum kontrolü |
| 16 | Commit sonrası ağır derlemeye devam etmek | Post-commit `disk-guard` `node_modules`'ı sildi → `Command tauri not found` | Disk eşiği (< 4 GB) temizliği | `pnpm install --frozen-lockfile` (~3 sn) | **süreç** — tur içi kesinti | Commit sonrası `df -h /`; eşik altındaysa önce temizle |
| 17 | Önceki turun yarım bıraktığı `DualReader.tsx` | Eksik import derlemeyi kırdı (F06-T10 turu) | Değişiklik yarım kalmış, `tsc` koşulmamış | Import + fixture alanı (`isCurrentRun`) | **işlevsel** — arayüz derlenmiyordu | `tsc --noEmit` + `vite build` + `reader.test.ts` |
| 18 | `review_items.state`'e `superseded` eklemek | CHECK kısıtı reddeder; tabloyu yeniden kurmak FK riski | Şema kısıtı ile API durumu karıştırıldı | `superseded_at` ile ifade, durum API'de hesaplanır | **işlevsel** (gösterim) | Karar geçmişi testleri |
| 19 | Test fixture'da sabit `storage_id` | İkinci çağrıda `UNIQUE constraint failed: blobs.storage_id` | Şema UNIQUE, fixture sabit | `Uuid::now_v7()` | **test** | Blob/itibar testleri |
| 20 | `WorkerError::FrameTooLarge` tanımlıydı ama **hiç üretilmiyordu** | 1 MiB üstü çerçeve "bağlantı koptu" (`E_WORKER_DISCONNECTED`) olarak raporlanıyordu (F01-T10 testleri yazılırken bulundu) | Okuyucu iş parçacığı hatayı `String` olarak taşıyor, çağıran taraf hepsini `Disconnected`'a çeviriyordu | ~10 satır + 4 yeni test | **işlevsel** — tanılama yanlış: süreç ölümü ile protokol ihlali aynı kodla görünüyor, yanlış müdahale (yeniden başlat) yaptırıyordu | `oversized_frame_is_reported_as_frame_error` (`E_WORKER_FRAME`), `malformed_frame_...` (`E_WORKER_RESPONSE`), süreç ölümü ve zaman aşımı testleri |
| 21 | Karartma çıktısını yalnız metin çıkarımıyla doğrulamak | Annotation taşıyan PDF'te karartma hiç çalışmıyordu (`AttributeError: 'NoneType' object has no attribute 'm_internal'`) | PyMuPDF 1.24.14 `delete_responses`, yanıtı olmayan her annotta `None` dönen `JM_find_annot_irt` sonucunu kullanıyor; `scrub` varsayılan `reset_responses=True` | `reset_responses=False` + yanıtları kendi taramamızla silmek | **kritik** — yapışkan notu olan her hukuk dosyası maskelenemiyordu | `test_annotation_without_marker_does_not_block_redaction`, `test_annotation_text_is_removed_and_harmless_note_kept` |
| 22 | Yapışkan not metnini `scrub`'ın temizlediğini varsaymak | `/Contents` ve `/T` (yazar) alanındaki işaretçi çıktıda **ham baytta** duruyordu; `remaining=[]`, arayüz "doğrulandı" diyordu | `scrub` yalnız yanıt (`/IRT`, popup) zincirini siler; annotation metni sayfa metnine girmediği için `search_for` de görmez | ~40 satır (`_strip_leaking_annotations`) + 2 test | **kritik** — karartıldı sanılan isim dosyada kalıyordu (hukuki sızıntı) | `test_annotation_text_is_removed_and_harmless_note_kept` + gerçek worker fikstürü (`annotations_removed=1`) |
| 23 | Sayfa dışı metni görünmez saymak | MediaBox dışındaki (x=900) metin çıktıda kalıyordu | `search_for`/`get_text` sayfa dışını döndürmez; `remove_hidden` kapsamıyor | ~30 satır (`_strip_offpage_text`, `get_texttrace` + `cropbox`) + 2 test | **kritik** — görünmeyen ama kopyalanabilen metin | `test_offpage_text_is_removed_and_visible_text_survives`, `test_rotated_page_visible_text_is_not_mistaken_for_offpage` |
| 24 | `scrub(javascript=True)`'nun gömülü eylemleri sildiğini varsaymak | Katalogdaki `/OpenAction <</S/JavaScript/JS(...)>>` işaretçisi çıktıda kalıyordu | `scrub` yalnız **ayrı nesne** olan `/S /JavaScript` xref'ini sıfırlar; satır içi sözlük hiçbir xref'te görünmez | ~25 satır (`_strip_document_scripts`) + 1 test | **işlevsel** — belgeyle giden gömülü betik/veri temizlenmiyordu | `test_inline_javascript_is_removed` + worker fikstürü (`scripts_removed=1`) |
| 25 | Kendi bayt tarayıcımızın yalnız düz UTF-8 araması | Tarayıcı, **sayfa metnini hiç görmüyordu**: PyMuPDF metni onaltılık dize (`<5a5a…>`) yazıyor; "temiz" sonucu kanıtsızdı (F01-T05 ölçümü) | PDF dize kodlamaları (kaçışlı, UTF-16BE, UTF-8 onaltılık) bilinmiyordu | `_term_variants` + 3 test | **kritik** — doğrulama kör olabilirdi; ürün doğru, kanıt yanlıştı | `test_compressed_stream_is_inflated_before_searching`, `test_black_box_over_text_is_not_redaction`, `test_scanner_detects_known_leak_not_blind` |
| 26 | pypdf kurulu değilken motor listesine "pypdf" yazmak | Doğrulama hiç koşmadığı hâlde arayüz "pypdf" diyordu | `_verify_with_pypdf` ImportError'da `[]` dönüyor, çağıran yine de listeye ekliyordu | Dönüş tipi `(found, ran)` + 1 test | **kritik** — yalan doğrulama iddiası (denetim izi) | `test_missing_pypdf_is_not_claimed_as_verifier` |
| 27 | Vekil süreç satırı yazdıktan hemen sonra ölüyor | CI'da `malformed_frame_is_reported_as_bad_response` kırmızı: `E_WORKER_RESPONSE` beklerken `Disconnected` (25 Eki, run `40149933704`) | `/bin/sh -c "printf …"` çıkıyor; okuyucu iş parçacığı EOF ile satırı **yarıştırıyor** (lokal geçti, CI'da kaybetti) | `; sleep 5` (çerçeve ve aşırı çerçeve testlerinde) | **test** — yalancı kırmızı; ürün davranışı doğru | 3 kez üst üste yeşil + CI |
| 28 | Paket doğrulaması dizinleri "fazla dosya" sayıyordu | `packaging::tests::verify_accepts_a_matching_tree` kırmızı: `extra=[\"_internal\"]` — eşleşen ağaç "kusurlu" görünüyordu | Dizin taraması `file_type()` ile dosya/bağ ayrımı yapmıyor, her girdiyi listeye koyuyordu | Yalnız dizin olmayan girdiler sayılır (1 test) | **işlevsel** — yanlış "kusurlu paket" hükmü | `verify_accepts_a_matching_tree` |
| 29 | Manifest dizin sembolik bağlarını hiç kaydetmiyordu | `packaged_worker_matches_its_manifest` kırmızı: `extra=[\"Python.framework/Resources\", \"…/Versions/Current\"]` — gerçek pakette 2 bağ manifestte yoktu | Üretici `os.walk` kullanıyordu; `os.walk` dizin bağlarını `dirnames`e koyar, `filenames`e değil | `scandir` tabanlı yürüyüş: bağlar `kind=symlink` olarak kaydedilir, ağaçlarına inilmez (2 test: dizin bağı + kaçış) | **kritik** — envanter eksikse "doğrulandı" iddiası eksik kanıta dayanır | `test_directory_symlinks_are_recorded_and_checked`, gerçek paket testi (136/136) |
| 30 | Çevrimdışı kanıtın kendi dedektörü yanlış okudu | `verify-worker-offline.py` ilk koşuda "sandbox ağı gerçekten kapattı ✗ — ağ AÇIKTI" dedi; profil doğruydu | Prob, worker'a verilen **kasıtlı bozuk** `PYTHONHOME`/`PYTHONPATH`'i devraldı, Python başlayamadı, sıfır dışı çıkış "ağ açık" sayıldı | Prob temiz ortamda koşar ve kendi işaretini yazar (`DENIED`/`OPEN`); başka sonuç "ölçülemedi" | **kritik** — yanlış tanı dedektörü yalancı yapar | gerçek koşu: `DENIED`, 7/7 kontrol yeşil |
| 32 | `pages_for_run` **indekssiz `text_segments.page_id`** ile tam tablo taraması | B-01 ölçümünde 50 örnekli "ilk sayfa" döngüsü **12+ dakikada bitmedi** (koşu durduruldu). A/B: sayfa başına **914,9 ms** (indekssiz) → **0,098 ms** (indeksli), ~9.336× | Üç korelasyonlu alt sorgu her sayfa için `page_id` ile arıyordu; bu sütunda indeks yoktu → her satır SQLCipher ile çözülüp HMAC doğrulandı | Migration **0018** (`idx_text_segments_page`) + 3 regresyon testi | **kritik** — 100 sayfalık belge açılışı ~91 sn; okuyucu pratikte donuyordu | `pages_for_run_plan_uses_indexes` (plan indeksi kullanmalı), `pages_for_run_reads_only_its_own_run`, `pages_for_run_returns_first_segment_of_a_page`; ölçüm: ilk sayfa p50 **0,314 ms** |
| 33 | `pre_update` testinde **sabit `v17`** yedek adı | Şema v18'e çıkınca `lists_only_this_vaults_backups_and_flags_corrupt_ones` kırmızı: ad v17, dosya v18 → `name_mismatch` + `problem` doldu | Test adı `CURRENT_SCHEMA_VERSION`'dan türetmiyordu; aynı kusur `upgrade_tests`'te bir kez yaşanmıştı (satır 3) | 4 yerde ad türetimi (`v{CURRENT_SCHEMA_VERSION}`) | **test** — yalancı kırmızı; doğrulama kapısı gevşetilmedi | storage **267** test yeşil; ad/sürüm karşılaştırması korunuyor |

## Yeniden denenmeyecekler (gerekçesi kodda yazılı)

- **`count(*) OVER ()`** — `bm25` alt sorgusu düzleştirilince sıralama bağlamı
  kayboluyor; toplam, doğrulanmış eşleşmelerle uyuşmuyor. Kodda gerekçe var,
  kullanılmıyor (`crates/storage/src/parsing.rs`).
- **`LIMIT -1` materyalizasyonu** — ölçüldü, mevcut plandan yavaş
  (`docs/releases/F15/BENCHMARK.md`).

## Açık kalan denemeler (yeşil sayılmaz)

| Deneme | Neden açık | Kapanış koşulu |
|---|---|---|
| Windows ARM64 MSI (F17) | `openssl-src` aarch64-pc-windows-msvc derlemiyor | Upstream düzeltmesi ya da OpenSSL'siz TLS yolu; o güne kadar iş kırmızı/`continue-on-error` |
| Windows MSI'ın gerçek makinede kurulumu (F02-T08/T09) | Windows donanımı/oturumu yok | Kurulum + kaldırma + kısayol elle denenir |
| Gerçek model çağrısı (F11-T04) | Sağlayıcı/hesap yok | Hesap verilince tek çağrı + maliyet kaydı |
| ARM64 macOS worker izolasyonu (F01-T07) | Kum havuzu politikası var, native sandbox yok | `unsafe_code` kapısı gevşetilmeden çözüm yok; belgeli açık |

## Bu defterin sınırı

Defter **yaşanmış** olayları taşır; yaşanmamış riskleri "denenmiş" gibi yazmaz.
Bir olay buraya girdiğinde yeniden test kararı da yazılır — kararın kendisi
testle bağlanmıyorsa satır eksik sayılır.
