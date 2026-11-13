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
    capabilities, evaluate, evaluate_with_evidence, evaluate_with_measurement, help_text_of,
    mentions_flag, run_help_probe, AdapterCapabilities, Capability, CapabilityReport,
    CapabilityRequirement, CapabilityRow, CapabilityState, HelpProbe, LinkState, MeasuredAccess,
    MeasuredEvidence, Mechanism, EVIDENCE_UNREADABLE, HELP_TIMEOUT, MATRIX,
};
pub use events::{final_message, parse_codex_json_lines, CodexEvent, CodexUsage, EventError};
pub use exec::{
    clamp_timeout, default_env_allowlist, ensure_prompt_shape, hardened_args, prompt_argv,
    prompt_shape, prompt_shape_with_evidence, prompt_shape_with_measurement, retry_while_busy,
    run_cli, run_cli_with_prompt, sanitize_env, splice_prompt, validate_prompt, CliRunRequest,
    PromptShape, RunError, RunOutcome, RunScope, RunStatus, MAX_OUTPUT_BYTES, MAX_TIMEOUT,
    MIN_TIMEOUT, PROMPT_MAX_BYTES, PROMPT_PLACEHOLDER, SPAWN_ATTEMPTS, SPAWN_RETRY_DELAY,
};
pub use hardening::{
    canary_body, classify_not_run, classify_reply, probe_prompt, run_hardening_probe,
    HardeningReport, NotRun, PrintEnvelope, ToolAccess, CANARY_FILE, CANARY_TOKEN,
    JSON_PRINT_ADAPTERS, NOT_RUN_REASON_CHARS, REPLY_HEAD_CHARS,
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

/// İzinli bir bayrağın **değer** politikası. Argüman kapısı yalnız bayrak adına
/// bakmaz: değeri bir güvenlik/format kararı taşıyan bayraklar kapalı küme
/// bildirir, geri kalanı gerekçesiyle `Free` işaretlenir.
///
/// **Neden:** kapı yalnız adı denetlediğinde "izinli argüman" güvenlik kararını
/// taşımıyordu. Ölçüldü (codex-cli 0.159.2 `--help`): `--sandbox` için olası
/// değerler `read-only · workspace-write · danger-full-access`; adı izinli
/// olduğu için `--sandbox=danger-full-access` kapıdan **geçiyordu**. Aynı sınıf:
/// claude `--tools` (2.1.303 `--help`: `""` tüm araçları kapatır, `default`
/// hepsini açar) — `--tools=default` adı izinli olduğu için geçerdi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgValue {
    /// Bayrak değer **almaz**: hem `--bayrak=değer` hem `--bayrak değer`
    /// reddedilir (değer sessizce yorumlanmasın).
    None,
    /// Yalnız bu değerler kabul edilir. Boş dize de bir değerdir (`--tools ""`).
    OneOf(&'static [&'static str]),
    /// Değer serbest: bayrak adı denetimi yeterli, değer güvenlik/format kapısı
    /// taşımıyor. Gerekçe **zorunlu** alan — kararı yazmadan bayrak eklenemez.
    Free(&'static str),
}

impl ArgValue {
    /// Bayrak değer alıyor mu? (Ayrı belirteçli biçimi kabul eden tek durum.)
    #[must_use]
    pub const fn takes_value(&self) -> bool {
        !matches!(self, Self::None)
    }
}

/// `allowed_args` girişi: bayrak **ve** değer kararı tek satırda durur. Kararı
/// atlamak mümkün değil — yeni bayrak ekleyen tip sistemi karar vermeye zorlar.
#[derive(Debug, Clone, Copy)]
pub struct AllowedArg {
    pub flag: &'static str,
    pub value: ArgValue,
}

/// Değer almayan (boolean/alt komut) bayrak.
const fn plain(flag: &'static str) -> AllowedArg {
    AllowedArg {
        flag,
        value: ArgValue::None,
    }
}

