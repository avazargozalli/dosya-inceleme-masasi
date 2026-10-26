//! Kuyruk gecikmesi ölçümü (F15-T05 "iş kuyruğu" maddesi).
//!
//! Ölçülen üç şey ayrı raporlanır, birbirine karıştırılmaz:
//! 1. **Kuyruk mekanizması maliyeti** — `enqueue`, `claim`, `complete`
//!    çağrılarının kendi süresi (SQL + işlem).
//! 2. **Kuyruk beklemesi** — işin oluşturulma anından sahiplenilme anına
//!    kadar geçen süre; seri bir yürütücü varsayımıyla (her iş 2 ms sentetik
//!    iş) ölçülür. Bu sayı **iş süresine bağlıdır**: kuyruk doldukça büyür.
//! 3. **Döngü süresi** — sahiplen→bitir arası toplam süre (mekanizma +
//!    sentetik iş).
//!
//! Dürüst sınırlar: bu bir **çekirdek** ölçümüdür; Python worker, IPC ve
//! arayüz yoktur. Sentetik iş gerçek ayrıştırma değildir (2 ms sabit). Tek
//! makine, sıcak önbellek. Tek süreçli yürütücü ölçülür; iki worker'ın
//! gerçek paralelliği ölçülmez (kasa başına tek yazıcı bağlantı vardır).
//!
//! Çalıştırma (elle, release):
//! `DIM_QUEUE_BENCH_OUT=/mutlak/yol/bench-queue.json \
//!  cargo test -p dim-storage --release --test queue_latency -- --ignored --nocapture`

use std::time::{Duration, Instant};

use dim_storage::{CaseKind, Database, DatabaseKey, NewCase, Sensitivity, JOB_PARSE_DOCUMENT};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn stats(values: &[f64]) -> serde_json::Value {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mean = if sorted.is_empty() {
        0.0
    } else {
        sorted.iter().sum::<f64>() / sorted.len() as f64
    };
    serde_json::json!({
        "n": sorted.len(),
        "p50": (percentile(&sorted, 0.50) * 1000.0).round() / 1000.0,
        "p95": (percentile(&sorted, 0.95) * 1000.0).round() / 1000.0,
        "max": (percentile(&sorted, 1.0) * 1000.0).round() / 1000.0,
        "mean": (mean * 1000.0).round() / 1000.0,
    })
}

fn parse_ts(text: &str) -> OffsetDateTime {
    OffsetDateTime::parse(text, &Rfc3339).unwrap_or(OffsetDateTime::UNIX_EPOCH)
}

