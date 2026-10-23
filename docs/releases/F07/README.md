# F07 — Arama ve belge okuyucu

- **Sürüm etiketi:** `v0.6.0`
- **Durum:** Devam ediyor (aşağıdaki açık işler)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F07-T01 | ✅ | FTS5 araması; dosya kapsamı zorunlu (`case_id` her sorguda) |
| F07-T02 | ⏳ | tam/esnek/tırnaklı ifade ve E./K. numarası seçenekleri açık |
| F07-T03 | ✅ | belge türü, tarih rolü, etiket ve OCR durumu filtreleri |
| F07-T04 | ✅ | sonuç alıntısı `raw_text`ten; vurgu metin düğümüyle (XSS yüzeyi yok) |
| F07-T05 | ✅ | keyset sayfalama, 200 sonuç sınırı, kapsam görünürlüğü |
| F07-T06 | ⏳ | PDF okuyucuda döndürme gösterilir; yakınlaştırma ve sayfa etiketi ayrımı açık |
| F07-T07 | ⏳ | DOCX/UDF yapısal görünüm açık |

## F07-T03 — arama filtreleri

- `SearchFilters`: `documentTypes`, `dateRole`, `tags`, `ocrStatus`.
- **Belge türü**: `documents.document_type`; birden fazlaysa VEYA.
- **OCR durumu**: `pages.text_origin IN ('ocr','mixed')` ya da `= 'native'`.
  `ocr_used` diye ayrı bir kolon yok; sayfanın gerçek metin kaynağı kullanılır.
- **Etiket**: `document_tags` (şema v10, `0010_document_tags.sql`). Etiket
  belgeye bağlıdır, sürüme değil. Kanonik biçim `normalize_tag`: boşluk
  sadeleşir, küçük harfe iner, Türkçe `İ` küçültmesinin bıraktığı birleşik
  nokta atılır, 40 karakter sınırı. Birden fazla etiket **VE**: hepsi aynı
  belgede olmalı. Etiket ekleme idempotent; her değişiklik denetim kaydı yazar.
- **Tarih rolü**: belgeye **kaynaklı** olayın `date_role` değeri. Bağ
  `events → event_sources → source_anchors → document_versions` üzerinden
  kurulur; belgeye bağlanmamış bir olay belgeyi sonuca sokmaz.
- Bilinmeyen filtre değeri sessizce yok sayılmaz: `E_SEARCH_FILTER`.
- Arayüz: Arama ekranında filtre çubuğu, etkin filtre rozetleri ve "Filtreler
  değişti" uyarısı (eski sonuç yeni filtreyle gösterilmez). Etiketler Dosyalar
  ekranındaki belge satırından eklenir/kaldırılır.

## F07-T05 — keyset sayfalama

- `search_segments_page(case, query, filters, limit, cursor, include_total)`.
- Sıra `bm25` (küçük = daha iyi), eşitlikte `segment_id`. İmleç son sonucun
  `f64` **bitleri** + segment kimliğidir; ondalık metne çevrilmez, eşit
  skorlarda sayfa kaymaz. Bozuk/yabancı imleç `E_SEARCH_CURSOR`.
- `limit` 1–200 arasına kırpılır; bir fazla satır çekilerek `has_more` gerçek
  veriden üretilir. `total` yalnız ilk sayfada istenir.
- Her sonuç kapsam üst verisini taşır: `caseId`, `documentType`,
  `documentStatus`, `textOrigin`, `tags`. Böylece liste büyürken sonucun hangi
  dosyaya/belgeye ait olduğu görünür kalır.
- Arayüz: "Daha fazla sonuç" yalnız sunucunun imleciyle çağrılır; aynı segment
  iki kez gösterilmez.

## Doğrulama

- `cargo test -p dim-storage --lib` → 125 test (8 yeni: 5 arama, 3 etiket).
- `cargo test --workspace` → storage 125, ingest 12 + E2E 1, vault 15,
  domain 13, jobs 5, evidence 7, policy 7, providers 5, cli-adapters 7.
- Web: `pnpm --filter @dim/desktop typecheck` + `vitest` **73** (13 yeni) +
  `vite build`.
- CI: bu turdaki commit sonrası doğrulanır.

## Bilinen sınırlar

- FTS tokenizer `unicode61 remove_diacritics 2`; Türkçe `ı/i` için özel
  tokenizer F07-T02'de değerlendirilecek.
- Tarih **aralığı** filtresi yok; yalnız tarih rolü var (planda rol/aralık
  birlikte anılır, aralık açık iş).
- Filtre çubuğu belge türünde tek seçim sunar; çekirdek API çoklu türü
  destekler ve test eder.
- Tauri penceresi bu ortamda açılmadı; ekranlar typecheck, birim testleri ve
  üretim derlemesiyle doğrulandı — gerçek tıklama/görsel QA yok.
