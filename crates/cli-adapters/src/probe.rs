//! Arayüzdeki "Bağlantıyı sına" düğmesinin arkasındaki **gerçek** yol
//! (F10-T05/T12/T14).
//!
//! Sürüm yoklaması yürütme çekirdeğinden geçer: argv yalnız `version_args`,
//! ortam varsayılan allowlist, iş dizini **boş** geçici dizin (proje talimatı
//! okunmaz), süre sınırı ve iptal bayrağı. Böylece arayüz düğmesi, testler ve
//! raporlanan kanıt aynı kodu çalıştırır.
//!
//! Sınır: bu bir sürüm yoklamasıdır. İstem gönderilmez, kimlik dosyası
//! okunmaz, model çağrılmaz; arayüzden serbest argüman alınmaz.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::{default_env_allowlist, run_cli, AdapterSpec, CliRunRequest, RunError, RunOutcome};

/// Arayüzün istediği sınama süresi; çekirdek yine 1–600 sn aralığına kırpar.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

static WORKDIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Yürütmenin çalıştığı **boş** geçici dizin. Proje talimatı (`AGENTS.md`,
/// `CLAUDE.md`), repo içeriği veya kullanıcı dosyası burada bulunmaz; bırakılınca
/// dizin silinir.
#[derive(Debug)]
pub struct ProbeWorkdir {
    path: PathBuf,
}

impl ProbeWorkdir {
    pub fn create() -> Result<Self, RunError> {
        let base = std::env::temp_dir();
        for _ in 0..8 {
            let n = WORKDIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let candidate = base.join(format!("dim-cli-probe-{}-{n}", std::process::id()));
            match std::fs::create_dir(&candidate) {
                Ok(()) => {
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
            "boş dizin adı bulunamadı".to_string(),
        ))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ProbeWorkdir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Arayüze raporlanan sınama sonucu: **hangi** argv ile, **hangi** ortam
/// anahtarlarıyla, nerede ve ne kadar süreyle çalıştığı.
#[derive(Debug, Clone)]
pub struct ProbeReport {
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub env_keys: Vec<String>,
    pub workdir: PathBuf,
    pub timeout: Duration,
    pub outcome: RunOutcome,
}

/// Sürüm yoklaması isteği. `args` **yalnız** `spec.version_args`; arayüzden
/// serbest argüman alınmadığı için izinli argüman kapısı atlanamaz.
#[must_use]
pub fn probe_request(
    spec: &AdapterSpec,
    executable: &Path,
    workdir: &Path,
    timeout: Duration,
) -> CliRunRequest {
    CliRunRequest {
        adapter_id: spec.id.to_string(),
        executable: executable.to_path_buf(),
        args: spec
            .version_args
            .first()
            .map(|flag| vec![(*flag).to_string()])
            .unwrap_or_default(),
        stdin_text: String::new(),
        workdir: workdir.to_path_buf(),
        timeout,
        env: default_env_allowlist(),
    }
}

/// Boş geçici iş dizininde sürüm yoklamasını yürütür ve dizini siler.
pub fn run_probe(
    spec: &AdapterSpec,
    executable: PathBuf,
    cancel: &Arc<AtomicBool>,
) -> Result<ProbeReport, RunError> {
    let workdir = ProbeWorkdir::create()?;
    let request = probe_request(spec, &executable, workdir.path(), PROBE_TIMEOUT);
    let env_keys: Vec<String> = request.env.iter().map(|(key, _)| key.clone()).collect();
    let outcome = run_cli(&request, cancel)?;
    Ok(ProbeReport {
        executable,
        args: request.args,
        env_keys,
        workdir: workdir.path().to_path_buf(),
        timeout: request.timeout,
        outcome,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{spec, RunStatus};

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
        path
    }

    fn temp_root() -> PathBuf {
        std::env::temp_dir()
            .canonicalize()
            .unwrap_or_else(|_| std::env::temp_dir())
    }

    #[test]
    fn probe_workdir_is_empty_isolated_and_removed() {
        let dir = ProbeWorkdir::create().expect("dizin");
        let path = dir.path().to_path_buf();
        assert!(path.is_dir());
        assert_eq!(std::fs::read_dir(&path).expect("oku").count(), 0);
        let name = path.file_name().expect("ad").to_string_lossy().to_string();
        assert!(name.starts_with("dim-cli-probe-"), "{name}");
        assert!(path.starts_with(temp_root()), "{}", path.display());
        // Repo içinde değil: proje talimatı (AGENTS.md/CLAUDE.md) görünmez.
        assert!(!path.to_string_lossy().contains("dosya-inceleme-masasi"));
        drop(dir);
        assert!(!path.exists(), "dizin bırakılınca silinmeli");
    }

    #[test]
    fn probe_request_uses_only_version_args_and_safe_env() {
        let adapter = spec("claude-code").expect("spec");
        let request = probe_request(
            adapter,
            Path::new("/usr/bin/true"),
            Path::new("/tmp"),
            PROBE_TIMEOUT,
        );
        assert_eq!(request.args, vec!["--version".to_string()]);
        assert!(request.stdin_text.is_empty());
        let upper: Vec<String> = request
            .env
            .iter()
            .map(|(k, _)| k.to_ascii_uppercase())
            .collect();
        for key in &upper {
            for needle in [
                "TOKEN", "SECRET", "API_KEY", "PASSWORD", "COOKIE", "SESSION",
            ] {
                assert!(!key.contains(needle), "{key}");
            }
        }
    }

    #[test]
    fn probe_runs_real_process_in_isolated_dir() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(tmp.path(), "claude", "echo 'claude 1.2.3'\npwd");
        let adapter = spec("claude-code").expect("spec");
        let cancel = Arc::new(AtomicBool::new(false));
        let report = run_probe(adapter, exe, &cancel).expect("koştu");
        assert_eq!(report.outcome.status, RunStatus::Exited(0));
        assert!(report.outcome.stdout_head.contains("claude 1.2.3"));
        assert_eq!(report.args, vec!["--version".to_string()]);
        // Alt süreç boş geçici dizinde çalıştı.
        let printed = report
            .outcome
            .stdout_head
            .lines()
            .nth(1)
            .unwrap_or("")
            .trim();
        assert_eq!(printed, report.workdir.to_string_lossy());
        // Rapor döndükten sonra dizin silinmiş olur.
        assert!(!report.workdir.exists());
    }

    #[test]
    fn cancelled_probe_reports_cancelled_without_waiting() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(tmp.path(), "slow", "sleep 10");
        let adapter = spec("claude-code").expect("spec");
        let cancel = Arc::new(AtomicBool::new(true));
        let started = std::time::Instant::now();
        let report = run_probe(adapter, exe, &cancel).expect("iptal");
        assert_eq!(report.outcome.status, RunStatus::Cancelled);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn interactive_only_adapter_never_runs() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(tmp.path(), "kimi", "echo kimi");
        let adapter = spec("kimi-cli").expect("spec");
        let cancel = Arc::new(AtomicBool::new(false));
        let err = run_probe(adapter, exe, &cancel).expect_err("kapalı");
        assert_eq!(err.code(), "E_CLI_BATCH_FORBIDDEN");
    }
}
