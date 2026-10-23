# ADR-0005: CLI'ler yalnız resmî yürütülebilir dosyaları üzerinden kullanılır

- **Durum:** Kabul edildi (F01 spike doğrulaması bekliyor)
- **Tarih:** 2026-10-23
- **Karar sahibi:** Teknik lider + ürün sahibi
- **Bağlı gereksinimler:** şartname §04.2, §18

## Bağlam

Sağlayıcı koşulları abonelik kimliğinin üçüncü tarafa aktarılmasını yasaklar; oturum güvenliği CLI'ye aittir (INV-10, INV-11).

## Karar

Claude Code, Codex, Antigravity ve Kimi CLI'leri kullanıcının kurduğu resmî araçlarla çalıştırılır; token/çerez kopyalanmaz.

## Uygulama

Adaptör yalnız `--version`, kimliksiz yetenek yoklaması ve izinli argümanlarla süreç başlatır; ortam değişkenleri allowlist ile geçer.

## Sonuçlar ve sınırlar

CLI sürümü/yeteneği değişince bağlantı `incompatible` olur; otomatik düzeltme yapılmaz, kullanıcıya gösterilir.

## Doğrulama

F01 kabul kanıtı: `docs/releases/F01/`. Doğrulama başarısız olursa bu ADR `Proposed`
durumuna döner ve uygulama kodu ona göre düzeltilir; karar sessizce değiştirilmez.
