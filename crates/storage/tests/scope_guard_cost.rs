//! Şema v31 (zincir) ve v32 (referans) kapsam tetikleyicilerinin **yazma
//! maliyeti**.
//!
//! Tetikleyiciler her `text_segments` satırında iki birincil anahtar araması
//! yapar (koşunun dosyası + sayfanın koşusu). Bu test aynı işi tetikleyicili ve
//! tetikleyicisiz ölçer; aradaki fark kapının bedelidir. Ölçüm **debug**
//! derlemede yapılır: amaç oranı görmek, mutlak üretim sayısı vermek değil.
//!
//! `#[ignore]`: normal `cargo test` koşusuna girmez (yavaş, ölçüm testi).
//!
//!     cargo test -p dim-storage --test scope_guard_cost -- --ignored --nocapture

use std::time::Instant;

use dim_storage::{Database, DatabaseKey};

const ROWS: i64 = 20_000;

fn chain(db: &Database) {
    db.conn()
        .execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','aa11',1,10,'ref','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','Belge','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions
               (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',10,'application/pdf','2026-01-01T00:00:00Z');
             INSERT INTO parse_runs (id,document_version_id,case_id,engine,engine_version,settings_hash,status,created_at)
             VALUES ('r1','v1','c1','pymupdf','1.24','h','succeeded','2026-01-01T00:00:00Z');
             INSERT INTO pages (id,parse_run_id,page_index,text_origin)
             VALUES ('p1','r1',0,'native');",
        )
        .expect("zincir");
}

/// `from..from+ROWS` aralığında segment yazar; satır başına mikrosaniye döner.
fn insert_segments(db: &Database, from: i64) -> f64 {
    let started = Instant::now();
    let tx = db.conn().unchecked_transaction().expect("işlem");
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
                 VALUES (?1,'r1','p1','c1',?2,?2,'2026-01-01T00:00:00Z')",
            )
            .expect("hazırlık");
        for index in from..from + ROWS {
            let text = format!("segment {index} itiraz reddine karar verildi");
            stmt.execute(rusqlite::params![format!("s{index}"), text])
                .expect("segment");
        }
    }
    tx.commit().expect("commit");
    started.elapsed().as_secs_f64() * 1_000_000.0 / ROWS as f64
}

#[test]
#[ignore = "ölçüm testi: toplu yazma maliyeti"]
fn scope_triggers_add_a_predictable_cost_per_segment() {
    let db = Database::open_in_memory(&DatabaseKey::new([77u8; 32])).expect("db");
    chain(&db);

    let with_guards = insert_segments(&db, 0);

    for trigger in [
        "scope_guard_text_segments_bi",
        "scope_guard_text_segments_bu",
    ] {
        db.conn()
            .execute(&format!("DROP TRIGGER {trigger}"), [])
            .expect("tetikleyici düşür");
    }
    let without_guards = insert_segments(&db, ROWS);

    let stored: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM text_segments", [], |row| row.get(0))
        .expect("sayım");
    assert_eq!(stored, ROWS * 2, "iki koşu da yazılmalı");

    let scan_started = Instant::now();
    let counts = db.scope_violations().expect("tarama");
    let scan_ms = scan_started.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(counts.total(), 0, "yazılan satırlar tutarlı");

    println!(
        "kapsam tetikleyicileri: {ROWS} satır tetikleyicili {with_guards:.1} µs/satır, \
         tetikleyicisiz {without_guards:.1} µs/satır, fark {:.1} µs/satır ({:.0}%)",
        with_guards - without_guards,
        (with_guards / without_guards - 1.0) * 100.0
    );
    println!(
        "scope_violations() taraması: {} segmentte {scan_ms:.1} ms",
        ROWS * 2
    );
}

const REF_ROWS: i64 = 20_000;

/// `from..from+REF_ROWS` aralığında **meşru** iddia yazar (çapa `a1`, dosya
/// `c1`); satır başına mikrosaniye döner.
fn insert_claims(db: &Database, from: i64) -> f64 {
    let started = Instant::now();
    let tx = db.conn().unchecked_transaction().expect("işlem");
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO claims (id,case_id,claim_text,claim_kind,status,source_anchor_id,created_at,updated_at)
                 VALUES (?1,'c1',?2,'allegation','draft','a1','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            )
            .expect("hazırlık");
        for index in from..from + REF_ROWS {
            let text = format!("iddia {index}");
            stmt.execute(rusqlite::params![format!("c{index}"), text])
                .expect("iddia");
        }
    }
    tx.commit().expect("commit");
    started.elapsed().as_secs_f64() * 1_000_000.0 / REF_ROWS as f64
}

