//! Worker paketi manifesti ve bütünlük doğrulaması (F01-T02).
//!
//! PyInstaller `onedir` çıktısı tek dosya değil, yüzlerce dosyalık bir ağaçtır;
//! "paketlendi" demek "paket eksiksiz" demek değildir. Manifest (`scripts/worker_manifest.py`
//! üretir) her dosyanın rolünü, boyutunu ve sha256'sını taşır. Bu modül manifesti
//! **ayrıştırır** ve dizine karşı **yeniden hesaplayarak** doğrular: eksik dosya,
//! değişmiş bayt, kaymış sembolik bağ ve manifestte olmayan fazla dosya ayrı
//! raporlanır. Sessiz "tamam" yoktur.
//!
//! Güvenlik: manifestteki yollar **paket dizini dışına çıkamaz** (`..`, mutlak
//! yol, `\` reddedilir) — aksi hâlde doğrulama kullanıcının diskini taramaya
//! dönüşürdü. Sembolik bağlar izlenmez; hedef metni karşılaştırılır.
//!
//! Kapsam: bu doğrulama **bütünlük** kanıtıdır, **kimlik/imza** kanıtı değildir.
//! Manifesti üreten süreçle doğrulayan süreç aynı güven sınırındadır; kötü niyetli
//! bir kurcalama manifesti de güncelleyebilir. Arayüzde bu sınır yazılıdır.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Paket dizinindeki manifest dosyasının adı.
pub const MANIFEST_FILE: &str = "WORKER-MANIFEST.json";
/// Desteklenen manifest biçimi. Yeni alan eklemek sürümü artırmaz; anlamı
/// değiştiren değişiklik artırır.
pub const MANIFEST_VERSION: u32 = 1;
/// Manifestte kabul edilen en çok dosya sayısı.
pub const MAX_ENTRIES: usize = 20_000;
/// Dizin taramasında gezen en çok girdi (fazla dosya tespiti için).
pub const MAX_SCAN_ENTRIES: usize = 60_000;
/// Arayüze dönen en çok sorun/FAZLA kaydı. Sayıların toplamı **tam** kalır.
pub const MAX_REPORTED: usize = 50;

/// Paket dizininde depoda izlenen, paketin parçası olmayan dosyalar
/// (`.gitkeep` Tauri glob'u için, `README.md` dizin açıklaması). Manifest
/// bunları listelemez; fazla dosya sayılmazlar.
pub const NON_PACKAGE_FILES: [&str; 2] = [".gitkeep", "README.md"];

const HASH_CHUNK: usize = 256 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PackagingError {
    #[error("worker paketi bulunamadı")]
    Missing,
    #[error("manifest okunamadı: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest bozuk: {0}")]
    Malformed(String),
    #[error("desteklenmeyen manifest sürümü: {0}")]
    UnsupportedVersion(u64),
    #[error("manifestte güvenli olmayan yol: {0}")]
    UnsafePath(String),
    #[error("manifest çok fazla girdi taşıyor: {0}")]
    TooManyEntries(usize),
    #[error("manifest aynı yolu iki kez listeliyor: {0}")]
    DuplicatePath(String),
}

