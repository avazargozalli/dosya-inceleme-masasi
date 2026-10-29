//! F15-T05 — **OCR gecikme** ölçüm koşucusu (yayımlanan p50/p95).
//!
//! Neden ayrı koşucu: yayımlanan ölçümler arasında "OCR" maddesi **yoktu**
//! (`docs/releases/F15/BENCHMARK.md`: "OCR ve iş kuyruğu gecikmesi ölçülmedi").
//! OCR, ürünün en pahalı yerel işi (sayfa başına rasterleştirme + Tesseract) ve
//! tek sayfalık fikstür toplam maliyeti göstermiyordu. Bu koşucu **gerçek
//! worker'ı** (Python + PyMuPDF + Tesseract), **gerçek çok sayfalı taramayı**
//! (`fixtures/synthetic/scanned-pages.pdf`, metin katmanı yok) ve **OCR
//! bütçesini** (`WorkerKind::Ocr`) birlikte kullanır; süreç başlatma dahildir.
//!
//! Ölçüm **rapordur, kapı değildir**: kabul hedefi F15-T05'te ayrıca
//! değerlendirilir. Yine de sessiz bozulmaya karşı katı doğrulamalar var:
//! OCR her sayfayı okumazsa, sayfa sayısı tutmazsa ya da bütçe aşılırsa koşu
//! **düşer** — "0 ms, 0 sayfa" gibi bir rapor üretilmez.
//!
//! Kullanım:
//! ```bash
//! DIM_WORKER_PYTHON="$PWD/workers/document/.venv/bin/python" \
//!   DIM_BENCH_OCR_RUNS=10 \
//!   DIM_BENCH_OUT=docs/releases/F15/bench-ocr-2026-10-29.json \
//!   cargo test -p dim-ingest --test bench_ocr -- --ignored --nocapture
//! ```
//!
//! Ortam değişkenleri:
//! - `DIM_BENCH_OCR_RUNS` (10) — çok sayfalı belge için koşu sayısı.
//! - `DIM_BENCH_OCR_SINGLE_RUNS` (5) — tek sayfa fikstürü için koşu sayısı.
//! - `DIM_BENCH_OCR_LANG` (`tur`) — Tesseract dil paketi; yoksa koşu düşer
//!   (sessizce `eng`e düşülmez).
//! - `DIM_BENCH_OUT` — JSON rapor yolu; boşsa yalnız stdout.

// Ölçüm koşucusu yalnız geliştirme makinesinde elle çalışır (`#[ignore]`).
// Buradaki süreç başlatma **tanılama** içindir: `sysctl` donanım kimliği,
// `tesseract --version` motor sürümü. Ürün yolunda süreç başlatma yasağı
// (ADR-05) aynen geçerlidir; bu istisna yalnız bu test dosyası içindir.
#![allow(clippy::disallowed_methods)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dim_ingest::budget::{WorkerBudget, WorkerKind};
use dim_ingest::worker::{dev_python_with_module, dev_worker_dir, Extraction, WorkerClient};
use serde_json::json;

