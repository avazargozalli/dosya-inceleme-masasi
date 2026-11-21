//! **Doğrulanmış argv fixture'ı** (F10-T08i) ve yürütme anındaki **son argv kapısı**.
//!
//! Şartname (12.6) tek cümleyle istiyor: *"F10'da gerçek CLI sürümüne ait
//! doğrulanmış argv fixture'ı oluşturulur."* Bu modül o fixture'ı **sicilde**
//! tutar ve iki soruyu ayrı yanıtlar:
//!
//! 1. **Ürün gerçekten hangi komut satırını kuruyor?** [`FIXTURES`] her adaptör
//!    için `version` · `help` · `prompt` senaryolarının **tam** argv'sini taşır.
//!    Test, fixture'ı ürünün kendi kurma yoluyla (`build_args` →
//!    `hardened_args` → `splice_prompt`) yeniden üretip **birebir** karşılaştırır;
//!    fixture elle yazılmış bir belge değil, kodun aynasıdır.
//! 2. **Bu argv kurulu sürümde doğrulandı mı?** Fixture hangi CLI sürümünde
//!    `--help`/`--version` ile doğrulandığını taşır ([`version_key`] ile
//!    karşılaştırılır). İstem gönderim kapısı bu yanıtı **ister**: sürüm
//!    eşleşmiyorsa (ya da sürüm bilinmiyorsa) kapı temkinli tarafta kapanır —
//!    "fixture var" tek başına "bu sürümde geçerli" demek değildir.
//!
//! [`verify_executed_argv`] ise **yürütme sınırında** durur: son argv'nin her
//! belirteci ya sicildeki bir bayraktır (değeri kuralına uyar) ya da istem
//! biçiminin ([`crate::prompt_argv`]) kendisidir. Bugün ürün yollarının hepsi
//! bu kapıdan geçer; kapı gelecekte eklenecek bir sertleştirme grubunun ya da
//! yeni bir istem biçiminin sessizce sicil dışına çıkmasını engeller. Aynı
//! bayrağın iki kez geçmesi de reddedilir: CLI'lar son değeri sessizce uygular,
//! yani `--tools "" --tools default` gibi bir yazım güvenlik kararını ezer.
//!
//! **Sınır (dürüst):** fixture *biçimi* ve *bayrak kümesini* doğrular; CLI'nin
//! o bayrağı gerçekten o anlama geldiğini çalıştırarak kanıtlamaz. Davranış
//! kanıtı F10-T08 sınamasının (araç erişimi) işidir.

use crate::{spec, AdapterError, ArgValue, PROMPT_PLACEHOLDER};

/// `--version` senaryosu: sürüm yoklaması (ürün: [`crate::probe_version`]).
pub const SCENARIO_VERSION: &str = "version";
/// `--help` senaryosu: yetenek matrisinin yardım okuması.
pub const SCENARIO_HELP: &str = "help";
/// İstem senaryosu: ürünün sabit sınama istemini taşıyan argv (F10-T08).
pub const SCENARIO_PROMPT: &str = "prompt";

/// Bir adaptörün bir senaryodaki **doğrulanmış** komut satırı.
#[derive(Debug, Clone, Copy)]
pub struct ArgvFixture {
    pub adapter_id: &'static str,
    pub scenario: &'static str,
    /// Doğrulamanın yapıldığı CLI sürümü: `--version` çıktısının ilk satırı,
    /// **birebir**. Karşılaştırma [`version_key`] ile yapılır (önek/parantez
    /// farkı sürüm farkı değildir; ama sürüm **yoksa** eşleşme de yoktur).
    pub verified_version: &'static str,
    /// Ürünün çağıran olarak verdiği argümanlar (senaryonun girdisi). Test
    /// ürünün kurma yolunu bunlarla çalıştırır.
    pub caller_args: &'static [&'static str],
    /// Ürünün **gerçekten** koşacağı tam argv. İstem yerine
    /// [`PROMPT_PLACEHOLDER`] durur (istemi ürün üretir; arayüzden gelmez).
    pub argv: &'static [&'static str],
    /// Doğrulama kanıtı (hangi sürümde hangi bayraklar yardım metninde görüldü).
    pub evidence: &'static str,
}

