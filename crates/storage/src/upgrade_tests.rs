//! F16-T03 — atomik güncelleme / geri dönüş tatbikatı.
//!
//! Tatbikat gerçek dosyalarla çalışır: eski şemada (v12) bir kasa kurulur,
//! güncelleme uygulanır, migrasyon yarıda bırakılır, yedek bozulur ve geri
//! dönüş denenir. Hiçbir adım taklit edilmez; her adım gerçek SQLCipher
//! dosyasını açar ve okur.

use super::*;

fn key() -> DatabaseKey {
    DatabaseKey::new([9u8; 32])
}

/// `max_version`a kadar **gerçek** migrasyon dosyalarıyla eski şemayı kurar.
fn create_schema_at(path: &Path, max_version: u32) -> Connection {
    let conn = Connection::open(path).expect("dosya açılır");
    apply_key_and_pragmas(&conn, &key()).expect("anahtar");
    parsing::register_search_functions(&conn).expect("arama fonksiyonları");
    conn.execute_batch(
        "CREATE TABLE schema_migrations (
           version INTEGER PRIMARY KEY,
           name TEXT NOT NULL,
           checksum TEXT NOT NULL,
           applied_at TEXT NOT NULL
         );",
    )
    .expect("sürüm tablosu");
    for migration in MIGRATIONS.iter().filter(|m| m.version <= max_version) {
        conn.execute_batch(migration.sql).expect("eski migrasyon");
        conn.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                migration.version,
                migration.name,
                hex::encode(Sha256::digest(migration.sql.as_bytes())),
                now_iso()
            ],
        )
        .expect("sürüm kaydı");
    }
    conn
}

/// Güncelleme öncesi durumu kurar: eski şemada (v12) bir kasa ve bir dosya
/// kaydı.
fn create_v12(path: &Path) {
    let conn = create_schema_at(path, 12);
    conn.execute(
        "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
         VALUES ('c-eski','Eski dosya','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
        [],
    )
    .expect("dosya kaydı");
    // WAL kalıntısı bırakma: sonraki adımlar dosyayı tek başına okuyabilsin.
    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
}

fn case_titles(path: &Path) -> Vec<String> {
    let version = Database::verify_database_file(path, &key()).expect("doğrula");
    assert!(version <= CURRENT_SCHEMA_VERSION);
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("aç");
    apply_key_and_pragmas(&conn, &key()).expect("anahtar");
    let mut stmt = conn
        .prepare("SELECT title FROM cases ORDER BY title")
        .expect("hazırla");
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("sorgu");
    rows.map(|r| r.expect("satır")).collect()
}

#[test]
fn upgrade_takes_verified_backup_before_migrating() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);
    let backup = pre_migration_backup_path(&path, 12);
    assert!(!backup.exists(), "güncellemeden önce yedek olmamalı");

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    assert_eq!(db.pre_migration_backup(), Some(backup.as_path()));
    assert!(backup.is_file(), "yedek dosyası yazılmalı");
    assert_eq!(
        Database::verify_database_file(&backup, &key()).expect("yedek doğrulanır"),
        12,
        "yedek eski şemayı taşımalı"
    );
    drop(db);

    // Güncelleme veriyi korur, yedek de eski veriyi tek başına taşır.
    assert_eq!(case_titles(&path), vec!["Eski dosya".to_string()]);
    assert_eq!(case_titles(&backup), vec!["Eski dosya".to_string()]);
}

#[test]
fn upgrade_reports_cross_case_rows_left_by_the_previous_schema() {
    // Şema v30'da kapsam tetikleyicisi yoktu: çapraz dosya satırı yazmak
    // mümkündü. v31 tetikleyicileri yalnız **yeni** yazmayı engeller; eski
    // satır güncelleme anında sayı olarak denetime düşer (kimlik/içerik yok).
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 30);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('c2','Diğer','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','aa11',1,10,'ref','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','Belge','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',10,'application/pdf','2026-01-01T00:00:00Z'),
                    ('v-old','d1','c2',2,'b1','eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',10,'application/pdf','2026-01-01T00:00:00Z');
             INSERT INTO parse_runs (id,document_version_id,case_id,engine,engine_version,settings_hash,status,created_at)
             VALUES ('r1','v1','c1','pymupdf','1.24','h','succeeded','2026-01-01T00:00:00Z');
             INSERT INTO pages (id,parse_run_id,page_index,text_origin)
             VALUES ('p1','r1',0,'native');
             INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
             VALUES ('s-old','r1','p1','c2','metin','metin','2026-01-01T00:00:00Z');",
        )
        .expect("v30 çapraz satırlar");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let metadata: String = db
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE operation='scope_violations'",
            [],
            |row| row.get(0),
        )
        .expect("kapsam taraması denetime düşmeli");
    let parsed: serde_json::Value = serde_json::from_str(&metadata).expect("metadata json");
    assert_eq!(parsed["documentVersions"], 1, "çapraz sürüm sayılır");
    assert_eq!(parsed["textSegments"], 1, "çapraz segment sayılır");
    assert_eq!(parsed["parseRuns"], 0);
    // Satırlar **silinmez**: tarama yalnız sayar.
    let versions: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM document_versions", [], |r| r.get(0))
        .expect("sayım");
    assert_eq!(versions, 2);
    // Yeni çapraz yazma artık kapıda durur.
    let rejected = db.conn().execute(
        "INSERT INTO document_versions
           (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
         VALUES ('v-new','d1','c2',3,'b1','ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',10,'application/pdf','2026-01-01T00:00:00Z')",
        [],
    );
    assert!(
        rejected.is_err(),
        "v31 kapısı yeni çapraz yazmayı reddetmeli"
    );
}

