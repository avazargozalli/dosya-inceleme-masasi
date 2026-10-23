//! Resmî CLI adaptörlerinin **sicili** ve argüman kapısı.
//!
//! Bu crate süreç çalıştırmaz. Kimlik dosyası okumaz, token kopyalamaz
//! (INV-11). Gerçek yürütme F10'da, kullanıcının kurduğu resmî araçlar
//! üzerinden ve yalnız burada tanımlı izinli argümanlarla yapılacaktır.

// Süreç yalnız `--version` yoklaması için ve yalnız burada başlatılır; kimlik
// dosyası okunmaz, token kopyalanmaz (ADR-05, INV-11).
#![allow(clippy::disallowed_methods)]

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// Kullanıcının aboneliği; CLI kendi oturumunu yönetir.
    Subscription,
    /// Kullanıcının API anahtarı; yalnız OS secret ref tutulur.
    ApiKey,
    /// Etkileşimli kullanım; otomatik toplu işe bağlanmaz.
    InteractiveOnly,
}

#[derive(Debug, Clone)]
pub struct AdapterSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub command: &'static str,
    pub version_args: &'static [&'static str],
    /// Yürütmede kabul edilen ek bayraklar. Listede olmayan bayrak reddedilir.
    pub allowed_args: &'static [&'static str],
    pub auth_mode: AuthMode,
    /// Abonelik modunda otomatik/toplu kullanımın yasak olduğu sağlayıcılar.
    pub batch_forbidden: bool,
}

const COMMON: &[&str] = &["--version", "--help"];

/// Dört zorunlu adaptör. Sürüm aralıkları F10 uyumluluk sicilinde doğrulanır;
/// burada tahmin edilmiş sürüm numarası yazılmaz.
pub const SPECS: &[AdapterSpec] = &[
    AdapterSpec {
        id: "claude-code",
        display_name: "Claude Code",
        command: "claude",
        version_args: COMMON,
        allowed_args: &["--print", "--output-format", "--model", "--max-turns"],
        auth_mode: AuthMode::Subscription,
        batch_forbidden: false,
    },
    AdapterSpec {
        id: "codex-cli",
        display_name: "Codex CLI",
        command: "codex",
        version_args: COMMON,
        allowed_args: &[
            "exec",
            "--json",
            "--model",
            "--sandbox",
            "--skip-git-repo-check",
        ],
        auth_mode: AuthMode::Subscription,
        batch_forbidden: false,
    },
    AdapterSpec {
        id: "antigravity-cli",
        display_name: "Antigravity CLI",
        command: "antigravity",
        version_args: COMMON,
        allowed_args: &["--print", "--model"],
        auth_mode: AuthMode::Subscription,
        batch_forbidden: false,
    },
    AdapterSpec {
        id: "kimi-cli",
        display_name: "Kimi CLI",
        command: "kimi",
        version_args: COMMON,
        allowed_args: &["--print", "--model"],
        // Kimi aboneliği etkileşimli kullanım içindir; otomatik toplu iş engellenir.
        auth_mode: AuthMode::InteractiveOnly,
        batch_forbidden: true,
    },
];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AdapterError {
    #[error("bilinmeyen adaptör: {0}")]
    UnknownAdapter(String),
    #[error("izin verilmeyen argüman: {0}")]
    DisallowedArgument(String),
    #[error("toplu/otomatik kullanım bu sağlayıcıda engelli")]
    BatchForbidden,
    #[error("araç kurulu değil")]
    NotInstalled,
    #[error("sürüm yoklaması başarısız")]
    ProbeFailed,
}

impl AdapterError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownAdapter(_) => "E_CLI_UNKNOWN",
            Self::DisallowedArgument(_) => "E_CLI_ARGUMENT",
            Self::BatchForbidden => "E_CLI_BATCH_FORBIDDEN",
            Self::NotInstalled => "E_CLI_NOT_INSTALLED",
            Self::ProbeFailed => "E_CLI_PROBE",
        }
    }
}