/// Dört adaptörün doğrulanmış argv fixture'ları.
///
/// Sürümler bu makinede **2026-11-21**'de ölçüldü: `claude --version` →
/// `2.1.303 (Claude Code)`, `codex --version` → `codex-cli 0.159.2`,
/// `agy --version` → `1.5.6`. Kimi bu makinede kurulu değil; satırı T09'un
/// 2026-11-13 ölçümünden gelir ve istem yolu **yoktur** (toplu yürütme kapalı),
/// bu yüzden `prompt` fixture'ı bilinçli olarak yoktur.
pub const FIXTURES: &[ArgvFixture] = &[
    ArgvFixture {
        adapter_id: "claude-code",
        scenario: SCENARIO_VERSION,
        verified_version: "2.1.303 (Claude Code)",
        caller_args: &["--version"],
        argv: &["--version"],
        evidence: "claude 2.1.303 --version (2026-11-21)",
    },
    ArgvFixture {
        adapter_id: "claude-code",
        scenario: SCENARIO_HELP,
        verified_version: "2.1.303 (Claude Code)",
        caller_args: &["--help"],
        argv: &[
            "--help",
            "--no-session-persistence",
            "--tools",
            "",
            "--strict-mcp-config",
        ],
        evidence: "claude 2.1.303 --help: --print, --no-session-persistence, --tools, --strict-mcp-config, --output-format, --json-schema görüldü (2026-11-21)",
    },
    ArgvFixture {
        adapter_id: "claude-code",
        scenario: SCENARIO_PROMPT,
        verified_version: "2.1.303 (Claude Code)",
        caller_args: &[],
        argv: &[
            "--no-session-persistence",
            "--tools",
            "",
            "--strict-mcp-config",
            "--print",
            PROMPT_PLACEHOLDER,
        ],
        evidence: "claude 2.1.303 --help: --print boolean, --tools \"\" kapatır, --no-session-persistence yalnız --print ile (2026-11-21)",
    },
    ArgvFixture {
        adapter_id: "codex-cli",
        scenario: SCENARIO_VERSION,
        verified_version: "codex-cli 0.159.2",
        caller_args: &["--version"],
        argv: &["--version"],
        evidence: "codex-cli 0.159.2 --version (2026-11-21)",
    },
    ArgvFixture {
        adapter_id: "codex-cli",
        scenario: SCENARIO_HELP,
        verified_version: "codex-cli 0.159.2",
        caller_args: &["exec", "--help"],
        argv: &[
            "exec",
            "--help",
            "--ignore-user-config",
            "--ignore-rules",
        ],
        evidence: "codex-cli 0.159.2 exec --help: exec, --json, --sandbox, --skip-git-repo-check, --ephemeral, --output-schema, --ignore-user-config, --ignore-rules görüldü (2026-11-21)",
    },
    ArgvFixture {
        adapter_id: "codex-cli",
        scenario: SCENARIO_PROMPT,
        verified_version: "codex-cli 0.159.2",
        caller_args: &["--skip-git-repo-check"],
        argv: &[
            "exec",
            "--skip-git-repo-check",
            "--ignore-user-config",
            "--ignore-rules",
            PROMPT_PLACEHOLDER,
        ],
        evidence: "codex exec [OPTIONS] [PROMPT]: alt komut ilk belirteç, istem konumsal (0.159.2 --help, 2026-11-21)",
    },
    ArgvFixture {
        adapter_id: "antigravity-cli",
        scenario: SCENARIO_VERSION,
        verified_version: "1.5.6",
        caller_args: &["--version"],
        argv: &["--version"],
        evidence: "agy 1.5.6 --version (2026-11-21)",
    },
    ArgvFixture {
        adapter_id: "antigravity-cli",
        scenario: SCENARIO_HELP,
        verified_version: "1.5.6",
        caller_args: &["--help"],
        argv: &["--help", "--sandbox", "--disable-slash-commands"],
        evidence: "agy 1.5.6 --help: --print, --output-format, --json-schema, --sandbox, --disable-slash-commands, --print-timeout görüldü (2026-11-21)",
    },
    ArgvFixture {
        adapter_id: "antigravity-cli",
        scenario: SCENARIO_PROMPT,
        verified_version: "1.5.6",
        caller_args: &["--output-format=json"],
        argv: &[
            "--output-format=json",
            "--sandbox",
            "--disable-slash-commands",
            "--print=<istem>",
        ],
        evidence: "agy 1.4.12'de ölçüldü: `--print` komşu bayrağı istem sanar → istem `=` ile bağlanır; 1.5.6'da --print/--sandbox/--disable-slash-commands/--output-format yardımda görüldü (2026-11-21)",
    },
    ArgvFixture {
        adapter_id: "kimi-cli",
        scenario: SCENARIO_VERSION,
        verified_version: "1.41.0",
        caller_args: &["--version"],
        argv: &["--version"],
        evidence: "Kimi CLI 1.41.0, T09 ölçümü (2026-11-13); 2026-11-21'de bu makinede kurulu değil",
    },
];