#[test]
fn upgrade_reports_cross_case_references_left_by_the_previous_schema() {
    // Şema v31'de zincir kapısı vardı ama **referans** kapısı yoktu: iddia,
    // kanıt bağı, olay kaynağı, iddia ilişkisi ve görev başka dosyanın çapasına
    // bağlanabiliyordu. v32 tetikleyicileri yalnız **yeni** yazmayı engeller;
    // eski satır güncelleme anında sayı olarak denetime düşer (kimlik/içerik yok).
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 31);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('c2','Diger','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','aa11',1,10,'ref','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','Belge','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('d2','c2','Diger belge','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',10,'application/pdf','2026-01-01T00:00:00Z'),
                    ('v2','d2','c2',1,'b1','eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',10,'application/pdf','2026-01-01T00:00:00Z');
             INSERT INTO parse_runs (id,document_version_id,case_id,engine,engine_version,settings_hash,status,created_at)
             VALUES ('r1','v1','c1','pymupdf','1.24','h','succeeded','2026-01-01T00:00:00Z'),
                    ('r2','v2','c2','pymupdf','1.24','h','succeeded','2026-01-01T00:00:00Z');
             INSERT INTO pages (id,parse_run_id,page_index,text_origin)
             VALUES ('p1','r1',0,'native'),('p2','r2',0,'native');
             INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
             VALUES ('s1','r1','p1','c1','itiraz','itiraz','2026-01-01T00:00:00Z'),
                    ('s2','r2','p2','c2','gizli','gizli','2026-01-01T00:00:00Z');
             INSERT INTO source_anchors
               (id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,
                exact_quote,quote_hash,created_at,updated_at)
             VALUES ('a1','c1','v1','r1','p1','s1',0,6,'itiraz','bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('a2','c2','v2','r2','p2','s2',0,5,'gizli','ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO claims (id,case_id,claim_text,claim_kind,status,created_at,updated_at)
             VALUES ('k1','c1','Iddia','allegation','draft','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('k2','c2','Diger iddia','allegation','draft','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO events (id,case_id,event_kind,date_precision,date_role,title,created_at,updated_at)
             VALUES ('e1','c1','payment','day','event','Kira odendi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             -- v31'de kabul edilen çapraz referanslar.
             UPDATE claims SET source_anchor_id='a2' WHERE id='k1';
             INSERT INTO evidence_links (id,case_id,claim_id,source_anchor_id,relation,rationale,review_state,created_at,updated_at)
             VALUES ('el-old','c1','k1','a2','supports_candidate','','accepted','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO event_sources (id,event_id,source_anchor_id,relation,user_confirmed)
             VALUES ('es-old','e1','a2','supports',1);
             INSERT INTO claim_relations (id,case_id,claim_id,related_claim_id,relation,rationale,created_at,updated_at)
             VALUES ('cr-old','c1','k1','k2','response_to','','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO tasks (id,case_id,title,source_anchor_id,created_at,updated_at)
             VALUES ('t-old','c1','Dilekce','a2','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
        )
        .expect("v31 çapraz referanslar");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let metadata: String = db
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE operation='scope_violations'",
            [],
            |row| row.get(0),
        )
        .expect("kapsam taraması denetime düşmeli");
    let parsed: serde_json::Value = serde_json::from_str(&metadata).expect("metadata json");
    assert_eq!(parsed["claims"], 1, "çapraz iddia çapası sayılır");
    assert_eq!(parsed["evidenceLinks"], 1);
    assert_eq!(parsed["eventSources"], 1);
    assert_eq!(parsed["claimRelations"], 1);
    assert_eq!(parsed["tasks"], 1);
    assert_eq!(parsed["documentVersions"], 0, "zincir temiz");
    // Satırlar **silinmez**: tarama yalnız sayar.
    let links: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM evidence_links", [], |r| r.get(0))
        .expect("sayım");
    assert_eq!(links, 1);
    // Yeni çapraz referans artık kapıda durur (E_DB_SCOPE).
    let rejected = db.conn().execute(
        "INSERT INTO evidence_links (id,case_id,claim_id,source_anchor_id,relation,rationale,review_state,created_at,updated_at)
         VALUES ('el-new','c1','k2','a1','supports_candidate','','accepted','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
        [],
    );
    match rejected.map_err(StorageError::from) {
        Err(StorageError::ScopeViolation { table }) => {
            assert_eq!(table, "evidence_links")
        }
        other => panic!("v32 kapısı yeni çapraz yazmayı reddetmeli: {other:?}"),
    }
}

#[test]
fn upgrade_reports_cross_case_json_anchor_lists_left_by_the_previous_schema() {
    // Şema v32'de tek sütunlu kanıt referansları kapılıydı ama **JSON dizi**
    // sütunları (`source_anchor_ids`) kapı dışındaydı: not, revizyon, inceleme
    // öğesi ve taslak başka dosyanın çapasını "kaynağım" diye taşıyabiliyordu.
    // v33 tetikleyicileri yalnız **yeni** yazmayı engeller; eski satırlar
    // güncelleme anında sayı olarak denetime düşer (kimlik/içerik yok).
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 32);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('c2','Diger','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','aa11',1,10,'ref','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','Belge','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('d2','c2','Diger belge','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',10,'application/pdf','2026-01-01T00:00:00Z'),
                    ('v2','d2','c2',1,'b1','eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',10,'application/pdf','2026-01-01T00:00:00Z');
             INSERT INTO parse_runs (id,document_version_id,case_id,engine,engine_version,settings_hash,status,created_at)
             VALUES ('r1','v1','c1','pymupdf','1.24','h','succeeded','2026-01-01T00:00:00Z'),
                    ('r2','v2','c2','pymupdf','1.24','h','succeeded','2026-01-01T00:00:00Z');
             INSERT INTO pages (id,parse_run_id,page_index,text_origin)
             VALUES ('p1','r1',0,'native'),('p2','r2',0,'native');
             INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
             VALUES ('s1','r1','p1','c1','itiraz','itiraz','2026-01-01T00:00:00Z'),
                    ('s2','r2','p2','c2','gizli','gizli','2026-01-01T00:00:00Z');
             INSERT INTO source_anchors
               (id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,
                exact_quote,quote_hash,created_at,updated_at)
             VALUES ('a1','c1','v1','r1','p1','s1',0,6,'itiraz','bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('a2','c2','v2','r2','p2','s2',0,5,'gizli','ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             -- v32'de kabul edilen çapraz JSON dizi referansları.
             INSERT INTO notes (id,case_id,title,body_ast,author_kind,visibility_label,source_anchor_ids,created_at,updated_at)
             VALUES ('n-old','c1','Eski not','{\"v\":1,\"text\":\"metin\"}','user','case_private','[\"a2\"]',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO note_revisions (note_id,revision_no,title,body_ast,source_anchor_ids,origin,created_at)
             VALUES ('n-old',2,'Eski not','{\"v\":1,\"text\":\"metin\"}','[\"a2\"]','edit','2026-01-01T00:00:00Z');
             INSERT INTO review_items (id,case_id,item_kind,payload,source_anchor_ids,created_at,updated_at)
             VALUES ('i-old','c1','claim_candidate','{}','[\"a2\"]','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO note_drafts (case_id,note_id,title,body,source_anchor_ids,updated_at)
             VALUES ('c1','','Taslak','yarim','[\"a2\"]','2026-01-01T00:00:00Z');",
        )
        .expect("v32 çapraz JSON dizileri");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let metadata: String = db
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE operation='scope_violations'",
            [],
            |row| row.get(0),
        )
        .expect("kapsam taraması denetime düşmeli");
    let parsed: serde_json::Value = serde_json::from_str(&metadata).expect("metadata json");
    assert_eq!(parsed["notes"], 1, "çapraz not kaynağı sayılır");
    assert_eq!(parsed["noteRevisions"], 1);
    assert_eq!(parsed["reviewItems"], 1);
    assert_eq!(parsed["noteDrafts"], 1);
    assert_eq!(parsed["claims"], 0, "v32 sınıfı temiz");
    // Satırlar **silinmez**: tarama yalnız sayar.
    let notes: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
        .expect("sayım");
    assert_eq!(notes, 1);
    // Yeni çapraz dizi artık kapıda durur (E_DB_SCOPE).
    let rejected = db.conn().execute(
        "INSERT INTO notes (id,case_id,title,body_ast,author_kind,visibility_label,source_anchor_ids,created_at,updated_at)
         VALUES ('n-new','c1','Yeni not','{\"v\":1,\"text\":\"metin\"}','user','case_private','[\"a2\"]',
                 '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
        [],
    );
    match rejected.map_err(StorageError::from) {
        Err(StorageError::ScopeViolation { table }) => assert_eq!(table, "notes"),
        other => panic!("v33 kapısı yeni çapraz diziyi reddetmeli: {other:?}"),
    }
}

#[test]
fn upgrade_reports_cross_case_output_and_context_links_left_by_the_previous_schema() {
    // Şema v33'te zincir, kanıt referansları ve JSON dizi listeleri kapılıydı
    // ama **soy ağacı** (`document_versions.parent_version_id`), **maskeleme
    // çıktısı** (`redaction_plans.output_version_id`) ve **gönderim sicili**
    // (`ai_context_items`) kapı dışındaydı: A dosyasının planı/çalıştırması B
    // dosyasının sürümünü/çapasını gösterebiliyordu. v34 tetikleyicileri yalnız
    // **yeni** yazmayı engeller; eski satırlar güncelleme anında sayı olarak
    // denetime düşer (kimlik/içerik yok).
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 33);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('c2','Diger','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','aa11',1,10,'ref','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','Belge','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('d2','c2','Diger belge','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',10,'application/pdf','2026-01-01T00:00:00Z'),
                    ('v2','d2','c2',1,'b1','eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',10,'application/pdf','2026-01-01T00:00:00Z');
             INSERT INTO parse_runs (id,document_version_id,case_id,engine,engine_version,settings_hash,status,created_at)
             VALUES ('r1','v1','c1','pymupdf','1.24','h','succeeded','2026-01-01T00:00:00Z'),
                    ('r2','v2','c2','pymupdf','1.24','h','succeeded','2026-01-01T00:00:00Z');
             INSERT INTO pages (id,parse_run_id,page_index,text_origin)
             VALUES ('p1','r1',0,'native'),('p2','r2',0,'native');
             INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
             VALUES ('s1','r1','p1','c1','itiraz','itiraz','2026-01-01T00:00:00Z'),
                    ('s2','r2','p2','c2','gizli','gizli','2026-01-01T00:00:00Z');
             INSERT INTO source_anchors
               (id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,
                exact_quote,quote_hash,created_at,updated_at)
             VALUES ('a1','c1','v1','r1','p1','s1',0,6,'itiraz','bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('a2','c2','v2','r2','p2','s2',0,5,'gizli','ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO ai_runs (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,status,usage_report,created_at)
             VALUES ('run1','c1','summarize','m','h','p1','o1','running','{\"billing\":\"unknown\"}','2026-01-01T00:00:00Z');
             -- v33'te kabul edilen çapraz bağlar.
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at,parent_version_id)
             VALUES ('v-old','d1','c1',2,'b1','dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',10,
                     'application/pdf','2026-01-01T00:00:00Z','v2');
             INSERT INTO redaction_plans
               (id,case_id,document_version_id,render_hash,coordinate_space,plan_version,state,output_profile,created_at,updated_at,output_version_id,output_version_ref)
             VALUES ('p-old','c1','v1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','pdf_page_points',1,
                     'applied','share','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','v2','v2');
             INSERT INTO ai_context_items (id,ai_run_id,document_version_id,source_anchor_id,segment_ids,inclusion_reason,bytes_sent)
             VALUES ('ctx-old','run1','v2','a2','[]','onaylanan belge',7);
             INSERT INTO transfer_segments (id,ai_run_id,outbound_segment_id,document_version_id,outbound_text,outbound_hash)
             VALUES ('t-old','run1','s2','v2','gizli','hash');",
        )
        .expect("v33 çapraz bağlar");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let metadata: String = db
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE operation='scope_violations'",
            [],
            |row| row.get(0),
        )
        .expect("kapsam taraması denetime düşmeli");
    let parsed: serde_json::Value = serde_json::from_str(&metadata).expect("metadata json");
    assert_eq!(parsed["documentVersions"], 1, "soy ağacı sayılır");
    assert_eq!(parsed["redactionPlans"], 1, "maskeleme çıktısı sayılır");
    assert_eq!(parsed["aiContextItems"], 1, "gönderim sicili sayılır");
    assert_eq!(
        parsed["transferSegments"], 1,
        "giden segment sicili sayılır"
    );
    assert_eq!(parsed["notes"], 0, "v33 sınıfı temiz");
    // Satırlar **silinmez**: tarama yalnız sayar.
    let contexts: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM ai_context_items", [], |r| r.get(0))
        .expect("sayım");
    assert_eq!(contexts, 1);

    // Yeni çapraz bağlar artık kapıda durur (E_DB_SCOPE) — her sınıf ayrı.
    for (sql, table) in [
        (
            "UPDATE ai_context_items SET source_anchor_id='a1' WHERE id='ctx-old'",
            "ai_context_items",
        ),
        (
            "INSERT INTO ai_context_items (id,ai_run_id,document_version_id,source_anchor_id,segment_ids,inclusion_reason,bytes_sent)
             VALUES ('ctx-new','run1','v2',NULL,'[]','onaylanan belge',7)",
            "ai_context_items",
        ),
        (
            "INSERT INTO redaction_plans
               (id,case_id,document_version_id,render_hash,coordinate_space,plan_version,state,output_profile,created_at,updated_at,output_version_id,output_version_ref)
             VALUES ('p-new','c1','v1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','pdf_page_points',1,
                     'applied','share','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','v2','v2')",
            "redaction_plans",
        ),
        (
            "INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at,parent_version_id)
             VALUES ('v-new','d1','c1',3,'b1','cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',10,
                     'application/pdf','2026-01-01T00:00:00Z','v2')",
            "document_versions",
        ),
        (
            "INSERT INTO transfer_segments (id,ai_run_id,outbound_segment_id,document_version_id,outbound_text,outbound_hash)
             VALUES ('t-new','run1','s2','v2','gizli','bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb')",
            "transfer_segments",
        ),
    ] {
        let rejected = db.conn().execute(sql, []);
        match rejected.map_err(StorageError::from) {
            Err(StorageError::ScopeViolation { table: found }) => assert_eq!(found, table),
            other => panic!("v34 kapısı yeni çapraz bağı reddetmeli ({table}): {other:?}"),
        }
    }
    // Aynı dosyadaki meşru yollar açık kalır.
    db.conn()
        .execute(
            "INSERT INTO ai_context_items (id,ai_run_id,document_version_id,source_anchor_id,segment_ids,inclusion_reason,bytes_sent)
             VALUES ('ctx-ok','run1','v1','a1','[\"s1\"]','onaylanan belge',7)",
            [],
        )
        .expect("aynı dosyanın bağı meşru");
}

/// v34'te zincir, referanslar ve JSON diziler kapılıydı ama
/// `documents.current_version_id` **kendi belgesine** bağlı değildi: tek bir
/// doğrudan `UPDATE` belgeyi başka dosyanın sürümüne çevirebiliyordu (ölçüldü:
/// A'nın alıntısı B'nin metninde `exact` dönüyor, kapsam raporu B'nin
/// sayfalarını A'da sayıyordu). v35 tetikleyicileri yalnız **yeni** yazmayı
/// engeller; eski satır güncelleme anında sayı olarak denetime düşer
/// (kimlik/içerik yok).
#[test]
fn upgrade_reports_a_document_pointing_at_another_documents_version() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 34);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('c2','Diger','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','aa11',1,10,'ref','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','Belge','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('d2','c1','Kopya','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('d9','c2','Diger dosya','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',10,'application/pdf','2026-01-01T00:00:00Z'),
                    ('v2','d2','c1',1,'b1','bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',10,'application/pdf','2026-01-01T00:00:00Z'),
                    ('v9','d9','c2',1,'b1','eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',10,'application/pdf','2026-01-01T00:00:00Z');
             -- v34'te kabul edilen işaretçiler: (1) başka dosyanın sürümü,
             -- (2) aynı dosyada başka belgenin sürümü.
             UPDATE documents SET current_version_id='v9' WHERE id='d1';
             UPDATE documents SET current_version_id='v1' WHERE id='d2';",
        )
        .expect("v34 bozuk işaretçiler");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let metadata: String = db
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE operation='scope_violations'",
            [],
            |row| row.get(0),
        )
        .expect("kapsam taraması denetime düşmeli");
    let parsed: serde_json::Value = serde_json::from_str(&metadata).expect("metadata json");
    assert_eq!(parsed["documents"], 2, "iki bozuk işaretçi sayılır");
    // Satırlar **silinmez**: tarama yalnız sayar.
    let documents: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM documents", [], |r| r.get(0))
        .expect("sayım");
    assert_eq!(documents, 3);
    // Yeni çapraz işaretçi kapıda durur (E_DB_SCOPE).
    let rejected = db.conn().execute(
        "UPDATE documents SET current_version_id='v2' WHERE id='d1'",
        [],
    );
    match rejected.map_err(StorageError::from) {
        Err(StorageError::ScopeViolation { table }) => assert_eq!(table, "documents"),
        other => panic!("v35 kapısı yeni çapraz işaretçiyi reddetmeli: {other:?}"),
    }
    // Kendi sürümüne dönmek serbest.
    db.conn()
        .execute(
            "UPDATE documents SET current_version_id='v2' WHERE id='d2'",
            [],
        )
        .expect("kendi sürümü meşru");
}

/// v36'da iki **sürüm izi sicili** dosya sınırını bilmiyordu: atıf kontrolü
/// (çekirdek **ve** şema) hiçbir kapı taşımıyordu; dışa aktarma kalemi yalnız
/// çekirdekte doğrulanıyordu. Ölçüldü: A'nın belgesinden çıkarılmış metin B
/// dosyasının ekranında göründü, A'nın kayıt defteri B'nin belge adını yazdı.
/// v37 göçü atıf satırını **başvurduğu sürümün** dosyasına atfeder (başka bilgi
/// yoktu) ve sürümü silinmiş satırı "aidiyeti yazılmamış" sayar; çapraz dışa
/// aktarma kalemi olduğu gibi kalır ve sayılır. Yeni çapraz yazma kapıda durur.
#[test]
fn upgrade_reports_version_traces_left_by_the_previous_schema() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 36);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('c2','Diger','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','aa11',1,10,'ref','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','Kira sozlesmesi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('d9','c2','Diger dosya belgesi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',10,'application/pdf','2026-01-01T00:00:00Z'),
                    ('v9','d9','c2',1,'b1','eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',10,'application/pdf','2026-01-01T00:00:00Z');
             INSERT INTO exports (id,case_id,type,status,destination_kind,output_hash,created_at,completed_at)
             VALUES ('x1','c1','case_report','ready','folder','hash','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO export_items (id,export_id,source_version_id,redaction_state)
             VALUES ('xi1','x1','v9','none');
             INSERT INTO citation_checks
               (id,input_document_version_id,extracted_citation,identity_match,quote_match,
                meaning_review,reason_codes,created_at)
             VALUES ('cc1','v9','gizli tutanak metni','unknown','not_checked','not_reviewed','[]',
                     '2026-01-01T00:00:00Z'),
                    ('cc2','silinmis-surum','baska metin','unknown','not_checked','not_reviewed','[]',
                     '2026-01-01T00:00:00Z');",
        )
        .expect("v36 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let metadata: String = db
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE operation='scope_violations'",
            [],
            |row| row.get(0),
        )
        .expect("kapsam taraması denetime düşmeli");
    let parsed: serde_json::Value = serde_json::from_str(&metadata).expect("metadata json");
    assert_eq!(parsed["exportItems"], 1, "çapraz kalem sayılır");
    assert_eq!(
        parsed["citationChecks"], 1,
        "aidiyeti yazılmamış satır sayılır (çapraz satır göçle atfedildi)"
    );
    // Göç atfı: var olan sürümün dosyası yazılır; sürümü silinmiş satırda NULL.
    let case_of = |id: &str| -> Option<String> {
        db.conn()
            .query_row(
                "SELECT case_id FROM citation_checks WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .expect("satır")
    };
    assert_eq!(case_of("cc1").as_deref(), Some("c2"));
    assert_eq!(case_of("cc2"), None, "uydurulmaz");
    // Yeni çapraz yazma kapıda durur (E_DB_SCOPE).
    match db
        .conn()
        .execute(
            "INSERT INTO export_items (id,export_id,source_version_id,redaction_state)
             VALUES ('xi2','x1','v9','none')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::ScopeViolation { table }) => assert_eq!(table, "export_items"),
        other => panic!("v37 kapısı çapraz kalemi reddetmeli: {other:?}"),
    }
    match db
        .conn()
        .execute("UPDATE citation_checks SET case_id='c1' WHERE id='cc1'", [])
        .map_err(StorageError::from)
    {
        Err(StorageError::ScopeViolation { table }) => assert_eq!(table, "citation_checks"),
        other => panic!("v37 kapısı çapraz atfı reddetmeli: {other:?}"),
    }
    // Meşru yazma serbest: aynı dosyanın sürümü.
    db.conn()
        .execute(
            "INSERT INTO export_items (id,export_id,source_version_id,redaction_state)
             VALUES ('xi3','x1','v1','none')",
            [],
        )
        .expect("aynı dosya meşru");
}

/// v37'de giden segment sicilinin yalnız **sürümü** kapılıydı: satırın gösterdiği
/// segment kimliği serbestti. Ölçüldü: A'nın çalıştırmasına A'nın sürümü + B'nin
/// segmenti yazılabiliyor, A'nın gönderim sicili B dosyasının parçasını
/// gösteriyordu. v38 kapısı yeni yazmayı durdurur; eski satır olduğu gibi kalır
/// ve **sayılır** (var olmayan segment kimliği aykırı sayılmaz — denetim izi).
#[test]
fn upgrade_reports_transfer_segments_claiming_another_versions_segment() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 37);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('c2','Diger','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','aa11',1,10,'ref','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','Kira sozlesmesi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('d9','c2','Diger dosya belgesi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',10,'application/pdf','2026-01-01T00:00:00Z'),
                    ('v9','d9','c2',1,'b1','eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',10,'application/pdf','2026-01-01T00:00:00Z');
             INSERT INTO parse_runs (id,document_version_id,case_id,engine,engine_version,settings_hash,status,created_at)
             VALUES ('r1','v1','c1','pymupdf','1.24','h','succeeded','2026-01-01T00:00:00Z'),
                    ('r9','v9','c2','pymupdf','1.24','h9','succeeded','2026-01-01T00:00:00Z');
             INSERT INTO pages (id,parse_run_id,page_index,text_origin)
             VALUES ('p1','r1',0,'native'),('p9','r9',0,'native');
             INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
             VALUES ('s1','r1','p1','c1','itiraz','itiraz','2026-01-01T00:00:00Z'),
                    ('s9','r9','p9','c2','gizli','gizli','2026-01-01T00:00:00Z');
             INSERT INTO ai_runs (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,status,usage_report,created_at)
             VALUES ('run1','c1','summarize','m','h','p1','o1','running','{\"billing\":\"unknown\"}','2026-01-01T00:00:00Z');
             INSERT INTO transfer_segments
               (id,ai_run_id,outbound_segment_id,source_segment_id,document_version_id,outbound_text,outbound_hash)
             VALUES ('old-ts','run1','s9','s9','v1','gizli','hash'),
                    ('old-gone','run1','silinmis-segment','silinmis-segment','v1','metin','hash');",
        )
        .expect("v37 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let counts = db.scope_violations().expect("tarama");
    assert_eq!(counts.transfer_segments, 1, "yanlış segment sayılır");
    let metadata: String = db
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE operation='scope_violations'",
            [],
            |row| row.get(0),
        )
        .expect("kapsam taraması denetime düşmeli");
    let parsed: serde_json::Value = serde_json::from_str(&metadata).expect("metadata json");
    assert_eq!(parsed["transferSegments"], 1);
    // Yeni çapraz segment yazma kapıda durur; kendi segmenti serbest.
    match db
        .conn()
        .execute(
            "INSERT INTO transfer_segments
               (id,ai_run_id,outbound_segment_id,document_version_id,outbound_text,outbound_hash)
             VALUES ('new-ts','run1','s9','v1','gizli','bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::ScopeViolation { table }) => assert_eq!(table, "transfer_segments"),
        other => panic!("v38 kapısı yanlış segmenti reddetmeli: {other:?}"),
    }
    db.conn()
        .execute(
            "INSERT INTO transfer_segments
               (id,ai_run_id,outbound_segment_id,document_version_id,outbound_text,outbound_hash)
             VALUES ('new-ok','run1','s1','v1','itiraz','bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb')",
            [],
        )
        .expect("kendi sürümünün segmenti meşru");
}

