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
               (id,case_id,document_version_id,render_hash,coordinate_space,plan_version,state,output_profile,created_at,updated_at,output_version_id)
             VALUES ('p-new','c1','v1','bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb','pdf_page_points',1,
                     'applied','share','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','v2')",
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
             VALUES ('t-new','run1','s2','v2','gizli','hash')",
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
             VALUES ('new-ts','run1','s9','v1','gizli','hash')",
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
             VALUES ('new-ok','run1','s1','v1','itiraz','hash')",
            [],
        )
        .expect("kendi sürümünün segmenti meşru");
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
