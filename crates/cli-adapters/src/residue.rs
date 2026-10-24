//! CLI **kalıntı** envanteri, ölçümü ve yalnız uygulamaya ait kayıtların
//! temizliği (F10-T13).
//!
//! Şartname (18.4): transcript, debug log ve kalıcı oturumlar şifreli kasa
//! kapsamı dışında olabilir; her desteklenen sürümde bu yollar **envantere
//! alınır**, oturum kalıcılığını kapatma ve uygulamaya ait kalıntıları temizleme
//! sınanır. **Genel HOME/temp temizliği yapılmaz.**
//!
//! Bu modülün kuralları:
//! - Yalnız **bildirilen** yollar taranır (adaptör sicilindeki sabit liste).
//!   Kullanıcı ev dizini genel olarak taranmaz.
//! - Tarama **yalnız üst veri** okur (ad, bayt, değişim zamanı). Dosya
//!   **içeriği açılmaz**; sembolik bağ izlenmez.
//! - Temizlik yalnız **bu uygulamanın koşusu sırasında oluştuğu ölçümle
//!   kanıtlanmış** dosyaları siler. Önceden var olan dosya kullanıcınındır:
//!   asla silinmez, sayılır ve raporlanır.
//! - Uygulamanın açtığı kum havuzları (`dim-cli-sandbox-*`) çökme sonrası
//!   kalabilir; bunlar `0700`, sahibi biz ve yaş eşiğini geçmişse süpürülür.
//!
//! Dürüst sınır: bu bir **ölçüm ve politika** aracıdır, çekirdek düzeyinde
//! DLP değildir. CLI'nin bildirmediğimiz bir yola yazması bu envanterle
//! yakalanmaz; bu yüzden varsayılan yürütmede `HOME` kum havuzuna yönlendirilir
//! (F10-T05) ve kalıntı ölçümü **açık seçimle** gerçek ev dizininde yapılır.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::probe::PROBE_TIMEOUT;
use crate::sandbox::{HomeIsolation, RunSandbox};
use crate::{default_env_allowlist, run_cli, AdapterSpec, CliRunRequest, RunError, RunStatus};

/// Tarama başına en çok dosya (bellek/CPU sınırı).
pub const MAX_RESIDUE_FILES: usize = 4000;
/// Taramada inilecek en çok dizin derinliği.
pub const MAX_RESIDUE_DEPTH: usize = 8;
/// Uygulamanın kendi kum havuzu dizinlerinin öneki.
pub const SANDBOX_PREFIX: &str = "dim-cli-sandbox-";
/// Çökme sonrası kalıntı süpürmesinde en az yaş (süren bir koşuya dokunulmaz).
pub const STALE_SANDBOX_AGE: Duration = Duration::from_secs(3600);

/// Kalıntı türü. Arayüzde ve denetimde bu etiket kullanılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidueKind {
    Transcript,
    Log,
    Session,
    History,
    Cache,
}

impl ResidueKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Transcript => "transcript",
            Self::Log => "log",
            Self::Session => "session",
            Self::History => "history",
            Self::Cache => "cache",
        }
    }
}

/// Adaptör sicilinde bildirilen tek kalıntı yolu (ev dizinine **göreli**).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResiduePathSpec {
    pub relative: &'static str,
    pub kind: ResidueKind,
    /// Ne beklendiği ve dayanağı (tahmin değil, kaynak notu).
    pub note: &'static str,
}

/// Oturum kalıcılığını kapatma denetimi. `flag` **yalnız** `--help` çıktısında
/// doğrulandıysa doludur; doğrulanmamış bayrak yürütmede kullanılmaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistenceControl {
    pub flag: Option<&'static str>,
    /// Bayrağın geçerli olduğu alt komut/kip (ör. `--print`, `exec`).
    pub applies_to: &'static str,
    /// Hangi sürümde `--help` ile doğrulandı.
    pub verified_with: Option<&'static str>,
}

impl PersistenceControl {
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            flag: None,
            applies_to: "",
            verified_with: None,
        }
    }

    /// Kalıcılığı kapatma yolu **doğrulanmış** mı?
    #[must_use]
    pub const fn usable(&self) -> bool {
        self.flag.is_some() && self.verified_with.is_some()
    }
}

/// Claude Code kalıntı yolları. `note` alanı sürümle sınanmadığında bunu söyler.
const CLAUDE_PATHS: &[ResiduePathSpec] = &[
    ResiduePathSpec {
        relative: ".claude/projects",
        kind: ResidueKind::Transcript,
        note: "proje başına oturum transkriptleri (JSONL)",
    },
    ResiduePathSpec {
        relative: ".claude/history.jsonl",
        kind: ResidueKind::History,
        note: "komut geçmişi",
    },
    ResiduePathSpec {
        relative: ".claude/todos",
        kind: ResidueKind::Session,
        note: "oturum görev listeleri",
    },
    ResiduePathSpec {
        relative: ".claude/shell-snapshots",
        kind: ResidueKind::Cache,
        note: "kabuk anlık görüntüleri",
    },
    ResiduePathSpec {
        relative: ".claude/statsig",
        kind: ResidueKind::Cache,
        note: "yerel istatistik/telemetri tohumları",
    },
    ResiduePathSpec {
        relative: ".claude/debug",
        kind: ResidueKind::Log,
        note: "hata ayıklama kayıtları",
    },
];