/// Adaptörün fixture'ları (senaryo adıyla).
pub fn fixtures_for(adapter_id: &str) -> impl Iterator<Item = &'static ArgvFixture> + '_ {
    FIXTURES
        .iter()
        .filter(move |fixture| fixture.adapter_id == adapter_id)
}

/// Tek fixture. Yoksa `None` — "fixture yok" **geçti** demek değildir; çağıran
/// bunu kapalı tarafta yorumlar ([`prompt_fixture_check`]).
#[must_use]
pub fn fixture(adapter_id: &str, scenario: &str) -> Option<&'static ArgvFixture> {
    FIXTURES
        .iter()
        .find(|fixture| fixture.adapter_id == adapter_id && fixture.scenario == scenario)
}

/// Sürüm metninin **anahtarı**: ilk `v`/`V` önekli ya da düz, nokta taşıyan
/// sayısal belirteç. `2.1.303 (Claude Code)` → `2.1.303`, `codex-cli 0.159.2`
/// → `0.159.2`. Belirteç yoksa `None`: sürüm uydurulmaz.
///
/// Neden tam eşitlik değil: `--version` çıktısı ürün adını/parantezi
/// taşıyabiliyor; sürümün **kendisi** ile ürün etiketi karıştırılmamalı. Önek
/// dışında hiçbir normalizasyon yapılmaz: `1.2.3-beta.1` ayrı kalır, yani ön
/// sürüm ile sürüm birbirine karışmaz.
#[must_use]
pub fn version_key(text: &str) -> Option<String> {
    for raw in text.split_whitespace() {
        let token =
            raw.trim_matches(|c: char| !c.is_ascii_alphanumeric() && !matches!(c, '.' | '-' | '+'));
        let token = token
            .strip_prefix('v')
            .or_else(|| token.strip_prefix('V'))
            .unwrap_or(token);
        if !token.contains('.') {
            continue;
        }
        if !token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        if !token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
        {
            continue;
        }
        return Some(token.to_string());
    }
    None
}

/// İstem fixture'ının kurulu sürümdeki durumu. `reason` kapalıysa **neden**
/// kapalı olduğunu da söyler (`version_unknown` / `version_mismatch`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureCheck {
    pub verified: bool,
    pub fixture_version: &'static str,
    pub fixture_key: Option<String>,
    pub installed_key: Option<String>,
    pub reason: Option<&'static str>,
}