/// **Canlı sayaç yazımı (0022).** Ayrı kasa: ana senaryonun sayımlarını
/// kirletmez. İki şey ölçülür:
/// 1. tek `update_job_progress` çağrısının gecikmesi (UPDATE + WAL commit),
/// 2. `ProgressGate` (300 ms) ile 120 sayfalık bir aşamada kaç yazım olacağı —
///    iki uç hız için (hızlı belge: 50 µs/sayfa, yavaş/taranmış: 5 ms/sayfa).
///
/// İkincisi **ölçüm değil, kapı kuralının sayımı**; raporda ayrı alanda durur.
fn measure_progress_writes(pages: u32) -> serde_json::Value {
    let dir = tempfile::tempdir().expect("dizin");
    let db = Database::open(
        &dir.path().join("progress-bench.dimvault"),
        &DatabaseKey::new([23u8; 32]),
    )
    .expect("kasa");
    let case = db
        .create_case(&NewCase {
            title: "Sayaç ölçümü".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: dim_storage::AiPolicy::Off,
        })
        .expect("dosya");
    db.enqueue_job_with_priority(
        JOB_PARSE_DOCUMENT,
        Some(&case.id),
        "fp-progress",
        "parse:progress-bench",
        0,
    )
    .expect("iş");
    let job = db
        .claim_job_by_key(
            "parse:progress-bench",
            "bench-progress",
            300,
            OffsetDateTime::now_utc(),
        )
        .expect("sahiplen")
        .expect("iş var");

    let mut write_ms = Vec::with_capacity(pages as usize);
    for page in 1..=pages {
        let started = Instant::now();
        let written = db
            .update_job_progress(
                &job.id,
                "bench-progress",
                "pages",
                page,
                Some(pages),
                OffsetDateTime::now_utc(),
            )
            .expect("sayaç")
            .expect("uygulandı");
        write_ms.push(started.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(written.progress_total, Some(i64::from(pages)));
    }

    serde_json::json!({
        "write": stats(&write_ms),
        "frames": pages,
        "gateMs": 300,
        "writesFastDoc": simulated_writes(pages, 50),
        "writesSlowDoc": simulated_writes(pages, 5_000),
        "note": "write = gerçek yazım gecikmesi; writesFastDoc/writesSlowDoc kapı kuralının sayımı (kare sayısı değil).",
    })
}

/// `ProgressGate` ile aynı kare dizisinde kaç yazım üretileceğini sayar.
fn simulated_writes(pages: u32, per_page_us: u64) -> u32 {
    let mut gate = dim_storage::queue::ProgressGate::new(Duration::from_millis(300));
    let start = Instant::now();
    let mut writes = 0;
    for page in 1..=pages {
        let at = start + Duration::from_micros(u64::from(page) * per_page_us);
        if gate.should_write("pages", page, Some(pages), at) {
            writes += 1;
        }
    }
    writes
}

#[test]
#[ignore = "elle: kuyruk gecikmesi ölçümü (release, --ignored)"]
fn measure_queue_latency() {
    let jobs: usize = std::env::var("DIM_QUEUE_JOBS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400);
    let synthetic_ms: u64 = std::env::var("DIM_QUEUE_SYNTHETIC_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);

    let dir = tempfile::tempdir().expect("dizin");
    let db = Database::open(
        &dir.path().join("queue-bench.dimvault"),
        &DatabaseKey::new([17u8; 32]),
    )
    .expect("kasa");
    let case = db
        .create_case(&NewCase {
            title: "Kuyruk ölçümü".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: dim_storage::AiPolicy::Off,
        })
        .expect("dosya");

    // 1) Sıraya koyma maliyeti.
    let mut enqueue_ms = Vec::with_capacity(jobs);
    for i in 0..jobs {
        let started = Instant::now();
        db.enqueue_job_with_priority(
            JOB_PARSE_DOCUMENT,
            Some(&case.id),
            &format!("fp-{i}"),
            &format!("parse:bench-{i}"),
            0,
        )
        .expect("iş");
        enqueue_ms.push(started.elapsed().as_secs_f64() * 1000.0);
    }

    // 2) Seri yürütücü: sahiplen → sentetik iş → bitir. Kuyruk beklemesi
    //    işin oluşturulma anından sahiplenilme anına kadar ölçülür.
    let mut wait_ms = Vec::with_capacity(jobs);
    let mut loop_ms = Vec::with_capacity(jobs);
    let mut claim_ms = Vec::with_capacity(jobs);
    let mut processed = 0usize;
    while processed < jobs {
        let cycle = Instant::now();
        let claim_started = Instant::now();
        let claimed = db
            .claim_jobs(
                "bench-worker",
                1,
                300,
                &[JOB_PARSE_DOCUMENT],
                OffsetDateTime::now_utc(),
            )
            .expect("sahiplen");
        claim_ms.push(claim_started.elapsed().as_secs_f64() * 1000.0);
        let Some(job) = claimed.first() else {
            break;
        };
        wait_ms.push(
            (OffsetDateTime::now_utc() - parse_ts(&job.created_at)).as_seconds_f64() * 1000.0,
        );
        std::thread::sleep(Duration::from_millis(synthetic_ms));
        db.complete_job(&job.id, "bench-worker", OffsetDateTime::now_utc())
            .expect("bitir");
        loop_ms.push(cycle.elapsed().as_secs_f64() * 1000.0);
        processed += 1;
    }

    // 3) Toplu sahiplenme (pencere): 4'lük parti kuyruk beklemesini kısaltır mı?
    let mut batch_wait_ms = Vec::new();
    let batch_jobs = 40usize;
    for i in 0..batch_jobs {
        db.enqueue_job_with_priority(
            JOB_PARSE_DOCUMENT,
            Some(&case.id),
            &format!("fp-b{i}"),
            &format!("parse:batch-{i}"),
            0,
        )
        .expect("iş");
    }
    let mut done = 0usize;
    while done < batch_jobs {
        let claimed = db
            .claim_jobs(
                "bench-worker",
                4,
                300,
                &[JOB_PARSE_DOCUMENT],
                OffsetDateTime::now_utc(),
            )
            .expect("sahiplen");
        if claimed.is_empty() {
            break;
        }
        for job in &claimed {
            batch_wait_ms.push(
                (OffsetDateTime::now_utc() - parse_ts(&job.created_at)).as_seconds_f64() * 1000.0,
            );
            db.complete_job(&job.id, "bench-worker", OffsetDateTime::now_utc())
                .expect("bitir");
            done += 1;
        }
    }

    let summary = db.job_queue_summary().expect("özet");
    assert_eq!(summary.total as usize, jobs + batch_jobs);
    let succeeded = summary
        .by_state
        .iter()
        .find(|s| s.state == "completed")
        .map(|s| s.count)
        .unwrap_or(0);
    assert_eq!(succeeded as usize, jobs + batch_jobs, "her iş bitmeli");
    assert_eq!(processed, jobs);

    // 4) Canlı sayaç yazımı (0022) — ayrı kasa, ana senaryoyu kirletmez.
    let progress_pages: u32 = std::env::var("DIM_PROGRESS_PAGES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);
    let progress = measure_progress_writes(progress_pages);

    let report = serde_json::json!({
        "bench": "queue_latency",
        "generatedAt": OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default(),
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "jobs": jobs,
        "syntheticWorkMs": synthetic_ms,
        "note": "Çekirdek ölçüm: Python worker/IPC/arayüz yok; sentetik iş sabit süreli.",
        "ms": {
            "enqueue": stats(&enqueue_ms),
            "claim": stats(&claim_ms),
            "completeLoop": stats(&loop_ms),
            "queueWaitSerial": stats(&wait_ms),
            "queueWaitBatch4": stats(&batch_wait_ms),
        },
        "progress": progress,
        "summary": summary,
    });
    let text = serde_json::to_string_pretty(&report).expect("json");
    match std::env::var("DIM_QUEUE_BENCH_OUT") {
        Ok(path) => {
            std::fs::write(&path, text).expect("rapor yazılamadı");
            println!("rapor: {path}");
        }
        Err(_) => println!("{text}"),
    }
    println!("işlenen: {processed}, partili: {done}");
}
