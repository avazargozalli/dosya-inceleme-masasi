# Değişiklik günlüğü

Biçim: [Keep a Changelog](https://keepachangelog.com/tr/1.1.0/).
Sürümleme: şartnamedeki faz etiketleri (`docs/roadmap.md`).

## [0.3.0] — 2026-10-23 — F06/F07: OCR, DOCX ve kaynak zinciri

### Eklendi

- OCR: Tesseract, `tur+eng`, 300 dpi; dil paketi keşfi, motor yoksa açık hata
- DOCX: paragraf, tablo, üst/alt bilgi; sayfa düzeni uyarısı
- Düz metin (UTF-8) çıkarımı ve MIME dağıtımı
- Kaynak anchor: okuyucuda seçim → Unicode scalar offset → segmentte birebir
  doğrulama → hash + prefix/suffix ile kayıt
- Not ↔ kaynak alıntı bağı; not oluştururken anchor seçimi
- Yedek al / yedekten geri yükle: `sqlcipher_export`, manifest, açıp doğrulama
- Ayarlar ekranı: kasa bilgisi, yedek akışı, teşhis notu

### Düzeltildi

- Tauri NPM paketleri Rust crate minor'larıyla hizalandı (2.1.1 / 2.0.1)
- Ölü `macos-13` runner'ı kaldırıldı; x86_64 hedefi `macos-14` üzerinde

## [0.2.0] — 2026-10-23 — F02/F03 çekirdeği

### Eklendi

- Monorepo: `apps/desktop`, `crates/*`, `workers/document`, `packages/contracts`
- Şifreli kasa (`crates/vault`): Argon2id parola sarma, ayrı kurtarma yuvası,
  chunked AES-256-GCM blob, kesilme/bit değiştirme tespiti
- SQLCipher şeması (`crates/storage`): 47 tablo, FK/CHECK, migrasyon checksum,
  FTS5, denetim hash zinciri, `sqlcipher_export` ile şifreli yedek
- Dosya çalışma alanı: cases CRUD, iyimser eşzamanlılık, arşiv/geri dönüşümlü silme
- Belge içe aktarma: SHA-256, içerikten tür algılama, şifreli blob, sürüm kaydı
- Ayrıştırma: PyMuPDF worker, sürümlü `parse_run`, sayfa/segment kayıtları
- Arama: dosya kapsamlı FTS5; sonuçta sayfa ve ham metin
- Kronoloji: tarih türü/kesinliği, bilinmeyen tarih ayrı grup
- Görevler: kullanıcı tarihli, tamamlanabilir
- Arayüz: Açık Minimal tasarım; kasa, dosyalar, arama, kronoloji, görevler
- CI: sözleşme üretimi, TS/vitest, Rust (Linux+macOS), Tauri check, Python worker

### Bilinmeyenler / henüz yok

- OCR (Tesseract), DOCX/UDF yapı çıkarımı
- AI sağlayıcı bağlantısı (`OffProvider`), CLI yürütme
- Maskeleme, rapor, taşınabilir devir paketi, imzalı kurulum

## [0.0.0-plan] — 2026-10-23

- Şartname, kapsam matrisi, lisans sicili, tehdit modeli, ADR-0001…0010
