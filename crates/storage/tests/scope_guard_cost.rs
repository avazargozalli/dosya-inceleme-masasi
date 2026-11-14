//! Şema v31 kapsam tetikleyicilerinin **toplu yazma maliyeti**.
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
