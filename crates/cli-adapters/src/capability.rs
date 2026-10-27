//! F10-T11 — adaptör × sürüm × yetenek matrisi.
//!
//! Her adaptör için dört yetenek kaydedilir: **araçsız** çalışma, **hook/MCP
//! kapalı**, **şemalı çıktı** ve **oturum kalıcılığını kapatma**. Yeteneğin
//! kanıtı kurulu sürümün **kendi `--help` metni**dir: bayrak orada geçiyorsa
//! `Declared`, geçmiyorsa `Missing`, doğrulanmış yol yoksa `NoPath`, yardım
//! okunamadıysa `Unread`.
//!
//! `Declared` **davranış kanıtı değildir**: istem gönderilmediği için "araç
//! çağrısı sıfır" ya da "oturum yazılmadı" ölçülmedi. Bu yüzden bağlantı
//! durumu "hazır" diye değil `Declared` ("bildirildi · denenmedi") diye
//! raporlanır; gerekli bir yetenek `Missing` ise bağlantı **uyumsuz**, `NoPath`
//! ya da `Unread` ise **sınırlı** olur ve yetenek kapısı kapanır.
//!
//! Kimlik dosyası okunmaz, token kopyalanmaz (INV-11). Yardım yoklaması da
//! `exec::run_cli` üzerinden gider: izinli argv, allowlist ortam, boş `0700`
//! kum havuzu, süre sınırı ve iptal.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use crate::{
    default_env_allowlist, run_cli, spec, AdapterSpec, CliRunRequest, HomeIsolation, RunError,
    RunSandbox, RunStatus,
};

/// Yardım yoklaması süresi; çekirdek yine 1–600 sn aralığına kırpar.
/// Gerçek ölçüm: `claude --help` 2.1.303 bu makinede **13,1 sn** sürdü (kendi
/// yapılandırmasını tarıyor), `codex exec --help` 78 ms. 20 sn sınırı dar
/// kalıyordu; 30 sn'ye çıkarıldı.
pub const HELP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Araç çağrısı yapmadan yanıt üretme (araç listesi kapalı).
    ToolFree,
    /// Hook, MCP sunucusu ve proje talimatı yüklenmemesi.
    HookMcpOff,
    /// Şemaya bağlı (JSON Schema) yapısal çıktı.
    StructuredOutput,
    /// Oturum/transkript kalıcılığının kapatılması.
    SessionPersistenceOff,
}

impl Capability {
    pub const ALL: [Self; 4] = [
        Self::ToolFree,
        Self::HookMcpOff,
        Self::StructuredOutput,
        Self::SessionPersistenceOff,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::ToolFree => "tool_free",
            Self::HookMcpOff => "hook_mcp_off",
            Self::StructuredOutput => "structured_output",
            Self::SessionPersistenceOff => "session_persistence_off",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ToolFree => "Araçsız çalışma",
            Self::HookMcpOff => "Hook / MCP kapalı",
            Self::StructuredOutput => "Şemalı çıktı",
            Self::SessionPersistenceOff => "Oturum kalıcılığı kapalı",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mechanism {
    /// Kurulu sürümün `--help` metninde bildirilen bayrak(lar).
    HelpFlag,
    /// Doğrulanmış kapatma/çalıştırma yolu yok; durum `NoPath` kalır.
    NoVerifiedPath,
}

#[derive(Debug, Clone, Copy)]
pub struct CapabilityRequirement {
    pub capability: Capability,
    /// Güvenlik kapısı: bu yetenek doğrulanmadan otomatik inceleme açılmaz.
    pub required: bool,
    pub mechanism: Mechanism,
    /// Yardım metninde **birlikte** bulunması gereken bayraklar.
    pub flags: &'static [&'static str],
    /// Kullanıcıya gösterilen kısa gerekçe/açıklama.
    pub note: &'static str,
}

pub struct AdapterCapabilities {
    pub adapter_id: &'static str,
    /// Yardım metnini almak için argv; `build_args` kapısından geçmelidir.
    pub help_args: &'static [&'static str],
    /// Yardım yoklaması yapılabilir mi? Etkileşimli-only sağlayıcıda **hayır**.
    pub help_probe: bool,
    pub requirements: &'static [CapabilityRequirement],
}

