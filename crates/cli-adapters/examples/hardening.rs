//! Tanı aracı: **sertleştirme davranış sınaması** (F10-T08) — gerçek istem
//! gönderir.
//!
//! ```text
//! cargo run -p dim-cli-adapters --example hardening -- claude-code
//! cargo run -p dim-cli-adapters --example hardening -- codex-cli antigravity-cli
//! ```
//!
//! Ürünün sabit istemi ([`dim_cli_adapters::probe_prompt`]) kurulu CLI'ya
//! gönderilir; kum havuzundaki `KANIT.txt` dosyasını okuyup okuyamadığı
//! **ölçülür**. `granted` = model dosyayı okudu (sertleştirme yetmiyor),
//! `blocked` = okuyamadığını söyledi, `inconclusive` = ölçüm sonuç vermedi.
//!
//! **Kota uyarısı:** bu araç gerçek model çağrısı yapar ve kullanıcının
//! aboneliğinden kota harcar. Kimlik dosyası okunmaz; `HOME` kum havuzuna
//! yönlendirilir, bu yüzden kimliğini ev dizininde tutan CLI oturum açamaz.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use dim_cli_adapters::{
    discover, resolve_adapter_executable, run_hardening_probe, spec, DiscoveryState, SPECS,
};

fn main() {
    let requested: Vec<String> = std::env::args().skip(1).collect();
    let ids: Vec<&str> = if requested.is_empty() {
        SPECS.iter().map(|s| s.id).collect()
    } else {
        requested.iter().map(String::as_str).collect()
    };
    let cancel = Arc::new(AtomicBool::new(false));

    for id in ids {
        let Some(adapter) = spec(id) else {
            println!("{id}: bilinmeyen adaptör");
            continue;
        };
        if adapter.batch_forbidden {
            println!("{id}: atlandı — toplu yürütme kapalı (istem gönderilmez)");
            continue;
        }
        let discovery = discover(adapter);
        let version = match &discovery.state {
            DiscoveryState::Installed { version, .. } => version.clone(),
            _ => {
                println!("{id}: kurulu değil");
                continue;
            }
        };
        let Some((_, executable)) = resolve_adapter_executable(adapter) else {
            println!("{id}: yürütülebilir bulunamadı");
            continue;
        };
        println!("== {id} ({version}) ==");
        match run_hardening_probe(adapter, executable, &cancel) {
            Ok(report) => {
                println!("argv     : {}", report.argv_template.join(" "));
                println!(
                    "durum    : {:?} · {} ms · kanıt {} bayt",
                    report.outcome.status, report.outcome.duration_ms, report.canary_bytes_after
                );
                // Ölçüm **yoksa** nedeni yazılır: "inconclusive" tek başına
                // kota/kimlik hatasını gizler (ölçüldü: agy 1.5.6 kotada 3).
                match report.not_run {
                    Some(kind) => {
                        println!("ölçüm    : yok — sınama koşulamadı");
                        println!(
                            "koşmadı  : {} · {}",
                            kind.id(),
                            report.not_run_reason.clone().unwrap_or_default()
                        );
                    }
                    None => println!("ölçüm    : {}", report.tool_access.id()),
                }
                println!(
                    "işaret   : kanıt={} ret={} yanıt={}",
                    report.canary_in_reply,
                    report.decline_in_reply,
                    report.answered()
                );
                println!("yanıt    : {}", report.reply_head.trim());
                println!(
                    "kum      : {} dosya · {} bayt",
                    report.outcome.scope.inventory.len(),
                    report.outcome.scope.inventory_total_bytes
                );
            }
            Err(err) => println!("hata     : {} — {err}", err.code()),
        }
        println!();
    }
}
