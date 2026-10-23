# ADR-0001: Yerel şifreli kasa tek doğruluk kaynağıdır

- **Durum:** Kabul edildi (F01 spike doğrulaması bekliyor)
- **Tarih:** 2026-10-23
- **Karar sahibi:** Teknik lider + ürün sahibi
- **Bağlı gereksinimler:** şartname §04.2, §18

## Bağlam

Bulut zorunluluğu meslek sırrı ve veri sahipliği riski üretir; çevrimdışı çalışma temel gereksinimdir (REQ-001, REQ-039).

## Karar

Sunucu hesabı zorunlu değildir; verinin esas sahibi kullanıcının cihazındaki şifreli kasadır.

## Uygulama

Kasa dosyası (`*.dimvault`) SQLCipher DB + şifreli blob dizini + sürümlü başlık içerir. Sunucu sync yoktur.

## Sonuçlar ve sınırlar

Kullanıcı kasayı kaybederse kurtarma anahtarı olmadan veri açılamaz. Eşzamanlı çoklu cihaz yoktur.

## Doğrulama

F01 kabul kanıtı: `docs/releases/F01/`. Doğrulama başarısız olursa bu ADR `Proposed`
durumuna döner ve uygulama kodu ona göre düzeltilir; karar sessizce değiştirilmez.