/// v38'de **aktarım onayının belge listesi** kapı dışıydı: çekirdek
/// `create_transfer_consent` doğruluyordu, şema hiçbir şey istemiyordu.
/// Ölçüldü (v38, geçici sonda): doğrudan SQL ile A dosyasının onayına B
/// dosyasının belgesi yazılıyor, A'nın onay envanteri B'nin belgesini
/// "onaylanmış kapsam" diye sayıyordu; boş liste ve bozuk JSON da kabul
/// ediliyordu. v39 kapısı yeni yazmayı durdurur; eski satır olduğu gibi kalır
/// ve **sayılır** (sonradan silinmiş belge kimliği aykırı sayılmaz — denetim
/// izi; gönderim yolu onu zaten atlar).
#[test]
fn upgrade_reports_transfer_consents_naming_another_cases_document() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 38);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('c2','Diger','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','Kira sozlesmesi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('d9','c2','Diger dosya belgesi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO transfer_consents
               (id,provider_snapshot,case_scope,purpose,selected_document_ids,consent_text_version,approved_at)
             VALUES ('old-x','snap','c1','ozet','[\"d9\"]','1','2026-01-01T00:00:00Z'),
                    ('old-empty','snap','c1','ozet','[]','1','2026-01-01T00:00:00Z'),
                    ('old-bad','snap','c1','ozet','[oops','1','2026-01-01T00:00:00Z'),
                    ('old-ok','snap','c1','ozet','[\"d1\"]','1','2026-01-01T00:00:00Z');",
        )
        .expect("v38 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let counts = db.scope_violations().expect("tarama");
    assert_eq!(
        counts.transfer_consents, 3,
        "çapraz + boş + bozuk onay sayılır"
    );
    let metadata: String = db
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE operation='scope_violations'",
            [],
            |row| row.get(0),
        )
        .expect("kapsam taraması denetime düşmeli");
    let parsed: serde_json::Value = serde_json::from_str(&metadata).expect("metadata json");
    assert_eq!(parsed["transferConsents"], 3);
    // Yeni çapraz yazma kapıda durur.
    match db
        .conn()
        .execute(
            "INSERT INTO transfer_consents
               (id,provider_snapshot,case_scope,purpose,selected_document_ids,consent_text_version,approved_at)
             VALUES ('new-x','snap','c1','ozet','[\"d9\"]','1','2026-01-01T00:00:00Z')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::ScopeViolation { table }) => assert_eq!(table, "transfer_consents"),
        other => panic!("v39 kapısı çapraz belgeyi reddetmeli: {other:?}"),
    }
    // Kendi dosyasının belgesi serbest.
    db.conn()
        .execute(
            "INSERT INTO transfer_consents
               (id,provider_snapshot,case_scope,purpose,selected_document_ids,consent_text_version,approved_at)
             VALUES ('new-ok','snap','c1','ozet','[\"d1\"]','1','2026-01-01T00:00:00Z')",
            [],
        )
        .expect("kendi dosyasının belgesi meşru");
    // Eski aykırı satır **iptal edilebilir**: iptal kapsam sütunlarına dokunmaz,
    // kapı yalnız kapsamı değiştiren yazmada koşar (v35 dersi).
    db.conn()
        .execute(
            "UPDATE transfer_consents SET revoked_at='2026-02-01T00:00:00Z' WHERE id='old-x'",
            [],
        )
        .expect("eski satırın iptali kapıya takılmamalı");
}

/// v40 — kapıdan **önce** yazılmış çapraz karar hedefleri güncellemede sayılır
/// (silinmez), denetime düşer ve yeni çapraz yazma kapıda durur.
#[test]
fn upgrade_reports_review_items_pointing_at_another_cases_entity() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 39);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('c2','Diger','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO claims (id,case_id,claim_text,claim_kind,status,created_at,updated_at)
             VALUES ('x9','c2','B iddiasi','allegation','asserted','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO review_items
               (id,case_id,item_kind,payload,source_anchor_ids,target_entity_id,created_at,updated_at)
             VALUES ('old-x','c1','claim_candidate','{}','[]','x9',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('old-ok','c1','claim_candidate','{}','[]',NULL,
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
        )
        .expect("v39 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let counts = db.scope_violations().expect("tarama");
    assert_eq!(counts.review_items, 1, "çapraz karar hedefi sayılır");
    let metadata: String = db
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE operation='scope_violations'",
            [],
            |row| row.get(0),
        )
        .expect("kapsam taraması denetime düşmeli");
    let parsed: serde_json::Value = serde_json::from_str(&metadata).expect("metadata json");
    assert_eq!(parsed["reviewItems"], 1);
    // Yeni çapraz yazma kapıda durur.
    match db
        .conn()
        .execute(
            "INSERT INTO review_items
               (id,case_id,item_kind,payload,source_anchor_ids,target_entity_id,created_at,updated_at)
             VALUES ('new-x','c1','claim_candidate','{}','[]','x9',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::ScopeViolation { table }) => assert_eq!(table, "review_items"),
        other => panic!("v40 kapısı çapraz hedefi reddetmeli: {other:?}"),
    }
    // Eski aykırı satır **karara bağlanabilir kalır**: kapı yalnız kapsam
    // sütunlarına dokunan yazmada koşar; eski satır silinmez, sayılır.
    db.conn()
        .execute(
            "UPDATE review_items SET state='rejected' WHERE id='old-x'",
            [],
        )
        .expect("eski satırın durumu güncellenebilmeli");
    assert_eq!(
        db.conn()
            .query_row("SELECT count(*) FROM review_items", [], |row| row
                .get::<_, i64>(0))
            .expect("sayım"),
        2,
        "satırlar silinmez"
    );
}

/// v41 — kapıdan **önce** yazılmış çapraz/damgasız yerine-geçen bağları
/// güncellemede sayılır (silinmez), denetime düşer ve yeni yazma kapıda durur.
#[test]
fn upgrade_reports_cross_case_and_half_written_supersede_pointers() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 40);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('c2','Diger','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO review_items
               (id,case_id,item_kind,payload,source_anchor_ids,created_at,updated_at)
             VALUES ('old-a','c1','claim_candidate','{}','[]','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('old-b','c1','claim_candidate','{}','[]','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('old-9','c2','claim_candidate','{}','[]','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             -- Çapraz bağ (v40 kapısı bu sütunu hiç denetlemiyordu).
             UPDATE review_items SET superseded_by_item_id='old-9', superseded_at='2026-02-01T00:00:00Z'
              WHERE id='old-a';
             -- Damgasız yarım yazım: öğe kuyrukta karara açık kalıyordu.
             UPDATE review_items SET superseded_by_item_id='old-b' WHERE id='old-9';",
        )
        .expect("v40 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let counts = db.scope_violations().expect("tarama");
    assert_eq!(counts.review_items, 2, "çapraz bağ + damgasız bağ sayılır");
    let metadata: String = db
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE operation='scope_violations'",
            [],
            |row| row.get(0),
        )
        .expect("kapsam taraması denetime düşmeli");
    let parsed: serde_json::Value = serde_json::from_str(&metadata).expect("metadata json");
    assert_eq!(parsed["reviewItems"], 2);
    // Yeni çapraz bağ kapıda durur.
    match db
        .conn()
        .execute(
            "UPDATE review_items SET superseded_by_item_id='old-9', superseded_at='2026-03-01T00:00:00Z'
              WHERE id='old-b'",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::ScopeViolation { table }) => assert_eq!(table, "review_items"),
        other => panic!("v41 kapısı çapraz bağı reddetmeli: {other:?}"),
    }
    // Eski aykırı satır **karara bağlanabilir kalır**: kapı yalnız kapsam
    // sütunlarına dokunan yazmada koşar; eski satır silinmez, sayılır.
    db.conn()
        .execute(
            "UPDATE review_items SET state='rejected' WHERE id='old-a'",
            [],
        )
        .expect("eski satırın durumu güncellenebilmeli");
    assert_eq!(
        db.conn()
            .query_row("SELECT count(*) FROM review_items", [], |row| row
                .get::<_, i64>(0))
            .expect("sayım"),
        3,
        "satırlar silinmez"
    );
}

/// v42 — kapıdan **önce** yazılmış çapraz/kanıtsız/bozuk kontrol listesi
/// bağlarını güncellemede sayılır (silinmez), denetime düşer ve yeni yazma
/// kapıda durur.
#[test]
fn upgrade_reports_cross_case_checklist_links() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 41);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('c2','Diger','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','A sozlesmesi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('d9','c2','B tutanagi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO case_checklist_items
               (id,case_id,item_text,state,linked_document_ids,note,created_at,updated_at)
             VALUES ('old-x','c1','capraz','present','[\"d9\"]','','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('old-empty','c1','kanitsiz','present','[]','','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('old-bad','c1','bozuk','present','[oops','','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('old-ok','c1','mesru','present','[\"d1\"]','','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
        )
        .expect("v41 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let counts = db.scope_violations().expect("tarama");
    assert_eq!(
        counts.case_checklist_items, 3,
        "çapraz + kanıtsız + bozuk sayılır"
    );
    let metadata: String = db
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE operation='scope_violations'",
            [],
            |row| row.get(0),
        )
        .expect("kapsam taraması denetime düşmeli");
    let parsed: serde_json::Value = serde_json::from_str(&metadata).expect("metadata json");
    assert_eq!(parsed["caseChecklistItems"], 3);
    // Yeni çapraz bağ kapıda durur.
    match db
        .conn()
        .execute(
            "UPDATE case_checklist_items SET linked_document_ids='[\"d9\"]' WHERE id='old-ok'",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::ScopeViolation { table }) => {
            assert_eq!(table, "case_checklist_items")
        }
        other => panic!("v42 kapısı çapraz bağı reddetmeli: {other:?}"),
    }
    // Eski aykırı satırın **notu** güncellenebilir kalır: kapı yalnız kapsam
    // sütunlarına dokunan yazmada koşar; eski satır silinmez, sayılır.
    db.conn()
        .execute(
            "UPDATE case_checklist_items SET note='eski satır' WHERE id='old-x'",
            [],
        )
        .expect("eski satırın notu güncellenebilmeli");
    assert_eq!(
        db.conn()
            .query_row("SELECT count(*) FROM case_checklist_items", [], |row| row
                .get::<_, i64>(
                0
            ))
            .expect("sayım"),
        4,
        "satırlar silinmez"
    );
}

/// v43 — kapıdan **önce** yazılmış çapraz onaylı çalıştırmalar güncellemede
/// sayılır (silinmez), denetime düşer ve yeni çapraz yazma kapıda durur.
#[test]
fn upgrade_reports_ai_runs_citing_another_cases_consent() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 42);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('c2','Diger','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','A sozlesmesi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('d9','c2','B tutanagi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO transfer_consents
               (id,provider_snapshot,case_scope,purpose,selected_document_ids,consent_text_version,approved_at)
             VALUES ('tc1','snap','c1','ozet','[\"d1\"]','1','2026-01-01T00:00:00Z'),
                    ('tc9','snap','c2','ozet','[\"d9\"]','1','2026-01-01T00:00:00Z');
             INSERT INTO ai_runs
               (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,
                output_schema_version,consent_id,status,usage_report,created_at)
             VALUES ('old-ok','c1','summarize','m','h','p1','o1','tc1','running',
                     '{\"billing\":\"unknown\"}','2026-01-01T00:00:00Z'),
                    ('old-x','c1','summarize','m','h','p1','o1','tc9','running',
                     '{\"billing\":\"unknown\"}','2026-01-01T00:00:00Z'),
                    ('old-null','c1','summarize','m','h','p1','o1',NULL,'running',
                     '{\"billing\":\"unknown\"}','2026-01-01T00:00:00Z');",
        )
        .expect("v42 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let counts = db.scope_violations().expect("tarama");
    assert_eq!(counts.ai_runs, 1, "çapraz onaylı çalıştırma sayılır");
    let metadata: String = db
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE operation='scope_violations'",
            [],
            |row| row.get(0),
        )
        .expect("kapsam taraması denetime düşmeli");
    let parsed: serde_json::Value = serde_json::from_str(&metadata).expect("metadata json");
    assert_eq!(parsed["aiRuns"], 1);
    // Yeni çapraz yazma kapıda durur.
    match db
        .conn()
        .execute(
            "INSERT INTO ai_runs
               (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,
                output_schema_version,consent_id,status,usage_report,created_at)
             VALUES ('new-x','c1','summarize','m','h','p1','o1','tc9','running',
                     '{\"billing\":\"unknown\"}','2026-02-01T00:00:00Z')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::ScopeViolation { table }) => assert_eq!(table, "ai_runs"),
        other => panic!("v43 kapısı çapraz onayı reddetmeli: {other:?}"),
    }
    // Eski aykırı satırın **durumu** güncellenebilir kalır: kapı yalnız kapsam
    // sütunlarına dokunan yazmada koşar; eski satır silinmez, sayılır.
    db.conn()
        .execute("UPDATE ai_runs SET status='failed' WHERE id='old-x'", [])
        .expect("eski satırın durumu güncellenebilmeli");
    assert_eq!(
        db.conn()
            .query_row("SELECT count(*) FROM ai_runs", [], |row| row
                .get::<_, i64>(0))
            .expect("sayım"),
        3,
        "satırlar silinmez"
    );
}