/// Codex CLI kalıntı yolları.
const CODEX_PATHS: &[ResiduePathSpec] = &[
    ResiduePathSpec {
        relative: ".codex/sessions",
        kind: ResidueKind::Transcript,
        note: "rollout oturum dosyaları",
    },
    ResiduePathSpec {
        relative: ".codex/log",
        kind: ResidueKind::Log,
        note: "TUI/günlük kayıtları",
    },
    ResiduePathSpec {
        relative: ".codex/history.jsonl",
        kind: ResidueKind::History,
        note: "komut geçmişi",
    },
    ResiduePathSpec {
        relative: ".codex/cache",
        kind: ResidueKind::Cache,
        note: "yerel önbellek",
    },
];

/// Antigravity CLI kalıntı yolu (sürüm doğrulanmadı; yol yalnız envanterde).
const ANTIGRAVITY_PATHS: &[ResiduePathSpec] = &[ResiduePathSpec {
    relative: ".antigravity",
    kind: ResidueKind::Session,
    note: "oturum/uygulama verisi (sürüm bazlı doğrulanmadı)",
}];

/// Kimi CLI kalıntı yolu (etkileşimli-only; otomasyon kapalı).
const KIMI_PATHS: &[ResiduePathSpec] = &[ResiduePathSpec {
    relative: ".kimi",
    kind: ResidueKind::Session,
    note: "ACP oturum kayıtları (sürüm bazlı doğrulanmadı)",
}];

/// Adaptörün bildirilen kalıntı yolları. Bilinmeyen adaptörde boş liste döner:
/// sicilde olmayan adaptör için tarama yapılmaz.
#[must_use]
pub fn residue_paths(adapter_id: &str) -> &'static [ResiduePathSpec] {
    match adapter_id {
        "claude-code" => CLAUDE_PATHS,
        "codex-cli" => CODEX_PATHS,
        "antigravity-cli" => ANTIGRAVITY_PATHS,
        "kimi-cli" => KIMI_PATHS,
        _ => &[],
    }
}

/// Adaptörün oturum kalıcılığı denetimi.
///
/// Bayraklar bu makinede `--help` çıktısından **doğrulandı**: Claude Code
/// `2.1.303` → `--no-session-persistence` (yalnız `--print` ile), Codex CLI
/// `0.159.2` → `exec --ephemeral`. Kimi ve Antigravity için doğrulanmış
/// kapatma yolu **yok**; kalıcılık kapatılamıyorsa uygulama ev dizinini kum
/// havuzuna yönlendirir (F10-T05).
#[must_use]
pub fn persistence_control(adapter_id: &str) -> PersistenceControl {
    match adapter_id {
        "claude-code" => PersistenceControl {
            flag: Some("--no-session-persistence"),
            applies_to: "--print",
            verified_with: Some("2.1.303"),
        },
        "codex-cli" => PersistenceControl {
            flag: Some("--ephemeral"),
            applies_to: "exec",
            verified_with: Some("0.159.2"),
        },
        _ => PersistenceControl::unavailable(),
    }
}

/// Bildirilen tek yolun tarama sonucu. İçerik taşımaz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidueEntry {
    pub relative: String,
    pub kind: &'static str,
    pub note: &'static str,
    pub exists: bool,
    pub files: usize,
    pub bytes: u64,
    pub truncated: bool,
    /// En yeni dosyanın yaşı (saniye); hiç dosya yoksa `None`.
    pub newest_age_secs: Option<u64>,
}

/// Bir adaptörün kalıntı taraması: yalnız bildirilen yollar, yalnız üst veri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidueScan {
    pub adapter: String,
    pub root: PathBuf,
    pub entries: Vec<ResidueEntry>,
    pub total_files: usize,
    pub total_bytes: u64,
    /// Tarama dosya sınırına takıldı mı? (Sonuç eksik sayılır.)
    pub truncated: bool,
}

impl ResidueScan {
    /// Hiçbir bildirilen yolda dosya yok mu?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total_files == 0
    }
}

/// Tek dosyanın üst veri damgası (içerik yok).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidueFileStamp {
    pub relative: String,
    pub bytes: u64,
    pub modified_secs: u64,
}

/// Karşılaştırma için anlık görüntü: dosyalar + dizinler.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResidueSnapshot {
    pub adapter: String,
    pub root: PathBuf,
    pub files: BTreeMap<String, ResidueFileStamp>,
    pub dirs: BTreeSet<String>,
    pub truncated: bool,
}

/// Önce/sonra farkı: hangi dosyalar **bu koşuda** oluştu, hangileri değişti.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResidueDelta {
    pub created: Vec<ResidueFileStamp>,
    pub modified: Vec<ResidueFileStamp>,
    pub removed: Vec<String>,
    pub created_dirs: Vec<String>,
    pub bytes_added: u64,
}

impl ResidueDelta {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.modified.is_empty() && self.created_dirs.is_empty()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ResidueError {
    #[error("yol bildirilen kalıntı kapsamı dışında: {0}")]
    OutOfScope(String),
    #[error("yol sembolik bağ veya normal dosya değil: {0}")]
    NotARegularFile(String),
    #[error("kalıntı okunamadı: {0}")]
    Io(String),
}

impl ResidueError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::OutOfScope(_) => "E_CLI_RESIDUE_SCOPE",
            Self::NotARegularFile(_) => "E_CLI_RESIDUE_SYMLINK",
            Self::Io(_) => "E_CLI_RESIDUE_IO",
        }
    }
}

