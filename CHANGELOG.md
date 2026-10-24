# Değişiklik günlüğü

Biçim: [Keep a Changelog](https://keepachangelog.com/tr/1.1.0/).
Sürümleme: şartnamedeki faz etiketleri (`docs/roadmap.md`).

## [0.7.2] — 2026-10-23 — F06 görüntü/TIFF

- PNG/JPEG/TIFF artık ayrıştırılır; çok kareli TIFF'te her kare ayrı fiziksel sayfa
- OCR kapalıyken metin uydurulmaz; sayfa "metin katmanı yok (OCR gerekli)" uyarısıyla boş döner
- Kare başına 50 MP ve 100 kare sınırı; aşım `E_IMAGE_TOO_LARGE`
- OCR güven skoru tek geçişte `image_to_data` ile ölçülür; düşük skor (<60) uyarıda açıkça yazılır
- Görüntü koordinatlarının piksel olduğu açıkça uyarılır; basılı sayfa ölçüsü sayılmaz

## [0.7.1] — 2026-10-23 — F06 UDF

- UDF (UYAP) ZIP/XML güvenli ayrıştırma: DTD/ENTITY reddi, zip-slip/şifreli giriş/boyut sınırları
- `content.xml` metin havuzundan Unicode scalar offset'lerle metin; paragraf birleştirme
- Bilinmeyen öğe ve gömülü nesne envanteri uyarı olarak; imza yalnız "tespit edildi — denetlenmedi"
- İçe aktarma `.udf` dosyasını yalnız ZIP içinde `content.xml` varsa kabul eder; worker MIME `application/udf`
- Okuyucuda ayrıştırma uyarıları açılır listede gösterilir

## [0.7.0] — 2026-10-23 — F07 okuyucu

- Arama sonucu kendi belge sürümünü taşır (`documentVersionId`); sonuç "en son sürüm" varsayımıyla açılmaz
- `anchor_locate`: anchor → belge sürümü, fiziksel sayfa indeksi ve alıntının segmentte hâlâ birebir durduğu bilgisi
- Yan yana iki belge okuyucusu; her panel kendi sürümünü, sayfasını ve metin seçimini korur
- Arama sonucundan "Solda aç / Sağda aç" ile doğru sürüm ve sayfaya atlama, sorgu vurgusu
- Kaynak alıntılar listesinden "Okuyucuda aç"; kopmuş alıntı ve kısmi ayrıştırma rozetleri
- Belgeler ekranına "Oku" girişi; metin seçiminden kaynak bağlama okuyucuda

## [0.10.1] — 2026-10-24 — F14 şifreli devir paketi

- `vault::package` seal/open: Argon2id + AES-256-GCM, `DIMPAK1` başlığı
- `export_package` parola ile zip'i `.dimpak` dosyasına çevirir, klasörü kaldırır
- 4 vault testi (roundtrip, yanlış parola, kurcalama, yabancı bayt)
- Çıktılar ekranında paket parolası alanı ve şifreli yol gösterimi

## [0.10.0] — 2026-10-23 — F13 elle bölge maskeleme

- Sayfa önizlemesi üzerinde dikdörtgen seçimi; piksel → punto (72/dpi) çevrimi
- `redact_pdf_regions`: bölge doğrulaması, çok sayfa, küçük/geçersiz bölge reddi
- Seçili bölgede metin kalırsa `E_REDACT_VERIFY`, dosya yazılmaz
- 2 yeni Python testi; bölge modunda OCR doğrulaması yok (dürüst bayrak)

## [0.9.1] — 2026-10-23 — F03 parola değiştirme + kurtarma düzeltmesi

- `VaultFile::change_password` (atomik başlık yazımı, DB anahtarı değişmez)
- **Kritik:** kurtarma anahtarı KDF girdisi base32 metniydi; ham 32 bayta çevrildi
- Ayarlar'da parola değiştirme formu; 3 vault testi

## [0.9.0] — 2026-10-23 — F03 blob GC

- Referanssız blob taraması/temizliği (kuru çalıştırma + gerçek), dosya uyarısıyla
- Ayarlar'da iki adımlı akış; 3 storage testi

## [0.8.1] — 2026-10-23 — F03 otomatik kilit

- 10 dakika hareketsizlikte kasa kendiliğinden kilitlenir (30 sn kontrol)
- Saat geri alınması güvenli tarafta ele alınır; 5 birim testi

## [0.8.0] — 2026-10-23 — F02 worker paketleme

- PyInstaller onedir worker `.app` içine gömülüyor; `bundled_worker_exe()` çift `resources` yolunu arıyor
- Tauri NPM paketleri yayınlanan sürümlere sabitlendi, lockfile eşitlendi
- Paketlenmiş ikili macOS arm64'te ping/capabilities ile doğrulandı (PyMuPDF 1.24.14, DOCX, UDF, OCR)

## [0.7.0] — 2026-10-23 — F06 kalanları

- DOCX dipnot/sonnot/yorum/izlenen değişiklik envanteri metne eklenir
- Karma PDF'te yalnız metinsiz sayfa OCR edilir (kanıtlı test)
- `settings_hash` MIME + OCR + dil + motor + tessdata parmak izi taşır

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