const CLAUDE: &[CapabilityRequirement] = &[
    CapabilityRequirement {
        capability: Capability::ToolFree,
        required: true,
        mechanism: Mechanism::HelpFlag,
        flags: &["--tools"],
        note: "`--tools \"\"` tüm yerleşik araçları kapatır; ürün bu bayrağı **zorunlu** ekler. Ölçüldü (2.1.303): boş iş dizininde bile mutlak yol okunuyordu, bayrakla okunamadı. `--tools=` biçimi kapatmıyor.",
    },
    CapabilityRequirement {
        capability: Capability::HookMcpOff,
        required: true,
        mechanism: Mechanism::HelpFlag,
        flags: &["--strict-mcp-config"],
        note: "`--strict-mcp-config` yalnız `--mcp-config` sunucularını yükler; ürün MCP yapılandırması vermez → MCP kapalı. **Hook'lar için doğrulanmış bayrak yok**: `--bare` oturumu düşürüyor (ölçüldü, \"Not logged in\").",
    },
    CapabilityRequirement {
        capability: Capability::StructuredOutput,
        required: true,
        mechanism: Mechanism::HelpFlag,
        flags: &["--json-schema"],
        note: "`--json-schema` ile JSON Schema'ya bağlı çıktı.",
    },
    CapabilityRequirement {
        capability: Capability::SessionPersistenceOff,
        required: true,
        mechanism: Mechanism::HelpFlag,
        flags: &["--no-session-persistence"],
        note: "`--no-session-persistence` yalnız `--print` ile.",
    },
];

const CODEX: &[CapabilityRequirement] = &[
    CapabilityRequirement {
        capability: Capability::ToolFree,
        required: true,
        mechanism: Mechanism::NoVerifiedPath,
        flags: &[],
        note: "`codex exec` araçsız kip bildirmiyor; `--sandbox read-only` araçları kısıtlar ama kapatmaz.",
    },
    CapabilityRequirement {
        capability: Capability::HookMcpOff,
        required: true,
        mechanism: Mechanism::HelpFlag,
        flags: &["--ignore-user-config", "--ignore-rules"],
        note: "`--ignore-user-config` kullanıcı yapılandırmasını (MCP sunucuları dahil) yüklemez; `--ignore-rules` proje kurallarını atlar.",
    },
    CapabilityRequirement {
        capability: Capability::StructuredOutput,
        required: true,
        mechanism: Mechanism::HelpFlag,
        flags: &["--output-schema"],
        note: "`--output-schema` JSON Schema dosyasıyla son yanıt şekli.",
    },
    CapabilityRequirement {
        capability: Capability::SessionPersistenceOff,
        required: true,
        mechanism: Mechanism::HelpFlag,
        flags: &["--ephemeral"],
        note: "`--ephemeral` oturum dosyası yazmaz.",
    },
];

const NO_TRANSPORT: &[CapabilityRequirement] = &[
    CapabilityRequirement {
        capability: Capability::ToolFree,
        required: true,
        mechanism: Mechanism::NoVerifiedPath,
        flags: &[],
        note: "Doğrulanmış resmî taşıma yok; araç kısıtı kanıtlanamaz.",
    },
    CapabilityRequirement {
        capability: Capability::HookMcpOff,
        required: true,
        mechanism: Mechanism::NoVerifiedPath,
        flags: &[],
        note: "Doğrulanmış resmî taşıma yok; hook/MCP kapatma yolu kanıtlanamaz.",
    },
    CapabilityRequirement {
        capability: Capability::StructuredOutput,
        required: true,
        mechanism: Mechanism::NoVerifiedPath,
        flags: &[],
        note: "Doğrulanmış resmî taşıma yok; şemalı çıktı kanıtlanamaz.",
    },
    CapabilityRequirement {
        capability: Capability::SessionPersistenceOff,
        required: true,
        mechanism: Mechanism::NoVerifiedPath,
        flags: &[],
        note: "Doğrulanmış resmî taşıma yok; oturum kapatma yolu kanıtlanamaz.",
    },
];

/// Adaptör başına yetenek sicili. Sürüm aralığı **tahmin edilmez**: kanıt,
/// yoklama anındaki kurulu sürümün yardım metnidir ve raporda sürümle taşınır.
pub const MATRIX: &[AdapterCapabilities] = &[
    AdapterCapabilities {
        adapter_id: "claude-code",
        help_args: &["--help"],
        help_probe: true,
        requirements: CLAUDE,
    },
    AdapterCapabilities {
        adapter_id: "codex-cli",
        // Yetenek bayrakları `codex exec --help` altında bildirilir.
        help_args: &["exec", "--help"],
        help_probe: true,
        requirements: CODEX,
    },
    AdapterCapabilities {
        adapter_id: "antigravity-cli",
        help_args: &["--help"],
        help_probe: true,
        requirements: NO_TRANSPORT,
    },
    AdapterCapabilities {
        adapter_id: "kimi-cli",
        help_args: &[],
        // Etkileşimli-only: otomatik yürütme kapalı, yardım yoklaması yapılmaz.
        help_probe: false,
        requirements: NO_TRANSPORT,
    },
];

