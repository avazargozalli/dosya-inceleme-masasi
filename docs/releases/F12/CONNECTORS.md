# F12-T06 — dış bağlayıcılar (sicil) ve toplu içe alma

Tarih: 2026-10-24 · Durum: **toplu içe alma kodlandı; dış bağlayıcılar belgelendi, açılmadı**

## 1. Karar: v1.0'da ağ bağlayıcısı yok

Şartname bölüm 14.4, resmî web kaynaklarına bağlayıcıyı v1.0 kapsamına almaz:
her bağlayıcı için **erişim yöntemi, kullanım koşulları, veri kapsamı, hız
sınırı ve yeniden kullanım değerlendirmesi** önce yazılı olmalıdır. Kimlik
doğrulama, CAPTCHA veya erişim engeli aşılmaz; belgelendirilmemiş uç nokta
ürün garantisine dönüştürülmez. Bu nedenle üründe **ağ isteği gönderen hiçbir
kod yolu yoktur**.

| Kaynak | İşletici | Erişim | Kapsam | Yeniden kullanım notu | Durum |
|---|---|---|---|---|---|
| Yargıtay karar arama | T.C. Yargıtay Başkanlığı | Resmî web arayüzü (kullanıcı tarayıcısı) | İçerik/künye alanlarıyla aranan kararların tam metni | Toplu indirme ve kalıcı yeniden yayımlama yetkisi doğrulanmadı | `documented_not_enabled` |
| Danıştay karar arama | T.C. Danıştay Başkanlığı | Resmî web arayüzü | Karar arama arayüzü | Belgelendirilmiş veri servisi yok; kazıma yapılmaz | `documented_not_enabled` |
| AYM Kararlar Bilgi Bankası | T.C. Anayasa Mahkemesi | Resmî web arayüzü | Bireysel başvuru ve norm denetimi kararları | Yeniden kullanım koşulları ayrıca değerlendirilmeden otomatik içe alma açılmaz | `documented_not_enabled` |

Sicil kodda `crates/storage/src/connectors.rs` içinde **gömülüdür** ve
`connector_catalog_hash()` ile içerik özeti taşır: kayıt sessizce değişirse
özet değişir. Her satırda `network_allowed=false`; arayüzde "etkin değil"
rozetiyle, gerekçesiyle gösterilir. İleride bir bağlayıcının açılması
F18 kapsam kararı + kendi görev/test planı ister.

**Sınırlama (dürüst):** bu turda hiçbir kaynaktan veri çekilmedi, çekilemez
de. Toplu içe alma yalnız **kullanıcının kendi indirdiği** dosyalarla çalışır.

## 2. Toplu içe alma

Arayüz: **Emsal ve atıf kontrolü → Toplu içe alma** kartı.

- **Dosya seçimi:** çoklu seçim (PDF, DOCX, UDF, TXT/MD). Yol **mutlak**
  olmalı; göreli yol `E_BULK_PATH` ile reddedilir. Tek turda en çok **200**
  dosya (fazlası sessizce kırpılmaz, `E_BULK_TOO_MANY`).
- **Metin çıkarma:** belge worker'ı (`extract_text`), OCR kapalı, dil `tur`.
  Görüntü dosyası (PNG/JPEG/TIFF) `E_BULK_IMAGE` ile reddedilir: görüntüden
  karar metni çıkarılmaz, OCR'li PDF gerekir. Metin çıkmazsa dosya
  **yazılmaz** (`E_BULK_EMPTY_TEXT`) — boş nüsha kütüphaneye girmez.
- **Künye listesi (CSV, isteğe bağlı):** sütunlar `dosya, mahkeme, daire,
  esas, karar, tarih, başlık, kaynak url, etiketler, konu`. `dosya` zorunlu;
  bilinmeyen sütun adı `E_MANIFEST` ile reddedilir (yanlış başlıkla içe alma
  fark edilmesin diye). Tırnaklı alan, kaçırılmış tırnak (`""`) ve alan içi
  satır sonu desteklenir. Aynı dosya adı iki kez geçerse liste reddedilir:
  hangi satırın geçerli olduğu **tahmin edilmez**.
- **Künye tamamlama yok:** liste veya varsayılanlarda mahkeme yoksa dosya
  `E_BULK_COURT` ile atlanır; esas/karar numarası metinden **çıkarılmaz**.
  Eksik künye `incomplete` kalır.
- **Nüsha karşılaştırma (şema v14):**
  - aynı künye + aynı SHA-256 → `duplicate`, yeni sürüm **yazılmaz**;
  - aynı künye + farklı metin → `conflict`: nüsha ayrı sürüm olarak eklenir,
    `identity_conflict=1` ve `conflict_with_version_id` ile işaretlenir,
    arayüzde "aynı künye, farklı metin — inceleyin" rozeti çıkar. Sessiz
    birleştirme yok, eski nüsha silinmez;
  - farklı mahkeme/daire aynı numarayı kullansa bile **ayrı emsal**;
  - numara (esas/karar/başvuru) yoksa aynı metin bile birleştirilmez.
- **Hata izolasyonu:** bir dosyanın hatası diğerlerini durdurmaz; rapor her
  dosya için `created | duplicate | conflict | error` + kod/mesaj döner.
- **Denetim:** her nüsha için `precedent_version`/`create` kaydı; yalnız hash
  ve karakter sayısı yazılır, **karar metni denetime yazılmaz**.

Komutlar: `precedent_bulk_import`, `precedent_connector_list`.
Çekirdek: `crates/storage/src/precedent_import.rs` (+ `connectors.rs`).

## 3. Doğrulama

- Rust: `cargo test --workspace --all-targets` → storage **189** (9 yeni
  `precedent_import` + 2 yeni `connectors`), toplam **292** yeşil.
  `cargo clippy --workspace --all-targets -D warnings` ve Tauri manifesti
  temiz; `cargo fmt --all` + Tauri manifesti temiz.
- Web: `pnpm typecheck`, `pnpm test` → **158** (5 yeni `precedents.test.ts`),
  `pnpm build` yeşil.
- Not: `upgrade_tests::newer_schema_is_refused_before_any_write` "gelecek
  sürüm" olarak sabit 14 yazıyordu; v14 gerçek migrasyon olunca test
  `CURRENT_SCHEMA_VERSION + 1` kullanacak şekilde düzeltildi (kapı
  gevşetilmedi, ayrıca v14 sütununun da yazılmadığı doğrulanıyor).

## 4. Açık kalanlar

- **Gerçek abonelik/hesap yok:** hiçbir resmî kaynaktan indirme denenmedi;
  bağlayıcılar bu yüzden "belgelendi, etkin değil".
- **Görsel QA yok:** Tauri penceresi bu ortamda açılmadı; kart, seçim ve
  rapor tip denetimi + birim testleri + üretim derlemesiyle doğrulandı.
- **Büyük partiler:** metin çıkarma sıralıdır (tek worker); 200 dosyalık parti
  dakikalar sürebilir. Komut senkron çalışır, arayüz "İçe alınıyor…" gösterir.
- **Künye listesi biçimi:** yalnız virgüllü CSV; noktalı virgül ayırıcı ve
  Excel'in `;` varsayılanı desteklenmez (açık hata verir, sessiz yanlış
  eşleme yapmaz).
