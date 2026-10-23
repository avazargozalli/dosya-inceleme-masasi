# ADR-0008: UYAP ve imza otomasyonu kapsam dışıdır

- **Durum:** Kabul edildi (F01 spike doğrulaması bekliyor)
- **Tarih:** 2026-10-23
- **Karar sahibi:** Teknik lider + ürün sahibi
- **Bağlı gereksinimler:** şartname §04.2, §18

## Bağlam

Resmî sistemle otomatik etkileşim yetki, sorumluluk ve mevzuat riski taşır (REQ-039).

## Karar

Uygulama UYAP'a girmez, belge göndermez, kesin hukuki süre hesabı yapmaz.

## Uygulama

Yalnız yerel belgeler işlenir; kullanıcı tarihleri elle doğrular, uygulama 'son gün hesaplandı' demez.

## Sonuçlar ve sınırlar

Kullanıcı talebi olursa ayrı ürün/entegrasyon değerlendirmesi gerekir.

## Doğrulama

F01 kabul kanıtı: `docs/releases/F01/`. Doğrulama başarısız olursa bu ADR `Proposed`
durumuna döner ve uygulama kodu ona göre düzeltilir; karar sessizce değiştirilmez.