#[must_use]
pub fn capabilities(adapter_id: &str) -> Option<&'static AdapterCapabilities> {
    MATRIX.iter().find(|row| row.adapter_id == adapter_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityState {
    /// Kurulu sürümün yardım metni bayrağı bildiriyor; **davranış denenmedi**.
    Declared { flags: Vec<&'static str> },
    /// Yardım metni okundu, bayrak(lar) yok: bu sürümde yol yok.
    Missing { absent: Vec<&'static str> },
    /// Doğrulanmış yol yok (sicil) — "desteklenmiyor" demek için kanıt yok.
    NoPath { reason: &'static str },
    /// Yardım metni okunamadı (kurulu değil, çıkış kodu, süre, iptal).
    Unread { reason: String },
}

impl CapabilityState {
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::Declared { .. } => "declared",
            Self::Missing { .. } => "missing",
            Self::NoPath { .. } => "no_path",
            Self::Unread { .. } => "unread",
        }
    }

    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Declared { .. } => "bildirildi · denenmedi",
            Self::Missing { .. } => "kurulu sürümde yok",
            Self::NoPath { .. } => "doğrulanmış yol yok",
            Self::Unread { .. } => "okunamadı",
        }
    }

    #[must_use]
    pub const fn is_declared(&self) -> bool {
        matches!(self, Self::Declared { .. })
    }

    /// `Missing` = desteklenmediği **gösterildi**; kapıyı uyumsuzluğa çeker.
    #[must_use]
    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    NotInstalled,
    /// Gerekli bir yetenek kurulu sürümde **yok** ya da otomatik kullanım kapalı.
    Incompatible,
    /// Gerekli bir yetenek doğrulanamadı (yol yok / yardım okunamadı).
    Limited,
    /// Gerekli yeteneklerin hepsi yardım metninde bildirildi; davranış denenmedi.
    Declared,
}

