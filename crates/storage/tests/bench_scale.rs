//! F15-T04/T05 — kapasite ve gecikme ölçüm koşucusu.
//!
//! Elle çalıştırılır (CI'da `#[ignore]`), çünkü ölçüm dakikalar sürer ve
//! referans donanımda yapılmalıdır. Ürettiği JSON rapordur; **eşik kapısı
//! değildir** — kabul hedefi B-01 profiline göre ayrı değerlendirilir.
//!
//! Kullanım:
//! ```bash
//! DIM_BENCH_DOCS=2000 DIM_BENCH_PAGES=3 DIM_BENCH_OUT=/tmp/bench.json \
//!   cargo test -p dim-storage --test bench_scale -- --ignored --nocapture
//! ```

use std::time::Instant;

use dim_storage::{
    AiPolicy, CaseKind, Database, DatabaseKey, NewCase, NewPage, SearchFilters, SearchMode,
    Sensitivity,
};

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

const VOCAB: &[&str] = &[
    "itirazın",
    "reddine",
    "karar",
    "verildi",
    "davacı",
    "vekili",
    "tebliğ",
    "tarih",
    "sözleşme",
    "kira",
    "bedeli",
    "ödenmedi",
    "tanık",
    "beyan",
    "mahkeme",
    "esas",
    "numaralı",
    "dosya",
    "bilirkişi",
    "raporu",
    "temyiz",
    "süre",
    "hak",
    "talep",
    "faiz",
    "alacak",
    "borç",
    "senet",
    "ipotek",
    "tapu",
    "kayıt",
    "şerh",
    "icra",
    "takip",
    "haciz",
    "ihtarname",
    "noter",
    "onaylı",
    "suret",
    "ek",
    "beyanları",
    "hususunda",
    "yönünden",
    "gereği",
    "düzenlenen",
    "incelenmesi",
];

/// Deterministik sözde-rastgele (LCG): aynı girdi → aynı korpus → aynı ölçüm.
fn lcg(seed: &mut u64) -> usize {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*seed >> 33) as usize) % VOCAB.len()
}

fn sentence(seed: &mut u64, words: usize) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(words + 1);
    for _ in 0..words {
        parts.push(VOCAB[lcg(seed)]);
    }
    parts.push(".");
    let mut text = parts.join(" ");
    // Baş harfi büyüt: gerçek metne benzesin.
    if let Some(first) = text.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    text
}

fn page_text(doc: usize, page: usize) -> String {
    let mut seed = ((doc as u64) << 32) ^ (page as u64 + 1) ^ 0x5DEECE66D;
    let mut out = String::new();
    for _ in 0..8 {
        out.push_str(&sentence(&mut seed, 12));
        out.push(' ');
    }
    out.push_str(&format!("Esas 2024/{} E.", 1000 + doc % 9000));
    out
}

fn fixed_queries() -> Vec<(String, SearchMode, bool, &'static str)> {
    let mut queries = Vec::with_capacity(200);
    let mut seed = 0x1234_5678_9ABC_DEF0u64;
    // 50 tek kelime (esnek), 50 iki kelimelik ifade (ifade kipi),
    // 50 Türkçe karaktersiz esnek arama, 50 esas/karar numarası.
    for i in 0..50 {
        let word = VOCAB[lcg(&mut seed)];
        let with_filter = i % 5 == 0;
        queries.push((word.to_string(), SearchMode::Flexible, with_filter, "word"));
    }
    for _ in 0..50 {
        let a = VOCAB[lcg(&mut seed)];
        let b = VOCAB[lcg(&mut seed)];
        queries.push((format!("\"{a} {b}\""), SearchMode::Phrase, false, "phrase"));
    }
    for _ in 0..50 {
        let word = VOCAB[lcg(&mut seed)];
        let folded = word
            .replace('ı', "i")
            .replace('ş', "s")
            .replace('ğ', "g")
            .replace('ü', "u")
            .replace('ö', "o")
            .replace('ç', "c");
        queries.push((folded, SearchMode::Flexible, false, "folded"));
    }
    for i in 0..50 {
        queries.push((
            format!("2024/{} E.", 1000 + i * 37),
            SearchMode::Docket,
            false,
            "docket",
        ));
    }
    queries
}

fn percentile(sorted: &[u128], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx] as f64 / 1000.0
}

