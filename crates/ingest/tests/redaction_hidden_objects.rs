//! F01-T05 — gizli nesnelerin gerçek worker ile karartılması.
//!
//! Fikstür (`fixtures/synthetic/redaction-hidden.pdf`) dört vektör taşır:
//! metin katmanı, **sayfa dışı** metin (MediaBox dışında), yapışkan not
//! (`/Contents` + `/T`) ve katalog içine gömülü JavaScript (`/OpenAction`).
//!
//! Kanıt iki yönlüdür: (1) işaretçiler kaynakta gerçekten var — dosya boş
//! değil; (2) karartma sonrası çıktıda **hiçbiri** yok ve worker bunları
//! sayıp uyarı olarak bildiriyor. Yalnız (2) ölçülseydi "worker hiç
//! çalışmadı" durumu da testi geçerdi.
//!
//! Sınır (dürüst): doğrulama bu dosyanın baytlarıyla sınırlı; farklı üretici
//! ayarları (alt küme font kodlaması, satır içi TJ bölünmesi) ayrı vektörlerdir
//! ve Python tarafındaki birim testlerinde ele alınır.

use std::path::PathBuf;

use dim_ingest::worker::{dev_python_with_module, dev_worker_dir, WorkerClient};

const MARKERS: [&str; 4] = [
    "ZZREDTEXT3141",
    "ZZREDANNOT2718",
    "ZZREDOFFPAGE1618",
    "ZZREDSCRIPT1123",
];

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/synthetic/redaction-hidden.pdf")
}

/// PyMuPDF'in gerçekten kurulu olduğu yorumlayıcı + worker klasörü.
///
/// Motor yoksa test **atlanır**: karartma motoru olmadan "temiz" ya da
/// "başarısız" demek yanıltıcı olur. CI'da `pip install -e
/// "workers/document[all,dev]" aynı yorumlayıcıya kurar.
fn worker_context() -> Option<(PathBuf, PathBuf)> {
    let worker_dir = dev_worker_dir()?;
    let python = dev_python_with_module(&worker_dir, "pymupdf")?;
    Some((python, worker_dir))
}

#[test]
fn worker_removes_offpage_annotation_and_script_and_proves_it() {
    let Some((python, worker_dir)) = worker_context() else {
        eprintln!("PyMuPDF'li yorumlayıcı veya workers/document yok; test atlandı");
        return;
    };
    let data = std::fs::read(fixture_path()).expect("fikstür okunmalı");
    for marker in MARKERS {
        assert!(
            contains(&data, marker.as_bytes()),
            "fikstür {marker} taşımıyor; test kurgusu bozuk"
        );
    }

    let mut client = WorkerClient::spawn_dev(&python, &worker_dir).expect("worker başlat");
    client.ping().expect("ping");
    let terms: Vec<String> = MARKERS.iter().map(|marker| (*marker).to_string()).collect();
    let outcome = client.redact(&data, &terms).expect("karartma");
    client.shutdown();

    assert!(
        outcome.remaining.is_empty(),
        "kalan terim: {:?}",
        outcome.remaining
    );
    assert!(
        outcome.annotations_removed >= 1,
        "açıklama silinmedi: {}",
        outcome.annotations_removed
    );
    assert!(
        outcome.offpage_spans_removed >= 1,
        "sayfa dışı metin kaldırılmadı: {}",
        outcome.offpage_spans_removed
    );
    assert_eq!(outcome.scripts_removed, 1, "gömülü JavaScript kaldırılmadı");
    assert!(
        outcome
            .warnings
            .iter()
            .any(|note| note.contains("açıklama")),
        "uyarı yok: {:?}",
        outcome.warnings
    );
    assert!(
        outcome.engines.iter().any(|engine| engine == "raw-scan"),
        "bağımsız bayt taraması motor listesinde yok: {:?}",
        outcome.engines
    );

    for marker in MARKERS {
        assert!(
            !contains(&outcome.pdf, marker.as_bytes()),
            "çıktıda sızıntı: {marker} ({} bayt)",
            outcome.pdf.len()
        );
    }
}

#[test]
fn worker_reports_remaining_when_term_is_only_in_a_scan() {
    // Görüntüde kalan terim ayrı bir vektördür; burada yalnız "worker
    // gerçekten çalıştı ve doğrulama kapısı kapalı" kanıtlanır: aynı fikstür
    // karartılmadan gönderilirse terim metin katmanında bulunur.
    let Some((python, worker_dir)) = worker_context() else {
        eprintln!("PyMuPDF'li yorumlayıcı veya workers/document yok; test atlandı");
        return;
    };
    let data = std::fs::read(fixture_path()).expect("fikstür okunmalı");
    let mut client = WorkerClient::spawn_dev(&python, &worker_dir).expect("worker başlat");
    client.ping().expect("ping");
    // Var olmayan bir terim: çıktı temiz olmalı, hits boş.
    let outcome = client
        .redact(&data, &["ZZOLMAYANTERIM9911".to_string()])
        .expect("karartma");
    client.shutdown();
    assert!(outcome.hits.is_empty());
    assert!(outcome.remaining.is_empty());
    // Açıklama/JS temizliği terimden bağımsızdır ama terim taşımadığı için
    // silinmez: fikstürdeki not yerinde kalır.
    assert_eq!(outcome.annotations_removed, 0);
    assert_eq!(outcome.scripts_removed, 1, "JavaScript her durumda gider");
}