/// v44 — kapıdan **önce** yazılmış yanlış çakışma işaretleri güncellemede
/// **silinmez** (sütunlar değişmez olduğu için onarılamaz da), yeni yanlış
/// işaret kapıda durur. Bu tablo `case_id` taşımadığı için kapsam taramasına
/// girmez — dürüst sınır.
#[test]
fn upgrade_guards_precedent_conflict_markers_written_before_v44() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 43);
        conn.execute_batch(
            "INSERT INTO precedents (id,court,created_at)
             VALUES ('pA','Yargıtay','2026-01-01T00:00:00Z'),
                    ('pB','Danıştay','2026-01-01T00:00:00Z');
             INSERT INTO precedent_versions
               (id,precedent_id,acquired_at,source_kind,body_hash,verification_state,
                identity_conflict,conflict_with_version_id)
             VALUES ('old-ok','pA','2026-01-01T00:00:00Z','paste','hash-a','text_verified',0,NULL),
                    ('old-honest','pA','2026-01-02T00:00:00Z','paste','hash-b','text_verified',1,'old-ok'),
                    ('vB1','pB','2026-01-03T00:00:00Z','paste','hash-z','text_verified',0,NULL),
                    ('old-x','pA','2026-01-04T00:00:00Z','paste','hash-z','text_verified',1,'vB1'),
                    ('old-same','pA','2026-01-05T00:00:00Z','paste','hash-a','text_verified',1,'old-ok');",
        )
        .expect("v43 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    // Eski satırlar silinmez; yanlış işaretler yerinde kalır (onarım ürün kararı).
    let preserved: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM precedent_versions", [], |row| {
            row.get(0)
        })
        .expect("sayım");
    assert_eq!(preserved, 5, "satırlar silinmez");
    // Eski yanlış işaret UPDATE ile onarılamaz: sütunlar değişmez.
    match db
        .conn()
        .execute(
            "UPDATE precedent_versions SET conflict_with_version_id='old-ok' WHERE id='old-x'",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("precedent_versions"), "mesaj: {message}")
        }
        other => panic!("v44 değişmezlik kapısı onarım yazmasını durdurmalı: {other:?}"),
    }
    // Yeni yanlış işaret kapıda durur (v48 notu: satır **biçimce geçerli**
    // yazılır — reddi veren kural v44'tür, biçim kapısı maskelemez).
    match db
        .conn()
        .execute(
            "INSERT INTO precedent_versions
               (id,precedent_id,acquired_at,source_kind,body_hash,verification_state,
                full_text,text_char_count,identity_conflict,conflict_with_version_id)
             VALUES ('new-x','pA','2026-02-01T00:00:00Z','paste',
                     '9999999999999999999999999999999999999999999999999999999999999999',
                     'text_verified','Yeni metin',10,1,'vB1')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("precedent_versions"), "mesaj: {message}")
        }
        other => panic!("v44 kapısı çapraz işareti reddetmeli: {other:?}"),
    }
    // Meşru çakışma yazımı hâlâ çalışır.
    db.conn()
        .execute(
            "INSERT INTO precedent_versions
               (id,precedent_id,acquired_at,source_kind,body_hash,verification_state,
                full_text,text_char_count,identity_conflict,conflict_with_version_id)
             VALUES ('new-ok','pA','2026-02-02T00:00:00Z','paste',
                     'a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1',
                     'text_verified','Yeni metin',10,1,'old-ok')",
            [],
        )
        .expect("meşru çakışma yazılabilmeli");
    // Kapsam taraması bu tabloyu saymaz (kapsam sütunu yok).
    assert_eq!(db.scope_violations().expect("tarama").total(), 0);
}

/// v48 — kapıdan **önce** yazılmış bozuk metin/özet satırları **silinmez**;
/// metinleri UPDATE ile onarılamaz (içerik donmuştur), yeni bozuk yazım
/// kapıda durur, çekirdeğin biçimi geçer. Onarım yolu **yeni sürüm** yazmaktır
/// (nüsha tekrarı `body_hash` ile yakalanır).
#[test]
fn upgrade_keeps_precedent_versions_written_before_the_content_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 47);
        conn.execute_batch(
            "INSERT INTO precedents (id,court,created_at)
             VALUES ('p1','Yargıtay','2026-01-01T00:00:00Z');
             INSERT INTO precedent_versions
               (id,precedent_id,acquired_at,source_kind,body_hash,verification_state,
                full_text,text_char_count,identity_conflict)
             VALUES ('old-bad-hash','p1','2026-01-01T00:00:00Z','paste','hash',
                     'text_verified','Eski metin',10,0),
                    ('old-no-text','p1','2026-01-02T00:00:00Z','paste',
                     'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
                     'text_verified',NULL,0,0);",
        )
        .expect("v47 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let preserved: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM precedent_versions", [], |row| {
            row.get(0)
        })
        .expect("sayım");
    assert_eq!(preserved, 2, "satırlar silinmez");
    // Bozuk metin UPDATE ile onarılamaz: içerik donmuştur.
    match db
        .conn()
        .execute(
            "UPDATE precedent_versions SET full_text='Onarılmış metin', text_char_count=15
               WHERE id='old-no-text'",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("precedent_versions"), "mesaj: {message}")
        }
        other => panic!("v48 değişmezlik kapısı onarım yazmasını durdurmalı: {other:?}"),
    }
    // Yeni bozuk yazım kapıda durur.
    match db
        .conn()
        .execute(
            "INSERT INTO precedent_versions
               (id,precedent_id,acquired_at,source_kind,body_hash,verification_state,
                full_text,text_char_count,identity_conflict)
             VALUES ('new-bad','p1','2026-02-01T00:00:00Z','paste','hash',
                     'text_verified','Metin',5,0)",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("precedent_versions"), "mesaj: {message}")
        }
        other => panic!("v48 biçim kapısı bozuk özeti durdurmalı: {other:?}"),
    }
    // Çekirdeğin biçimi (64 hex + dolu metin + uyumlu sayı) geçer.
    db.conn()
        .execute(
            "INSERT INTO precedent_versions
               (id,precedent_id,acquired_at,source_kind,body_hash,verification_state,
                full_text,text_char_count,identity_conflict)
             VALUES ('new-ok','p1','2026-02-02T00:00:00Z','paste',
                     'dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
                     'text_verified','Yeni metin',10,0)",
            [],
        )
        .expect("çekirdeğin biçimi yazılabilmeli");
    // Kapsam taraması bu tabloyu saymaz (kapsam sütunu yok).
    assert_eq!(db.scope_violations().expect("tarama").total(), 0);
}

#[test]
fn upgrade_keeps_precedent_notes_written_before_the_content_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 48);
        conn.execute_batch(
            "INSERT INTO cases
               (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Dosya','review','active','normal','off',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO precedents (id,court,created_at)
             VALUES ('p1','Yargıtay','2026-01-01T00:00:00Z');
             INSERT INTO precedent_notes
               (id,precedent_id,case_id_optional,useful_passage_anchor,similarity_note,
                distinguishing_note,user_conclusion,created_at)
             VALUES ('old-untrimmed','p1','c1',NULL,' kırpılmamış ','','',
                     '2026-01-02T00:00:00Z'),
                    ('old-no-date','p1',NULL,NULL,'','','','');",
        )
        .expect("v48 notları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let preserved: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM precedent_notes", [], |row| row.get(0))
        .expect("sayım");
    assert_eq!(preserved, 2, "satırlar silinmez");
    // Bozuk kapsam/metin UPDATE ile onarılamaz: not içeriği donmuştur.
    match db
        .conn()
        .execute(
            "UPDATE precedent_notes SET similarity_note='kırpılmış'
               WHERE id='old-untrimmed'",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("precedent_notes"), "mesaj: {message}")
        }
        other => panic!("v49 değişmezlik kapısı onarım yazmasını durdurmalı: {other:?}"),
    }
    // Yeni bozuk yazım kapıda durur.
    match db
        .conn()
        .execute(
            "INSERT INTO precedent_notes
               (id,precedent_id,case_id_optional,useful_passage_anchor,similarity_note,
                distinguishing_note,user_conclusion,created_at)
             VALUES ('new-bad','p1',NULL,NULL,' kırpılmamış ','','',
                     '2026-02-01T00:00:00Z')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("precedent_notes"), "mesaj: {message}")
        }
        other => panic!("v49 metin kapısı kırpılmamış notu durdurmalı: {other:?}"),
    }
    // Çekirdeğin biçimi (kırpılmış metin, dolu zaman, yaşayan dosya) geçer.
    db.conn()
        .execute(
            "INSERT INTO precedent_notes
               (id,precedent_id,case_id_optional,useful_passage_anchor,similarity_note,
                distinguishing_note,user_conclusion,created_at)
             VALUES ('new-ok','p1','c1','Ek-3 s.14','benzer','','',
                     '2026-02-02T00:00:00Z')",
            [],
        )
        .expect("çekirdeğin biçimi yazılabilmeli");
    // Not kapsamı dosya zinciri değildir; tarama alanı eklenmez.
    assert_eq!(db.scope_violations().expect("tarama").total(), 0);
}

#[test]
fn upgrade_does_not_start_when_backup_cannot_be_written() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);
    // Yedek yolunda bir klasör var: yedek yazılamaz.
    std::fs::create_dir_all(pre_migration_backup_path(&path, 12)).expect("engel");

    let err = Database::open(&path, &key()).expect_err("yedeksiz güncelleme yok");
    assert_eq!(err.code(), "E_DB_MIGRATION_BACKUP");
    assert_eq!(
        Database::verify_database_file(&path, &key()).expect("dosya sağlam"),
        12,
        "güncelleme başlamamalı"
    );
}

#[test]
fn failed_migration_keeps_old_version_and_backup_stays_usable() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);
    {
        let conn = Connection::open(&path).expect("aç");
        apply_key_and_pragmas(&conn, &key()).expect("anahtar");
        // v13'ün oluşturacağı indeks adını önceden kap: ALTER'lar geçer,
        // CREATE INDEX patlar → migrasyon yarıda kalır.
        conn.execute_batch("CREATE INDEX precedent_versions_by_precedent ON cases(id);")
            .expect("çakışma");
    }

    let err = Database::open(&path, &key()).expect_err("migrasyon düşer");
    assert_eq!(err.code(), "E_DB_MIGRATION");
    assert_eq!(
        Database::verify_database_file(&path, &key()).expect("dosya sağlam"),
        12,
        "yarım migrasyon dosyada iz bırakmamalı"
    );
    let backup = pre_migration_backup_path(&path, 12);
    assert_eq!(
        Database::verify_database_file(&backup, &key()).expect("yedek doğrulanır"),
        12
    );
    assert_eq!(case_titles(&backup), vec!["Eski dosya".to_string()]);
}

#[test]
fn newer_schema_is_refused_before_any_write() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);
    {
        let conn = Connection::open(&path).expect("aç");
        apply_key_and_pragmas(&conn, &key()).expect("anahtar");
        // “Gelecek” sürüm daima mevcut şemanın bir üstüdür; sabit sayı
        // yazmak her yeni migrasyonda testi yanlış yönde kırıyordu.
        let future = crate::CURRENT_SCHEMA_VERSION + 1;
        conn.execute(
            &format!(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                 VALUES ({future}, '00{future}_future', 'x', '2026-01-01T00:00:00Z')"
            ),
            [],
        )
        .expect("gelecek sürüm");
    }

    let err = Database::open(&path, &key()).expect_err("yeni şema reddedilir");
    assert_eq!(err.code(), "E_DB_SCHEMA_NEWER");

    // Ne v13 uygulanmış ne de v13 sütunları eklenmiş olmalı.
    let conn = Connection::open(&path).expect("aç");
    apply_key_and_pragmas(&conn, &key()).expect("anahtar");
    let has_13: i64 = conn
        .query_row(
            "SELECT count(*) FROM schema_migrations WHERE version = 13",
            [],
            |row| row.get(0),
        )
        .expect("say");
    assert_eq!(has_13, 0);
    let columns: i64 = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('precedent_versions') WHERE name = 'full_text'",
            [],
            |row| row.get(0),
        )
        .expect("sütunlar");
    assert_eq!(columns, 0, "yeni şema yazılmamalı");
    let conflict_columns: i64 = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('precedent_versions') WHERE name = 'identity_conflict'",
            [],
            |row| row.get(0),
        )
        .expect("sütunlar");
    assert_eq!(conflict_columns, 0, "v14 de yazılmamalı");
}

#[test]
fn rollback_restores_previous_version_and_keeps_safety_copy() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);

    let db = Database::open(&path, &key()).expect("güncelle");
    db.conn()
        .execute(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c-yeni','Yeni dosya','review','active','normal','off','2026-01-02T00:00:00Z','2026-01-02T00:00:00Z')",
            [],
        )
        .expect("yeni kayıt");
    let backup = db.pre_migration_backup().expect("yedek").to_path_buf();
    drop(db);

    // Önceki çökmenin artığı: geri dönüş başlamadan temizlenmeli.
    let leftover = dir.path().join(".vault.db.rollback-999-0");
    std::fs::write(&leftover, b"yarim").expect("artık");

    let outcome = rollback_vault(&path, &backup, &key()).expect("geri dönüş");
    assert_eq!(outcome.restored_version, 12);
    assert_eq!(outcome.swept_temps, 1, "sahipsiz geçici dosya temizlenir");
    assert!(!leftover.exists(), "geçici dosya silinir");
    assert!(outcome.safety_copy.is_file(), "güvenlik kopyası kalmalı");

    assert_eq!(case_titles(&path), vec!["Eski dosya".to_string()]);
    assert_eq!(
        case_titles(&outcome.safety_copy),
        vec!["Eski dosya".to_string(), "Yeni dosya".to_string()],
        "güvenlik kopyası güncelleme sonrası veriyi korur"
    );
}