/// İstem fixture'ı kurulu sürümü kapsıyor mu? Adaptörün `prompt` fixture'ı
/// yoksa `None` (istem yolu yok — kapı zaten başka nedenle kapalı).
#[must_use]
pub fn prompt_fixture_check(
    adapter_id: &str,
    installed_version: Option<&str>,
) -> Option<FixtureCheck> {
    let fixture = fixture(adapter_id, SCENARIO_PROMPT)?;
    let fixture_key = version_key(fixture.verified_version);
    let installed_key = installed_version.and_then(version_key);
    let (verified, reason) = match (&fixture_key, &installed_key) {
        (Some(fixture_key), Some(installed_key)) if fixture_key == installed_key => (true, None),
        (Some(_), Some(_)) => (false, Some("version_mismatch")),
        _ => (false, Some("version_unknown")),
    };
    Some(FixtureCheck {
        verified,
        fixture_version: fixture.verified_version,
        fixture_key,
        installed_key,
        reason,
    })
}

/// Bir belirtecin bayrak adı: `--tools` → `--tools`, `--tools=` → `--tools`.
/// Bayrak gibi görünmeyen belirteç (`exec`, istem metni) kendisi döner.
fn flag_part(token: &str) -> &str {
    token.split_once('=').map_or(token, |(flag, _)| flag)
}

fn duplicate(flag: &str) -> AdapterError {
    AdapterError::ArgumentValue {
        flag: flag.to_string(),
        reason: "bayrak yineleniyor: son değer öncekini sessizce ezer".to_string(),
    }
}

/// **Yürütme anındaki son argv kapısı.**
///
/// Her belirteç şu üç kaynaktan birine girmelidir:
/// * sicildeki bir bayrak (değer kuralı [`crate::check_arg_value`] ile aynen),
/// * adaptörün istem biçimi ([`crate::prompt_argv`], yalnız istem gerçekten
///   gönderiliyorsa ve **tek** kez),
/// * sicildeki konumsal belirteç (`codex exec` gibi — `arg_rule` kapsar).
///
/// Aynı bayrak iki kez geçemez; boş argv reddedilir. Bu kapı **yürütmeden
/// önce** ve **süreç başlamadan** çalışır.
pub fn verify_executed_argv(
    adapter_id: &str,
    argv: &[String],
    prompt: Option<&str>,
) -> Result<(), AdapterError> {
    let adapter =
        spec(adapter_id).ok_or_else(|| AdapterError::UnknownAdapter(adapter_id.to_string()))?;
    if argv.is_empty() {
        return Err(AdapterError::PromptShape("argv boş".to_string()));
    }
    // İstem biçimi **tek kaynaktan** doğrulanır ([`crate::prompt_argv`]);
    // burada yalnız "bu istem metni argv'de tam olarak nerede duruyor" sorusu
    // yanıtlanır. Biçim yeniden yazılmaz.
    let prompt_slot = match prompt {
        Some(text) => locate_prompt(adapter_id, argv, text)?,
        None => None,
    };
    let mut seen: Vec<&str> = Vec::with_capacity(argv.len());
    let mut index = 0;
    while index < argv.len() {
        if Some(index) == prompt_slot {
            // İstem metni konumsaldır: bayrak kapısına girmez (biçimi
            // `locate_prompt` doğruladı).
            index += 1;
            continue;
        }
        let token = &argv[index];
        let flag = flag_part(token);
        let inline = token.split_once('=').map(|(_, value)| value);
        let rule = adapter
            .arg_rule(flag)
            .ok_or_else(|| AdapterError::DisallowedArgument(flag.to_string()))?;
        if seen.contains(&flag) {
            return Err(duplicate(flag));
        }
        seen.push(flag);
        match (rule, inline) {
            (ArgValue::None, None) => index += 1,
            (ArgValue::None, Some(_)) => {
                return Err(AdapterError::ArgumentValue {
                    flag: flag.to_string(),
                    reason: "bayrak değer almaz".to_string(),
                })
            }
            (rule, Some(value)) => {
                crate::check_arg_value(flag, rule, value, true)?;
                index += 1;
            }
            (rule, None) => {
                let value = argv
                    .get(index + 1)
                    .ok_or_else(|| AdapterError::ArgumentValue {
                        flag: flag.to_string(),
                        reason: "değer eksik".to_string(),
                    })?;
                crate::check_arg_value(flag, rule, value, false)?;
                index += 2;
            }
        }
    }
    Ok(())
}

