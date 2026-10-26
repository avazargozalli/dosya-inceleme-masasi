//! F15 — arama **maliyet ayrıştırması**: kalan ~5,5 ms nereden geliyor?
//!
//! Elle çalıştırılır (`#[ignore]`); kapı değildir, rapor üretir.
//!
//! Önceki tur (iki aşamalı tarama) ilk sayfayı 16,9 → 5,9 ms'ye indirdi ama
//! ilk sayfa yalnız 21 aday doğrulanırken de ~5,5 ms sürüyordu. O turda kalan
//! maliyetin **FTS'in bütün eşleşmelere `bm25` hesaplayıp sıralaması** olduğu
//! *çıkarılmıştı*, ölçülmemişti. Bu koşucu hipotezi bileşenlere ayırır:
//!
//! 1. `ftsCount` — yalnız FTS numaralandırması (`count(*)`, sıralama yok).
//! 2. `orderedIds` — `ORDER BY bm25` + yalnız `s.id` (sıralama var, metin yok).
//! 3. `unorderedIds` — sıralama yok, yalnız `s.id` (FTS indeks sırası).
//! 4. `orderedText` — üretim sorgusunun şekli (`raw_text` + `normalized_text`).
//! 5. `unorderedText` — aynı yük, sıralama yok.
//! 6. `ftsCountPhrase` / `phraseOrdered*` — tam **bitişik** ifade (`"a b" *`).
//! 7. `ftsCountNear` / `nearOrdered*` — üretimdeki daraltma
//!    (`NEAR("a" "b"*, 2)`): seyrek profilde 3.000 tuzağı FTS'ten hiç
//!    geçirmez, tek sembol belirtecine pay bırakır.
//!
//! Farklar doğrudan okunur: `orderedIds − unorderedIds` = sıralama,
//! `orderedText − orderedIds` = metin yükü/şifre çözme, `ftsCount` = taban.
//!
//! ```bash
//! DIM_COST_BENCH_DECOYS=3000 DIM_COST_BENCH_HITS=30 DIM_COST_BENCH_ROUNDS=21 \
//!   DIM_COST_BENCH_OUT=/tmp/cost.json \
//!   cargo test --release -p dim-storage --test search_cost_bench -- --ignored --nocapture
//! ```
//!
//! Ortam: `DIM_COST_BENCH_DECOYS` (3000), `DIM_COST_BENCH_HITS` (30),
//! `DIM_COST_BENCH_ROUNDS` (21), `DIM_COST_BENCH_OUT`.

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
        "{{\"p50\":{:.3},\"min\":{:.3},\"p95\":{:.3},\"max\":{:.3},\"n\":{}}}",
        percentile(values, 0.5),
        values.iter().copied().fold(f64::MAX, f64::min),
        percentile(values, 0.95),
        values.iter().copied().fold(0.0_f64, f64::max),
        values.len()
    )
}

/// Sorguyu `rounds` kez koşar; satır sayısını da döner (sonuç aynı olmalı).
fn time_sql(
    db: &Database,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
    rounds: usize,
) -> (Vec<f64>, i64) {
    let mut times = Vec::with_capacity(rounds);
    let mut rows_out = 0_i64;
    for _ in 0..rounds {
        let started = Instant::now();
        let mut stmt = db.conn().prepare(sql).expect("hazırla");
        let mut rows = stmt.query(params).expect("sorgu");
        let mut count = 0_i64;
        while rows.next().expect("satır").is_some() {
            count += 1;
        }
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        rows_out = count;
    }
    (times, rows_out)
}

