# F01 — Teknik doğrulamalar ve mimari kararlar

- **Sürüm etiketi:** `v0.0.1-spike` (hedef)
- **Durum:** Kısmi — yalnız T07 (worker izolasyonu) ve T09 (yerel model ölçümü) açık

## Spike matrisi

| İş | Deney | Durum | Kanıt |
|---|---|---|---|
| F01-T01 | Tauri/Rust/React üç hedefte derleme | ✅ 7/7 iş yeşil (Ubuntu, macOS, Tauri check, Windows ayrı) | `.github/workflows/ci.yml` |
| F01-T02 | PyInstaller onedir + PyMuPDF + Tesseract çevrimdışı | ✅ Ölçüldü (macOS arm64): 118 dosya + 18 bağ · 75,6 MiB · manifest 136/136 doğrulandı · ağ kapalı sandbox'ta gerçek çıkarım. Tesseract **gömülmedi** | `scripts/build-worker.sh`, `scripts/worker_manifest.py`, `scripts/verify-worker-offline.py`, `crates/ingest/src/packaging.rs`, `docs/releases/F01/PACKAGING.md` |
| F01-T03 | SQLCipher/FTS5/WAL açık metin işaretçi taraması | ✅ Ölçüldü (sınırlar aşağıda) | `crates/storage/tests/plaintext.rs` (6), `crates/ingest/tests/worker_temp_plaintext.rs` (2) |
| F01-T04 | Streaming AEAD + truncation + kurtarma | ✅ `crates/vault`; parola değiştirme + kurtarma KDF düzeltmesi | `cargo test -p dim-vault` (15 + 3 + 4) |
| F01-T05 | PDF redaksiyon bağımsız doğrulama | ✅ Ölçüldü (sınırlar aşağıda) | `workers/document/tests/test_redaction_independent.py` (18), `crates/ingest/tests/redaction_hidden_objects.rs` (2), `fixtures/synthetic/redaction-hidden.pdf` |
| F01-T06 | UDF/DOCX yapı ve ek envanteri | ✅ Ölçüldü: DOCX yapı+ek+değişiklik izi; UDF yapı+paket girdileri (sınırlar aşağıda) | `workers/document/tests/test_inventory.py` (14), `crates/ingest/tests/document_inventory.rs` (3), `docs/releases/F01/INVENTORY.md` |
| F01-T07 | Worker izolasyonu (kapsam dışı okuma/yazma/ağ) | ⬜ | — |
| F01-T08 | CLI araçsız/izinli profil kanıtı | 🚧 Codex: `--ignore-user-config --ignore-rules` ile hook kapalı (gerçek koşu); Claude/Antigravity/Kimi açık | `crates/cli-adapters/src/exec.rs`, `docs/releases/F10/HOOKS.md` |
| F01-T09 | Yerel model RAM/ilk yanıt/iptal | ⬜ | — |
| F01-T10 | Worker protokolü bozuk frame/süreç ölümü | ✅ Rust tarafı ölçüldü (Unix); Windows denenmedi | `crates/ingest/src/worker.rs` testleri (4 yeni: süreç ölümü, zaman aşımı, aşırı çerçeve, bozuk çerçeve) + `workers/document/tests/test_frames.py`, `test_protocol.py` (91 pytest) |
| F01-T11 | Sürüm kilitleme ve ADR üretimi | ✅ | `Cargo.toml`, `package.json`, `docs/adr/` |
| F01-T12 | Başarısız deneme kaydı | ✅ Defter yürürlükte (20 gerçek olay; yaşadıkça eklenir) | `docs/releases/F01/FAILURES.md` |

> Not: F01'in platform spike'ları gerçek cihaz/CI koşusu olmadan "geçti" sayılmaz.
> Bu dosya yalnız gerçekten çalıştırılan deneylerin sonucunu taşır.

## F01-T03 — açık metin işaretçi taraması (ölçüldü, 25 Eki 2026)