#[test]
fn in_app_rollback_restores_old_data_and_resumes_current_schema() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    db.conn()
        .execute(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c-yeni','Yeni dosya','review','active','normal','off','2026-01-02T00:00:00Z','2026-01-02T00:00:00Z')",
            [],
        )
        .expect("yeni kayıt");
    drop(db);

    // Uygulamanın gördüğü liste: kasanın yanındaki yedek, doğrulanmış sürümüyle.
    let list = pre_update_backups(&path, &key()).expect("liste");
    assert_eq!(list.len(), 1, "güncelleme öncesi yedek listelenir");
    assert_eq!(list[0].claimed_version, 12);
    assert_eq!(list[0].verified_version, Some(12));
    assert!(list[0].usable());
    assert!(!list[0].name_mismatch());

    let resumed = rollback_and_resume(&path, &list[0].path, &key()).expect("geri dönüş");
    assert_eq!(resumed.restored_version, 12);
    assert!(resumed.safety_copy.is_file(), "güvenlik kopyası kalır");
    assert_eq!(
        resumed.database.schema_version(),
        CURRENT_SCHEMA_VERSION,
        "kasa güncel şemaya ilerletilir"
    );

    // Geri dönen kasa yalnız güncelleme öncesi veriyi taşır; güncelleme
    // sonrası kayıt güvenlik kopyasında durur.
    let titles: Vec<String> = {
        let mut stmt = resumed
            .database
            .conn()
            .prepare("SELECT title FROM cases ORDER BY title")
            .expect("hazırla");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("sorgu");
        rows.map(|r| r.expect("satır")).collect()
    };
    assert_eq!(titles, vec!["Eski dosya".to_string()]);
    assert_eq!(
        case_titles(&resumed.safety_copy),
        vec!["Eski dosya".to_string(), "Yeni dosya".to_string()]
    );

    // Yedek dosyası yerinde kalır: geri dönüş onu tüketmez.
    assert!(list[0].path.is_file(), "yedek silinmez");

    // Denetim kaydı geri konan kasada, sürümlerle; dosya yolu yazılmaz.
    let (category, operation): (String, String) = resumed
        .database
        .conn()
        .query_row(
            "SELECT category, operation FROM audit_events WHERE category = 'vault_rollback' ORDER BY sequence DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("denetim");
    assert_eq!(category, "vault_rollback");
    assert_eq!(operation, "completed");
    let metadata: String = resumed
        .database
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE category = 'vault_rollback' ORDER BY sequence DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("meta");
    assert!(
        metadata.contains("\"restoredVersion\":12"),
        "meta: {metadata}"
    );
    assert!(
        !metadata.contains("vault.db"),
        "denetime dosya yolu yazılmaz: {metadata}"
    );
}

/// Neden sıra önemli: kasa dosyası yokken `Database::open` (`SQLITE_OPEN_CREATE`)
/// **boş bir kasa yaratır** — kullanıcı için "bütün dosyalar kayboldu" demek.
/// Geri dönüş bu yüzden dosyayı hiç ortadan kaldırmaz (sabit bağlantı).
#[test]
fn opening_a_missing_vault_creates_an_empty_one() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    assert!(!path.exists());
    let db = Database::open(&path, &key()).expect("boş kasa yaratılır");
    assert!(path.is_file(), "open dosyayı yaratır");
    assert_eq!(db.list_cases(true).expect("dosyalar").len(), 0);
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
}

/// Dolu bir `-wal` yan dosyası varken geri dönüş **başlamaz**: yan dosyalar
/// yerinden oynatılırsa içlerindeki işlemler sessizce kaybolurdu.
#[test]
fn rollback_refuses_when_wal_holds_data() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);
    let db = Database::open(&path, &key()).expect("güncelle");
    db.conn()
        .execute(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c-yeni','Yeni dosya','review','active','normal','off','2026-01-02T00:00:00Z','2026-01-02T00:00:00Z')",
            [],
        )
        .expect("yeni kayıt");
    let backup = db.pre_migration_backup().expect("yedek").to_path_buf();
    drop(db);

    let before = std::fs::read(&path).expect("okundu");
    std::fs::write(dir.path().join("vault.db-wal"), b"wal-verisi").expect("wal");

    let err = rollback_vault(&path, &backup, &key()).expect_err("WAL varken başlamaz");
    assert_eq!(err.code(), "E_DB_ROLLBACK_WAL", "ayrı kod: {err}");
    assert_eq!(
        std::fs::read(&path).expect("okundu"),
        before,
        "kasa baytları değişmemeli"
    );
    let safety: Vec<String> = std::fs::read_dir(dir.path())
        .expect("liste")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".safety"))
        .collect();
    assert!(safety.is_empty(), "güvenlik kopyası oluşmamalı: {safety:?}");

    // Kasa bir kez açılıp kapanınca WAL emilir; geri dönüş yine çalışır.
    std::fs::remove_file(dir.path().join("vault.db-wal")).expect("wal silinir");
    let outcome = rollback_vault(&path, &backup, &key()).expect("WAL gidince geri dönüş");
    assert_eq!(outcome.restored_version, 12);
    assert_eq!(case_titles(&path), vec!["Eski dosya".to_string()]);
}

#[test]
fn rollback_refuses_corrupt_backup_and_leaves_vault_untouched() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);
    let db = Database::open(&path, &key()).expect("güncelle");
    let backup = db.pre_migration_backup().expect("yedek").to_path_buf();
    drop(db);

    std::fs::write(&backup, b"bozuk yedek").expect("boz");
    let err = rollback_vault(&path, &backup, &key()).expect_err("bozuk yedek reddedilir");
    assert_eq!(err.code(), "E_DB_MIGRATION_BACKUP");

    assert_eq!(
        Database::verify_database_file(&path, &key()).expect("kasa sağlam"),
        CURRENT_SCHEMA_VERSION,
        "mevcut kasa bozulmamalı"
    );
    assert_eq!(case_titles(&path), vec!["Eski dosya".to_string()]);
}

/// v20 kasası: eski durum adlarıyla (`leased`) bir iş ve bir deneme kaydı.
fn create_v20_with_legacy_job(path: &Path) {
    let conn = Connection::open(path).expect("dosya açılır");
    apply_key_and_pragmas(&conn, &key()).expect("anahtar");
    parsing::register_search_functions(&conn).expect("arama fonksiyonları");
    conn.execute_batch(
        "CREATE TABLE schema_migrations (
           version INTEGER PRIMARY KEY,
           name TEXT NOT NULL,
           checksum TEXT NOT NULL,
           applied_at TEXT NOT NULL
         );",
    )
    .expect("sürüm tablosu");
    for migration in MIGRATIONS.iter().filter(|m| m.version <= 20) {
        conn.execute_batch(migration.sql).expect("eski migrasyon");
        conn.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                migration.version,
                migration.name,
                hex::encode(Sha256::digest(migration.sql.as_bytes())),
                now_iso()
            ],
        )
        .expect("sürüm kaydı");
    }
    conn.execute(
        "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
         VALUES ('c-eski','Eski dosya','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
        [],
    )
    .expect("dosya");
    conn.execute(
        "INSERT INTO jobs (id,type,scope,case_id,input_fingerprint,idempotency_key,state,
                           attempt_count,next_run_at,lease_owner,lease_until,created_at,updated_at)
         VALUES ('j-legacy','parse_document','case','c-eski','fp','parse:v1','leased',
                 1,'2026-01-01T00:00:00Z','worker-eski','2026-01-01T00:05:00Z',
                 '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
        [],
    )
    .expect("iş");
    conn.execute(
        "INSERT INTO job_attempts (id,job_id,attempt_no,started_at)
         VALUES ('a-1','j-legacy',1,'2026-01-01T00:00:00Z')",
        [],
    )
    .expect("deneme");
    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
}

/// 0021 `jobs` tablosunu yeniden yazar (SQLite `CHECK` kısıtını `ALTER` ile
/// değiştirmiyor). Yeniden yazma sırasında yabancı anahtar denetimi açık
/// olsaydı eski tablonun kaldırılması çocuk `job_attempts` satırlarını
/// `ON DELETE CASCADE` ile **silerdi** (ölçüldü: 1 → 0). Kanıt: güncelleme
/// sonrası deneme satırı yerinde, eski ad yeni ada eşlenmiş, denetim yeniden
/// açık (yetim deneme yazılamaz).
#[test]
fn job_state_rewrite_keeps_attempts_and_remaps_legacy_states() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v20_with_legacy_job(&path);

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let job = db.get_job("j-legacy").expect("oku").expect("iş var");
    assert_eq!(job.state, "running", "leased → running eşlenmeli");
    assert_eq!(job.blocked_reason, None);
    let attempts: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM job_attempts WHERE job_id='j-legacy'",
            [],
            |r| r.get(0),
        )
        .expect("denemeler");
    assert_eq!(
        attempts, 1,
        "tablo yeniden yazılırken deneme satırları silinmemeli"
    );

    // Denetim gerçekten yeniden açık: yetim deneme yazılamaz.
    let orphan = db.conn().execute(
        "INSERT INTO job_attempts (id,job_id,attempt_no,started_at)
         VALUES ('a-yetim','olmayan-is',1,'2026-01-01T00:00:00Z')",
        [],
    );
    assert!(
        orphan.is_err(),
        "migrasyon sonrası foreign_keys açık olmalı"
    );

    // Ve yeni durumlar gerçekten yazılabiliyor (CHECK kümesi tamam).
    for state in [
        "blocked",
        "validating",
        "canceling",
        "interrupted",
        "partial",
    ] {
        db.conn()
            .execute("UPDATE jobs SET state=?1 WHERE id='j-legacy'", [state])
            .expect("yeni durum yazılabilmeli");
    }
}

/// v28 kasası: **bağlı** maskeleme planı, `output_version_ref` sütunu
/// eklenmeden önce. v29 bu kasayı açarken bağın izini geriye dönük yazmalı.
fn create_v28_with_linked_plan(path: &Path) {
    let conn = create_schema_at(path, 28);
    conn.execute(
        "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
         VALUES ('c-eski','Eski dosya','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
        [],
    )
    .expect("dosya kaydı");
    conn.execute(
        "INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
         VALUES ('b1','st-1',1,10,'w','2026-01-01T00:00:00Z')",
        [],
    )
    .expect("blob");
    for (doc_id, name) in [("d-kaynak", "Belge"), ("d-cikti", "Belge (maskeli)")] {
        conn.execute(
            "INSERT INTO documents (id,case_id,display_name,document_type,created_at,updated_at)
             VALUES (?1,'c-eski',?2,'pdf','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            rusqlite::params![doc_id, name],
        )
        .expect("belge");
    }
    for (version_id, doc_id, sha, parent) in [
        ("v-kaynak", "d-kaynak", "a".repeat(64), None),
        ("v-cikti", "d-cikti", "b".repeat(64), Some("v-kaynak")),
    ] {
        conn.execute(
            "INSERT INTO document_versions (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at,parent_version_id,is_original)
             VALUES (?1,?2,'c-eski',1,'b1',?3,10,'application/pdf','2026-01-01T00:00:00Z',?4,1)",
            rusqlite::params![version_id, doc_id, sha, parent],
        )
        .expect("sürüm");
    }
    conn.execute(
        "INSERT INTO redaction_plans (id,case_id,document_version_id,render_hash,coordinate_space,plan_version,state,output_profile,created_at,updated_at,output_version_id)
         VALUES ('p-eski','c-eski','v-kaynak',?1,'pdf_page_points',1,'applied','share','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','v-cikti')",
        rusqlite::params!["a".repeat(64)],
    )
    .expect("plan");
    // WAL kalıntısı bırakma: yükseltme dosyayı tek başına okumalı.
    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
}

/// v29 yükseltmesi, v28 kasasındaki bağlı planın **izini** geriye dönük yazar:
/// yükseltmeden sonra çıktı silinse bile plan "bağ yok" demez.
#[test]
fn upgrade_backfills_redaction_output_trace() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v28_with_linked_plan(&path);

    let db = Database::open(&path, &key()).expect("yükselt");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let rows = db.list_redaction_plans("c-eski").expect("liste");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].output_version_id.as_deref(), Some("v-cikti"));
    // İz, sütun eklenmeden önce yazılmış plana **geriye dönük** yazıldı.
    assert_eq!(rows[0].output_version_ref.as_deref(), Some("v-cikti"));
    assert_eq!(rows[0].output_link_state, "linked");

    // Yükseltmeden sonra çıktı silinirse iz kalır: sicil "kayıtlı değil" demez.
    db.conn()
        .execute("DELETE FROM document_versions WHERE id='v-cikti'", [])
        .expect("çıktı silindi");
    let rows = db.list_redaction_plans("c-eski").expect("liste");
    assert!(rows[0].output_version_id.is_none());
    assert_eq!(rows[0].output_link_state, "removed");
    assert_eq!(rows[0].output_version_ref.as_deref(), Some("v-cikti"));
}

/// v45 — kapıdan **önce** yazılmış yanlış dönüşüm bağları silinmez (immutability
/// onları UPDATE ile onarmayı da engeller), yeni yanlış bağ kapıda durur. Bu
/// sınıf kapsam taramasında **sayılmaz** (tablolar `case_id` taşımaz) — dürüst
/// sınır.
#[test]
fn upgrade_guards_transfer_transform_bindings_written_before_v45() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 44);
        conn.execute(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','api_allowed','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        )
        .expect("dosya");
        conn.execute(
            "INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','storage-1',1,10,'k1','2026-01-01T00:00:00Z')",
            [],
        )
        .expect("blob");
        conn.execute(
            "INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','Belge','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        )
        .expect("belge");
        conn.execute(
            "INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1',?1,10,'application/pdf','2026-01-01T00:00:00Z')",
            [&"a".repeat(64)],
        )
        .expect("sürüm");
        conn.execute_batch(
            "INSERT INTO outbound_transforms (id,kind,settings_hash,term_count,placeholder,created_at)
             VALUES ('tA','mask_terms','h-a',1,'[MASKED]','2026-01-01T00:00:00Z'),
                    ('tI','identity','h-id',0,'[MASKED]','2026-01-01T00:00:00Z');
             INSERT INTO transfer_consents
               (id,provider_snapshot,case_scope,purpose,selected_document_ids,
                consent_text_version,approved_at,outbound_transform_id)
             VALUES ('con-old','snap','c1','ozet','[\"d1\"]','1','2026-01-01T00:00:00Z',NULL),
                    ('con-x','snap','c1','ozet','[\"d1\"]','1','2026-01-02T00:00:00Z','tA');
             INSERT INTO ai_runs
               (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,
                output_schema_version,consent_id,status,usage_report,created_at)
             VALUES ('old-bad','c1','summarize','m','h','p1','o1','con-x','running',
                     '{\"billing\":\"unknown\"}','2026-01-03T00:00:00Z'),
                    ('old-ident','c1','summarize','m','h','p1','o1','con-old','running',
                     '{\"billing\":\"unknown\"}','2026-01-03T00:00:00Z');
             -- v45 öncesi yazılmış yanlış bağlar: (a) uydurma dönüşüm kimliği,
             -- (b) maskeli onayın çalıştırmasında 'identity' satırı.
             INSERT INTO transfer_segments
               (id,ai_run_id,outbound_segment_id,source_segment_id,outbound_text,
                outbound_hash,transform_manifest_id,source_range_map,document_version_id,
                source_char_count)
             VALUES ('old-bad-row','old-bad','s-bad','s-bad','[MASKED]','hash',
                     'uydurma-transform','[]',NULL,44),
                    ('old-ident-row','old-ident','s-ident','s-ident','düz metin','hash',
                     'tI','[]',NULL,9);",
        )
        .expect("v44 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let preserved: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM transfer_segments", [], |row| {
            row.get(0)
        })
        .expect("sayım");
    assert_eq!(preserved, 2, "satırlar silinmez");
    assert_eq!(
        db.scope_violations().expect("tarama").transfer_segments,
        0,
        "dönüşüm bağı sınıfı kapsam taramasında sayılmaz (dürüst sınır)"
    );
    // Yeni yanlış bağ kapıda durur.
    match db
        .conn()
        .execute(
            "INSERT INTO transfer_segments
               (id,ai_run_id,outbound_segment_id,source_segment_id,outbound_text,
                outbound_hash,transform_manifest_id,source_range_map,document_version_id,
                source_char_count)
             VALUES ('new-x','old-bad','s-new','s-new','[MASKED]','bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     'uydurma-transform','[]',NULL,44)",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("transfer_segments"), "mesaj: {message}")
        }
        other => panic!("v45 kapısı uydurma dönüşümü reddetmeli: {other:?}"),
    }
    // Onayın maske bağı dondu: eski satır NULL'a çekilemez (onarım ürün kararı).
    match db
        .conn()
        .execute(
            "UPDATE transfer_consents SET outbound_transform_id=NULL WHERE id='con-x'",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("transfer_consents"), "mesaj: {message}")
        }
        other => panic!("v45 onay bağını dondurmalı: {other:?}"),
    }
    // Eski yanlış sicil satırı da UPDATE ile onarılamaz (bağ değişmez).
    match db
        .conn()
        .execute(
            "UPDATE transfer_segments SET transform_manifest_id='tA' WHERE id='old-bad-row'",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("transfer_segments"), "mesaj: {message}")
        }
        other => panic!("v45 eski yanlış bağı onarmayı durdurmalı: {other:?}"),
    }
    // Meşru yazma hâlâ çalışır: onaylanan maskeyle yeni sicil satırı.
    db.conn()
        .execute(
            "INSERT INTO transfer_segments
               (id,ai_run_id,outbound_segment_id,source_segment_id,outbound_text,
                outbound_hash,transform_manifest_id,source_range_map,document_version_id,
                source_char_count)
             VALUES ('new-ok','old-bad','s-ok','s-ok','[MASKED]','bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb','tA','[]',NULL,44)",
            [],
        )
        .expect("meşru sicil yazılabilir");
}