/// İstem metninin argv'deki **tek** konumu. Biçim adaptöre göre değişir ve
/// [`crate::prompt_argv`] ile [`crate::splice_prompt`]ın ürettiği yapıyla
/// birebir aynı olmalıdır:
///
/// * `claude-code`: `--print <istem>` — tam bir `--print` ve **hemen ardından**
///   istem.
/// * `codex-cli`: `exec … <istem>` — `exec` ilk belirteç, istem **son**
///   belirteç.
/// * `antigravity-cli`: `--print=<istem>` — tek belirteç, tam bir kez.
///
/// `None` döner: istem gönderilmedi (bu durumda istem biçimi taşıyan bir
/// belirteç zaten bayrak kapısından düşer — `--print=x` değer almaz).
fn locate_prompt(
    adapter_id: &str,
    argv: &[String],
    prompt: &str,
) -> Result<Option<usize>, AdapterError> {
    let shaped = crate::prompt_argv(adapter_id, prompt)?;
    match adapter_id {
        "antigravity-cli" => {
            let token = shaped
                .first()
                .ok_or_else(|| AdapterError::PromptShape("istem biçimi boş".to_string()))?;
            let hits: Vec<usize> = argv
                .iter()
                .enumerate()
                .filter(|(_, candidate)| *candidate == token)
                .map(|(index, _)| index)
                .collect();
            match hits.as_slice() {
                [index] => Ok(Some(*index)),
                [] => Err(AdapterError::PromptShape(
                    "istem argv'de yok: biçim uygulanmamış".to_string(),
                )),
                _ => Err(AdapterError::PromptShape(
                    "istem argv'de birden fazla kez".to_string(),
                )),
            }
        }
        "claude-code" => {
            let print_hits: Vec<usize> = argv
                .iter()
                .enumerate()
                .filter(|(_, candidate)| candidate.as_str() == "--print")
                .map(|(index, _)| index)
                .collect();
            match print_hits.as_slice() {
                [index] if argv.get(index + 1).is_some_and(|next| next == prompt) => {
                    Ok(Some(index + 1))
                }
                [] => Err(AdapterError::PromptShape(
                    "istem argv'de yok: --print <istem> biçimi uygulanmamış".to_string(),
                )),
                _ => Err(AdapterError::PromptShape(
                    "istem biçimi belirsiz: --print yineleniyor ya da istem komşu değil"
                        .to_string(),
                )),
            }
        }
        "codex-cli" => {
            let first_is_exec = argv.first().is_some_and(|first| first == "exec");
            let last_is_prompt = argv.len() >= 2 && argv.last().is_some_and(|last| last == prompt);
            let exec_hits = argv
                .iter()
                .filter(|candidate| candidate.as_str() == "exec")
                .count();
            if first_is_exec && last_is_prompt && exec_hits == 1 {
                Ok(argv.len().checked_sub(1))
            } else {
                Err(AdapterError::PromptShape(
                    "istem biçimi bozuk: `exec … <istem>` bekleniyor".to_string(),
                ))
            }
        }
        _ => Err(AdapterError::PromptShape(format!(
            "{adapter_id}: istem yolu tanımlı değil"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    /// Fixture'ın **her** bayrağı sicilden geçer: fixture ile kapı ayrışamaz.
    #[test]
    fn every_fixture_flag_is_in_the_registry() {
        for fixture in FIXTURES {
            let adapter = spec(fixture.adapter_id).expect("adaptör");
            for token in fixture.argv {
                if *token == PROMPT_PLACEHOLDER || token.is_empty() {
                    // Boş belirteç yalnız `--tools ""` gibi bayrak **değeri**
                    // olarak geçer; kendi başına bayrak değildir.
                    continue;
                }
                let flag = flag_part(token);
                assert!(
                    adapter.arg_rule(flag).is_some(),
                    "{} {}: sicilde olmayan bayrak {flag}",
                    fixture.adapter_id,
                    fixture.scenario
                );
            }
        }
    }

    /// Fixture ürünün kendi kurma yolunun aynasıdır: `build_args` →
    /// `hardened_args` → `splice_prompt` çıktısı fixture argv'sine **birebir**
    /// eşit olmalı. Fixture elle yazılmış bir belge değildir.
    #[test]
    fn fixtures_match_the_product_build() {
        for fixture in FIXTURES {
            let adapter = spec(fixture.adapter_id).expect("adaptör");
            let actual: Vec<String> = match fixture.scenario {
                SCENARIO_VERSION => adapter
                    .version_args
                    .first()
                    .map(|arg| vec![(*arg).to_string()])
                    .unwrap_or_default(),
                SCENARIO_HELP => {
                    let help = crate::capabilities(fixture.adapter_id)
                        .expect("yetenek satırı")
                        .help_args
                        .iter()
                        .map(|arg| (*arg).to_string())
                        .collect::<Vec<_>>();
                    crate::hardened_args(
                        fixture.adapter_id,
                        crate::build_args(fixture.adapter_id, &help).expect("yardım bayrakları"),
                    )
                }
                SCENARIO_PROMPT => {
                    let args = crate::build_args(fixture.adapter_id, &owned(fixture.caller_args))
                        .expect("çağıran argümanları");
                    let hardened = crate::hardened_args(fixture.adapter_id, args);
                    crate::splice_prompt(fixture.adapter_id, hardened, PROMPT_PLACEHOLDER)
                        .expect("istem biçimi")
                }
                other => panic!("bilinmeyen senaryo {other}"),
            };
            assert_eq!(
                actual,
                owned(fixture.argv),
                "{} {}: fixture ürünün kurduğu argv'den koptu",
                fixture.adapter_id,
                fixture.scenario
            );
        }
    }

    /// Dört adaptörün beklenen senaryo kümesi: üçü üç senaryo, Kimi yalnız
    /// sürüm (istem yolu yok). Yeni adaptör sessizce kapsamsız kalamaz.
    #[test]
    fn fixtures_cover_every_adapter_and_scenario() {
        for id in ["claude-code", "codex-cli", "antigravity-cli"] {
            for scenario in [SCENARIO_VERSION, SCENARIO_HELP, SCENARIO_PROMPT] {
                assert!(fixture(id, scenario).is_some(), "{id} {scenario}");
            }
        }
        assert!(fixture("kimi-cli", SCENARIO_VERSION).is_some());
        assert!(
            fixture("kimi-cli", SCENARIO_PROMPT).is_none(),
            "Kimi'de toplu yürütme kapalı: istem fixture'ı olamaz"
        );
        assert_eq!(FIXTURES.len(), 10);
    }

    /// Yürütme kapısı ürünün bütün fixture argv'lerini **kabul** eder: kapı
    /// ürünün kendi yolunu kilitlemez.
    #[test]
    fn the_execution_gate_accepts_every_fixture() {
        for fixture in FIXTURES {
            let prompt = (fixture.scenario == SCENARIO_PROMPT).then_some(PROMPT_PLACEHOLDER);
            verify_executed_argv(fixture.adapter_id, &owned(fixture.argv), prompt).unwrap_or_else(
                |err| {
                    panic!(
                        "{} {}: ürünün argv'si kapıdan düştü: {err}",
                        fixture.adapter_id, fixture.scenario
                    )
                },
            );
        }
    }

    /// Saldırı sınıfı 1: sicilde olmayan bayrak son argv'ye eklenirse kapı
    /// yürütmeden **önce** reddeder.
    #[test]
    fn an_unregistered_flag_is_rejected() {
        let mut argv = owned(
            fixture("claude-code", SCENARIO_PROMPT)
                .expect("fixture")
                .argv,
        );
        argv.push("--dangerously-skip-permissions".to_string());
        let err = verify_executed_argv("claude-code", &argv, Some(PROMPT_PLACEHOLDER)).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
        assert!(
            err.to_string().contains("--dangerously-skip-permissions"),
            "{err}"
        );
    }

    /// Saldırı sınıfı 2: bayrak olmayan (konumsal) bir belirteç argv'ye
    /// sokulamaz — ikinci istem, dosya yolu, komut vb.
    #[test]
    fn a_positional_injection_is_rejected() {
        let argv = owned(&["--print", "metin"]);
        let err = verify_executed_argv("claude-code", &argv, None).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
        assert!(err.to_string().contains("metin"), "{err}");
    }

    /// Saldırı sınıfı 3: istem gönderilmediği hâlde istem biçimi argv'de
    /// olamaz (`--print=<istem>` sicilde değer almayan bayraktır).
    #[test]
    fn a_prompt_shaped_token_without_a_prompt_is_rejected() {
        let argv = owned(&[
            "--output-format=json",
            "--sandbox",
            "--disable-slash-commands",
            "--print=metin",
        ]);
        let err = verify_executed_argv("antigravity-cli", &argv, None).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
    }

    /// Saldırı sınıfı 4: aynı bayrak iki kez geçemez. CLI'lar son değeri
    /// uygular; `--tools "" --tools default` güvenlik kararını ezer.
    #[test]
    fn a_repeated_flag_is_rejected() {
        let claude = owned(&[
            "--tools",
            "",
            "--tools",
            "default",
            "--print",
            PROMPT_PLACEHOLDER,
        ]);
        let err =
            verify_executed_argv("claude-code", &claude, Some(PROMPT_PLACEHOLDER)).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
        assert!(err.to_string().contains("yineleniyor"), "{err}");

        // İstem bayrağının ikinci kez geçmesi de yinelenmedir.
        let mut prompt_twice = owned(
            fixture("claude-code", SCENARIO_PROMPT)
                .expect("fixture")
                .argv,
        );
        prompt_twice.extend(owned(&["--print", "ikinci"]));
        let err = verify_executed_argv("claude-code", &prompt_twice, Some(PROMPT_PLACEHOLDER))
            .unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
    }

    /// Saldırı sınıfı 5: istem gönderildiği hâlde argv'de istem biçimi yoksa
    /// kapı "gönderilmiş gibi" geçmez.
    #[test]
    fn a_missing_prompt_shape_is_rejected() {
        let argv = owned(&[
            "--no-session-persistence",
            "--tools",
            "",
            "--strict-mcp-config",
        ]);
        let err = verify_executed_argv("claude-code", &argv, Some(PROMPT_PLACEHOLDER)).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
        assert!(err.to_string().contains("istem"), "{err}");
    }

    /// Codex'te istem **son** belirteçtir (`exec … <istem>`); ortada duran bir
    /// istem, sonraki bayrağı istemin arkasına atar ve biçim bozulur.
    #[test]
    fn codex_prompt_must_be_the_last_token() {
        let argv = owned(&["exec", PROMPT_PLACEHOLDER, "--json"]);
        let err = verify_executed_argv("codex-cli", &argv, Some(PROMPT_PLACEHOLDER)).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
    }

    /// Saldırı sınıfı 6: değer kuralları son kapıda da geçerlidir (kapalı küme
    /// ve bayrak gibi görünen değer).
    #[test]
    fn value_rules_still_apply_at_the_execution_gate() {
        let danger = owned(&["exec", "--sandbox", "danger-full-access", "metin"]);
        let err = verify_executed_argv("codex-cli", &danger, Some("metin")).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");

        let flag_like = owned(&["--model", "--json"]);
        let err = verify_executed_argv("claude-code", &flag_like, None).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");

        // `--tools ""` ayrı belirteçte geçer; `=` ile boş değer geçmez.
        assert!(verify_executed_argv(
            "claude-code",
            &owned(&["--tools", "", "--print", PROMPT_PLACEHOLDER]),
            Some(PROMPT_PLACEHOLDER)
        )
        .is_ok());
        let attached = owned(&["--tools=", "--print", PROMPT_PLACEHOLDER]);
        assert!(verify_executed_argv("claude-code", &attached, Some(PROMPT_PLACEHOLDER)).is_err());
    }

    #[test]
    fn an_empty_argv_is_rejected() {
        let err = verify_executed_argv("claude-code", &[], None).unwrap_err();
        assert_eq!(err.code(), "E_CLI_ARGUMENT");
    }

    /// Ürünün **gerçek** saldırı yardım argv'si (sertleştirme + `--help`) de
    /// kapıdan geçer: yeni kapı mevcut yolları kilitlemez.
    #[test]
    fn the_real_attack_argv_passes_the_execution_gate() {
        for adapter in crate::SPECS {
            if crate::capabilities(adapter.id).is_none_or(|caps| caps.help_args.is_empty()) {
                continue;
            }
            let argv = crate::attack::hardening_argv(adapter.id).expect("argv");
            verify_executed_argv(adapter.id, &argv, None)
                .unwrap_or_else(|err| panic!("{}: {err}", adapter.id));
        }
    }

    #[test]
    fn version_key_extracts_the_numeric_core() {
        assert_eq!(
            version_key("2.1.303 (Claude Code)").as_deref(),
            Some("2.1.303")
        );
        assert_eq!(version_key("codex-cli 0.159.2").as_deref(), Some("0.159.2"));
        assert_eq!(version_key("1.5.6").as_deref(), Some("1.5.6"));
        assert_eq!(version_key("v1.2.3").as_deref(), Some("1.2.3"));
        assert_eq!(
            version_key("Claude Code 2.1.303").as_deref(),
            Some("2.1.303")
        );
        // Ön sürüm ayrı kalır: normalizasyon yalnız önek/etiket temizler.
        assert_eq!(version_key("1.2.3-beta.1").as_deref(), Some("1.2.3-beta.1"));
        // Sürüm yoksa uydurulmaz.
        assert_eq!(version_key(""), None);
        assert_eq!(version_key("bilinmiyor"), None);
        assert_eq!(version_key("2026-11-21"), None);
    }

    /// Fixture kurulu sürüme **bağlıdır**: aynı sürüm taze, farklı sürüm ve
    /// bilinmeyen sürüm bayat. "Fixture var" tek başına geçiş değildir.
    #[test]
    fn the_prompt_fixture_is_fresh_only_for_its_verified_version() {
        let same =
            prompt_fixture_check("claude-code", Some("2.1.303 (Claude Code)")).expect("fixture");
        assert!(same.verified);
        assert_eq!(same.reason, None);
        assert_eq!(same.fixture_key.as_deref(), Some("2.1.303"));

        // `--version` biçimi değişse bile sürüm çekirdeği eşleşir.
        let bare = prompt_fixture_check("claude-code", Some("2.1.303")).expect("fixture");
        assert!(bare.verified);

        let newer =
            prompt_fixture_check("claude-code", Some("2.1.400 (Claude Code)")).expect("fixture");
        assert!(!newer.verified);
        assert_eq!(newer.reason, Some("version_mismatch"));
        assert_eq!(newer.installed_key.as_deref(), Some("2.1.400"));

        let unknown = prompt_fixture_check("claude-code", None).expect("fixture");
        assert!(!unknown.verified);
        assert_eq!(unknown.reason, Some("version_unknown"));

        let codex = prompt_fixture_check("codex-cli", Some("codex-cli 0.159.2")).expect("fixture");
        assert!(codex.verified);
        let agy = prompt_fixture_check("antigravity-cli", Some("1.5.6")).expect("fixture");
        assert!(agy.verified);
        // Fixture başka adaptörün sürümüyle tazelenmez.
        let crossed = prompt_fixture_check("claude-code", Some("0.159.2")).expect("fixture");
        assert!(!crossed.verified);
    }

    /// İstem yolu olmayan adaptörde fixture **yoktur**: `None`, "geçti" diye
    /// okunmaz (kapı `batch_forbidden` ile zaten kapalı).
    #[test]
    fn a_missing_prompt_fixture_is_not_a_pass() {
        assert!(prompt_fixture_check("kimi-cli", Some("1.41.0")).is_none());
        assert!(fixture("kimi-cli", SCENARIO_PROMPT).is_none());
    }
}
