//! F15-T04/T05 — kapasite ve gecikme ölçüm koşucusu.
//!
//! Elle çalıştırılır (CI'da `#[ignore]`), çünkü ölçüm dakikalar sürer ve
//! referans donanımda yapılmalıdır. Ürettiği JSON **rapordur**; kapı değildir —
//! kabul hedefi B-01 profiline göre ayrı değerlendirilir (rapor `b01` bloğunda
//! hedefin uygulanıp uygulanmadığını dürüstçe yazar).
//!
//! Ölçülen katman **çekirdek**: IPC ve arayüz gösterimi yok. B-01 toplam süreyi
//! kapsadığı için buradaki sayı alt sınırdır (rapora yazılır).
//!
//! Kullanım (B-01 profili: 1.000 belge / 100.000 segment / 200 sorgu):
//! ```bash
//! DIM_BENCH_DOCS=1000 DIM_BENCH_PAGES=100 DIM_BENCH_QUERIES_PER_GROUP=50 \
//!   DIM_BENCH_OUT=/tmp/bench-b01.json \
//!   cargo test -p dim-storage --test bench_scale -- --ignored --nocapture
//! ```
//!
//! Ortam değişkenleri:
//! - `DIM_BENCH_DOCS` (varsayılan 200), `DIM_BENCH_PAGES` (3) — korpus boyu.
//! - `DIM_BENCH_QUERIES_PER_GROUP` (50) → toplam 4×N sorgu; B-01 için N=50.
//! - `DIM_BENCH_PAGE_ONLY_EVERY` (10) — her N. sorguda toplam sayı **istenmeden**
//!   de ölçülür; `include_total` maliyeti ayrı raporlanır (0 = kapalı).
//! - `DIM_BENCH_PROGRESS_EVERY` (otomatik) — yükleme sırasında ilerleme satırı.
//! - `DIM_BENCH_PAGE_AB=1` — indeksli/indekssiz `pages_for_run` A/B ölçümü
//!   (indeks geçici düşürülür, ölçümden sonra geri kurulur).
//! - `DIM_BENCH_OUT` — JSON raporun yazılacağı yol; boşsa yalnız stdout.

// Ölçüm koşucusu yalnız geliştirme makinesinde elle çalışır (`#[ignore]`).
// Buradaki süreç başlatma **tanılama** içindir: `ps` ile süreç RSS'i,
// `sysctl` ile donanım kimliği (CPU modeli, RAM). Ürün yolunda süreç başlatma
// yasağı (ADR-05) aynen geçerlidir; bu istisna yalnız bu test dosyası içindir.
#![allow(clippy::disallowed_methods)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dim_storage::{
    AiPolicy, CaseKind, Database, DatabaseKey, NewCase, NewPage, SearchFilters, SearchMode,
    SearchQuery, Sensitivity,
};
use sha2::{Digest, Sha256};

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

/// `DIM_BENCH_*_AB=1` gibi bayraklar.
fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("yes")
    )
}

/// Kabuk komutunun tek satır çıktısı; başarısızsa `None` (ölçüm düşmez).
fn command_line(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn macos_sysctl(key: &str) -> Option<String> {
    command_line("sysctl", &["-n", key])
}

/// Süreç RSS'i (bayt). `ps` alt süreci `unsafe` gerektirmez (workspace kapısı);
/// Windows'ta `ps` yok → `None` ve raporda alan boş kalır.
fn rss_bytes() -> Option<u64> {
    let pid = std::process::id().to_string();
    let text = command_line("ps", &["-o", "rss=", "-p", &pid])?;
    let kib: u64 = text.trim().parse().ok()?;
    Some(kib * 1024)
}

/// Arka planda RSS örnekleyip **tepe** değeri tutar. Örnekleme aralığı
/// varsayılan 2 sn: ölçümün kendisini yavaşlatmayacak kadar seyrek.
struct RssSampler {
    stop: Arc<AtomicBool>,
    peak: Arc<AtomicU64>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl RssSampler {
    fn start(interval: Duration) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let peak = Arc::new(AtomicU64::new(rss_bytes().unwrap_or(0)));
        let handle = {
            let stop = Arc::clone(&stop);
            let peak = Arc::clone(&peak);
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    if let Some(bytes) = rss_bytes() {
                        peak.fetch_max(bytes, Ordering::Relaxed);
                    }
                    std::thread::sleep(interval);
                }
            })
        };
        Self {
            stop,
            peak,
            handle: Some(handle),
        }
    }

    fn peak(&self) -> u64 {
        self.peak.load(Ordering::Relaxed)
    }

    /// Örnekleyiciyi durdurur ve tepe değeri döner.
    fn finish(mut self) -> u64 {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.peak()
    }
}