/// `relative` bildirilen yollardan birinin kendisi mi, altında mı?
fn is_declared(adapter_id: &str, relative: &str) -> bool {
    residue_paths(adapter_id).iter().any(|spec| {
        relative == spec.relative
            || relative
                .strip_prefix(spec.relative)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// Göreli yolu güvenli biçimde birleştirir: boş, mutlak, `..` ve `.` içeren
/// yollar reddedilir. Bu kapı olmadan `join` ile kapsam dışına çıkılabilirdi.
fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, ResidueError> {
    if relative.is_empty() || relative.starts_with('/') {
        return Err(ResidueError::OutOfScope(relative.to_string()));
    }
    let mut out = root.to_path_buf();
    for part in relative.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(ResidueError::OutOfScope(relative.to_string()));
        }
        out.push(part);
    }
    Ok(out)
}

/// Dizini sınırlı derinlikte tarar; **içerik açmaz**, sembolik bağ izlemez.
struct WalkState<'a> {
    base: &'a Path,
    budget: usize,
    files: usize,
    bytes: u64,
    newest_age: Option<u64>,
    dirs: BTreeSet<String>,
    now_secs: u64,
    truncated: bool,
}

impl WalkState<'_> {
    fn walk(&mut self, dir: &Path, depth: usize) {
        if depth > MAX_RESIDUE_DEPTH {
            self.truncated = true;
            return;
        }
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            if self.budget == 0 {
                self.truncated = true;
                return;
            }
            let path = entry.path();
            let Ok(link_meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if link_meta.file_type().is_symlink() {
                continue;
            }
            if link_meta.is_dir() {
                if let Ok(rel) = path.strip_prefix(self.base) {
                    self.dirs.insert(rel.to_string_lossy().replace('\\', "/"));
                }
                self.walk(&path, depth + 1);
            } else if link_meta.is_file() {
                self.files += 1;
                self.bytes = self.bytes.saturating_add(link_meta.len());
                self.budget -= 1;
                let age = link_meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map(|d| self.now_secs.saturating_sub(d.as_secs()));
                if let Some(age) = age {
                    self.newest_age = Some(self.newest_age.map_or(age, |cur| cur.min(age)));
                }
            }
        }
    }
}

