//! CLI alt süreci için **kum havuzu** ve dosya izin kapsamı (F10-T05).
//!
//! Sınırlar (hepsi `run_cli` içinde uygulanır):
//! - İş dizini uygulama tarafından açılır; **boş** olmalı, sembolik bağ olamaz,
//!   Unix'te izni `0700` olmalı (grup/diğer erişimi yok). Kullanıcının ev dizini
//!   veya ev dizinin üstündeki bir dizin iş dizini olamaz.
//! - `HOME` ve `TMPDIR` kum havuzunun içine yönlendirilir; alt süreç varsayılan
//!   konumlarda kullanıcı yapılandırmasını görmez.
//! - stdin metni `MAX_STDIN_BYTES` ile sınırlıdır; aşan istek **süreç
//!   başlamadan** reddedilir (`E_CLI_STDIN`).
//! - Alt sürecin kum havuzunda bıraktığı dosyalar çalıştırmadan sonra **sayılır**
//!   (göreli ad + bayt; içerik okunmaz) ve dizin bırakılınca silinir.
//!
//! Dürüst sınır: bu bir **politika** kapsamıdır, çekirdek düzeyinde dosya
//! sistemi sanal alanı değildir. Alt süreç mutlak yolla kullanıcının
//! okuyabildiği dosyaları yine okuyabilir. `unsafe` yasak olduğu için
//! `setrlimit`/`umask` (fork sonrası kanca) uygulanamaz; dosya boyutu sınırı
//! yalnız **sonradan** ölçülür.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::RunError;

/// Kum havuzu dizininin Unix izni: yalnız sahibi.
pub const SANDBOX_DIR_MODE: u32 = 0o700;
/// stdin'e yazılabilecek en çok bayt (1 MiB).
pub const MAX_STDIN_BYTES: usize = 1024 * 1024;
/// Sayımda listelenecek en çok dosya.
pub const MAX_INVENTORY_ENTRIES: usize = 200;
/// Kum havuzunda bırakılabilecek en çok bayt (sonradan denetlenir).
pub const MAX_SANDBOX_BYTES: u64 = 64 * 1024 * 1024;
/// Sayımda inilecek en çok dizin derinliği.
const MAX_INVENTORY_DEPTH: usize = 4;
/// Kum havuzundaki `HOME` alt dizininin adı.
pub const HOME_DIR_NAME: &str = "home";
/// Kum havuzundaki `TMPDIR` alt dizininin adı.
pub const TMP_DIR_NAME: &str = "tmp";

static SANDBOX_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Alt sürecin `HOME`/`TMPDIR` politikası. Varsayılan **kum havuzu**;
/// kullanıcı yapılandırmasını görmesi gereken profiller bunu **açıkça**
/// `UserHome` seçer ve raporda öyle görünür.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeIsolation {
    /// `HOME`/`TMPDIR` kum havuzunun içine yönlendirilir.
    Sandbox,
    /// Çağıranın allowlist'teki `HOME`/`TMPDIR` değerleri korunur.
    UserHome,
}

impl HomeIsolation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sandbox => "sandbox",
            Self::UserHome => "user_home",
        }
    }
}

/// İş dizini denetiminin sonucu: gerçek (kanonik) yol ve Unix izni.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkdirCheck {
    pub path: PathBuf,
    pub mode: Option<u32>,
}

/// Kum havuzunda kalan tek dosya: göreli ad + bayt. İçerik okunmaz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxEntry {
    pub name: String,
    pub bytes: u64,
}

/// Alt sürecin bıraktığı dosyaların sayımı.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SandboxInventory {
    pub entries: Vec<SandboxEntry>,
    pub total_bytes: u64,
    pub truncated: bool,
}

/// Uygulamanın açtığı boş kum havuzu dizini. Bırakılınca **silinir**; alt
/// sürecin yazdığı hiçbir şey kullanıcının dosya sisteminde kalmaz.
#[derive(Debug)]
pub struct RunSandbox {
    path: PathBuf,
}