/// Değeri **kapalı küme** olan bayrak: güvenlik/format kararı taşır.
const fn closed(flag: &'static str, values: &'static [&'static str]) -> AllowedArg {
    AllowedArg {
        flag,
        value: ArgValue::OneOf(values),
    }
}

/// Değeri serbest ama gerekçesi yazılmış bayrak.
const fn free(flag: &'static str, why: &'static str) -> AllowedArg {
    AllowedArg {
        flag,
        value: ArgValue::Free(why),
    }
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
    /// Yürütmede kabul edilen ek bayraklar **ve değer kararları**. Listede
    /// olmayan bayrak, izinli olmayan değer reddedilir.
    pub allowed_args: &'static [AllowedArg],
    pub auth_mode: AuthMode,
    /// Abonelik modunda otomatik/toplu kullanımın yasak olduğu sağlayıcılar.
    pub batch_forbidden: bool,
}

impl AdapterSpec {
    /// Bayrağın değer kararı. Ortak sürüm bayrakları (`--version`, `--help`)
    /// değer almaz. Listede olmayan bayrak `None` döner → kapı reddeder.
    #[must_use]
    pub fn arg_rule(&self, flag: &str) -> Option<ArgValue> {
        if let Some(entry) = self.allowed_args.iter().find(|entry| entry.flag == flag) {
            return Some(entry.value);
        }
        self.version_args.contains(&flag).then_some(ArgValue::None)
    }

    /// İzinli bayrak **adları** (arayüz/sicil görünümü; değer kararı taşımaz).
    pub fn allowed_flags(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.allowed_args.iter().map(|entry| entry.flag)
    }
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
            plain("--print"),
            // Ürün düz metin okur; `stream-json` ayrıştırıcısı yok.
            closed("--output-format", &["text", "json"]),
            free("--model", "model adı; güvenlik/format denetimi taşımaz"),
            free(
                "--max-turns",
                "sayı; ürün tur sınırını ayrıca timeout ile koyar",
            ),
            // `claude --help` (2.1.303) ile doğrulandı: yalnız `--print` ile.
            plain("--no-session-persistence"),
            // F10-T11 yetenek matrisi: yardım metninde bildirilen kapatma yolları.
            // Matris ile kapı ayrışmasın diye `capability::MATRIX` ile birlikte
            // tutulur (`declared_capability_flags_pass_the_argument_gate`).
            // ÖLÇÜLDÜ (2.1.303 `--help`): değer `""` **tüm** araçları kapatır,
            // `default` hepsini açar, araç adları seçer. Ürün yalnız kapatmayı
            // kullanır; kapıdan başka değer geçemez (`--tools=default` reddedilir).
            closed("--tools", &[""]),
            // `--bare` oturumu düşürüyor (ölçüldü: "Not logged in") — boolean.
            plain("--bare"),
            plain("--strict-mcp-config"),
            free(
                "--json-schema",
                "satır içi JSON şema; ürün şemayı kendi üretir",
            ),
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
            plain("exec"),
            plain("--json"),
            free("--model", "model adı; güvenlik/format denetimi taşımaz"),
            // ÖLÇÜLDÜ (codex-cli 0.159.2 `exec --help`): olası değerler
            // `read-only · workspace-write · danger-full-access`. Ürün **tam
            // erişim istemez**; adı izinli diye `danger-full-access` geçemez.
            closed("--sandbox", &["read-only", "workspace-write"]),
            plain("--skip-git-repo-check"),
            // `codex exec --help` (0.159.2) ile doğrulandı: oturum dosyası yazmaz.
            plain("--ephemeral"),
            // F10-T11 yetenek matrisi: yapılandırma/kural yüklemeyi kapatma ve
            // şemalı çıktı bayrakları.
            free("--output-schema", "JSON şema **dosyası** yolu"),
            plain("--ignore-user-config"),
            plain("--ignore-rules"),
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
            // `--print` **değer alır** (istem `=` ile bağlanır) — ama istem
            // yalnız `prompt_argv`den, `validate_prompt` sonrası girer. Argüman
            // kapısından istem geçemez; bu yüzden burada değer kabul edilmez.
            plain("--print"),
            free("--model", "model adı; güvenlik/format denetimi taşımaz"),
            // `agy --help` (1.4.2) ile doğrulandı. Ürün yalnız `json` zarfını
            // okur (T08e): başka biçim ayrıştırıcıyı kırar.
            closed("--output-format", &["json"]),
            free(
                "--json-schema",
                "satır içi JSON şema; ürün şemayı kendi üretir",
            ),
            plain("--sandbox"),
            // `agy --help` (1.4.7) ile doğrulandı: print kipinde slash komut ve
            // skill genişlemesini kapatır (sertleştirmede zorunlu).
            plain("--disable-slash-commands"),
            free("--print-timeout", "süre; biçim sağlayıcıya bağlı"),
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
        allowed_args: &[
            plain("--print"),
            free("--model", "model adı; güvenlik/format denetimi taşımaz"),
        ],
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
    /// Bayrak izinli ama **değeri** değil. Ad denetimi tek başına yetmiyordu:
    /// `--sandbox=danger-full-access` gibi bir değer sertleştirmeyi sessizce
    /// kaldırır (ölçüldü: codex-cli 0.159.2 `--help` olası değerleri sayar).
    #[error("izin verilmeyen argüman değeri: {flag} — {reason}")]
    ArgumentValue { flag: String, reason: String },
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
            Self::ArgumentValue { .. } => "E_CLI_ARGUMENT",
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

/// İzinli argüman kapısı. `--flag=value` ve `--flag value` biçimleri aynı
/// kurallardan geçer; bayrak adı listede yoksa istek reddedilir (INV-10).
/// Sicilde tanımlı **sürüm bayrakları** (`version_args`) ayrıca izinlidir:
/// yoklama kendi sicilindeki bayrağı kullanabildiği hâlde kapıya takılmamalı.
///
/// Her belirteç **bayrak adı** ve — varsa — **değeri** ile denetlenir. Adı
/// izinli olmak yetmez: değer bir güvenlik/format kararı taşıyorsa [`ArgValue`]
/// kapalı kümesine uymalıdır (`--sandbox=danger-full-access` reddedilir). Boş
/// belirteç bayrak sayılmaz; yalnız kapalı kümesinde boş değer ilan edilmiş bir
/// bayrağın değeri olarak geçebilir (`--tools ""`). Değer `-` ile başlayamaz:
/// CLI onu bayrak sanar (istem kapısındaki kuralın aynısı).
pub fn build_args(adapter_id: &str, extra: &[String]) -> Result<Vec<String>, AdapterError> {
    let spec =
        spec(adapter_id).ok_or_else(|| AdapterError::UnknownAdapter(adapter_id.to_string()))?;
    let mut out = Vec::with_capacity(extra.len());
    let mut index = 0;
    while index < extra.len() {
        let arg = &extra[index];
        if arg.is_empty() {
            return Err(AdapterError::ArgumentValue {
                flag: String::new(),
                reason: "boş belirteç bayrak değildir (yalnız değer olarak geçebilir)".to_string(),
            });
        }
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) => (flag, Some(value)),
            None => (arg.as_str(), None),
        };
        let rule = spec
            .arg_rule(flag)
            .ok_or_else(|| AdapterError::DisallowedArgument(flag.to_string()))?;
        match (rule, inline) {
            (ArgValue::None, None) => {
                out.push(arg.clone());
                index += 1;
            }
            (ArgValue::None, Some(_)) => {
                return Err(AdapterError::ArgumentValue {
                    flag: flag.to_string(),
                    reason: "bayrak değer almaz".to_string(),
                })
            }
            (rule, Some(value)) => {
                check_arg_value(flag, rule, value, true)?;
                out.push(arg.clone());
                index += 1;
            }
            (rule, None) => {
                // Ayrı belirteçli değer: değer **tek** belirteçtir ve bayrak
                // gibi görünemez; eksikse istek reddedilir.
                let value = extra
                    .get(index + 1)
                    .ok_or_else(|| AdapterError::ArgumentValue {
                        flag: flag.to_string(),
                        reason: "değer eksik".to_string(),
                    })?;
                check_arg_value(flag, rule, value, false)?;
                out.push(arg.clone());
                out.push(value.clone());
                index += 2;
            }
        }
    }
    Ok(out)
}

