//! F01-T03 (worker ayağı) — belge baytları worker'a **dosya yolu olarak değil**
//! stdin'den gider (ADR-03). Bu test, çıkarım sırasında worker'ın süreç geçici
//! dizininde (`TMPDIR`) ve çalışma dizininde belge metninin düz olarak
//! kalmadığını işaretçiyle ölçer.
//!
//! Kanıt iki yönlüdür: (1) çıkarım işaretçiyi geri döndürür — yani metin
//! gerçekten işlendi; (2) aynı işaretçi yeni doğan hiçbir dosyada bulunmaz.
//! Yalnız (2) ölçülseydi "worker hiç çalışmadı" durumu da testi geçerdi.
//!
//! Sınırlar (dürüst): ölçüm **gözlemlenen** dosya izidir; işletim sisteminin
//! sayfa takasını (swap) veya Python'un kendi bellek davranışını kapsamaz.
//! Test, Python + `workers/document` varsa koşar; yoksa atlandığını yazar.

use std::path::{Path, PathBuf};

use dim_ingest::worker::{dev_python, dev_worker_dir, WorkerClient};

const MARKER: &str = "ZZ-WORKER-MARKER-4B2E11";

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// Dizin ağacındaki dosya yollarını toplar (sembolik bağ izlenmez).
fn walk_files(root: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => walk_files(&path, out, depth + 1),
            Ok(kind) if kind.is_file() => out.push(path),
            _ => {}
        }
    }
}

/// Geçici dizinin **üst düzey** girdileri. 1.500'den fazla eski girdi olabilir;
/// her koşuda tüm ağacı taramak yerine yalnız yeni doğanlara inilir.
fn top_level(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries.flatten().map(|entry| entry.path()).collect()
}

fn files_containing(paths: impl IntoIterator<Item = PathBuf>, needle: &[u8]) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    for path in paths {
        if path.is_dir() {
            let mut nested = Vec::new();
            walk_files(&path, &mut nested, 0);
            for file in nested {
                if let Ok(bytes) = std::fs::read(&file) {
                    if contains(&bytes, needle) {
                        hits.push(file);
                    }
                }
            }
        } else if let Ok(bytes) = std::fs::read(&path) {
            if contains(&bytes, needle) {
                hits.push(path);
            }
        }
    }
    hits
}

#[test]
fn planted_marker_is_detected_by_the_scan() {
    // Dedektör kanıtı: worker testindeki "yeni dosya yok" sonucu körlük değil.
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("temiz.txt"), b"ilgisiz icerik").expect("temiz");
    let nested = dir.path().join("alt");
    std::fs::create_dir(&nested).expect("alt dizin");
    std::fs::write(nested.join("sizinti.txt"), format!("{MARKER} metin")).expect("sizinti");

    let found = files_containing(vec![dir.path().to_path_buf()], MARKER.as_bytes());
    assert_eq!(found.len(), 1, "tam olarak bir sızıntı bulunmalı");
    assert!(found[0].ends_with("sizinti.txt"));
}

#[test]
fn worker_leaves_no_plaintext_in_temp_or_working_dir() {
    let (Some(py), Some(dir)) = (dev_python(), dev_worker_dir()) else {
        eprintln!("python veya workers/document bulunamadı; test atlandı");
        return;
    };

    let temp_dir = std::env::temp_dir();
    let before_temp = top_level(&temp_dir);
    let mut before_work = Vec::new();
    walk_files(&dir, &mut before_work, 0);

    let mut client = WorkerClient::spawn_dev(&py, &dir).expect("worker başlat");
    client.ping().expect("ping");

    let document = format!("{MARKER} kira bedeli ödenmedi, ihtar tebliğ edildi.").into_bytes();
    let extraction = client
        .extract_text(&document, "text/plain", false, "eng", Some("utf-8"), None)
        .expect("çıkarım");

    // (1) Metin gerçekten işlendi: işaretçi çıkarımda geri geliyor.
    assert_eq!(extraction.engine, "plain");
    assert_eq!(extraction.pages.len(), 1);
    assert_eq!(
        extraction.pages[0].text,
        String::from_utf8(document.clone()).unwrap()
    );
    assert!(extraction.pages[0].text.contains(MARKER));

    client.shutdown();

    // (2) Çıkarım sırasında doğan hiçbir dosyada işaretçi yok.
    let after_temp = top_level(&temp_dir);
    let created: Vec<PathBuf> = after_temp
        .into_iter()
        .filter(|path| !before_temp.contains(path))
        .collect();
    let leaked = files_containing(created.clone(), MARKER.as_bytes());
    assert!(
        leaked.is_empty(),
        "worker geçici dizininde düz metin sızıntısı: {leaked:?} (yeni girdiler: {created:?})"
    );

    // Çalışma dizininde yeni dosya doğabilir (ör. `__pycache__` bayt kodu) ama
    // belge metni taşıyamaz.
    let mut after_work = Vec::new();
    walk_files(&dir, &mut after_work, 0);
    let new_work: Vec<PathBuf> = after_work
        .into_iter()
        .filter(|path| !before_work.contains(path))
        .collect();
    let leaked_work = files_containing(new_work.clone(), MARKER.as_bytes());
    assert!(
        leaked_work.is_empty(),
        "worker çalışma dizininde düz metin sızıntısı: {leaked_work:?}"
    );
}