#[must_use]
pub fn spec(id: &str) -> Option<&'static AdapterSpec> {
    SPECS.iter().find(|s| s.id == id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryState {
    NotInstalled,
    /// Yürütülebilir bulundu ve `--version` yanıt verdi. Yetenek/izin kapıları
    /// ayrıca geçilmeden bağlantı "hazır" sayılmaz.
    Installed {
        path: PathBuf,
        version: String,
    },
    /// Dosya var ama sürüm okunamadı; bağlantı "sınırlı" gösterilir.
    Unverified {
        path: PathBuf,
        reason: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    pub adapter_id: &'static str,
    pub state: DiscoveryState,
}

/// PATH üzerinde aracı arar ve **yalnız** `--version` çalıştırır. Sürüm
/// okunamazsa "kurulu" denmez; `Unverified` döner.
#[must_use]
pub fn discover(spec: &AdapterSpec) -> Discovery {
    let Some(path) = find_on_path(spec.command) else {
        return Discovery {
            adapter_id: spec.id,
            state: DiscoveryState::NotInstalled,
        };
    };
    match probe_version(&path, spec) {
        Ok(version) => Discovery {
            adapter_id: spec.id,
            state: DiscoveryState::Installed { path, version },
        },
        Err(_) => Discovery {
            adapter_id: spec.id,
            state: DiscoveryState::Unverified {
                path,
                reason: "version_probe_failed",
            },
        },
    }
}

/// `--version` çıktısından sürümü ayıklar; kontrol karakterleri temizlenir ve
/// ilk 80 karakterle sınırlanır.
#[must_use]
pub fn parse_version_output(output: &str) -> Option<String> {
    let line = output.lines().map(str::trim).find(|l| !l.is_empty())?;
    let cleaned: String = line.chars().filter(|c| !c.is_control()).take(80).collect();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn probe_version(path: &Path, spec: &AdapterSpec) -> Result<String, AdapterError> {
    let flag = spec.version_args.first().copied().unwrap_or("--version");
    let output = Command::new(path)
        .arg(flag)
        .output()
        .map_err(|_| AdapterError::ProbeFailed)?;
    if !output.status.success() {
        return Err(AdapterError::ProbeFailed);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = if stdout.trim().is_empty() {
        String::from_utf8_lossy(&output.stderr).to_string()
    } else {
        stdout.to_string()
    };
    parse_version_output(&text).ok_or(AdapterError::ProbeFailed)
}

#[must_use]
pub fn find_on_path(command: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(command);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            for ext in ["exe", "cmd", "bat"] {
                let c = dir.join(format!("{command}.{ext}"));
                if is_executable(&c) {
                    return Some(c);
                }
            }
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
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

/// İzinli argüman kapısı. `--flag=value` biçimi de kabul edilir; bayrak adı
/// listede yoksa istek reddedilir (INV-10).
pub fn build_args(adapter_id: &str, extra: &[String]) -> Result<Vec<String>, AdapterError> {
    let spec =
        spec(adapter_id).ok_or_else(|| AdapterError::UnknownAdapter(adapter_id.to_string()))?;
    let mut out = Vec::with_capacity(extra.len());
    for arg in extra {
        let flag = arg.split('=').next().unwrap_or(arg);
        if !spec.allowed_args.contains(&flag) {
            return Err(AdapterError::DisallowedArgument(flag.to_string()));
        }
        out.push(arg.clone());
    }
    Ok(out)
}

/// Toplu iş kapısı: etkileşimli-only sağlayıcıda batch çağrısı yapılamaz.
pub fn ensure_batch_allowed(adapter_id: &str) -> Result<(), AdapterError> {
    let spec =
        spec(adapter_id).ok_or_else(|| AdapterError::UnknownAdapter(adapter_id.to_string()))?;
    if spec.batch_forbidden && spec.auth_mode == AuthMode::InteractiveOnly {
        return Err(AdapterError::BatchForbidden);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_required_adapters_are_registered() {
        assert_eq!(SPECS.len(), 4);
        for id in ["claude-code", "codex-cli", "antigravity-cli", "kimi-cli"] {
            assert!(spec(id).is_some(), "{id}");
        }
    }

    #[test]
    fn disallowed_flag_is_rejected() {
        let err =
            build_args("claude-code", &["--dangerously-skip-permissions".into()]).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
    }

    #[test]
    fn allowed_flag_with_value_is_accepted() {
        let args =
            build_args("codex-cli", &["--model=gpt-x".into(), "--json".into()]).expect("izinli");
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn unknown_adapter_rejected() {
        assert_eq!(build_args("yok", &[]).unwrap_err().code(), "E_CLI_UNKNOWN");
    }

    #[test]
    fn kimi_batch_is_blocked() {
        assert_eq!(
            ensure_batch_allowed("kimi-cli").unwrap_err().code(),
            "E_CLI_BATCH_FORBIDDEN"
        );
        assert!(ensure_batch_allowed("claude-code").is_ok());
    }

    #[test]
    fn discovery_never_claims_installed_without_probe() {
        let spec = spec("kimi-cli").expect("spec");
        let d = discover(spec);
        assert!(matches!(
            d.state,
            DiscoveryState::NotInstalled
                | DiscoveryState::Installed { .. }
                | DiscoveryState::Unverified { .. }
        ));
        if let DiscoveryState::Installed { version, .. } = d.state {
            assert!(!version.is_empty());
        }
    }

    #[test]
    fn version_output_is_cleaned_and_truncated() {
        assert_eq!(
            parse_version_output("  codex-cli 0.9.1\n").as_deref(),
            Some("codex-cli 0.9.1")
        );
        assert_eq!(parse_version_output("\n\n"), None);
        let noisy = format!("v{}\u{0007}", "1".repeat(200));
        let parsed = parse_version_output(&noisy).expect("sürüm");
        assert!(!parsed.contains('\u{0007}'));
        assert!(parsed.chars().count() <= 80);
    }
}
