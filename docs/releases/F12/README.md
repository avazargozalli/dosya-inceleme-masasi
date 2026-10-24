# F12 — Emsal kütüphanesi ve atıf kontrolü

- **Sürüm etiketi:** `v0.11.0` (hedef)
- **Durum:** Devam ediyor
- **Tarih:** 2026-10-24

| İş | Durum | Kanıt |
|---|---|---|
| F12-T01 | ✅ | emsal künyesi, etiket, konu; eksik künye `incomplete` |
| F12-T02 | ✅ | kullanım notu: benzerlik / ayrışan nokta / kullanıcı sonucu |
| F12-T03 | ✅ | atıf kontrolü: künye ve alıntı boyutları ayrı |
| F12-T04 | ✅ | “bulunamadı” ile “uydurulmuş” ayrımı korunur; sahte hükmü yok |
| F12-T05 | ✅ | karar tam metni (şema v13) + metin içi alıntı doğrulama |
| F12-T06 | ✅ | toplu içe alma (CSV künye listesi, çakışma işareti) + dış bağlayıcı sicili (etkin değil) |

## F12-T06 — toplu içe alma ve dış bağlayıcı sicili

**Şema v14** (`0014_precedent_conflict.sql`): `precedent_versions.identity_conflict`
ve `conflict_with_version_id`; `(precedent_id, body_hash)` indeksi.

- `Database::import_precedent_copy(&PrecedentCopyInput)` — metin **çıkarılmış**
  olarak gelir; bu katman ağ, dosya okuma veya worker çağrısı yapmaz.
  Sonuç: `created` | `duplicate` | `conflict` (+ `needs_review`).
  Aynı künye + aynı SHA-256 yeni sürüm yazmaz; aynı künye + farklı metin
  nüshayı **ayrı sürüm** olarak ekler ve incelemeye işaretler (sessiz
  birleştirme yok). Kimlik: mahkeme + daire + esas/karar/başvuru numarası
  birebir; numara yoksa birleştirme yapılmaz, farklı mahkeme ayrı emsaldir.
- `parse_manifest` — bağımlılıksız RFC 4180'e yakın CSV: tırnaklı alan,
  kaçırılmış tırnak, alan içi satır sonu, BOM. Türkçe/İngilizce başlıklar;
  bilinmeyen sütun ve aynı dosya adının tekrarı `E_MANIFEST` ile reddedilir.
- `precedent_bulk_import` (Tauri): dosya seçimi → `dim_ingest::detect` →
  worker `extract_text` → künye (CSV satırı, yoksa varsayılan) → kasa yazımı.
  Mutlak yol, 200 dosya ve 250 MiB sınırları; görüntü dosyası reddi; boş
  metin yazılmaz. Bir dosyanın hatası diğerlerini durdurmaz.
- `connectors.rs` + `precedent_connector_list`: Yargıtay, Danıştay ve AYM için
  erişim yöntemi/kapsam/yeniden kullanım notu **belgelenir**, hiçbiri etkin
  değildir (`network_allowed=false`), sicil içerik özeti (sha256) taşır.
  Ayrıntı ve karar gerekçesi: `docs/releases/F12/CONNECTORS.md`.
- Arayüz: "Toplu içe alma" kartı (dosya seç, CSV seç, varsayılan künye, dosya
  başına sonuç rozetleri, çakışma uyarısı) ve "Dış bağlayıcılar" kartı
  (kaynak başına "etkin değil" + gerekçe + koşullar bağlantısı).
  Sürüm listesinde çakışan nüsha "aynı künye, farklı metin" rozetiyle görünür.

## F12-T05 — karar tam metni ve metin içi alıntı doğrulama

**Şema v13** (`0013_precedent_text.sql`): `precedent_versions.full_text`
(şifreli veritabanında) ve `text_char_count`; `precedent_id, acquired_at DESC`
indeksi.

