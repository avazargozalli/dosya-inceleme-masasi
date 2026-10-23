# Güvenlik test matrisi

Her tehdit, onu **gerçekten** sınayan test ya da kanıtla eşleşir. Boş hücre
"test edilmedi" demektir; niyet değil kanıt aranır.

| # | Tehdit | Varlık | Kanıt | Durum |
|---|---|---|---|---|
| T1 | Cihaz kaybı, kasa dosyasının kopyalanması | `crates/vault` | Argon2id 64 MiB/3 tur, AES-256-GCM; yanlış parola `E_DB_KEY` (vault testleri) | ✅ |
| T2 | Kurcalanmış başlık/blob | `header.rs`, `blob.rs` | AEAD tag + başlık MAC; truncation/tamper testleri | ✅ |
| T3 | DB'de açık metin kalması | `crates/storage` | SQLCipher `PRAGMA key`; yanlış anahtarla `sqlite_master` okunamaz | ✅ |
| T4 | Yedekten sızıntı | `vault_backup` | `sqlcipher_export` + şifreli blob kopyası; manifest hash | ✅ |
| T5 | Arayüzün veri katmanına doğrudan erişimi | `apps/desktop` | Yalnız Tauri komutları; her komut `case_id` kapsamı doğrular | ✅ |
| T6 | Worker'ın dosya sistemine/ağa erişimi | `crates/ingest` | Worker'a dosya yolu değil bayt verilir; NDJSON protokolü | ✅ |
| T7 | Onaysız AI aktarımı | `crates/storage/src/ai.rs` | Onay yok/iptal/süresi geçmiş → `E_AI_CONSENT` (5 test) | ✅ |
| T8 | CLI kimliğinin kopyalanması | `crates/cli-adapters` | Yalnız `--version`; token/oturum okunmaz | ✅ |
| T9 | Maskeli dosyada metin kalması | `workers/document` | `apply_redactions` + pypdf bağımsız okuma + OCR kontrolü | ✅ |
| T10 | Dışa aktarmanın izsiz kalması | `exports` | Her çıktı `exports` kaydı + sha256 | ✅ |
| T11 | Rapor HTML'inde betik çalışması | `engines.py` | Tüm kullanıcı verisi `html.escape`; `<script>` metin olarak kalır (test) | ✅ |
| T12 | Erişilebilirlik regresyonu | `apps/desktop` | `src/a11y.test.ts` (etiket/alt/buton adı) | ✅ |
| T13 | Tedarik zinciri | bağımlılıklar | `scripts/sbom.sh` + CI `sbom` job'u | 🚧 |
| T14 | Paketlenmiş uygulamada worker'sız kalma | `.app` | Paket içi `dim-worker` ping/capabilities (macOS arm64) | ✅ |
| T15 | Otomatik güncelleme imzası | updater | Anahtar yok; iskelet ve doğrulama F16 | ⬜ |

## Kapsam dışı (bilinçli)

- Sunucu tarafı yetkilendirme: ürün tamamen yerel, sunucu yok.
- Çok kullanıcılı kasa: tek kullanıcı, tek kasa.
- Bellek dökümüne karşı tam koruma: `zeroize` kullanılıyor ama işletim sistemi
  düzeyinde tam garanti verilemez; bu, tehdit modelinde kabul edilmiş risk.
