//! İzinli argümanlarla **gerçek** CLI yürütme (F10-T04/T05/T07/T14).
//!
//! Kurallar:
//! - Yürütülebilir **mutlak** yol olmalı ve gerçekten çalıştırılabilir olmalı;
//!   shell string birleştirme yok, argümanlar `build_args` kapısından geçer.
//! - Ortam **allowlist** ile kurulur; `Command::env_clear()` sonrası yalnız
//!   verilen değişkenler aktarılır. Anahtar/token/cookie gibi görünen adlar
//!   allowlist'e yazılsa bile reddedilir.
//! - İş dizini **kapsamlı** olmalı (`sandbox::check_workdir`): mutlak, sembolik
//!   bağ değil, boş, ev dizini ya da üstü değil, Unix'te `0700`.
//! - `HOME`/`TMPDIR` varsayılan olarak kum havuzunun içine yönlendirilir
//!   (`HomeIsolation::Sandbox`).
//! - stdin metni `MAX_STDIN_BYTES` ile sınırlıdır; aşan istek süreç başlamadan
//!   reddedilir. Yazım parçalı ve iptal duyarlıdır.
//! - Süre sınırı ve iptal bayrağı birlikte uygulanır; süre dolunca/iptalde alt
//!   süreç **öldürülür**, kısmi çıktı raporlanır.
//! - Çıktı en çok 64 KiB baş kısmıyla döner; kesildiği ayrıca işaretlenir.
//! - Çalıştırmadan sonra kum havuzunda kalan dosyalar **sayılır** (ad + bayt);
//!   içerik okunmaz, denetime yazılmaz.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::sandbox::{
    check_workdir, HomeIsolation, SandboxEntry, MAX_INVENTORY_ENTRIES, MAX_SANDBOX_BYTES,
    MAX_STDIN_BYTES,
};
use crate::{build_args, ensure_batch_allowed, AdapterError};

/// Alt süreçten geri getirilen en çok bayt (baş kısmı).
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;
pub const MIN_TIMEOUT: Duration = Duration::from_secs(1);
pub const MAX_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStatus {
    /// Süreç kendi koduyla çıktı.
    Exited(i32),
    /// Süre doldu; süreç öldürüldü.
    TimedOut,
    /// Kullanıcı iptal etti; süreç öldürüldü.
    Cancelled,
    /// Sinyalle sonlandı (kod yok).
    Signaled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub status: RunStatus,
    pub stdout_head: String,
    pub stderr_head: String,
    pub duration_ms: u64,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    /// Kapsam kanıtı: iş dizini izni, `HOME`/`TMPDIR` politikası, stdin
    /// boyutu ve alt sürecin bıraktığı dosyaların sayımı.
    pub scope: RunScope,
    /// `codex-cli --json` ise ayrıştırılmış olaylar. Çıktı kırpıldıysa boş
    /// bırakılır — yarım JSONL'den "cevap" uydurulmaz.
    pub codex_events: Vec<crate::events::CodexEvent>,
}

/// Yürütmenin **kapsam** kanıtı. İçerik taşımaz: yalnız izin, yol, sayı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunScope {
    pub workdir: PathBuf,
    /// Unix izni (ör. `0o700`); Unix dışında `None`.
    pub workdir_mode: Option<u32>,
    /// `HomeIsolation::Sandbox` ise kum havuzundaki `HOME`.
    pub home: Option<PathBuf>,
    /// `HomeIsolation::Sandbox` ise kum havuzundaki `TMPDIR`.
    pub tmp: Option<PathBuf>,
    pub home_isolation: HomeIsolation,
    pub stdin_bytes: usize,
    pub stdin_limit: usize,
    pub inventory: Vec<SandboxEntry>,
    pub inventory_total_bytes: u64,
    pub inventory_truncated: bool,
    pub inventory_limit_entries: usize,
    pub sandbox_size_limit: u64,
}

impl RunScope {
    /// Kum havuzu boyut sınırı aşıldı mı? (Sonradan ölçülür, sert sınır yok.)
    #[must_use]
    pub const fn size_limit_exceeded(&self) -> bool {
        self.inventory_total_bytes > self.sandbox_size_limit
    }
}

