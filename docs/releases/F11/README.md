# F11 — Kaynaklı AI inceleme araçları

- **Sürüm etiketi:** `v0.10.0`
- **Durum:** Kısmi
- **Tarih:** 2026-10-24

| İş | Durum | Kanıt |
|---|---|---|
| F11-T01 aday kaydı | ✅ | `ai_candidates`, `valid/invalid/partial`; altı aday türü şeması |
| F11-T02 aday → inceleme kuyruğu | ✅ | `promote_candidate`, `review_items` `pending`, doğrulanmış alıntı → `source_anchors` |
| F11-T03 kapsamlılık raporu | ✅ | belge/sayfa/OCR/boş sayfa/ayrıştırılmamış sayıları |
| F11-T04 gerçek model çağrısı | ⬜ | sağlayıcı yok |
| F11-T05 şema doğrulama + alıntı eşleştirme | ✅ | `crates/storage/src/ai_candidate.rs`, şema v17, `ai_candidate_validate` |
| F11-T06 kapsamlılık raporunun rapora gömülmesi | ⬜ | — |

## Dürüst durum

AI çıktısı **adaydır**; avukat kabul etmeden hukuki kayıt oluşmaz. Model çağrısı
yapılmadığı için gerçek çıkarım kalitesi ölçülmedi.

## F11-T05 — şema doğrulama ve alıntı eşleştirme (2026-10-24)

**Doğrulama durumu artık hesaplanır.** `record_ai_candidate` çağıranın verdiği
durumu kabul etmez; yükü türüne göre doğrular, alıntıları belge metninde arar ve
`valid | partial | invalid` sonucunu kendisi yazar. Arayüzden “geçerli” demenin
yolu yoktur.

- **Altı şema** (`event`, `person`, `date`, `claim`, `summary`, `answer`;
  `*_candidate` adları eski kayıtlarla uyum için kabul edilir):
  - ortak: `schemaVersion` (yalnız `1`), `quotes[]` (en çok 20; her biri
    `documentVersionId` + `quote`, isteğe bağlı `pageIndex`/`label`);
  - `event`: `title` (1–200) zorunlu; `date`, `participants` (≤20), `description`;
  - `person`: `name` zorunlu; `role` (`davaci|davali|vekil|tanik|hakim|bilirkisi|diger|unknown`), `organization`;
  - `date`: `rawText` + `kind` (`document|event|notification|hearing|unknown`)
    zorunlu; `isoDate` **gerçek bir gün** olmalı (`YYYY-AA-GG`, artık yıl
    denetimi), `precision` isteğe bağlı — yaklaşık tarih `rawText`te kalır;
  - `claim`: `text` zorunlu; `assertedBy`, `amountText` (tutar **özgün metin**
    olarak, sayıya çevrilmez);
  - `summary`: `text` zorunlu **ve en az bir alıntı** (F11-T07 dayanağı);
  - `answer`: `text` zorunlu; `question` isteğe bağlı.
- **Sorun kodları:** `invalid_json`, `not_object`, `missing_field`,
  `unknown_field` (uyarı), `wrong_type`, `empty_text`, `too_long`,
  `too_many_items`, `bad_enum`, `bad_date`, `control_chars`,
  `unknown_candidate_type`, `schema_version_unsupported`,
  `quote_missing_field`, `quote_too_short` (< 8 karakter), `duplicate_quote`.
  `error` adayı geçersiz kılar; `warning` yalnız `partial` yapar.
- **Alıntı eşleştirmesi:** her alıntı, belge sürümünün **en yeni**
  `succeeded/partial` ayrıştırmasının sayfalarında `dim_evidence::verify_quote`
  ile aranır. Modelin `startHint`i **kullanılmaz** (konum güvenilir değil).
  - `exact` (tek eşleşme, sayfa + karakter konumu), `normalized` (yalnız
    normalizasyonla), `ambiguous` (birden fazla yer; en çok 5 konum),
    `page_mismatch` (bulundu ama modelin dediği sayfada değil),
    `not_found`, `unverifiable` (`version_not_found`, `case_mismatch`,
    `no_text`).
  - **Başka dosyanın belgesi** kaynak olamaz: `case_mismatch` → `failed`.
  - Ayrıştırılmamış belge “doğrulandı” sayılmaz; `no_text` → `partial`.
- **Kapılar:** `quote_state=failed` aday `E_AI_QUOTE` ile, `validation_state=invalid`
  aday `E_AI_SCHEMA` ile inceleme kuyruğundan reddedilir. Alıntısı olmayan aday
  (`no_quotes`) kuyruğa girebilir ama `partial` işaretlidir.
- **Kaynağa bağlama:** terfi sırasında `exact` alıntılar için `source_anchors`
  kaydı oluşturulur (aynı segment/offset/alıntı varsa yeniden kullanılır) ve
  `review_items.source_anchor_ids` bu kimliklerle yazılır.
- **Yeniden doğrulama:** `revalidate_ai_candidate` güncel ayrıştırmaya göre
  sonucu tazeler; belge yeniden ayrıştırıldığında eski “geçerli” damgası kalmaz.
- **Şema v17** (`0017_ai_candidate_quotes.sql`): `schema_state`, `quote_state`,
  `quote_report`, `checked_at`. Bu sürümden önceki satırlar **`legacy`** olur —
  doğrulanmış gibi görünmez.
- **Denetim:** `ai_candidate`/`record|revalidate` kaydı yalnız tür, durum ve
  alıntı sayısı taşır; **yük metni ve alıntı içeriği denetime yazılmaz**.
- **Arayüz:** “Aday doğrula ve kaydet” kartı (tür seçimi, JSON taslağı,
  “Doğrula” / “Doğrula ve kaydet”), sonuçta şema/alıntı rozetleri, alıntı
  tablosu (durum, sayfa/karakter, tekrar, metin kaynağı), sorun listesi ve
  “Doğrulamanın sınırları”; aday listesinde şema + alıntı rozetleri,
  “Yeniden doğrula” ve engel gerekçesiyle kapalı “İncelemeye gönder”.

### Sınırlamalar (dürüst)

- **Model çağrısı yok**: aday yükü kullanıcı tarafından yapıştırılır; gerçek
  sağlayıcı çıktısıyla ölçüm yapılmadı.
- Eşleştirme **kasada ayrıştırılmış metinle** sınırlı: OCR yazım hatası veya
  taranmış sayfada metin olmaması eşleşmeyi etkiler; görüntü üzerinde
  doğrulama yok.
- **Maskeli bağlamın özgün kaynağa geri eşlenmesi (F11-T03) yapılmaz**; aday
  metni gönderildiği hâliyle saklanır, alıntı bu metinde aranır.
- Şema katıdır: tanımsız alan `warning`, yanlış tür/eksik zorunlu alan `error`.
  Model serbest metin döndürürse aday geçersiz olur — bu bilinçli bir kapıdır.
- `pageIndex` 0 tabanlıdır; arayüz 1 tabanlı gösterir. `isoDate` ile `rawText`
  arasındaki tutarlılık **denetlenmez** (Türkçe tarih ayrıştırma kapsam dışı).
- Tauri penceresi açılmadı → kartların görsel QA'sı yok.
