# ADR-0006: Proje lisansı AGPL-3.0-or-later

- **Durum:** Kabul edildi (F01 spike doğrulaması bekliyor)
- **Tarih:** 2026-10-23
- **Karar sahibi:** Teknik lider + ürün sahibi
- **Bağlı gereksinimler:** şartname §04.2, §18

## Bağlam

PyMuPDF AGPL-3.0 ve hukuk alanında şeffaflık beklentisi; kapalı türev ürünlerin topluluğa geri katkısı korunur.

## Karar

Kaynak açık; ağ üzerinden hizmet verilirse kaynak sunma yükümlülüğü kabul edilir.

## Uygulama

`LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md`, SBOM ve kaynak dağıtım adımları CI'da zorunludur.

## Sonuçlar ve sınırlar

AGPL uyumsuz bağımlılık kullanılamaz; ticari kullanım için ayrı lisans görüşmesi.

## Doğrulama

F01 kabul kanıtı: `docs/releases/F01/`. Doğrulama başarısız olursa bu ADR `Proposed`
durumuna döner ve uygulama kodu ona göre düzeltilir; karar sessizce değiştirilmez.
