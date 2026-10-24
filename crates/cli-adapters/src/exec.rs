//! İzinli argümanlarla **gerçek** CLI yürütme (F10-T04/T05/T07/T14).
//!
//! Kurallar:
//! - Yürütülebilir **mutlak** yol olmalı ve gerçekten çalıştırılabilir olmalı;
//!   shell string birleştirme yok, argümanlar `build_args` kapısından geçer.
//! - Ortam **allowlist** ile kurulur; `Command::env_clear()` sonrası yalnız
//!   verilen değişkenler aktarılır. Anahtar/token/cookie gibi görünen adlar
//!   allowlist'e yazılsa bile reddedilir.
//! - İş dizini var olan bir dizin olmalı; stdin metin olarak verilir.
//! - Süre sınırı ve iptal bayrağı birlikte uygulanır; süre dolunca/iptalde alt
//!   süreç **öldürülür**, kısmi çıktı raporlanır.
//! - Çıktı en çok 64 KiB baş kısmıyla döner; kesildiği ayrıca işaretlenir.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
}

/// Varsayılan ortam allowlist'i: yalnız süreç çalışması için gerekenler.
/// API anahtarı, token, cookie **yok**.
#[must_use]
pub fn default_env_allowlist() -> Vec<(String, String)> {
    ["PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "TZ"]
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
    if !req.executable.is_absolute() {
        return Err(RunError::RelativeExecutable);
    }
    if !executable_ok(&req.executable) {
        return Err(RunError::ExecutableUnavailable);
    }
    if !req.workdir.is_dir() {
        return Err(RunError::InvalidWorkdir);
    }
    let env = sanitize_env(&req.env)?;
    let timeout = clamp_timeout(req.timeout);

    let mut command = Command::new(&req.executable);
    command
        .args(&args)
        .current_dir(&req.workdir)
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
    let stdin_payload = req.stdin_text.clone();
    let mut stdin_handle = child.stdin.take();
    let writer = std::thread::spawn(move || {
        if let Some(mut pipe) = stdin_handle.take() {
            let _ = pipe.write_all(stdin_payload.as_bytes());
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

    Ok(RunOutcome {
        status: status.unwrap_or(RunStatus::Signaled),
        stdout_head,
        stderr_head,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        stdout_truncated,
        stderr_truncated,
    })
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

    fn request(
        exe: PathBuf,
        dir: &Path,
        args: &[&str],
        stdin: &str,
        timeout: Duration,
    ) -> CliRunRequest {
        CliRunRequest {
            adapter_id: "claude-code".into(),
            executable: exe,
            args: args.iter().map(|a| (*a).to_string()).collect(),
            stdin_text: stdin.into(),
            workdir: dir.to_path_buf(),
            timeout,
            env: vec![("DIM_TEST_MARK".into(), "evet".into())],
        }
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
        // çalışan süreçtir (torun değil).
        let exe = script(tmp.path(), "slow.sh", "echo başladı\nexec sleep 5");
        let cancel = Arc::new(AtomicBool::new(false));
        let outcome = run_cli(
            &request(
                exe,
                tmp.path(),
                &["--print"],
                "",
                Duration::from_millis(400),
            ),
            &cancel,
        )
        .expect("çalıştı");
        assert_eq!(outcome.status, RunStatus::TimedOut);
        assert!(outcome.stdout_head.contains("başladı"));
        // Öldürme çalışmasaydı `wait` 5 sn sürerdi.
        assert!(outcome.duration_ms < 3000, "süre: {}", outcome.duration_ms);
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
        req = request(
            exe,
            &tmp.path().join("yok"),
            &[],
            "",
            Duration::from_secs(5),
        );
        assert_eq!(
            run_cli(&req, &cancel).expect_err("dizin").code(),
            "E_CLI_EXEC_WORKDIR"
        );
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
