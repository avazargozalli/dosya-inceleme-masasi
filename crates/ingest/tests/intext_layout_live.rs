//! F12 — **canlı** uçtan uca alıntı doğrulaması: gerçek PDF → gerçek worker →
//! gerçek kasa → eşleşme katmanları.
//!
//! `#[ignore]`: Python + PyMuPDF (`workers/document/.venv`) gerekir; CI'da
//! koşmaz. Bu testin kapısı **kayıtla birebirlik**: `fixtures/synthetic/
//! intext-layout.extracted.json` ürün worker'ının çıktısıdır; canlı çıkarım
//! ondan farklıysa (PyMuPDF sürümü, fikstür değişti) koşu **düşer** — sessizce
//! eski kayda göre "yeşil" denmez.
//!
//! Kullanım:
//! ```bash
//! DIM_WORKER_PYTHON="$PWD/workers/document/.venv/bin/python" \
//!   cargo test -p dim-ingest --test intext_layout_live -- --ignored --nocapture
//! ```
//!
//! Geri kalan alıntı davranışları (yedi vakalık tablo) CI'da kayıtlı çıkarımla
//! `crates/storage/tests/intext_real_layout.rs` içinde koşar; bu test yalnız
//! "kayıt gerçekten canlı çıkarıma eşit" zincirini ve iki kusur sınıfını
//! (sayfa sonu tiresi, ligatür) tek koşuda doğrular.

// Gerçek worker süreci başlatılır (ADR-05 ürün yolu için geçerlidir; bu istisna
// yalnız ölçüm testi içindir).
#![allow(clippy::disallowed_methods)]

use std::path::{Path, PathBuf};

use dim_ingest::budget::{WorkerBudget, WorkerKind};
use dim_ingest::worker::{dev_python_with_module, dev_worker_dir, WorkerClient};
use dim_storage::{
    AiPolicy, CaseKind, Database, DatabaseKey, InTextQuoteInput, NewCase, NewPage, Sensitivity,
};

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(relative)
}

struct SnapshotPage {
    index: i64,
    text: String,
    width: f64,
    height: f64,
    rotation: i64,
    coordinate_space: String,
}