/// Şema v32 **referans** kapıları (iddia → çapa) satır başına ne kadar
/// maliyetli? Yazma hacmi düşük (kullanıcı eylemi başına bir satır) ama sayı
/// ölçülmeden "ihmal edilebilir" denmez.
#[test]
#[ignore = "ölçüm testi: kanıt referansı kapılarının maliyeti"]
fn reference_triggers_add_a_predictable_cost_per_row() {
    let db = Database::open_in_memory(&DatabaseKey::new([78u8; 32])).expect("db");
    chain(&db);
    // `chain()` çapa kurmaz (maliyet testi yalnız sayfa/sürüm kurar): meşru
    // iddia için kendi dosyasında bir segment + çapa gerekir.
    db.conn()
        .execute_batch(
            "INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
             VALUES ('s-base','r1','p1','c1','itiraz','itiraz','2026-01-01T00:00:00Z');
             INSERT INTO source_anchors
               (id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,
                exact_quote,quote_hash,created_at,updated_at)
             VALUES ('a1','c1','v1','r1','p1','s-base',0,6,'itiraz',
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
        )
        .expect("çapa");

    let with_guards = insert_claims(&db, 0);
    for trigger in ["scope_guard_claims_bi", "scope_guard_claims_bu"] {
        db.conn()
            .execute(&format!("DROP TRIGGER {trigger}"), [])
            .expect("tetikleyici düşür");
    }
    let without_guards = insert_claims(&db, REF_ROWS);

    let stored: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM claims", [], |row| row.get(0))
        .expect("sayım");
    assert_eq!(stored, REF_ROWS * 2, "iki koşu da yazılmalı");

    let scan_started = Instant::now();
    let counts = db.scope_violations().expect("tarama");
    let scan_ms = scan_started.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(counts.total(), 0, "yazılan satırlar tutarlı");

    println!(
        "referans kapıları (iddia → çapa): {REF_ROWS} satır tetikleyicili {with_guards:.1} µs/satır, \
         tetikleyicisiz {without_guards:.1} µs/satır, fark {:.1} µs/satır ({:.0}%)",
        with_guards - without_guards,
        (with_guards / without_guards - 1.0) * 100.0
    );
    println!(
        "scope_violations() taraması: {} iddiada {scan_ms:.1} ms",
        REF_ROWS * 2
    );
}

const NOTE_ROWS: i64 = 20_000;

/// `from..from+NOTE_ROWS` aralığında **meşru** not yazar (kaynak: aynı
/// dosyadaki `a1` ve `a2`); satır başına mikrosaniye döner.
///
/// Not yazımı kullanıcı eylemi başına **tek** satırdır; asıl bedel, listenin
/// her elemanı için yapılan çapa aramasıdır (`json_each` + indeks).
fn insert_notes(db: &Database, from: i64) -> f64 {
    let started = Instant::now();
    let tx = db.conn().unchecked_transaction().expect("işlem");
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO notes (id,case_id,title,body_ast,author_kind,visibility_label,source_anchor_ids,created_at,updated_at)
                 VALUES (?1,'c1','Not','{\"v\":1,\"text\":\"metin\"}','user','case_private','[\"a1\",\"a2\"]',
                         '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            )
            .expect("hazırlık");
        for index in from..from + NOTE_ROWS {
            stmt.execute(rusqlite::params![format!("n{index}")])
                .expect("not");
        }
    }
    tx.commit().expect("commit");
    started.elapsed().as_secs_f64() * 1_000_000.0 / NOTE_ROWS as f64
}

/// Şema v33 **JSON dizi** kapıları (not → iki çapa) satır başına ne kadar
/// maliyetli? Kapı, listenin her elemanı için bir indeksli arama yapar.
#[test]
#[ignore = "ölçüm testi: JSON dizi kapsam kapılarının maliyeti"]
fn json_scope_triggers_add_a_predictable_cost_per_note() {
    let db = Database::open_in_memory(&DatabaseKey::new([79u8; 32])).expect("db");
    chain(&db);
    db.conn()
        .execute_batch(
            "INSERT INTO text_segments (id,parse_run_id,page_id,case_id,raw_text,normalized_text,created_at)
             VALUES ('s-base','r1','p1','c1','itiraz','itiraz','2026-01-01T00:00:00Z'),
                    ('s-iki','r1','p1','c1','odeme','odeme','2026-01-01T00:00:00Z');
             INSERT INTO source_anchors
               (id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,
                exact_quote,quote_hash,created_at,updated_at)
             VALUES ('a1','c1','v1','r1','p1','s-base',0,6,'itiraz',
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                    ('a2','c1','v1','r1','p1','s-iki',0,5,'odeme',
                     'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
                     '2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
        )
        .expect("iki çapa");

    let with_guards = insert_notes(&db, 0);
    for trigger in ["scope_guard_notes_bi", "scope_guard_notes_bu"] {
        db.conn()
            .execute(&format!("DROP TRIGGER {trigger}"), [])
            .expect("tetikleyici düşür");
    }
    let without_guards = insert_notes(&db, NOTE_ROWS);

    let stored: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM notes", [], |row| row.get(0))
        .expect("sayım");
    assert_eq!(stored, NOTE_ROWS * 2, "iki koşu da yazılmalı");

    let scan_started = Instant::now();
    let counts = db.scope_violations().expect("tarama");
    let scan_ms = scan_started.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(counts.total(), 0, "yazılan satırlar tutarlı");

    println!(
        "JSON dizi kapıları (not → 2 çapa): {NOTE_ROWS} satır tetikleyicili {with_guards:.1} µs/satır, \
         tetikleyicisiz {without_guards:.1} µs/satır, fark {:.1} µs/satır ({:.0}%)",
        with_guards - without_guards,
        (with_guards / without_guards - 1.0) * 100.0
    );
    println!(
        "scope_violations() taraması: {} notta {scan_ms:.1} ms",
        NOTE_ROWS * 2
    );
}

const REMAIN_ROWS: i64 = 20_000;

/// `from..from+REMAIN_ROWS` aralığında **meşru** gönderim sicili satırı yazar
/// (çalıştırma `run1` dosya `c1`, sürüm `v1`); satır başına mikrosaniye döner.
fn insert_context_items(db: &Database, from: i64) -> f64 {
    let started = Instant::now();
    let tx = db.conn().unchecked_transaction().expect("işlem");
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO ai_context_items (id,ai_run_id,document_version_id,source_anchor_id,segment_ids,inclusion_reason,bytes_sent)
                 VALUES (?1,'run1','v1',NULL,'[]','onaylanan belge',7)",
            )
            .expect("hazırlık");
        for index in from..from + REMAIN_ROWS {
            stmt.execute(rusqlite::params![format!("ctx{index}")])
                .expect("bağlam");
        }
    }
    tx.commit().expect("commit");
    started.elapsed().as_secs_f64() * 1_000_000.0 / REMAIN_ROWS as f64
}

/// `from..from+REMAIN_ROWS` aralığında **meşru** giden segment satırı yazar.
fn insert_transfer_segments(db: &Database, from: i64) -> f64 {
    let started = Instant::now();
    let tx = db.conn().unchecked_transaction().expect("işlem");
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO transfer_segments (id,ai_run_id,outbound_segment_id,document_version_id,outbound_text,outbound_hash)
                 VALUES (?1,'run1',?1,'v1','itiraz','hash')",
            )
            .expect("hazırlık");
        for index in from..from + REMAIN_ROWS {
            stmt.execute(rusqlite::params![format!("ts{index}")])
                .expect("giden segment");
        }
    }
    tx.commit().expect("commit");
    started.elapsed().as_secs_f64() * 1_000_000.0 / REMAIN_ROWS as f64
}