/// v45'te atıf kontrolü sicili **hükmünü kanıtlamıyordu**: düz SQL var olmayan
/// emsale bağlanabiliyor, `quote_match` sonradan çevrilebiliyor, gerekçesiz
/// «yanıltıcı» ve damgasız «incelendi» yazılabiliyordu (ölçüm: 22 kusur
/// yazımının 21'i kabul; `scope_violations()` bozuk 8 satırın hiçbirini
/// saymıyordu). v46 göçü eski satırları **silmez**; tarama onları **sayar**,
/// immutability onarımı engeller (temizlik ürün kararı).
#[test]
fn upgrade_keeps_citation_checks_written_before_the_verdict_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 45);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO precedents (id,court,chamber,docket_no,decision_no,application_no,
                                     decision_date,title,source_url,user_tags,legal_issue,
                                     completeness,created_at)
             VALUES ('p1','Yargitay','7. Hukuk','2024/7','2025/8',NULL,NULL,'Emsal A',NULL,'[]',
                     NULL,'complete','2026-01-01T00:00:00Z');
             -- v46 öncesi yazılmış iki tutarsız sicil satırı:
             -- (a) var olmayan emsal bağı, (b) gerekçesiz «yanıltıcı» hüküm.
             INSERT INTO citation_checks
               (id,case_id,input_document_version_id,extracted_citation,candidate_precedent_id,
                identity_match,quote_match,meaning_review,meaning_note,meaning_reviewed_at,
                reason_codes,created_at)
             VALUES ('cc-gone','c1','v1','alıntı','uydurma-emsal','unknown','not_checked',
                     'not_reviewed','',NULL,'[]','2026-01-01T00:00:00Z'),
                    ('cc-liar','c1','v1','başka alıntı','p1','match','match','misleading',
                     '','2026-02-02T00:00:00Z','[]','2026-01-01T00:00:00Z');",
        )
        .expect("v45 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let preserved: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM citation_checks", [], |row| row.get(0))
        .expect("sayım");
    assert_eq!(preserved, 2, "satırlar silinmez");
    assert_eq!(
        db.scope_violations().expect("tarama").citation_checks,
        2,
        "bağı çözülemeyen emsal ve gerekçesiz hüküm sayılır"
    );
    // Yeni yazma kapıda durur: var olmayan emsal kimliği.
    match db
        .conn()
        .execute(
            "INSERT INTO citation_checks
               (id,case_id,input_document_version_id,extracted_citation,candidate_precedent_id,
                identity_match,quote_match,meaning_review,reason_codes,created_at)
             VALUES ('cc-new','c1','v1','alıntı','uydurma-emsal','unknown','not_checked',
                     'not_reviewed','[]','2026-01-03T00:00:00Z')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("citation_checks"), "mesaj: {message}")
        }
        other => panic!("v46 kapısı uydurma emsali reddetmeli: {other:?}"),
    }
    // Eski hüküm UPDATE ile onarılamaz: konu ve hüküm dondu.
    match db
        .conn()
        .execute(
            "UPDATE citation_checks SET quote_match='mismatch' WHERE id='cc-liar'",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("citation_checks"), "mesaj: {message}")
        }
        other => panic!("v46 eski hükmü onarmayı durdurmalı: {other:?}"),
    }
    // Meşru avukat eylemi hâlâ çalışır: gerekçeli anlam incelemesi.
    let reviewed = db
        .review_citation_meaning("cc-liar", "misleading", "kararın kapsamı farklı")
        .expect("gerekçeli anlam incelemesi");
    assert_eq!(reviewed.meaning_review, "misleading");
    assert!(reviewed.meaning_reviewed_at.is_some());
}

/// v47'de gönderim sicilinin **metni/özeti** kanıtlanabilir olmalı.
///
/// v46'da `transfer_segments.outbound_text`/`outbound_hash`,
/// `source_range_map`, `source_char_count` ve özne sütunları UPDATE'e açıktı
/// (ölçüm: 13 kusur yazımı kabul): yazılmış satırın metni sonradan
/// değiştirilebiliyor, `sent_segments` alıntıyı sahte metinde arıyor,
/// `list_transfer_segments` sahte metni/hash'i ekrana basıyordu. v47 göçü eski
/// satırları **silmez** (bozuk hash'li satır olduğu gibi kalır) ve BU onların
/// UPDATE ile onarılmasını engeller (temizlik ürün kararı).
#[test]
fn upgrade_keeps_transfer_segments_written_before_the_content_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 46);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','aa11',1,10,'ref','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','Kira sozlesmesi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',10,
                     'application/pdf','2026-01-01T00:00:00Z');
             INSERT INTO parse_runs (id,document_version_id,case_id,engine,engine_version,settings_hash,status,created_at)
             VALUES ('r1','v1','c1','pymupdf','1.24','h','succeeded','2026-01-01T00:00:00Z');
             INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
             VALUES ('s1','r1',NULL,'c1','gizli','gizli','2026-01-01T00:00:00Z');
             INSERT INTO ai_runs (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,
                                  output_schema_version,status,usage_report,created_at)
             VALUES ('run1','c1','summarize','m','h','p1','o1','running','{\"billing\":\"unknown\"}','2026-01-01T00:00:00Z');
             -- v46 öncesi yazılmış satır: 64 haneli olmayan hash (BUGÜN de geçersiz biçim).
             INSERT INTO transfer_segments
               (id,ai_run_id,outbound_segment_id,source_segment_id,document_version_id,outbound_text,outbound_hash)
             VALUES ('legacy-ts','run1','s1','s1','v1','gizli','hash');",
        )
        .expect("v46 satırı");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let preserved: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM transfer_segments", [], |row| {
            row.get(0)
        })
        .expect("sayım");
    assert_eq!(preserved, 1, "eski satır silinmez");
    // Eski satır onarılamaz: içerik ve özne dondu.
    match db
        .conn()
        .execute(
            "UPDATE transfer_segments SET outbound_text='baska metin' WHERE id='legacy-ts'",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("transfer_segments"), "mesaj: {message}")
        }
        other => panic!("v47 eski satırı dondurmalı: {other:?}"),
    }
    // Yeni satır **doğarken** hash biçimi denetlenir.
    match db
        .conn()
        .execute(
            "INSERT INTO transfer_segments
               (id,ai_run_id,outbound_segment_id,document_version_id,outbound_text,outbound_hash)
             VALUES ('new-bad','run1','s2','v1','gizli','hash')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("transfer_segments"), "mesaj: {message}")
        }
        other => panic!("v47 hash olmayan özeti reddetmeli: {other:?}"),
    }
    // Meşru yazma (çekirdeğin biçimi) hâlâ çalışır.
    db.conn()
        .execute(
            "INSERT INTO transfer_segments
               (id,ai_run_id,outbound_segment_id,document_version_id,outbound_text,outbound_hash)
             VALUES ('new-ok','run1','s2','v1','gizli',
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb')",
            [],
        )
        .expect("meşru satır yazılabilir");
}

/// v55 — aktarım makbuzu. v54 kasasında yazılmış **bozuk** makbuz (boş hedef,
/// hex olmayan özet, negatif öğe/bayt, bilinmeyen durum) güncellemede
/// **silinmez**; ama donar (onarım yolu **yeni gönderim**dir) ve yeni bozuk
/// makbuz doğamaz. Sonucu yazılmamış eski `attempted` satır, kapıya rağmen
/// açılış uzlaştırmasıyla kapanabilir — kapı çekirdeğin temizliğini durdurmaz.
#[test]
fn upgrade_keeps_transfer_receipts_written_before_the_evidence_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 54);
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO ai_runs (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,
                                  output_schema_version,consent_id,status,usage_report,created_at)
             VALUES ('run1','c1','summarize','m','h','p1','o1',NULL,'running','{}','2026-01-01T00:00:00Z');
             -- v54 öncesi yazılmış satır: boş hedef, hash biçimi yok, negatif hacim.
             INSERT INTO transfer_receipts
               (id,ai_run_id,destination_label,outbound_hash,item_count,byte_count,status,dispatched_at)
             VALUES ('legacy-bad','run1','','hash',-5,-1,'sent','dün');
             -- Sonucu yazılmamış eski deneme: uzlaştırma hâlâ kapatabilmeli.
             INSERT INTO transfer_receipts
               (id,ai_run_id,destination_label,outbound_hash,item_count,byte_count,status,dispatched_at)
             VALUES ('legacy-open','run1','api.ornek.com · m',
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     1,10,'attempted','2026-01-01T00:00:00.000000Z');",
        )
        .expect("v54 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    // Eski satırlar silinmez, değerleri **düzeltilmez**.
    let (dest, hash, items, bytes, status): (String, String, i64, i64, String) = db
        .conn()
        .query_row(
            "SELECT destination_label, outbound_hash, item_count, byte_count, status
             FROM transfer_receipts WHERE id='legacy-bad'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("eski satır");
    assert_eq!(
        (dest.as_str(), hash.as_str(), items, bytes, status.as_str()),
        ("", "hash", -5, -1, "sent"),
        "v55 eski satırı onarmaz"
    );
    // Bozuk eski satır donar: kanıt sütunu çevrilemez, durumu ilerletilemez.
    for sql in [
        "UPDATE transfer_receipts SET destination_label='baska.example · m' WHERE id='legacy-bad'",
        "UPDATE transfer_receipts SET byte_count=10 WHERE id='legacy-bad'",
        "UPDATE transfer_receipts SET status='dispatched' WHERE id='legacy-bad'",
    ] {
        match db.conn().execute(sql, []).map_err(StorageError::from) {
            Err(StorageError::Integrity(message)) => {
                assert!(message.contains("transfer_receipts"), "mesaj: {message}")
            }
            other => panic!("v55 eski satırı dondurmalı: {other:?} ({sql})"),
        }
    }
    // Yeni bozuk makbuz doğamaz (hedef boş, hash biçimsiz).
    match db
        .conn()
        .execute(
            "INSERT INTO transfer_receipts
               (id,ai_run_id,destination_label,outbound_hash,item_count,byte_count,status,dispatched_at)
             VALUES ('new-bad','run1','','hash',1,10,'attempted','2026-01-01T00:00:00.000000Z')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("transfer_receipts"), "mesaj: {message}")
        }
        other => panic!("v55 boş hedefli makbuzu reddetmeli: {other:?}"),
    }
    // Meşru yazma (çekirdeğin biçimi) hâlâ çalışır ve sonuçlanabilir.
    db.conn()
        .execute(
            "INSERT INTO transfer_receipts
               (id,ai_run_id,destination_label,outbound_hash,item_count,byte_count,status,dispatched_at)
             VALUES ('new-ok','run1','api.ornek.com · m',
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     1,10,'attempted','2026-01-01T00:00:00.000000Z')",
            [],
        )
        .expect("meşru makbuz yazılabilir");
    // Açılış uzlaştırması eski **attempted** satırı kapatır; kapanmışa dokunmaz.
    assert_eq!(
        db.reconcile_attempted_receipts().expect("uzlaştırma"),
        2,
        "legacy-open + new-ok kapanır, 'sent' satırına dokunulmaz"
    );
    let status_of = |id: &str| -> String {
        db.conn()
            .query_row(
                "SELECT status FROM transfer_receipts WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .expect("satır")
    };
    assert_eq!(status_of("legacy-open"), "interrupted");
    assert_eq!(status_of("legacy-bad"), "sent");
}

