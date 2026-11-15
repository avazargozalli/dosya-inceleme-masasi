//! Dosya kapsamı **veritabanı düzeyinde** (şema v31 + v32 + v33 + v34): tetikleyiciler
//! ve mevcut satır taraması.
//!
//! Neden: çekirdek yazma yolları (`add_version_with_parent`, `start_parse_run`,
//! `store_pages`, `create_anchor`, `create_claim`, `link_evidence`,
//! `record_context_items`, …) kapsamı kendi sorgularında doğrular, ama **doğrudan
//! SQL** yazan bir yol bu kapıyı atlar. Kapsam ihlali P0 kabul engelidir: A
//! dosyasının metni B dosyasının aramasında bulunur. Bu yüzden kural artık şemada
//! da durur (`migrations/0031_scope_guards.sql` zincir,
//! `migrations/0032_reference_scope_guards.sql` kanıt/iddia referansları,
//! `migrations/0033_json_reference_scope_guards.sql` JSON dizi kanıt listeleri,
//! `migrations/0034_remaining_reference_scope_guards.sql` soy ağacı, maskeleme
//! çıktısı ve gönderim sicili).
//!
//! Dört sınıf kapı vardır:
//!
//! * **zincir** (v31) — `case_id` taşıyan satırın üst kaydı aynı dosyada olmalı
//!   (belge → sürüm → koşu → segment → çapa).
//! * **referans** (v32) — bir satırın **işaret ettiği** kanıt/iddia kendi
//!   dosyasından olmalı (`claims.source_anchor_id`, `evidence_links`,
//!   `event_sources`, `claim_relations`, `tasks.source_anchor_id`). Aksi hâlde
//!   B dosyasındaki belge silinince A'nın kanıt bağı sessizce yok olur ve
//!   kimlikle okuyan yollar (`anchor_locate`) A dosyasında B'nin metnini
//!   gösterir.
//! * **JSON dizi referansı** (v33) — kanıt listesi taşıyan dört sütun
//!   (`notes`, `note_revisions`, `review_items`, `note_drafts` →
//!   `source_anchor_ids`). Her eleman kendi dosyasının çapası olmalı;
//!   `note_drafts` bilinçli olarak **daha gevşektir** (var olmayan kimlik
//!   kalabilir — taslak metni kurtarır — ama başka dosyanın çapası giremez).
//! * **kalan referanslar** (v34) — soy ağacı (`document_versions.parent_version_id`),
//!   maskeleme çıktısı (`redaction_plans.output_version_id`) ve gönderim sicili
//!   (`ai_context_items.document_version_id` / `source_anchor_id` / `segment_ids`).
//!   Sicil, `case_id` taşımadığı için dosyayı **üst kaydından** (`ai_runs`) okur;
//!   0033'ün taslak kuralıyla aynı gevşeklikte: başka dosyada **var olan** bağ
//!   reddedilir, var olmayan kimlik denetim izinde kalabilir.
//! * **güncel sürüm işaretçisi** (v35) — `documents.current_version_id` kendi
//!   belgesinin sürümünü göstermeli. Bu işaretçi her okuma yolunun çıkış
//!   noktasıdır: kapsam raporu, alıntı denetimi ve AI gövdesi “güncel sürüm”ü
//!   buradan çözer. Tek bir doğrudan `UPDATE` belgeyi başka dosyanın sürümüne
//!   çevirebiliyordu (ölçüldü: A dosyasının alıntısı B dosyasının metninde
//!   `exact` döndü). Aynı değişmez ters yönden de korunur: güncel sürüm
//!   işaretçisi olan satır başka belgeye taşınamaz.
//! * **okuma görünümü** (v36) — güncel sürüm işaretçisi çaprazsa belge
//!   *sürümsüz* görünür; yedi okuma yolu tek tanımdan
//!   (`current_document_versions`) geçer, böylece eski bozuk satır yanlış veri
//!   değil “veri yok” üretir.
//! * **sürüm izi sicilleri** (v37) — `citation_checks.input_document_version_id`
//!   ve `export_items.source_version_id`: bir sicil satırının gösterdiği belge
//!   sürümü **kendi dosyasından** olmalı. İkisi de `case_id` taşımıyordu;
//!   atıf kontrolü çekirdekte de kapısızdı (ölçüldü: A'nın çekirdeğinden
//!   çıkarılmış metin B dosyasının ekranında göründü), dışa aktarma kalemi ise
//!   yalnız çekirdekte doğrulanıyordu (doğrudan SQL A'nın kaydında B'nin belge
//!   adını gösterdi). `citation_checks` kendi `case_id`'sini taşır;
//!   `export_items` dosyayı `exports` satırından okur.
//! * **giden segment kimliği** (v38) — `transfer_segments` satırı "V sürümünün
//!   S segmenti gönderildi" der; S **var olan** bir segment ise V'nin segmenti
//!   olmalı (sürüm `NULL` ise çalıştırmanın dosyasında). 0034 yalnız sürümü
//!   kapılıyordu; segment kimliği serbestti (ölçüldü: A'nın gönderim sicili B
//!   dosyasının segmentini gösterdi). Var olmayan segment kimliği bilinçli
//!   olarak serbesttir (denetim izi; segment sonradan silinebilir).
//! * **aktarım onayının belge listesi** (v39) — `transfer_consents`
//!   (`case_scope` + `selected_document_ids`) satırı "şu belgeler için aktarım
//!   izni verildi" der; cihazdan ne çıkabileceğinin tek yetki kaydıdır. Dizinin
//!   her elemanı kendi dosyasında **silinmemiş** bir belge olmalı, liste boş
//!   olamaz. Ölçüldü (v38): doğrudan SQL ile A'nın onayına B'nin belgesi
//!   yazılıyor ve envanter onu A'nın onaylanmış kapsamı diye sayıyordu; bozuk
//!   JSON sessizce "0 belge" okunuyordu. Gönderim yolu çapraz kimliği zaten
//!   atlar (bayt çıkmaz) — kusur yetki **kaydındaydı**.
//!
//! Bu modül iki şeyi taşır:
//!
//! * `scope_violations()` — **bu şema sürümünden önce** yazılmış aykırı
//!   satırların sayısı (tetikleyiciler yalnız yeni yazmayı engeller; eski veri
//!   kendiliğinden düzelmez). Kimlik/İçerik taşımaz; yalnız sayı.
//! * Kapı testleri — her kapsam sütunu taşıyan tablonun tetikleyicisi **var mı**
//!   (şemadan türetilir; yeni tablo ya da yeni referans sütunu sessizce
//!   kapısız kalamaz).

#[cfg(test)]
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{Database, StorageError};

/// Kapsam zincirinin veritabanı düzeyinde kapılı tabloları. Şemadan türetilen
/// test bu liste ile veritabanının gerçeğini karşılaştırır.
///
/// İlk sekizi **zincir** kapısıdır (v31): satırın `case_id`'si üst kaydının
/// dosyasıyla aynı olmalı. Sonraki beşi **referans** kapısıdır (v32): satırın
/// işaret ettiği kanıt/iddia kendi dosyasından olmalı. Sonraki dördü **JSON dizi
/// referansı** kapısıdır (v33): `source_anchor_ids` dizisinin her elemanı. Son
/// gelen **kalan referans** kapısıdır (v34): soy ağacı, maskeleme çıktısı ve
/// gönderim sicili (`ai_context_items` — kendi `case_id`'si yoktur, dosyayı
/// `ai_runs`tan okur). Sonraki **güncel sürüm işaretçisi**dir (v35):
/// `documents.current_version_id` kendi belgesinin sürümünü gösterir. Son iki
/// **sürüm izi sicili**dir (v37): `citation_checks` (kendi `case_id`'si) ve
/// `export_items` (dosyayı `exports`tan okur). Son gelen **aktarım onayının
/// belge listesi**dir (v39): `transfer_consents` kapsam sütununu `case_scope`
/// adıyla taşır; `selected_document_ids` dizisinin her elemanı kendi
/// dosyasında silinmemiş bir belge olmalı.
#[cfg(test)]
pub(crate) const GUARDED_TABLES: [&str; 23] = [
    // zincir (v31)
    "document_versions",
    "parse_runs",
    "text_segments",
    "source_anchors",
    "source_marks",
    "redaction_plans",
    "document_tags",
    "embeddings",
    // referans (v32)
    "claims",
    "evidence_links",
    "event_sources",
    "claim_relations",
    "tasks",
    // JSON dizi referansı (v33)
    "notes",
    "note_revisions",
    "review_items",
    "note_drafts",
    // kalan referans (v34)
    "ai_context_items",
    "transfer_segments",
    // güncel sürüm işaretçisi (v35)
    "documents",
    // sürüm izi sicilleri (v37)
    "citation_checks",
    "export_items",
    // aktarım onayının belge listesi (v39)
    "transfer_consents",
];

/// Bir tablodaki aykırı satır sayısı. Alanlar tablo adıyla aynıdır ki rapor
/// hangi zincirin koptuğunu söylesin.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeViolationCounts {
    pub document_versions: i64,
    pub parse_runs: i64,
    pub text_segments: i64,
    pub source_anchors: i64,
    pub source_marks: i64,
    pub redaction_plans: i64,
    pub document_tags: i64,
    pub embeddings: i64,
    pub claims: i64,
    pub evidence_links: i64,
    pub event_sources: i64,
    pub claim_relations: i64,
    pub tasks: i64,
    pub notes: i64,
    pub note_revisions: i64,
    pub review_items: i64,
    pub note_drafts: i64,
    pub ai_context_items: i64,
    pub transfer_segments: i64,
    /// Güncel sürüm işaretçisi başka belgeyi gösteren belge sayısı (v35).
    pub documents: i64,
    /// Atıf kontrolü satırı kendi dosyasının sürümünü göstermeyen (ya da
    /// dosyası yazılmamış, `case_id IS NULL`) satır sayısı (v37).
    pub citation_checks: i64,
    /// Dışa aktarma kalemi, üst kaydın dosyasından olmayan bir sürümü gösteren
    /// satır sayısı (v37).
    pub export_items: i64,
    /// Aktarım onayı, kendi dosyasında **var olmayan/silinmiş** ya da başka
    /// dosyaya ait bir belgeyi gösteren (ya da listesi boş/bozuk) satır sayısı
    /// (v39). Yalnız **kanıtlanabilir** uyuşmazlık sayılır: başka dosyada
    /// bulunan kimlik, boş liste, bozuk JSON. Sonradan silinmiş belge kimliği
    /// denetim izidir — aykırı sayılmaz (0038 ile aynı gevşeklik).
    pub transfer_consents: i64,
}

impl ScopeViolationCounts {
    /// Toplam aykırı satır. Sıfır değilse kasa, bu şema sürümünden önceki bir
    /// yoldan çapraz dosya satırı taşıyor demektir.
    pub fn total(&self) -> i64 {
        self.document_versions
            + self.parse_runs
            + self.text_segments
            + self.source_anchors
            + self.source_marks
            + self.redaction_plans
            + self.document_tags
            + self.embeddings
            + self.claims
            + self.evidence_links
            + self.event_sources
            + self.claim_relations
            + self.tasks
            + self.notes
            + self.note_revisions
            + self.review_items
            + self.note_drafts
            + self.ai_context_items
            + self.transfer_segments
            + self.documents
            + self.citation_checks
            + self.export_items
            + self.transfer_consents
    }
}

