//! Resmî CLI adaptörlerinin **sicili** ve argüman kapısı.
//!
//! Bu crate süreç çalıştırmaz. Kimlik dosyası okumaz, token kopyalamaz
//! (INV-11). Gerçek yürütme F10'da, kullanıcının kurduğu resmî araçlar
//! üzerinden ve yalnız burada tanımlı izinli argümanlarla yapılacaktır.

use std::path::{Path, PathBuf};

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
}

impl AdapterError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownAdapter(_) => "E_CLI_UNKNOWN",
            Self::DisallowedArgument(_) => "E_CLI_ARGUMENT",
            Self::BatchForbidden => "E_CLI_BATCH_FORBIDDEN",
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
    Installed {
        path: PathBuf,
    },
    /// Dosya var ama sürüm/uyumluluk doğrulanmadı; bağlantı "sınırlı" gösterilir.
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

/// PATH üzerinde yürütülebilir dosyayı arar. Süreç çalıştırmaz, `--version`
/// çağırmaz; bu yüzden "kurulu" sonucu tek başına güvenli kabul değildir.
#[must_use]
pub fn discover(spec: &AdapterSpec) -> Discovery {
    match find_on_path(spec.command) {
        Some(path) => Discovery {
            adapter_id: spec.id,
            state: DiscoveryState::Unverified {
                path,
                reason: "version_not_probed",
            },
        },
        None => Discovery {
            adapter_id: spec.id,
            state: DiscoveryState::NotInstalled,
        },
    }
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
            DiscoveryState::NotInstalled | DiscoveryState::Unverified { .. }
        ));
    }
}