fn load_snapshot() -> (String, String, Vec<SnapshotPage>) {
    let raw = std::fs::read_to_string(repo_path("fixtures/synthetic/intext-layout.extracted.json"))
        .expect("anlık görüntü okunamadı");
    let value: serde_json::Value = serde_json::from_str(&raw).expect("geçerli JSON");
    let pages = value["pages"]
        .as_array()
        .expect("pages")
        .iter()
        .map(|page| SnapshotPage {
            index: page["index"].as_i64().expect("index"),
            text: page["text"].as_str().expect("text").to_string(),
            width: page["width"].as_f64().unwrap_or(595.0),
            height: page["height"].as_f64().unwrap_or(842.0),
            rotation: page["rotation"].as_i64().unwrap_or(0),
            coordinate_space: page["coordinateSpace"]
                .as_str()
                .unwrap_or("unrotated_cropbox_points")
                .to_string(),
        })
        .collect();
    (
        value["engine"].as_str().unwrap_or_default().to_string(),
        value["engineVersion"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        pages,
    )
}

fn vault_with(pages: &[SnapshotPage], engine: &str, engine_version: &str) -> (Database, String) {
    let db = Database::open_in_memory(&DatabaseKey::new([78u8; 32])).expect("kasa");
    let case = db
        .create_case(&NewCase {
            title: "Canlı alıntı".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: AiPolicy::Off,
        })
        .expect("dosya");
    let blob = db
        .create_blob_record("blob-intext-layout", 74_295, "aad")
        .expect("blob");
    let document = db
        .create_document(&case.id, "karar.pdf", "pdf", None, "unknown")
        .expect("belge");
    let version = db
        .add_version(
            &document.id,
            &case.id,
            &blob,
            &"b".repeat(64),
            74_295,
            "application/pdf",
        )
        .expect("sürüm");
    let run = db
        .start_parse_run(
            &version.id,
            &case.id,
            engine,
            engine_version,
            "pdf|ocr=false",
        )
        .expect("koşu");
    let stored: Vec<NewPage> = pages
        .iter()
        .map(|page| NewPage {
            index: page.index,
            text: page.text.clone(),
            width: Some(page.width),
            height: Some(page.height),
            origin: "native".into(),
            warnings: Vec::new(),
            rotation: page.rotation,
            coordinate_space: Some(page.coordinate_space.clone()),
            page_label: None,
            blocks: Vec::new(),
        })
        .collect();
    db.store_pages(&run.id, &case.id, &stored)
        .expect("sayfalar");
    db.finish_parse_run(&run.id, "succeeded", &[])
        .expect("bitir");
    (db, case.id)
}

#[test]
#[ignore = "gerçek Python worker + PyMuPDF gerekir (DIM_WORKER_PYTHON)"]
fn live_pdf_extraction_matches_the_recorded_snapshot_and_quotes_resolve() {
    let (engine, engine_version, recorded) = load_snapshot();
    let pdf_path = repo_path("fixtures/synthetic/intext-layout.pdf");
    let data = std::fs::read(&pdf_path).expect("fikstür PDF okunamadı");

    let worker_dir = dev_worker_dir().expect("worker dizini");
    let python = dev_python_with_module(&worker_dir, "pymupdf")
        .expect("DIM_WORKER_PYTHON (PyMuPDF kurulu venv) verilmeli");
    let mut client = WorkerClient::spawn_dev_budgeted(
        &python,
        &worker_dir,
        WorkerBudget::for_kind(WorkerKind::Extract),
    )
    .expect("worker başlatılamadı");
    let extraction = client
        .extract_text(&data, "application/pdf", false, "eng", None, None)
        .expect("çıkarım");
    let report = client.budget_report().expect("bütçe");
    client.shutdown();
    assert_eq!(report.violated, None, "çıkarım bütçeyi aşmamalı");

    assert_eq!(extraction.engine, engine, "motor kayıtla aynı olmalı");
    assert_eq!(
        extraction.engine_version, engine_version,
        "PyMuPDF sürümü değişti: kayıt yeniden üretilmeli (fixtures/synthetic/generate_intext.py)"
    );
    assert_eq!(
        extraction.pages.len(),
        recorded.len(),
        "canlı çıkarım {} sayfa, kayıt {} sayfa",
        extraction.pages.len(),
        recorded.len()
    );
    for (live, saved) in extraction.pages.iter().zip(recorded.iter()) {
        assert_eq!(live.index, u32::try_from(saved.index).unwrap_or(u32::MAX));
        assert_eq!(
            live.text, saved.text,
            "sayfa {} metni kayıttan farklı — kayıt yeniden üretilmeli",
            saved.index
        );
    }

    // Canlı metinle alıntı katmanları: kayıtta ölçülen iki kusur sınıfı.
    let (db, case_id) = vault_with(&recorded, &engine, &engine_version);
    let check = db
        .check_in_text_quotes(
            &case_id,
            &InTextQuoteInput {
                text: "Kararda “reddine karar verilmiştir” deniyor; sözleşme “taraflar arasındaki \
                       sözleşme” diye anılıyor."
                    .into(),
                document_id: None,
            },
        )
        .expect("kontrol");
    assert_eq!(check.quotes.len(), 2);
    let describe = |finding: &dim_storage::InTextQuoteFinding| {
        format!(
            "{} → {} / {} (s.{}{})",
            finding.quote,
            finding.state,
            finding
                .position
                .as_ref()
                .map(|p| p.match_kind.as_str())
                .unwrap_or("-"),
            finding
                .position
                .as_ref()
                .map(|p| p.page_index)
                .unwrap_or(-1),
            if finding.position.as_ref().is_some_and(|p| p.spans_pages) {
                " +sayfa sınırı"
            } else {
                ""
            }
        )
    };
    println!(
        "canlı ölçüm: {} | {}",
        describe(&check.quotes[0]),
        describe(&check.quotes[1])
    );
    let spanning = check.quotes[0].position.as_ref().expect("tire konumu");
    assert_eq!(spanning.match_kind, "dehyphenated");
    assert!(spanning.spans_pages);
    let ligature = check.quotes[1].position.as_ref().expect("ligatür konumu");
    assert_eq!(ligature.match_kind, "ligature");
    assert!(ligature.source_text.contains('\u{FB02}'));
    assert_eq!(check.dehyphenated, 1);
    assert_eq!(check.ligature, 1);
}