impl RunOutcome {
    #[must_use]
    pub const fn succeeded(&self) -> bool {
        matches!(self.status, RunStatus::Exited(0))
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RunError {
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error("yürütülebilir yol mutlak olmalı")]
    RelativeExecutable,
    #[error("yürütülebilir bulunamadı veya çalıştırılamaz")]
    ExecutableUnavailable,
    #[error("iş dizini yok veya dizin değil")]
    InvalidWorkdir,
    #[error("iş dizini kapsam dışı (ev dizini ya da üstü): {0}")]
    WorkdirNotScoped(PathBuf),
    #[error("iş dizini boş değil: {0}")]
    WorkdirNotEmpty(PathBuf),
    #[error("iş dizini izni çok geniş: {0:o}")]
    WorkdirPermissive(u32),
    #[error("stdin çok büyük: {bytes} bayt (sınır {limit})")]
    StdinTooLarge { bytes: usize, limit: usize },
    #[error("geçici iş dizini hazırlanamadı: {0}")]
    WorkdirUnavailable(String),
    #[error("ortam değişkeni reddedildi: {0}")]
    EnvRejected(String),
    #[error("süreç başlatılamadı: {0}")]
    SpawnFailed(String),
}

impl RunError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Adapter(err) => err.code(),
            Self::RelativeExecutable => "E_CLI_EXEC_PATH",
            Self::ExecutableUnavailable => "E_CLI_EXEC_MISSING",
            Self::InvalidWorkdir => "E_CLI_EXEC_WORKDIR",
            Self::WorkdirNotScoped(_) => "E_CLI_WORKDIR_SCOPE",
            Self::WorkdirNotEmpty(_) => "E_CLI_WORKDIR_NOT_EMPTY",
            Self::WorkdirPermissive(_) => "E_CLI_WORKDIR_PERM",
            Self::StdinTooLarge { .. } => "E_CLI_STDIN",
            Self::WorkdirUnavailable(_) => "E_CLI_EXEC_WORKDIR",
            Self::EnvRejected(_) => "E_CLI_EXEC_ENV",
            Self::SpawnFailed(_) => "E_CLI_EXEC_SPAWN",
        }
    }
}

/// Yürütme isteği. `env` **tam** ortamdır: başka hiçbir değişken aktarılmaz.
#[derive(Debug, Clone)]
pub struct CliRunRequest {
    pub adapter_id: String,
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub stdin_text: String,
    pub workdir: PathBuf,
    pub timeout: Duration,
    pub env: Vec<(String, String)>,
    /// `HOME`/`TMPDIR` politikası. Varsayılan çağrı yerleri
    /// `HomeIsolation::Sandbox` seçer; `UserHome` **açık** seçimdir ve raporda
    /// görünür.
    pub home_isolation: HomeIsolation,
}

/// Varsayılan ortam allowlist'i: yalnız süreç çalışması için gerekenler.
/// API anahtarı, token, cookie **yok**.
///
/// `USER` **zorunlu** (ölçüldü, Claude Code 2.1.303, 2026-10-27): allowlist'te
/// yokken CLI `--print` çağrısı "Not logged in · Please run /login" ile
/// düşüyordu; `USER` eklendiğinde aynı çağrı kimlik doğrulayıp yanıt verdi.
/// Gizli değer değildir (süreç zaten o kullanıcı olarak çalışır), ama eksikliği
/// kimlikli CLI yolunu tamamen kapatıyordu.
#[must_use]
pub fn default_env_allowlist() -> Vec<(String, String)> {
    ["PATH", "HOME", "USER", "LANG", "LC_ALL", "TMPDIR", "TZ"]
        .iter()
        .filter_map(|key| {
            std::env::var(key)
                .ok()
                .map(|value| ((*key).to_string(), value))
        })
        .collect()
}

fn looks_secret(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    [
        "TOKEN",
        "SECRET",
        "API_KEY",
        "APIKEY",
        "PASSWORD",
        "PASSWD",
        "COOKIE",
        "CREDENTIAL",
        "PRIVATE_KEY",
        "SESSION",
    ]
    .iter()
    .any(|needle| upper.contains(needle))
}

/// Ortam allowlist'ini denetler: gizli görünen adlar ve boş adlar reddedilir.
pub fn sanitize_env(allowlist: &[(String, String)]) -> Result<Vec<(String, String)>, RunError> {
    let mut out = Vec::with_capacity(allowlist.len());
    for (key, value) in allowlist {
        let trimmed = key.trim();
        if trimmed.is_empty()
            || trimmed.contains('=')
            || trimmed.contains('\0')
            || value.contains('\0')
        {
            return Err(RunError::EnvRejected(key.clone()));
        }
        if looks_secret(trimmed) {
            return Err(RunError::EnvRejected(trimmed.to_string()));
        }
        out.push((trimmed.to_string(), value.clone()));
    }
    Ok(out)
}

#[must_use]
pub fn clamp_timeout(requested: Duration) -> Duration {
    if requested < MIN_TIMEOUT {
        MIN_TIMEOUT
    } else if requested > MAX_TIMEOUT {
        MAX_TIMEOUT
    } else {
        requested
    }
}