fn now_secs(now: SystemTime) -> u64 {
    now.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Bildirilen yolları tarar. Yalnız üst veri; dosya içeriği **açılmaz**.
#[must_use]
pub fn scan_residue(root: &Path, adapter_id: &str, now: SystemTime) -> ResidueScan {
    let now_s = now_secs(now);
    let mut budget = MAX_RESIDUE_FILES;
    let mut total_files = 0usize;
    let mut total_bytes = 0u64;
    let mut any_truncated = false;
    let mut entries = Vec::new();
    for spec in residue_paths(adapter_id) {
        let path = root.join(spec.relative);
        let exists = std::fs::symlink_metadata(&path).is_ok_and(|m| !m.file_type().is_symlink());
        let mut state = WalkState {
            base: root,
            budget,
            files: 0,
            bytes: 0,
            newest_age: None,
            dirs: BTreeSet::new(),
            now_secs: now_s,
            truncated: false,
        };
        if exists {
            let Ok(meta) = std::fs::metadata(&path) else {
                entries.push(ResidueEntry {
                    relative: spec.relative.to_string(),
                    kind: spec.kind.as_str(),
                    note: spec.note,
                    exists: true,
                    files: 0,
                    bytes: 0,
                    truncated: false,
                    newest_age_secs: None,
                });
                continue;
            };
            if meta.is_dir() {
                state.walk(&path, 0);
            } else if meta.is_file() {
                state.files = 1;
                state.bytes = meta.len();
                state.budget = state.budget.saturating_sub(1);
                state.newest_age = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map(|d| now_s.saturating_sub(d.as_secs()));
            }
        }
        budget = state.budget;
        any_truncated |= state.truncated;
        total_files += state.files;
        total_bytes = total_bytes.saturating_add(state.bytes);
        entries.push(ResidueEntry {
            relative: spec.relative.to_string(),
            kind: spec.kind.as_str(),
            note: spec.note,
            exists,
            files: state.files,
            bytes: state.bytes,
            truncated: state.truncated,
            newest_age_secs: state.newest_age,
        });
    }
    ResidueScan {
        adapter: adapter_id.to_string(),
        root: root.to_path_buf(),
        entries,
        total_files,
        total_bytes,
        truncated: any_truncated,
    }
}

/// Ölçüm için anlık görüntü. `scan` ile aynı kapsam, ama dosya kimliği taşır.
#[must_use]
pub fn snapshot_residue(root: &Path, adapter_id: &str) -> ResidueSnapshot {
    let mut snapshot = ResidueSnapshot {
        adapter: adapter_id.to_string(),
        root: root.to_path_buf(),
        ..ResidueSnapshot::default()
    };
    let mut budget = MAX_RESIDUE_FILES;
    for spec in residue_paths(adapter_id) {
        let base = root.join(spec.relative);
        let Ok(meta) = std::fs::symlink_metadata(&base) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_file() {
            if let Some(stamp) = file_stamp(root, &base) {
                snapshot.files.insert(stamp.relative.clone(), stamp);
                budget = budget.saturating_sub(1);
            }
            continue;
        }
        if !meta.is_dir() {
            continue;
        }
        snapshot.dirs.insert(spec.relative.to_string());
        collect_snapshot(root, &base, 0, &mut budget, &mut snapshot);
    }
    snapshot
}

fn collect_snapshot(
    root: &Path,
    dir: &Path,
    depth: usize,
    budget: &mut usize,
    snapshot: &mut ResidueSnapshot,
) {
    if depth > MAX_RESIDUE_DEPTH {
        snapshot.truncated = true;
        return;
    }
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        if *budget == 0 {
            snapshot.truncated = true;
            return;
        }
        let path = entry.path();
        let Ok(link_meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if link_meta.file_type().is_symlink() {
            continue;
        }
        if link_meta.is_dir() {
            if let Ok(rel) = path.strip_prefix(root) {
                snapshot
                    .dirs
                    .insert(rel.to_string_lossy().replace('\\', "/"));
            }
            collect_snapshot(root, &path, depth + 1, budget, snapshot);
        } else if link_meta.is_file() {
            if let Some(stamp) = file_stamp(root, &path) {
                snapshot.files.insert(stamp.relative.clone(), stamp);
                *budget -= 1;
            }
        }
    }
}

fn file_stamp(root: &Path, path: &Path) -> Option<ResidueFileStamp> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let relative = path
        .strip_prefix(root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    let modified_secs = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some(ResidueFileStamp {
        relative,
        bytes: meta.len(),
        modified_secs,
    })
}

/// İki anlık görüntüyü karşılaştırır. `created` = yalnız sonra var olan;
/// `modified` = boyut ya da değişim zamanı değişen.
#[must_use]
pub fn diff_snapshots(before: &ResidueSnapshot, after: &ResidueSnapshot) -> ResidueDelta {
    let mut delta = ResidueDelta::default();
    for (relative, stamp) in &after.files {
        match before.files.get(relative) {
            None => {
                delta.created.push(stamp.clone());
                delta.bytes_added = delta.bytes_added.saturating_add(stamp.bytes);
            }
            Some(old) if old.bytes != stamp.bytes || old.modified_secs != stamp.modified_secs => {
                delta.modified.push(stamp.clone());
            }
            Some(_) => {}
        }
    }
    for relative in before.files.keys() {
        if !after.files.contains_key(relative) {
            delta.removed.push(relative.clone());
        }
    }
    for dir in &after.dirs {
        if !before.dirs.contains(dir) {
            delta.created_dirs.push(dir.clone());
        }
    }
    delta.created.sort_by(|a, b| a.relative.cmp(&b.relative));
    delta.modified.sort_by(|a, b| a.relative.cmp(&b.relative));
    delta.removed.sort();
    // Derin önce silinsin diye en uzun yol başta.
    delta.created_dirs.sort_by(|a, b| {
        b.matches('/')
            .count()
            .cmp(&a.matches('/').count())
            .then(a.cmp(b))
    });
    delta
}

/// Temizlik sonucu: yalnız sayılar ve gerekçeler; dosya **içeriği** yok.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CleanupReport {
    pub removed_files: usize,
    pub removed_bytes: u64,
    pub removed_dirs: usize,
    /// `(yol, gerekçe)` — silinmeyenler.
    pub skipped: Vec<(String, String)>,
    /// `(yol, hata kodu)` — silinemeyenler.
    pub errors: Vec<(String, String)>,
}

/// **Yalnız bu ölçümde oluştuğu kanıtlanmış** dosyaları siler.
///
/// Kapılar: yol bildirilen kapsamda olmalı, `..`/mutlak yol reddedilir, hedef
/// normal dosya olmalı (sembolik bağ reddedilir). Önceden var olan dosya bu
/// listeye hiç girmez; yine de savunma olarak `created` dışındaki yol silinmez.
pub fn cleanup_created(root: &Path, adapter_id: &str, delta: &ResidueDelta) -> CleanupReport {
    let mut report = CleanupReport::default();
    for stamp in &delta.created {
        let relative = stamp.relative.as_str();
        if !is_declared(adapter_id, relative) {
            report
                .skipped
                .push((relative.to_string(), "kapsam dışı".to_string()));
            continue;
        }
        let path = match safe_join(root, relative) {
            Ok(path) => path,
            Err(_) => {
                report
                    .skipped
                    .push((relative.to_string(), "güvenli olmayan yol".to_string()));
                continue;
            }
        };
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                report
                    .skipped
                    .push((relative.to_string(), "sembolik bağ".to_string()));
            }
            Ok(meta) if !meta.is_file() => {
                report
                    .skipped
                    .push((relative.to_string(), "normal dosya değil".to_string()));
            }
            Ok(meta) => {
                let bytes = meta.len();
                match std::fs::remove_file(&path) {
                    Ok(()) => {
                        report.removed_files += 1;
                        report.removed_bytes = report.removed_bytes.saturating_add(bytes);
                    }
                    Err(err) => report
                        .errors
                        .push((relative.to_string(), format!("E_CLI_RESIDUE_IO: {err}"))),
                }
            }
            Err(err) => report
                .errors
                .push((relative.to_string(), format!("E_CLI_RESIDUE_IO: {err}"))),
        }
    }
    // Yalnız bu ölçümde oluşan ve artık boş kalan dizinler kaldırılır.
    for relative in &delta.created_dirs {
        if !is_declared(adapter_id, relative) {
            continue;
        }
        let Ok(path) = safe_join(root, relative) else {
            continue;
        };
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        let empty = std::fs::read_dir(&path)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if empty && std::fs::remove_dir(&path).is_ok() {
            report.removed_dirs += 1;
        }
    }
    report
}

