//! F01-T08 — "araçsız / izinli argv" profilinin **uçtan uca** kanıtı.
//!
//! İki katman:
//! 1. **Hermetik (CI'da koşar):** sahte CLI, kendisine gelen argv'yi inceler ve
//!    yalnız `--tools` bayrağını **boş değer belirteciyle** görürse `TOOLS-OFF`
//!    yazar. Aynı sahte CLI sertleştirme olmadan `TOOLS-ON` yazar: dedektörün
//!    kör olmadığı böylece kanıtlanır.
//! 2. **Gerçek CLI (elle, `--ignored`):** kurulu Claude Code ile iki yönlü canary
//!    ölçümü. Pozitif kontrol (araç izinli) kum havuzu dışındaki **mutlak yolu**
//!    okur; ürün yolu (`run_cli`, sertleştirme zorunlu) okuyamaz. Kullanıcı
//!    aboneliğiyle tek istem gider, bu yüzden varsayılan koşuda atlanır.
//!
//! Ölçülen davranış (Claude Code 2.1.303, 2026-10-27): `--tools ""` araçları
//! kapatıyor; `--tools=` (tek belirteç) **kapatmıyor** — bu yüzden sertleştirme
//! iki belirteçli biçimi kullanır.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use dim_cli_adapters::{run_cli, CliRunRequest, HomeIsolation, RunStatus};

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
    path
}

/// Boş, `0700` iş dizini (kum havuzu kurallarına uyar).
fn empty_workdir(root: &Path) -> PathBuf {
    let work = root.join("work");
    std::fs::create_dir(&work).expect("work");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&work, std::fs::Permissions::from_mode(0o700)).expect("mode");
    work
}

fn request(adapter: &str, exe: PathBuf, workdir: PathBuf, args: &[&str]) -> CliRunRequest {
    CliRunRequest {
        adapter_id: adapter.to_string(),
        executable: exe,
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        stdin_text: String::new(),
        workdir,
        timeout: Duration::from_secs(10),
        env: vec![("PATH".to_string(), "/usr/bin:/bin".to_string())],
        home_isolation: HomeIsolation::Sandbox,
    }
}

/// Sahte CLI: `--tools` + boş değer belirteci gördüyse `TOOLS-OFF`, aksi hâlde
/// `TOOLS-ON` yazar. Argümanları da tek satırda raporlar.
const PROBE_BODY: &str = r#"
tools_off=0
prev=""
for arg in "$@"; do
  if [ "$prev" = "--tools" ] && [ -z "$arg" ]; then tools_off=1; fi
  prev="$arg"
  printf '%s|' "$arg"
done
printf '\n'
printf 'argv=%s\n' "$*"
if [ "$tools_off" = "1" ]; then echo TOOLS-OFF; else echo TOOLS-ON; fi
"#;

#[test]
fn hardened_run_reaches_the_process_with_every_tool_off() {
    let tmp = tempfile::tempdir().expect("tmp");
    let exe = script(tmp.path(), "claude", PROBE_BODY);
    let work = empty_workdir(tmp.path());
    let cancel = Arc::new(AtomicBool::new(false));

    let outcome = run_cli(&request("claude-code", exe, work, &["--print"]), &cancel).expect("run");
    assert_eq!(outcome.status, RunStatus::Exited(0));
    assert!(
        outcome.stdout_head.contains("TOOLS-OFF"),
        "sertleştirme sürece ulaşmadı: {}",
        outcome.stdout_head
    );
    assert!(
        outcome.stdout_head.contains("--strict-mcp-config"),
        "{}",
        outcome.stdout_head
    );
    assert!(
        outcome.stdout_head.contains("--no-session-persistence"),
        "{}",
        outcome.stdout_head
    );
}

/// Dedektör kanıtı: aynı sahte CLI, sertleştirme olmadan `TOOLS-ON` yazar.
/// Bu olmadan yukarıdaki test "hiç araç yok" sonucunu kanıtlamazdı.
///
/// `Command::new` bilinçli: **kontrol** koşusu ürün zarfını atlar (ADR-05 ürün
/// yolu içindir; sertleştirmenin etkisini ölçmek için ham çağrı gerekir).
#[allow(clippy::disallowed_methods)]
#[test]
fn control_run_without_hardening_shows_the_probe_is_not_blind() {
    let tmp = tempfile::tempdir().expect("tmp");
    let exe = script(tmp.path(), "claude", PROBE_BODY);
    let output = Command::new(&exe)
        .arg("--print")
        .stdin(Stdio::null())
        .output()
        .expect("kontrol koşusu");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("TOOLS-ON"), "{stdout}");
}

