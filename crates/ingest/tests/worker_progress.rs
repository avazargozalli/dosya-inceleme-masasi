//! F04-T07 — canlı aşama/sayım uçtan uca, **gerçek** Python worker ile.
//!
//! Vekil süreç testleri (`worker::tests`) protokolü kanıtlar; bu dosya gerçek
//! `extract_pdf` yolunun sayfa sayısını doğru bildirdiğini ölçer. Kurgu
//! fikstür kullanılır (`fixtures/synthetic/progress-pages.pdf`, 4 sayfa).

use std::path::PathBuf;
use std::time::Instant;

use dim_ingest::worker::{dev_python_with_module, dev_worker_dir, ParseProgress, WorkerClient};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/synthetic")
        .join(name)
}

fn worker() -> Option<(PathBuf, PathBuf)> {
    let dir = dev_worker_dir()?;
    let python = dev_python_with_module(&dir, "pymupdf")?;
    Some((python, dir))
}

#[test]
fn real_worker_reports_open_and_each_page() {
    let (Some((python, dir)), Ok(data)) = (worker(), std::fs::read(fixture("progress-pages.pdf")))
    else {
        eprintln!("python+pymupdf veya fikstür yok; test atlandı");
        return;
    };
    let mut client = WorkerClient::spawn_dev(&python, &dir).expect("başlat");
    let mut seen: Vec<ParseProgress> = Vec::new();
    let started = Instant::now();
    let extraction = client
        .extract_text_with_progress(
            &data,
            "application/pdf",
            false,
            "eng",
            None,
            None,
            &mut |update| seen.push(update),
        )
        .expect("ayrıştırma");
    println!("ilerleme: {seen:?} ({:?})", started.elapsed());

    assert_eq!(extraction.pages.len(), 4, "fikstür 4 sayfa");
    let first = seen.first().expect("en az bir ilerleme çerçevesi");
    assert_eq!(first.stage, "open");
    assert_eq!(first.done, 0);
    assert_eq!(first.total, Some(4));

    let pages: Vec<(u32, Option<u32>)> = seen
        .iter()
        .filter(|update| update.stage == "pages")
        .map(|update| (update.done, update.total))
        .collect();
    assert_eq!(
        pages,
        vec![(1, Some(4)), (2, Some(4)), (3, Some(4)), (4, Some(4))],
        "her sayfa için tek sayaç, artan sırayla"
    );
    assert!(
        seen.iter()
            .all(|update| update.stage == "open" || update.stage == "pages"),
        "beklenmeyen aşama: {seen:?}"
    );
    client.shutdown();
}

/// Tek sayfalı düz metinde sahte sayaç üretilmez: arayüz "0 / 1" görmez.
#[test]
fn real_worker_reports_no_progress_for_plain_text() {
    let Some((python, dir)) = worker() else {
        eprintln!("python+pymupdf yok; test atlandı");
        return;
    };
    let mut client = WorkerClient::spawn_dev(&python, &dir).expect("başlat");
    let mut seen: Vec<ParseProgress> = Vec::new();
    let extraction = client
        .extract_text_with_progress(
            "kira sözleşmesi".as_bytes(),
            "text/plain",
            false,
            "eng",
            Some("auto"),
            None,
            &mut |update| seen.push(update),
        )
        .expect("ayrıştırma");
    assert_eq!(extraction.pages.len(), 1);
    assert!(seen.is_empty(), "düz metinde sayaç yok: {seen:?}");
    client.shutdown();
}