#[test]
#[ignore = "ölçüm elle çalıştırılır; dakikalar sürer"]
fn bench_scale_report() {
    let docs = env_usize("DIM_BENCH_DOCS", 200);
    let pages_per_doc = env_usize("DIM_BENCH_PAGES", 3);
    let out_path = std::env::var("DIM_BENCH_OUT").ok();

    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("bench.db");
    let key = DatabaseKey::new([5u8; 32]);
    let db = Database::open(&path, &key).expect("kasa açılır");

    let case = db
        .create_case(&NewCase {
            title: "Ölçüm dosyası".into(),
            case_kind: CaseKind::Review,
            reference_code: Some("2024/1 E.".into()),
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: AiPolicy::Off,
        })
        .expect("dosya");

    let ingest_start = Instant::now();
    let mut run_ids: Vec<String> = Vec::with_capacity(docs);
    for doc in 0..docs {
        let blob = db
            .create_blob_record(&format!("bench-{doc}"), 1024, "aad")
            .expect("blob");
        let document = db
            .create_document(
                &case.id,
                &format!("Belge {doc:06}.pdf"),
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
                &"a".repeat(64),
                1024,
                "application/pdf",
            )
            .expect("sürüm");
        let run = db
            .start_parse_run(&version.id, &case.id, "bench", "1", "{}")
            .expect("ayrıştırma");
        let pages: Vec<NewPage> = (0..pages_per_doc as i64)
            .map(|page| NewPage {
                index: page,
                text: page_text(doc, page as usize),
                width: Some(595.0),
                height: Some(842.0),
                origin: "native".into(),
                warnings: Vec::new(),
                rotation: 0,
                coordinate_space: Some("pdf".into()),
                page_label: None,
                blocks: Vec::new(),
            })
            .collect();
        db.store_pages(&run.id, &case.id, &pages).expect("sayfalar");
        run_ids.push(run.id.clone());
        assert_eq!(version.version_no, 1);
    }
    let ingest_secs = ingest_start.elapsed().as_secs_f64();

    // Isınma: önbellek ve FTS sayfaları yerleşsin.
    let filters = SearchFilters {
        document_types: Vec::new(),
        date_role: None,
        tags: Vec::new(),
        ocr_status: None,
    };
    let _ = db
        .search_segments_page(&case.id, "karar", &filters, 50, None, false)
        .expect("ısınma");

    let queries = fixed_queries();
    let mut search_us: Vec<u128> = Vec::with_capacity(queries.len());
    let mut groups: std::collections::BTreeMap<&str, Vec<u128>> = std::collections::BTreeMap::new();
    let mut hits: usize = 0;
    for (text, _mode, with_filter, group) in &queries {
        let mut f = filters.clone();
        if *with_filter {
            f.document_types = vec!["petition".into()];
        }
        let started = Instant::now();
        let page = db
            .search_segments_page(
                &case.id, text, &f, 50, None,
                true, // gerçekçi: kullanıcı toplam sayıyı da görür
            )
            .expect("arama");
        let elapsed = started.elapsed().as_micros();
        search_us.push(elapsed);
        groups.entry(group).or_default().push(elapsed);
        hits += page.hits.len();
    }
    search_us.sort_unstable();

    // "İlk sayfa" gecikmesi: son belgenin ilk sayfasını oku.
    let last_run = run_ids.last().expect("en az bir belge");
    let mut page_us: Vec<u128> = Vec::with_capacity(50);
    for _ in 0..50 {
        let started = Instant::now();
        let pages = db.pages_for_run(last_run).expect("sayfalar");
        assert!(!pages.is_empty());
        page_us.push(started.elapsed().as_micros());
    }
    page_us.sort_unstable();

    let segments: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM text_segments", [], |r| r.get(0))
        .expect("segment sayısı");
    let page_rows: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM pages", [], |r| r.get(0))
        .expect("sayfa sayısı");
    let db_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let wal_bytes = std::fs::metadata(path.with_extension("db-wal"))
        .map(|m| m.len())
        .unwrap_or(0);

    let report = serde_json::json!({
        "generatedAt": dim_storage::now_iso(),
        "profile": "core-only (IPC/UI yok)",
        "modeResolution": "auto: tırnaklı metin → ifade, 2024/N E. → esas/karar no",
        "machine": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
        },
        "corpus": {
            "documents": docs,
            "pages": page_rows,
            "segments": segments,
            "pagesPerDocument": pages_per_doc,
        },
        "ingest": {
            "seconds": (ingest_secs * 1000.0).round() / 1000.0,
            "documentsPerSecond": (docs as f64 / ingest_secs * 100.0).round() / 100.0,
            "pagesPerSecond": (page_rows as f64 / ingest_secs * 100.0).round() / 100.0,
        },
        "search": {
            "queries": queries.len(),
            "p50Ms": percentile(&search_us, 0.50),
            "p95Ms": percentile(&search_us, 0.95),
            "maxMs": percentile(&search_us, 1.0),
            "totalHits": hits,
        },
        "searchByGroup": groups.iter_mut().map(|(group, samples)| {
            samples.sort_unstable();
            (group.to_string(), serde_json::json!({
                "queries": samples.len(),
                "p50Ms": percentile(samples, 0.50),
                "p95Ms": percentile(samples, 0.95),
            }))
        }).collect::<serde_json::Map<String, serde_json::Value>>(),
        "firstPage": {
            "samples": page_us.len(),
            "p50Ms": percentile(&page_us, 0.50),
            "p95Ms": percentile(&page_us, 0.95),
        },
        "storage": {
            "dbBytes": db_bytes,
            "walBytes": wal_bytes,
        },
    });
    let text = serde_json::to_string_pretty(&report).expect("json");
    println!("{text}");
    if let Some(path) = out_path {
        std::fs::write(path, &text).expect("rapor yazılır");
    }
}
