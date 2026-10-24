//! Kalıntı ölçümü — gerçek CLI ile (F10-T13).
//!
//! Kullanım:
//! ```text
//! cargo run -p dim-cli-adapters --example residue -- [adaptör] [--cleanup]
//! ```
//!
//! Yol arayüzdeki "Kalıntıyı ölç" düğmesiyle **aynı** çekirdeği çalıştırır:
//! sürüm yoklaması boş kum havuzu iş dizininde, `HOME` kullanıcının kendi ev
//! dizini; önce/sonra anlık görüntüsü yalnız bildirilen yollarda karşılaştırılır.
//! İçerik açılmaz, kimlik dosyası okunmaz, istem gönderilmez.
//!
//! `--cleanup` yalnız **bu koşuda oluştuğu kanıtlanan** dosyaları siler;
//! önceden var olan hiçbir dosyaya dokunulmaz.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use dim_cli_adapters::{
    cleanup_created, find_on_path, measure_residue, persistence_control, residue_paths, spec,
    sweep_stale_sandboxes, STALE_SANDBOX_AGE,
};

fn main() {
    let mut adapter = "claude-code".to_string();
    let mut cleanup = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--cleanup" => cleanup = true,
            other => adapter = other.to_string(),
        }
    }
    let Some(spec) = spec(&adapter) else {
        eprintln!("bilinmeyen adaptör: {adapter}");
        std::process::exit(2);
    };
    let Some(executable) = find_on_path(spec.command) else {
        eprintln!("{} PATH üzerinde bulunamadı", spec.command);
        std::process::exit(3);
    };
    let home = PathBuf::from(std::env::var("HOME").expect("HOME"));
    let cancel = Arc::new(AtomicBool::new(false));
    let measurement = measure_residue(spec, &executable, &home, &cancel).expect("ölçüm");

    println!("adaptör: {}", measurement.adapter);
    println!("yürütülebilir: {}", executable.display());
    println!("argv: {}", measurement.args.join(" "));
    println!(
        "durum: {:?} · {} ms",
        measurement.status, measurement.duration_ms
    );
    let control = persistence_control(&adapter);
    println!(
        "kalıcılık kapatma: {}",
        match control.flag {
            Some(flag) => format!(
                "{flag} (yalnız {}; {} sürümünde --help ile doğrulandı)",
                control.applies_to,
                control.verified_with.unwrap_or("?")
            ),
            None => "doğrulanmış bayrak yok".to_string(),
        }
    );
    println!("--- bildirilen yollar (ölçüm öncesi) ---");
    for entry in &measurement.scan_before.entries {
        println!(
            "  {:<32} {:<11} var={:<5} dosya={:<4} bayt={:<9} not={}",
            entry.relative, entry.kind, entry.exists, entry.files, entry.bytes, entry.note
        );
    }
    println!(
        "  toplam: {} dosya · {} bayt · kesik={}",
        measurement.scan_before.total_files,
        measurement.scan_before.total_bytes,
        measurement.scan_before.truncated
    );
    println!("--- bu koşuda oluşan ---");
    for stamp in &measurement.delta.created {
        println!("  + {} ({} bayt)", stamp.relative, stamp.bytes);
    }
    for stamp in &measurement.delta.modified {
        println!("  ~ {} ({} bayt)", stamp.relative, stamp.bytes);
    }
    for relative in &measurement.delta.removed {
        println!("  - {relative}");
    }
    println!(
        "  oluşan {} dosya · {} bayt · dizin {} · değişen {}",
        measurement.delta.created.len(),
        measurement.delta.bytes_added,
        measurement.delta.created_dirs.len(),
        measurement.delta.modified.len()
    );

    if cleanup {
        let report = cleanup_created(&home, &adapter, &measurement.delta);
        println!(
            "--- temizlik: {} dosya · {} bayt · {} dizin · atlanan {} · hata {}",
            report.removed_files,
            report.removed_bytes,
            report.removed_dirs,
            report.skipped.len(),
            report.errors.len()
        );
        for (path, reason) in &report.skipped {
            println!("  atlandı: {path} — {reason}");
        }
        for (path, error) in &report.errors {
            println!("  hata: {path} — {error}");
        }
    } else {
        println!("--- temizlik yapılmadı (--cleanup verilmedi) ---");
    }

    let sweep = sweep_stale_sandboxes(
        &std::env::temp_dir(),
        STALE_SANDBOX_AGE.min(Duration::from_secs(0)),
        SystemTime::now(),
    );
    println!(
        "--- bekleyen kum havuzu: {} aday · {} silindi · {} atlandı",
        sweep.considered,
        sweep.removed.len(),
        sweep.skipped.len()
    );
    for (name, reason) in &sweep.skipped {
        println!("  atlandı: {name} — {reason}");
    }
    println!("bildirilen yol sayısı: {}", residue_paths(&adapter).len());
}
