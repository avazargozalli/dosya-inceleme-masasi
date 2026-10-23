# ADR-0003: Belge işleme ayrı worker sürecindedir

- **Durum:** Kabul edildi (F01 spike doğrulaması bekliyor)
- **Tarih:** 2026-10-23
- **Karar sahibi:** Teknik lider + ürün sahibi
- **Bağlı gereksinimler:** şartname §04.2, §18

## Bağlam

PyMuPDF/Tesseract Rust'a gömülünce saldırı yüzeyi ve lisans karmaşası büyür; süreç sınırı iptal/limit yönetimini kolaylaştırır.

## Karar

PDF/OCR/DOCX işlemleri paketlenmiş Python worker'da çalışır; ayrı süreç tek başına OS sandbox değildir.

## Uygulama

Worker'a kasa anahtarı, DB bağlantısı ve tüm klasör yolu verilmez; iş bazında dosya/parça gönderilir. Çıktı şema doğrulamasından geçer.

## Sonuçlar ve sınırlar

Kötü niyetli belge worker'ı ele geçirebilir; bu nedenle worker'ın yetkisi dosya okuma listesi + bellek limitiyle daraltılır (F06).

## Doğrulama

F01 kabul kanıtı: `docs/releases/F01/`. Doğrulama başarısız olursa bu ADR `Proposed`
durumuna döner ve uygulama kodu ona göre düzeltilir; karar sessizce değiştirilmez.