const DOC_PAGES: usize = 6;

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn env_string(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
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

/// Doğrusal ara değerli sıra istatistiği (numpy varsayılanı): küçük
/// örneklerde en yakın sıraya yuvarlamak p50'yi yukarı kaydırıyordu.
fn percentile(sorted: &[u128], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let position = (sorted.len() as f64 - 1.0) * p.clamp(0.0, 1.0);
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let weight = position - lower as f64;
    let low = sorted[lower] as f64;
    let high = sorted[upper] as f64;
    (low + (high - low) * weight) / 1000.0
}

fn rounded(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn mb(bytes: u64) -> f64 {
    rounded(bytes as f64 / (1024.0 * 1024.0))
}

#[cfg(test)]
mod percentile_tests {
    use super::percentile;

    #[test]
    fn empty_sample_reports_zero_instead_of_panicking() {
        assert_eq!(percentile(&[], 0.5), 0.0);
    }

    #[test]
    fn single_sample_is_its_own_median_and_p95() {
        let sample = [1_500u128];
        assert_eq!(percentile(&sample, 0.5), 1.5);
        assert_eq!(percentile(&sample, 0.95), 1.5);
    }

    #[test]
    fn p50_sits_between_neighbours_and_p95_keeps_the_tail() {
        let sample: Vec<u128> = (1..=10).map(|v| v * 1_000).collect();
        let close = |value: f64, expected: f64| {
            assert!(
                (value - expected).abs() < 1e-9,
                "{value} ≈ {expected} değil"
            );
        };
        // Doğrusal ara değer: (10-1)*0.5 = 4.5 → 5 ve 6 ms arasında orta → 5,5 ms
        close(percentile(&sample, 0.5), 5.5);
        // (10-1)*0.95 = 8.55 → 9 ms + 0,55 × (10-9) → 9,55 ms
        close(percentile(&sample, 0.95), 9.55);
        // Uçlar örnekle birebir: en küçük ve en büyük.
        close(percentile(&sample, 0.0), 1.0);
        close(percentile(&sample, 1.0), 10.0);
    }
}

/// Tek koşunun ölçümü. `elapsed` süreç başlatmayı **içerir** (kullanıcı
/// deneyimi: OCR işi başladığında geçen süre).
struct Run {
    elapsed: Duration,
    pages: usize,
    text_chars: usize,
    warnings: usize,
    peak_memory_bytes: u64,
    output_bytes: u64,
}

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// Bir OCR koşusu: yeni worker süreci → `extract_text` → bütçe raporu → kapat.
fn measure_once(
    python: &Path,
    worker_dir: &Path,
    data: &[u8],
    lang: &str,
    ocr: bool,
) -> (Run, Extraction) {
    let budget = WorkerBudget::for_kind(if ocr {
        WorkerKind::Ocr
    } else {
        WorkerKind::Extract
    });
    let started = Instant::now();
    let mut client =
        WorkerClient::spawn_dev_budgeted(python, worker_dir, budget).expect("worker başlatılamadı");
    let extraction = client
        .extract_text(data, "application/pdf", ocr, lang, None, None)
        .unwrap_or_else(|error| {
            panic!("OCR çıkarımı düştü (dil paketi '{lang}' kurulu mu?): {error:?}")
        });
    let elapsed = started.elapsed();
    let report = client.budget_report().expect("bütçe raporu");
    client.shutdown();
    assert_eq!(
        report.violated, None,
        "gerçek OCR işi bütçeyi aşmamalı: {:?}",
        report.violated
    );
    let run = Run {
        elapsed,
        pages: extraction.pages.len(),
        text_chars: extraction
            .pages
            .iter()
            .map(|p| p.text.chars().count())
            .sum(),
        warnings: extraction.warnings.len(),
        peak_memory_bytes: report.peak_memory_bytes,
        output_bytes: report.output_bytes,
    };
    (run, extraction)
}

/// OCR'ın **gerçekten** okuduğunu doğrular: her sayfa kendi başlığını taşır.
/// Tesseract sessizce boş metin döndürürse rapor "hızlı" görünürdü — bu kapı
/// onu engeller.
fn assert_every_page_was_read(extraction: &Extraction, pages: usize) {
    assert_eq!(
        extraction.pages.len(),
        pages,
        "OCR sayfa sayısı fikstürle eşleşmeli ({pages})"
    );
    for (index, page) in extraction.pages.iter().enumerate() {
        let marker = format!("MADDE {}", index + 1);
        assert!(
            page.text.contains(&marker),
            "sayfa {} okunmadı: {:?}",
            index + 1,
            page.text.chars().take(60).collect::<String>()
        );
    }
}

#[test]
#[ignore = "gerçek worker + Tesseract ölçümü: DIM_WORKER_PYTHON ile elle koşulur"]
fn bench_ocr_latency_report() {
    let worker_dir = dev_worker_dir().expect("workers/document klasörü bulunamadı");
    let python = dev_python_with_module(&worker_dir, "pytesseract")
        .expect("pytesseract içe aktaran yorumlayıcı yok (kurulum: pip install -e '.[ocr]')");
    let lang = env_string("DIM_BENCH_OCR_LANG", "tur");
    let runs = env_usize("DIM_BENCH_OCR_RUNS", 10);
    let single_runs = env_usize("DIM_BENCH_OCR_SINGLE_RUNS", 5);

    let document_path = fixture("../../fixtures/synthetic/scanned-pages.pdf");
    let single_path = fixture("../../fixtures/synthetic/scanned-page.pdf");
    let document = std::fs::read(&document_path).expect("scanned-pages.pdf okunamadı");
    let single = std::fs::read(&single_path).expect("scanned-page.pdf okunamadı");
    let document_sha = dim_ingest::sha256_hex(&document);

    // --- A) çok sayfalı belge -------------------------------------------
    let mut document_runs: Vec<Run> = Vec::new();
    let mut warmup: Option<Duration> = None;
    for index in 0..runs {
        let (run, extraction) = measure_once(&python, &worker_dir, &document, &lang, true);
        assert_every_page_was_read(&extraction, DOC_PAGES);
        if index == 0 {
            warmup = Some(run.elapsed);
        }
        println!(
            "belge koşu {}/{}: {:.0} ms · {} sayfa · {} karakter · tepe {:.1} MiB · uyarı {}",
            index + 1,
            runs,
            run.elapsed.as_secs_f64() * 1000.0,
            run.pages,
            run.text_chars,
            mb(run.peak_memory_bytes),
            run.warnings
        );
        document_runs.push(run);
    }

    // --- B) tek sayfa (sayfa başına maliyetin doğrudan ölçümü) ----------
    let mut single_runs_measured: Vec<Run> = Vec::new();
    for index in 0..single_runs {
        let (run, extraction) = measure_once(&python, &worker_dir, &single, &lang, true);
        assert_every_page_was_read(&extraction, 1);
        println!(
            "tek sayfa koşu {}/{}: {:.0} ms · {} karakter · tepe {:.1} MiB",
            index + 1,
            single_runs,
            run.elapsed.as_secs_f64() * 1000.0,
            run.text_chars,
            mb(run.peak_memory_bytes)
        );
        single_runs_measured.push(run);
    }

    // --- C) OCR kapalı taban çizgisi ------------------------------------
    // Aynı taramada metin katmanı yok: OCR kapalıyken metin **boş** gelir.
    // Bu, "OCR gerçekten çalışıyor mu" sorusunun ölçülmüş kanıtıdır; taban
    // çizgisi olmadan "OCR pahalı" demek eksik olurdu.
    let (baseline_run, baseline_extraction) =
        measure_once(&python, &worker_dir, &document, &lang, false);
    let baseline_chars: usize = baseline_extraction
        .pages
        .iter()
        .map(|p| p.text.chars().count())
        .sum();
    assert_eq!(
        baseline_chars, 0,
        "OCR kapalıyken taranmış sayfadan metin gelmemeli (fikstür metin katmansız)"
    );
    println!(
        "OCR kapalı taban: {:.0} ms · {} sayfa · {} karakter (metin katmanı yok)",
        baseline_run.elapsed.as_secs_f64() * 1000.0,
        baseline_extraction.pages.len(),
        baseline_chars
    );

    // --- rapor ----------------------------------------------------------
    let mut document_times: Vec<u128> = document_runs
        .iter()
        .map(|r| r.elapsed.as_micros())
        .collect();
    document_times.sort_unstable();
    let mut single_times: Vec<u128> = single_runs_measured
        .iter()
        .map(|r| r.elapsed.as_micros())
        .collect();
    single_times.sort_unstable();

    let report = json!({
        "benchmark": "F15-T05 OCR gecikmesi",
        "generatedAt": chrono_like_timestamp(),
        "profile": {
            "kind": "ocr",
            "lang": lang,
            "worker": "python -m dim_worker.worker (gerçek süreç, OCR bütçesi)",
            "budget": {
                "cpuSeconds": WorkerBudget::for_kind(WorkerKind::Ocr).cpu_seconds,
                "memoryMiB": mb(WorkerBudget::for_kind(WorkerKind::Ocr).memory_bytes),
                "outputMiB": mb(WorkerBudget::for_kind(WorkerKind::Ocr).output_bytes),
                "wallClockSeconds": WorkerBudget::for_kind(WorkerKind::Ocr).wall_clock.as_secs(),
            }
        },
        "environment": {
            "cpuModel": command_line("sysctl", &["-n", "machdep.cpu.brand_string"]),
            "memoryBytes": command_line("sysctl", &["-n", "hw.memsize"])
                .and_then(|v| v.parse::<u64>().ok()),
            "machine": command_line("sysctl", &["-n", "hw.model"]),
            "os": command_line("sw_vers", &["-productVersion"]),
            "arch": command_line("uname", &["-m"]),
            "tesseractVersion": command_line("tesseract", &["--version"]),
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "workerPython": python.display().to_string(),
        },
        "fixtures": {
            "document": {
                "path": "fixtures/synthetic/scanned-pages.pdf",
                "pages": DOC_PAGES,
                "bytes": document.len(),
                "sha256": document_sha,
                "textLayer": false,
            },
            "singlePage": {
                "path": "fixtures/synthetic/scanned-page.pdf",
                "pages": 1,
                "bytes": single.len(),
                "sha256": dim_ingest::sha256_hex(&single),
                "textLayer": false,
            },
        },
        "document": {
            "runs": runs,
            "p50Ms": rounded(percentile(&document_times, 0.5)),
            "p95Ms": rounded(percentile(&document_times, 0.95)),
            "maxMs": rounded(percentile(&document_times, 1.0)),
            "minMs": rounded(percentile(&document_times, 0.0)),
            "firstRunMs": warmup.map(|d| rounded(d.as_secs_f64() * 1000.0)),
            "perPageP50Ms": rounded(percentile(&document_times, 0.5) / DOC_PAGES as f64),
            "pagesPerRun": document_runs.iter().map(|r| r.pages).max(),
            "textCharsPerRun": document_runs.iter().map(|r| r.text_chars).min(),
            "peakMemoryMiB": document_runs.iter().map(|r| mb(r.peak_memory_bytes)).fold(0.0, f64::max),
            "outputBytes": document_runs.iter().map(|r| r.output_bytes).max(),
            "warningsPerRun": document_runs.iter().map(|r| r.warnings).max(),
            "samples": document_times.iter().map(|v| rounded(*v as f64 / 1000.0)).collect::<Vec<f64>>(),
        },
        "singlePage": {
            "runs": single_runs,
            "p50Ms": rounded(percentile(&single_times, 0.5)),
            "p95Ms": rounded(percentile(&single_times, 0.95)),
            "maxMs": rounded(percentile(&single_times, 1.0)),
            "peakMemoryMiB": single_runs_measured.iter().map(|r| mb(r.peak_memory_bytes)).fold(0.0, f64::max),
            "samples": single_times.iter().map(|v| rounded(*v as f64 / 1000.0)).collect::<Vec<f64>>(),
        },
        "withoutOcr": {
            "elapsedMs": rounded(baseline_run.elapsed.as_secs_f64() * 1000.0),
            "pages": baseline_extraction.pages.len(),
            "textChars": baseline_chars,
            "note": "metin katmanı yok → OCR kapalıyken metin boş; OCR'ın gerçekten çalıştığının ölçülmüş kanıtı",
        },
        "honestLimits": [
            "Ölçülen katman çekirdek worker: Tauri IPC, kasa yazımı ve arayüz gösterimi yok (raporlanan süre alt sınırdır).",
            "Fikstür metni ASCII ve her sayfa aynı düzende; gerçek taramalarda eğim/gürültü/yoğunluk OCR süresini değiştirir.",
            "Süre süreç başlatmayı ve PyMuPDF importunu içerir; ilk koşu soğuk önbellekle gelir (firstRunMs ayrı yazılır).",
            "Tek makine, tek Tesseract sürümü; farklı CPU/OS ile karşılaştırılmaz.",
            "Dil modelinin kalite etkisi ölçülmedi (fikstür ASCII; Türkçe diyakritik karşılaştırması ayrı iş).",
            "OCR sayfa başına sıralı koşar; iki belge paralelliğinin OCR'a etkisi bu koşucuda ölçülmez."
        ]
    });

    let serialized = serde_json::to_string_pretty(&report).expect("rapor JSON");
    if let Ok(path) = std::env::var("DIM_BENCH_OUT") {
        if !path.trim().is_empty() {
            // Göreli yol **depo köküne** göre çözülür: `cargo test` süreci
            // çalışma alanı üyesinin klasöründe koşar, kullanıcı ise yolu depo
            // kökünden yazar (`docs/releases/F15/...`). Sessizce yazmamak yok:
            // yol açılamazsa test düşer.
            let out = PathBuf::from(path.trim());
            let out = if out.is_absolute() {
                out
            } else {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .join(out)
            };
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).expect("rapor klasörü açılamadı");
            }
            std::fs::write(&out, serialized.as_bytes()).expect("rapor yazılamadı");
            println!("\nrapor: {}", out.display());
        }
    }
    println!("\n{serialized}");

    // Rapor "boş" olamaz: süre ve bellek gerçekten ölçülmüş olmalı.
    assert!(
        report["document"]["p50Ms"].as_f64().unwrap_or(0.0) > 0.0,
        "p50 ölçülmedi"
    );
    assert!(
        report["document"]["peakMemoryMiB"].as_f64().unwrap_or(0.0) > 0.0,
        "tepe bellek ölçülmedi"
    );
}

/// `chrono` bağımlılığı yok: ISO-8601 UTC zaman damgası `SystemTime`den.
fn chrono_like_timestamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Gün sayısından tarih (sivil takvim); saat/dakika/saniye kalan.
    let days = seconds / 86_400;
    let remainder = seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        remainder / 3600,
        (remainder % 3600) / 60,
        remainder % 60
    )
}

/// Howard Hinnant'ın `civil_from_days` algoritması (1970-01-01 = 0).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod timestamp_tests {
    use super::civil_from_days;

    #[test]
    fn epoch_and_known_dates_map_to_the_civil_calendar() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_000), (2022, 1, 8));
        // 2028 artık yıl: 29 Şubat (21.243) ve sonraki gün (21.244).
        assert_eq!(civil_from_days(21_243), (2028, 2, 29));
        assert_eq!(civil_from_days(21_244), (2028, 3, 1));
    }
}