- `add_precedent_text(precedent_id, {sourceKind, text, documentVersionId})`:
  kaynak türü `paste` | `document` | `file` ile sınırlı; verilen
  `document_version_id` kasadaki gerçek sürüm olmalı (`E_NOT_FOUND`). Boş metin
  ve bilinmeyen kaynak türü `E_DB_SETTING`. `body_hash` = SHA-256(tam metin),
  `verification_state='text_verified'`, denetime **metin yazılmaz** (yalnız
  hash + karakter sayısı).
- `precedent_versions` / `get_precedent_version`: metin gövdesi **dönmez**;
  yalnız `has_text`, `charCount`, `bodyHash`, `sourceKind`, tarih.
- `verify_precedent_quote(precedent_id, quote)`: kararın **en yeni** tam metnine
  karşı üç aşamalı doğrulama —
  1. birebir (`method='exact'`, kaynak metindeki **karakter** konumu),
  2. yalnız satır/boşluk farkı (`method='whitespace'`),
  3. Türkçe harf sadeleştirmesi (`method='turkish_fold'`, OCR/klavye farkı).
  Eşleşme sayısı (`occurrences`) ayrıca döner. Sonuç `match` | `mismatch` |
  `not_checked`; `not_checked` nedenleri `no_full_text`, `quote_too_short`
  (< 8 karakter — yanlış kesinlik üretmemek için), `mismatch` nedeni `not_found`.
- `normalize_with_map`, `dim_evidence::normalize` ile **aynı** sadeleştirmeyi
  yapar ama konum eşlemesi taşır; ikisinin ayrışmasını
  `fold_matches_evidence_normalize` testi yakalar.
- Tauri: `precedent_text_add`, `precedent_version_list`,
  `precedent_quote_verify`.
- Arayüz (`Precedents.tsx` → “Tam metin ve alıntı doğrulama” kartı): sürüm
  listesi (kaynak türü, tarih, sha256 kısa, `tam metin · N karakter` /
  `tam metin yok`), tam metin yapıştırma, alıntı doğrulama; sonuç dürüst metinle
  gösterilir (“birebir bulundu (konum 1.234)” / “yalnız satır/boşluk farkıyla —
  birebir değil” / “bulunamadı … uydurulmuş demek değildir” / “tam metin yok;
  doğrulanamadı”). Sonuç, tek düğmeyle atıf kontrolü formunun `quote_match`
  alanına taşınır; künye ve anlam boyutları ayrı kalır.
- `apps/desktop/src/lib/precedents.ts`: durum metinleri/tonları saf fonksiyon
  olarak; `precedents.test.ts` 11 test.

## Dürüst durum

Karar **tam metni** yoksa alıntı doğrulanamaz; kontrol `not_checked` olarak
kalır ve “uydurulmuş” anlamına gelmez. Emsal notu hukuki uygunluk
değerlendirmesidir; sistem hükmü değildir.

## Sınırlamalar

- Tam metin **yapıştırma** veya **toplu içe alma** (dosyadan, `source_kind=file`)
  ile girer. Kasadaki belge sürümünden aktarma (`source_kind=document`) veri
  modelinde hazır; ayrı düğmesi yok (toplu içe almada dosya yolu kullanılıyor).
- Doğrulama yalnız metin eşleşmesidir; **anlam/bağlam** incelemesi ayrı boyut
  olarak kalır (F12-T07).
- Sadeleştirme Türkçe harf katlamasıyla sınırlı; kelime değişikliği, çeviri
  veya özet metin eşleşmez — bu durumda sonuç `mismatch` (“bulunamadı”) olur,
  “uydurulmuş” değil.
- Tauri penceresi bu ortamda açılmadı: kart, yapıştırma ve doğrulama akışı
  typecheck + birim testleri + üretim derlemesiyle doğrulandı; gerçek tıklama ve
  görsel QA yok.

## Doğrulama

- Rust: `cargo test -p dim-storage precedents::` → **14/14** (8 yeni);
  `cargo test --workspace --all-targets` → **256** (storage 169); clippy
  workspace + tauri `-D warnings`; rustfmt iki manifest.
- Web: `pnpm typecheck`, `pnpm test` → **146** (11 yeni), `pnpm build` yeşil.
- CI: bu commit ile doğrulanacak (aşağıdaki kayıt güncellenir).
