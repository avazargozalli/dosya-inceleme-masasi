//! Tanı aracı: arayüzdeki "Bağlantıyı sına" düğmesiyle **aynı** yolu çalıştırır.
//!
//! ```text
//! cargo run -p dim-cli-adapters --example probe -- claude-code
//! ```
//!
//! Çıktı: bulunan mutlak yol, izinli argv, allowlist ortam anahtarları, boş
//! geçici iş dizini, durum, çıkış kodu, süre ve stdout/stderr baş kısmı.
//! İstem gönderilmez, kimlik dosyası okunmaz; yalnız `--version` yoklanır.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use dim_cli_adapters::{find_on_path, run_probe, spec};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let id = args.first().map(String::as_str).unwrap_or("claude-code");
    let Some(adapter) = spec(id) else {
        eprintln!("bilinmeyen adaptör: {id}");
        std::process::exit(2);
    };
    let Some(executable) = find_on_path(adapter.command) else {
        println!("{}: PATH üzerinde bulunamadı", adapter.display_name);
        std::process::exit(3);
    };
    println!("adaptör     : {}", adapter.id);
    println!("yürütülebilir: {}", executable.display());
    println!(
        "argv        : {} {:?}",
        adapter.command,
        adapter.version_args.first()
    );

    let cancel = Arc::new(AtomicBool::new(false));
    match run_probe(adapter, executable, &cancel) {
        Ok(report) => {
            let scope = &report.outcome.scope;
            println!("ortam       : {}", report.env_keys.join(", "));
            println!(
                "iş dizini   : {} (boş, izin {:04o})",
                report.workdir.display(),
                scope.workdir_mode.unwrap_or(0)
            );
            println!(
                "kapsam      : HOME={} ({}) · TMPDIR={}",
                scope
                    .home
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "kullanıcı ev dizini".into()),
                scope.home_isolation.as_str(),
                scope
                    .tmp
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "kullanıcı TMPDIR".into())
            );
            println!(
                "stdin       : {} bayt / {} bayt sınır",
                scope.stdin_bytes, scope.stdin_limit
            );
            println!(
                "kalan dosya : {} ({} bayt){}",
                scope.inventory.len(),
                scope.inventory_total_bytes,
                if scope.inventory_truncated {
                    " · liste kesik"
                } else {
                    ""
                }
            );
            println!("sınır       : {:?}", report.timeout);
            println!("durum       : {:?}", report.outcome.status);
            println!("süre        : {} ms", report.outcome.duration_ms);
            println!("stdout      : {}", report.outcome.stdout_head.trim());
            if !report.outcome.stderr_head.trim().is_empty() {
                println!("stderr      : {}", report.outcome.stderr_head.trim());
            }
            println!(
                "kesildi     : stdout={} stderr={}",
                report.outcome.stdout_truncated, report.outcome.stderr_truncated
            );
            if !report.outcome.succeeded() {
                std::process::exit(1);
            }
        }
        Err(err) => {
            println!("hata        : {} — {}", err.code(), err);
            std::process::exit(1);
        }
    }
}
