//! Tanı aracı: **başlangıç saldırılarını** (F10-T12) gerçek CLI'larda ölçer.
//!
//! ```text
//! cargo run -p dim-cli-adapters --example attack
//! cargo run -p dim-cli-adapters --example attack -- claude-code codex-cli
//! ```
//!
//! İki ayrı ölçüm yazdırılır ve karıştırılmaz:
//! - **Fikstür:** kirli/temiz yerleşimde naif yükleyici taklidi (dedektör kanıtı),
//!   kirli proje dizininin iş dizini olarak reddi ve ürün yapılandırmasında
//!   vekilin ulaşabildikleri (canary dahil).
//! - **Gerçek CLI:** sertleştirme bayrakları + `--help`, boş kum havuzu, `HOME`
//!   sahte ev dizini. İstem gönderilmez; işaretçi/yansıma aranır.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use dim_cli_adapters::{
    capabilities, discover, evaluate, find_on_path, help_text_of, outcomes, run_real_attack, spec,
    AttackFixture, AttackVector, DiscoveryState, HelpProbe, RealAttack, RunStatus, SPECS,
};

fn print_fixture(result: &dim_cli_adapters::FixtureResult) {
    println!("== fikstür (geçici dizin; bırakılınca silinir)");
    println!(
        "   dedektör : kirli {}/{} · temiz {}/{} · kanıtlı={} · kontrol temiz={}",
        result.detector.polluted_hits,
        AttackVector::ALL.len(),
        result.detector.clean_hits,
        AttackVector::ALL.len(),
        result.detector.proven,
        result.detector.control_clean
    );
    println!(
        "   kapı     : {} ({:?})",
        if result.gate.rejected {
            "kirli proje dizini iş dizini olarak REDDEDİLDİ"
        } else {
            "KABUL EDİLDİ (beklenmiyordu)"
        },
        result.gate.code
    );
    for observation in &result.product_config {
        println!(
            "   ürün cfg : {:<34} {}",
            observation.vector.label(),
            if observation.hit {
                "ULAŞILDI"
            } else {
                "ulaşılamadı"
            }
        );
    }
    println!(
        "   not      : proje/hook/MCP vektörü temiz, canary mutlak yolla okunabiliyor={}",
        result.canary_read_in_product_config()
    );
}

fn print_real(adapter: &str, version: Option<&str>, run: &RealAttack) {
    println!(
        "   koşu     : {:?} · {} ms · stdout {} B · stderr {} B · kesik={}",
        run.status, run.duration_ms, run.stdout_bytes, run.stderr_bytes, run.truncated
    );
    println!("   argv     : {:?}", run.argv);
    println!("   HOME     : {} (sahte ev)", run.home.display());
    let hits = run.observations.iter().filter(|o| o.hit).count();
    println!("   işaret   : {hits}");

    let probe = HelpProbe {
        args: run.argv.clone(),
        stdout: run.stdout_head.clone(),
        stderr: run.stderr_head.clone(),
        status: run.status.clone(),
        duration_ms: run.duration_ms,
        truncated: run.truncated,
    };
    let help = help_text_of(&probe).ok();
    let report = evaluate(adapter, version, help, None);
    let Some(report) = report else {
        println!("   yetenek  : sicil yok");
        return;
    };
    println!(
        "   kapı     : {} · {:?}",
        report.link.label(),
        report.gate_reasons
    );
    for outcome in outcomes(&report, &run.observations, true) {
        println!(
            "   - {:<34} {:<36} kapatma: {}",
            outcome.vector.label(),
            outcome.verdict.label(),
            if outcome.guard.flags().is_empty() {
                "-".to_string()
            } else {
                outcome.guard.flags().join(" ")
            }
        );
    }
}

fn main() {
    let requested: Vec<String> = std::env::args().skip(1).collect();
    let ids: Vec<&str> = if requested.is_empty() {
        SPECS.iter().map(|s| s.id).collect()
    } else {
        requested.iter().map(String::as_str).collect()
    };

    let fixture = AttackFixture::create().expect("fikstür");
    let result = fixture.run().expect("fikstür ölçümü");
    print_fixture(&result);

    let cancel = Arc::new(AtomicBool::new(false));
    for id in ids {
        let Some(adapter) = spec(id) else {
            println!("{id}: bilinmeyen adaptör");
            continue;
        };
        println!("\n== {} ({})", adapter.display_name, adapter.id);
        let Some(caps) = capabilities(id) else {
            println!("   yetenek sicili yok");
            continue;
        };
        let discovery = discover(adapter);
        let version = match &discovery.state {
            DiscoveryState::Installed { version, .. } => Some(version.clone()),
            _ => None,
        };
        match &discovery.state {
            DiscoveryState::Installed { version, path } => {
                println!("   sürüm    : {version} · {}", path.display());
            }
            DiscoveryState::Unverified { path, reason } => {
                println!("   sürüm    : okunamadı ({reason}) · {}", path.display());
            }
            DiscoveryState::NotInstalled => println!("   sürüm    : kurulu değil"),
        }
        if version.is_none() || caps.help_args.is_empty() {
            println!("   saldırı  : koşulmadı (kurulu değil ya da etkileşimli-only)");
            continue;
        }
        let Some(executable) = find_on_path(adapter.command) else {
            println!("   saldırı  : koşulmadı (yürütülebilir bulunamadı)");
            continue;
        };
        match run_real_attack(&fixture, adapter, &executable, &cancel) {
            Ok(run) => {
                if matches!(run.status, RunStatus::Exited(0)) {
                    print_real(adapter.id, version.as_deref(), &run);
                } else {
                    print_real(adapter.id, version.as_deref(), &run);
                    println!("   UYARI    : koşu sıfır dışı bitti; kararlar kanıt sayılmaz");
                }
            }
            Err(err) => println!("   saldırı  : çalıştırılamadı ({})", err.code()),
        }
    }
}