/// v56 — emsal künyesi. v55 kasasında yazılmış **tutarsız** künye
/// (`complete` rozeti ama iki numara da yok, tarih `dün`, etiket bozuk JSON,
/// boş `created_at`) güncellemede **silinmez ve düzeltilmez**; donar (onarım
/// yolu yeni kayıttır) ve yeni tutarsız künye doğamaz. Kullanım koşulu yazımı
/// (`set_precedent_usage_terms`) bu kapının dışında — meşru yol çalışmaya
/// devam eder.
#[test]
fn upgrade_keeps_precedents_written_before_the_identity_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 55);
        conn.execute_batch(
            "INSERT INTO precedents
               (id,court,chamber,docket_no,decision_no,application_no,
                decision_date,title,source_url,user_tags,legal_issue,
                completeness,created_at)
             VALUES ('legacy-bad','Yargitay','7. Hukuk',NULL,NULL,NULL,
                     'dün','Emsal A',NULL,'bozuk',NULL,'complete',''),
                    ('legacy-ok','Danistay',NULL,'2024/7','2025/8',NULL,
                     '2025-05-02','Emsal B','https://example.invalid/karar','[]',
                     NULL,'complete','2026-01-01T00:00:00Z');",
        )
        .expect("v55 künyeleri");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let count: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM precedents", [], |row| row.get(0))
        .expect("sayım");
    assert_eq!(count, 2, "satırlar silinmez");
    // Eski tutarsız satır **düzeltilmez**: rozet yalanı ve tarih olduğu gibi kalır.
    let (completeness, date, tags): (String, Option<String>, String) = db
        .conn()
        .query_row(
            "SELECT completeness, decision_date, user_tags FROM precedents
              WHERE id='legacy-bad'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("eski satır");
    assert_eq!(
        (completeness.as_str(), date.as_deref(), tags.as_str()),
        ("complete", Some("dün"), "bozuk"),
        "v56 eski künyeyi onarmaz"
    );
    // Donar: kimlik sütunu çevrilemez, rozet sonradan gerçeğe de çekilemez.
    for sql in [
        "UPDATE precedents SET court='Danistay' WHERE id='legacy-bad'",
        "UPDATE precedents SET completeness='incomplete' WHERE id='legacy-bad'",
        "UPDATE precedents SET user_tags='[]' WHERE id='legacy-bad'",
        "UPDATE precedents SET decision_date='2025-01-01' WHERE id='legacy-bad'",
    ] {
        match db.conn().execute(sql, []).map_err(StorageError::from) {
            Err(StorageError::Integrity(message)) => {
                assert!(message.contains("precedents"), "mesaj: {message}")
            }
            other => panic!("v56 eski künyeyi dondurmalı: {other:?} ({sql})"),
        }
    }
    // Yeni tutarsız künye doğamaz: rozet `complete` ama numara yok.
    match db
        .conn()
        .execute(
            "INSERT INTO precedents
               (id,court,chamber,docket_no,decision_no,application_no,decision_date,
                title,source_url,user_tags,legal_issue,completeness,created_at)
             VALUES ('new-bad','Yargitay',NULL,NULL,NULL,NULL,NULL,NULL,NULL,'[]',
                     NULL,'complete','2026-01-01T00:00:00Z')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("precedents"), "mesaj: {message}")
        }
        other => panic!("v56 rozet yalanını reddetmeli: {other:?}"),
    }
    // Meşru künye (Türkçe tarih ISO'ya çevrilir) ve kullanım koşulu yazımı çalışır.
    let p = db
        .create_precedent(&NewPrecedent {
            court: "Yargıtay".into(),
            chamber: None,
            docket_no: Some("2024/1".into()),
            decision_no: Some("2025/2".into()),
            application_no: None,
            decision_date: Some("03.05.2025".into()),
            title: None,
            source_url: None,
            user_tags: vec![],
            legal_issue: None,
        })
        .expect("meşru künye");
    assert_eq!(p.completeness, "complete");
    assert_eq!(p.decision_date.as_deref(), Some("2025-05-03"));
    let terms = db
        .set_precedent_usage_terms(
            &p.id,
            &SetUsageTermsInput {
                usage_basis: "licence".into(),
                redistribution: "allowed".into(),
                terms_url: Some("https://example.invalid/terms".into()),
                terms_note: String::new(),
            },
        )
        .expect("koşul yazımı");
    assert_eq!(terms.usage_basis, "licence");
}

/// v57 — kullanım koşulu iddiası. v56 kasasında yazılmış **kanıtsız**
/// `allowed` satırları (dayanak `unknown`/`internal_only`, yazılı dayanak ve
/// damga yok) güncellemede **silinmez, kendiliğinden düzelmez**; kapı yalnız
/// yeni tutarsızlığı durdurur. 0056'nın aksine burada **onarım yolu açıktır**:
/// aynı satır dayanak + yazılı kanıt + damga ile tutarlı hâle getirilebilir.
#[test]
fn upgrade_keeps_precedents_written_before_the_usage_terms_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 56);
        conn.execute_batch(
            "INSERT INTO precedents
               (id,court,completeness,created_at,usage_basis,redistribution,
                terms_url,terms_note,terms_recorded_at)
             VALUES ('legacy-unbacked','Yargitay','incomplete','2026-01-01T00:00:00Z',
                     'unknown','allowed',NULL,'',NULL),
                    ('legacy-internal','Danistay','incomplete','2026-01-01T00:00:00Z',
                     'internal_only','allowed','https://example.invalid/karar','',NULL),
                    ('legacy-half','Yargitay','incomplete','2026-01-01T00:00:00Z',
                     'official_public','allowed','https://example.invalid/karar','',NULL),
                    ('legacy-ok','Yargitay','incomplete','2026-01-01T00:00:00Z',
                     'licence','allowed','https://example.invalid/karar','CC-BY',
                     '2026-01-01T00:00:00Z');",
        )
        .expect("v56 koşul satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let count: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM precedents", [], |row| row.get(0))
        .expect("sayım");
    assert_eq!(count, 4, "satırlar silinmez");
    // Eski kanıtsız izin **düzeltilmez**: sütun «izinli» demeye devam eder.
    let (basis, redistribution, stamp): (String, String, Option<String>) = db
        .conn()
        .query_row(
            "SELECT usage_basis, redistribution, terms_recorded_at FROM precedents
              WHERE id='legacy-unbacked'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("eski satır");
    assert_eq!(
        (basis.as_str(), redistribution.as_str(), stamp.as_deref()),
        ("unknown", "allowed", None),
        "v57 eski iddiayı onarmaz"
    );

    // Koşul sütunlarına dokunmayan yazma takılmaz (no-op).
    db.conn()
        .execute(
            "UPDATE precedents SET terms_note='' WHERE id='legacy-unbacked'",
            [],
        )
        .expect("no-op yazma takılmamalı");
    // Hâlâ kanıtsız kalan değişiklik reddedilir.
    for sql in [
        "UPDATE precedents SET terms_note='başka' WHERE id='legacy-unbacked'",
        "UPDATE precedents SET terms_note='kurum izni' WHERE id='legacy-internal'",
    ] {
        match db.conn().execute(sql, []).map_err(StorageError::from) {
            Err(StorageError::Integrity(message)) => {
                assert!(message.contains("precedents"), "mesaj: {message}")
            }
            other => panic!("v57 kanıtsız iddiayı dondurmalı: {other:?} ({sql})"),
        }
    }
    // Onarım yolu **açık**: dayanak + yazılı kanıt + damga birlikte yazılır.
    for (id, basis) in [
        ("legacy-unbacked", "licence"),
        ("legacy-internal", "permission"),
    ] {
        db.conn()
            .execute(
                "UPDATE precedents SET usage_basis=?1, terms_note='yazılı izin',
                        terms_recorded_at='2026-02-01T00:00:00Z'
                  WHERE id=?2",
                rusqlite::params![basis, id],
            )
            .unwrap_or_else(|error| panic!("v57 onarımı engellememeli ({id}): {error}"));
    }
    // Yeni kanıtsız izin doğamaz.
    match db
        .conn()
        .execute(
            "INSERT INTO precedents
               (id,court,completeness,created_at,usage_basis,redistribution,
                terms_url,terms_note,terms_recorded_at)
             VALUES ('new-bad','Yargitay','incomplete','2026-01-01T00:00:00Z',
                     'unknown','allowed',NULL,'',NULL)",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("precedents"), "mesaj: {message}")
        }
        other => panic!("v57 kanıtsız izni reddetmeli: {other:?}"),
    }
    // Çekirdek yolu (izin + yazılı dayanak) çalışmaya devam eder.
    let p = db
        .create_precedent(&NewPrecedent {
            court: "Yargıtay".into(),
            chamber: None,
            docket_no: Some("2024/1".into()),
            decision_no: Some("2025/2".into()),
            application_no: None,
            decision_date: None,
            title: None,
            source_url: None,
            user_tags: vec![],
            legal_issue: None,
        })
        .expect("meşru künye");
    let terms = db
        .set_precedent_usage_terms(
            &p.id,
            &SetUsageTermsInput {
                usage_basis: "permission".into(),
                redistribution: "allowed".into(),
                terms_url: None,
                terms_note: "kurumdan yazılı izin 03.06.2026".into(),
            },
        )
        .expect("koşul yazımı");
    assert_eq!(terms.redistribution, "allowed");
    assert_eq!(terms.redistribution_effective, "allowed");
    assert!(terms.terms_recorded_at.is_some());
}

/// v50 — maskeleme bölgesi kanıtı. v49 kasasında yazılmış **bozuk** bölgeler
/// (negatif sayfa, bozuk JSON, yanlış uzay, boş kategori, `keep` kararı)
/// güncellemede **silinmez**; ama donar (onarım yolu yeni plandır) ve kapanmış
/// plana sonradan bölge eklenemez.
#[test]
fn upgrade_keeps_redaction_regions_written_before_the_evidence_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 49);
        conn.execute_batch(
            "INSERT INTO cases
               (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Dosya','review','active','normal','off',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs
               (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','st-legacy',1,10,'k','2026-01-01T00:00:00Z');
             INSERT INTO documents
               (id,case_id,display_name,document_type,created_at,updated_at)
             VALUES ('d1','c1','Belge','pdf',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,
                detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1',
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     10,'application/pdf','2026-01-01T00:00:00Z');
             INSERT INTO redaction_plans
               (id,case_id,document_version_id,render_hash,coordinate_space,plan_version,
                state,output_profile,created_at,updated_at)
             VALUES ('p-old','c1','v1',
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     'pdf_page_points',1,'applied','share',
                     '2026-03-01T00:00:00Z','2026-03-01T00:00:00Z');
             -- v50 öncesi yazılmış bölgeler: biçim/kapsam kapısı yoktu.
             INSERT INTO redaction_regions
               (id,plan_id,page_index,quads,coordinate_space,category,
                detected_text_hash,user_decision)
             VALUES ('old-broken','p-old',-1,'bozuk','image_pixels','','','keep'),
                    ('old-good','p-old',2,'[[10.0,20.0,30.0,40.0]]',
                     'pdf_page_points','user_term',NULL,'redact');",
        )
        .expect("v49 bölgeleri");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let preserved: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM redaction_regions", [], |row| {
            row.get(0)
        })
        .expect("sayım");
    assert_eq!(preserved, 2, "eski satırlar silinmez");
    // Eski bozuk satır **onarılamaz**: bölge kanıtı donmuştur.
    match db
        .conn()
        .execute(
            "UPDATE redaction_regions SET quads='[[1,2,3,4]]' WHERE id='old-broken'",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("redaction_regions"), "mesaj: {message}")
        }
        other => panic!("v50 eski bölgeyi dondurmalı: {other:?}"),
    }
    // Kapanmış plana sonradan kanıt eklenemez.
    match db
        .conn()
        .execute(
            "INSERT INTO redaction_regions
               (id,plan_id,page_index,quads,coordinate_space,category,
                detected_text_hash,user_decision)
             VALUES ('new-late','p-old',0,'[[1,2,3,4]]','pdf_page_points',
                     'user_term',NULL,'redact')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("redaction_regions"), "mesaj: {message}")
        }
        other => panic!("v50 kapanmış plana eklemeyi reddetmeli: {other:?}"),
    }
}

/// v51 — maskeleme planının **kanıt bağı**. v50 kasasında yazılmış tutarsız
/// planlar (başka render'a bağlı, yanlış uzayda, çıktı izi uydurma) güncellemede
/// **silinmez**; ama donar ve yeni tutarsız satır doğamaz.
#[test]
fn upgrade_keeps_redaction_plans_written_before_the_evidence_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 50);
        conn.execute_batch(
            "INSERT INTO cases
               (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Dosya','review','active','normal','off',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs
               (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','st-legacy',1,10,'k','2026-01-01T00:00:00Z');
             INSERT INTO documents
               (id,case_id,display_name,document_type,created_at,updated_at)
             VALUES ('d1','c1','Belge','pdf',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,
                detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1',
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     10,'application/pdf','2026-01-01T00:00:00Z');
             -- v51 öncesi: kapı yoktu — render özeti başka, uzay 'page',
             -- profil boşluk, bağ yokken uydurma iz.
             INSERT INTO redaction_plans
               (id,case_id,document_version_id,render_hash,coordinate_space,plan_version,
                state,output_profile,created_at,updated_at,output_version_id,output_version_ref)
             VALUES ('p-legacy','c1','v1',
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     'page',1,'applied','   ',
                     '2026-03-01T00:00:00Z','2026-03-01T00:00:00Z',NULL,'v9'),
                    ('p-legacy-ok','c1','v1',
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     'pdf_page_points',1,'draft','share',
                     '2026-03-01T00:00:00Z','2026-03-01T00:00:00Z',NULL,NULL);",
        )
        .expect("v50 planları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let preserved: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM redaction_plans", [], |row| row.get(0))
        .expect("sayım");
    assert_eq!(preserved, 2, "eski satırlar silinmez");
    // Eski tutarsız plan **onarılamaz**: kanıt bağı donmuştur.
    match db
        .conn()
        .execute(
            "UPDATE redaction_plans SET render_hash=?1 WHERE id='p-legacy'",
            [&"a".repeat(64)],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("redaction_plans"), "mesaj: {message}")
        }
        other => panic!("v51 eski planı dondurmalı: {other:?}"),
    }
    // Yeni tutarsız plan doğamaz (aynı render özeti kusuru).
    match db
        .conn()
        .execute(
            "INSERT INTO redaction_plans
               (id,case_id,document_version_id,render_hash,coordinate_space,plan_version,
                state,output_profile,created_at,updated_at)
             VALUES ('p-new','c1','v1',?1,'pdf_page_points',1,'draft','share',
                     '2026-08-01T00:00:00Z','2026-08-01T00:00:00Z')",
            [&"b".repeat(64)],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("redaction_plans"), "mesaj: {message}")
        }
        other => panic!("v51 tutarsız plan doğuşunu reddetmeli: {other:?}"),
    }
    // Meşru doğuş (çekirdeğin yolu) takılmaz.
    db.conn()
        .execute(
            "INSERT INTO redaction_plans
               (id,case_id,document_version_id,render_hash,coordinate_space,plan_version,
                state,output_profile,created_at,updated_at)
             VALUES ('p-new-ok','c1','v1',?1,'pdf_page_points',1,'draft','share',
                     '2026-08-01T00:00:00Z','2026-08-01T00:00:00Z')",
            [&"a".repeat(64)],
        )
        .expect("meşru plan");
}