impl PackagingError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        "E_WORKER_PACKAGE"
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestTotals {
    #[serde(default)]
    pub files: u64,
    #[serde(default)]
    pub symlinks: u64,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub unsafe_links: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TesseractStatus {
    pub bundled: bool,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_kind() -> String {
    "file".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub path: String,
    #[serde(default)]
    pub role: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub bytes: Option<u64>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub link_target: Option<String>,
    #[serde(default)]
    pub unsafe_link: bool,
    #[serde(default)]
    pub error: Option<String>,
}

impl ManifestEntry {
    #[must_use]
    pub fn is_symlink(&self) -> bool {
        self.kind == "symlink"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerManifest {
    pub manifest_version: u32,
    #[serde(default)]
    pub worker_version: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub build_platform: String,
    #[serde(default)]
    pub python_version: String,
    #[serde(default)]
    pub pyinstaller_version: String,
    #[serde(default)]
    pub entry_point: String,
    #[serde(default)]
    pub totals: ManifestTotals,
    #[serde(default)]
    pub roles: BTreeMap<String, u64>,
    #[serde(default)]
    pub tesseract: TesseractStatus,
    #[serde(default)]
    pub files: Vec<ManifestEntry>,
}

/// Manifestteki bir yolun paket dizini içinde kaldığını doğrular.
///
/// Reddedilenler: boş yol, `\` içeren yol (Windows ayırıcısı karışıklığı),
/// denetim karakteri, mutlak yol, `..`/`.` bileşeni. Sembolik bağ hedefi de
/// aynı kurala tabidir.
fn safe_relative_path(raw: &str) -> bool {
    if raw.is_empty() || raw.contains('\\') || raw.chars().any(|c| c.is_control()) {
        return false;
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return false;
    }
    let mut saw_normal = false;
    for component in path.components() {
        match component {
            Component::Normal(_) => saw_normal = true,
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => return false,
        }
    }
    saw_normal
}

/// Manifest metnini tipli yapıya çevirir. Sürüm kapısı ve yol güvenliği
/// burada; doğrulama çağrılmadan önce manifest güvenilir olmalı.
pub fn parse_manifest(text: &str) -> Result<WorkerManifest, PackagingError> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| PackagingError::Malformed(e.to_string()))?;
    let version = value
        .get("manifestVersion")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| PackagingError::Malformed("manifestVersion yok".to_string()))?;
    if version != u64::from(MANIFEST_VERSION) {
        return Err(PackagingError::UnsupportedVersion(version));
    }
    let manifest: WorkerManifest =
        serde_json::from_value(value).map_err(|e| PackagingError::Malformed(e.to_string()))?;
    if manifest.files.len() > MAX_ENTRIES {
        return Err(PackagingError::TooManyEntries(manifest.files.len()));
    }
    if !manifest.entry_point.is_empty() && !safe_relative_path(&manifest.entry_point) {
        return Err(PackagingError::UnsafePath(manifest.entry_point.clone()));
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for entry in &manifest.files {
        if !safe_relative_path(&entry.path) {
            return Err(PackagingError::UnsafePath(entry.path.clone()));
        }
        if !seen.insert(entry.path.as_str()) {
            return Err(PackagingError::DuplicatePath(entry.path.clone()));
        }
    }
    Ok(manifest)
}

/// Diskten manifest okur.
pub fn read_manifest(dir: &Path) -> Result<WorkerManifest, PackagingError> {
    if !dir.is_dir() {
        return Err(PackagingError::Missing);
    }
    let path = dir.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PackagingError::Missing
        } else {
            PackagingError::Io(error)
        }
    })?;
    parse_manifest(&text)
}

