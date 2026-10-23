# ADR-0010: AI kalıcı hukuk kaydına doğrudan yazmaz

- **Durum:** Kabul edildi (F01 spike doğrulaması bekliyor)
- **Tarih:** 2026-10-23
- **Karar sahibi:** Teknik lider + ürün sahibi
- **Bağlı gereksinimler:** şartname §04.2, §18

## Bağlam

Hukuki kaydın sorumluluğu avukattadır; otomatik yazma denetimi ve geri alınabilirliği bozar (INV-04, INV-08).

## Karar

AI yalnız `review_items` adayı üretir; kalıcı kayıt domain komutu + kullanıcı onayıyla oluşur.

## Uygulama

Aday → inceleme kararı → domain transaction (event/claim/note) + audit + outbox tek transaction.

## Sonuçlar ve sınırlar

Toplu kabul yalnız kaynağı doğrulanmış ve OCR uyarısı olmayan adaylarda mümkündür.

## Doğrulama

F01 kabul kanıtı: `docs/releases/F01/`. Doğrulama başarısız olursa bu ADR `Proposed`
durumuna döner ve uygulama kodu ona göre düzeltilir; karar sessizce değiştirilmez.