impl Database {
    /// Mevcut aykırı satırları **sayar** (silmez, düzeltmez). Tetikleyiciler
    /// yalnız yeni yazmayı engellediği için eski kasalarda kalmış olabilecek
    /// çapraz dosya satırları buradan görünür.
    pub fn scope_violations(&self) -> Result<ScopeViolationCounts, StorageError> {
        let conn = self.conn();
        let count = |sql: &str| -> Result<i64, StorageError> {
            Ok(conn.query_row(sql, [], |row| row.get(0))?)
        };
        Ok(ScopeViolationCounts {
            document_versions: count(
                "SELECT count(*) FROM document_versions dv
                  WHERE NOT EXISTS (SELECT 1 FROM documents d
                                     WHERE d.id = dv.document_id AND d.case_id = dv.case_id)
                     OR (dv.parent_version_id IS NOT NULL AND NOT EXISTS (
                           SELECT 1 FROM document_versions p
                            WHERE p.id = dv.parent_version_id AND p.case_id = dv.case_id))",
            )?,
            parse_runs: count(
                "SELECT count(*) FROM parse_runs pr
                  WHERE NOT EXISTS (SELECT 1 FROM document_versions dv
                                     WHERE dv.id = pr.document_version_id AND dv.case_id = pr.case_id)",
            )?,
            text_segments: count(
                "SELECT count(*) FROM text_segments s
                  WHERE NOT EXISTS (SELECT 1 FROM parse_runs pr
                                     WHERE pr.id = s.parse_run_id AND pr.case_id = s.case_id)
                     OR (s.page_id IS NOT NULL AND NOT EXISTS (
                           SELECT 1 FROM pages p
                            WHERE p.id = s.page_id AND p.parse_run_id = s.parse_run_id))",
            )?,
            source_anchors: count(
                "SELECT count(*) FROM source_anchors a
                  WHERE NOT EXISTS (SELECT 1 FROM document_versions dv
                                     WHERE dv.id = a.document_version_id AND dv.case_id = a.case_id)
                     OR NOT EXISTS (SELECT 1 FROM parse_runs pr
                                     WHERE pr.id = a.parse_run_id AND pr.case_id = a.case_id)
                     OR (a.page_id IS NOT NULL AND NOT EXISTS (
                           SELECT 1 FROM pages p
                            WHERE p.id = a.page_id AND p.parse_run_id = a.parse_run_id))
                     OR NOT EXISTS (SELECT 1 FROM text_segments s
                                     WHERE s.id = a.segment_id
                                       AND s.parse_run_id = a.parse_run_id
                                       AND s.case_id = a.case_id)",
            )?,
            source_marks: count(
                "SELECT count(*) FROM source_marks m
                  WHERE NOT EXISTS (SELECT 1 FROM document_versions dv
                                     WHERE dv.id = m.document_version_id AND dv.case_id = m.case_id)
                     OR (m.parse_run_id IS NOT NULL AND NOT EXISTS (
                           SELECT 1 FROM parse_runs pr
                            WHERE pr.id = m.parse_run_id AND pr.case_id = m.case_id))
                     OR (m.page_id IS NOT NULL AND NOT EXISTS (
                           SELECT 1 FROM pages p
                             JOIN parse_runs pr ON pr.id = p.parse_run_id
                            WHERE p.id = m.page_id AND pr.case_id = m.case_id))
                     OR (m.anchor_id IS NOT NULL AND NOT EXISTS (
                           SELECT 1 FROM source_anchors a
                            WHERE a.id = m.anchor_id AND a.case_id = m.case_id))",
            )?,
            redaction_plans: count(
                "SELECT count(*) FROM redaction_plans rp
                  WHERE NOT EXISTS (SELECT 1 FROM document_versions dv
                                     WHERE dv.id = rp.document_version_id AND dv.case_id = rp.case_id)
                     OR (rp.parse_run_id IS NOT NULL AND NOT EXISTS (
                           SELECT 1 FROM parse_runs pr
                            WHERE pr.id = rp.parse_run_id AND pr.case_id = rp.case_id))
                     OR (rp.output_version_id IS NOT NULL AND NOT EXISTS (
                           SELECT 1 FROM document_versions ov
                            WHERE ov.id = rp.output_version_id AND ov.case_id = rp.case_id))",
            )?,
            document_tags: count(
                "SELECT count(*) FROM document_tags t
                  WHERE NOT EXISTS (SELECT 1 FROM documents d
                                     WHERE d.id = t.document_id AND d.case_id = t.case_id)",
            )?,
            embeddings: count(
                "SELECT count(*) FROM embeddings e
                  WHERE NOT EXISTS (SELECT 1 FROM text_segments s
                                     WHERE s.id = e.segment_id AND s.case_id = e.case_id)",
            )?,
            claims: count(
                "SELECT count(*) FROM claims c
                  WHERE c.source_anchor_id IS NOT NULL AND NOT EXISTS (
                        SELECT 1 FROM source_anchors a
                         WHERE a.id = c.source_anchor_id AND a.case_id = c.case_id)",
            )?,
            evidence_links: count(
                "SELECT count(*) FROM evidence_links l
                  WHERE NOT EXISTS (SELECT 1 FROM claims c
                                     WHERE c.id = l.claim_id AND c.case_id = l.case_id)
                     OR NOT EXISTS (SELECT 1 FROM source_anchors a
                                     WHERE a.id = l.source_anchor_id AND a.case_id = l.case_id)",
            )?,
            event_sources: count(
                "SELECT count(*) FROM event_sources es
                  WHERE NOT EXISTS (SELECT 1 FROM events e
                                     WHERE e.id = es.event_id
                                       AND EXISTS (SELECT 1 FROM source_anchors a
                                                    WHERE a.id = es.source_anchor_id
                                                      AND a.case_id = e.case_id))",
            )?,
            claim_relations: count(
                "SELECT count(*) FROM claim_relations r
                  WHERE NOT EXISTS (SELECT 1 FROM claims c
                                     WHERE c.id = r.claim_id AND c.case_id = r.case_id)
                     OR NOT EXISTS (SELECT 1 FROM claims c
                                     WHERE c.id = r.related_claim_id AND c.case_id = r.case_id)",
            )?,
            tasks: count(
                "SELECT count(*) FROM tasks t
                  WHERE t.source_anchor_id IS NOT NULL AND NOT EXISTS (
                        SELECT 1 FROM source_anchors a
                         WHERE a.id = t.source_anchor_id AND a.case_id = t.case_id)",
            )?,
            // v33 — JSON dizi kanıt listeleri. Bozuk JSON da aykırı sayılır:
            // referans doğrulanamıyorsa "temiz" denemez.
            notes: count(
                "SELECT count(*) FROM notes n
                  WHERE NOT json_valid(n.source_anchor_ids)
                     OR EXISTS (
                          SELECT 1 FROM json_each(
                                 CASE WHEN json_valid(n.source_anchor_ids)
                                      THEN n.source_anchor_ids ELSE '[]' END) j
                           WHERE j.type <> 'text'
                              OR NOT EXISTS (SELECT 1 FROM source_anchors a
                                              WHERE a.id = j.value AND a.case_id = n.case_id))",
            )?,
            note_revisions: count(
                "SELECT count(*) FROM note_revisions r
                  WHERE NOT json_valid(r.source_anchor_ids)
                     OR NOT EXISTS (SELECT 1 FROM notes n WHERE n.id = r.note_id)
                     OR EXISTS (
                          SELECT 1 FROM json_each(
                                 CASE WHEN json_valid(r.source_anchor_ids)
                                      THEN r.source_anchor_ids ELSE '[]' END) j
                           WHERE j.type <> 'text'
                              OR NOT EXISTS (SELECT 1 FROM source_anchors a
                                               JOIN notes n ON n.id = r.note_id
                                              WHERE a.id = j.value AND a.case_id = n.case_id))",
            )?,
            review_items: count(
                "SELECT count(*) FROM review_items i
                  WHERE NOT json_valid(i.source_anchor_ids)
                     OR (i.ai_run_id IS NOT NULL AND EXISTS (
                           SELECT 1 FROM ai_runs r
                            WHERE r.id = i.ai_run_id AND r.case_id <> i.case_id))
                     OR EXISTS (
                          SELECT 1 FROM json_each(
                                 CASE WHEN json_valid(i.source_anchor_ids)
                                      THEN i.source_anchor_ids ELSE '[]' END) j
                           WHERE j.type <> 'text'
                              OR NOT EXISTS (SELECT 1 FROM source_anchors a
                                              WHERE a.id = j.value AND a.case_id = i.case_id))",
            )?,
            // Taslak bilinçli olarak yalnız **başka dosyada var olan** çapayı
            // aykırı sayar (var olmayan kimlik taslakta meşru: metni kurtarır).
            note_drafts: count(
                "SELECT count(*) FROM note_drafts d
                  WHERE NOT json_valid(d.source_anchor_ids)
                     OR (d.note_id <> '' AND EXISTS (
                           SELECT 1 FROM notes n
                            WHERE n.id = d.note_id AND n.case_id <> d.case_id))
                     OR EXISTS (
                          SELECT 1 FROM json_each(
                                 CASE WHEN json_valid(d.source_anchor_ids)
                                      THEN d.source_anchor_ids ELSE '[]' END) j
                            JOIN source_anchors a ON a.id = j.value
                           WHERE a.case_id <> d.case_id)",
            )?,
            // v34 — gönderim sicili. Dosya, üst kaydın (`ai_runs`) dosyasıdır.
            // Tarama yalnız **dosya sınırını** sayar (sürüm/çapa başka dosyada
            // *var*): var olmayan kimlik bu tabloda meşru bir denetim izidir
            // (belge gönderimden sonra silinmiş olabilir). Segment listesinin
            // sürüme aidiyeti çekirdek + tetikleyici kapısındadır.
            ai_context_items: count(
                "SELECT count(*) FROM ai_context_items c
                  WHERE EXISTS (
                          SELECT 1 FROM document_versions dv
                           WHERE dv.id = c.document_version_id
                             AND dv.case_id <> (SELECT r.case_id FROM ai_runs r
                                                 WHERE r.id = c.ai_run_id))
                     OR (c.source_anchor_id IS NOT NULL AND EXISTS (
                           SELECT 1 FROM source_anchors a
                            WHERE a.id = c.source_anchor_id
                              AND a.case_id <> (SELECT r.case_id FROM ai_runs r
                                                 WHERE r.id = c.ai_run_id)))",
            )?,
            // v34 + v38 — giden segment sicili (maskelenmiş metnin kendisi bu
            // tabloda): sürüm başka dosyadan **ya da** gösterilen segment başka
            // sürümün/dosyanın segmenti olamaz.
            transfer_segments: count(
                "SELECT count(*) FROM transfer_segments t
                  WHERE (t.document_version_id IS NOT NULL AND EXISTS (
                          SELECT 1 FROM document_versions dv
                           WHERE dv.id = t.document_version_id
                             AND dv.case_id <> (SELECT r.case_id FROM ai_runs r
                                                 WHERE r.id = t.ai_run_id)))
                     OR EXISTS (
                          SELECT 1
                            FROM text_segments s
                            JOIN parse_runs pr ON pr.id = s.parse_run_id
                           WHERE s.id IN (t.outbound_segment_id, t.source_segment_id)
                             AND ((t.document_version_id IS NOT NULL
                                   AND pr.document_version_id IS NOT t.document_version_id)
                               OR (t.document_version_id IS NULL
                                   AND pr.case_id IS NOT (SELECT r.case_id FROM ai_runs r
                                                           WHERE r.id = t.ai_run_id))))",
            )?,
            // v35 — belgenin güncel sürüm işaretçisi **başka belgeyi** gösteriyor.
            // (Sürümü silinmiş, boşta kalmış işaretçi çapraz okuma üretmez; bu
            // sayıma girmez — kapı yalnız yeni yazmayı engeller.)
            documents: count(
                "SELECT count(*) FROM documents d
                  WHERE d.current_version_id IS NOT NULL AND EXISTS (
                          SELECT 1 FROM document_versions dv
                           WHERE dv.id = d.current_version_id
                             AND dv.document_id <> d.id)",
            )?,
            // v37 — atıf kontrolü **kendi dosyasının** sürümünü gösterir.
            // Dosyası yazılmamış (`case_id IS NULL`) satır da sayılır: hangi
            // dosyaya ait olduğu kanıtlanamıyor (sürümü silinmiş eski satır).
            // Var **olmayan** sürüm kimliği sayılmaz: bu tabloda denetim izi
            // olarak meşrudur (kapı yalnız "başka dosyada var olan"ı reddeder).
            // Dürüst sınır: v37 öncesi **çapraz** satır göçle başvurduğu
            // sürümün dosyasına atfedildiği için burada görünmez.
            citation_checks: count(
                "SELECT count(*) FROM citation_checks c
                  WHERE c.case_id IS NULL OR EXISTS (
                          SELECT 1 FROM document_versions v
                            JOIN documents d ON d.id = v.document_id
                           WHERE v.id = c.input_document_version_id
                             AND d.case_id <> c.case_id)",
            )?,
            // v37 — dışa aktarma kalemi, **üst kaydın** dosyasının sürümünü
            // gösterir (satırın kendi `case_id`'si yok; 0034 `ai_context_items`
            // kuralıyla aynı; var olmayan sürüm iz sayılmaz).
            export_items: count(
                "SELECT count(*) FROM export_items ei
                  WHERE EXISTS (
                          SELECT 1 FROM exports e
                            JOIN document_versions v ON v.id = ei.source_version_id
                            JOIN documents d ON d.id = v.document_id
                           WHERE e.id = ei.export_id
                             AND e.case_id <> d.case_id)",
            )?,
            // v39 — aktarım onayının belge listesi. Yalnız kanıtlanabilir
            // uyuşmazlık sayılır: **başka dosyada var olan** belge kimliği,
            // boş liste, bozuk/array olmayan JSON. Sonradan silinmiş ya da
            // hiç var olmamış kimlik denetim izidir (gönderim yolu onu atlar);
            // aykırı sayılmaz — 0038'in `transfer_segments` kuralıyla aynı.
            transfer_consents: count(
                "SELECT count(*) FROM transfer_consents c
                  WHERE NOT json_valid(c.selected_document_ids)
                     OR json_type(c.selected_document_ids) <> 'array'
                     OR (CASE WHEN json_valid(c.selected_document_ids)
                                   AND json_type(c.selected_document_ids) = 'array'
                              THEN json_array_length(c.selected_document_ids) ELSE -1 END) = 0
                     OR EXISTS (
                          SELECT 1 FROM json_each(
                                 CASE WHEN json_valid(c.selected_document_ids)
                                      THEN c.selected_document_ids ELSE '[]' END) j
                           WHERE j.type <> 'text'
                              OR EXISTS (SELECT 1 FROM documents d
                                          WHERE d.id = j.value
                                            AND d.case_id <> c.case_scope))",
            )?,
        })
    }