impl Drop for RssSampler {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // `join` yok: düşen bir ölçümde iş parçacığı en fazla bir aralık yaşar.
    }
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

/// Korpus üreticisinin kimliği: aynı değer → aynı korpus. Rapora yazılır ki
/// ölçüm başka bir turla karşılaştırılabilir olsun.
const CORPUS_GENERATOR: &str =
    "lcg-6364136223846793005-v1 (46 kelime, 8 cümle × 12 kelime, Esas 2024/N E.)";

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

#[derive(Clone)]
struct BenchQuery {
    text: String,
    mode: SearchMode,
    with_filter: bool,
    group: &'static str,
}

impl BenchQuery {
    fn as_report(&self) -> serde_json::Value {
        serde_json::json!({
            "text": self.text,
            "mode": format!("{:?}", self.mode),
            "group": self.group,
            "filter": if self.with_filter { "documentType=petition" } else { "" },
        })
    }
}

/// Sabit sorgu listesi. Gruplar **dönüşümlü** sıralanır: koşu bir bütçeyle
/// kesilse bile her grup örneklenmiş olur (eski sıralamada kesme yalnız ilk
/// grubu ölçerdi).
fn fixed_queries(per_group: usize) -> Vec<BenchQuery> {
    let mut seed = 0x1234_5678_9ABC_DEF0u64;
    let mut words: Vec<BenchQuery> = Vec::with_capacity(per_group);
    let mut phrases: Vec<BenchQuery> = Vec::with_capacity(per_group);
    let mut folded: Vec<BenchQuery> = Vec::with_capacity(per_group);
    let mut dockets: Vec<BenchQuery> = Vec::with_capacity(per_group);

    for i in 0..per_group {
        let word = VOCAB[lcg(&mut seed)];
        words.push(BenchQuery {
            text: word.to_string(),
            mode: SearchMode::Flexible,
            with_filter: i % 5 == 0,
            group: "word",
        });
    }
    for _ in 0..per_group {
        let a = VOCAB[lcg(&mut seed)];
        let b = VOCAB[lcg(&mut seed)];
        phrases.push(BenchQuery {
            text: format!("\"{a} {b}\""),
            mode: SearchMode::Phrase,
            with_filter: false,
            group: "phrase",
        });
    }
    for _ in 0..per_group {
        let word = VOCAB[lcg(&mut seed)];
        folded.push(BenchQuery {
            text: word
                .replace('ı', "i")
                .replace('ş', "s")
                .replace('ğ', "g")
                .replace('ü', "u")
                .replace('ö', "o")
                .replace('ç', "c"),
            mode: SearchMode::Flexible,
            with_filter: false,
            group: "folded",
        });
    }
    for i in 0..per_group {
        dockets.push(BenchQuery {
            text: format!("2024/{} E.", 1000 + i * 37),
            mode: SearchMode::Docket,
            with_filter: false,
            group: "docket",
        });
    }

    let groups = [words, phrases, folded, dockets];
    let mut out: Vec<BenchQuery> = Vec::with_capacity(per_group * groups.len());
    for index in 0..per_group {
        for group in &groups {
            if let Some(query) = group.get(index) {
                out.push(query.clone());
            }
        }
    }
    out
}

fn percentile(sorted: &[u128], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx] as f64 / 1000.0
}

