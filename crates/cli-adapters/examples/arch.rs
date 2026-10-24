//! Tanı aracı: **mimari uyum** denetimi (F10-T01) — gerçek CLI'larda.
//!
//! ```text
//! cargo run -p dim-cli-adapters --example arch
//! cargo run -p dim-cli-adapters --example arch -- claude-code codex-cli
//! ```
//!
//! Her adaptör için PATH'te bulunan yürütülebilirin **başlık baytları** okunur:
//! biçim (Mach-O/ELF/PE/betik), mimari dilimleri ve uygulamanın mimarisiyle
//! karşılaştırma. Süreç **başlatılmaz**; dosya çalıştırılmaz.

use dim_cli_adapters::{app_arch_label, find_on_path, inspect_path, spec, SPECS};

fn main() {
    let requested: Vec<String> = std::env::args().skip(1).collect();
    let ids: Vec<&str> = if requested.is_empty() {
        SPECS.iter().map(|s| s.id).collect()
    } else {
        requested.iter().map(String::as_str).collect()
    };

    println!("uygulama mimarisi: {}", app_arch_label());
    for id in ids {
        let Some(adapter) = spec(id) else {
            println!("{id}: bilinmeyen adaptör");
            continue;
        };
        println!("== {} ({})", adapter.display_name, adapter.id);
        let Some(executable) = find_on_path(adapter.command) else {
            println!("   yol      : PATH'te yok");
            continue;
        };
        let report = inspect_path(&executable);
        println!("   yol      : {}", report.path.display());
        if report.symlink {
            if let Some(resolved) = &report.resolved {
                println!("   bağ      : → {}", resolved.display());
            }
        }
        println!("   biçim    : {}", report.format.label());
        let arches: Vec<String> = report.architectures.iter().map(|a| a.as_str()).collect();
        println!(
            "   mimari   : {}",
            if arches.is_empty() {
                "—".to_string()
            } else {
                arches.join(" + ")
            }
        );
        println!("   karar    : {}", report.summary());
        if let Some(interpreter) = &report.interpreter {
            println!("   shebang  : {}", interpreter.command);
            if let Some(path) = &interpreter.path {
                println!("   yorumlayıcı: {}", path.display());
            }
        }
        if let Some(reason) = report.reason {
            println!("   neden    : {reason}");
        }
        println!("   başlık   : {} bayt okundu", report.header_bytes);
    }
}