impl LinkState {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotInstalled => "not_installed",
            Self::Incompatible => "incompatible",
            Self::Limited => "limited",
            Self::Declared => "declared",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotInstalled => "kurulu değil",
            Self::Incompatible => "uyumsuz",
            Self::Limited => "sınırlı",
            Self::Declared => "bildirildi · denenmedi",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityRow {
    pub capability: Capability,
    pub required: bool,
    pub note: &'static str,
    pub state: CapabilityState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReport {
    pub adapter_id: &'static str,
    pub installed_version: Option<String>,
    pub help_args: Vec<String>,
    /// Yardım metni okunduysa `true` (içerik rapora konmaz).
    pub help_read: bool,
    pub help_error: Option<String>,
    pub rows: Vec<CapabilityRow>,
    pub link: LinkState,
    /// Gerekli yeteneklerin hepsi `Declared` ise `true`. Davranış kanıtı değil.
    pub gate_open: bool,
    /// Kapanış gerekçeleri: `yetenek:durum` (metin/çıktı içermez).
    pub gate_reasons: Vec<String>,
}

/// Saf değerlendirme: yardım metni ve sürüm verilir, rapor üretilir. Süreç
/// başlatmaz; testler gerçek CLI olmadan da aynı kuralları doğrular.
#[must_use]
pub fn evaluate(
    adapter_id: &str,
    version: Option<&str>,
    help: Option<&str>,
    help_error: Option<&str>,
) -> Option<CapabilityReport> {
    let adapter = capabilities(adapter_id)?;
    let mut rows = Vec::with_capacity(adapter.requirements.len());
    for requirement in adapter.requirements {
        let state = match (version, help) {
            (None, _) => CapabilityState::Unread {
                reason: "not_installed".to_string(),
            },
            (Some(_), _) if !adapter.help_probe => CapabilityState::NoPath {
                reason: requirement.note,
            },
            (Some(_), _) if requirement.mechanism == Mechanism::NoVerifiedPath => {
                CapabilityState::NoPath {
                    reason: requirement.note,
                }
            }
            (Some(_), Some(text)) => {
                let absent: Vec<&'static str> = requirement
                    .flags
                    .iter()
                    .copied()
                    .filter(|flag| !mentions_flag(text, flag))
                    .collect();
                if absent.is_empty() {
                    CapabilityState::Declared {
                        flags: requirement.flags.to_vec(),
                    }
                } else {
                    CapabilityState::Missing { absent }
                }
            }
            (Some(_), None) => CapabilityState::Unread {
                reason: help_error.unwrap_or("help_unavailable").to_string(),
            },
        };
        rows.push(CapabilityRow {
            capability: requirement.capability,
            required: requirement.required,
            note: requirement.note,
            state,
        });
    }

    let batch_forbidden = spec(adapter_id).is_some_and(|s| s.batch_forbidden);
    let link = if version.is_none() {
        LinkState::NotInstalled
    } else if batch_forbidden || rows.iter().any(|r| r.required && r.state.is_missing()) {
        LinkState::Incompatible
    } else if rows.iter().any(|r| r.required && !r.state.is_declared()) {
        LinkState::Limited
    } else {
        LinkState::Declared
    };

    let gate_reasons: Vec<String> = rows
        .iter()
        .filter(|r| r.required && !r.state.is_declared())
        .map(|r| format!("{}:{}", r.capability.id(), r.state.id()))
        .collect();
    let gate_open = link == LinkState::Declared;

    Some(CapabilityReport {
        adapter_id: adapter.adapter_id,
        installed_version: version.map(str::to_string),
        help_args: adapter.help_args.iter().map(|a| (*a).to_string()).collect(),
        help_read: help.is_some(),
        help_error: help_error.map(str::to_string),
        rows,
        link,
        gate_open,
        gate_reasons,
    })
}

/// Bayrağın yardım metninde **belirteç olarak** geçtiğini söyler: hemen
/// öncesi boşluk/parantez/virgül, hemen sonrası boşluk/`=`/`,`/`<` olmalı.
/// Böylece `--ephemeral` metni `--ephemeral-x` ile eşleşmez.
#[must_use]
pub fn mentions_flag(help: &str, flag: &str) -> bool {
    if flag.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(offset) = help[from..].find(flag) {
        let start = from + offset;
        let end = start + flag.len();
        let before_ok = match help[..start].chars().next_back() {
            None => true,
            Some(c) => c.is_whitespace() || matches!(c, '(' | ',' | '[' | '/'),
        };
        let after_ok = match help[end..].chars().next() {
            None => true,
            Some(c) => c.is_whitespace() || matches!(c, '=' | ',' | '<' | ')' | ']' | ':'),
        };
        if before_ok && after_ok {
            return true;
        }
        from = end;
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpProbe {
    pub args: Vec<String>,
    pub stdout: String,
    pub stderr: String,
    pub status: RunStatus,
    pub duration_ms: u64,
    /// Çıktı 64 KiB'te kesildiyse `true`: eksik bayrak "yok" sayılmamalı.
    pub truncated: bool,
}

/// Yardım yoklaması: izinli argv, allowlist ortam, boş `0700` kum havuzu,
/// süre sınırı ve iptal. İstem gönderilmez, kimlik dosyası okunmaz.
pub fn run_help_probe(
    adapter: &AdapterSpec,
    executable: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<HelpProbe, RunError> {
    let row = capabilities(adapter.id)
        .ok_or_else(|| crate::AdapterError::UnknownAdapter(adapter.id.to_string()))?;
    let sandbox = RunSandbox::create()?;
    let request = CliRunRequest {
        adapter_id: adapter.id.to_string(),
        executable: executable.to_path_buf(),
        args: row.help_args.iter().map(|a| (*a).to_string()).collect(),
        stdin_text: String::new(),
        workdir: sandbox.path().to_path_buf(),
        timeout: HELP_TIMEOUT,
        env: default_env_allowlist(),
        home_isolation: HomeIsolation::Sandbox,
    };
    let outcome = run_cli(&request, cancel)?;
    Ok(HelpProbe {
        args: request.args,
        stdout: outcome.stdout_head,
        stderr: outcome.stderr_head,
        status: outcome.status,
        duration_ms: outcome.duration_ms,
        truncated: outcome.stdout_truncated || outcome.stderr_truncated,
    })
}

/// Rapor için yardım metnini seçer: stdout boşsa stderr kullanılır (bazı
/// araçlar yardımı stderr'e yazar). Kesilmiş çıktı **kanıt sayılmaz**.
pub fn help_text_of(probe: &HelpProbe) -> Result<&str, &'static str> {
    if probe.truncated {
        return Err("help_truncated");
    }
    if !matches!(probe.status, RunStatus::Exited(0)) {
        return Err("help_exit_nonzero");
    }
    let text = if probe.stdout.trim().is_empty() {
        probe.stderr.as_str()
    } else {
        probe.stdout.as_str()
    };
    if text.trim().is_empty() {
        return Err("help_empty");
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_args, spec};
    use std::path::PathBuf;

    /// Gerçek `claude --help` (2.1.303) metninden kısaltılmış örnek.
    const CLAUDE_HELP: &str = "\
  --allowedTools, --allowed-tools <tools...>
  --bare                                Minimal mode: skip hooks (those defined
                                        in settings and by installed plugins)
  --disallowedTools, --disallowed-tools <tools...>
  --json-schema <schema>                JSON Schema for structured output
  --mcp-config <configs...>             Load MCP servers from JSON files or
  --no-session-persistence              Disable session persistence - sessions
  --strict-mcp-config                   Only use MCP servers from --mcp-config,
  --tools <tools...>                    Specify the list of available tools from
                                        the built-in set. Use \"\" to disable all
";

    /// Gerçek `codex exec --help` (0.159.2) metninden kısaltılmış örnek.
    const CODEX_HELP: &str = "\
      --ephemeral
      --ignore-rules
      --ignore-user-config
      --json
          Print events to stdout as JSONL
      --output-schema <FILE>
          Path to a JSON Schema file describing the model's final response shape
      -s, --sandbox <SANDBOX_MODE>
";

    #[test]
    fn unknown_adapter_has_no_matrix() {
        assert!(evaluate("yok", Some("1.0.0"), Some(CLAUDE_HELP), None).is_none());
        assert!(capabilities("yok").is_none());
    }

    #[test]
    fn claude_help_declares_all_four_capabilities() {
        let report =
            evaluate("claude-code", Some("2.1.303"), Some(CLAUDE_HELP), None).expect("rapor");
        assert_eq!(report.rows.len(), 4);
        assert!(report.rows.iter().all(|r| r.state.is_declared()));
        assert_eq!(report.link, LinkState::Declared);
        assert!(report.gate_open);
        assert!(report.gate_reasons.is_empty());
        assert_eq!(report.installed_version.as_deref(), Some("2.1.303"));
        assert_eq!(report.help_args, vec!["--help".to_string()]);
    }

    #[test]
    fn missing_flag_makes_link_incompatible_and_closes_gate() {
        let without_schema = CLAUDE_HELP
            .lines()
            .filter(|l| !l.contains("--json-schema"))
            .collect::<Vec<_>>()
            .join("\n");
        let report =
            evaluate("claude-code", Some("2.1.303"), Some(&without_schema), None).expect("rapor");
        let structured = report
            .rows
            .iter()
            .find(|r| r.capability == Capability::StructuredOutput)
            .expect("satır");
        assert_eq!(structured.state.id(), "missing");
        assert_eq!(report.link, LinkState::Incompatible);
        assert!(!report.gate_open);
        assert_eq!(report.gate_reasons, vec!["structured_output:missing"]);
    }

    #[test]
    fn unreadable_help_keeps_link_limited_not_incompatible() {
        let report = evaluate(
            "claude-code",
            Some("2.1.303"),
            None,
            Some("help_exit_nonzero"),
        )
        .expect("rapor");
        assert_eq!(report.link, LinkState::Limited);
        assert!(!report.gate_open);
        assert!(report.rows.iter().all(|r| r.state.id() == "unread"));
        assert!(report
            .gate_reasons
            .iter()
            .all(|reason| reason.ends_with(":unread")));
    }

    #[test]
    fn not_installed_is_reported_without_claiming_capabilities() {
        let report = evaluate("claude-code", None, None, None).expect("rapor");
        assert_eq!(report.link, LinkState::NotInstalled);
        assert!(!report.gate_open);
        assert!(report.rows.iter().all(|r| r.state.id() == "unread"));
    }

    #[test]
    fn interactive_only_adapter_is_incompatible_even_with_help() {
        let report = evaluate("kimi-cli", Some("1.9.0"), Some(CLAUDE_HELP), None).expect("rapor");
        assert_eq!(report.link, LinkState::Incompatible);
        assert!(!report.gate_open);
        // Kimi'de yardım yoklaması yapılmaz: durum NoPath kalır.
        assert!(report.rows.iter().all(|r| r.state.id() == "no_path"));
    }

    #[test]
    fn codex_tool_free_stays_unverified_and_limits_link() {
        let report = evaluate("codex-cli", Some("0.159.2"), Some(CODEX_HELP), None).expect("rapor");
        let tool_free = report
            .rows
            .iter()
            .find(|r| r.capability == Capability::ToolFree)
            .expect("satır");
        assert_eq!(tool_free.state.id(), "no_path");
        assert_eq!(report.rows.len(), 4);
        assert_eq!(report.link, LinkState::Limited);
        assert!(!report.gate_open);
        assert_eq!(report.gate_reasons, vec!["tool_free:no_path"]);
        // Kalan üç yetenek gerçekten yardım metninde bulundu.
        assert_eq!(
            report.rows.iter().filter(|r| r.state.is_declared()).count(),
            3
        );
    }

    #[test]
    fn flag_matching_requires_token_boundaries() {
        assert!(mentions_flag(CLAUDE_HELP, "--tools"));
        assert!(mentions_flag(
            "--no-session-persistence\n",
            "--no-session-persistence"
        ));
        assert!(mentions_flag("(e.g. --ephemeral)", "--ephemeral"));
        // Belirteç sınırı: daha uzun bayrağın içinde eşleşmez.
        assert!(!mentions_flag("--ephemeral-extra\n", "--ephemeral"));
        assert!(!mentions_flag("--allowedTools\n", "--tools"));
        assert!(!mentions_flag("x--tools\n", "--tools"));
        assert!(!mentions_flag("", "--tools"));
    }

    #[test]
    fn declared_capability_flags_pass_the_argument_gate() {
        // Matris ile argüman kapısı birbirinden ayrışmamalı: her bildirilen
        // bayrak gerçekten izinli olmalı.
        for adapter in MATRIX {
            if !adapter.help_args.is_empty() {
                build_args(
                    adapter.adapter_id,
                    &adapter
                        .help_args
                        .iter()
                        .map(|a| (*a).to_string())
                        .collect::<Vec<_>>(),
                )
                .unwrap_or_else(|err| {
                    panic!("{}: yardım argv kapıya takıldı: {err}", adapter.adapter_id)
                });
            }
            for requirement in adapter.requirements {
                for flag in requirement.flags {
                    build_args(adapter.adapter_id, &[(*flag).to_string()]).unwrap_or_else(|err| {
                        panic!("{}: {flag} kapıya takıldı: {err}", adapter.adapter_id)
                    });
                }
            }
        }
    }

    #[test]
    fn matrix_covers_every_registered_adapter() {
        for adapter in crate::SPECS {
            assert!(
                capabilities(adapter.id).is_some(),
                "{}: yetenek sicili yok",
                adapter.id
            );
        }
    }

    #[cfg(unix)]
    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
        path
    }

    #[cfg(unix)]
    #[test]
    fn real_help_probe_runs_in_sandbox_and_yields_declared_rows() {
        let tmp = tempfile::tempdir().expect("tmp");
        let exe = script(
            tmp.path(),
            "claude",
            "cat <<'EOF'\n  --bare\n  --json-schema <schema>\n  --no-session-persistence\n  --strict-mcp-config\n  --tools <tools...>\nEOF",
        );
        let adapter = spec("claude-code").expect("spec");
        let cancel = Arc::new(AtomicBool::new(false));
        let probe = run_help_probe(adapter, &exe, &cancel).expect("koştu");
        assert_eq!(probe.args, vec!["--help".to_string()]);
        assert_eq!(probe.status, RunStatus::Exited(0));
        let text = help_text_of(&probe).expect("yardım metni");
        let report = evaluate("claude-code", Some("2.1.303"), Some(text), None).expect("rapor");
        assert_eq!(report.link, LinkState::Declared);
        assert!(report.gate_open);
    }

    #[cfg(unix)]
    #[test]
    fn truncated_help_is_not_evidence() {
        let probe = HelpProbe {
            args: vec!["--help".to_string()],
            stdout: "--tools\n".to_string(),
            stderr: String::new(),
            status: RunStatus::Exited(0),
            duration_ms: 1,
            truncated: true,
        };
        assert_eq!(help_text_of(&probe), Err("help_truncated"));
    }

    #[cfg(unix)]
    #[test]
    fn failed_help_is_not_evidence() {
        let probe = HelpProbe {
            args: vec!["--help".to_string()],
            stdout: String::new(),
            stderr: "boom".to_string(),
            status: RunStatus::Exited(2),
            duration_ms: 1,
            truncated: false,
        };
        assert_eq!(help_text_of(&probe), Err("help_exit_nonzero"));
    }
}
