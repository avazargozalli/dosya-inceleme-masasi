# ADR-0007: İlk sürümde ortak düzenleme ve canlı senkronizasyon yoktur

- **Durum:** Kabul edildi (F01 spike doğrulaması bekliyor)
- **Tarih:** 2026-10-23
- **Karar sahibi:** Teknik lider + ürün sahibi
- **Bağlı gereksinimler:** şartname §04.2, §18

## Bağlam

Çok yazıcılı SQLite ve bulut çakışması veri bozulması üretir; kurtarma ve denetim izi zayıflar (INV-09).

## Karar

Kasa tek yazıcılıdır; OneDrive/iCloud gibi araçlarla canlı eşzamanlama desteklenmez.

## Uygulama

Yedek kapalı ve tutarlı snapshot olarak taşınır; geri yükleme yeni kasa olarak açılır.

## Sonuçlar ve sınırlar

Ekip çalışması ihtiyacı F18 sonrası ayrı tasarım konusudur.

## Doğrulama

F01 kabul kanıtı: `docs/releases/F01/`. Doğrulama başarısız olursa bu ADR `Proposed`
durumuna döner ve uygulama kodu ona göre düzeltilir; karar sessizce değiştirilmez.
