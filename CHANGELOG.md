# Değişiklik günlüğü

Biçim: [Keep a Changelog](https://keepachangelog.com/tr/1.1.0/).
Sürümleme: şartnamedeki faz etiketleri (`docs/roadmap.md`).

## [0.7.0] — 2026-10-23 — F07 okuyucu

- Arama sonucu kendi belge sürümünü taşır (`documentVersionId`); sonuç "en son sürüm" varsayımıyla açılmaz
- `anchor_locate`: anchor → belge sürümü, fiziksel sayfa indeksi ve alıntının segmentte hâlâ birebir durduğu bilgisi
- Yan yana iki belge okuyucusu; her panel kendi sürümünü, sayfasını ve metin seçimini korur
- Arama sonucundan "Solda aç / Sağda aç" ile doğru sürüm ve sayfaya atlama, sorgu vurgusu
- Kaynak alıntılar listesinden "Okuyucuda aç"; kopmuş alıntı ve kısmi ayrıştırma rozetleri
- Belgeler ekranına "Oku" girişi; metin seçiminden kaynak bağlama okuyucuda

## [0.6.1] — 2026-10-23 — F05 parolalı PDF

- `document_parse` isteğe bağlı parola ile parola korumalı PDF açar; parola saklanmaz
- Eksik/yanlış parola `E_PDF_PASSWORD`; belge açılmış sayılmaz
- Belgeler ekranında parola alanı + Ayrıştır/OCR düğmeleri

## [0.6.0] — 2026-10-23 — F08 olay–kaynak

- Olay → kaynak anchor bağı (`event_sources`): belirtir/destekler/çelişir/bağlam
- Aynı bağ idempotent; dosya dışı anchor reddi; kullanıcı onaylı bağ
- Kronoloji ekranında kaynak paneli

## [0.5.1] — 2026-10-23 — F13 ikinci doğrulayıcı

- Karartma çıktısı pypdf ile bağımsız okunur; terim kaldıysa sonuç reddedilir
- OCR sızıntı kontrolü (tesseract varsa) ve dürüst `ocrVerified` bayrağı
- Arayüzde doğrulayan motorlar ve OCR durumu gösterilir

## [0.5.0] — 2026-10-23 — F09/F11/F14

### Eklendi

- Aktarım onayı (`transfer_consents`): dosya kapsamlı, süreli, iptal edilebilir; onaysız çalıştırma `E_AI_CONSENT`
- AI çalıştırma sicili: `ai_runs`, `ai_context_items`, `transfer_receipts`
- Aday kaydı ve inceleme kuyruğuna terfi; aday hukuki kayıt sayılmaz
- Kapsamlılık raporu: belge, sayfa, OCR, boş sayfa, ayrıştırılmamış belge
- PDF rapor çıktısı (HTML kaçışlı, `Story` ile sayfalama)
- Devir paketi: `case.json` + `manifest.json` + `README.txt`
- Çıktılar ve AI inceleme ekranları

## [0.4.0] — 2026-10-23 — F08/F10/F12/F13

### Eklendi

- İddia–delil matrisi, inceleme kuyruğu, kabul/ret/düzelt geçmişi
- CLI keşfi: yalnız `--version` yoklaması; kimlik okunmaz, token kopyalanmaz
- Emsal kütüphanesi, kullanım notu, boyutları ayrı atıf kontrolü
- Gerçek karartma: `apply_redactions` + çıktı doğrulaması; başarısızsa kayıt yok
- Maskeli kopya yeni sürüm olarak bağlanır (`parent_version_id`)

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
