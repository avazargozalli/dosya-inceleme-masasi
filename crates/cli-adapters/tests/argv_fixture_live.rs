//! **Canlı argv fixture doğrulaması** (F10-T08i) — yalnız bu makinede,
//! `--ignored` ile koşulur. CI'da CLI kurulu değildir; bu yüzden test
//! varsayılan koşuda atlanır.
//!
//! Ölçtüğü üç şey:
//! 1. Kurulu CLI'nin `--version` çıktısı fixture'ın `verified_version`ıyla
//!    **aynı sürüm anahtarını** veriyor mu (fixture taze mi?),
//! 2. Fixture'ın her bayrağı kurulu sürümün `--help` metninde **görünüyor** mu
//!    (bayrak o sürümde var mı?),
//! 3. `--help` senaryosunun **ürün argv'si** gerçekten koşuyor mu (ürün yolu).
//!
//! İstem senaryosu **koşulmaz**: gerçek model çağrısı kota harcar. Onun
//! doğrulaması bayrakların yardım metninde görünmesi + sürüm eşleşmesidir;
//! davranış kanıtı F10-T08 sınamasının (araç erişimi) işidir.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use dim_cli_adapters::{
    capabilities, discover, fixture, help_text_of, inspect_path, resolve_adapter_executable,
    run_help_probe, spec, version_key, DiscoveryState, SCENARIO_HELP, SCENARIO_PROMPT,
    SCENARIO_VERSION,
};

#[test]
#[ignore = "kurulu CLI gerekir: kargo test -p dim-cli-adapters --test argv_fixture_live -- --ignored --nocapture"]
fn installed_clis_match_the_checked_in_argv_fixtures() {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut checked = 0;
    for adapter in dim_cli_adapters::SPECS {
        // Sürüm **ürünün kendi yoklamasıyla** okunur (`--version`); test kendi
        // süreç başlatma yolunu kurmaz.
        let raw_version = match discover(adapter).state {
            DiscoveryState::Installed { version, .. } => version,
            other => {
                println!(
                    "{}: kurulu değil ({other:?}) — fixture doğrulanamadı",
                    adapter.id
                );
                continue;
            }
        };
        let Some((resolved, path)) = resolve_adapter_executable(adapter) else {
            println!("{}: yürütülebilir çözülemedi", adapter.id);
            continue;
        };
        let installed_key = version_key(&raw_version);
        println!(
            "{}: {resolved} → {raw_version} (anahtar {installed_key:?})",
            adapter.id
        );

        // 1) Sürüm: fixture hangi sürümde doğrulandı?
        let version_fixture = fixture(adapter.id, SCENARIO_VERSION).expect("sürüm fixture'ı");
        assert_eq!(
            version_key(version_fixture.verified_version),
            installed_key,
            "{}: fixture sürümü kurulu sürümle eşleşmiyor",
            adapter.id
        );

        // 2) Yardım: fixture'ın her bayrağı bu sürümün yardım metninde var mı?
        let caps = capabilities(adapter.id).expect("yetenek satırı");
        if caps.help_args.is_empty() {
            println!(
                "{}: yardım yoklaması kapalı — bayrak kanıtı atlandı",
                adapter.id
            );
            continue;
        }
        let probe = run_help_probe(adapter, &path, &cancel).expect("yardım yoklaması");
        let help = match help_text_of(&probe) {
            Ok(text) => text.to_string(),
            Err(reason) => {
                println!(
                    "{}: yardım okunamadı ({reason}) — bayrak kanıtı atlandı",
                    adapter.id
                );
                continue;
            }
        };
        for scenario in [SCENARIO_HELP, SCENARIO_PROMPT] {
            let Some(fixture) = fixture(adapter.id, scenario) else {
                continue;
            };
            for token in fixture.argv {
                if token.is_empty() || *token == dim_cli_adapters::PROMPT_PLACEHOLDER {
                    continue;
                }
                let flag = token.split_once('=').map_or(*token, |(flag, _)| flag);
                // Ortak sürüm bayrakları (`--help`, `--version`) yardım
                // metninde **anılmaz**: kanıtları koşunun kendisidir.
                if spec(adapter.id).is_some_and(|spec| spec.version_args.contains(&flag)) {
                    continue;
                }
                assert!(
                    help.contains(flag),
                    "{} {scenario}: {flag} kurulu sürümün yardım metninde yok",
                    adapter.id
                );
            }
        }

        // 3) Yardım senaryosunun ürün argv'si gerçekten koştu mu?
        let help_fixture = fixture(adapter.id, SCENARIO_HELP).expect("yardım fixture'ı");
        assert_eq!(
            probe.args,
            help_fixture
                .caller_args
                .iter()
                .map(|arg| (*arg).to_string())
                .collect::<Vec<_>>(),
            "{}: ürünün yardım argümanları fixture'dan koptu",
            adapter.id
        );
        assert!(
            probe.status == dim_cli_adapters::RunStatus::Exited(0),
            "{}: yardım koşusu çıkış kodu {:?}",
            adapter.id,
            probe.status
        );
        let arch = inspect_path(&path);
        println!(
            "{}: fixture {} · {} belirteç yardımda görüldü · mimari {}",
            adapter.id,
            version_fixture.verified_version,
            help_fixture
                .argv
                .iter()
                .filter(|token| !token.is_empty())
                .count(),
            arch.summary()
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "hiçbir CLI kurulu değil: canlı doğrulama yapılamadı"
    );
}