/// v52 öncesi yazılmış **tutarsız sürüm künyeleri** korunur (silinmez), ama
/// güncelleme artık donmuştur ve yeni tutarsız satır doğamaz.
#[test]
fn upgrade_keeps_document_versions_written_before_the_identity_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 51);
        conn.execute_batch(
            "INSERT INTO cases
               (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Dosya','review','active','normal','off',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs
               (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','st-legacy',1,10,'k','2026-01-01T00:00:00Z');
             INSERT INTO documents
               (id,case_id,display_name,document_type,created_at,updated_at)
             VALUES ('d1','c1','Belge','pdf',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             -- v52 öncesi: özet büyük harf hex, mime yalnız boşluk, damga ISO değil.
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,
                detected_mime,imported_at)
             VALUES ('v-legacy','d1','c1',1,'b1',
                     'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
                     10,'   ','dün'),
                    ('v-ok','d1','c1',2,'b1',
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     10,'application/pdf','2026-01-01T00:00:00Z');",
        )
        .expect("v51 sürümleri");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let preserved: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM document_versions", [], |row| {
            row.get(0)
        })
        .expect("sayım");
    assert_eq!(preserved, 2, "eski satırlar silinmez");
    let kept: (String, String, String) = db
        .conn()
        .query_row(
            "SELECT original_sha256, detected_mime, imported_at FROM document_versions \
             WHERE id='v-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("eski satır");
    assert_eq!(kept.0, "A".repeat(64), "özet sessizce düzeltilmez");
    assert_eq!(kept.1, "   ");
    assert_eq!(kept.2, "dün");

    // Eski tutarsız satır **onarılamaz** (kimlik künyesi donmuştur).
    match db
        .conn()
        .execute(
            "UPDATE document_versions SET original_sha256=?1 WHERE id='v-legacy'",
            [&"a".repeat(64)],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("document_versions"), "mesaj: {message}")
        }
        other => panic!("v52 eski sürümü dondurmalı: {other:?}"),
    }
    // Yeni tutarsız sürüm doğamaz.
    match db
        .conn()
        .execute(
            "INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,
                detected_mime,imported_at)
             VALUES ('v-new','d1','c1',3,'b1',?1,10,'application/pdf','dün')",
            [&"Z".repeat(64)],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("document_versions"), "mesaj: {message}")
        }
        other => panic!("v52 tutarsız sürüm doğuşunu reddetmeli: {other:?}"),
    }
    // Meşru doğuş (çekirdeğin yolu) takılmaz.
    db.add_version("d1", "c1", "b1", &"c".repeat(64), 10, "application/pdf")
        .expect("meşru sürüm");
}

/// v53: **kaynak bağı kanıt kapısı** (source_anchors) yükseltmede eski
/// satırları silmez/düzeltmez; alıntı kimliği donar ve yeni tutarsız çapa
/// doğamaz.
#[test]
fn upgrade_keeps_source_anchors_written_before_the_evidence_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 52);
        conn.execute_batch(
            "INSERT INTO cases
               (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Dosya','review','active','normal','off',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs
               (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','st-legacy',1,10,'k','2026-01-01T00:00:00Z');
             INSERT INTO documents
               (id,case_id,display_name,document_type,created_at,updated_at)
             VALUES ('d1','c1','Belge','pdf',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,
                detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1',
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     10,'application/pdf','2026-01-01T00:00:00Z');
             INSERT INTO parse_runs
               (id,document_version_id,case_id,engine,engine_version,settings_hash,
                status,created_at)
             VALUES ('r1','v1','c1','pymupdf','1.24','h','succeeded',
                     '2026-01-01T00:00:00Z');
             INSERT INTO pages (id,parse_run_id,page_index,text_origin)
             VALUES ('p1','r1',0,'native');
             INSERT INTO text_segments
               (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
             VALUES ('s1','r1','p1','c1','Mahkeme itirazın reddine karar verdi.',
                     'Mahkeme itirazın reddine karar verdi.','2026-01-01T00:00:00Z');
             -- v53 öncesi: özet büyük harf hex, çevre metni uydurma.
             INSERT INTO source_anchors
               (id,case_id,document_version_id,parse_run_id,page_id,segment_id,
                start_offset,end_offset,exact_quote,prefix,suffix,quote_hash,
                anchor_status,created_at,updated_at)
             VALUES ('a-legacy','c1','v1','r1','p1','s1',8,24,'itirazın reddine',
                     'ZZZ','ZZZ',
                     'ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ',
                     'active','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('a-ok','c1','v1','r1','p1','s1',8,24,'itirazın reddine',
                     'Mahkeme ',' karar verdi.',
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     'active','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
        )
        .expect("v52 çapaları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let preserved: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM source_anchors", [], |row| row.get(0))
        .expect("sayım");
    assert_eq!(preserved, 2, "eski satırlar silinmez");
    let kept: (String, String, String) = db
        .conn()
        .query_row(
            "SELECT exact_quote, prefix, quote_hash FROM source_anchors WHERE id='a-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("eski satır");
    assert_eq!(kept.0, "itirazın reddine", "alıntı sessizce değiştirilmez");
    assert_eq!(kept.1, "ZZZ");
    assert_eq!(kept.2, "Z".repeat(64));

    // Eski tutarsız satırın **alıntı kimliği** donmuştur.
    match db
        .conn()
        .execute(
            "UPDATE source_anchors SET quote_hash=?1 WHERE id='a-legacy'",
            [&"b".repeat(64)],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("source_anchors"), "mesaj: {message}")
        }
        other => panic!("v53 eski çapayı dondurmalı: {other:?}"),
    }
    // Yeni tutarsız çapa doğamaz (alıntı iddia edilen ofsette değil).
    match db
        .conn()
        .execute(
            "INSERT INTO source_anchors
               (id,case_id,document_version_id,parse_run_id,page_id,segment_id,
                start_offset,end_offset,exact_quote,prefix,suffix,quote_hash,
                anchor_status,created_at,updated_at)
             VALUES ('a-new','c1','v1','r1','p1','s1',0,16,'itirazın reddine','','',
                     ?1,'active','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [&"b".repeat(64)],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("source_anchors"), "mesaj: {message}")
        }
        other => panic!("v53 tutarsız çapa doğuşunu reddetmeli: {other:?}"),
    }
    // Meşru doğuş (çekirdeğin yolu) takılmaz.
    db.create_anchor(&NewAnchor {
        case_id: "c1".into(),
        document_version_id: "v1".into(),
        parse_run_id: "r1".into(),
        segment_id: "s1".into(),
        start_offset: 8,
        end_offset: 24,
        exact_quote: "itirazın reddine".into(),
    })
    .expect("meşru çapa");
}

/// v54 — maskeleme bölgesinin **sayfa sınırı**. v53 kasasında yazılmış sayfa
/// dışı bölge (page_index 9; koşuda yalnız sayfa 0 var) güncellemede
/// **silinmez**; bölge kanıtı donmuştur (onarım yolu yeni plandır) ve yeni
/// sayfa dışı bölge doğamaz.
#[test]
fn upgrade_keeps_redaction_regions_written_before_the_page_bound_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 53);
        conn.execute_batch(
            "INSERT INTO cases
               (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Dosya','review','active','normal','off',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs
               (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','st-legacy',1,10,'k','2026-01-01T00:00:00Z');
             INSERT INTO documents
               (id,case_id,display_name,document_type,created_at,updated_at)
             VALUES ('d1','c1','Belge','pdf',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,
                detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1',
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     10,'application/pdf','2026-01-01T00:00:00Z');
             INSERT INTO parse_runs
               (id,document_version_id,case_id,engine,engine_version,settings_hash,
                status,created_at)
             VALUES ('r1','v1','c1','pymupdf','1.24','h','succeeded',
                     '2026-01-01T00:00:00Z');
             INSERT INTO pages
               (id,parse_run_id,page_index,width,height,text_origin,coordinate_space)
             VALUES ('p0','r1',0,595.0,842.0,'native','unrotated_cropbox_points');
             INSERT INTO redaction_plans
               (id,case_id,document_version_id,parse_run_id,render_hash,coordinate_space,
                plan_version,state,output_profile,created_at,updated_at)
             VALUES ('p-old','c1','v1','r1',
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     'pdf_page_points',1,'draft','share',
                     '2026-03-01T00:00:00Z','2026-03-01T00:00:00Z'),
                    ('p-draft','c1','v1','r1',
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     'pdf_page_points',1,'draft','share',
                     '2026-03-01T00:00:00Z','2026-03-01T00:00:00Z');
             -- v54 öncesi yazılmış bölge: sayfa 9 koşuda yok.
             INSERT INTO redaction_regions
               (id,plan_id,page_index,quads,coordinate_space,category,
                detected_text_hash,user_decision)
             VALUES ('old-out','p-old',9,'[[10,10,50,50]]','pdf_page_points',
                     'user_term',NULL,'redact');",
        )
        .expect("v53 bölgesi");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    let kept: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM redaction_regions", [], |row| {
            row.get(0)
        })
        .expect("sayım");
    assert_eq!(kept, 1, "eski satır silinmez");
    let page: i64 = db
        .conn()
        .query_row(
            "SELECT page_index FROM redaction_regions WHERE id='old-out'",
            [],
            |row| row.get(0),
        )
        .expect("eski bölge");
    assert_eq!(page, 9, "eski satır sessizce değiştirilmez");
    // Eski bölge donmuştur: geometri onarılamaz.
    match db
        .conn()
        .execute(
            "UPDATE redaction_regions SET quads='[[20,20,60,60]]' WHERE id='old-out'",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("redaction_regions"), "mesaj: {message}")
        }
        other => panic!("v50 eski bölgeyi dondurmalı: {other:?}"),
    }
    // Yeni sayfa dışı bölge doğamaz (draft plan, kutu biçimce geçerli).
    match db
        .conn()
        .execute(
            "INSERT INTO redaction_regions
               (id,plan_id,page_index,quads,coordinate_space,category,
                detected_text_hash,user_decision)
             VALUES ('new-out','p-draft',9,'[[10,10,50,50]]','pdf_page_points',
                     'user_term',NULL,'redact')",
            [],
        )
        .map_err(StorageError::from)
    {
        Err(StorageError::Integrity(message)) => {
            assert!(message.contains("redaction_regions"), "mesaj: {message}")
        }
        other => panic!("v54 sayfa dışı bölgeyi reddetmeli: {other:?}"),
    }
    // Meşru bölge (koşudaki sayfa 0, kutu sayfa içinde) takılmaz.
    db.conn()
        .execute(
            "INSERT INTO redaction_regions
               (id,plan_id,page_index,quads,coordinate_space,category,
                detected_text_hash,user_decision)
             VALUES ('new-ok','p-draft',0,'[[10,10,50,50]]','pdf_page_points',
                     'user_term',NULL,'redact')",
            [],
        )
        .expect("meşru bölge yazılabilir");
}

#[test]
fn upgrade_keeps_backup_runs_written_before_the_receipt_gate() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    {
        let conn = create_schema_at(&path, 57);
        conn.execute_batch(
            "INSERT INTO backup_runs
               (id,snapshot_id,schema_version,manifest_hash,status,
                verification_result,destination_label,created_at,completed_at)
             VALUES ('legacy-bad','../kacis',-3,'hash','verified',
                     NULL,'  ','dün',NULL);
             -- Sonucu yazılmamış eski deneme: uzlaştırma hâlâ kapatabilmeli.
             INSERT INTO backup_runs
               (id,snapshot_id,schema_version,manifest_hash,status,
                verification_result,destination_label,created_at,completed_at)
             VALUES ('legacy-running','DimYedek-1',57,
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     'running','integrity_check','/Volumes/Yedek',
                     '2026-11-01T00:00:00.000000Z','2026-11-01T00:00:00.000000Z');",
        )
        .expect("v57 satırları");
        let _ =
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
    }

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    // Eski satır silinmez, değerleri **düzeltilmez** (v58 onarmaz).
    let (snapshot, schema, hash, dest, created): (String, i64, String, String, String) = db
        .conn()
        .query_row(
            "SELECT snapshot_id, schema_version, manifest_hash, destination_label, created_at
             FROM backup_runs WHERE id='legacy-bad'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("eski satır");
    assert_eq!(
        (
            snapshot.as_str(),
            schema,
            hash.as_str(),
            dest.as_str(),
            created.as_str()
        ),
        ("../kacis", -3, "hash", "  ", "dün"),
        "v58 eski satırı onarmaz"
    );
    // Bozuk eski satır donar: kanıt sütunu ve damga çevrilemez.
    for sql in [
        "UPDATE backup_runs SET manifest_hash='bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' WHERE id='legacy-bad'",
        "UPDATE backup_runs SET created_at='2030-01-01T00:00:00.000000Z' WHERE id='legacy-bad'",
        "UPDATE backup_runs SET status='succeeded', completed_at='2026-11-19T00:00:00.000000Z' WHERE id='legacy-bad'",
    ] {
        match db.conn().execute(sql, []).map_err(StorageError::from) {
            Err(StorageError::Integrity(message)) => {
                assert!(message.contains("backup_runs"), "mesaj: {message}")
            }
            other => panic!("v58 eski makbuzu dondurmalı: {other:?} ({sql})"),
        }
    }
    // Yeni biçimsiz makbuz doğamaz (kanıtsız «doğrulandı», damga 'dün').
    for sql in [
        "INSERT INTO backup_runs
           (id,snapshot_id,schema_version,manifest_hash,status,
            verification_result,destination_label,created_at,completed_at)
         VALUES ('new-bad','DimYedek-1',57,
                 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                 'verified',NULL,'/Volumes/Yedek',
                 '2026-11-19T00:00:00.000000Z','2026-11-19T00:00:00.000000Z')",
        "INSERT INTO backup_runs
           (id,snapshot_id,schema_version,manifest_hash,status,
            verification_result,destination_label,created_at,completed_at)
         VALUES ('new-bad-2','DimYedek-1',57,
                 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                 'verified','integrity_check','/Volumes/Yedek',
                 'dün','2026-11-19T00:00:00.000000Z')",
    ] {
        match db.conn().execute(sql, []).map_err(StorageError::from) {
            Err(StorageError::Integrity(message)) => {
                assert!(message.contains("backup_runs"), "mesaj: {message}")
            }
            other => panic!("v58 biçimsiz makbuzu reddetmeli: {other:?}"),
        }
    }
    // Meşru yeni makbuz ve eski `running` satırın tamamlanması takılmaz.
    db.conn()
        .execute(
            "INSERT INTO backup_runs
               (id,snapshot_id,schema_version,manifest_hash,status,
                verification_result,destination_label,created_at,completed_at)
             VALUES ('new-ok','DimYedek-20261119-0200',58,
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     'verified','integrity_check','/Volumes/Yedek',
                     '2026-11-19T02:00:00.000000Z','2026-11-19T02:00:00.000000Z')",
            [],
        )
        .expect("meşru makbuz yazılabilir");
    db.conn()
        .execute(
            "UPDATE backup_runs SET status='succeeded',
                    completed_at='2026-11-19T03:00:00.000000Z'
             WHERE id='legacy-running'",
            [],
        )
        .expect("eski running satır tamamlanabilir");
}