fn executable_ok(path: &Path) -> bool {
    if !path.is_absolute() || !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn read_limited<R: Read>(mut reader: R) -> (String, bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let room = MAX_OUTPUT_BYTES - buf.len();
                if n <= room {
                    buf.extend_from_slice(&chunk[..n]);
                } else {
                    buf.extend_from_slice(&chunk[..room]);
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }
    (String::from_utf8_lossy(&buf).to_string(), truncated)
}

/// Yeni yazılmış bir dosyayı hemen yürütmek Linux'ta `ETXTBSY` ("text file
/// busy") verebilir: başka bir iş parçacığının `fork`'u yazma tanıtıcısını bir an
/// taşır. Kısa aralıklarla yeniden denenir; son hata metni korunur.
fn spawn_with_retry(command: &mut Command) -> Result<std::process::Child, RunError> {
    let mut last = String::new();
    for attempt in 0..6 {
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(err) => {
                last = err.to_string();
                if attempt < 5 {
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
    Err(RunError::SpawnFailed(last))
}

/// CLI'yi çalıştırır. `cancel` işaretlenirse süreç öldürülür ve `Cancelled`
/// döner; süre aşılırsa `TimedOut`. Kısmi çıktı her iki durumda da raporlanır.
pub fn run_cli(req: &CliRunRequest, cancel: &Arc<AtomicBool>) -> Result<RunOutcome, RunError> {
    // Abonelik/etkileşim kapıları: Kimi gibi etkileşimli-only sağlayıcılar
    // otomatik yürütmeye açılmaz.
    ensure_batch_allowed(&req.adapter_id)?;
    let args = build_args(&req.adapter_id, &req.args)?;
    let args = hardened_args(&req.adapter_id, args);
    if !req.executable.is_absolute() {
        return Err(RunError::RelativeExecutable);
    }
    if !executable_ok(&req.executable) {
        return Err(RunError::ExecutableUnavailable);
    }
    // İş dizini kapsamı: boş, özel, ev dizini/üstü değil, sembolik bağ değil.
    let workdir = check_workdir(&req.workdir)?;
    // stdin sınırı **süreç başlamadan** uygulanır.
    if req.stdin_text.len() > MAX_STDIN_BYTES {
        return Err(RunError::StdinTooLarge {
            bytes: req.stdin_text.len(),
            limit: MAX_STDIN_BYTES,
        });
    }
    let mut env = sanitize_env(&req.env)?;
    let mut home = None;
    let mut tmp = None;
    if req.home_isolation == HomeIsolation::Sandbox {
        // Kum havuzu içinde özel HOME/TMPDIR: alt süreç varsayılan konumlarda
        // kullanıcı yapılandırmasını görmez.
        let home_dir = workdir.path.join(crate::sandbox::HOME_DIR_NAME);
        let tmp_dir = workdir.path.join(crate::sandbox::TMP_DIR_NAME);
        for dir in [&home_dir, &tmp_dir] {
            std::fs::create_dir_all(dir)
                .map_err(|err| RunError::WorkdirUnavailable(err.to_string()))?;
            crate::sandbox::set_private_mode(dir)?;
        }
        let home_canonical = home_dir.canonicalize().unwrap_or(home_dir);
        let tmp_canonical = tmp_dir.canonicalize().unwrap_or(tmp_dir);
        env.retain(|(key, _)| key != "HOME" && key != "TMPDIR");
        env.push((
            "HOME".to_string(),
            home_canonical.to_string_lossy().to_string(),
        ));
        env.push((
            "TMPDIR".to_string(),
            tmp_canonical.to_string_lossy().to_string(),
        ));
        home = Some(home_canonical);
        tmp = Some(tmp_canonical);
    }
    let timeout = clamp_timeout(req.timeout);

    let mut command = Command::new(&req.executable);
    command
        .args(&args)
        .current_dir(&workdir.path)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &env {
        command.env(key, value);
    }

    let started = Instant::now();
    let mut child = spawn_with_retry(&mut command)?;

    // stdin ayrı iş parçacığında yazılır: boru dolarsa süreç kilitlenmesin.
    // Yazım parçalıdır (8 KiB) ve parçalar arasında iptal bayrağına bakar.
    let stdin_payload = req.stdin_text.clone();
    let stdin_bytes = stdin_payload.len();
    let stdin_cancel = Arc::clone(cancel);
    let mut stdin_handle = child.stdin.take();
    let writer = std::thread::spawn(move || {
        if let Some(mut pipe) = stdin_handle.take() {
            for chunk in stdin_payload.as_bytes().chunks(8192) {
                if stdin_cancel.load(Ordering::SeqCst) {
                    break;
                }
                if pipe.write_all(chunk).is_err() {
                    break;
                }
            }
            let _ = pipe.flush();
        }
    });

    let stdout_handle = child
        .stdout
        .take()
        .map(|pipe| std::thread::spawn(move || read_limited(pipe)));
    let stderr_handle = child
        .stderr
        .take()
        .map(|pipe| std::thread::spawn(move || read_limited(pipe)));

    let deadline = started + timeout;
    let mut status = None;
    while status.is_none() {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            status = Some(RunStatus::Cancelled);
            break;
        }
        match child.try_wait() {
            Ok(Some(exit)) => {
                status = Some(match exit.code() {
                    Some(code) => RunStatus::Exited(code),
                    None => RunStatus::Signaled,
                });
                break;
            }
            Ok(None) => {}
            Err(_) => {
                let _ = child.kill();
                status = Some(RunStatus::Signaled);
                break;
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            status = Some(RunStatus::TimedOut);
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let _ = &writer;
    // stdin yazıcısı büyük girdide bloke kalabilir; süreç öldükten sonra
    // sınırlı bekle, kalan işi bırak (boru kapanınca kendiliğinden biter).
    let writer_deadline = Instant::now() + Duration::from_millis(1000);
    while !writer.is_finished() && Instant::now() < writer_deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if writer.is_finished() {
        let _ = writer.join();
    }

    // Okuyucular boru kapanınca biter. Torun süreç boruyu açık tutarsa
    // sonsuza kadar beklememek için sınırlı süre bekle.
    let mut stdout_head = String::new();
    let mut stderr_head = String::new();
    let mut stdout_truncated = false;
    let mut stderr_truncated = false;
    let mut stdout_done = stdout_handle.is_none();
    let mut stderr_done = stderr_handle.is_none();
    let reader_deadline = Instant::now() + Duration::from_millis(2000);
    loop {
        let out_done = stdout_handle
            .as_ref()
            .is_none_or(std::thread::JoinHandle::is_finished);
        let err_done = stderr_handle
            .as_ref()
            .is_none_or(std::thread::JoinHandle::is_finished);
        if out_done && err_done {
            break;
        }
        if Instant::now() >= reader_deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if let Some(handle) = stdout_handle {
        if handle.is_finished() {
            if let Ok((text, truncated)) = handle.join() {
                stdout_head = text;
                stdout_truncated = truncated;
                stdout_done = true;
            }
        }
    }
    if let Some(handle) = stderr_handle {
        if handle.is_finished() {
            if let Ok((text, truncated)) = handle.join() {
                stderr_head = text;
                stderr_truncated = truncated;
                stderr_done = true;
            }
        }
    }
    // Okuyucu kapanmadıysa çıktı eksik: bunu kesilmiş say, asla tam deme.
    if !stdout_done {
        stdout_truncated = true;
    }
    if !stderr_done {
        stderr_truncated = true;
    }

    // Kum havuzunda kalan dosyalar sayılır (içerik okunmaz).
    let inventory = crate::sandbox::inventory_of(&workdir.path);

    let codex_events = if req.adapter_id == "codex-cli"
        && req.args.iter().any(|arg| arg == "--json")
        && !stdout_truncated
    {
        crate::events::parse_codex_json_lines(&stdout_head).unwrap_or_default()
    } else {
        Vec::new()
    };
    Ok(RunOutcome {
        status: status.unwrap_or(RunStatus::Signaled),
        stdout_head,
        stderr_head,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        stdout_truncated,
        stderr_truncated,
        codex_events,
        scope: RunScope {
            workdir: workdir.path.clone(),
            workdir_mode: workdir.mode,
            home,
            tmp,
            home_isolation: req.home_isolation,
            stdin_bytes,
            stdin_limit: MAX_STDIN_BYTES,
            inventory_total_bytes: inventory.total_bytes,
            inventory_truncated: inventory.truncated,
            inventory_limit_entries: MAX_INVENTORY_ENTRIES,
            sandbox_size_limit: MAX_SANDBOX_BYTES,
            inventory: inventory.entries,
        },
    })
}

/// Bir bayrak grubu: bayrak ve (varsa) ürünün sabitlediği değeri **birlikte**
/// eklenir. Değer ayrı belirteçte olabilir (`--tools ""`).
type FlagGroup = &'static [&'static str];

/// CLI'nın kullanıcı yapılandırmasını (hook, MCP, proje talimatı, kurallar) ve
/// araçlarını devre dışı bırakan **zorunlu** gruplar. Çağıran bunları vermeyi
/// unutsa bile ürün kendi güvenlik çerçevesini uygular.
///
/// Kanıt (Codex 0.159.2, 2026-10-25): `--ignore-user-config --ignore-rules`
/// olmadan kullanıcının `SessionStart`/`Stop` hook'ları çalıştı ve çıktıya
/// karıştı; bu bayraklarla hook satırları kayboldu, model yanıtı korundu.
fn hardened_groups(adapter_id: &str) -> &'static [FlagGroup] {
    match adapter_id {
        // Kullanıcı hook/MCP/proje talimatı yüklenmesin.
        "codex-cli" => &[&["--ignore-user-config"], &["--ignore-rules"]],
        "claude-code" => &[
            // Oturum kalıcılığı kapalı: her çağrı bağımsız, geçmiş taşınmaz.
            &["--no-session-persistence"],
            // ÖLÇÜLDÜ (Claude Code 2.1.303, 2026-10-27): boş iş dizininde bile
            // `--allowedTools Read` ile **mutlak yol** okunuyordu (kum havuzu dışı
            // canary). `--tools ""` bunu kapatıyor; `--tools=` (tek belirteç)
            // **kapatmıyor** — model araç çağrısı üretmeye devam etti.
            &["--tools", ""],
            // `--mcp-config` verilmediği için hiçbir MCP sunucusu yüklenmez.
            &["--strict-mcp-config"],
        ],
        // `--sandbox` tek başına dosya okumayı engellemiyor (ölçüldü); yanına
        // slash komut / skill genişlemesini kapatan bayrak eklenir.
        "antigravity-cli" => &[&["--sandbox"], &["--disable-slash-commands"]],
        _ => &[],
    }
}

/// `args` içinde `flag` geçiyorsa onu (ve bayrak değer alıyorsa ardından gelen
/// **bayrak olmayan** belirteçleri) atar. Böylece zorunlu bayrak çağıranın
/// verdiği değerle değil, ürünün verdiği değerle çalışır; yinelenen bayrak olmaz.
fn strip_flag(args: &mut Vec<String>, flag: &str, takes_value: bool) {
    let mut i = 0;
    while i < args.len() {
        let same = args[i] == flag
            || (args[i].starts_with(flag)
                && args[i].len() > flag.len()
                && args[i][flag.len()..].starts_with('='));
        if !same {
            i += 1;
            continue;
        }
        args.remove(i);
        while takes_value && i < args.len() && !args[i].starts_with('-') {
            args.remove(i);
        }
    }
}

pub fn hardened_args(adapter_id: &str, mut args: Vec<String>) -> Vec<String> {
    for group in hardened_groups(adapter_id) {
        let flag = group[0];
        // Değerli bayrak: grupta ikinci belirteç varsa bayrak değer alıyor demektir.
        strip_flag(&mut args, flag, group.len() > 1);
        args.extend(group.iter().map(|arg| (*arg).to_string()));
    }
    args
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
        }
        path
    }

    /// Boş, `0700` bir alt dizin: iş dizini kapsam kurallarına uyar.
    fn isolated(root: &Path) -> PathBuf {
        let work = root.join("iso");
        if !work.is_dir() {
            std::fs::create_dir(&work).expect("iso dizin");
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&work, std::fs::Permissions::from_mode(0o700)).expect("mode");
        }
        work
    }

    fn request_at(
        exe: PathBuf,
        workdir: PathBuf,
        args: &[&str],
        stdin: &str,
        timeout: Duration,
        home_isolation: HomeIsolation,
    ) -> CliRunRequest {
        CliRunRequest {
            adapter_id: "claude-code".into(),
            executable: exe,
            args: args.iter().map(|a| (*a).to_string()).collect(),
            stdin_text: stdin.into(),
            workdir,
            timeout,
            env: vec![("DIM_TEST_MARK".into(), "evet".into())],
            home_isolation,
        }
    }

    fn request(
        exe: PathBuf,
        root: &Path,
        args: &[&str],
        stdin: &str,
        timeout: Duration,
    ) -> CliRunRequest {
        request_at(
            exe,
            isolated(root),
            args,
            stdin,
            timeout,
            HomeIsolation::Sandbox,
        )
    }

    #[test]
    fn runs_real_process_with_stdin_and_allowed_args() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(tmp.path(), "cat.sh", "cat");
        let cancel = Arc::new(AtomicBool::new(false));
        let outcome = run_cli(
            &request(
                exe,
                tmp.path(),
                &["--print"],
                "itirazın reddine",
                Duration::from_secs(10),
            ),
            &cancel,
        )
        .expect("çalıştı");
        assert_eq!(outcome.status, RunStatus::Exited(0));
        assert!(outcome.succeeded());
        assert_eq!(outcome.stdout_head, "itirazın reddine");
        assert!(!outcome.stdout_truncated);
    }

    #[test]
    fn disallowed_argument_is_rejected_before_spawn() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(tmp.path(), "ok.sh", "echo calisti");
        let cancel = Arc::new(AtomicBool::new(false));
        let err = run_cli(
            &request(
                exe,
                tmp.path(),
                &["--dangerously-skip-permissions"],
                "",
                Duration::from_secs(5),
            ),
            &cancel,
        )
        .expect_err("reddedilmeli");
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
    }

    #[test]
    fn timeout_kills_child_and_reports_partial_output() {
        let tmp = tempfile::tempdir().expect("tmp");
        // `exec`: shell kendini sleep ile değiştirir; öldürülen süreç gerçekten
        // çalışan süreçtir (torun değil). 30 sn uyku + 5 sn sınır: yük altında
        // bile "önce çıktı, sonra öldürme" sırası garanti.
        let exe = script(tmp.path(), "slow.sh", "echo başladı; exec sleep 30");
        let cancel = Arc::new(AtomicBool::new(false));
        let outcome = run_cli(
            &request(exe, tmp.path(), &["--print"], "", Duration::from_secs(5)),
            &cancel,
        )
        .expect("çalıştı");
        assert_eq!(outcome.status, RunStatus::TimedOut);
        assert!(
            outcome.stdout_head.contains("başladı"),
            "stdout: {:?} stderr: {:?}",
            outcome.stdout_head,
            outcome.stderr_head
        );
        // Öldürme çalışmasaydı `wait` 30 sn sürerdi.
        assert!(
            outcome.duration_ms < 15_000,
            "süre: {}",
            outcome.duration_ms
        );
    }

    #[test]
    fn cancel_flag_stops_running_child() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(tmp.path(), "sleep.sh", "sleep 5");
        let cancel = Arc::new(AtomicBool::new(true));
        // İptal bayrağı işaretli: yürütme hemen kesilir.
        let outcome = run_cli(
            &request(exe, tmp.path(), &["--print"], "", Duration::from_secs(10)),
            &cancel,
        )
        .expect("çalıştı");
        assert_eq!(outcome.status, RunStatus::Cancelled);
        assert!(outcome.duration_ms < 3000);
    }

    /// `USER` allowlist'te olmalı: yoksa kimlikli CLI çağrısı "Not logged in"
    /// ile düşüyor (ölçüldü). Ortamda `USER` yoksa test atlanır.
    #[test]
    fn env_allowlist_carries_the_user_variable() {
        if std::env::var_os("USER").is_none() {
            return;
        }
        let keys: Vec<String> = default_env_allowlist()
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert!(keys.iter().any(|key| key == "USER"), "{keys:?}");
    }

    #[test]
    fn env_is_allowlisted_only() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(tmp.path(), "env.sh", "env");
        let cancel = Arc::new(AtomicBool::new(false));
        let mut req = request(exe, tmp.path(), &["--print"], "", Duration::from_secs(10));
        req.env = vec![("DIM_TEST_MARK".into(), "evet".into())];
        let outcome = run_cli(&req, &cancel).expect("çalıştı");
        assert!(outcome.stdout_head.contains("DIM_TEST_MARK=evet"));
        // Allowlist'te olmayan PATH çocuğa geçmez.
        assert!(!outcome.stdout_head.contains("PATH="));
        assert!(!outcome.stdout_head.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn secret_env_names_are_rejected() {
        let err = sanitize_env(&[("OPENAI_API_KEY".into(), "x".into())]).expect_err("red");
        assert_eq!(err.code(), "E_CLI_EXEC_ENV");
        let err = sanitize_env(&[("GITHUB_TOKEN".into(), "x".into())]).expect_err("red");
        assert_eq!(err.code(), "E_CLI_EXEC_ENV");
        assert!(sanitize_env(&[("LANG".into(), "tr_TR.UTF-8".into())]).is_ok());
    }

    #[test]
    fn relative_executable_and_missing_workdir_rejected() {
        let tmp = tempfile::tempdir().expect("tmp");
        let cancel = Arc::new(AtomicBool::new(false));
        let mut req = request(
            PathBuf::from("cat"),
            tmp.path(),
            &[],
            "",
            Duration::from_secs(5),
        );
        assert_eq!(
            run_cli(&req, &cancel).expect_err("göreli").code(),
            "E_CLI_EXEC_PATH"
        );
        let exe = script(tmp.path(), "cat.sh", "cat");
        req = request_at(
            exe,
            tmp.path().join("yok"),
            &[],
            "",
            Duration::from_secs(5),
            HomeIsolation::Sandbox,
        );
        assert_eq!(
            run_cli(&req, &cancel).expect_err("dizin").code(),
            "E_CLI_EXEC_WORKDIR"
        );
    }

    #[test]
    fn workdir_scope_rules_are_enforced_before_spawn() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(tmp.path(), "cat.sh", "cat");
        let cancel = Arc::new(AtomicBool::new(false));

        // 1) Grup/diğer erişimi olan dizin reddedilir.
        let open = tmp.path().join("acik");
        std::fs::create_dir(&open).expect("dizin");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o755)).expect("mode");
        }
        let req = request_at(
            exe.clone(),
            open,
            &["--print"],
            "",
            Duration::from_secs(5),
            HomeIsolation::Sandbox,
        );
        assert_eq!(
            run_cli(&req, &cancel).expect_err("izin").code(),
            "E_CLI_WORKDIR_PERM"
        );

        // 2) Boş olmayan dizin (proje talimatı/hook dosyası) reddedilir.
        let dirty = tmp.path().join("kirli");
        std::fs::create_dir(&dirty).expect("dizin");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dirty, std::fs::Permissions::from_mode(0o700)).expect("mode");
        }
        std::fs::write(dirty.join("AGENTS.md"), "talimat").expect("dosya");
        let req = request_at(
            exe,
            dirty,
            &["--print"],
            "",
            Duration::from_secs(5),
            HomeIsolation::Sandbox,
        );
        assert_eq!(
            run_cli(&req, &cancel).expect_err("boş değil").code(),
            "E_CLI_WORKDIR_NOT_EMPTY"
        );

        // 3) Ev dizini iş dizini olamaz.
        let home = PathBuf::from(std::env::var("HOME").expect("HOME"));
        let req = request_at(
            PathBuf::from("/bin/echo"),
            home,
            &["--print"],
            "",
            Duration::from_secs(5),
            HomeIsolation::Sandbox,
        );
        assert_eq!(
            run_cli(&req, &cancel).expect_err("ev").code(),
            "E_CLI_WORKDIR_SCOPE"
        );
    }

    #[test]
    fn stdin_over_limit_is_rejected_before_spawn() {
        let tmp = tempfile::tempdir().expect("tmp");
        // Süreç başlarsa bu dosya oluşur; oluşmaması "başlamadı" kanıtıdır.
        let exe = script(tmp.path(), "ran.sh", "echo x > ran.txt");
        let cancel = Arc::new(AtomicBool::new(false));
        let too_big = "a".repeat(MAX_STDIN_BYTES + 1);
        let req = request(
            exe,
            tmp.path(),
            &["--print"],
            &too_big,
            Duration::from_secs(5),
        );
        let work = req.workdir.clone();
        assert_eq!(
            run_cli(&req, &cancel).expect_err("stdin").code(),
            "E_CLI_STDIN"
        );
        assert!(!work.join("ran.txt").exists(), "süreç başlamamalıydı");
        assert!(
            !work.join("home").exists(),
            "stdin kapısı kum havuzu dizinlerinden önce çalışır"
        );
    }

    #[test]
    fn stdin_at_limit_is_delivered_completely() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(tmp.path(), "wc.sh", "wc -c");
        let cancel = Arc::new(AtomicBool::new(false));
        let payload = "b".repeat(MAX_STDIN_BYTES);
        let outcome = run_cli(
            &request(
                exe,
                tmp.path(),
                &["--print"],
                &payload,
                Duration::from_secs(20),
            ),
            &cancel,
        )
        .expect("çalıştı");
        assert_eq!(outcome.status, RunStatus::Exited(0));
        assert_eq!(
            outcome.stdout_head.trim().parse::<usize>().expect("sayı"),
            MAX_STDIN_BYTES
        );
        assert_eq!(outcome.scope.stdin_bytes, MAX_STDIN_BYTES);
        assert_eq!(outcome.scope.stdin_limit, MAX_STDIN_BYTES);
    }

    #[test]
    fn home_and_tmp_are_redirected_and_inventoried() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(
            tmp.path(),
            "scope.sh",
            "printf '%s\\n%s\\n' \"$HOME\" \"$TMPDIR\"\n\
             printf '{\"a\":1}' > \"$HOME/ayar.json\"\n\
             printf 'abc' > \"$TMPDIR/gecici\"",
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let outcome = run_cli(
            &request(exe, tmp.path(), &["--print"], "", Duration::from_secs(10)),
            &cancel,
        )
        .expect("çalıştı");
        let scope = &outcome.scope;
        assert_eq!(scope.home_isolation, HomeIsolation::Sandbox);
        let home = scope.home.clone().expect("HOME kum havuzunda");
        let tmpdir = scope.tmp.clone().expect("TMPDIR kum havuzunda");
        assert!(home.starts_with(&scope.workdir));
        assert!(tmpdir.starts_with(&scope.workdir));
        // Çocuğun gördüğü HOME/TMPDIR gerçekten kum havuzu.
        let printed: Vec<&str> = outcome.stdout_head.lines().collect();
        assert_eq!(printed[0], home.to_string_lossy());
        assert_eq!(printed[1], tmpdir.to_string_lossy());
        // Bıraktığı dosyalar ad + bayt olarak sayıldı, içerik okunmadı.
        let names: Vec<&str> = scope.inventory.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"home/ayar.json"), "{names:?}");
        assert!(names.contains(&"tmp/gecici"), "{names:?}");
        assert_eq!(scope.inventory_total_bytes, 7 + 3);
        assert!(!scope.inventory_truncated);
        assert!(!scope.size_limit_exceeded());
        assert_eq!(scope.workdir_mode, Some(0o700));
    }

    #[test]
    fn user_home_isolation_is_explicit_and_reported() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(tmp.path(), "home.sh", "printf '%s' \"$HOME\"");
        let cancel = Arc::new(AtomicBool::new(false));
        let mut req = request_at(
            exe,
            isolated(tmp.path()),
            &["--print"],
            "",
            Duration::from_secs(10),
            HomeIsolation::UserHome,
        );
        req.env = vec![("HOME".into(), "/tmp/kullanici-evi".into())];
        let outcome = run_cli(&req, &cancel).expect("çalıştı");
        assert_eq!(outcome.stdout_head, "/tmp/kullanici-evi");
        assert_eq!(outcome.scope.home_isolation, HomeIsolation::UserHome);
        assert_eq!(outcome.scope.home, None);
        assert!(outcome.scope.inventory.is_empty());
    }

    #[test]
    fn interactive_only_provider_is_not_run() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(tmp.path(), "cat.sh", "cat");
        let cancel = Arc::new(AtomicBool::new(false));
        let mut req = request(exe, tmp.path(), &["--print"], "", Duration::from_secs(5));
        req.adapter_id = "kimi-cli".into();
        assert_eq!(
            run_cli(&req, &cancel).expect_err("kapalı").code(),
            "E_CLI_BATCH_FORBIDDEN"
        );
    }

    #[test]
    fn large_output_is_truncated_and_flagged() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(
            tmp.path(),
            "big.sh",
            "head -c 200000 /dev/zero | tr '\\0' 'a'",
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let outcome = run_cli(
            &request(exe, tmp.path(), &["--print"], "", Duration::from_secs(20)),
            &cancel,
        )
        .expect("çalıştı");
        assert!(outcome.stdout_truncated);
        assert_eq!(outcome.stdout_head.len(), MAX_OUTPUT_BYTES);
    }

    #[test]
    fn timeout_is_clamped() {
        assert_eq!(clamp_timeout(Duration::from_millis(1)), MIN_TIMEOUT);
        assert_eq!(clamp_timeout(Duration::from_secs(10_000)), MAX_TIMEOUT);
        assert_eq!(
            clamp_timeout(Duration::from_secs(30)),
            Duration::from_secs(30)
        );
    }
}