/// Tek dosyanın sha256'sı (küçük parçalarla; dosya belleğe alınmaz).
fn file_sha256(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0u8; HASH_CHUNK];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex::encode(digest.finalize()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageProblemKind {
    /// Manifestte listelenen dosya diskte yok.
    Missing,
    /// Boyut manifesttekinden farklı (hash hesaplanmadı — ucuz kapı).
    SizeChanged,
    /// Boyut aynı, baytlar farklı.
    HashChanged,
    /// Sembolik bağın hedefi değişmiş.
    LinkTargetChanged,
    /// Manifestte boyut/hash yok: doğrulanamaz.
    Unverifiable,
    /// Diskteki girdi beklenen türde değil (bağ ↔ dosya).
    WrongKind,
    /// Okunamadı (izin/bozuk).
    Unreadable,
    /// Bağ hedefi paket dışına çıkıyor.
    UnsafeLink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageProblem {
    pub path: String,
    pub problem: PackageProblemKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageVerification {
    /// Yeniden hesaplanıp manifestle eşleşen girdi sayısı.
    pub verified: u64,
    /// **Toplam** sorun sayısı (liste `problems` kırpılmış olabilir).
    pub problem_count: u64,
    pub problems: Vec<PackageProblem>,
    /// Manifestte olmayan dosya sayısı (toplam).
    pub extra_count: u64,
    pub extra: Vec<String>,
    /// Doğrulanan dosyaların bayt toplamı.
    pub total_bytes: u64,
    /// Giriş noktası ikilisi diskte ve dosya.
    pub entry_point_present: bool,
    /// Tarama sınırına takıldı: sonuç eksik.
    pub truncated: bool,
    pub ok: bool,
}

struct Scan {
    paths: Vec<String>,
    truncated: bool,
}

/// Dizini gezer (sembolik bağları izlemez), göreli yolları toplar.
fn scan_paths(root: &Path) -> std::io::Result<Scan> {
    let mut paths = Vec::new();
    let mut truncated = false;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            if paths.len() >= MAX_SCAN_ENTRIES {
                truncated = true;
                stack.clear();
                break;
            }
            let path = entry.path();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            // Dizinler "fazla dosya" değildir; yalnız dosya ve bağlar sayılır.
            if !is_dir {
                if let Ok(relative) = path.strip_prefix(root) {
                    if let Some(text) = relative.to_str() {
                        let normalized = text.replace('\\', "/");
                        if normalized != MANIFEST_FILE
                            && !NON_PACKAGE_FILES.contains(&normalized.as_str())
                        {
                            paths.push(normalized);
                        }
                    }
                }
            }
            if is_dir {
                stack.push(path);
            }
        }
    }
    Ok(Scan { paths, truncated })
}

/// Paketi manifeste karşı doğrular: boyut → sha256 → bağ hedefi; sonra
/// manifestte olmayan dosyaları sayar. Hiçbir dosya yazılmaz.
pub fn verify_package(
    dir: &Path,
    manifest: &WorkerManifest,
) -> Result<PackageVerification, PackagingError> {
    if !dir.is_dir() {
        return Err(PackagingError::Missing);
    }
    let mut report = PackageVerification::default();
    let mut listed: BTreeSet<&str> = BTreeSet::new();
    let push_problem = |report: &mut PackageVerification, problem: PackageProblem| {
        report.problem_count += 1;
        if report.problems.len() < MAX_REPORTED {
            report.problems.push(problem);
        }
    };

    for entry in &manifest.files {
        listed.insert(entry.path.as_str());
        let full = dir.join(&entry.path);
        let meta = match std::fs::symlink_metadata(&full) {
            Ok(meta) => meta,
            Err(_) => {
                push_problem(
                    &mut report,
                    PackageProblem {
                        path: entry.path.clone(),
                        problem: PackageProblemKind::Missing,
                        detail: None,
                    },
                );
                continue;
            }
        };
        let is_symlink = meta.file_type().is_symlink();
        if entry.is_symlink() != is_symlink {
            push_problem(
                &mut report,
                PackageProblem {
                    path: entry.path.clone(),
                    problem: PackageProblemKind::WrongKind,
                    detail: Some(if is_symlink {
                        "diskte bağ, manifestte dosya".to_string()
                    } else {
                        "diskte dosya, manifestte bağ".to_string()
                    }),
                },
            );
            continue;
        }
        if is_symlink {
            if entry.unsafe_link {
                push_problem(
                    &mut report,
                    PackageProblem {
                        path: entry.path.clone(),
                        problem: PackageProblemKind::UnsafeLink,
                        detail: Some("bağ hedefi paket dışına çıkıyor".to_string()),
                    },
                );
            }
            match (std::fs::read_link(&full), entry.link_target.as_deref()) {
                (Ok(actual), Some(expected)) if actual.as_os_str() == expected => {
                    report.verified += 1;
                }
                (Ok(actual), _) => push_problem(
                    &mut report,
                    PackageProblem {
                        path: entry.path.clone(),
                        problem: PackageProblemKind::LinkTargetChanged,
                        detail: Some(actual.to_string_lossy().to_string()),
                    },
                ),
                (Err(_), _) => push_problem(
                    &mut report,
                    PackageProblem {
                        path: entry.path.clone(),
                        problem: PackageProblemKind::Unreadable,
                        detail: Some("bağ okunamadı".to_string()),
                    },
                ),
            }
            continue;
        }
        let (Some(expected_bytes), Some(expected_hash)) = (entry.bytes, entry.sha256.as_deref())
        else {
            push_problem(
                &mut report,
                PackageProblem {
                    path: entry.path.clone(),
                    problem: PackageProblemKind::Unverifiable,
                    detail: Some("manifestte boyut/hash yok".to_string()),
                },
            );
            continue;
        };
        if meta.len() != expected_bytes {
            push_problem(
                &mut report,
                PackageProblem {
                    path: entry.path.clone(),
                    problem: PackageProblemKind::SizeChanged,
                    detail: Some(format!("beklenen {expected_bytes}, diskte {}", meta.len())),
                },
            );
            continue;
        }
        match file_sha256(&full) {
            Ok(actual) if actual.eq_ignore_ascii_case(expected_hash) => {
                report.verified += 1;
                report.total_bytes += expected_bytes;
            }
            Ok(_) => push_problem(
                &mut report,
                PackageProblem {
                    path: entry.path.clone(),
                    problem: PackageProblemKind::HashChanged,
                    detail: None,
                },
            ),
            Err(error) => push_problem(
                &mut report,
                PackageProblem {
                    path: entry.path.clone(),
                    problem: PackageProblemKind::Unreadable,
                    detail: Some(error.kind().to_string()),
                },
            ),
        }
    }

    let scan = scan_paths(dir)?;
    report.truncated = scan.truncated;
    for path in scan.paths {
        if !listed.contains(path.as_str()) {
            report.extra_count += 1;
            if report.extra.len() < MAX_REPORTED {
                report.extra.push(path);
            }
        }
    }

    report.entry_point_present =
        !manifest.entry_point.is_empty() && dir.join(&manifest.entry_point).is_file();
    report.ok = report.problem_count == 0
        && report.extra_count == 0
        && report.entry_point_present
        && !report.truncated
        && manifest
            .files
            .iter()
            .all(|entry| entry.is_symlink() || (entry.bytes.is_some() && entry.sha256.is_some()));
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn write(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("dizin");
        }
        std::fs::write(path, bytes).expect("yaz");
    }

    fn sha(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn manifest_for(entries: Vec<ManifestEntry>) -> WorkerManifest {
        WorkerManifest {
            manifest_version: MANIFEST_VERSION,
            worker_version: "0.1.0".into(),
            mode: "onedir".into(),
            build_platform: "test".into(),
            python_version: "3.12.0".into(),
            pyinstaller_version: "6.11.1".into(),
            entry_point: "dim-worker".into(),
            totals: ManifestTotals::default(),
            roles: BTreeMap::new(),
            tesseract: TesseractStatus::default(),
            files: entries,
        }
    }

    fn file_entry(path: &str, bytes: &[u8]) -> ManifestEntry {
        ManifestEntry {
            path: path.into(),
            role: "other".into(),
            kind: "file".into(),
            bytes: Some(u64::try_from(bytes.len()).expect("boyut")),
            sha256: Some(sha(bytes)),
            link_target: None,
            unsafe_link: false,
            error: None,
        }
    }

    #[test]
    fn parse_rejects_unknown_version_before_reading_fields() {
        let err = parse_manifest(r#"{"manifestVersion": 2, "files": []}"#).unwrap_err();
        assert!(matches!(err, PackagingError::UnsupportedVersion(2)));
        assert_eq!(err.code(), "E_WORKER_PACKAGE");
    }

    #[test]
    fn parse_rejects_escaping_paths_and_duplicates() {
        for bad in ["../kacis", "/mutlak/yol", "a/../../b", "a\\\\b", "", "./a"] {
            let text = format!(
                r#"{{"manifestVersion":1,"entryPoint":"dim-worker","files":[{{"path":"{bad}"}}]}}"#
            );
            let err = parse_manifest(&text).unwrap_err();
            assert!(
                matches!(err, PackagingError::UnsafePath(_)),
                "{bad:?} reddedilmeliydi: {err:?}"
            );
        }
        let duplicate = r#"{"manifestVersion":1,"entryPoint":"dim-worker","files":[
            {"path":"a"} , {"path":"a"}]}"#;
        assert!(matches!(
            parse_manifest(duplicate).unwrap_err(),
            PackagingError::DuplicatePath(_)
        ));
    }

    #[test]
    fn parse_tolerates_missing_optional_fields() {
        let manifest = parse_manifest(r#"{"manifestVersion":1,"files":[{"path":"dim-worker"}]}"#)
            .expect("parse");
        assert_eq!(manifest.worker_version, "");
        assert!(!manifest.tesseract.bundled);
        assert_eq!(manifest.files[0].kind, "file");
    }

    #[test]
    fn verify_accepts_a_matching_tree() {
        let dir = tempfile::tempdir().expect("dizin");
        write(&dir.path().join("dim-worker"), b"ikili");
        write(&dir.path().join("_internal/base_library.zip"), b"arsiv");
        let manifest = manifest_for(vec![
            file_entry("dim-worker", b"ikili"),
            file_entry("_internal/base_library.zip", b"arsiv"),
        ]);
        let report = verify_package(dir.path(), &manifest).expect("doğrula");
        assert!(report.ok, "{report:?}");
        assert_eq!(report.verified, 2);
        assert_eq!(report.problem_count, 0);
        assert_eq!(report.extra_count, 0);
        assert_eq!(report.total_bytes, 10);
        assert!(report.entry_point_present);
    }

    #[test]
    fn verify_reports_missing_changed_and_extra_separately() {
        let dir = tempfile::tempdir().expect("dizin");
        write(&dir.path().join("dim-worker"), b"ikili");
        write(&dir.path().join("ayni-boyut"), b"AAAA");
        write(&dir.path().join("fazla.txt"), b"x");
        let manifest = manifest_for(vec![
            file_entry("dim-worker", b"ikili"),
            file_entry("ayni-boyut", b"BBBB"),
            file_entry("silinmis", b"yok"),
        ]);
        let report = verify_package(dir.path(), &manifest).expect("doğrula");
        assert!(!report.ok);
        assert_eq!(report.verified, 1);
        // ayni-boyut: hash değişti (boyut aynı) · silinmis: yok · fazla.txt: fazla
        assert_eq!(report.problem_count, 2);
        assert_eq!(report.extra_count, 1);
        assert!(report
            .problems
            .iter()
            .any(|p| p.path == "ayni-boyut" && p.problem == PackageProblemKind::HashChanged));
        assert!(report
            .problems
            .iter()
            .any(|p| p.path == "silinmis" && p.problem == PackageProblemKind::Missing));
        assert_eq!(report.extra, vec!["fazla.txt".to_string()]);
    }

    #[test]
    fn verify_flags_size_change_without_hashing() {
        let dir = tempfile::tempdir().expect("dizin");
        write(&dir.path().join("dim-worker"), b"uzun-icerik");
        let manifest = manifest_for(vec![file_entry("dim-worker", b"kisa")]);
        let report = verify_package(dir.path(), &manifest).expect("doğrula");
        assert!(!report.ok);
        assert_eq!(report.problem_count, 1);
        assert_eq!(report.problems[0].problem, PackageProblemKind::SizeChanged);
        // Boyut tutmadığında hash hesaplanmaz → doğrulanan bayt 0.
        assert_eq!(report.total_bytes, 0);
    }

    #[test]
    fn verify_checks_symlink_targets_and_unsafe_links() {
        let dir = tempfile::tempdir().expect("dizin");
        write(&dir.path().join("dim-worker"), b"ikili");
        write(&dir.path().join("hedef"), b"govde");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("hedef", dir.path().join("bag")).expect("bağ");
        }
        #[cfg(not(unix))]
        {
            // Windows'ta sembolik bağ oluşturmak ayrıcalık ister; test atlanır.
            return;
        }
        let mut entry = ManifestEntry {
            path: "bag".into(),
            role: "other".into(),
            kind: "symlink".into(),
            bytes: None,
            sha256: None,
            link_target: Some("hedef".into()),
            unsafe_link: true,
            error: None,
        };
        let manifest = manifest_for(vec![file_entry("dim-worker", b"ikili"), entry.clone()]);
        let report = verify_package(dir.path(), &manifest).expect("doğrula");
        // Hedef doğru ama güvensiz işaretli → sorun sayılır, "ok" olmaz.
        assert!(!report.ok);
        assert_eq!(report.problem_count, 1);
        assert_eq!(report.problems[0].problem, PackageProblemKind::UnsafeLink);

        entry.unsafe_link = false;
        entry.link_target = Some("baska-hedef".into());
        let manifest = manifest_for(vec![file_entry("dim-worker", b"ikili"), entry]);
        let report = verify_package(dir.path(), &manifest).expect("doğrula");
        assert_eq!(report.problem_count, 1);
        assert_eq!(
            report.problems[0].problem,
            PackageProblemKind::LinkTargetChanged
        );
    }

    #[test]
    fn verify_marks_entries_without_hash_unverifiable() {
        let dir = tempfile::tempdir().expect("dizin");
        write(&dir.path().join("dim-worker"), b"ikili");
        let manifest = manifest_for(vec![ManifestEntry {
            path: "dim-worker".into(),
            role: "bootloader".into(),
            kind: "file".into(),
            bytes: None,
            sha256: None,
            link_target: None,
            unsafe_link: false,
            error: None,
        }]);
        let report = verify_package(dir.path(), &manifest).expect("doğrula");
        assert_eq!(report.problem_count, 1);
        assert_eq!(report.problems[0].problem, PackageProblemKind::Unverifiable);
        assert!(!report.ok);
    }

    #[test]
    fn verify_ignores_repository_placeholders() {
        let dir = tempfile::tempdir().expect("dizin");
        write(&dir.path().join("dim-worker"), b"ikili");
        write(&dir.path().join(".gitkeep"), b"");
        write(&dir.path().join("README.md"), b"aciklama");
        write(&dir.path().join(MANIFEST_FILE), b"{}");
        let manifest = manifest_for(vec![file_entry("dim-worker", b"ikili")]);
        let report = verify_package(dir.path(), &manifest).expect("doğrula");
        assert_eq!(report.extra_count, 0, "{report:?}");
        assert!(report.ok);
    }

    #[test]
    fn missing_package_directory_is_an_error_not_an_empty_pass() {
        let err = verify_package(Path::new("/olmayan/dizin"), &manifest_for(vec![])).unwrap_err();
        assert!(matches!(err, PackagingError::Missing));
    }
}
