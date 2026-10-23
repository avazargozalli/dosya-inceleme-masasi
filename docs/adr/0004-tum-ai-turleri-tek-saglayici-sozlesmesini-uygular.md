# ADR-0004: Tüm AI türleri tek sağlayıcı sözleşmesini uygular

- **Durum:** Kabul edildi (F01 spike doğrulaması bekliyor)
- **Tarih:** 2026-10-23
- **Karar sahibi:** Teknik lider + ürün sahibi
- **Bağlı gereksinimler:** şartname §04.2, §18

## Bağlam

Aynı çıktı doğrulama ve inceleme akışı sağlayıcıdan bağımsız olmalıdır (INV-02, INV-08).

## Karar

Yerel model, CLI adaptörleri ve doğrudan API aynı `Provider` arayüzünü kullanır; kaynak doğrulaması sağlayıcı dışındadır.

## Uygulama

`propose() -> Candidate` üretir; aday `evidence` katmanında alıntı eşleşmesinden geçmeden kayda dönüşmez.

## Sonuçlar ve sınırlar

Sağlayıcıya özgü yetenek farkları `capabilities` ile açıkça raporlanır; sessiz geri düşme yoktur.

## Doğrulama

F01 kabul kanıtı: `docs/releases/F01/`. Doğrulama başarısız olursa bu ADR `Proposed`
durumuna döner ve uygulama kodu ona göre düzeltilir; karar sessizce değiştirilmez.