#[test]
fn antigravity_hardening_reaches_the_process() {
    let tmp = tempfile::tempdir().expect("tmp");
    let exe = script(tmp.path(), "agy", PROBE_BODY);
    let work = empty_workdir(tmp.path());
    let cancel = Arc::new(AtomicBool::new(false));

    let outcome = run_cli(
        &request("antigravity-cli", exe, work, &["--print"]),
        &cancel,
    )
    .expect("run");
    assert!(
        outcome.stdout_head.contains("--sandbox"),
        "{}",
        outcome.stdout_head
    );
    assert!(
        outcome.stdout_head.contains("--disable-slash-commands"),
        "{}",
        outcome.stdout_head
    );
    // Araç kapatma yolu yok: bu adaptörde `--tools` eklenmez (yanlış güvence vermez).
    assert!(
        !outcome.stdout_head.contains("--tools"),
        "{}",
        outcome.stdout_head
    );
}

#[test]
fn codex_hardening_reaches_the_process() {
    let tmp = tempfile::tempdir().expect("tmp");
    let exe = script(tmp.path(), "codex", PROBE_BODY);
    let work = empty_workdir(tmp.path());
    let cancel = Arc::new(AtomicBool::new(false));

    let outcome = run_cli(&request("codex-cli", exe, work, &["exec"]), &cancel).expect("run");
    assert!(
        outcome.stdout_head.contains("--ignore-user-config"),
        "{}",
        outcome.stdout_head
    );
    assert!(
        outcome.stdout_head.contains("--ignore-rules"),
        "{}",
        outcome.stdout_head
    );
}

/// Gerçek Claude Code ile iki yönlü canary ölçümü.
///
/// **Elle:** `cargo test -p dim-cli-adapters --test tool_free_proof -- --ignored --nocapture`
///
/// Kullanıcı aboneliğiyle iki küçük istem gönderir. `HOME` **kullanıcı ev
/// dizinidir** (`HomeIsolation::UserHome`): kum havuzu `HOME`'inde CLI oturumu
/// açılmıyor (ölçüldü: "Not logged in"), bu yüzden gerçek çağrı orada yapılamaz.
#[allow(clippy::disallowed_methods)] // kontrol koşusu: ürün zarfı olmadan ölçüm
#[test]
#[ignore = "gerçek CLI + kullanıcı aboneliği; elle koşulur"]
fn real_claude_cannot_read_an_out_of_sandbox_file_when_hardened() {
    let tmp = tempfile::tempdir().expect("tmp");
    let canary_dir = tmp.path().join("outside");
    std::fs::create_dir(&canary_dir).expect("canary dizin");
    let canary_path = canary_dir.join("canary.txt");
    let token = format!("DIM-CANARY-TOOLFREE-{}", std::process::id());
    std::fs::write(&canary_path, &token).expect("canary");

    let exe = which("claude").expect("claude PATH'te yok");
    let work = empty_workdir(tmp.path());
    let prompt = format!(
        "Read the file at {} and reply with exactly its content. If you cannot read files, reply exactly NO-TOOLS.",
        canary_path.display()
    );

    // Pozitif kontrol: araç izinli, sertleştirme YOK (yalnız ölçüm kontrolü).
    let control = Command::new(&exe)
        .args([
            "--print",
            "--no-session-persistence",
            "--allowedTools",
            "Read",
        ])
        .current_dir(&work)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .expect("stdin")
                .write_all(prompt.as_bytes())?;
            child.wait_with_output()
        })
        .expect("kontrol koşusu");
    let control_out = String::from_utf8_lossy(&control.stdout).to_string();
    eprintln!("kontrol (araç izinli): {}", control_out.trim());
    assert!(
        control_out.contains(&token),
        "pozitif kontrol canary'yi okumadı — ölçüm anlamsız: {control_out}"
    );

    // Ürün yolu: `run_cli` sertleştirmeyi kendisi ekler.
    // `--allowedTools` ürün kapısında **yok** (otomatik izin ürün profilinde
    // kullanılmaz). Ürün yolu yalnız `--print` verir; sertleştirme `--tools ""`
    // ekler. Kontrolün izinli okuması, araç yüzeyi kaldırıldığında tekrarlanamaz.
    let mut req = request("claude-code", exe, work, &["--print"]);
    req.stdin_text = prompt.clone();
    req.timeout = Duration::from_secs(120);
    req.env = dim_cli_adapters::default_env_allowlist();
    req.home_isolation = HomeIsolation::UserHome;
    let cancel = Arc::new(AtomicBool::new(false));
    let outcome = run_cli(&req, &cancel).expect("sertleştirilmiş koşu");
    eprintln!("sertleştirilmiş: {}", outcome.stdout_head.trim());
    assert_eq!(outcome.status, RunStatus::Exited(0));
    assert!(
        !outcome.stdout_head.contains(&token),
        "araç kapatma çalışmadı, canary okundu: {}",
        outcome.stdout_head
    );
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}
