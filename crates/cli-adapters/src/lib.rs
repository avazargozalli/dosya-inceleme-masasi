//! Resmî CLI adaptörlerinin **sicili**, argüman kapısı ve izinli yürütme.
//!
//! Kimlik dosyası okunmaz, token kopyalanmaz (INV-11). Yürütme yalnız burada
//! tanımlı izinli argümanlarla, mutlak yürütülebilir yoluyla, allowlist ortamla,
//! süre sınırı ve iptal desteğiyle yapılır (`exec` modülü).

// Süreç yalnız `--version` yoklaması ve izinli CLI yürütmesi için başlatılır;
// kimlik dosyası okunmaz, token kopyalanmaz (ADR-05, INV-11).
#![allow(clippy::disallowed_methods)]

use std::path::{Path, PathBuf};
use std::process::Command;

pub mod arch;
pub mod attack;
pub mod capability;
mod events;
pub mod exec;
pub mod hardening;
pub mod probe;
pub mod residue;
pub mod sandbox;
pub use arch::{
    app_arch_label, inspect_path, ArchComparison, ArchReport, BinaryArch, BinaryFormat,
    ScriptInterpreter, HEADER_READ_BYTES, MAX_FAT_SLICES, MAX_PE_OFFSET,
};
pub use attack::{
    guard_state, hardening_argv, outcomes, plant_attacks, run_real_attack, AttackError,
    AttackFixture, AttackObservation, AttackOutcome, AttackVector, AttackVerdict, DetectorProof,
    FixtureResult, GateCheck, GuardState, HitSource, LoaderLayout, NaiveLoad, PlantedAttacks,
    RealAttack, ATTACK_MARKER_ENV,
};
pub use capability::{
    capabilities, evaluate, help_text_of, mentions_flag, run_help_probe, AdapterCapabilities,
    Capability, CapabilityReport, CapabilityRequirement, CapabilityRow, CapabilityState, HelpProbe,
    LinkState, Mechanism, HELP_TIMEOUT, MATRIX,
};
pub use events::{final_message, parse_codex_json_lines, CodexEvent, CodexUsage, EventError};
pub use exec::{
    clamp_timeout, default_env_allowlist, ensure_prompt_shape, hardened_args, prompt_argv,
    prompt_shape, retry_while_busy, run_cli, run_cli_with_prompt, sanitize_env, splice_prompt,
    validate_prompt, CliRunRequest, PromptShape, RunError, RunOutcome, RunScope, RunStatus,
    MAX_OUTPUT_BYTES, MAX_TIMEOUT, MIN_TIMEOUT, PROMPT_MAX_BYTES, SPAWN_ATTEMPTS,
    SPAWN_RETRY_DELAY,
};
pub use hardening::{
    canary_body, classify_reply, probe_prompt, run_hardening_probe, HardeningReport, ToolAccess,
    CANARY_FILE, CANARY_TOKEN, REPLY_HEAD_CHARS,
};
pub use probe::{probe_request, run_probe, ProbeReport, PROBE_TIMEOUT};
pub use residue::{
    cleanup_created, cleanup_created_since, diff_snapshots, measure_residue, persistence_control,
    residue_paths, scan_residue, snapshot_residue, sweep_stale_sandboxes, CleanupReport,
    PersistenceControl, ResidueDelta, ResidueEntry, ResidueError, ResidueFileStamp, ResidueKind,
    ResidueMeasurement, ResiduePathSpec, ResidueScan, ResidueSnapshot, SweepReport,
    MAX_RESIDUE_FILES, SANDBOX_PREFIX, STALE_SANDBOX_AGE,
};
pub use sandbox::{
    check_workdir, HomeIsolation, RunSandbox, SandboxEntry, SandboxInventory, WorkdirCheck,
    MAX_INVENTORY_ENTRIES, MAX_SANDBOX_BYTES, MAX_STDIN_BYTES, SANDBOX_DIR_MODE,
};

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
    /// PATH'te **önce** aranan ad (bildirilen komut).
    pub command: &'static str,
    /// Kurulu ikilinin gerçek adı bildirilenden farklı olabiliyor (ölçüldü:
    /// Antigravity `agy` olarak kurulur). Eş adlar **bildirilen addan sonra**
    /// denenir; sessizce başka bir araca bağlanmamak için liste kapalı ve
    /// kısa tutulur.
    pub aliases: &'static [&'static str],
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
        aliases: &[],
        version_args: COMMON,
        allowed_args: &[
            "--print",
            "--output-format",
            "--model",
            "--max-turns",
            // `claude --help` (2.1.303) ile doğrulandı: yalnız `--print` ile.
            "--no-session-persistence",
            // F10-T11 yetenek matrisi: yardım metninde bildirilen kapatma yolları.
            // Matris ile kapı ayrışmasın diye `capability::MATRIX` ile birlikte
            // tutulur (`declared_capability_flags_pass_the_argument_gate`).
            "--tools",
            "--bare",
            "--strict-mcp-config",
            "--json-schema",
        ],
        auth_mode: AuthMode::Subscription,
        batch_forbidden: false,
    },
    AdapterSpec {
        id: "codex-cli",
        display_name: "Codex CLI",
        command: "codex",
        aliases: &[],
        version_args: COMMON,
        allowed_args: &[
            "exec",
            "--json",
            "--model",
            "--sandbox",
            "--skip-git-repo-check",
            // `codex exec --help` (0.159.2) ile doğrulandı: oturum dosyası yazmaz.
            "--ephemeral",
            // F10-T11 yetenek matrisi: yapılandırma/kural yüklemeyi kapatma ve
            // şemalı çıktı bayrakları.
            "--output-schema",
            "--ignore-user-config",
            "--ignore-rules",
        ],
        auth_mode: AuthMode::Subscription,
        batch_forbidden: false,
    },
    AdapterSpec {
        id: "antigravity-cli",
        display_name: "Antigravity CLI",
        command: "antigravity",
        // Ölçüldü (2026-11-12): Antigravity CLI 1.5.6 Homebrew'da **`agy`**
        // adıyla kurulur; ürün `antigravity` aradığı için "kurulu değil"
        // diyordu ve sertleştirme sınaması koşulamıyordu.
        aliases: &["agy"],
        version_args: COMMON,
        allowed_args: &[
            "--print",
            "--model",
            // `agy --help` (1.4.2) ile doğrulandı.
            "--output-format",
            "--json-schema",
            "--sandbox",
            // `agy --help` (1.4.7) ile doğrulandı: print kipinde slash komut ve
            // skill genişlemesini kapatır (sertleştirmede zorunlu).
            "--disable-slash-commands",
            "--print-timeout",
        ],
        auth_mode: AuthMode::Subscription,
        batch_forbidden: false,
    },
    AdapterSpec {
        id: "kimi-cli",
        display_name: "Kimi CLI",
        command: "kimi",
        aliases: &[],
        version_args: COMMON,
        allowed_args: &["--print", "--model"],
        // Kimi aboneliği etkileşimli kullanım içindir; otomatik toplu iş engellenir.
        auth_mode: AuthMode::InteractiveOnly,
        batch_forbidden: true,
    },
];

