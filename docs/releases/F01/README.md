# F01 — Teknik doğrulamalar ve mimari kararlar

- **Sürüm etiketi:** `v0.0.1-spike` (hedef)
- **Durum:** Devam ediyor

## Spike matrisi

| İş | Deney | Durum | Kanıt |
|---|---|---|---|
| F01-T01 | Tauri/Rust/React üç hedefte derleme | 🚧 CI matrisi kuruldu, ilk koşu bekleniyor | `.github/workflows/ci.yml` |
| F01-T02 | PyInstaller onedir + PyMuPDF + Tesseract çevrimdışı | ⬜ | — |
| F01-T03 | SQLCipher/FTS5/WAL açık metin işaretçi taraması | ✅ Ölçüldü (sınırlar aşağıda) | `crates/storage/tests/plaintext.rs` (6), `crates/ingest/tests/worker_temp_plaintext.rs` (2) |
| F01-T04 | Streaming AEAD + truncation + kurtarma | 🚧 `crates/vault` prototipi | `cargo test -p dim-vault` |
| F01-T05 | PDF redaksiyon bağımsız doğrulama | ⬜ | — |
| F01-T06 | UDF/DOCX yapı ve ek envanteri | ⬜ | — |
| F01-T07 | Worker izolasyonu (kapsam dışı okuma/yazma/ağ) | ⬜ | — |
| F01-T08 | CLI araçsız/izinli profil kanıtı | ⬜ | — |
| F01-T09 | Yerel model RAM/ilk yanıt/iptal | ⬜ | — |
| F01-T10 | Worker protokolü bozuk frame/süreç ölümü | 🚧 | `workers/document/tests` |
| F01-T11 | Sürüm kilitleme ve ADR üretimi | ✅ | `Cargo.toml`, `package.json`, `docs/adr/` |
| F01-T12 | Başarısız deneme kaydı | ⬜ | bu dosya |

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