    /// Bir tablonun kapsam tetikleyicilerinin adları. Yalnız test/denetim
    /// yüzeyi: üretim kodu tetikleyicileri adıyla çağırmaz.
    #[cfg(test)]
    pub(crate) fn scope_trigger_names(&self, table: &str) -> Result<Vec<String>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT name FROM sqlite_master WHERE type='trigger' AND tbl_name=?1 ORDER BY name",
        )?;
        let rows = stmt.query_map([table], |row| row.get::<_, String>(0))?;
        let mut names = Vec::new();
        for row in rows {
            names.push(row?);
        }
        Ok(names)
    }

    /// Tetikleyici gövdesi (şema metni). Test, gövdenin **doğru üst kaydı**
    /// denetlediğini arar; ada bakmak yetmez.
    #[cfg(test)]
    pub(crate) fn trigger_sql(&self, name: &str) -> Result<Option<String>, StorageError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='trigger' AND name=?1",
                [name],
                |row| row.get(0),
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseKey;

    fn db() -> Database {
        Database::open_in_memory(&DatabaseKey::new([31u8; 32])).expect("db")
    }

    /// Tutarlı bir delil zinciri kurar (ham SQL): iki dosya, bir belge, bir
    /// sürüm, iki ayrıştırma koşusu, sayfalar, segmentler ve bir çapa.
    ///
    /// `c1` zincirin sahibi; `c2` çapraz yazma denemelerinin hedefi.
    fn chain(db: &Database) {
        db.conn()
            .execute_batch(
                "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
                 VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                        ('c2','Diğer','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
                 INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
                 VALUES ('b1','aa11',1,10,'ref','2026-01-01T00:00:00Z');
                 INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
                 VALUES ('d1','c1','Kira sozlesmesi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                        ('d2','c1','Kopya','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
                 INSERT INTO document_versions
                   (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
                 VALUES ('v1','d1','c1',1,'b1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',10,'application/pdf','2026-01-01T00:00:00Z');
                 INSERT INTO parse_runs (id,document_version_id,case_id,engine,engine_version,settings_hash,status,created_at)
                 VALUES ('r1','v1','c1','pymupdf','1.24','h','succeeded','2026-01-01T00:00:00Z'),
                        ('r2','v1','c1','pymupdf','1.24','h2','succeeded','2026-01-01T00:00:00Z');
                 INSERT INTO pages (id,parse_run_id,page_index,text_origin)
                 VALUES ('p1','r1',0,'native'),('p2','r2',0,'native');
                 INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
                 VALUES ('s1','r1','p1','c1','itiraz','itiraz','2026-01-01T00:00:00Z'),
                        ('s2','r2','p2','c1','itiraz','itiraz','2026-01-01T00:00:00Z');
                 INSERT INTO source_anchors
                   (id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,
                    exact_quote,quote_hash,created_at,updated_at)
                 VALUES ('a1','c1','v1','r1','p1','s1',0,6,'itiraz',
                         'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                         '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
            )
            .expect("zincir");
    }

    fn count(db: &Database, table: &str) -> i64 {
        db.conn()
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .expect("sayım")
    }

    fn scope_error(result: Result<usize, rusqlite::Error>, table: &str) {
        match result.map_err(StorageError::from) {
            Err(StorageError::ScopeViolation { table: found }) => assert_eq!(found, table),
            other => panic!("beklenen E_DB_SCOPE ({table}), ölçülen: {other:?}"),
        }
    }

    /// `c2` dosyasında **kendi** belge zincirini ve iddiasını kurar. A dosyasının
    /// satırları bu dosyanın çapasına/iddiasına bağlanmaya çalışacak.
    ///
    /// `chain()` iki dosyayı da kurar ama `c2`de belge yoktur; kimlikler
    /// çakışmasın diye burada `9` son ekli kimlikler kullanılır.
    fn foreign_chain(db: &Database) {
        db.conn()
            .execute_batch(
                "INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
                 VALUES ('d9','c2','Diger dosya belgesi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
                 INSERT INTO document_versions
                   (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
                 VALUES ('v9','d9','c2',1,'b1','eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',10,'application/pdf','2026-01-01T00:00:00Z');
                 INSERT INTO parse_runs (id,document_version_id,case_id,engine,engine_version,settings_hash,status,created_at)
                 VALUES ('r9','v9','c2','pymupdf','1.24','h9','succeeded','2026-01-01T00:00:00Z');
                 INSERT INTO pages (id,parse_run_id,page_index,text_origin)
                 VALUES ('p9','r9',0,'native');
                 INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
                 VALUES ('s9','r9','p9','c2','gizli','gizli','2026-01-01T00:00:00Z');
                 INSERT INTO source_anchors
                   (id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,
                    exact_quote,quote_hash,created_at,updated_at)
                 VALUES ('a9','c2','v9','r9','p9','s9',0,5,'gizli',
                         'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
                         '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
            )
            .expect("yabancı zincir");
    }

    /// İki dosyanın iddiaları (k1 kanıtlı, k1b kanıtsız, k9 yabancı) + iki olay.
    fn claims_and_events(db: &Database) {
        db.conn()
            .execute_batch(
                "INSERT INTO claims (id,case_id,claim_text,claim_kind,status,source_anchor_id,created_at,updated_at)
                 VALUES ('k1','c1','Kiraci itiraz etti','allegation','draft','a1','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                        ('k1b','c1','Kiraci odemedi','allegation','draft',NULL,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                        ('k9','c2','Diger dosya iddiasi','allegation','draft','a9','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
                 INSERT INTO events (id,case_id,event_kind,date_precision,date_role,title,created_at,updated_at)
                 VALUES ('e1','c1','payment','day','event','Kira odendi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                        ('e9','c2','payment','day','event','Diger olay','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
            )
            .expect("iddialar ve olaylar");
    }

    #[test]
    fn a_version_cannot_be_written_into_another_case() {
        let db = db();
        chain(&db);
        let before = count(&db, "document_versions");
        let result = db.conn().execute(
            "INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
             VALUES ('vx','d1','c2',2,'b1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',10,'application/pdf','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(result, "document_versions");
        assert_eq!(
            count(&db, "document_versions"),
            before,
            "reddedilen yazma iz bırakmaz"
        );
    }

    #[test]
    fn a_parse_run_cannot_be_written_into_another_case() {
        let db = db();
        chain(&db);
        let result = db.conn().execute(
            "INSERT INTO parse_runs (id,document_version_id,case_id,engine,engine_version,settings_hash,status,created_at)
             VALUES ('rx','v1','c2','pymupdf','1.24','h3','queued','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(result, "parse_runs");
    }

    #[test]
    fn a_segment_cannot_be_written_into_another_case_or_onto_a_foreign_page() {
        let db = db();
        chain(&db);
        let cross_case = db.conn().execute(
            "INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
             VALUES ('sx','r1','p1','c2','metin','metin','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(cross_case, "text_segments");
        // Sayfa başka koşunun: aynı dosyada bile segment yanlış belgeye konumlanır.
        let foreign_page = db.conn().execute(
            "INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
             VALUES ('sy','r1','p2','c1','metin','metin','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(foreign_page, "text_segments");
        assert_eq!(count(&db, "text_segments"), 2, "yalnız meşru iki segment");
    }

    #[test]
    fn an_anchor_cannot_point_across_cases_or_runs() {
        let db = db();
        chain(&db);
        let cross_case = db.conn().execute(
            "INSERT INTO source_anchors
               (id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,
                exact_quote,quote_hash,created_at,updated_at)
             VALUES ('ax','c2','v1','r1','p1','s1',0,6,'itiraz',
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(cross_case, "source_anchors");
        // Segment başka koşunun (r2), çapa r1 diyor: bağ yanlış koşuya işaret eder.
        let cross_run = db.conn().execute(
            "INSERT INTO source_anchors
               (id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,
                exact_quote,quote_hash,created_at,updated_at)
             VALUES ('ay','c1','v1','r1','p1','s2',0,6,'itiraz',
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(cross_run, "source_anchors");
        // Sayfa başka koşunun (p2, r2), çapa r1 diyor: çapa yanlış belgenin
        // geometrisini gösterir (0031 `text_segments.page_id` kuralının ikizi).
        let foreign_page = db.conn().execute(
            "INSERT INTO source_anchors
               (id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,
                exact_quote,quote_hash,created_at,updated_at)
             VALUES ('az','c1','v1','r1','p2','s1',0,6,'itiraz',
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(foreign_page, "source_anchors");
    }

    #[test]
    fn a_mark_cannot_bind_another_cases_version_or_anchor() {
        let db = db();
        chain(&db);
        let cross_case = db.conn().execute(
            "INSERT INTO source_marks
               (id,case_id,document_version_id,parse_run_id,anchor_id,page_id,mark_kind,start_offset,end_offset,
                quoted_text,rects_json,label,created_at)
             VALUES ('mx','c2','v1','r1','a1','p1','text',0,6,'itiraz','[]','','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(cross_case, "source_marks");
    }

    #[test]
    fn a_redaction_plan_cannot_bind_another_cases_version() {
        let db = db();
        chain(&db);
        let result = db.conn().execute(
            "INSERT INTO redaction_plans
               (id,case_id,document_version_id,parse_run_id,render_hash,coordinate_space,plan_version,state,
                output_profile,created_at,updated_at)
             VALUES ('rpx','c2','v1','r1','h','page',1,'draft','pdf','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(result, "redaction_plans");
    }

    #[test]
    fn a_tag_and_an_embedding_cannot_cross_cases() {
        let db = db();
        chain(&db);
        let tag = db.conn().execute(
            "INSERT INTO document_tags (document_id,case_id,tag,created_at)
             VALUES ('d1','c2','kira','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(tag, "document_tags");
        let embedding = db.conn().execute(
            "INSERT INTO embeddings
               (id,case_id,segment_id,model_profile_id,model_revision,dimensions,vector_bytes,normalized,
                input_hash,created_at)
             VALUES ('ex','c2','s1','m','rev',3,x'000000','1',
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(embedding, "embeddings");
    }

    #[test]
    fn moving_a_row_across_cases_is_rejected() {
        let db = db();
        chain(&db);
        let moved_version = db.conn().execute(
            "UPDATE document_versions SET case_id='c2' WHERE id='v1'",
            [],
        );
        scope_error(moved_version, "document_versions");
        let moved_segment = db
            .conn()
            .execute("UPDATE text_segments SET page_id='p2' WHERE id='s1'", []);
        scope_error(moved_segment, "text_segments");
        // Satırlar yerinde: reddedilen UPDATE kısmi iz bırakmaz.
        let case_of: String = db
            .conn()
            .query_row(
                "SELECT case_id FROM document_versions WHERE id='v1'",
                [],
                |r| r.get(0),
            )
            .expect("sürüm");
        assert_eq!(case_of, "c1");
        let page_of: Option<String> = db
            .conn()
            .query_row("SELECT page_id FROM text_segments WHERE id='s1'", [], |r| {
                r.get(0)
            })
            .expect("segment");
        assert_eq!(page_of.as_deref(), Some("p1"));
    }

    /// İddianın kanıtı kendi dosyasından olmalı (v32). Bugünkü canlı kusur:
    /// çapraz kanıt bağı, **hedef dosyanın belgesi silinince** `ON DELETE
    /// CASCADE` ile sessizce yok oluyordu.
    #[test]
    fn a_claim_cannot_cite_another_cases_anchor() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        let before = count(&db, "claims");
        let cross = db.conn().execute(
            "INSERT INTO claims (id,case_id,claim_text,claim_kind,status,source_anchor_id,created_at,updated_at)
             VALUES ('kx','c1','Iddia','allegation','draft','a9','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(cross, "claims");
        assert_eq!(count(&db, "claims"), before, "reddedilen yazma iz bırakmaz");

        // Aynı dosyanın çapası ve kanıtsız iddia meşru.
        claims_and_events(&db);
        assert_eq!(count(&db, "claims"), before + 3);

        // Var olan iddia UPDATE ile başka dosyanın çapasına taşınamaz.
        let moved = db
            .conn()
            .execute("UPDATE claims SET source_anchor_id='a9' WHERE id='k1'", []);
        scope_error(moved, "claims");
        let anchor: Option<String> = db
            .conn()
            .query_row(
                "SELECT source_anchor_id FROM claims WHERE id='k1'",
                [],
                |r| r.get(0),
            )
            .expect("iddia");
        assert_eq!(
            anchor.as_deref(),
            Some("a1"),
            "reddedilen UPDATE satırı bırakır"
        );
    }

    /// Kanıt bağı: iddia **ve** çapa aynı dosyada olmalı.
    #[test]
    fn an_evidence_link_cannot_bind_another_cases_claim_or_anchor() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        claims_and_events(&db);

        let foreign_anchor = db.conn().execute(
            "INSERT INTO evidence_links (id,case_id,claim_id,source_anchor_id,relation,rationale,review_state,created_at,updated_at)
             VALUES ('elx','c1','k1','a9','supports_candidate','','accepted','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(foreign_anchor, "evidence_links");
        let foreign_claim = db.conn().execute(
            "INSERT INTO evidence_links (id,case_id,claim_id,source_anchor_id,relation,rationale,review_state,created_at,updated_at)
             VALUES ('ely','c1','k9','a1','supports_candidate','','accepted','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(foreign_claim, "evidence_links");
        assert_eq!(count(&db, "evidence_links"), 0, "hiçbir satır yazılmadı");

        // Meşru bağ: aynı dosyanın iddiası + aynı dosyanın çapası.
        db.conn()
            .execute(
                "INSERT INTO evidence_links (id,case_id,claim_id,source_anchor_id,relation,rationale,review_state,created_at,updated_at)
                 VALUES ('el1','c1','k1','a1','supports_candidate','','accepted','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                [],
            )
            .expect("aynı dosyada kanıt bağı yazılır");
        let moved = db.conn().execute(
            "UPDATE evidence_links SET source_anchor_id='a9' WHERE id='el1'",
            [],
        );
        scope_error(moved, "evidence_links");
    }

    /// Olay kaynağı: satırda `case_id` yok — olayın dosyası ile çapanın dosyası
    /// aynı olmalı.
    #[test]
    fn an_event_source_cannot_link_another_cases_anchor() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        claims_and_events(&db);
        let cross = db.conn().execute(
            "INSERT INTO event_sources (id,event_id,source_anchor_id,relation,user_confirmed)
             VALUES ('esx','e1','a9','supports',1)",
            [],
        );
        scope_error(cross, "event_sources");
        assert_eq!(count(&db, "event_sources"), 0);
        db.conn()
            .execute(
                "INSERT INTO event_sources (id,event_id,source_anchor_id,relation,user_confirmed)
                 VALUES ('es1','e1','a1','supports',1)",
                [],
            )
            .expect("aynı dosyada olay kaynağı yazılır");
        let moved = db.conn().execute(
            "UPDATE event_sources SET source_anchor_id='a9' WHERE id='es1'",
            [],
        );
        scope_error(moved, "event_sources");
    }

    /// İddia ilişkisi: iki iddia da satırın dosyasında olmalı.
    #[test]
    fn a_claim_relation_cannot_cross_cases() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        claims_and_events(&db);
        let cross = db.conn().execute(
            "INSERT INTO claim_relations (id,case_id,claim_id,related_claim_id,relation,rationale,created_at,updated_at)
             VALUES ('crx','c1','k1','k9','response_to','','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(cross, "claim_relations");
        let upside_down = db.conn().execute(
            "INSERT INTO claim_relations (id,case_id,claim_id,related_claim_id,relation,rationale,created_at,updated_at)
             VALUES ('cry','c1','k9','k1','response_to','','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(upside_down, "claim_relations");
        db.conn()
            .execute(
                "INSERT INTO claim_relations (id,case_id,claim_id,related_claim_id,relation,rationale,created_at,updated_at)
                 VALUES ('cr1','c1','k1','k1b','response_to','','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                [],
            )
            .expect("aynı dosyada ilişki yazılır");
        let moved = db.conn().execute(
            "UPDATE claim_relations SET related_claim_id='k9' WHERE id='cr1'",
            [],
        );
        scope_error(moved, "claim_relations");
    }

    /// Görev: `source_anchor_id` bugün hiç yazılmıyor (sütun ileriye dönük) ama
    /// yazılırsa hedef kendi dosyasından olmalı.
    #[test]
    fn a_task_cannot_cite_another_cases_anchor() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        let cross = db.conn().execute(
            "INSERT INTO tasks (id,case_id,title,source_anchor_id,created_at,updated_at)
             VALUES ('tx','c1','Dilekce','a9','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        );
        scope_error(cross, "tasks");
        assert_eq!(count(&db, "tasks"), 0);
        db.conn()
            .execute(
                "INSERT INTO tasks (id,case_id,title,source_anchor_id,created_at,updated_at)
                 VALUES ('t1','c1','Dilekce','a1','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                [],
            )
            .expect("aynı dosyada görev dayanağı yazılır");
        db.conn()
            .execute(
                "INSERT INTO tasks (id,case_id,title,source_anchor_id,created_at,updated_at)
                 VALUES ('t2','c1','Dayanaksiz',NULL,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                [],
            )
            .expect("dayanaksız görev meşru");
        let moved = db
            .conn()
            .execute("UPDATE tasks SET source_anchor_id='a9' WHERE id='t1'", []);
        scope_error(moved, "tasks");
    }

    /// v33 — notun **JSON dizi** kaynak listesi: her eleman kendi dosyasının
    /// çapası olmalı. Bugünkü canlı yol: `validate_note_anchors` doğruluyor,
    /// ama doğrudan SQL yazan bir yol bu kapıyı atlar.
    #[test]
    fn a_note_cannot_cite_another_cases_anchor_in_its_source_list() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        let insert = |id: &str, case: &str, anchors: &str| {
            db.conn().execute(
                "INSERT INTO notes (id,case_id,title,body_ast,author_kind,visibility_label,source_anchor_ids,created_at,updated_at)
                 VALUES (?1,?2,'Not','{\"v\":1,\"text\":\"metin\"}','user','case_private',?3,
                         '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                rusqlite::params![id, case, anchors],
            )
        };

        scope_error(insert("nx", "c1", "[\"a9\"]"), "notes");
        scope_error(insert("ny", "c1", "[\"a1\",\"a9\"]"), "notes");
        // Var olmayan kimlik de yazılmaz: doğrulanamayan referans kabul edilmez.
        scope_error(insert("nz", "c1", "[\"yok\"]"), "notes");
        // Bozuk JSON: kapı kendi koduyla reddeder (SQLite "malformed JSON" değil).
        scope_error(insert("nw", "c1", "[\"a9\""), "notes");
        assert_eq!(count(&db, "notes"), 0, "reddedilen yazma iz bırakmaz");

        insert("n1", "c1", "[\"a1\"]").expect("aynı dosyada kaynak yazılır");
        insert("n2", "c1", "[]").expect("kaynaksız not meşru");

        let moved = db.conn().execute(
            "UPDATE notes SET source_anchor_ids='[\"a9\"]' WHERE id='n1'",
            [],
        );
        scope_error(moved, "notes");
        let kept: String = db
            .conn()
            .query_row(
                "SELECT source_anchor_ids FROM notes WHERE id='n1'",
                [],
                |r| r.get(0),
            )
            .expect("not");
        assert_eq!(kept, "[\"a1\"]", "reddedilen UPDATE satırı bırakır");
    }

    /// v33 — revizyonda `case_id` yok: dosya **üst nottan** okunur.
    #[test]
    fn a_note_revision_cannot_cite_another_cases_anchor() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        db.conn()
            .execute_batch(
                "INSERT INTO notes (id,case_id,title,body_ast,author_kind,visibility_label,source_anchor_ids,created_at,updated_at)
                 VALUES ('n1','c1','Not','{\"v\":1,\"text\":\"metin\"}','user','case_private','[\"a1\"]',
                         '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                        ('n9','c2','Diger not','{\"v\":1,\"text\":\"gizli\"}','user','case_private','[\"a9\"]',
                         '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
            )
            .expect("notlar");
        let insert = |note: &str, anchors: &str| {
            db.conn().execute(
                "INSERT INTO note_revisions (note_id,revision_no,title,body_ast,source_anchor_ids,origin,created_at)
                 VALUES (?1,2,'Not','{\"v\":1,\"text\":\"metin\"}',?2,'edit','2026-01-01T00:00:00Z')",
                rusqlite::params![note, anchors],
            )
        };

        scope_error(insert("n1", "[\"a9\"]"), "note_revisions");
        scope_error(insert("n1", "[\"a1\",\"a9\"]"), "note_revisions");
        scope_error(insert("n9", "[\"a1\"]"), "note_revisions");
        assert_eq!(count(&db, "note_revisions"), 0, "hiçbir revizyon yazılmadı");

        insert("n1", "[\"a1\"]").expect("üst notun dosyasındaki çapa meşru");
        let moved = db.conn().execute(
            "UPDATE note_revisions SET source_anchor_ids='[\"a9\"]' WHERE note_id='n1'",
            [],
        );
        scope_error(moved, "note_revisions");
    }

    /// v33 — inceleme öğesinin kanıt listesi kendi dosyasından.
    #[test]
    fn a_review_item_cannot_cite_another_cases_anchor_in_its_source_list() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        let insert = |id: &str, case: &str, anchors: &str| {
            db.conn().execute(
                "INSERT INTO review_items (id,case_id,item_kind,payload,source_anchor_ids,created_at,updated_at)
                 VALUES (?1,?2,'claim_candidate','{}',?3,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                rusqlite::params![id, case, anchors],
            )
        };

        scope_error(insert("ix", "c1", "[\"a9\"]"), "review_items");
        scope_error(insert("iw", "c1", "[\"a1\",\"a9\"]"), "review_items");
        assert_eq!(count(&db, "review_items"), 0);

        insert("i1", "c1", "[\"a1\"]").expect("aynı dosyada öneri yazılır");
        let moved = db.conn().execute(
            "UPDATE review_items SET source_anchor_ids='[\"a9\"]' WHERE id='i1'",
            [],
        );
        scope_error(moved, "review_items");
    }

    /// v33 — taslak bilinçli olarak **gevşek**: var olmayan kimlik kalır (metin
    /// kurtarılır), başka dosyanın çapası girmez.
    #[test]
    fn a_note_draft_keeps_a_missing_anchor_but_not_a_foreign_one() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        let save = |posture: &str, anchors: &str| {
            db.conn().execute(
                "INSERT INTO note_drafts (case_id,note_id,title,body,source_anchor_ids,updated_at)
                 VALUES ('c1',?1,'Taslak','yaziyor',?2,'2026-01-01T00:00:00Z')
                 ON CONFLICT(case_id,note_id) DO UPDATE SET source_anchor_ids=excluded.source_anchor_ids",
                rusqlite::params![posture, anchors],
            )
        };

        // Yarım kalan otomatik kayıt: çapa henüz yok → metin kaybolmaz.
        save("", "[\"henuz-yok\"]").expect("var olmayan kimlik taslakta meşru");
        save("", "[\"a1\"]").expect("aynı dosyanın çapası meşru");
        // INSERT yolu (yeni taslak) ve UPDATE yolu (var olan taslak) ayrı kapılar.
        scope_error(save("yeni-not", "[\"a9\"]"), "note_drafts");
        scope_error(save("", "[\"a9\"]"), "note_drafts");
        scope_error(save("", "[\"a1\",\"a9\"]"), "note_drafts");
        let kept: String = db
            .conn()
            .query_row(
                "SELECT source_anchor_ids FROM note_drafts WHERE case_id='c1' AND note_id=''",
                [],
                |r| r.get(0),
            )
            .expect("taslak");
        assert_eq!(kept, "[\"a1\"]", "reddedilen yazma taslağı bozmaz");

        let moved = db.conn().execute(
            "UPDATE note_drafts SET source_anchor_ids='[\"a9\"]' WHERE case_id='c1'",
            [],
        );
        scope_error(moved, "note_drafts");

        // v34: taslağın bağlandığı **not** da kendi dosyasından olmalı.
        db.conn()
            .execute_batch(
                "INSERT INTO notes (id,case_id,title,body_ast,author_kind,visibility_label,source_anchor_ids,created_at,updated_at)
                 VALUES ('n1','c1','Not','{}','user','case_private','[]','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                        ('n9','c2','Diger not','{}','user','case_private','[]','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
            )
            .expect("notlar");
        let draft_of = |note_id: &str| {
            db.conn().execute(
                "INSERT INTO note_drafts (case_id,note_id,title,body,source_anchor_ids,updated_at)
                 VALUES ('c1',?1,'Taslak','yaziyor','[]','2026-01-01T00:00:00Z')",
                [note_id],
            )
        };
        draft_of("n1").expect("kendi dosyasının notu meşru");
        draft_of("henuz-yok").expect("var olmayan not taslakta meşru");
        scope_error(draft_of("n9"), "note_drafts");
    }

    /// v34: maskeleme planının **çıktı sürümü** kendi dosyasından olmalı.
    ///
    /// Ölçülen kusur (düzeltme öncesi): doğrudan SQL ile A dosyasının planına B
    /// dosyasının sürümü çıktı olarak yazılabiliyordu; `list_redaction_plans`
    /// A dosyasının sicilinde **B'nin belge adını** gösteriyordu.
    #[test]
    fn a_redaction_plan_output_cannot_be_another_cases_version() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        // Aynı dosyada, kaynağın alt sürümü (meşru maskeleme çıktısı).
        db.conn()
            .execute_batch(
                "INSERT INTO document_versions
                   (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at,parent_version_id)
                 VALUES ('vc','d2','c1',1,'b1','cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',10,
                         'application/pdf','2026-01-01T00:00:00Z','v1');",
            )
            .expect("meşru çıktı sürümü");
        let plan = |id: &str, output: &str| {
            db.conn().execute(
                "INSERT INTO redaction_plans
                   (id,case_id,document_version_id,render_hash,coordinate_space,plan_version,state,
                    output_profile,created_at,updated_at,output_version_id)
                 VALUES (?1,'c1','v1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                         'pdf_page_points',1,'applied','share','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z',?2)",
                rusqlite::params![id, output],
            )
        };
        plan("p1", "vc").expect("aynı dosyanın çıktısı meşru");
        scope_error(plan("px", "v9"), "redaction_plans");
        // UPDATE ile taşıma da kapıda durur (INSERT kapısı tek başına yetmez).
        let moved = db.conn().execute(
            "UPDATE redaction_plans SET output_version_id='v9' WHERE id='p1'",
            [],
        );
        scope_error(moved, "redaction_plans");
        let kept: Option<String> = db
            .conn()
            .query_row(
                "SELECT output_version_id FROM redaction_plans WHERE id='p1'",
                [],
                |r| r.get(0),
            )
            .expect("plan");
        assert_eq!(kept.as_deref(), Some("vc"), "reddedilen yazma planı bozmaz");
        assert_eq!(
            count(&db, "redaction_plans"),
            1,
            "reddedilen plan iz bırakmaz"
        );
    }

    /// v34: sürüm **soy ağacı** dosya sınırını aşmaz (maskeleme izi).
    #[test]
    fn a_version_parent_cannot_be_another_cases_version() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        let version = |id: &str, doc: &str, parent: &str| {
            db.conn().execute(
                "INSERT INTO document_versions
                   (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,
                    imported_at,parent_version_id)
                 VALUES (?1,?2,'c1',2,'b1','dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
                         10,'application/pdf','2026-01-01T00:00:00Z',?3)",
                rusqlite::params![id, doc, parent],
            )
        };
        scope_error(version("vp-x", "d1", "v9"), "document_versions");
        // Aynı dosyada **başka belgenin** sürümü ebeveyn olabilir (maskeli kopya yolu).
        version("vp", "d2", "v1").expect("aynı dosyada türev meşru");
        // UPDATE ile taşıma da reddedilir.
        let moved = db.conn().execute(
            "UPDATE document_versions SET parent_version_id='v9' WHERE id='vp'",
            [],
        );
        scope_error(moved, "document_versions");
        // Ebeveynsiz sürüm (NULL) kapıya takılmaz.
        db.conn()
            .execute(
                "INSERT INTO document_versions
                   (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
                 VALUES ('vn','d1','c1',3,'b1','eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',10,
                         'application/pdf','2026-01-01T00:00:00Z')",
                [],
            )
            .expect("ebeveynsiz sürüm");
    }

    /// v34: adayın **kökeni** (`review_items.ai_run_id`) kendi dosyasından.
    ///
    /// Var olmayan çalıştırma kimliği eski satırlarda kalabilir (kapı yalnız
    /// başka dosyada **var olan** kimliği reddeder).
    #[test]
    fn a_review_item_cannot_cite_another_cases_ai_run() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        db.conn()
            .execute_batch(
                "INSERT INTO ai_runs (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,status,usage_report,created_at)
                 VALUES ('run1','c1','summarize','m','h','p1','o1','running','{\"billing\":\"unknown\"}','2026-01-01T00:00:00Z'),
                        ('run9','c2','summarize','m','h','p1','o1','running','{\"billing\":\"unknown\"}','2026-01-01T00:00:00Z');",
            )
            .expect("çalıştırmalar");
        let item = |id: &str, run: Option<&str>| {
            db.conn().execute(
                "INSERT INTO review_items (id,case_id,item_kind,payload,source_anchor_ids,ai_run_id,created_at,updated_at)
                 VALUES (?1,'c1','claim_candidate','{}','[]',?2,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                rusqlite::params![id, run],
            )
        };
        item("i1", Some("run1")).expect("aynı dosyanın çalıştırması meşru");
        item("i-kaynak-yok", Some("olmayan-run")).expect("var olmayan köken meşru");
        item("i-run-yok", None).expect("köken yok meşru");
        scope_error(item("i9", Some("run9")), "review_items");
        assert_eq!(count(&db, "review_items"), 3, "yalnız meşru satırlar");
    }

    /// v34: **gönderim sicili** kendi dosyasının sürümünü/çapasını gösterir.
    ///
    /// Ölçülen kusur (düzeltme öncesi): `record_context_items` çekirdek kapısı
    /// yoktu — A dosyasının çalıştırmasına B dosyasının sürümü + çapası yazıldı
    /// ve `list_context_items` A dosyasının ekranında **B'nin belge adını**
    /// gösterdi. Var olmayan kimlik ise meşru denetim izidir (belge gönderimden
    /// sonra silinebilir) — kapı onu reddetmez.
    #[test]
    fn an_ai_context_item_cannot_link_another_cases_version_or_anchor() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        db.conn()
            .execute_batch(
                "INSERT INTO ai_runs (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,status,usage_report,created_at)
                 VALUES ('run1','c1','summarize','m','h','p1','o1','running','{\"billing\":\"unknown\"}','2026-01-01T00:00:00Z');",
            )
            .expect("çalıştırma");
        let item = |id: &str, version: &str, anchor: Option<&str>, segments: &str| {
            db.conn().execute(
                "INSERT INTO ai_context_items (id,ai_run_id,document_version_id,source_anchor_id,segment_ids,inclusion_reason,bytes_sent)
                 VALUES (?1,'run1',?2,?3,?4,'onaylanan belge',7)",
                rusqlite::params![id, version, anchor, segments],
            )
        };
        item("ci-ok", "v1", Some("a1"), "[\"s1\"]").expect("aynı dosyanın bağı meşru");
        item("ci-gone", "silinmis-surum", None, "[]")
            .expect("var olmayan kimlik denetim izinde kalır");
        scope_error(item("ci-x", "v9", None, "[]"), "ai_context_items");
        scope_error(item("ci-y", "v1", Some("a9"), "[]"), "ai_context_items");
        // Segment listesi satırın **kendi** sürümünün segmentleri olmalı.
        scope_error(item("ci-z", "v1", None, "[\"s9\"]"), "ai_context_items");
        scope_error(item("ci-w", "v1", None, "bozuk-json"), "ai_context_items");
        assert_eq!(
            count(&db, "ai_context_items"),
            2,
            "yalnız iki meşru satır yazıldı"
        );
        // UPDATE ile taşıma da kapıda durur.
        let moved = db.conn().execute(
            "UPDATE ai_context_items SET document_version_id='v9' WHERE id='ci-ok'",
            [],
        );
        scope_error(moved, "ai_context_items");
    }

    /// v34: **giden segment sicili** kendi dosyasının sürümünü gösterir.
    ///
    /// Tablo maskelenmiş metnin kendisini taşır; sürüm başka dosyadan ise A
    /// dosyasının gönderim sicili B dosyasının metnini gösterir. `document_version_id`
    /// NULL olabilir (0026 öncesi satır) — NULL kapıya takılmaz.
    #[test]
    fn a_transfer_segment_cannot_record_another_cases_version() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        db.conn()
            .execute_batch(
                "INSERT INTO ai_runs (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,status,usage_report,created_at)
                 VALUES ('run1','c1','summarize','m','h','p1','o1','running','{\"billing\":\"unknown\"}','2026-01-01T00:00:00Z');",
            )
            .expect("çalıştırma");
        let segment = |id: &str, version: Option<&str>| {
            db.conn().execute(
                "INSERT INTO transfer_segments (id,ai_run_id,outbound_segment_id,document_version_id,outbound_text,outbound_hash)
                 VALUES (?1,'run1',?1,?2,'itiraz','hash')",
                rusqlite::params![id, version],
            )
        };
        segment("t1", Some("v1")).expect("aynı dosyanın sürümü meşru");
        segment("t-null", None).expect("eski satır (sürüm yok) meşru");
        scope_error(segment("t-x", Some("v9")), "transfer_segments");
        let moved = db.conn().execute(
            "UPDATE transfer_segments SET document_version_id='v9' WHERE id='t1'",
            [],
        );
        scope_error(moved, "transfer_segments");
        assert_eq!(count(&db, "transfer_segments"), 2, "yalnız meşru satırlar");
    }

    /// v38: giden segment sicili **kendi segmentine** bağlıdır.
    ///
    /// Satır "V sürümünün S segmenti gönderildi" der; S **var olan** bir segment
    /// ise V'nin segmenti olmalı. 0034 yalnız gösterilen sürümü kapılıyordu —
    /// segment kimliği serbestti: doğrudan SQL ile A'nın çalıştırmasına A'nın
    /// sürümü + **B'nin segmenti** yazılıyor ve A'nın gönderim sicili B'nin
    /// parçasını gösteriyordu (denetim izi yanlış belgeyi yazar). Sürüm `NULL`
    /// olan eski biçimde ölçüt çalıştırmanın dosyasıdır. Var olmayan segment
    /// kimliği bilinçli olarak serbesttir (denetim izi; segment sonradan
    /// silinebilir) — bu test o gevşekliği de sabitler.
    #[test]
    fn a_transfer_segment_cannot_claim_a_segment_of_another_version() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        db.conn()
            .execute_batch(
                "INSERT INTO ai_runs (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,status,usage_report,created_at)
                 VALUES ('run1','c1','summarize','m','h','p1','o1','running','{\\\"billing\\\":\\\"unknown\\\"}','2026-01-01T00:00:00Z');",
            )
            .expect("çalıştırma");
        let row = |id: &str, segment: &str, version: Option<&str>| {
            db.conn().execute(
                "INSERT INTO transfer_segments
                   (id,ai_run_id,outbound_segment_id,source_segment_id,document_version_id,outbound_text,outbound_hash)
                 VALUES (?1,'run1',?2,?2,?3,'itiraz','hash')",
                rusqlite::params![id, segment, version],
            )
        };
        // Kendi sürümünün segmentleri meşru — iki ayrı koşunun segmenti de.
        row("t1", "s1", Some("v1")).expect("kendi sürümünün segmenti meşru");
        row("t2", "s2", Some("v1")).expect("aynı sürümün ikinci koşusunun segmenti meşru");
        row("t-gone", "silinmis-segment", Some("v1")).expect("var olmayan segment denetim izi");
        // B dosyasının segmenti A'nın sürümü altında yazılamaz; sürüm yoksa da
        // (dosya ölçütü) yazılamaz.
        scope_error(row("t-x", "s9", Some("v1")), "transfer_segments");
        scope_error(row("t-y", "s9", None), "transfer_segments");
        // İki segment kolonu ayrışırsa **gösterilen** alan da denetlenir:
        // `source_segment_id` meşru olması `outbound_segment_id`i aklamaz.
        scope_error(
            db.conn().execute(
                "INSERT INTO transfer_segments
                   (id,ai_run_id,outbound_segment_id,source_segment_id,document_version_id,outbound_text,outbound_hash)
                 VALUES ('t-split','run1','s9','s1','v1','gizli','hash')",
                [],
            ),
            "transfer_segments",
        );
        // UPDATE ile segment değiştirme de kapıda durur.
        scope_error(
            db.conn().execute(
                "UPDATE transfer_segments SET outbound_segment_id='s9', source_segment_id='s9'
                  WHERE id='t1'",
                [],
            ),
            "transfer_segments",
        );
        assert_eq!(count(&db, "transfer_segments"), 3, "yalnız meşru satırlar");
    }

    /// v38 taraması: kapıdan önce yazılmış **yanlış segmentli** giden sicil
    /// satırları sayılır (silinmez); var olmayan segment kimliği aykırı
    /// **değildir** (denetim izi — kanıtlanabilir uyuşmazlık yok).
    #[test]
    fn the_scan_counts_transfer_segments_claiming_another_versions_segment() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        db.conn()
            .execute_batch(
                "INSERT INTO ai_runs (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,status,usage_report,created_at)
                 VALUES ('run1','c1','summarize','m','h','p1','o1','running','{\\\"billing\\\":\\\"unknown\\\"}','2026-01-01T00:00:00Z');",
            )
            .expect("çalıştırma");
        assert_eq!(db.scope_violations().expect("tarama").transfer_segments, 0);
        // v38 öncesi kasa: kapı yokken yazılmış yanlış segmentli satır.
        db.conn()
            .execute_batch(
                "DROP TRIGGER scope_guard_transfer_segments_bi;
                 DROP TRIGGER scope_guard_transfer_segments_bu;
                 INSERT INTO transfer_segments
                   (id,ai_run_id,outbound_segment_id,source_segment_id,document_version_id,outbound_text,outbound_hash)
                 VALUES ('old-t','run1','s9','s9','v1','gizli','hash');",
            )
            .expect("eski yanlış segmentli satır");
        let counts = db.scope_violations().expect("tarama");
        assert_eq!(counts.transfer_segments, 1, "yanlış segment sayılır");
        assert_eq!(counts.total(), 1);
        // Segmenti artık kasada olmayan satır aykırı sayılmaz (bilinçli).
        db.conn()
            .execute(
                "INSERT INTO transfer_segments
                   (id,ai_run_id,outbound_segment_id,document_version_id,outbound_text,outbound_hash)
                 VALUES ('old-gone','run1','silinmis-segment','v1','metin','hash')",
                [],
            )
            .expect("segmenti silinmiş satır");
        assert_eq!(
            db.scope_violations().expect("tarama").transfer_segments,
            1,
            "var olmayan segment aykırı değil"
        );
        // Satırlar silinmez, yalnız sayılır.
        assert_eq!(count(&db, "transfer_segments"), 2);
    }

    /// v39: **aktarım onayı** kendi dosyasının **silinmemiş** belgelerini
    /// gösterir.
    ///
    /// Onay satırı "şu belgeler için aktarım izni verildi" der — cihazdan ne
    /// çıkabileceğinin tek yetki kaydıdır. Şema hiçbir şey istemiyordu:
    /// ölçüldü (v38, geçici sonda), doğrudan SQL ile A dosyasının onayına
    /// **B dosyasının belgesi** yazılıyor ve `list_transfer_consents('c1')`
    /// A'nın envanterinde `["d9"]` gösteriyordu; boş liste (`[]`), bozuk JSON
    /// ve silinmiş belge kimliği de kabul ediliyordu. Kapı çekirdeğin
    /// `create_transfer_consent` kuralının aynısıdır: her eleman `case_scope`
    /// dosyasında, silinmemiş bir belge olmalı; liste boş olamaz.
    /// **Sonradan** silinen belge kimliği denetim izidir (kapı yalnız yeni
    /// yazmada koşar; tarama da onu aykırı saymaz).
    #[test]
    fn a_transfer_consent_cannot_name_another_cases_document() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        let consent = |id: &str, case: &str, ids: &str| {
            db.conn().execute(
                "INSERT INTO transfer_consents
                   (id,provider_snapshot,case_scope,purpose,selected_document_ids,consent_text_version,approved_at)
                 VALUES (?1,'snap',?2,'ozet',?3,'1','2026-01-01T00:00:00Z')",
                rusqlite::params![id, case, ids],
            )
        };
        // Kendi dosyasının belgeleri meşru — tek ve iki elemanlı liste.
        consent("tc1", "c1", r#"["d1"]"#).expect("kendi belgesi meşru");
        consent("tc2", "c1", r#"["d1","d2"]"#).expect("iki belge meşru");
        // B dosyasının belgesi A'nın onayına yazılamaz (ve ters yön de kapalı).
        scope_error(consent("tc-x", "c1", r#"["d9"]"#), "transfer_consents");
        scope_error(consent("tc-y", "c2", r#"["d1"]"#), "transfer_consents");
        // Boş liste hiçbir şeyi yetkilendirmez; bozuk/array olmayan JSON
        // doğrulanamaz; var olmayan belge kimliği yetki kaydına giremez.
        scope_error(consent("tc-empty", "c1", "[]"), "transfer_consents");
        scope_error(consent("tc-bad", "c1", "bozuk-json"), "transfer_consents");
        scope_error(
            consent("tc-obj", "c1", r#"{"belge":"d1"}"#),
            "transfer_consents",
        );
        scope_error(
            consent("tc-none", "c1", r#"["olmayan-belge"]"#),
            "transfer_consents",
        );
        // Silinmiş belge yeni onaya yazılamaz.
        db.conn()
            .execute(
                "UPDATE documents SET deleted_at='2026-02-01T00:00:00Z' WHERE id='d2'",
                [],
            )
            .expect("sil");
        scope_error(consent("tc-del", "c1", r#"["d2"]"#), "transfer_consents");
        // UPDATE ile kapsamı genişletmek/taşımak da kapıda durur.
        scope_error(
            db.conn().execute(
                "UPDATE transfer_consents SET selected_document_ids='[\"d9\"]' WHERE id='tc1'",
                [],
            ),
            "transfer_consents",
        );
        scope_error(
            db.conn().execute(
                "UPDATE transfer_consents SET case_scope='c2' WHERE id='tc1'",
                [],
            ),
            "transfer_consents",
        );
        // İptal (kapsam dışı sütun) kapıya takılmaz — v35 dersi.
        db.conn()
            .execute(
                "UPDATE transfer_consents SET revoked_at='2026-03-01T00:00:00Z' WHERE id='tc1'",
                [],
            )
            .expect("iptal kapıya takılmamalı");
        assert_eq!(count(&db, "transfer_consents"), 2, "yalnız meşru satırlar");
    }

    /// v39 taraması: kapıdan önce yazılmış çapraz/boş/bozuk onay satırları
    /// **sayılır** (silinmez); sonradan silinmiş belgeyi gösteren satır aykırı
    /// **değildir** (onay verildikten sonra belge silinebilir — denetim izi).
    #[test]
    fn the_scan_counts_transfer_consents_written_before_the_guard_existed() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        assert_eq!(
            db.scope_violations().expect("tarama").transfer_consents,
            0,
            "temiz kasa"
        );
        db.conn()
            .execute_batch(
                "DROP TRIGGER scope_guard_transfer_consents_bi;
                 DROP TRIGGER scope_guard_transfer_consents_bu;
                 INSERT INTO transfer_consents
                   (id,provider_snapshot,case_scope,purpose,selected_document_ids,consent_text_version,approved_at)
                 VALUES ('old-x','snap','c1','ozet','[\"d9\"]','1','2026-01-01T00:00:00Z'),
                        ('old-empty','snap','c1','ozet','[]','1','2026-01-01T00:00:00Z'),
                        ('old-bad','snap','c1','ozet','[oops','1','2026-01-01T00:00:00Z'),
                        ('old-ok','snap','c1','ozet','[\"d1\"]','1','2026-01-01T00:00:00Z');",
            )
            .expect("eski satırlar");
        let counts = db.scope_violations().expect("tarama");
        assert_eq!(counts.transfer_consents, 3, "çapraz + boş + bozuk");
        assert_eq!(counts.total(), 3);
        // Belge sonradan silinirse kimliği aykırı sayılmaz (gönderim yolu onu atlar).
        db.conn()
            .execute(
                "UPDATE documents SET deleted_at='2026-02-01T00:00:00Z' WHERE id='d1'",
                [],
            )
            .expect("sil");
        assert_eq!(
            db.scope_violations().expect("tarama").transfer_consents,
            3,
            "silinmiş belge aykırı değil"
        );
        // Satırlar silinmez, yalnız sayılır.
        assert_eq!(count(&db, "transfer_consents"), 4);
    }

    /// v33 taraması: bu şema sürümünden önce yazılmış çapraz JSON dizileri
    /// **sayılır** (silinmez); bozuk JSON da "doğrulanamıyor" diye sayılır.
    #[test]
    fn the_scan_counts_json_array_rows_written_before_the_guards_existed() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        assert_eq!(db.scope_violations().expect("tarama").total(), 0);
        db.conn()
            .execute_batch(
                "DROP TRIGGER scope_guard_notes_bi;
                 DROP TRIGGER scope_guard_note_revisions_bi;
                 DROP TRIGGER scope_guard_review_items_bi;
                 DROP TRIGGER scope_guard_note_drafts_bi;",
            )
            .expect("tetikleyici düşür");
        db.conn()
            .execute_batch(
                "INSERT INTO notes (id,case_id,title,body_ast,author_kind,visibility_label,source_anchor_ids,created_at,updated_at)
                 VALUES ('old-n','c1','Eski not','{\"v\":1,\"text\":\"x\"}','user','case_private','[\"a9\"]',
                         '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
                 INSERT INTO note_revisions (note_id,revision_no,title,body_ast,source_anchor_ids,origin,created_at)
                 VALUES ('old-n',1,'Eski not','{\"v\":1,\"text\":\"x\"}','[\"a9\"]','create','2026-01-01T00:00:00Z');
                 INSERT INTO review_items (id,case_id,item_kind,payload,source_anchor_ids,created_at,updated_at)
                 VALUES ('old-i','c1','claim_candidate','{}','[\"a9\"]','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
                 INSERT INTO note_drafts (case_id,note_id,title,body,source_anchor_ids,updated_at)
                 VALUES ('c1','','Taslak','x','[\"a9\"]','2026-01-01T00:00:00Z');
                 INSERT INTO note_drafts (case_id,note_id,title,body,source_anchor_ids,updated_at)
                 VALUES ('c1','bozuk','Taslak','x','[oops','2026-01-01T00:00:00Z');",
            )
            .expect("eski çapraz diziler");
        let counts = db.scope_violations().expect("tarama");
        assert_eq!(counts.notes, 1, "çapraz not kaynağı sayılır");
        assert_eq!(counts.note_revisions, 1);
        assert_eq!(counts.review_items, 1);
        assert_eq!(counts.note_drafts, 2, "çapraz + bozuk JSON");
        assert_eq!(counts.total(), 5);
        // Diğer sınıflar temiz: tarama sınıfları ayırır.
        assert_eq!(counts.claims, 0);
        assert_eq!(counts.document_versions, 0);
        // Satırlar silinmez, yalnız sayılır.
        assert_eq!(count(&db, "notes"), 1);
    }

    #[test]
    fn the_scan_counts_reference_rows_written_before_the_guards_existed() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        claims_and_events(&db);
        assert_eq!(db.scope_violations().expect("tarama").total(), 0);
        // v32 öncesi kasa: referans tetikleyicileri yokken yazılmış çapraz satırlar.
        db.conn()
            .execute_batch(
                "DROP TRIGGER scope_guard_claims_bi;
                 DROP TRIGGER scope_guard_evidence_links_bi;
                 DROP TRIGGER scope_guard_event_sources_bi;
                 DROP TRIGGER scope_guard_claim_relations_bi;
                 DROP TRIGGER scope_guard_tasks_bi;",
            )
            .expect("tetikleyici düşür");
        db.conn()
            .execute_batch(
                "INSERT INTO claims (id,case_id,claim_text,claim_kind,status,source_anchor_id,created_at,updated_at)
                 VALUES ('old-k','c1','Iddia','allegation','draft','a9','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
                 INSERT INTO evidence_links (id,case_id,claim_id,source_anchor_id,relation,rationale,review_state,created_at,updated_at)
                 VALUES ('old-el','c1','k1','a9','supports_candidate','','accepted','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
                 INSERT INTO event_sources (id,event_id,source_anchor_id,relation,user_confirmed)
                 VALUES ('old-es','e1','a9','supports',1);
                 INSERT INTO claim_relations (id,case_id,claim_id,related_claim_id,relation,rationale,created_at,updated_at)
                 VALUES ('old-cr','c1','k1','k9','response_to','','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
                 INSERT INTO tasks (id,case_id,title,source_anchor_id,created_at,updated_at)
                 VALUES ('old-t','c1','Dilekce','a9','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
            )
            .expect("eski çapraz referanslar");
        let counts = db.scope_violations().expect("tarama");
        assert_eq!(counts.claims, 1, "çapraz iddia çapası sayılır");
        assert_eq!(counts.evidence_links, 1);
        assert_eq!(counts.event_sources, 1);
        assert_eq!(counts.claim_relations, 1);
        assert_eq!(counts.tasks, 1);
        assert_eq!(counts.total(), 5);
        // Zincir tabloları temiz: tarama sınıfları ayırır.
        assert_eq!(counts.document_versions, 0);
        assert_eq!(counts.source_anchors, 0);
    }

    /// v34 taraması: bu şema sürümünden önce yazılmış soy ağacı, maskeleme
    /// çıktısı ve gönderim sicili bağları **sayılır** (silinmez). Tarama yalnız
    /// dosya sınırını sayar: gönderim sicilindeki var olmayan kimlik meşru bir
    /// denetim izidir.
    #[test]
    fn the_scan_counts_remaining_reference_links_written_before_the_guards_existed() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        db.conn()
            .execute_batch(
                "INSERT INTO ai_runs (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,status,usage_report,created_at)
                 VALUES ('run1','c1','summarize','m','h','p1','o1','running','{\"billing\":\"unknown\"}','2026-01-01T00:00:00Z');",
            )
            .expect("çalıştırma");
        assert_eq!(db.scope_violations().expect("tarama").total(), 0);
        // v34 öncesi kasa: üç sınıfın tetikleyicileri yokken yazılmış çapraz bağlar.
        db.conn()
            .execute_batch(
                "DROP TRIGGER scope_guard_document_versions_parent_bi;
                 DROP TRIGGER scope_guard_redaction_plans_output_bi;
                 DROP TRIGGER scope_guard_ai_context_items_bi;
                 DROP TRIGGER scope_guard_transfer_segments_bi;
                 DROP TRIGGER scope_guard_source_marks_page_bi;
                 INSERT INTO document_versions
                   (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at,parent_version_id)
                 VALUES ('old-vp','d1','c1',5,'b1','dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',10,
                         'application/pdf','2026-01-01T00:00:00Z','v9');
                 INSERT INTO redaction_plans
                   (id,case_id,document_version_id,render_hash,coordinate_space,plan_version,state,output_profile,created_at,updated_at,output_version_id,output_version_ref)
                 VALUES ('old-p','c1','v1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','pdf_page_points',1,
                         'applied','share','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','v9','v9');
                 INSERT INTO ai_context_items (id,ai_run_id,document_version_id,source_anchor_id,segment_ids,inclusion_reason,bytes_sent)
                 VALUES ('old-c','run1','v9','a9','[]','onaylanan belge',7),
                        ('old-gone','run1','silinmis-surum',NULL,'[]','onaylanan belge',7);
                 INSERT INTO transfer_segments (id,ai_run_id,outbound_segment_id,document_version_id,outbound_text,outbound_hash)
                 VALUES ('old-t','run1','s9','v9','gizli','hash');
                 INSERT INTO source_marks
                   (id,case_id,document_version_id,parse_run_id,anchor_id,page_id,mark_kind,start_offset,end_offset,
                    quoted_text,rects_json,label,created_at)
                 VALUES ('old-m','c1','v1','r1','a1','p9','text',0,6,'itiraz','[]','','2026-01-01T00:00:00Z');",
            )
            .expect("eski çapraz bağlar");
        let counts = db.scope_violations().expect("tarama");
        assert_eq!(counts.document_versions, 1, "soy ağacı sayılır");
        assert_eq!(counts.redaction_plans, 1, "maskeleme çıktısı sayılır");
        assert_eq!(counts.ai_context_items, 1, "gönderim sicili sayılır");
        assert_eq!(counts.transfer_segments, 1, "giden segment sicili sayılır");
        assert_eq!(counts.source_marks, 1, "işaretin sayfası sayılır");
        assert_eq!(counts.total(), 5, "var olmayan denetim izi aykırı değil");
        // Diğer sınıflar temiz: tarama sınıfları ayırır.
        assert_eq!(counts.claims, 0);
        assert_eq!(counts.notes, 0);
        // Satırlar silinmez, yalnız sayılır.
        assert_eq!(count(&db, "ai_context_items"), 2);
    }

    /// Maskeli kopya meşru: **aynı dosyada başka belge** ebeveyn olabilir.
    #[test]
    fn a_derivative_version_in_the_same_case_still_writes() {
        let db = db();
        chain(&db);
        db.conn()
            .execute(
                "INSERT INTO document_versions
                   (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at,
                    parent_version_id,is_original)
                 VALUES ('v2','d2','c1',1,'b1','cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',10,
                         'application/pdf','2026-01-01T00:00:00Z','v1',0)",
                [],
            )
            .expect("aynı dosyada türev sürüm yazılır");
        assert_eq!(count(&db, "document_versions"), 2);
    }

    #[test]
    fn the_scan_counts_rows_written_before_the_guards_existed() {
        let db = db();
        chain(&db);
        assert_eq!(
            db.scope_violations().expect("tarama").total(),
            0,
            "temiz zincir"
        );
        // v31 öncesi bir kasa: tetikleyiciler yokken yazılmış çapraz satır.
        db.conn()
            .execute_batch(
                "DROP TRIGGER scope_guard_document_versions_bi;
                 DROP TRIGGER scope_guard_text_segments_bi;",
            )
            .expect("tetikleyici düşür");
        db.conn()
            .execute_batch(
                "INSERT INTO document_versions
                   (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
                 VALUES ('old-v','d1','c2',9,'b1','dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',10,
                         'application/pdf','2026-01-01T00:00:00Z');
                 INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
                 VALUES ('old-s','r1','p1','c2','metin','metin','2026-01-01T00:00:00Z');",
            )
            .expect("eski aykırı satırlar");
        let counts = db.scope_violations().expect("tarama");
        assert_eq!(counts.document_versions, 1);
        assert_eq!(counts.text_segments, 1);
        assert_eq!(counts.total(), 2);
        // Diğer tablolar temiz: tarama tabloyu tabloyu ayırır.
        assert_eq!(counts.parse_runs, 0);
        assert_eq!(counts.source_anchors, 0);
    }

    /// Şemadan türetilen kapı: kapsam sütunu taşıyan her tablonun iki tetikleyicisi
    /// olmalı (INSERT + UPDATE) ve gövdesi kendi tablosunu `DIM_SCOPE:` ile
    /// adlandırmalı. Yeni bir tablo ya da yeni bir referans sütunu sessizce
    /// kapısız kalamaz.
    ///
    /// Üç kural: (1) `case_id` + zincir/referans sürüm sütunu; (2) `case_id`
    /// taşımayan ama iki kapsamlı kaydı bağlayan tablo (`event_sources`:
    /// `event_id` + `source_anchor_id`); (3) **JSON dizi** kanıt listesi
    /// (`*_anchor_ids` — `note_revisions`in `case_id`'si yoktur, dosya üst
    /// nottan okunur; kural yalnız sütun adına bakar). Dördüncü kural (v37):
    /// `case_id`'si olmayan, dosyayı üst kaydından miras alan sürüm izi sicili
    /// (`export_id` + `source_version_id`).
    #[test]
    fn every_case_scoped_table_has_guard_triggers() {
        let db = db();
        let scope_columns = [
            // zincir (v31)
            "document_id",
            "document_version_id",
            "parse_run_id",
            "segment_id",
            // referans (v32)
            "source_anchor_id",
            "anchor_id",
            "claim_id",
            "related_claim_id",
            // kalan referans (v34)
            "parent_version_id",
            "output_version_id",
            // güncel sürüm işaretçisi (v35)
            "current_version_id",
            // sürüm izi sicilleri (v37) — sürümü **adıyla** gösteren sütunlar
            "input_document_version_id",
            "source_version_id",
        ];
        // Referans sütunlarının **tam** listesi: bir kapsamlı tabloda bu
        // sütunlardan biri varsa, o tablonun tetikleyicilerinden biri sütunu
        // adıyla anmalı (kapı sessizce yeni bir sütunu atlayamaz).
        let reference_columns = [
            "document_id",
            "document_version_id",
            "parent_version_id",
            "output_version_id",
            "parse_run_id",
            "segment_id",
            "page_id",
            "source_anchor_id",
            "anchor_id",
            "claim_id",
            "related_claim_id",
            "event_id",
            "ai_run_id",
            "note_id",
            // sürüm izi sicilleri (v37)
            "input_document_version_id",
            "source_version_id",
            "export_id",
            // aktarım onayının belge listesi (v39)
            "selected_document_ids",
        ];
        // JSON dizi referansı taşıyan tablolar: kapı `json_each` ile dizinin
        // **her** elemanını denetlemeli.
        let json_anchor_tables = ["notes", "note_revisions", "review_items", "note_drafts"];
        let mut stmt = db
            .conn()
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            )
            .expect("tablolar");
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("tablolar")
            .collect::<Result<_, _>>()
            .expect("tablolar");

        let mut derived = Vec::new();
        for table in tables {
            if table == "schema_migrations" {
                continue;
            }
            let mut info = db
                .conn()
                .prepare(&format!("PRAGMA table_info(\"{table}\")"))
                .expect("sütunlar");
            let columns: Vec<String> = info
                .query_map([], |row| row.get::<_, String>(1))
                .expect("sütunlar")
                .collect::<Result<_, _>>()
                .expect("sütunlar");
            let has = |wanted: &str| columns.iter().any(|c| c == wanted);
            if (has("case_id") && scope_columns.iter().any(|c| has(c)))
                || (has("event_id") && has("source_anchor_id"))
                // `case_id`'si olmayan, kapsamı üst kaydından miras alan ama
                // **kanıt referansı** taşıyan tablo (gönderim sicili).
                || (has("ai_run_id") && has("document_version_id"))
                // `case_id`'si olmayan, kapsamı üst kaydından (`exports`)
                // miras alan **sürüm izi sicili** (v37).
                || (has("export_id") && has("source_version_id"))
                // Kapsam sütunu `case_scope` adını taşıyan **aktarım onayı**
                // (v39): belge listesi (`selected_document_ids`) kendi
                // dosyasının belgelerini göstermeli.
                || (has("case_scope") && has("selected_document_ids"))
                || columns.iter().any(|c| c.ends_with("_anchor_ids"))
            {
                derived.push(table);
            }
        }
        derived.sort();
        let mut guarded: Vec<&str> = GUARDED_TABLES.to_vec();
        guarded.sort();
        assert_eq!(
            derived, guarded,
            "kapsam sütunu taşıyan tablo kümesi ile kapılı tablolar ayrıştı"
        );

        for table in GUARDED_TABLES {
            let names = db.scope_trigger_names(table).expect("tetikleyiciler");
            let mut all_sql = String::new();
            for name in &names {
                if let Some(sql) = db.trigger_sql(name).expect("gövde") {
                    all_sql.push_str(&sql);
                    all_sql.push('\n');
                }
            }
            for suffix in ["_bi", "_bu"] {
                let expected = format!("scope_guard_{table}{suffix}");
                assert!(
                    names.contains(&expected),
                    "{table}: {expected} yok (ölçülen: {names:?})"
                );
                let sql = db.trigger_sql(&expected).expect("gövde").expect("var");
                assert!(
                    sql.contains(&format!("DIM_SCOPE:{table}")),
                    "{expected} kendi tablosunu adlandırmıyor"
                );
                if json_anchor_tables.contains(&table) {
                    assert!(
                        sql.contains("json_each("),
                        "{expected} dizinin elemanlarını denetlemiyor"
                    );
                }
            }
            // Sütun kapsamı: tablodaki her referans sütunu bir tetikleyicide
            // **adıyla** anılmalı. Yeni bir referans sütunu eklendiğinde bu test
            // kapıyı olmadan geçmez.
            let mut info = db
                .conn()
                .prepare(&format!("PRAGMA table_info(\"{table}\")"))
                .expect("sütunlar");
            let columns: Vec<String> = info
                .query_map([], |row| row.get::<_, String>(1))
                .expect("sütunlar")
                .collect::<Result<_, _>>()
                .expect("sütunlar");
            for column in reference_columns {
                if columns.iter().any(|c| c == column) {
                    assert!(
                        all_sql.contains(column),
                        "{table}.{column} hiçbir kapsam tetikleyicisinde anılmıyor"
                    );
                }
            }
            if table == "ai_context_items" || table == "transfer_consents" {
                assert!(
                    all_sql.contains("json_each("),
                    "{table} JSON listesini denetlemiyor"
                );
            }
        }
    }

    /// Tetikleyicileri geçici düşürüp **v35 öncesi** bozuk satırı üretir, sonra
    /// aynı gövdeleri geri kurar (eski kasadan gelen satırı taklit eder).
    fn write_legacy_pointer(db: &Database, document_id: &str, version_id: &str) {
        let bi = db
            .trigger_sql("scope_guard_documents_bi")
            .expect("gövde")
            .expect("var");
        let bu = db
            .trigger_sql("scope_guard_documents_bu")
            .expect("gövde")
            .expect("var");
        let mv = db
            .trigger_sql("scope_guard_document_versions_current_ref_bu")
            .expect("gövde")
            .expect("var");
        db.conn()
            .execute_batch(
                "DROP TRIGGER scope_guard_documents_bi;
                 DROP TRIGGER scope_guard_documents_bu;
                 DROP TRIGGER scope_guard_document_versions_current_ref_bu;",
            )
            .expect("düşür");
        db.conn()
            .execute(
                "UPDATE documents SET current_version_id=?1 WHERE id=?2",
                rusqlite::params![version_id, document_id],
            )
            .expect("eski satır");
        db.conn()
            .execute_batch(&format!("{bi};\n{bu};\n{mv};"))
            .expect("geri kur");
    }

    /// v35: belgenin güncel sürüm işaretçisi başka dosyanın (ya da aynı dosyada
    /// başka belgenin) sürümünü gösteremez. Ölçülen kusur (düzeltme öncesi):
    /// bu UPDATE kabul ediliyor, kapsam raporu B'nin sayfalarını A'da sayıyor
    /// ve A'nın alıntısı B'nin metninde `exact` dönüyordu.
    #[test]
    fn documents_cannot_point_at_a_foreign_or_sibling_version() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        // Başka dosyanın sürümü (c2 → c1 belgesi).
        scope_error(
            db.conn().execute(
                "UPDATE documents SET current_version_id='v9' WHERE id='d1'",
                [],
            ),
            "documents",
        );
        // Aynı dosyada **başka belgenin** sürümü de olamaz (v1 → d2).
        scope_error(
            db.conn().execute(
                "UPDATE documents SET current_version_id='v1' WHERE id='d2'",
                [],
            ),
            "documents",
        );
        // INSERT yolu: var olan yabancı sürüm.
        scope_error(
            db.conn().execute(
                "INSERT INTO documents (id,case_id,display_name,current_version_id,created_at,updated_at)
                 VALUES ('d-yeni','c1','Yeni','v9','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                [],
            ),
            "documents",
        );
        // INSERT yolu: olmayan sürüm de yazılmaz (fail-closed).
        scope_error(
            db.conn().execute(
                "INSERT INTO documents (id,case_id,display_name,current_version_id,created_at,updated_at)
                 VALUES ('d-yeni2','c1','Yeni','v-yok','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                [],
            ),
            "documents",
        );
        // Meşru yol: belgenin **kendi** sürümü.
        let changed = db
            .conn()
            .execute(
                "UPDATE documents SET current_version_id='v1' WHERE id='d1'",
                [],
            )
            .expect("kendi sürümü");
        assert_eq!(changed, 1);
        // NULL her zaman serbest (belge henüz sürümsüz).
        db.conn()
            .execute(
                "UPDATE documents SET current_version_id=NULL WHERE id='d1'",
                [],
            )
            .expect("null");
    }

    /// Eski kasadan gelen **boşta kalmış** işaretçi (sürümü silinmiş) başka
    /// alanların güncellenmesini engellemez; yalnız işaretçiyi **değiştiren**
    /// yazma kapıda durur. Aksi hâlde bozuk tek satır belgeyi kilitleyirdi.
    #[test]
    fn a_dangling_pointer_does_not_block_other_updates() {
        let db = db();
        chain(&db);
        write_legacy_pointer(&db, "d2", "v-silinmis");
        let changed = db
            .conn()
            .execute(
                "UPDATE documents SET display_name='Yeni ad', current_version_id=current_version_id
                  WHERE id='d2'",
                [],
            )
            .expect("adlandırma engellenmemeli");
        assert_eq!(changed, 1);
        scope_error(
            db.conn().execute(
                "UPDATE documents SET current_version_id='v1' WHERE id='d2'",
                [],
            ),
            "documents",
        );
    }

    /// Değişmez **ters yönden** de korunur: bir belgenin güncel sürümünü
    /// gösteren satır başka belgeye taşınamaz (kapı sürüm satırı taşınarak
    /// atlatılamaz). İşaretçi çekilince taşıma serbesttir.
    #[test]
    fn a_document_current_version_cannot_move_to_another_document() {
        let db = db();
        chain(&db);
        db.conn()
            .execute(
                "UPDATE documents SET current_version_id='v1' WHERE id='d1'",
                [],
            )
            .expect("işaretçi");
        scope_error(
            db.conn().execute(
                "UPDATE document_versions SET document_id='d2' WHERE id='v1'",
                [],
            ),
            "documents",
        );
        db.conn()
            .execute(
                "UPDATE documents SET current_version_id=NULL WHERE id='d1'",
                [],
            )
            .expect("çek");
        let moved = db
            .conn()
            .execute(
                "UPDATE document_versions SET document_id='d2' WHERE id='v1'",
                [],
            )
            .expect("taşıma");
        assert_eq!(moved, 1);
    }

    /// Tarama **eski** çapraz işaretçiyi sayar, silmez; boşta kalmış işaretçiyi
    /// çapraz saymaz (o çapraz okuma üretmez).
    #[test]
    fn scope_scan_counts_documents_with_a_foreign_current_version() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        write_legacy_pointer(&db, "d1", "v9");
        let counts = db.scope_violations().expect("tarama");
        assert_eq!(counts.documents, 1, "çapraz işaretçi sayılır");
        assert!(counts.total() >= 1);
        assert_eq!(count(&db, "documents"), 3, "satır silinmez");
        db.conn()
            .execute(
                "UPDATE documents SET current_version_id=NULL WHERE id='d1'",
                [],
            )
            .expect("temizle");
        write_legacy_pointer(&db, "d2", "v-yok");
        let counts = db.scope_violations().expect("tarama 2");
        assert_eq!(counts.documents, 0, "boşta kalan işaretçi çapraz sayılmaz");
    }

    /// Meşru yazma yolu (çekirdek) etkilenmez: yeni sürüm belgenin kendi
    /// işaretçisini günceller.
    #[test]
    fn adding_a_version_still_points_the_document_at_its_own_version() {
        let db = db();
        chain(&db);
        let version = db
            .add_version("d1", "c1", "b1", &"c".repeat(64), 10, "application/pdf")
            .expect("sürüm");
        let doc = db.get_document("d1").expect("belge").expect("var");
        assert_eq!(doc.current_version_id.as_deref(), Some(version.id.as_str()));
    }

    // ---- v36: çapraz işaretçi **okuma** yolunda güncel sürüm sayılmaz ----
    //
    // v35 yalnız **yeni** yazmayı kapıladı; v35 öncesi çapraz satır bugün de
    // kasada olabilir. v36 görünümü (`current_document_versions`) okuma
    // yollarında yabancı sürümü "güncel" saymaz: belge **sürümsüz** görünür
    // (fail-closed) — yanlış veri değil, veri yok.

    /// Belge künyesi, kapsam raporu ve ayrıştırma kuyruğu aynı görünümden
    /// okur: yabancı işaretçi hiçbirine "güncel sürüm" diye girmez.
    #[test]
    fn stale_cross_pointer_is_not_read_as_a_current_version() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        write_legacy_pointer(&db, "d1", "v9");
        // Künye: yabancı kimlik dönmez.
        let doc = db.get_document("d1").expect("belge").expect("var");
        assert_eq!(doc.current_version_id, None, "yabancı işaretçi okunmaz");
        let listed = db.list_documents("c1").expect("liste");
        let d1 = listed.iter().find(|d| d.id == "d1").expect("d1");
        assert_eq!(d1.current_version_id, None);
        // Kapsam raporu yabancı sürümün sayfa/segmentini saymaz.
        let report = db.coverage_report("c1").expect("rapor");
        assert_eq!(report.pages, 0, "yabancı sürümün sayfası sayılmaz");
        assert_eq!(report.segments, 0, "yabancı sürümün segmenti sayılmaz");
        assert_eq!(report.unparsed_documents, 2, "iki belge de sürümsüz");
    }

    /// Ayrıştırma kuyruğu belgeyi **yabancı** sürüm kimliğiyle listelemez
    /// (kuyruk işi yanlış sürümü ayrıştırmaya gitmez).
    #[test]
    fn stale_cross_pointer_is_not_queued_for_parse() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        // Koşusu olmayan yabancı sürüm: geçerli işaretçiyle kuyruğa girerdi.
        db.conn()
            .execute_batch(
                "INSERT INTO document_versions
                   (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
                 VALUES ('v8','d9','c2',2,'b1','dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',11,'application/pdf','2026-01-01T00:00:00Z');",
            )
            .expect("koşusuz yabancı sürüm");
        write_legacy_pointer(&db, "d2", "v8");
        assert!(
            db.documents_pending_parse("c1", 50)
                .expect("kuyruk")
                .is_empty(),
            "yabancı sürüm kimliğiyle belge listelenmemeli"
        );
    }

    /// Plan okuması yabancı sürüm numarasını "belgenin güncel sürümü" diye
    /// yazmaz; plan yine `version_superseded` der (yeniden inceleme nedeni
    /// korunur, numara uydurulmaz).
    #[test]
    fn stale_cross_pointer_is_not_the_current_version_in_redaction_reads() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        db.conn()
            .execute(
                "INSERT INTO redaction_plans
                   (id,case_id,document_version_id,render_hash,coordinate_space,plan_version,state,output_profile,created_at,updated_at)
                 VALUES ('rp1','c1','v1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','pdf_page_points',1,'approved','masked','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                [],
            )
            .expect("plan");
        write_legacy_pointer(&db, "d1", "v9");
        let plan = db
            .get_redaction_plan("rp1")
            .expect("plan okuma")
            .expect("var");
        assert_eq!(
            plan.current_version_no, None,
            "yabancı sürüm numarası yazılmaz"
        );
        assert_eq!(
            plan.reinspect_reason.as_deref(),
            Some("version_superseded"),
            "plan güncel sürümde değil"
        );
    }

    /// Alıntı denetimi yabancı metinde eşleşme bulmaz: sayfa taranmaz, hüküm
    /// `unverifiable` (INV-05: alıntı ekrandaki belgede geçmiyor).
    #[test]
    fn stale_cross_pointer_does_not_verify_a_quote_in_foreign_text() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        write_legacy_pointer(&db, "d1", "v9");
        // Yabancı segmentin metni kısa alıntı eşiğinin üstünde olmalı.
        db.conn()
            .execute(
                "UPDATE text_segments SET raw_text='gizli tutanak metni', normalized_text='gizli tutanak metni' WHERE id='s9'",
                [],
            )
            .expect("yabancı metin");
        let input = crate::intext_quotes::InTextQuoteInput {
            text: "Metinde «gizli tutanak» geçiyor.".into(),
            document_id: Some("d1".into()),
        };
        let check = db.check_in_text_quotes("c1", &input).expect("denetim");
        assert_eq!(check.exact, 0, "yabancı metinde eşleşme sayılmaz");
        assert_eq!(check.quotes[0].state, "not_found");
        assert!(
            check.quotes[0].position.is_none(),
            "yabancı segmentte konum üretilmez"
        );
        assert!(
            check.historical_versions_scanned >= 1,
            "yalnız belgenin **kendi** eski sürümü taranır"
        );
    }

    /// Gönderim önizlemesi (onayla aynı kapsam çözümü) yabancı metni
    /// göndermez; belge **nedeniyle** kapsam dışıdır (sessiz atlama yok).
    #[test]
    fn stale_cross_pointer_is_not_sent_in_the_transfer_scope() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        write_legacy_pointer(&db, "d1", "v9");
        let preview = db
            .consent_preview_masked("c1", &["d1".to_string()], &[], None)
            .expect("önizleme");
        assert_eq!(preview.documents, 0, "yabancı sürüm gönderilmez");
        assert_eq!(preview.segments, 0);
        assert_eq!(preview.bytes, 0);
        assert!(preview.sample.is_empty(), "yabancı metin örneği sızmaz");
        assert_eq!(
            preview.skipped.first().map(|skip| skip.reason.as_str()),
            Some("no_current_version")
        );
    }

    /// Çapa okumaları yabancı sürümü "belgenin güncel sürümü" diye yazmaz
    /// (numara uydurulmaz; yeniden bağlama önerisi de yabancı sürüme gitmez).
    #[test]
    fn stale_cross_pointer_is_not_the_current_version_in_anchor_reads() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        write_legacy_pointer(&db, "d1", "v9");
        let location = db.locate_anchor("a1").expect("konum").expect("var");
        assert_eq!(location.current_version_id, None);
        assert!(!location.is_current_version);
        assert_eq!(location.current_version_no, None);
        let suggestion = db.rebind_suggestion("a1").expect("öneri");
        assert_eq!(suggestion.current_version_id, None);
    }

    /// v37: atıf kontrolü **kendi dosyasının** sürümünü gösterir. Ölçülen
    /// kusur (düzeltme öncesi, v36): B dosyasının sürümüne A dosyası adına
    /// satır yazılabiliyor ve B dosyasının listesinde A'nın metni görünüyordu.
    #[test]
    fn a_citation_check_cannot_reference_another_cases_version() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        let write = |id: &str, case: Option<&str>, version: &str| {
            db.conn().execute(
                "INSERT INTO citation_checks
                   (id,case_id,input_document_version_id,extracted_citation,identity_match,
                    quote_match,meaning_review,reason_codes,created_at)
                 VALUES (?1,?2,?3,'gizli tutanak metni','unknown','not_checked','not_reviewed','[]',
                         '2026-01-01T00:00:00Z')",
                rusqlite::params![id, case, version],
            )
        };
        write("cc-ok", Some("c1"), "v1").expect("aynı dosyanın sürümü meşru");
        // Var olmayan sürüm kimliği denetim izinde kalabilir (0033/0034
        // gevşekliği); kapı yalnız **var olan çapraz** sürümü reddeder.
        write("cc-gone", Some("c1"), "silinmis-surum").expect("var olmayan kimlik meşru");
        scope_error(write("cc-x", Some("c1"), "v9"), "citation_checks");
        // Satır kendi dosyasını **taşımak zorunda** (NULL yazılamaz).
        scope_error(write("cc-null", None, "v1"), "citation_checks");
        assert_eq!(
            count(&db, "citation_checks"),
            2,
            "yalnız meşru satırlar yazıldı"
        );
        // UPDATE ile taşıma da kapıda durur.
        scope_error(
            db.conn().execute(
                "UPDATE citation_checks SET input_document_version_id='v9' WHERE id='cc-ok'",
                [],
            ),
            "citation_checks",
        );
        scope_error(
            db.conn().execute(
                "UPDATE citation_checks SET case_id='c2' WHERE id='cc-ok'",
                [],
            ),
            "citation_checks",
        );
        // Anlam incelemesi (kapsam dışı sütun) kapıya takılmaz.
        db.conn()
            .execute(
                "UPDATE citation_checks SET meaning_review='accurate' WHERE id='cc-ok'",
                [],
            )
            .expect("anlam incelemesi serbest");
    }

    /// v37: dışa aktarma kalemi, **üst kaydın** dosyasının sürümünü gösterir.
    /// Ölçülen kusur (düzeltme öncesi, v36): `record_export` çekirdek kapısı
    /// doğruydu ama doğrudan SQL çapraz sürümü yazabiliyor, `list_export_items`
    /// A'nın kayıt defterinde **B'nin belge adını** gösteriyordu.
    #[test]
    fn an_export_item_cannot_reference_another_cases_version() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        let export = db
            .record_export(
                "c1",
                "case_report",
                Some("case-report@1"),
                &[],
                "folder",
                "",
                "hash",
            )
            .expect("dışa aktarma");
        let item = |id: &str, version: &str| {
            db.conn().execute(
                "INSERT INTO export_items
                   (id,export_id,source_version_id,output_page_range,citation_label,redaction_state)
                 VALUES (?1,?2,?3,NULL,NULL,'none')",
                rusqlite::params![id, &export.id, version],
            )
        };
        item("ei-ok", "v1").expect("aynı dosyanın sürümü meşru");
        item("ei-gone", "silinmis-surum").expect("var olmayan iz meşru");
        scope_error(item("ei-x", "v9"), "export_items");
        assert_eq!(count(&db, "export_items"), 2, "yalnız meşru kalemler");
        scope_error(
            db.conn().execute(
                "UPDATE export_items SET source_version_id='v9' WHERE id='ei-ok'",
                [],
            ),
            "export_items",
        );
        // Okuma, çapraz satırı gösteremez: varsa da adı yabancı belgeden gelir —
        // bu yüzden kapı yazmayı reddeder (aşağıdaki tarama testi).
    }

    /// v37 taraması: kapı düşürülüp yazılmış çapraz satırlar **sayılır**
    /// (kimlik/içerik taşımadan). Gerçek bir saldırgan da tetikleyiciyi
    /// düşürebilir; tarama o durumu görünür kılar.
    #[test]
    fn scope_violations_reports_cross_case_version_traces() {
        let db = db();
        chain(&db);
        foreign_chain(&db);
        let export = db
            .record_export(
                "c1",
                "case_report",
                Some("case-report@1"),
                &[],
                "folder",
                "",
                "hash",
            )
            .expect("dışa aktarma");
        let names = [
            "scope_guard_citation_checks_bi",
            "scope_guard_citation_checks_bu",
            "scope_guard_export_items_bi",
            "scope_guard_export_items_bu",
        ];
        let mut bodies = Vec::new();
        for name in names {
            bodies.push(
                db.trigger_sql(name)
                    .expect("gövde")
                    .unwrap_or_else(|| panic!("{name} yok")),
            );
            db.conn()
                .execute_batch(&format!("DROP TRIGGER {name};"))
                .expect("düşür");
        }
        db.conn()
            .execute(
                "INSERT INTO citation_checks
                   (id,case_id,input_document_version_id,extracted_citation,identity_match,
                    quote_match,meaning_review,reason_codes,created_at)
                 VALUES ('cc-lege','c1','v9','gizli tutanak metni','unknown','not_checked',
                         'not_reviewed','[]','2026-01-01T00:00:00Z')",
                [],
            )
            .expect("eski çapraz satır");
        db.conn()
            .execute(
                "INSERT INTO export_items
                   (id,export_id,source_version_id,output_page_range,citation_label,redaction_state)
                 VALUES ('ei-lege',?1,'v9',NULL,NULL,'none')",
                [&export.id],
            )
            .expect("eski çapraz kalem");
        db.conn()
            .execute_batch(&format!("{};", bodies.join(";\n")))
            .expect("geri kur");
        let counts = db.scope_violations().expect("tarama");
        assert_eq!(counts.citation_checks, 1);
        assert_eq!(counts.export_items, 1);
        assert_eq!(counts.total(), 2);
        // Satırlar silinmez; tarama yalnız sayar.
        assert_eq!(count(&db, "citation_checks"), 1);
        assert_eq!(count(&db, "export_items"), 1);
        // Okuma iki anahtarlıdır (dosya + sürüm): satır **yazan** dosyanın
        // listesinde kalır. Bu yüzden v37 öncesi çapraz satır "çapraz" diye
        // ayırt edilemez (satırda yazan dosya bilgisi hiç yoktu) — dürüst sınır;
        // temizliği ürün kararıdır.
        assert_eq!(
            db.list_citation_checks("c1", "v9").expect("liste").len(),
            1,
            "göç satırı başvurduğu sürümün dosyasına atfeder"
        );
        let items = db.list_export_items(&export.id).expect("kalemler");
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].document_title.as_deref(),
            Some("Diger dosya belgesi"),
            "eski çapraz satır yabancı künyeyi gösterir — temizliği ürün kararı"
        );
    }

    /// Şemadan türetim: görünüm **tek tanım** olarak durur ve kendi belgesine
    /// ait olma koşulunu taşır (gevşetilirse bütün okuma yolları sessizce
    /// yabancı sürüme döner).
    #[test]
    fn current_version_view_requires_the_same_document() {
        let db = db();
        let sql: String = db
            .conn()
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='view' AND name='current_document_versions'",
                [],
                |row| row.get(0),
            )
            .expect("current_document_versions görünümü yok");
        assert!(
            sql.contains("v.document_id = d.id"),
            "görünüm kendi belgesini denetlemiyor: {sql}"
        );
    }
}