/// Değer kapısı.
///
/// - Boş değer **yalnız** ayrı belirteçte ve yalnız kapalı kümesinde boş değer
///   ilan edilmiş bayrakta geçer. Ölçüldü (Claude Code 2.1.303): `--tools=""`
///   araçları **kapatmıyor**, `--tools ""` kapatıyor → `=` ile boş değer
///   reddedilir, yoksa kapı ürünün kapatma yolunu sessizce etkisiz kılardı.
/// - `-` ile başlayan değer bayrak sanılır.
/// - Kapalı küme bildiren bayrakta değer kümede olmalıdır.
fn check_arg_value(
    flag: &str,
    rule: ArgValue,
    value: &str,
    attached: bool,
) -> Result<(), AdapterError> {
    if value.is_empty() {
        let empty_allowed =
            !attached && matches!(rule, ArgValue::OneOf(values) if values.contains(&""));
        if !empty_allowed {
            let reason = if attached {
                "boş değer '=' ile verilemez: bayrağı kapatmaz (ayrı belirteç kullanın)"
            } else {
                "değer boş olamaz"
            };
            return Err(AdapterError::ArgumentValue {
                flag: flag.to_string(),
                reason: reason.to_string(),
            });
        }
    }
    if value.starts_with('-') {
        return Err(AdapterError::ArgumentValue {
            flag: flag.to_string(),
            reason: "değer '-' ile başlayamaz: bayrak sanılır".to_string(),
        });
    }
    if let ArgValue::OneOf(allowed) = rule {
        if !allowed.contains(&value) {
            let shown: String = value.chars().take(40).collect();
            return Err(AdapterError::ArgumentValue {
                flag: flag.to_string(),
                reason: format!("değer izinli kümede değil ({shown})"),
            });
        }
    }
    Ok(())
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
        let (name, path) =
            resolve_in_dirs(adapter, std::slice::from_ref(&dir)).expect("eş ad bulunmalı");
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
        let (name, path) =
            resolve_in_dirs(adapter, std::slice::from_ref(&dir)).expect("bildirilen ad");
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

    /// Ölçülen kusur (2026-11-13): kapı yalnız **bayrak adına** bakıyordu, yani
    /// `--sandbox=danger-full-access` "izinli argüman" sayılıyordu. Codex
    /// `exec --help` (0.159.2) olası değerleri sayar:
    /// `read-only · workspace-write · danger-full-access`. Ürün tam erişim
    /// istemez; değer kapıda reddedilir.
    #[test]
    fn closed_set_flag_rejects_a_value_outside_the_set() {
        let danger = build_args("codex-cli", &["--sandbox=danger-full-access".into()]).unwrap_err();
        assert_eq!(danger.code(), "E_CLI_ARGUMENT");
        assert!(danger.to_string().contains("--sandbox"), "{danger}");
        // Ayrı belirteçli yazım da aynı kümeden geçer.
        let danger2 = build_args(
            "codex-cli",
            &["--sandbox".into(), "danger-full-access".into()],
        )
        .unwrap_err();
        assert_eq!(danger2.code(), "E_CLI_ARGUMENT");
        // Ürünün kabul ettiği değerler geçer (iki yazım da).
        assert!(build_args("codex-cli", &["--sandbox=read-only".into()]).is_ok());
        assert_eq!(
            build_args("codex-cli", &["--sandbox".into(), "workspace-write".into()])
                .expect("izinli"),
            vec!["--sandbox".to_string(), "workspace-write".to_string()]
        );
    }

    /// Aynı sınıf, sertleştirme bayrağında: claude `--tools` değeri `""` tüm
    /// araçları kapatır, `default` hepsini **açar** (2.1.303 `--help`).
    /// `--tools=default` adı izinli olduğu için kapıdan geçerdi.
    #[test]
    fn hardening_flag_value_cannot_be_replaced() {
        for attempt in ["--tools=default", "--tools=Bash,Read", "--tools="] {
            let err = build_args("claude-code", &[attempt.to_string()]).unwrap_err();
            assert_eq!(err.code(), "E_CLI_ARGUMENT", "{attempt}");
        }
        // Ürünün kullandığı biçim geçer: ayrı belirteçte boş değer.
        assert_eq!(
            build_args("claude-code", &["--tools".into(), String::new()]).expect("izinli"),
            vec!["--tools".to_string(), String::new()]
        );
    }

    /// Değer almayan bayrağa değer verilemez: `--ephemeral=false` gibi bir
    /// yazım bayrağı sessizce kapatabilir. Her iki yazım da reddedilir.
    #[test]
    fn value_on_a_plain_flag_is_rejected() {
        for (adapter, arg) in [
            ("codex-cli", "--ephemeral=false"),
            ("claude-code", "--strict-mcp-config=false"),
            ("claude-code", "--no-session-persistence=0"),
            ("antigravity-cli", "--sandbox=false"),
        ] {
            let err = build_args(adapter, &[arg.to_string()]).unwrap_err();
            assert_eq!(err.code(), "E_CLI_ARGUMENT", "{adapter} {arg}");
        }
        // Ayrı belirteçli değer de kabul edilmez: `--ephemeral false`.
        let err = build_args("codex-cli", &["--ephemeral".into(), "false".into()]).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
    }

    /// Değer `-` ile başlayamaz: CLI onu bayrak sanar (istem kapısındaki
    /// kuralın aynısı) → `--model --dangerously-skip-permissions` sızamaz.
    #[test]
    fn value_that_looks_like_a_flag_is_rejected() {
        for arg in ["--model=--yolo", "--model=-x"] {
            let err = build_args("claude-code", &[arg.to_string()]).unwrap_err();
            assert_eq!(err.code(), "E_CLI_ARGUMENT", "{arg}");
        }
        let err = build_args("claude-code", &["--model".into(), "--yolo".into()]).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
        // Değer eksikse istek reddedilir (sessizce bayrak eklenmez).
        let err = build_args("claude-code", &["--model".into()]).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
    }

    /// AntiGravity istemi `--print=<istem>` ile taşır; istem yolu
    /// `prompt_argv`dir ve `validate_prompt`den geçer. Argüman kapısından
    /// istem geçemez — aksi hâlde istem doğrulaması atlanırdı.
    #[test]
    fn prompt_cannot_enter_through_the_argument_gate() {
        let err = build_args("antigravity-cli", &["--print=uzun bir istem".into()]).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
        assert!(build_args("antigravity-cli", &["--print".into()]).is_ok());
        // agy'nin zarf biçimi kapalı: ürün yalnız `json` okur (T08e).
        assert!(build_args("antigravity-cli", &["--output-format=json".into()]).is_ok());
        assert_eq!(
            build_args("antigravity-cli", &["--output-format=text".into()])
                .unwrap_err()
                .code(),
            "E_CLI_ARGUMENT"
        );
    }

    /// Sicil sözleşmesi: değer kararı **verilmeden** bayrak eklenemez; her
    /// izinli bayrak tam olarak bir karar taşır ve `version_args` değer almaz.
    #[test]
    fn every_allowed_flag_carries_a_value_decision() {
        for adapter in SPECS {
            let flags: Vec<&str> = adapter.allowed_flags().collect();
            let mut unique = flags.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(
                unique.len(),
                flags.len(),
                "{}: yinelenen bayrak",
                adapter.id
            );
            for flag in flags {
                assert!(
                    adapter.arg_rule(flag).is_some(),
                    "{}: {flag} kararı yok",
                    adapter.id
                );
                assert!(!flag.is_empty(), "{}: boş bayrak", adapter.id);
                // Sürüm bayrakları ayrı listede; çakışma olmamalı.
                assert!(
                    !adapter.version_args.contains(&flag),
                    "{}: {flag} hem izinli hem sürüm bayrağı",
                    adapter.id
                );
            }
            for flag in adapter.version_args {
                assert_eq!(adapter.arg_rule(flag), Some(ArgValue::None), "{flag}");
            }
            // Kapalı küme boş olamaz: boş küme bayrağı sessizce kapatırdı.
            for entry in adapter.allowed_args {
                if let ArgValue::OneOf(values) = entry.value {
                    assert!(
                        !values.is_empty(),
                        "{}: {} boş kapalı küme",
                        adapter.id,
                        entry.flag
                    );
                }
            }
            // Listedeki hiçbir bayrak kapının dışında kalmaz.
            assert!(adapter.arg_rule("--nope-not-listed").is_none());
        }
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
