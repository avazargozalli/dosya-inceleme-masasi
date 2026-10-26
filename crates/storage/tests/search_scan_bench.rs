//! F15 — arama **aday taraması** ölçümü (iki aşamalı tarama A/B).
//!
//! Elle çalıştırılır (`#[ignore]`), çünkü ölçüm referans donanımda yapılır ve
//! rapor üretir. Kapı değildir.
//!
//! Korpus **seyrek eşleşme** senaryosudur: birçok segment iki belirteci de
//! içerir ama **bitişik değil** (FTS adayı olur, doğrulama reddeder), az sayıda
//! segment gerçekten bitişik içerir. Kuyruğun kaldığı yer tam burasıdır.
//!
//! İki profil ölçülür:
//! - `fullScan`: limit 200 → sayfa erken dolmaz, **bütün** adaylar doğrulanır
//!   (seyrek eşleşmenin gerçek maliyeti).
//! - `firstPage`: limit 20 → tipik ilk sayfa (erken kesme).
//!
//! ```bash
//! DIM_SCAN_BENCH_DECOYS=3000 DIM_SCAN_BENCH_HITS=30 DIM_SCAN_BENCH_ROUNDS=21 \
//!   DIM_SCAN_BENCH_OUT=/tmp/scan-ab.json \
//!   cargo test --release -p dim-storage --test search_scan_bench -- --ignored --nocapture
//! ```
//!
//! Ortam değişkenleri: `DIM_SCAN_BENCH_DECOYS` (3000), `DIM_SCAN_BENCH_HITS` (30),
//! `DIM_SCAN_BENCH_ROUNDS` (21), `DIM_SCAN_BENCH_OUT` (JSON rapor yolu).

use std::time::Instant;

use dim_storage::{
    AiPolicy, CaseKind, Database, DatabaseKey, NewCase, NewPage, SearchFilters, SearchQuery,
    Sensitivity,
};

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

/// Yüzdelik dilim (en yakın sıra); küçük örneklemde dürüst: sıralanmış dizi.
fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let index = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

fn summarize(values: &[f64]) -> String {
    format!(
        "{{\"p50\":{:.3},\"p95\":{:.3},\"max\":{:.3},\"n\":{}}}",
        percentile(values, 0.5),
        percentile(values, 0.95),
        values.iter().copied().fold(0.0_f64, f64::max),
        values.len()
    )
}

fn measure(
    db: &Database,
    case_id: &str,
    query: &str,
    limit: i64,
    rounds: usize,
) -> (Vec<f64>, i64, i64, i64) {
    let mut times = Vec::with_capacity(rounds);
    let mut last = None;
    for _ in 0..rounds {
        let started = Instant::now();
        let page = db
            .search_page(
                case_id,
                &SearchQuery::auto(query),
                &SearchFilters::default(),
                limit,
                None,
                false,
            )
            .expect("arama");
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        last = Some(page);
    }
    let page = last.expect("en az bir koşu");
    (
        times,
        page.verification.candidates_examined,
        page.returned,
        i64::from(page.has_more),
    )
}

#[test]
#[ignore]
fn search_scan_sparse_match() {
    let decoys = env_usize("DIM_SCAN_BENCH_DECOYS", 3000);
    let hits = env_usize("DIM_SCAN_BENCH_HITS", 30);
    let rounds = env_usize("DIM_SCAN_BENCH_ROUNDS", 21);
    let out_path = std::env::var("DIM_SCAN_BENCH_OUT").ok();

    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("scan-ab.db");
    let db = Database::open(&path, &DatabaseKey::new([23u8; 32])).expect("kasa");
    let case = db
        .create_case(&NewCase {
            title: "Aday taraması ölçümü".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: AiPolicy::Off,
        })
        .expect("dosya");

    let ingest_start = Instant::now();
    let mut texts: Vec<String> = Vec::with_capacity(decoys + hits);
    for index in 0..decoys {
        // İki belirteç de var, **bitişik değil** → FTS adayı, doğrulama reddeder.
        texts.push(format!(
            "Reddine dair gerekçe {index} dosyada itirazın konusu ayrıca ele alındı"
        ));
    }
    for index in 0..hits {
        texts.push(format!(
            "Mahkeme sonunda itirazın reddine karar verdi, kayıt {index}"
        ));
    }
    for (doc, text) in texts.iter().enumerate() {
        let blob = db
            .create_blob_record(&format!("scan-bench-{doc}"), 1024, "aad")
            .expect("blob");
        let document = db
            .create_document(
                &case.id,
                &format!("Belge {doc:05}.pdf"),
                "petition",
                None,
                "day",
            )
            .expect("belge");
        let version = db
            .add_version(
                &document.id,
                &case.id,
                &blob,
                &"c".repeat(64),
                1024,
                "application/pdf",
            )
            .expect("sürüm");
        let run = db
            .start_parse_run(&version.id, &case.id, "bench", "1", "{}")
            .expect("koşu");
        db.store_pages(
            &run.id,
            &case.id,
            &[NewPage {
                index: 0,
                text: text.clone(),
                width: Some(595.0),
                height: Some(842.0),
                origin: "native".into(),
                warnings: Vec::new(),
                rotation: 0,
                coordinate_space: Some("pdf".into()),
                page_label: None,
                blocks: Vec::new(),
            }],
        )
        .expect("sayfalar");
        db.finish_parse_run(&run.id, "succeeded", &[])
            .expect("bitir");
    }
    let ingest_ms = ingest_start.elapsed().as_secs_f64() * 1000.0;

    let segments: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM text_segments", [], |row| row.get(0))
        .expect("sayım");

    let mut report = String::new();
    report.push_str("{\n");
    report.push_str(&format!(
        "\"corpus\":{{\"decoys\":{decoys},\"hits\":{hits},\"segments\":{segments},\"rounds\":{rounds},\"ingestMs\":{ingest_ms:.1}}},\n"
    ));
    report.push_str("\"queries\":[\n");
    let queries = ["itirazın reddine", "\"itirazın reddine\""];
    let mut blocks: Vec<String> = Vec::new();
    for query in queries {
        let (full, examined, hit_count, more) = measure(&db, &case.id, query, 200, rounds);
        let (first, first_examined, first_hits, first_more) =
            measure(&db, &case.id, query, 20, rounds);
        blocks.push(format!(
            "  {{\"query\":{q},\"fullScan\":{full},\"fullScanExamined\":{examined},\
             \"fullScanHits\":{hit_count},\"fullScanHasMore\":{more},\
             \"firstPage\":{first},\"firstPageExamined\":{first_examined},\
             \"firstPageHits\":{first_hits},\"firstPageHasMore\":{first_more}}}",
            q = serde_json::to_string(query).expect("json"),
            full = summarize(&full),
            first = summarize(&first),
        ));
    }
    report.push_str(&blocks.join(",\n"));
    report.push_str("\n]\n}\n");

    println!("{report}");
    if let Some(path) = out_path {
        std::fs::write(&path, &report).expect("rapor yazılır");
        println!("rapor: {path}");
    }
}