#[cfg(test)]
mod hardening_tests {
    use super::*;

    #[test]
    fn codex_always_gets_config_and_rule_isolation() {
        let args = hardened_args("codex-cli", vec!["exec".into()]);
        assert_eq!(args.len(), 3, "{args:?}");
        assert!(args.contains(&"--ignore-user-config".to_string()));
        assert!(args.contains(&"--ignore-rules".to_string()));
    }

    #[test]
    fn isolation_flags_are_not_duplicated() {
        let args = hardened_args(
            "codex-cli",
            vec![
                "exec".into(),
                "--ignore-user-config".into(),
                "--ignore-rules".into(),
            ],
        );
        assert_eq!(args.len(), 3, "{args:?}");
    }

    #[test]
    fn claude_requires_no_session_persistence() {
        let args = hardened_args("claude-code", vec!["--print".into()]);
        assert!(
            args.contains(&"--no-session-persistence".to_string()),
            "{args:?}"
        );
    }

    #[test]
    fn claude_shuts_every_tool_off_with_an_empty_value_token() {
        let args = hardened_args("claude-code", vec!["--print".into()]);
        let at = args
            .iter()
            .position(|arg| arg == "--tools")
            .unwrap_or_else(|| panic!("--tools yok: {args:?}"));
        assert_eq!(
            args.get(at + 1).map(String::as_str),
            Some(""),
            "boş değer ayrı belirteç olmalı: {args:?}"
        );
        assert!(
            args.contains(&"--strict-mcp-config".to_string()),
            "{args:?}"
        );
        // `--tools=` tek belirteç biçimi araçları kapatmıyor (ölçüldü) → kullanılmaz.
        assert!(
            !args.iter().any(|arg| arg.starts_with("--tools=")),
            "{args:?}"
        );
    }

