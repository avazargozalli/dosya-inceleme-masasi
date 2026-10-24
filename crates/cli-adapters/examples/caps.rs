//! Tanı aracı: **yetenek matrisi**ni (F10-T11) gerçek CLI'larda yoklar.
//!
//! ```text
//! cargo run -p dim-cli-adapters --example caps
//! cargo run -p dim-cli-adapters --example caps -- claude-code codex-cli
//! ```
//!
//! Her adaptör için: kurulu sürüm, `--help` yoklamasının argv'si, dört yeteneğin
//! durumu ve bağlantı durumu. İstem gönderilmez, kimlik dosyası okunmaz; yardım
//! metni de boş `0700` kum havuzunda çalışır.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use dim_cli_adapters::{
    capabilities, discover, evaluate, find_on_path, help_text_of, run_help_probe, spec,
    DiscoveryState, SPECS,
};

fn main() {
    let requested: Vec<String> = std::env::args().skip(1).collect();
    let ids: Vec<&str> = if requested.is_empty() {
        SPECS.iter().map(|s| s.id).collect()
    } else {
        requested.iter().map(String::as_str).collect()
    };

    for id in ids {
        let Some(adapter) = spec(id) else {
            println!("{id}: bilinmeyen adaptör");
            continue;
        };
        let Some(caps) = capabilities(id) else {
            println!("{id}: yetenek sicili yok");
            continue;
        };
        let discovery = discover(adapter);
        let version = match &discovery.state {
            DiscoveryState::Installed { version, .. } => Some(version.clone()),
            _ => None,
        };
        println!("== {} ({})", adapter.display_name, adapter.id);
        match &discovery.state {
            DiscoveryState::Installed { path, version } => {
                println!("   sürüm    : {version}");
                println!("   yol      : {}", path.display());
            }
            DiscoveryState::Unverified { path, reason } => {
                println!("   sürüm    : okunamadı ({reason}) · {}", path.display());
            }
            DiscoveryState::NotInstalled => println!("   sürüm    : kurulu değil"),
        }
        println!("   yardım   : {:?}", caps.help_args);

        let (help, help_error) = if version.is_some() && caps.help_probe {
            match find_on_path(adapter.command) {
                Some(executable) => {
                    let cancel = Arc::new(AtomicBool::new(false));
                    match run_help_probe(adapter, &executable, &cancel) {
                        Ok(probe) => {
                            println!(
                                "   yoklama  : {:?} · {} ms · kesik={}",
                                probe.status, probe.duration_ms, probe.truncated
                            );
                            match help_text_of(&probe) {
                                Ok(text) => (Some(text.to_string()), None),
                                Err(reason) => (None, Some(reason.to_string())),
                            }
                        }
                        Err(err) => (None, Some(err.code().to_string())),
                    }
                }
                None => (None, Some("not_installed".to_string())),
            }
        } else if version.is_some() {
            (None, Some("help_probe_disabled".to_string()))
        } else {
            (None, None)
        };

        let report = evaluate(
            id,
            version.as_deref(),
            help.as_deref(),
            help_error.as_deref(),
        )
        .expect("rapor");
        for row in &report.rows {
            println!(
                "   - {:<26} {:<24} {}",
                row.capability.label(),
                row.state.label(),
                row.note
            );
        }
        println!(
            "   bağlantı : {} · kapı={} · gerekçe={:?}",
            report.link.label(),
            if report.gate_open {
                "açık"
            } else {
                "kapalı"
            },
            report.gate_reasons
        );
    }
}