impl RunSandbox {
    pub fn create() -> Result<Self, RunError> {
        let base = std::env::temp_dir();
        for _ in 0..8 {
            let n = SANDBOX_COUNTER.fetch_add(1, Ordering::Relaxed);
            let candidate = base.join(format!("dim-cli-sandbox-{}-{n}", std::process::id()));
            match std::fs::create_dir(&candidate) {
                Ok(()) => {
                    set_dir_mode(&candidate, SANDBOX_DIR_MODE)?;
                    // macOS'ta `/var` → `/private/var` bağı; raporlanan yol gerçek
                    // konumu göstersin.
                    let path = candidate.canonicalize().unwrap_or(candidate);
                    return Ok(Self { path });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(RunError::WorkdirUnavailable(err.to_string())),
            }
        }
        Err(RunError::WorkdirUnavailable(
            "boş kum havuzu adı bulunamadı".to_string(),
        ))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `HOME` ve `TMPDIR` için kum havuzu içinde `0700` iki dizin açar.
    pub fn prepare_child_dirs(&self) -> Result<(PathBuf, PathBuf), RunError> {
        let home = self.path.join(HOME_DIR_NAME);
        let tmp = self.path.join(TMP_DIR_NAME);
        for dir in [&home, &tmp] {
            std::fs::create_dir_all(dir)
                .map_err(|err| RunError::WorkdirUnavailable(err.to_string()))?;
            set_dir_mode(dir, SANDBOX_DIR_MODE)?;
        }
        Ok((
            home.canonicalize().unwrap_or(home),
            tmp.canonicalize().unwrap_or(tmp),
        ))
    }

    /// Çalıştırmadan sonra kum havuzunda kalan dosyaları sayar.
    #[must_use]
    pub fn inventory(&self) -> SandboxInventory {
        inventory_of(&self.path)
    }
}

impl Drop for RunSandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Verilen iş dizinini kapsam kurallarına göre denetler. Hata kodları:
/// `E_CLI_EXEC_WORKDIR` (yok/bağ/ göreli), `E_CLI_WORKDIR_SCOPE` (ev dizini ya
/// da üstü), `E_CLI_WORKDIR_NOT_EMPTY` (boş değil), `E_CLI_WORKDIR_PERM`
/// (grup/diğer erişimi var).
pub fn check_workdir(path: &Path) -> Result<WorkdirCheck, RunError> {
    if !path.is_absolute() {
        return Err(RunError::InvalidWorkdir);
    }
    let meta = std::fs::symlink_metadata(path).map_err(|_| RunError::InvalidWorkdir)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(RunError::InvalidWorkdir);
    }
    let canonical = path.canonicalize().map_err(|_| RunError::InvalidWorkdir)?;
    if let Some(home) = home_dir() {
        // Ev dizini ya da ev dizinin atası: alt süreç tüm kişisel dosyaları
        // varsayılan adlarla görürdü.
        if canonical == home || home.starts_with(&canonical) {
            return Err(RunError::WorkdirNotScoped(canonical));
        }
    }
    let entries = std::fs::read_dir(&canonical)
        .map_err(|_| RunError::InvalidWorkdir)?
        .count();
    if entries != 0 {
        return Err(RunError::WorkdirNotEmpty(canonical));
    }
    let mode = dir_mode(&canonical);
    if let Some(mode) = mode {
        if mode & 0o077 != 0 {
            return Err(RunError::WorkdirPermissive(mode));
        }
    }
    Ok(WorkdirCheck {
        path: canonical,
        mode,
    })
}

/// Kum havuzundaki dosyaları (göreli ad + bayt) sayar. Simgeler izlenmez,
/// içerik okunmaz; `MAX_INVENTORY_ENTRIES` aşılırsa `truncated` işaretlenir.
#[must_use]
pub fn inventory_of(root: &Path) -> SandboxInventory {
    let mut out = SandboxInventory::default();
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
        children.sort();
        for child in children {
            if out.entries.len() >= MAX_INVENTORY_ENTRIES {
                out.truncated = true;
                return out;
            }
            let Ok(meta) = std::fs::symlink_metadata(&child) else {
                continue;
            };
            if meta.file_type().is_dir() {
                if depth + 1 < MAX_INVENTORY_DEPTH {
                    stack.push((child, depth + 1));
                }
                continue;
            }
            let name = child
                .strip_prefix(root)
                .unwrap_or(&child)
                .to_string_lossy()
                .to_string();
            out.total_bytes = out.total_bytes.saturating_add(meta.len());
            out.entries.push(SandboxEntry {
                name,
                bytes: meta.len(),
            });
        }
    }
    out.entries.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn home_dir() -> Option<PathBuf> {
    let raw = std::env::var_os("HOME")?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

#[cfg(unix)]
fn dir_mode(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .ok()
        .map(|meta| meta.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn dir_mode(_path: &Path) -> Option<u32> {
    None
}

fn set_dir_mode(path: &Path, mode: u32) -> Result<(), RunError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .map_err(|err| RunError::WorkdirUnavailable(err.to_string()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}

/// Dizini yalnız sahibinin erişebileceği izne çeker (`0700`).
pub fn set_private_mode(path: &Path) -> Result<(), RunError> {
    set_dir_mode(path, SANDBOX_DIR_MODE)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn permissive(path: &Path) {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("mode");
    }

    #[test]
    fn sandbox_is_empty_private_and_removed() {
        let sandbox = RunSandbox::create().expect("kum havuzu");
        let path = sandbox.path().to_path_buf();
        assert!(path.is_dir());
        assert_eq!(
            std::fs::read_dir(&path).expect("oku").count(),
            0,
            "boş olmalı"
        );
        assert_eq!(dir_mode(&path), Some(SANDBOX_DIR_MODE));
        let name = path.file_name().expect("ad").to_string_lossy().to_string();
        assert!(name.starts_with("dim-cli-sandbox-"), "{name}");
        drop(sandbox);
        assert!(!path.exists(), "bırakılınca silinmeli");
    }

    #[test]
    fn child_dirs_are_private_and_inside_sandbox() {
        let sandbox = RunSandbox::create().expect("kum havuzu");
        let (home, tmp) = sandbox.prepare_child_dirs().expect("dizinler");
        assert!(home.starts_with(sandbox.path()));
        assert!(tmp.starts_with(sandbox.path()));
        assert_eq!(dir_mode(&home), Some(SANDBOX_DIR_MODE));
        assert_eq!(dir_mode(&tmp), Some(SANDBOX_DIR_MODE));
    }

    #[test]
    fn permissive_workdir_is_rejected() {
        let tmp = tempfile::tempdir().expect("tmp");
        let dir = tmp.path().join("acik");
        std::fs::create_dir(&dir).expect("dizin");
        permissive(&dir);
        let err = check_workdir(&dir).expect_err("reddedilmeli");
        assert_eq!(err.code(), "E_CLI_WORKDIR_PERM");
    }

    #[test]
    fn non_empty_workdir_is_rejected() {
        let tmp = tempfile::tempdir().expect("tmp");
        std::fs::write(tmp.path().join("AGENTS.md"), "talimat").expect("dosya");
        let err = check_workdir(tmp.path()).expect_err("reddedilmeli");
        assert_eq!(err.code(), "E_CLI_WORKDIR_NOT_EMPTY");
    }

    #[test]
    fn symlinked_workdir_is_rejected() {
        let tmp = tempfile::tempdir().expect("tmp");
        let real = RunSandbox::create().expect("kum havuzu");
        let link = tmp.path().join("bag");
        std::os::unix::fs::symlink(real.path(), &link).expect("bağ");
        let err = check_workdir(&link).expect_err("bağ reddedilmeli");
        assert_eq!(err.code(), "E_CLI_EXEC_WORKDIR");
    }

    #[test]
    fn home_directory_is_rejected_as_workdir() {
        let home = home_dir().expect("HOME");
        // ev dizini boş olmasa da kural önce kapsamı denetler.
        let err = check_workdir(&home).expect_err("ev dizini reddedilmeli");
        assert_eq!(err.code(), "E_CLI_WORKDIR_SCOPE");
    }

    #[test]
    fn inventory_counts_child_files_without_content() {
        let sandbox = RunSandbox::create().expect("kum havuzu");
        let (home, _tmp) = sandbox.prepare_child_dirs().expect("dizinler");
        std::fs::write(home.join("ayar.json"), "{\"a\":1}").expect("dosya");
        std::fs::create_dir(home.join("alt")).expect("alt");
        std::fs::write(home.join("alt/not.txt"), "merhaba").expect("dosya");
        let inventory = sandbox.inventory();
        let names: Vec<&str> = inventory.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["home/alt/not.txt", "home/ayar.json"]);
        assert_eq!(inventory.total_bytes, 7 + 7);
        assert!(!inventory.truncated);
    }

    #[test]
    fn inventory_is_truncated_at_limit() {
        let sandbox = RunSandbox::create().expect("kum havuzu");
        for i in 0..(MAX_INVENTORY_ENTRIES + 5) {
            std::fs::write(sandbox.path().join(format!("f{i:04}")), b"x").expect("dosya");
        }
        let inventory = sandbox.inventory();
        assert_eq!(inventory.entries.len(), MAX_INVENTORY_ENTRIES);
        assert!(inventory.truncated);
    }
}
