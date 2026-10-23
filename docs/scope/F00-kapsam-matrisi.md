# F00 — Kapsam matrisi ve izlenebilirlik

Kaynak: `docs/spec/DOSYA_INCELEME_MASASI_TAM_PROJE_PLANI.md` §03.2.
Bu tablo, her gereksinimin hangi fazda teslim edileceğini, sorumlu rolü ve **kabul testi
türünü** bağlar. Bir REQ, testi yazılmadan "karşılandı" sayılmaz.

| Kimlik | Zorunlu davranış | Öncelik | Faz | Sorumlu | Test türü |
|---|---|---|---|---|---|
| REQ-001 | Yerel kasa oluşturma, kilitleme, kurtarma | P0 | F03 | core | integration+security |
| REQ-002 | Dosya oluşturma, arşivleme, geri dönüşümlü silme | P1 | F04 | core | integration |
| REQ-003 | Dosya/klasör sürükle-bırak, güvenli dosya seçimi | P1 | F05 | ui+core | e2e |
| REQ-004 | PDF, DOCX, TXT, PNG, JPEG, TIFF, UDF kapsam matrisi | P1 | F05-F06 | worker | integration |
| REQ-005 | Özgün bayt ve SHA-256, türevlerin soy ağacı | P0 | F05 | core | integration |
| REQ-006 | Sayfa/paragraf düzeyinde metin ve konum | P0 | F06 | worker+core | integration |
| REQ-007 | Türkçe/İngilizce OCR ve kalite görünürlüğü | P1 | F06 | worker | integration+manual |
| REQ-008 | Tam metin arama ve filtreler | P1 | F07 | core | integration |
| REQ-009 | Yan yana belge, seçili metne not, kaynağa dönüş | P1 | F07 | ui | e2e |
| REQ-010 | Kişi/kurum ve rol kayıtları, incelemeli birleştirme | P1 | F08 | core | integration |
| REQ-011 | Olay/belge/tebliğ tarihi ayrımı | P0 | F08 | domain | unit+integration |
| REQ-012 | Manuel ve öneri tabanlı kronoloji | P1 | F08/F11 | core | integration |
| REQ-013 | İddia-beyan-belge bağlantıları ve karşılaştırma | P1 | F08 | domain | integration |
| REQ-014 | İnceleme kuyruğu, kabul/ret/düzeltme geçmişi | P0 | F08 | core | integration |
| REQ-015 | AI kapalı/yerel/CLI/API sağlayıcı katmanı | P1 | F09 | core | unit+integration |
| REQ-016 | Claude Code adaptörü | P1 | F10 | core | integration+manual |
| REQ-017 | Codex CLI adaptörü | P1 | F10 | core | integration+manual |
| REQ-018 | Antigravity CLI adaptörü | P1 | F10 | core | integration+manual |
| REQ-019 | Kimi CLI adaptörü | P1 | F10 | core | integration+manual |
| REQ-020 | Sağlayıcı sürüm ve yetenek sicili | P0 | F09-F10 | core | unit+integration |
| REQ-021 | Yerel model indirme/doğrulama, donanım kontrolü | P1 | F09 | core | integration+perf |
| REQ-022 | Gönderim önizlemesi ve aktarım kaydı | P0 | F09 | core+ui | integration+security |
| REQ-023 | Şemalı, kaynaklı AI sonuçları ve doğrulama | P0 | F11 | core | unit+integration |
| REQ-024 | Kapsamlılık raporu, okunamayan sayfalar ve veri boşluğu | P0 | F06/F11 | worker+ui | integration |
| REQ-025 | Emsal kütüphanesi ve künye/alıntı kontrolü | P1 | F12 | core | integration |
| REQ-026 | Maskelenmiş paylaşım kopyası ve çıktı denetimi | P0 | F13 | worker | security+integration |
| REQ-027 | Duruşma/devir notu, belge dizini, DOCX/PDF/CSV/JSON | P1 | F14 | worker+core | integration |
| REQ-028 | Görev ve kullanıcı onaylı tarih, yerel bildirim | P1 | F08/F14 | core+ui | integration |
| REQ-029 | Şifreli yedekleme, geri yükleme, bozulma tespiti | P0 | F03/F14 | core | security+integration |
| REQ-030 | Atomik migrasyon, imzalı güncelleme, kurtarma | P0 | F16 | core | integration+manual |
| REQ-031 | Klavye ve ekran okuyucu erişilebilirliği | P1 | F04/F15 | ui | manual+e2e |
| REQ-032 | macOS ve Windows kurulum/kaldırma akışı | P1 | F02/F16 | build | manual |
| REQ-033 | Hassas veri içermeyen yerel teşhis | P0 | F03/F15 | core | security |
| REQ-034 | Açık kaynak lisans, kaynak dağıtımı, SBOM, katkı süreci | P1 | F00/F16 | legal | review |
| REQ-035 | Sürümlü eklenti sözleşmesi (süre kuralı motoru) | P2 | F18 | domain | unit |
| REQ-036 | Dosya bazında taşıma ve kullanıcı seçimli paylaşım paketi | P1 | F14 | core | integration |
| REQ-037 | Genişletilebilir CLI adaptörü, imzalı yetenek kataloğu | P1 | F09/F18 | core | security |
| REQ-038 | Windows ARM64 yerel dağıtım | P2 | F17 | build | manual |
| REQ-039 | Otomatik dış işlem yapılmaması | P0 | Tüm fazlar | core | security+review |
| REQ-040 | Kota bittiğinde verinin ve kısmi işin korunması | P0 | F10-F11 | core | integration |

## İzlenebilirlik kuralları

- Her test dosyası en az bir REQ kimliği taşır (`// REQ-005` / `# REQ-007`).
- Bir REQ'in testi yoksa CI'da `untraced` olarak raporlanır; P0 için sürüm engelidir.
- Kapsam dışına çıkarma yalnız `docs/adr/` kaydı ve sürüm notu ile yapılır.
- F00 kabulü: üç örnek kullanıcı yolculuğu (arama → kaynak, kronoloji, duruşma hazırlığı)
  REQ'ten teste kadar izlenebilir; sahipsiz P0 yoktur.

## Kapsam sınırları (onaylı)

- UYAP'a giriş, otomatik belge gönderimi, kesin hukuki süre hesabı **yoktur** (ADR-08).
- Linux v1 kapsamı dışıdır; kod taşınabilirliği korunur.
- Bulut hesabı zorunlu değildir; senkronizasyon yoktur (ADR-01, ADR-07).