fn time_scalar(
    db: &Database,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
    rounds: usize,
) -> (Vec<f64>, i64) {
    let mut times = Vec::with_capacity(rounds);
    let mut value = 0_i64;
    for _ in 0..rounds {
        let started = Instant::now();
        value = db
            .conn()
            .query_row(sql, params, |row| row.get::<_, i64>(0))
            .expect("sayım");
        times.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    (times, value)
}

#[test]
#[ignore]
fn search_cost_breakdown() {
    let decoys = env_usize("DIM_COST_BENCH_DECOYS", 3000);
    let hits = env_usize("DIM_COST_BENCH_HITS", 30);
    let rounds = env_usize("DIM_COST_BENCH_ROUNDS", 21);
    let out_path = std::env::var("DIM_COST_BENCH_OUT").ok();

    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("cost.db");
    let db = Database::open(&path, &DatabaseKey::new([29u8; 32])).expect("kasa");
    let case = db
        .create_case(&NewCase {
            title: "Maliyet ayrıştırması".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: AiPolicy::Off,
        })
        .expect("dosya");

    // Tuzaklar: iki belirteç de var, **bitişik değil** → geniş FTS ifadesinde
    // aday olur, doğrulama reddeder, ifade daraltmasında hiç aday olmaz.
    let mut texts: Vec<String> = Vec::with_capacity(decoys + hits);
    for index in 0..decoys {
        texts.push(format!(
            "Reddine dair gerekçe {index} dosyada itirazın konusu ayrıca ele alındı"
        ));
    }
    for index in 0..hits {
        texts.push(format!(
            "Mahkeme sonunda itirazın reddine karar verdi, kayıt {index}"
        ));
    }

    let ingest_start = Instant::now();
    for (doc, text) in texts.iter().enumerate() {
        let blob = db
            .create_blob_record(&format!("cost-bench-{doc}"), 1024, "aad")
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
                &"d".repeat(64),
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

    // Daraltma öncesi aday ifadesi (eski `fts_expression`: belirteçler AND).
    let broad = "normalized_text : \"itirazin\" AND normalized_text : \"reddine\"*";
    // Ölçülen iki daraltma biçimi: tam bitişik ifade ve üretimdeki `NEAR`
    // (bir belirteçlik pay: sembol belirteçleri için).
    let phrase = "normalized_text : \"itirazin reddine\" *";
    let near = "normalized_text : NEAR(\"itirazin\" \"reddine\"*, 2)";

    let from = "text_segments_fts JOIN text_segments s ON s.rowid = text_segments_fts.rowid";
    let where_clause = " WHERE text_segments_fts MATCH ? AND s.case_id = ?";

    let fts_count_sql = "SELECT count(*) FROM text_segments_fts WHERE text_segments_fts MATCH ?";
    let ordered_ids =
        format!("SELECT s.id FROM {from}{where_clause} ORDER BY bm25(text_segments_fts), s.id");
    let unordered_ids = format!("SELECT s.id FROM {from}{where_clause}");
    let ordered_text = format!(
        "SELECT s.id, s.raw_text, s.normalized_text, bm25(text_segments_fts) AS r
         FROM {from}{where_clause} ORDER BY r, s.id"
    );
    let unordered_text = format!(
        "SELECT s.id, s.raw_text, s.normalized_text, bm25(text_segments_fts) AS r
         FROM {from}{where_clause}"
    );
    // `LIMIT` sıralamayı kısaltır mı? (Sıralama tüm eşleşmeler üzerinde kurulur.)
    let ordered_text_limited = format!("{ordered_text} LIMIT 200");

    let case_id = case.id.clone();
    let p_case: &dyn rusqlite::ToSql = &case_id;
    let p_broad: &dyn rusqlite::ToSql = &broad;
    let p_phrase: &dyn rusqlite::ToSql = &phrase;
    let p_near: &dyn rusqlite::ToSql = &near;

    let (fts_broad, fts_broad_n) = time_scalar(&db, fts_count_sql, &[p_broad], rounds);
    let (fts_phrase, fts_phrase_n) = time_scalar(&db, fts_count_sql, &[p_phrase], rounds);
    let (fts_near, fts_near_n) = time_scalar(&db, fts_count_sql, &[p_near], rounds);

    let (ord_ids, ord_ids_n) = time_sql(&db, &ordered_ids, &[p_broad, p_case], rounds);
    let (unord_ids, unord_ids_n) = time_sql(&db, &unordered_ids, &[p_broad, p_case], rounds);
    let (ord_text, ord_text_n) = time_sql(&db, &ordered_text, &[p_broad, p_case], rounds);
    let (unord_text, unord_text_n) = time_sql(&db, &unordered_text, &[p_broad, p_case], rounds);
    let (ord_text_lim, ord_text_lim_n) =
        time_sql(&db, &ordered_text_limited, &[p_broad, p_case], rounds);

    let (phr_ord_ids, phr_ord_ids_n) = time_sql(&db, &ordered_ids, &[p_phrase, p_case], rounds);
    let (phr_ord_text, phr_ord_text_n) = time_sql(&db, &ordered_text, &[p_phrase, p_case], rounds);
    let (near_ord_ids, near_ord_ids_n) = time_sql(&db, &ordered_ids, &[p_near, p_case], rounds);
    let (near_ord_text, near_ord_text_n) = time_sql(&db, &ordered_text, &[p_near, p_case], rounds);

    // Üretim API'si: ilk sayfa (limit 20, doğrulama Rust'ta).
    let mut api_times = Vec::with_capacity(rounds);
    let mut api_examined = 0_i64;
    for _ in 0..rounds {
        let started = Instant::now();
        let page = db
            .search_page(
                &case.id,
                &SearchQuery::auto("itirazın reddine"),
                &SearchFilters::default(),
                20,
                None,
                false,
            )
            .expect("arama");
        api_times.push(started.elapsed().as_secs_f64() * 1000.0);
        api_examined = page.verification.candidates_examined;
    }

    let mut report = String::new();
    report.push_str("{\n");
    report.push_str(&format!(
        "\"corpus\":{{\"decoys\":{decoys},\"hits\":{hits},\"segments\":{segments},\"rounds\":{rounds},\"ingestMs\":{ingest_ms:.1}}},\n"
    ));
    report.push_str(&format!(
        "\"candidates\":{{\"broad\":{fts_broad_n},\"phrase\":{fts_phrase_n},\"near\":{fts_near_n}}},\n"
    ));
    report.push_str(&format!("\"ftsCountBroad\":{},\n", summarize(&fts_broad)));
    report.push_str(&format!("\"ftsCountPhrase\":{},\n", summarize(&fts_phrase)));
    report.push_str(&format!("\"ftsCountNear\":{},\n", summarize(&fts_near)));
    report.push_str(&format!(
        "\"orderedIds\":{},\"orderedIdsRows\":{ord_ids_n},\n",
        summarize(&ord_ids)
    ));
    report.push_str(&format!(
        "\"unorderedIds\":{},\"unorderedIdsRows\":{unord_ids_n},\n",
        summarize(&unord_ids)
    ));
    report.push_str(&format!(
        "\"orderedText\":{},\"orderedTextRows\":{ord_text_n},\n",
        summarize(&ord_text)
    ));
    report.push_str(&format!(
        "\"unorderedText\":{},\"unorderedTextRows\":{unord_text_n},\n",
        summarize(&unord_text)
    ));
    report.push_str(&format!(
        "\"orderedTextLimited200\":{},\"orderedTextLimited200Rows\":{ord_text_lim_n},\n",
        summarize(&ord_text_lim)
    ));
    report.push_str(&format!(
        "\"phraseOrderedIds\":{},\"phraseOrderedIdsRows\":{phr_ord_ids_n},\n",
        summarize(&phr_ord_ids)
    ));
    report.push_str(&format!(
        "\"phraseOrderedText\":{},\"phraseOrderedTextRows\":{phr_ord_text_n},\n",
        summarize(&phr_ord_text)
    ));
    report.push_str(&format!(
        "\"nearOrderedIds\":{},\"nearOrderedIdsRows\":{near_ord_ids_n},\n",
        summarize(&near_ord_ids)
    ));
    report.push_str(&format!(
        "\"nearOrderedText\":{},\"nearOrderedTextRows\":{near_ord_text_n},\n",
        summarize(&near_ord_text)
    ));
    report.push_str(&format!(
        "\"apiFirstPage\":{},\"apiCandidatesExamined\":{api_examined}\n",
        summarize(&api_times)
    ));
    report.push_str("}\n");

    println!("{report}");
    if let Some(path) = out_path {
        std::fs::write(&path, &report).expect("rapor yazılır");
        println!("rapor: {path}");
    }
}
