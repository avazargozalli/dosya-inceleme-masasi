//! Dosya kapsamı **veritabanı düzeyinde** (şema v31): tetikleyiciler ve
//! mevcut satır taraması.
//!
//! Neden: çekirdek yazma yolları (`add_version_with_parent`, `start_parse_run`,
//! `store_pages`, `create_anchor`, …) kapsamı kendi sorgularında doğrular, ama
//! **doğrudan SQL** yazan bir yol bu kapıyı atlar. Kapsam ihlali P0 kabul
//! engelidir: A dosyasının metni B dosyasının aramasında bulunur. Bu yüzden
//! kural artık şemada da durur (`migrations/0031_scope_guards.sql`).
//!
//! Bu modül iki şeyi taşır:
//!
//! * `scope_violations()` — **bu şema sürümünden önce** yazılmış aykırı
//!   satırların sayısı (tetikleyiciler yalnız yeni yazmayı engeller; eski veri
//!   kendiliğinden düzelmez). Kimlik/İçerik taşımaz; yalnız sayı.
//! * Kapı testleri — her `case_id` + üst-kayıt sütunu taşıyan tablonun
//!   tetikleyicisi **var mı** (şemadan türetilir; yeni tablo sessizce kapısız
//!   kalamaz).

#[cfg(test)]
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{Database, StorageError};

/// Kapsam zincirinin veritabanı düzeyinde kapılı tabloları. Şemadan türetilen
/// test bu liste ile veritabanının gerçeğini karşılaştırır.
#[cfg(test)]
pub(crate) const GUARDED_TABLES: [&str; 8] = [
    "document_versions",
    "parse_runs",
    "text_segments",
    "source_anchors",
    "source_marks",
    "redaction_plans",
    "document_tags",
    "embeddings",
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

    /// Şemadan türetilen kapı: `case_id` **ve** bir üst-kayıt sütunu taşıyan her
    /// tablonun iki tetikleyicisi olmalı (INSERT + UPDATE) ve gövdesi kendi
    /// tablosunu `DIM_SCOPE:` ile adlandırmalı. Yeni bir tablo sessizce kapısız
    /// kalamaz.
    #[test]
    fn every_case_scoped_table_has_guard_triggers() {
        let db = db();
        let parent_columns = [
            "document_id",
            "document_version_id",
            "parse_run_id",
            "segment_id",
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
            if columns.iter().any(|c| c == "case_id")
                && columns.iter().any(|c| parent_columns.contains(&c.as_str()))
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
