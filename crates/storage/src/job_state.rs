//! Şartname §17.2 iş durum makinesi — **veritabanı ayağının** kapısı.
//!
//! Tek kaynak `crates/job_state_machine.tsv`; aynı dosyayı `dim-jobs` (kural
//! crate'i) da okur. Böylece "kural bir yerde, tablo başka yerde" ayrışması
//! yapısal olarak imkânsızdır: tablo değişirse iki taraf da değişir.
//!
//! `jobs.state` CHECK kümesi ile bu tablo arasındaki uyum testle tutulur
//! (`queue::tests::schema_check_covers_every_spec_state`): tabloda olup
//! şemada olmayan bir durum yazılamaz hâle gelirse test düşer.

use std::sync::OnceLock;

/// §17.2 tablosu (11 durum). `dim-jobs` aynı dosyayı okur.
const STATE_TABLE: &str = include_str!("../../job_state_machine.tsv");

/// Sahiplenilebilir durumlar (yürütücü `running`e alır).
///
/// `retry_wait → queued → running` iki geçiştir; sahiplenme ikisini **tek
/// işlemde** yapar (tabloda `retry_wait → running` yok, bu yüzden burada ayrı
/// yazılı; sıra `job_attempts` satırıyla kanıtlanır).
pub const CLAIMABLE: &[&str] = &["queued", "retry_wait"];

/// Tablodaki durumlar ve izinli geçişleri (tanım sırasıyla).
#[must_use]
pub fn all_states() -> &'static [(&'static str, &'static [&'static str])] {
    static TABLE: OnceLock<Vec<(&'static str, &'static [&'static str])>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            STATE_TABLE
                .lines()
                .filter(|line| !line.trim_start().starts_with('#') && !line.trim().is_empty())
                .map(|line| {
                    let mut parts = line.split('\t');
                    let from = parts.next().unwrap_or_default().trim();
                    let nexts: Vec<&'static str> = parts
                        .next()
                        .unwrap_or_default()
                        .split(',')
                        .map(str::trim)
                        .filter(|part| !part.is_empty())
                        .collect();
                    (
                        from,
                        Box::leak(nexts.into_boxed_slice()) as &'static [&'static str],
                    )
                })
                .collect()
        })
        .as_slice()
}

/// Eski (0001–0020) satır adlarını yeni ada çevirir; bilinmeyen adda `None`.
///
/// Migrasyon 0021 satırları yeniden yazar; bu eşleme eski bir kasadan okunan
/// satır ya da eski `job_attempts` kaydı için savunma katmanıdır.
#[must_use]
pub fn canonical(state: &str) -> Option<&'static str> {
    let name = match state {
        "queued" => "queued",
        "blocked" => "blocked",
        "running" | "leased" => "running",
        "validating" => "validating",
        "retry_wait" => "retry_wait",
        "canceling" => "canceling",
        "interrupted" => "interrupted",
        "partial" => "partial",
        "completed" | "succeeded" => "completed",
        "failed" => "failed",
        "canceled" | "cancelled" => "canceled",
        _ => return None,
    };
    Some(name)
}

/// İzinli geçiş mi? `to` her zaman yeni adla yazılır; `from` eski adla
/// gelebilir (eski kasadan okunan satır).
#[must_use]
pub fn allowed(from: &str, to: &str) -> bool {
    let Some(from) = canonical(from) else {
        return false;
    };
    let Some(to) = canonical(to) else {
        return false;
    };
    transitions(from).is_some_and(|nexts| nexts.contains(&to))
}

#[must_use]
pub fn transitions(state: &str) -> Option<&'static [&'static str]> {
    let state = canonical(state)?;
    all_states()
        .iter()
        .find(|(name, _)| *name == state)
        .map(|(_, nexts)| *nexts)
}

/// Giden geçişi olmayan durum.
#[must_use]
pub fn is_terminal(state: &str) -> bool {
    transitions(state).is_some_and(<[_]>::is_empty)
}

/// `state` yeni adlarla yazılabilir mi (şema kapısı için).
#[must_use]
pub fn is_canonical(state: &str) -> bool {
    all_states().iter().any(|(name, _)| *name == state)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &[(&str, &[&str])] = &[
        ("queued", &["running", "blocked", "canceled"]),
        ("blocked", &["queued", "canceled"]),
        (
            "running",
            &[
                "validating",
                "blocked",
                "retry_wait",
                "interrupted",
                "failed",
                "canceling",
            ],
        ),
        (
            "validating",
            &["completed", "partial", "failed", "canceling"],
        ),
        ("retry_wait", &["queued", "failed", "canceled"]),
        ("canceling", &["canceled", "interrupted"]),
        ("interrupted", &["queued", "partial", "failed"]),
        ("partial", &[]),
        ("completed", &[]),
        ("failed", &["queued"]),
        ("canceled", &[]),
    ];

    #[test]
    fn table_matches_spec_section_17_2() {
        assert_eq!(all_states(), SPEC);
    }

    #[test]
    fn legacy_names_map_to_canonical_states() {
        assert_eq!(canonical("leased"), Some("running"));
        assert_eq!(canonical("succeeded"), Some("completed"));
        assert_eq!(canonical("cancelled"), Some("canceled"));
        assert_eq!(canonical("bogus"), None);
        // Eski satırın geçişleri yeni adın kurallarıyla denetlenir.
        assert!(allowed("leased", "validating"), "running → validating");
        assert!(!allowed("succeeded", "queued"), "completed terminal");
        assert!(!allowed("cancelled", "queued"), "canceled terminal");
    }

    #[test]
    fn claimable_states_are_canonical_and_not_terminal() {
        for state in CLAIMABLE {
            assert!(is_canonical(state), "{state} tabloda olmalı");
            assert!(!is_terminal(state), "{state} terminal olmamalı");
        }
        // Sahiplenme sonucu her zaman `running`; tablo bunu `queued` için açar.
        assert!(allowed("queued", "running"));
        assert!(allowed("retry_wait", "queued"));
    }

    #[test]
    fn unknown_states_are_never_allowed() {
        assert!(!allowed("queued", "bogus"));
        assert!(!allowed("bogus", "running"));
        assert!(!is_terminal("bogus"));
    }
}