impl AdapterSpec {
    /// PATH'te aranacak adlar: **bildirilen ad önce**, sonra eş adlar. Sıra
    /// sözleşmedir: kurulu iki ad varsa bildirilen ad kazanır (sessizce eş ada
    /// kayma olmaz).
    pub fn command_candidates(&self) -> impl Iterator<Item = &'static str> + '_ {
        std::iter::once(self.command).chain(self.aliases.iter().copied())
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AdapterError {
    #[error("bilinmeyen adaptör: {0}")]
    UnknownAdapter(String),
    #[error("izin verilmeyen argüman: {0}")]
    DisallowedArgument(String),
    #[error("toplu/otomatik kullanım bu sağlayıcıda engelli")]
    BatchForbidden,
    /// İstem metni argv'ye **güvenle** konamaz (boş, `-` ile başlıyor, kontrol
    /// karakteri taşıyor ya da sınırı aşıyor). İstem ürün tarafından üretilir;
    /// bayrak sanılacak bir metin alt sürece istem olarak verilmez.
    #[error("geçersiz istem: {0}")]
    InvalidPrompt(String),
    /// `--print` çıplak kaldı ve yanındaki belirteç bir bayrak: sağlayıcı onu
    /// **istem** sanar (ölçüldü, `agy`), yani sertleştirme bayrağı sessizce
    /// isteme dönüşür. Kapı bunu yürütmeden önce reddeder.
    #[error("istem biçimi geçersiz: {0}")]
    PromptShape(String),
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
            Self::InvalidPrompt(_) => "E_CLI_ARGUMENT",
            Self::PromptShape(_) => "E_CLI_ARGUMENT",
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
    let Some((_, path)) = resolve_adapter_executable(spec) else {
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
    let mut command = Command::new(path);
    command.arg(flag);
    // Sürüm yoklaması da tam ortamı devralmaz (INV-11): `run_cli` ile aynı
    // allowlist. Böylece liste dışı bir API anahtarı alt sürece geçmez.
    command.env_clear();
    for (key, value) in default_env_allowlist() {
        command.env(key, value);
    }
    let output = command.output().map_err(|_| AdapterError::ProbeFailed)?;
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

/// PATH üzerinde **tek** bir komut adı arar (yorumlayıcı çözümü gibi tek adlı
/// aramalar için). Adaptör yürütülebiliri için [`resolve_adapter_executable`]
/// kullan: eş adlar orada denenir.
#[must_use]
pub fn find_on_path(command: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| find_in_dir(&dir, command))
}

/// Verilen dizinde komut adını arar; Windows'ta yürütülebilir uzantılarını da
/// dener.
fn find_in_dir(dir: &Path, command: &str) -> Option<PathBuf> {
    // Boş ad ya da yol ayırıcı taşıyan ad (`a/b`) dosya adı olarak kullanılmaz:
    // eş ad listesi sabittir, yine de kapı burada tutulur.
    if command.is_empty() || command.contains('/') || command.contains('\\') {
        return None;
    }
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
    None
}

/// Adaptörün yürütülebilirini PATH üzerinde arar: **bildirilen ad önce**
/// (`spec.command`), sonra eş adlar (`spec.aliases`). Dönen ad, hangi adın
/// bulunduğunu söyler — arayüz "eş ad: agy" yazabilsin diye.
#[must_use]
pub fn resolve_adapter_executable(spec: &AdapterSpec) -> Option<(&'static str, PathBuf)> {
    let dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    resolve_in_dirs(spec, &dirs)
}

/// [`resolve_adapter_executable`]in test edilebilir çekirdeği: PATH yerine
/// verilen dizinlerde arar (ortam değişkenini değiştirmeden ölçülür).
#[must_use]
pub fn resolve_in_dirs(spec: &AdapterSpec, dirs: &[PathBuf]) -> Option<(&'static str, PathBuf)> {
    spec.command_candidates().find_map(|name| {
        dirs.iter()
            .find_map(|dir| find_in_dir(dir, name).map(|path| (name, path)))
    })
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
/// listede yoksa istek reddedilir (INV-10). Sicilde tanımlı **sürüm bayrakları**
/// (`version_args`) ayrıca izinlidir: yoklama kendi sicilindeki bayrağı
/// kullanabildiği hâlde kapıya takılmamalı.
///
/// Boş dize tek başına anlamlı bir argüman değildir; yalnız bir bayrağın boş
/// değeri olarak kullanılır (`--tools ""`). İçerik taşımadığı için kapıyı
/// gevşetmez: serbest metin (istem, yol, komut) hâlâ reddedilir.
pub fn build_args(adapter_id: &str, extra: &[String]) -> Result<Vec<String>, AdapterError> {
    let spec =
        spec(adapter_id).ok_or_else(|| AdapterError::UnknownAdapter(adapter_id.to_string()))?;
    let mut out = Vec::with_capacity(extra.len());
    for arg in extra {
        if arg.is_empty() {
            out.push(arg.clone());
            continue;
        }
        let flag = arg.split('=').next().unwrap_or(arg);
        if !spec.allowed_args.contains(&flag) && !spec.version_args.contains(&flag) {
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

    fn temp_dir(tag: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("dim-alias-{}-{stamp}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dizin");
        dir
    }

    fn touch_executable(dir: &Path, name: &str) {
        let path = dir.join(name);
        std::fs::write(&path, b"#!/bin/sh\nexit 0\n").expect("yaz");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).expect("meta").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).expect("izin");
        }
    }

    /// Ölçülen kusur (2026-11-12): Antigravity CLI bu makinede **`agy`** adıyla
    /// kurulu, ürün `antigravity` arıyordu → "kurulu değil". Eş ad çözümü,
    /// bildirilen ad yoksa eş adı dener.
    #[test]
    fn alias_is_resolved_when_declared_command_is_absent() {
        let dir = temp_dir("alias");
        touch_executable(&dir, "agy");
        let adapter = spec("antigravity-cli").expect("spec");
        let (name, path) = resolve_in_dirs(adapter, std::slice::from_ref(&dir)).expect("eş ad bulunmalı");
        assert_eq!(name, "agy");
        assert_eq!(path, dir.join("agy"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Sıra sözleşmedir: iki ad da kuruluysa **bildirilen** ad kazanır.
    #[test]
    fn declared_command_wins_over_alias() {
        let dir = temp_dir("declared");
        touch_executable(&dir, "agy");
        touch_executable(&dir, "antigravity");
        let adapter = spec("antigravity-cli").expect("spec");
        let (name, path) = resolve_in_dirs(adapter, std::slice::from_ref(&dir)).expect("bildirilen ad");
        assert_eq!(name, "antigravity");
        assert_eq!(path, dir.join("antigravity"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Eş ad listesi **adaptöre özgüdür**: başka adaptör `agy` adını aramaz.
    #[test]
    fn aliases_do_not_leak_to_other_adapters() {
        let dir = temp_dir("leak");
        touch_executable(&dir, "agy");
        let codex = spec("codex-cli").expect("spec");
        assert!(resolve_in_dirs(codex, std::slice::from_ref(&dir)).is_none());
        assert_eq!(
            spec("antigravity-cli")
                .expect("spec")
                .command_candidates()
                .collect::<Vec<_>>(),
            vec!["antigravity", "agy"]
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Ad listesi kapalı: yol ayırıcı taşıyan ya da boş ad dosya adı olarak
    /// aranmaz. Üst dizinde gerçekten bir `agy` varken `../agy` **bulunmamalı**;
    /// yoksa ad listesi PATH dışına çözülür.
    #[test]
    fn only_plain_names_are_searched() {
        let dir = temp_dir("plain");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).expect("alt dizin");
        touch_executable(&dir, "agy");
        assert!(find_in_dir(&sub, "../agy").is_none());
        assert!(find_in_dir(&sub, "agy").is_none());
        assert!(find_in_dir(&dir, "agy").is_some());
        assert!(find_in_dir(&dir, "").is_none());
        std::fs::remove_dir_all(&dir).ok();
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
    fn version_flags_pass_the_argument_gate() {
        // Arayüzdeki "Bağlantıyı sına" düğmesi tam olarak bunu çalıştırır:
        // sicildeki sürüm bayrağı kapıya takılmamalı.
        let args = build_args("claude-code", &["--version".into()]).expect("izinli");
        assert_eq!(args, vec!["--version".to_string()]);
        assert!(build_args("kimi-cli", &["--help".into()]).is_ok());
        // Bilinmeyen bayrak hâlâ reddedilir.
        assert_eq!(
            build_args("claude-code", &["--yolo".into()])
                .unwrap_err()
                .code(),
            "E_CLI_ARGUMENT"
        );
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
