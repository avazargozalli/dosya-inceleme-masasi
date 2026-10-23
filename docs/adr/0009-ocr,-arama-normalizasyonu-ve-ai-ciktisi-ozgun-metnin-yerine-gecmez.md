# ADR-0009: OCR, arama normalizasyonu ve AI çıktısı özgün metnin yerine geçmez

- **Durum:** Kabul edildi (F01 spike doğrulaması bekliyor)
- **Tarih:** 2026-10-23
- **Karar sahibi:** Teknik lider + ürün sahibi
- **Bağlı gereksinimler:** şartname §04.2, §18

## Bağlam

Alıntı doğruluğu ve delil zinciri ancak özgün metne dönüşle korunur (INV-01, INV-02).

## Karar

Her türev ayrı sürüm ve kaynak konumu taşır; özgün baytlar değişmez.

## Uygulama

`document_versions` immutable; `parse_runs` sürümlü; anchor hangi parse'a bağlıysa o açılır.

## Sonuçlar ve sınırlar

Yeniden ayrıştırma eski anchor'ı bozmaz; yeniden bağlama önerisi kullanıcıya sorulur (07.3).

## Doğrulama

F01 kabul kanıtı: `docs/releases/F01/`. Doğrulama başarısız olursa bu ADR `Proposed`
durumuna döner ve uygulama kodu ona göre düzeltilir; karar sessizce değiştirilmez.
