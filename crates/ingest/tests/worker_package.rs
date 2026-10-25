//! F01-T02 — paketlenmiş worker'ın **gerçek** paket üzerinde doğrulanması.
//!
//! Paket depoda izlenmez (`apps/desktop/src-tauri/resources/worker/` gitignore).
//! Bu yüzden test ortam değişkeniyle kapılıdır:
//!
//! ```sh
//! DIM_WORKER_PACKAGE_DIR=$PWD/apps/desktop/src-tauri/resources/worker \
//!   cargo test -p dim-ingest --test worker_package -- --nocapture
//! ```
//!
//! Değişken yoksa test **atlandığını yazar** (sessiz yeşil yok). Paket varken:
//! manifest ayrıştırılır, her dosyanın boyutu + sha256'sı yeniden hesaplanır,
//! giriş noktası çalıştırılabilir mi bakılır ve ikili gerçekten NDJSON ile
//! konuşturulur (Python kurulu olmadan, paket dizininden).

use std::path::{Path, PathBuf};

use dim_ingest::packaging::{read_manifest, verify_package};
use dim_ingest::worker::WorkerClient;

fn package_dir() -> Option<PathBuf> {
    let raw = std::env::var_os("DIM_WORKER_PACKAGE_DIR")?;
    let dir = PathBuf::from(raw);
    if dir.join(dim_ingest::packaging::MANIFEST_FILE).is_file() {
        Some(dir)
    } else {
        println!(
            "ATLANDI: DIM_WORKER_PACKAGE_DIR var ama manifest yok ({})",
            dir.display()
        );
        None
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[test]
fn packaged_worker_matches_its_manifest() {
    let Some(dir) = package_dir() else {
        println!("ATLANDI: paketlenmiş worker yok (DIM_WORKER_PACKAGE_DIR tanımsız)");
        return;
    };

    let manifest = read_manifest(&dir).expect("manifest okunmalı");
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.mode, "onedir");
    assert!(!manifest.files.is_empty(), "manifest boş olamaz");

    let report = verify_package(&dir, &manifest).expect("doğrulama koşmalı");
    assert!(
        report.problems.is_empty(),
        "paket manifestle uyuşmuyor: {:?}",
        report.problems
    );
    assert_eq!(report.extra_count, 0, "fazla dosya: {:?}", report.extra);
    assert!(report.entry_point_present, "giriş noktası yok");
    assert!(report.ok, "doğrulama ok değil: {report:?}");

    // Manifestin bildirdiği toplamlar gerçek taramayla tutarlı olmalı; yoksa
    // "118 dosya" iddiası süs olur. `verified` dosya **ve** bağları sayar.
    assert_eq!(
        manifest.totals.files + manifest.totals.symlinks,
        report.verified,
        "manifest girdi sayısı doğrulananla eşleşmiyor"
    );
    assert_eq!(manifest.totals.bytes, report.total_bytes);

    let exe = dir.join(&manifest.entry_point);
    assert!(is_executable(&exe), "giriş noktası çalıştırılabilir değil");
    println!(
        "paket: {} dosya + {} bağ · {} bayt · platform {} · python {} · pyinstaller {}",
        manifest.totals.files,
        manifest.totals.symlinks,
        manifest.totals.bytes,
        manifest.build_platform,
        manifest.python_version,
        manifest.pyinstaller_version
    );
}

#[test]
fn packaged_worker_answers_without_python_on_path() {
    let Some(dir) = package_dir() else {
        println!("ATLANDI: paketlenmiş worker yok (DIM_WORKER_PACKAGE_DIR tanımsız)");
        return;
    };
    let manifest = read_manifest(&dir).expect("manifest");
    let exe = dir.join(&manifest.entry_point);

    let started = std::time::Instant::now();
    let mut worker = WorkerClient::spawn_bundled(&exe).expect("paket ikilisi başlamalı");
    worker.ping().expect("ping yanıtı");
    // `ping` sonuç taşımaz; sürüm `capabilities`/`ping` gövdesinden okunur.
    let pong = worker
        .request("ping", serde_json::json!({}))
        .expect("ping gövdesi");
    assert_eq!(pong["pong"], serde_json::Value::Bool(true));
    assert_eq!(pong["workerVersion"], manifest.worker_version.as_str());

    let caps = worker.capabilities().expect("capabilities yanıtı");
    // PDF motoru paketlenmiş olmalı; yoksa onedir paket eksiktir.
    assert_eq!(
        caps["engines"]["pdf"]["available"],
        serde_json::Value::Bool(true),
        "pakette pdf motoru yok: {caps}"
    );
    let version = caps["engines"]["pdf"]["version"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(!version.is_empty(), "pdf motoru sürüm bildirmiyor");
    let elapsed = started.elapsed();
    println!(
        "paketle çalıştı: pdf {version} · docx {} · udf {} · {elapsed:?}",
        caps["engines"]["docx"]["version"], caps["engines"]["udf"]["version"]
    );
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "paket ikilisi {elapsed:?} içinde yanıt vermedi"
    );
}