/// Şema v34 kapıları: gönderim sicili (`ai_context_items`) ve giden segment
/// sicili (`transfer_segments`) satır başına ne kadar maliyetli? İkisi de
/// kullanıcı eylemi başına **az** satır yazar (çalıştırma başına belge/segment
/// sayısı kadar); ölçüm oranı görünür kılmak için.
#[test]
#[ignore = "ölçüm testi: v34 kalan referans kapılarının maliyeti"]
fn remaining_reference_triggers_add_a_predictable_cost_per_row() {
    let db = Database::open_in_memory(&DatabaseKey::new([83u8; 32])).expect("db");
    chain(&db);
    db.conn()
        .execute_batch(
            "INSERT INTO ai_runs (id,case_id,task_type,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,status,usage_report,created_at)
             VALUES ('run1','c1','summarize','m','h','p1','o1','running','{\"billing\":\"unknown\"}','2026-01-01T00:00:00Z');",
        )
        .expect("çalıştırma");

    let context_with = insert_context_items(&db, 0);
    let transfer_with = insert_transfer_segments(&db, 0);
    for trigger in [
        "scope_guard_ai_context_items_bi",
        "scope_guard_ai_context_items_bu",
        "scope_guard_transfer_segments_bi",
        "scope_guard_transfer_segments_bu",
    ] {
        db.conn()
            .execute(&format!("DROP TRIGGER {trigger}"), [])
            .expect("tetikleyici düşür");
    }
    let context_without = insert_context_items(&db, REMAIN_ROWS);
    let transfer_without = insert_transfer_segments(&db, REMAIN_ROWS);

    let scan_started = Instant::now();
    let counts = db.scope_violations().expect("tarama");
    let scan_ms = scan_started.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(counts.total(), 0, "yazılan satırlar tutarlı");

    println!(
        "gönderim sicili (ai_context_items): {REMAIN_ROWS} satır tetikleyicili {context_with:.2} µs/satır, \
         tetikleyicisiz {context_without:.2} µs/satır, fark {:.2} µs ({:.0}%)",
        context_with - context_without,
        (context_with / context_without - 1.0) * 100.0
    );
    println!(
        "giden segment (transfer_segments): {REMAIN_ROWS} satır tetikleyicili {transfer_with:.2} µs/satır, \
         tetikleyicisiz {transfer_without:.2} µs/satır, fark {:.2} µs ({:.0}%)",
        transfer_with - transfer_without,
        (transfer_with / transfer_without - 1.0) * 100.0
    );
    println!(
        "scope_violations() taraması: {} bağlam + {} giden segment satırında {scan_ms:.1} ms",
        REMAIN_ROWS * 2,
        REMAIN_ROWS * 2
    );
}