İşaretçi: `ZZ-KIRA-MARKER-7F3A9C` (bayt araması) + `ZZKIRAMARKERZZ` (FTS belirteci).
Her test **iki yönlü** kanıt üretir: işaretçi gerçekten kasanın içine yazıldı
(sorgu/FTS ile geri okunur) **ve** diskteki baytlarda bulunamaz. Yalnız ikincisi
ölçülseydi "hiç yazılmamış" içerik de testi geçerdi.

| Test | Ne ölçüyor |
|---|---|
| `database_file_leaks_no_plaintext_and_rejects_wrong_key` | Kasa dosyasında SQLCipher başlığı, dosya adı, `sqlite_master`, FTS/gölge tablo adları ve işaretçi yok; yanlış anahtar açmıyor |
| `fts5_index_holds_marker_but_file_bytes_do_not` | İşaretçi FTS `MATCH` ile bulunuyor (indeks gerçekten dolu: `_data`/`_idx` satırları > 0), buna karşın dosya baytlarında yok |
| `wal_and_shm_sidecars_hold_no_plaintext` | `PRAGMA journal_mode` = `wal` doğrulanır; bağlantı **açıkken** `vault.db-wal` (≥ 4096 bayt) ve `vault.db-shm` taranır — kapanış WAL'ı sıfırladığı için tarama kapanıştan önce yapılır |
| `deleted_content_is_scrubbed_from_the_file` | Silme sonrası arama boş; `secure_delete = 1`; dosya baytları temiz |
| `temp_storage_is_memory_and_leaves_no_marker_file` | `PRAGMA temp_store` = 2 (MEMORY); geçici tablo + 5.000 satırlık sıralama sonrası `$TMPDIR`'da yeni `etilqs_*` dosyası yok |
| `planted_marker_is_detected_by_the_scan` | **Dedektör kanıtı**: aynı tarama kasıtlı bırakılan işaretçiyi buluyor (sıfır sonucu körlük değil) |
| `worker_leaves_no_plaintext_in_temp_or_working_dir` (ingest) | Gerçek Python worker'ı düz metin çıkarımı yapar (işaretçi çıkarımda geri döner), sonra `$TMPDIR`'da yeni doğan dosyalarda ve worker çalışma dizininde işaretçi aranır |
| `planted_marker_is_detected_by_the_scan` (ingest) | Aynı dedektör kanıtı, worker taraması için |

**Ölçülen sayı (gerçek çıktı):** FTS5 harici içerikli tabloda silme bölütü hemen
yok etmiyor — kalıntı bölüt sayısı silme sonrası **4**, `optimize` sonrası **2**.
Arama sonucu her iki durumda da boş; kalıntı yalnız şifreli sayfalarda duruyor.

**Dürüst sınırlar:**
- İnceleme bayt aramasıdır; kriptografik analiz yapılmaz, "çözülemez" kanıtlanmaz.
- `-wal`/`-shm` yalnız bağlantı açıkken anlamlı; testte bu an ölçülür, kapanış
  sonrası dosyalar zaten silinir/sıfırlanır.
- FTS silme kalıntısı sıfıra inmez (bölüt birleştirme eşiği); kalıntının
  şifreli olduğu ölçülür, "fiziksel olarak yok" denmez.
- Geçici depolama `temp_store` değeri + gözlemlenen dosya iziyle ölçülür;
  işletim sisteminin sayfa takası (swap) kapsam dışı.
- Worker testi Python + `workers/document` varsa koşar (bu makinede koştu,
  5 sn); yoksa atlandığını yazar — "geçti" sayılmaz.

## F01-T10 — worker protokol arıza yolları (Rust tarafı ölçüldü, Unix)

Vekil süreçlerle (`/bin/sh`) dört arıza yolu ve **asılma yok** ölçülür:

| Test | Arıza | Beklenen kod |
|---|---|---|
| `worker_process_death_is_reported_without_hanging` | Süreç istek yazıldıktan sonra ölür (`exit 7`) | `E_WORKER_DISCONNECTED`, < 15 sn |
| `unresponsive_worker_times_out_instead_of_hanging` | Süreç yanıt vermez (`sleep 30`), istemci sınırı 500 ms | `E_WORKER_TIMEOUT`, < 20 sn |
| `oversized_frame_is_reported_as_frame_error` | Tek satırda 1,2 MB (çerçeve sınırı 1 MiB) | `E_WORKER_FRAME` |
| `malformed_frame_is_reported_as_bad_response` | JSON olmayan satır | `E_WORKER_RESPONSE` |

**Bulunan kusur (düzeltildi):** `WorkerError::FrameTooLarge` tanımlıydı ama
**hiç üretilmiyordu** — okuyucu iş parçacığı aşırı çerçeveyi `Disconnected`'a
çeviriyordu; çağıran süreç ölümü ile protokol ihlalini ayırt edemiyor, yanlış
müdahale (yeniden başlat) yapıyordu. Ayrım eklendi (`E_WORKER_FRAME` /
`E_WORKER_RESPONSE` / `E_WORKER_DISCONNECTED`).

**Dürüst sınırlar:** testlerdeki süreçler vekildir; gerçek worker bu dört arızayı
üretmedi (pytest tarafı `test_frames.py`/`test_protocol.py` ayrı kapsam).
Windows'ta `/bin/sh` olmadığı için bu dört test **denenmedi**. Tauri katmanında
`E_WORKER_*` kodları için özel kullanıcı mesajı yok — genel hata olarak yüzeye
çıkar (ayrı iş).

## F01-T05 — karartmanın bağımsız okuyucuyla doğrulanması (ölçüldü, 25 Eki 2026)

Kararı veren kontroller artık PyMuPDF'e dayanmıyor:

1. **Ham bayt taraması** — hiçbir PDF kütüphanesi yok. Terim üç kodlamada aranır:
   düz UTF-8, kaçışlı PDF dizesi ve **onaltılık** (`<5a5a…>`, `<FEFF…>`).
2. **Flate akışlarını kendi açıcımız** (`zlib`) açıp aynı aramayı yapar —
   "metin katmanında yok" ile "dosyada yok" ayrımı ölçülebilir.
3. **pypdf** ikinci okuyucu (üretici değil); kurulu değilse motor listesine
   **girmez** ve uyarı yazılır.
4. **Tesseract** görüntü/piksel kontrolü; bölge karartmasında bölgenin
   OCR'ı (metin katmanı yoksa OCR'dan gelen kelimelerle) karşılaştırılır.

### Ölçülen vektörler ve bulunan kusurlar

| Vektör | Önce | Bulunan kusur | Şimdi |
|---|---|---|---|
| Metin katmanı | temiz | — | temiz |
| Görünmez metin (Tr 3) / beyaz / 0,6 punto | temiz | — | temiz |
| **Sayfa dışı metin** (MediaBox dışı, x=900) | **dosyada kalıyordu** | `search_for`/`get_text` sayfa dışını hiç görmez; `remove_hidden` kapsamıyor | `get_texttrace` + cropbox dışı bölge karartılır (`offpage_spans_removed`) |
| **Yapışkan not `/Contents`** | **ham baytta ve pypdf'te kalıyordu** | `scrub` yalnız yanıtları siler, notun metnini silmez | terimi taşıyan açıklama bütünüyle silinir (`annotations_removed`); zararsız not korunur |
| **Annotation `/T` (yazar)** | **kalıyordu** | aynı | aynı yol |
| **Katalog içine gömülü JavaScript** (`/OpenAction`) | **kalıyordu** | `scrub(javascript=True)` yalnız ayrı nesne olan `/S /JavaScript`'i siler | katalog/sayfa `/OpenAction`+`/AA` ve `/Names/JavaScript` kaldırılır (`scripts_removed`) |
| Ek dosya, `/Info` metadata, XMP | temiz | — | temiz |
| **Annotation içeren herhangi bir PDF** | **karartma çöküyordu** | PyMuPDF 1.24.14 `delete_responses`: yanıtı olmayan annotta `None.m_internal` | `reset_responses=False`; yanıt zincirleri terim taramasıyla silinir |
| **Raster (taranmış) metin** | "temiz" sanılıyordu | terim araması pikseli bulamaz | OCR varsa `remaining` → çıktı **reddedilir** (`E_REDACT_VERIFY`); yoksa `ocrVerified=false` |
| **Kara kutu çizmek** | maskeleme sanılırdı | metin dosyada kalır | dedektör yakalıyor (negatif kontrol testi) |
| **pypdf kurulu değilse** | motor listesinde "pypdf" yazıyordu | yalan doğrulama iddiası | listede yok + uyarı |