    #[test]
    fn hardening_replaces_a_caller_supplied_tool_list() {
        let args = hardened_args(
            "claude-code",
            vec![
                "--print".into(),
                "--tools".into(),
                "Read".into(),
                "Edit".into(),
            ],
        );
        assert_eq!(
            args.iter().filter(|arg| *arg == "--tools").count(),
            1,
            "yinelenmemeli: {args:?}"
        );
        let at = args
            .iter()
            .position(|arg| arg == "--tools")
            .expect("--tools");
        assert_eq!(args.get(at + 1).map(String::as_str), Some(""));
        assert!(!args.iter().any(|arg| arg == "Read" || arg == "Edit"));
    }

    #[test]
    fn hardening_replaces_a_caller_supplied_tools_equals_form() {
        let args = hardened_args(
            "claude-code",
            vec!["--print".into(), "--tools=Read,Edit".into()],
        );
        assert!(
            !args.iter().any(|arg| arg.starts_with("--tools=")),
            "{args:?}"
        );
        let at = args
            .iter()
            .position(|arg| arg == "--tools")
            .expect("--tools");
        assert_eq!(args.get(at + 1).map(String::as_str), Some(""));
    }

    #[test]
    fn hardening_is_idempotent() {
        for adapter in crate::SPECS {
            let once = hardened_args(adapter.id, vec!["--print".into()]);
            let twice = hardened_args(adapter.id, once.clone());
            assert_eq!(once, twice, "{}", adapter.id);
        }
    }