/// **Yalnız belirtilen andan sonra yazılmış** dosyaları siler (arayüz yolu).
///
/// Arayüz `relative` listesini kendi tuttuğu için burada ikinci bir kapı var:
/// dosyanın değişim zamanı `since`'ten eskiyse **dokunulmaz** — ölçümden önce
/// var olan dosya, liste yanlış gelse bile silinmez.
pub fn cleanup_created_since(
    root: &Path,
    adapter_id: &str,
    since: SystemTime,
    relative_paths: &[String],
) -> CleanupReport {
    let mut report = CleanupReport::default();
    let since_secs = now_secs(since).saturating_sub(2);
    let mut touched_dirs: BTreeSet<String> = BTreeSet::new();
    for relative in relative_paths.iter().take(MAX_RESIDUE_FILES) {
        if !is_declared(adapter_id, relative) {
            report
                .skipped
                .push((relative.clone(), "kapsam dışı".to_string()));
            continue;
        }
        let path = match safe_join(root, relative) {
            Ok(path) => path,
            Err(_) => {
                report
                    .skipped
                    .push((relative.clone(), "güvenli olmayan yol".to_string()));
                continue;
            }
        };
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(err) => {
                report
                    .errors
                    .push((relative.clone(), format!("E_CLI_RESIDUE_IO: {err}")));
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            report
                .skipped
                .push((relative.clone(), "sembolik bağ".to_string()));
            continue;
        }
        if !meta.is_file() {
            report
                .skipped
                .push((relative.clone(), "normal dosya değil".to_string()));
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if modified < since_secs {
            report
                .skipped
                .push((relative.clone(), "ölçümden önce vardı".to_string()));
            continue;
        }
        let bytes = meta.len();
        match std::fs::remove_file(&path) {
            Ok(()) => {
                report.removed_files += 1;
                report.removed_bytes = report.removed_bytes.saturating_add(bytes);
                let mut parent = relative.rsplit_once('/').map(|(dir, _)| dir.to_string());
                while let Some(dir) = parent {
                    if dir.is_empty() || !is_declared(adapter_id, &dir) {
                        break;
                    }
                    touched_dirs.insert(dir.clone());
                    parent = dir.rsplit_once('/').map(|(up, _)| up.to_string());
                }
            }
            Err(err) => report
                .errors
                .push((relative.clone(), format!("E_CLI_RESIDUE_IO: {err}"))),
        }
    }
    // Yalnız artık boşalan ve bildirilen kapsamda kalan dizinler kaldırılır.
    let mut dirs: Vec<String> = touched_dirs.into_iter().collect();
    dirs.sort_by(|a, b| {
        b.matches('/')
            .count()
            .cmp(&a.matches('/').count())
            .then(a.cmp(b))
    });
    for dir in dirs {
        let Ok(path) = safe_join(root, &dir) else {
            continue;
        };
        let empty = std::fs::read_dir(&path)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if empty && std::fs::remove_dir(&path).is_ok() {
            report.removed_dirs += 1;
        }
    }
    report
}

/// Çökme sonrası kalan kum havuzu dizinleri: yalnız bizim önekimiz, dizin,
/// sembolik bağ değil, Unix'te `0700` ve sahibi biz, yaş eşiğini geçmiş.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SweepReport {
    pub considered: usize,
    pub removed: Vec<String>,
    pub skipped: Vec<(String, String)>,
}

#[must_use]
pub fn sweep_stale_sandboxes(base: &Path, min_age: Duration, now: SystemTime) -> SweepReport {
    let mut report = SweepReport::default();
    let Ok(read) = std::fs::read_dir(base) else {
        return report;
    };
    let now_s = now_secs(now);
    let min_age_secs = min_age.as_secs();
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(SANDBOX_PREFIX) {
            continue;
        }
        report.considered += 1;
        let path = entry.path();
        let Ok(link_meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if link_meta.file_type().is_symlink() || !link_meta.is_dir() {
            report
                .skipped
                .push((name.clone(), "dizin değil".to_string()));
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if link_meta.mode() & 0o777 != crate::sandbox::SANDBOX_DIR_MODE {
                report
                    .skipped
                    .push((name.clone(), "izin 0700 değil".to_string()));
                continue;
            }
            if link_meta.uid() != uid_of_temp() {
                report
                    .skipped
                    .push((name.clone(), "başka kullanıcı".to_string()));
                continue;
            }
        }
        let age = link_meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map(|d| now_s.saturating_sub(d.as_secs()))
            .unwrap_or(0);
        if age < min_age_secs {
            report
                .skipped
                .push((name.clone(), "henüz süren koşu olabilir".to_string()));
            continue;
        }
        match std::fs::remove_dir_all(&path) {
            Ok(()) => report.removed.push(name),
            Err(err) => report.skipped.push((name, format!("silinemedi: {err}"))),
        }
    }
    report
}