fn rounded(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn mb(bytes: u64) -> f64 {
    rounded(bytes as f64 / (1024.0 * 1024.0))
}

#[test]
#[ignore = "ölçüm elle çalıştırılır; dakikalar sürer"]
fn bench_scale_report() {
    let docs = env_usize("DIM_BENCH_DOCS", 200);
    let pages_per_doc = env_usize("DIM_BENCH_PAGES", 3);
    let per_group = env_usize("DIM_BENCH_QUERIES_PER_GROUP", 50);
    let page_only_every = std::env::var("DIM_BENCH_PAGE_ONLY_EVERY")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(10);
    let progress_every = env_usize("DIM_BENCH_PROGRESS_EVERY", std::cmp::max(docs / 20, 200));
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

    // Faz süreleri ayrı: yükleme darboğazının hangi adımda olduğu raporda görünür.
    let mut phase_us = [0u128; 5]; // blob · belge · sürüm · koşu · sayfalar
    let sampler = RssSampler::start(Duration::from_secs(2));
    let ingest_start = Instant::now();
    let mut run_ids: Vec<String> = Vec::with_capacity(docs);
    for doc in 0..docs {
        let started = Instant::now();
        let blob = db
            .create_blob_record(&format!("bench-{doc}"), 1024, "aad")
            .expect("blob");
        phase_us[0] += started.elapsed().as_micros();

        let started = Instant::now();
        let document = db
            .create_document(
                &case.id,
                &format!("Belge {doc:06}.pdf"),
                "petition",
                None,
                "day",
            )
            .expect("belge");
        phase_us[1] += started.elapsed().as_micros();

        let started = Instant::now();
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
        phase_us[2] += started.elapsed().as_micros();

        let started = Instant::now();
        let run = db
            .start_parse_run(&version.id, &case.id, "bench", "1", "{}")
            .expect("ayrıştırma");
        phase_us[3] += started.elapsed().as_micros();

        let started = Instant::now();
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
        phase_us[4] += started.elapsed().as_micros();

        run_ids.push(run.id.clone());
        assert_eq!(version.version_no, 1);

        if doc > 0 && doc % progress_every == 0 {
            let elapsed = ingest_start.elapsed().as_secs_f64();
            println!(
                "[yükleme {doc}/{docs}] {elapsed:.1} sn · {:.1} belge/sn · rss {} MB",
                doc as f64 / elapsed,
                rss_bytes().map_or(-1.0, mb)
            );
        }
    }
    let ingest_secs = ingest_start.elapsed().as_secs_f64();
    let ingest_peak = sampler.finish();

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

    let queries = fixed_queries(per_group);
    let mut search_us: Vec<u128> = Vec::with_capacity(queries.len());
    let mut page_only_us: Vec<u128> = Vec::new();
    // Ertelenmiş sayımın (`search_total`) ayrı süresi (F15).
    let mut deferred_us: Vec<u128> = Vec::new();
    let mut totals_matched = 0usize;
    let mut groups: std::collections::BTreeMap<&str, Vec<u128>> = std::collections::BTreeMap::new();
    let mut per_query: Vec<serde_json::Value> = Vec::with_capacity(queries.len());
    let mut hits: usize = 0;
    let mut hits_page_only_checked = 0usize;
    let search_start = Instant::now();
    for (index, query) in queries.iter().enumerate() {
        let mut f = filters.clone();
        if query.with_filter {
            f.document_types = vec!["petition".into()];
        }
        let started = Instant::now();
        let page = db
            .search_segments_page(
                &case.id,
                &query.text,
                &f,
                50,
                None,
                true, // gerçekçi: kullanıcı toplam sayıyı da görür
            )
            .expect("arama");
        let elapsed = started.elapsed().as_micros();
        search_us.push(elapsed);
        groups.entry(query.group).or_default().push(elapsed);
        hits += page.hits.len();

        // `include_total` maliyetini ayırmak için aynı sorgu bir de toplam
        // istenmeden koşar; **ilk sayfa birebir aynı olmalı** (kanıt, iddia değil).
        let mut page_only_value = serde_json::Value::Null;
        let mut deferred_total_value = serde_json::Value::Null;
        if page_only_every > 0 && index % page_only_every == 0 {
            let started = Instant::now();
            let without_total = db
                .search_segments_page(&case.id, &query.text, &f, 50, None, false)
                .expect("arama (toplamsız)");
            let elapsed_page_only = started.elapsed().as_micros();
            page_only_us.push(elapsed_page_only);
            let ids_with: Vec<&str> = page.hits.iter().map(|h| h.segment_id.as_str()).collect();
            let ids_without: Vec<&str> = without_total
                .hits
                .iter()
                .map(|h| h.segment_id.as_str())
                .collect();
            assert_eq!(
                ids_with, ids_without,
                "toplam istenmeden ilk sayfa değişmemeli"
            );
            assert_eq!(without_total.total, None, "toplamsız aramada toplam yok");
            hits_page_only_checked += 1;
            page_only_value = serde_json::json!(rounded(elapsed_page_only as f64 / 1000.0));

            // Ertelenmiş sayım (arayüzün yeni yolu): **aynı** sayıyı vermeli ve
            // maliyeti sayfadan ayrı ölçülmeli.
            let started = Instant::now();
            let deferred = db
                .search_total(&case.id, &SearchQuery::auto(&query.text), &f)
                .expect("ertelenmiş sayım");
            deferred_us.push(started.elapsed().as_micros());
            assert_eq!(
                page.total,
                Some(deferred.total),
                "ertelenmiş sayım sayfa toplamıyla birebir aynı olmalı"
            );
            totals_matched += 1;
            deferred_total_value = serde_json::json!(rounded(deferred.took_ms));
        }

        let mut entry = query.as_report();
        let object = entry.as_object_mut().expect("nesne");
        object.insert(
            "withTotalMs".into(),
            serde_json::json!(rounded(elapsed as f64 / 1000.0)),
        );
        object.insert("pageOnlyMs".into(), page_only_value);
        object.insert("deferredTotalMs".into(), deferred_total_value);
        object.insert("hits".into(), serde_json::json!(page.hits.len()));
        object.insert("total".into(), serde_json::json!(page.total));
        object.insert("hasMore".into(), serde_json::json!(page.has_more));
        per_query.push(entry);

        if index > 0 && index % 20 == 0 {
            let elapsed = search_start.elapsed().as_secs_f64();
            println!("[arama {index}/{}] {elapsed:.1} sn", queries.len());
        }
    }
    search_us.sort_unstable();
    page_only_us.sort_unstable();
    let search_secs = search_start.elapsed().as_secs_f64();

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
    let search_peak = rss_bytes().unwrap_or(0);

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
    let shm_bytes = std::fs::metadata(path.with_extension("db-shm"))
        .map(|m| m.len())
        .unwrap_or(0);

    // --- İndeks A/B (F15-T05 kusuru) ---------------------------------------
    // Tek sayfalık ayrı bir koşu açılır; `pages_for_run` maliyeti sayfa başına
    // ölçülür (sayfa sayısıyla doğrusal). Sonra `text_segments.page_id`
    // indeksi düşürülüp aynı okuma tekrarlanır ve indeks geri kurulur.
    let mut page_ab = serde_json::Value::Null;
    if env_flag("DIM_BENCH_PAGE_AB") {
        let tiny_doc = db
            .create_document(&case.id, "Tek sayfa", "petition", None, "day")
            .expect("belge");
        let tiny_version = db
            .add_version(
                &tiny_doc.id,
                &case.id,
                &db.create_blob_record("bench-tiny", 1024, "aad")
                    .expect("blob"),
                &"b".repeat(64),
                1024,
                "application/pdf",
            )
            .expect("sürüm");
        let tiny_run = db
            .start_parse_run(&tiny_version.id, &case.id, "bench", "1", "{}")
            .expect("koşu");
        db.store_pages(
            &tiny_run.id,
            &case.id,
            &[NewPage {
                index: 0,
                text: page_text(docs, 0),
                width: Some(595.0),
                height: Some(842.0),
                origin: "native".into(),
                ..Default::default()
            }],
        )
        .expect("tek sayfa");

        let mut indexed_us: Vec<u128> = Vec::new();
        for _ in 0..20 {
            let started = Instant::now();
            let pages = db.pages_for_run(&tiny_run.id).expect("sayfa (indeksli)");
            assert_eq!(pages.len(), 1);
            indexed_us.push(started.elapsed().as_micros());
        }
        db.conn()
            .execute("DROP INDEX IF EXISTS idx_text_segments_page", [])
            .expect("indeks düşürülür");
        let mut unindexed_us: Vec<u128> = Vec::new();
        for _ in 0..2 {
            let started = Instant::now();
            let pages = db.pages_for_run(&tiny_run.id).expect("sayfa (indekssiz)");
            assert_eq!(pages.len(), 1);
            unindexed_us.push(started.elapsed().as_micros());
        }
        db.conn()
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_text_segments_page ON text_segments(page_id)",
                [],
            )
            .expect("indeks geri kurulur");
        indexed_us.sort_unstable();
        unindexed_us.sort_unstable();
        let per_page_indexed = percentile(&indexed_us, 0.50);
        let per_page_unindexed = percentile(&unindexed_us, 0.50);
        page_ab = serde_json::json!({
            "note": "pages_for_run, tek sayfalık koşu; maliyet sayfa sayısıyla doğrusal",
            "withIndex": {
                "samples": indexed_us.len(),
                "p50Ms": per_page_indexed,
                "maxMs": percentile(&indexed_us, 1.0),
            },
            "withoutIndex": {
                "samples": unindexed_us.len(),
                "p50Ms": per_page_unindexed,
                "maxMs": percentile(&unindexed_us, 1.0),
            },
            "speedup": if per_page_indexed > 0.0 {
                rounded(per_page_unindexed / per_page_indexed)
            } else {
                0.0
            },
            "impliedMsForDocumentWithPages": {
                "withIndex": rounded(per_page_indexed * pages_per_doc as f64),
                "withoutIndex": rounded(per_page_unindexed * pages_per_doc as f64),
            },
        });
    }

    let query_list = serde_json::Value::Array(queries.iter().map(BenchQuery::as_report).collect());
    let query_list_sha = {
        let canonical = serde_json::to_string(&query_list).expect("sorgu listesi");
        hex::encode(Sha256::digest(canonical.as_bytes()))
    };

    let p50 = percentile(&search_us, 0.50);
    let p95 = percentile(&search_us, 0.95);
    let max = percentile(&search_us, 1.0);
    let page_only_p50 = percentile(&page_only_us, 0.50);
    let page_only_p95 = percentile(&page_only_us, 0.95);
    deferred_us.sort_unstable();
    let deferred_p50 = percentile(&deferred_us, 0.50);
    let deferred_p95 = percentile(&deferred_us, 0.95);

    // B-01: 1.000 belge · 100.000 segment · 200 sorgu · p95 ≤ 2 sn. Hedef
    // yalnız profil gerçekten kurulduysa değerlendirilir; altında `null`.
    let b01_min_segments = 100_000i64;
    let b01_min_queries = 200usize;
    let b01_applies = segments >= b01_min_segments && queries.len() >= b01_min_queries;

    let report = serde_json::json!({
        "schemaVersion": 2,
        "generatedAt": dim_storage::now_iso(),
        "profile": "core-only (IPC/UI yok) — B-01 alt sınırı",
        "modeResolution": "auto: tırnaklı metin → ifade, 2024/N E. → esas/karar no",
        "machine": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
            "cpuModel": macos_sysctl("machdep.cpu.brand_string").or_else(|| command_line("uname", &["-m"])),
            "memoryBytes": macos_sysctl("hw.memsize").and_then(|v| v.parse::<u64>().ok()),
            "storage": macos_sysctl("hw.model"),
        },
        "corpusGenerator": CORPUS_GENERATOR,
        "corpus": {
            "documents": docs,
            "pages": page_rows,
            "segments": segments,
            "pagesPerDocument": pages_per_doc,
        },
        "queryListSha256": query_list_sha,
        "queryList": query_list,
        // Sorgu bazında ölçüm: p95'in sakladığı aykırı değerler burada görünür.
        "perQuery": per_query,
        "ingest": {
            "seconds": (ingest_secs * 1000.0).round() / 1000.0,
            "documentsPerSecond": rounded(docs as f64 / ingest_secs),
            "pagesPerSecond": rounded(page_rows as f64 / ingest_secs),
            "phaseMs": {
                "blob": rounded(phase_us[0] as f64 / 1000.0),
                "document": rounded(phase_us[1] as f64 / 1000.0),
                "version": rounded(phase_us[2] as f64 / 1000.0),
                "parseRun": rounded(phase_us[3] as f64 / 1000.0),
                "pages": rounded(phase_us[4] as f64 / 1000.0),
            },
        },
        "search": {
            "queries": queries.len(),
            "limit": 50,
            "includeTotal": true,
            "p50Ms": p50,
            "p95Ms": p95,
            "maxMs": max,
            "totalHits": hits,
            "seconds": rounded(search_secs),
        },
        // `include_total=false` yalnız ilk sayfa: arayüz toplamı geciktirirse
        // kazanç bu kadar (aynı sorgu, aynı ilk sayfa — test edildi).
        "searchPageOnly": {
            "queries": page_only_us.len(),
            "p50Ms": page_only_p50,
            "p95Ms": page_only_p95,
            "maxMs": percentile(&page_only_us, 1.0),
            "identicalFirstPageChecked": hits_page_only_checked,
            "overheadP50Ms": rounded((p50 - page_only_p50).max(0.0)),
            "overheadP95Ms": rounded((p95 - page_only_p95).max(0.0)),
        },
        // Ertelenmiş sayım: arayüzün gerçek yolu. Sayfa toplamı beklemiyor;
        // sayı **aynı** olmak zorunda (koşucu her örnekte karşılaştırır).
        "searchTotalDeferred": {
            "queries": deferred_us.len(),
            "pageP50Ms": page_only_p50,
            "pageP95Ms": page_only_p95,
            "totalP50Ms": deferred_p50,
            "totalP95Ms": deferred_p95,
            "totalMaxMs": percentile(&deferred_us, 1.0),
            "identicalTotalsChecked": totals_matched,
            "matchesPageTotal": totals_matched == deferred_us.len(),
            "note": "Sayfa `include_total=false` ile gelir; sayı ayrı komutla sonradan istenir. `pageP*` kullanıcının beklediği süredir (ilk sayfa), `totalP*` arka plandaki tam sayım.",
        },
        "searchByGroup": groups.iter_mut().map(|(group, samples)| {
            samples.sort_unstable();
            (group.to_string(), serde_json::json!({
                "queries": samples.len(),
                "p50Ms": percentile(samples, 0.50),
                "p95Ms": percentile(samples, 0.95),
                "maxMs": percentile(samples, 1.0),
            }))
        }).collect::<serde_json::Map<String, serde_json::Value>>(),
        "firstPage": {
            "samples": page_us.len(),
            "pagesInVersion": pages_per_doc,
            "p50Ms": percentile(&page_us, 0.50),
            "p95Ms": percentile(&page_us, 0.95),
        },
        // F15-T05 kusuru: `text_segments.page_id` indeksinin ölçümü (A/B).
        "pageLookupAb": page_ab,
        // F15-T05: bellek. Örnekleme 2 sn aralıkla; tepe değer yaklaşıktır.
        "memory": {
            "peakRssBytes": ingest_peak,
            "peakRssMb": mb(ingest_peak),
            "afterSearchRssMb": mb(search_peak),
            "samplingIntervalMs": 2000,
        },
        "b01": {
            "requirement": "1.000 belge · 100.000 segment · 200 sabit sorgu · ilk 50 sonuç · p95 ≤ 2.000 ms (çekirdek+IPC+UI)",
            "segments": segments,
            "queries": queries.len(),
            "applies": b01_applies,
            "coreP50Ms": p50,
            "coreP95Ms": p95,
            "coreMaxMs": max,
            "coreP95Met": if b01_applies { serde_json::json!(p95 <= 2000.0) } else { serde_json::Value::Null },
            // Ertelenmiş sayımda kullanıcı yalnız ilk sayfayı bekler; örneklem
            // `DIM_BENCH_PAGE_ONLY_EVERY` ile sınırlıdır (kuyruk sorgusu
            // örnekte olmayabilir) — bu yüzden hüküm ayrı alanda verilir.
            "deferredPageP95Ms": page_only_p95,
            "deferredPageQueries": page_only_us.len(),
            "deferredPageP95Met": if b01_applies && !page_only_us.is_empty() {
                serde_json::json!(page_only_p95 <= 2000.0)
            } else {
                serde_json::Value::Null
            },
            "note": if b01_applies {
                "Çekirdek p95 ölçüldü. B-01 toplam süreyi (çekirdek+IPC+arayüz) kapsar; bu sayı alt sınırdır ve yalnız bu makine için geçerlidir."
            } else {
                "Korpus ya da sorgu sayısı B-01 profilinin altında: sonuç hedef hükmü değildir."
            },
        },
        "costPerSegmentMs": {
            "p50": rounded(p50 / segments as f64 * 1000.0) / 1000.0,
            "p95": rounded(p95 / segments as f64 * 1000.0) / 1000.0,
        },
        "storage": {
            "dbBytes": db_bytes,
            "dbMb": mb(db_bytes),
            "walBytes": wal_bytes,
            "shmBytes": shm_bytes,
            "bytesPerSegment": if segments > 0 { db_bytes / segments as u64 } else { 0 },
        },
    });
    let text = serde_json::to_string_pretty(&report).expect("json");
    println!("{text}");
    if let Some(path) = out_path {
        std::fs::write(path, &text).expect("rapor yazılır");
    }
}