    #[test]
    fn antigravity_requires_sandbox() {
        let args = hardened_args("antigravity-cli", vec!["--print".into()]);
        assert!(args.contains(&"--sandbox".to_string()), "{args:?}");
    }

    #[test]
    fn antigravity_requires_slash_command_shutoff() {
        let args = hardened_args("antigravity-cli", vec!["--print".into()]);
        assert!(
            args.contains(&"--disable-slash-commands".to_string()),
            "{args:?}"
        );
    }

    /// Sicil ile sertleştirme ayrışmasın: zorunlu bayraklar izinli argüman
    /// kapısından geçebilmeli (boş değer belirteci kapıda kabul edilir).
    #[test]
    fn every_hardened_flag_passes_the_argument_gate() {
        for adapter in crate::SPECS {
            let base = adapter
                .version_args
                .first()
                .copied()
                .unwrap_or("--help")
                .to_string();
            let args = hardened_args(adapter.id, vec![base]);
            crate::build_args(adapter.id, &args)
                .unwrap_or_else(|err| panic!("{}: {err} ({args:?})", adapter.id));
        }
    }

    #[test]
    fn adapters_without_hardening_rules_are_untouched() {
        let args = hardened_args("kimi-cli", vec!["--print".into()]);
        assert_eq!(args, vec!["--print".to_string()]);
    }
}