#[cfg(unix)]
fn uid_of_temp() -> u32 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(std::env::temp_dir())
        .map(|m| m.uid())
        .unwrap_or(0)
}

/// Kalıntı ölçümü: gerçek ev diziniyle **tek** sürüm yoklaması, önce/sonra farkı.
#[derive(Debug, Clone)]
pub struct ResidueMeasurement {
    pub adapter: String,
    pub root: PathBuf,
    pub scan_before: ResidueScan,
    pub scan_after: ResidueScan,
    pub delta: ResidueDelta,
    pub args: Vec<String>,
    pub exit_code: Option<i32>,
    pub status: RunStatus,
    pub duration_ms: u64,
    pub persistence: PersistenceControl,
    /// Ölçüm sırasında `HOME` kullanıcının kendi ev diziniydi (açık seçim).
    pub used_user_home: bool,
}

/// Gerçek ev dizininde kalıntı ölçümü.
///
/// Sürüm yoklaması (`version_args[0]`) boş bir kum havuzu iş dizininde çalışır,
/// ancak `HOME` **kullanıcının kendi ev dizinidir**: amaç CLI'nin gerçek
/// konumlarda ne bıraktığını ölçmek. İstem gönderilmez, kimlik dosyası
/// okunmaz. Fark yalnız bildirilen yollarda hesaplanır.
pub fn measure_residue(
    spec: &AdapterSpec,
    executable: &Path,
    home_root: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<ResidueMeasurement, RunError> {
    let sandbox = RunSandbox::create()?;
    let args: Vec<String> = spec
        .version_args
        .first()
        .map(|flag| vec![(*flag).to_string()])
        .unwrap_or_default();
    let mut env = default_env_allowlist();
    // Ölçüm gerçek ev dizininde yapılır: `HOME`/`TMPDIR` açıkça verilir.
    for (key, value) in [
        ("HOME", home_root.to_path_buf()),
        ("TMPDIR", std::env::temp_dir()),
    ] {
        let value = value.to_string_lossy().to_string();
        if let Some(slot) = env.iter_mut().find(|(k, _)| k == key) {
            slot.1 = value;
        } else {
            env.push((key.to_string(), value));
        }
    }
    let request = CliRunRequest {
        adapter_id: spec.id.to_string(),
        executable: executable.to_path_buf(),
        args: args.clone(),
        stdin_text: String::new(),
        workdir: sandbox.path().to_path_buf(),
        timeout: PROBE_TIMEOUT,
        env,
        home_isolation: HomeIsolation::UserHome,
    };
    let before = scan_residue(home_root, spec.id, SystemTime::now());
    let snapshot_before = snapshot_residue(home_root, spec.id);
    let outcome = run_cli(&request, cancel)?;
    let after = scan_residue(home_root, spec.id, SystemTime::now());
    let snapshot_after = snapshot_residue(home_root, spec.id);
    let delta = diff_snapshots(&snapshot_before, &snapshot_after);
    Ok(ResidueMeasurement {
        adapter: spec.id.to_string(),
        root: home_root.to_path_buf(),
        scan_before: before,
        scan_after: after,
        delta,
        args,
        exit_code: match outcome.status {
            RunStatus::Exited(code) => Some(code),
            _ => None,
        },
        status: outcome.status,
        duration_ms: outcome.duration_ms,
        persistence: persistence_control(spec.id),
        used_user_home: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmpdir() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "dim-residue-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&base).expect("tmp");
        base
    }

    fn write(root: &Path, relative: &str, body: &str) -> PathBuf {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(&path, body).expect("write");
        path
    }

    #[test]
    fn scan_counts_only_declared_paths_and_never_reads_content() {
        let root = tmpdir();
        write(&root, ".claude/projects/a.jsonl", "gizli metin");
        write(&root, ".claude/history.jsonl", "abc");
        write(&root, "baska/yer.txt", "kapsam disi");
        let scan = scan_residue(&root, "claude-code", SystemTime::now());
        assert_eq!(scan.adapter, "claude-code");
        let projects = scan
            .entries
            .iter()
            .find(|e| e.relative == ".claude/projects")
            .expect("projects");
        assert!(projects.exists);
        assert_eq!(projects.files, 1);
        assert_eq!(projects.kind, "transcript");
        // Bildirilmeyen dizin sayılmaz.
        assert!(!scan.entries.iter().any(|e| e.relative.starts_with("baska")));
        assert_eq!(scan.total_files, 2);
    }

    #[test]
    fn unknown_adapter_scans_nothing() {
        let root = tmpdir();
        write(&root, ".claude/projects/a.jsonl", "x");
        let scan = scan_residue(&root, "bilinmeyen", SystemTime::now());
        assert!(scan.entries.is_empty());
        assert_eq!(scan.total_files, 0);
        assert!(scan.is_empty());
    }

    #[test]
    fn symlinked_path_is_not_followed() {
        let root = tmpdir();
        let outside = tmpdir();
        write(&outside, "baska.txt", "kapsam disi");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join(".claude")).expect("symlink");
        let scan = scan_residue(&root, "claude-code", SystemTime::now());
        assert!(scan.entries.iter().all(|e| !e.exists));
        assert_eq!(scan.total_files, 0);
    }

    #[test]
    fn diff_marks_only_new_files_as_created() {
        let root = tmpdir();
        write(&root, ".codex/sessions/eski.jsonl", "eski");
        let before = snapshot_residue(&root, "codex-cli");
        write(&root, ".codex/sessions/yeni.jsonl", "yeni icerik");
        write(&root, ".codex/log/tui.log", "log");
        let after = snapshot_residue(&root, "codex-cli");
        let delta = diff_snapshots(&before, &after);
        assert_eq!(delta.created.len(), 2);
        assert!(delta
            .created
            .iter()
            .all(|s| s.relative != ".codex/sessions/eski.jsonl"));
        assert_eq!(delta.bytes_added, 14);
        assert!(delta.created_dirs.contains(&".codex/log".to_string()));
    }

    #[test]
    fn cleanup_removes_only_created_files_and_empty_dirs() {
        let root = tmpdir();
        write(&root, ".codex/sessions/eski.jsonl", "kullanici verisi");
        let before = snapshot_residue(&root, "codex-cli");
        write(&root, ".codex/sessions/yeni.jsonl", "bizim");
        let after = snapshot_residue(&root, "codex-cli");
        let delta = diff_snapshots(&before, &after);
        let report = cleanup_created(&root, "codex-cli", &delta);
        assert_eq!(report.removed_files, 1);
        assert_eq!(report.removed_bytes, 5);
        // Kullanıcının önceden var olan dosyası yerinde.
        assert!(root.join(".codex/sessions/eski.jsonl").exists());
        assert!(!root.join(".codex/sessions/yeni.jsonl").exists());
    }

    #[test]
    fn cleanup_refuses_out_of_scope_and_traversal() {
        let root = tmpdir();
        let delta = ResidueDelta {
            created: vec![
                ResidueFileStamp {
                    relative: "baska/yer.txt".to_string(),
                    bytes: 1,
                    modified_secs: 0,
                },
                ResidueFileStamp {
                    relative: ".codex/../../kacis.txt".to_string(),
                    bytes: 1,
                    modified_secs: 0,
                },
            ],
            ..ResidueDelta::default()
        };
        write(&root, "baska/yer.txt", "x");
        let report = cleanup_created(&root, "codex-cli", &delta);
        assert_eq!(report.removed_files, 0);
        assert_eq!(report.skipped.len(), 2);
        assert!(root.join("baska/yer.txt").exists());
    }

    #[test]
    fn cleanup_refuses_symlink_target() {
        let root = tmpdir();
        let target = tmpdir();
        write(&target, "hedef.txt", "kullanici");
        fs::create_dir_all(root.join(".codex")).expect("mkdir");
        #[cfg(unix)]
        std::os::unix::fs::symlink(target.join("hedef.txt"), root.join(".codex/history.jsonl"))
            .expect("symlink");
        let delta = ResidueDelta {
            created: vec![ResidueFileStamp {
                relative: ".codex/history.jsonl".to_string(),
                bytes: 8,
                modified_secs: 0,
            }],
            ..ResidueDelta::default()
        };
        let report = cleanup_created(&root, "codex-cli", &delta);
        assert_eq!(report.removed_files, 0);
        assert!(report
            .skipped
            .iter()
            .any(|(_, reason)| reason == "sembolik bağ"));
        assert!(target.join("hedef.txt").exists());
    }

    #[test]
    fn cleanup_since_refuses_files_older_than_measurement() {
        let root = tmpdir();
        let old_path = write(&root, ".codex/history.jsonl", "eski kullanici verisi");
        let old_time = SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&old_path)
            .expect("open")
            .set_modified(old_time)
            .expect("mtime");
        write(&root, ".codex/log/tui.log", "bizim");
        let report = cleanup_created_since(
            &root,
            "codex-cli",
            SystemTime::now(),
            &[
                ".codex/history.jsonl".to_string(),
                ".codex/log/tui.log".to_string(),
                "baska/yer.txt".to_string(),
            ],
        );
        assert_eq!(report.removed_files, 1);
        assert!(
            old_path.exists(),
            "ölçümden önce var olan dosya silinmemeli"
        );
        assert!(!root.join(".codex/log/tui.log").exists());
        assert!(report.skipped.iter().any(
            |(path, reason)| path == ".codex/history.jsonl" && reason == "ölçümden önce vardı"
        ));
        assert!(report
            .skipped
            .iter()
            .any(|(_, reason)| reason == "kapsam dışı"));
    }

    #[cfg(unix)]
    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("mode");
        path
    }

    /// Uçtan uca: sahte CLI `$HOME/.claude/projects` altına işaretçi yazar;
    /// ölçüm onu **yeni** olarak bulur, temizlik yalnız onu siler.
    #[test]
    #[cfg(unix)]
    fn measure_then_cleanup_removes_only_the_new_marker() {
        let home = tmpdir();
        let user_file = write(&home, ".claude/projects/eski.jsonl", "kullanici verisi");
        let scripts = tmpdir();
        let exe = script(
            &scripts,
            "fake-claude",
            "mkdir -p \"$HOME/.claude/projects\"\nprintf 'yeni marker' > \"$HOME/.claude/projects/marker.jsonl\"\necho ok",
        );
        let spec = crate::spec("claude-code").expect("spec");
        let cancel = Arc::new(AtomicBool::new(false));
        let measurement = measure_residue(spec, &exe, &home, &cancel).expect("measure");
        assert_eq!(measurement.delta.created.len(), 1);
        assert_eq!(
            measurement.delta.created[0].relative,
            ".claude/projects/marker.jsonl"
        );
        assert_eq!(measurement.delta.bytes_added, 11);
        assert!(measurement.used_user_home);
        assert!(measurement.scan_after.total_files >= 2);

        let report = cleanup_created(&home, "claude-code", &measurement.delta);
        assert_eq!(report.removed_files, 1);
        assert_eq!(report.removed_bytes, 11);
        assert!(user_file.exists(), "kullanıcı dosyası korunmalı");
        assert!(!home.join(".claude/projects/marker.jsonl").exists());
    }

    /// Kum havuzu kipi: aynı sahte CLI `$HOME`'a yazsa bile yazdığı yer kum
    /// havuzudur; kullanıcının ev dizininde kalıntı **oluşmaz**.
    #[test]
    #[cfg(unix)]
    fn sandboxed_run_writes_no_residue_into_the_home() {
        let home = tmpdir();
        let scripts = tmpdir();
        let exe = script(
            &scripts,
            "fake-claude",
            "mkdir -p \"$HOME/.claude/projects\"\nprintf 'marker' > \"$HOME/.claude/projects/marker.jsonl\"\necho ok",
        );
        let sandbox = RunSandbox::create().expect("sandbox");
        let request = CliRunRequest {
            adapter_id: "claude-code".to_string(),
            executable: exe,
            args: vec!["--version".to_string()],
            stdin_text: String::new(),
            workdir: sandbox.path().to_path_buf(),
            timeout: Duration::from_secs(15),
            env: vec![
                (
                    "PATH".to_string(),
                    std::env::var("PATH").unwrap_or_default(),
                ),
                ("HOME".to_string(), home.to_string_lossy().to_string()),
            ],
            home_isolation: HomeIsolation::Sandbox,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let outcome = run_cli(&request, &cancel).expect("run");
        assert!(matches!(outcome.status, RunStatus::Exited(0)));
        assert!(
            outcome
                .scope
                .inventory
                .iter()
                .any(|entry| entry.name.contains("marker.jsonl")),
            "işaretçi kum havuzunda sayılmalı: {:?}",
            outcome.scope.inventory
        );
        let scan = scan_residue(&home, "claude-code", SystemTime::now());
        assert_eq!(scan.total_files, 0, "ev dizininde kalıntı olmamalı");
    }

    #[test]
    fn sweep_removes_only_old_app_sandboxes() {
        let base = tmpdir();
        let old = base.join(format!("{SANDBOX_PREFIX}1-0"));
        let fresh = base.join(format!("{SANDBOX_PREFIX}2-0"));
        fs::create_dir_all(&old).expect("old");
        fs::create_dir_all(&fresh).expect("fresh");
        fs::create_dir_all(base.join("baska-dizin")).expect("other");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&old, fs::Permissions::from_mode(0o700)).expect("mode");
            fs::set_permissions(&fresh, fs::Permissions::from_mode(0o700)).expect("mode");
        }
        let now = SystemTime::now();
        let report = sweep_stale_sandboxes(&base, Duration::from_secs(3600), now);
        assert_eq!(report.considered, 2);
        // Yeni dizin yaş eşiğini geçmediği için duruyor.
        assert!(fresh.exists());
        assert!(base.join("baska-dizin").exists());
        assert!(report.removed.is_empty());
    }

    #[test]
    fn sweep_skips_permissive_dir() {
        let base = tmpdir();
        let dir = base.join(format!("{SANDBOX_PREFIX}9-9"));
        fs::create_dir_all(&dir).expect("dir");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).expect("mode");
        }
        let report = sweep_stale_sandboxes(&base, Duration::from_secs(0), SystemTime::now());
        assert_eq!(report.removed.len(), 0);
        assert!(dir.exists());
        #[cfg(unix)]
        assert!(report
            .skipped
            .iter()
            .any(|(_, reason)| reason == "izin 0700 değil"));
    }

    #[test]
    fn persistence_control_is_version_scoped() {
        let claude = persistence_control("claude-code");
        assert!(claude.usable());
        assert_eq!(claude.flag, Some("--no-session-persistence"));
        let codex = persistence_control("codex-cli");
        assert_eq!(codex.flag, Some("--ephemeral"));
        assert_eq!(codex.applies_to, "exec");
        assert!(!persistence_control("kimi-cli").usable());
        assert!(!persistence_control("antigravity-cli").usable());
    }

    #[test]
    fn declared_flag_passes_the_argument_gate() {
        for (adapter, flag) in [
            ("claude-code", "--no-session-persistence"),
            ("codex-cli", "--ephemeral"),
        ] {
            assert!(
                crate::build_args(adapter, &[flag.to_string()]).is_ok(),
                "{adapter} {flag} kapıdan geçmeli"
            );
        }
    }
}
