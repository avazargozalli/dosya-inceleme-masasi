//! Dosya kapsamı **veritabanı düzeyinde** (şema v31 + v32): tetikleyiciler ve
//! mevcut satır taraması.
//!
//! Neden: çekirdek yazma yolları (`add_version_with_parent`, `start_parse_run`,
//! `store_pages`, `create_anchor`, `create_claim`, `link_evidence`, …) kapsamı
//! kendi sorgularında doğrular, ama **doğrudan SQL** yazan bir yol bu kapıyı
//! atlar. Kapsam ihlali P0 kabul engelidir: A dosyasının metni B dosyasının
//! aramasında bulunur. Bu yüzden kural artık şemada da durur
//! (`migrations/0031_scope_guards.sql` zincir, `migrations/0032_reference_scope_guards.sql`
//! kanıt/iddia referansları).
//!
//! İki sınıf kapı vardır:
//!
//! * **zincir** (v31) — `case_id` taşıyan satırın üst kaydı aynı dosyada olmalı
//!   (belge → sürüm → koşu → segment → çapa).
//! * **referans** (v32) — bir satırın **işaret ettiği** kanıt/iddia kendi
//!   dosyasından olmalı (`claims.source_anchor_id`, `evidence_links`,
//!   `event_sources`, `claim_relations`, `tasks.source_anchor_id`). Aksi hâlde
//!   B dosyasındaki belge silinince A'nın kanıt bağı sessizce yok olur ve
//!   kimlikle okuyan yollar (`anchor_locate`) A dosyasında B'nin metnini
//!   gösterir.
//!
//! Bu modül iki şeyi taşır:
//!
//! * `scope_violations()` — **bu şema sürümünden önce** yazılmış aykırı
//!   satırların sayısı (tetikleyiciler yalnız yeni yazmayı engeller; eski veri
//!   kendiliğinden düzelmez). Kimlik/İçerik taşımaz; yalnız sayı.
//! * Kapı testleri — her kapsam sütunu taşıyan tablonun tetikleyicisi **var mı**
//!   (şemadan türetilir; yeni tablo sessizce kapısız kalamaz).

#[cfg(test)]
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{Database, StorageError};

/// Kapsam zincirinin veritabanı düzeyinde kapılı tabloları. Şemadan türetilen
/// test bu liste ile veritabanının gerçeğini karşılaştırır.
///
/// İlk sekizi **zincir** kapısıdır (v31): satırın `case_id`'si üst kaydının
/// dosyasıyla aynı olmalı. Son beşi **referans** kapısıdır (v32): satırın
/// işaret ettiği kanıt/iddia kendi dosyasından olmalı.
#[cfg(test)]
pub(crate) const GUARDED_TABLES: [&str; 13] = [
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
                                     WHERE d.id = dv.document_id AND d.case_id = dv.case_id)",
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
                            WHERE pr.id = rp.parse_run_id AND pr.case_id = rp.case_id))",
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
    /// İki kural: (1) `case_id` + zincir/referans sütunu; (2) `case_id`
    /// taşımayan ama iki kapsamlı kaydı bağlayan tablo (`event_sources`:
    /// `event_id` + `source_anchor_id`).
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
        ];
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
            }
        }
    }
}
