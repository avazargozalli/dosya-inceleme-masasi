# ADR-0002: UI veri katmanına doğrudan erişmez

- **Durum:** Kabul edildi (F01 spike doğrulaması bekliyor)
- **Tarih:** 2026-10-23
- **Karar sahibi:** Teknik lider + ürün sahibi
- **Bağlı gereksinimler:** şartname §04.2, §18

## Bağlam

Renderer'da açık veri erişimi XSS/kötü niyetli belge senaryosunda kapsam atlamasına yol açar (INV-05, INV-06).

## Karar

React arayüzü SQL, dosya sistemi veya süreç çalıştırmaz; yalnız türlenmiş Tauri komutlarıyla çekirdeğe gider.

## Uygulama

Her komut `request_id` taşır, çekirdekte yeniden doğrulanır, `case_id` kapsamı komut içinde zorunludur.

## Sonuçlar ve sınırlar

Yeni komut eklemek şema + capability + test gerektirir; genel `query`/`exec` komutu yasaktır.

## Doğrulama

F01 kabul kanıtı: `docs/releases/F01/`. Doğrulama başarısız olursa bu ADR `Proposed`
durumuna döner ve uygulama kodu ona göre düzeltilir; karar sessizce değiştirilmez.