**Bulunan kusur (kendi tarayıcımızda):** ilk yazdığım bayt taraması yalnız düz
UTF-8 arıyordu; PyMuPDF metni **onaltılık** yazdığı için sayfa metnini hiç
görmüyor, "temiz" diyordu. Ölçüm bunu gösterdi (testte
`test_compressed_stream_is_inflated_before_searching` kurgusu: terim ham baytta
yok, açılmış akışta var). Aynı turda `endstream` içindeki `stream` sözcüğünü de
yakalayan regex düzeltildi.

### Kanıt

- `workers/document/tests/test_redaction_independent.py` — **18 test**; her
  testte iki yön: işaretçi kaynakta var (tarayıcı buluyor) **ve** çıktıda yok.
- `crates/ingest/tests/redaction_hidden_objects.rs` — gerçek worker, gerçek
  fikstür (`fixtures/synthetic/redaction-hidden.pdf`: metin + sayfa dışı metin +
  yapışkan not + gömülü JS). Ölçüm: `annotations_removed=1`,
  `offpage_spans_removed=1`, `scripts_removed=1`, `remaining=[]`, görünen metin
  korunuyor.
- `crates/ingest/src/worker.rs::redaction_response_parses_verification_counts…`
  — sayılar/uyarılar Tauri'ye taşınıyor; alanları bilmeyen eski worker 0/boş
  döner, hata vermez.
- Arayüz (Redaction sonuç kartı): "Bağımsız tarama" satırı (açılan/ikili/
  açılamayan akış), "Gizli nesne temizliği" sayıları ve "Doğrulama uyarıları"
  listesi — "doğrulandı" rozeti artık bu sayılarla birlikte okunuyor.

**Dürüst sınırlar:**
- Tarama **bayt düzeyindedir**: alt küme font kodlaması (özel CMap), `TJ`
  dizisinde bölünmüş dizeler veya başka bir filtre (ASCII85) kullanan üreticide
  terim baytlarda görünmez; o durumda karar pypdf/PyMuPDF metin çıkarımına
  dayanır ve bu sınır arayüzde yazılmaz (yalnız burada).
- Sayfa dışı temizlik `cropbox` sınırına dayanır; `TrimBox`/`BleedBox` ile
  oynanmış belgelerde kenar metni görünür sayılabilir (silinmez).
- Annotation **tamamen** silinir (kısmi metin temizliği yapılmaz): kullanıcı
  notu kaybolur, uyarı sayısı gösterilir, özgün sürüm kasadan silinmez.
- JavaScript temizliği yalnız `/OpenAction`, `/AA` ve `/Names/JavaScript`
  yollarını kapsar; form alanı eylemleri (`/A`) ayrı vektördür.
- OCR yolu PyMuPDF ile rasterleştirir (piksel doğrulaması bağımsız motordan
  değil, aynı rasterleştiriciden geçer); tesseract yoksa görüntü doğrulanmaz ve
  bu `ocrVerified=false` + uyarı olarak görünür.
- Görüntüde kalan terim **reddedilir** (bölge karartması gerekir); taranmış
  belgede terim araması tek başına yeterli değildir.
- Tauri penceresi açılmadı → kartın görsel QA'sı yok; alanlar tip denetimi,
  birim testleri ve üretim derlemesiyle doğrulandı.
