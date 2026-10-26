//! İş kuyruğu kuralları. Veritabanı erişimi yoktur; `jobs` tablosundaki
//! geçişler burada doğrulanır. Tek yazıcı ilkesi çağıran katmandadır.
//!
//! Durum kümesi ve geçişler **şartname §17.2**'nin birebir karşılığıdır ve tek
//! kaynaktan (`crates/job_state_machine.tsv`) okunur: aynı dosyayı
//! `dim-storage` da okur, böylece kural ile veritabanı ayağı ayrışamaz.

use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// §17.2 tablosu; `dim-storage` aynı dosyayı okur.
const STATE_TABLE: &str = include_str!("../../job_state_machine.tsv");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Blocked,
    Running,
    Validating,
    RetryWait,
    Canceling,
    Interrupted,
    Partial,
    Completed,
    Failed,
    Canceled,
}

impl JobState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Blocked => "blocked",
            Self::Running => "running",
            Self::Validating => "validating",
            Self::RetryWait => "retry_wait",
            Self::Canceling => "canceling",
            Self::Interrupted => "interrupted",
            Self::Partial => "partial",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }

    /// Eski (0001–0020) satır adları. `0021` bunları yeniden yazar; eski bir
    /// kasadan gelen satır yine de okunabilsin diye eşleme burada durur.
    #[must_use]
    pub fn from_db(raw: &str) -> Option<Self> {
        match raw {
            "queued" => Some(Self::Queued),
            "blocked" => Some(Self::Blocked),
            "running" | "leased" => Some(Self::Running),
            "validating" => Some(Self::Validating),
            "retry_wait" => Some(Self::RetryWait),
            "canceling" => Some(Self::Canceling),
            "interrupted" => Some(Self::Interrupted),
            "partial" => Some(Self::Partial),
            "completed" | "succeeded" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "canceled" | "cancelled" => Some(Self::Canceled),
            _ => None,
        }
    }

    /// Terminal: giden geçişi olmayan durum.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        transitions_of(self.as_str()).is_some_and(<[_]>::is_empty)
    }

    /// Şartname tablosuna göre geçiş izni.
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        transitions_of(self.as_str()).is_some_and(|nexts| nexts.contains(&next.as_str()))
    }

    pub fn transition(self, next: Self) -> Result<Self, JobError> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(JobError::InvalidTransition {
                from: self,
                to: next,
            })
        }
    }
}

/// Tablodaki bütün durumlar (tanım sırasıyla).
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

fn transitions_of(state: &str) -> Option<&'static [&'static str]> {
    all_states()
        .iter()
        .find(|(name, _)| *name == state)
        .map(|(_, nexts)| *nexts)
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum JobError {
    #[error("durum geçişi geçersiz: {from:?} -> {to:?}")]
    InvalidTransition { from: JobState, to: JobState },
    #[error("kira sahibi uyuşmuyor")]
    LeaseOwnerMismatch,
    #[error("kira süresi dolmuş")]
    LeaseExpired,
    #[error("yeniden deneme hakkı bitti")]
    RetriesExhausted,
}

impl JobError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidTransition { .. } => "E_JOB_TRANSITION",
            Self::LeaseOwnerMismatch => "E_JOB_LEASE_OWNER",
            Self::LeaseExpired => "E_JOB_LEASE_EXPIRED",
            Self::RetriesExhausted => "E_JOB_RETRIES",
        }
    }
}

/// Üstel geri çekilme; üst sınırla. `attempt` 1 tabanlıdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 2_000,
            max_delay_ms: 300_000,
        }
    }
}

impl RetryPolicy {
    #[must_use]
    pub fn delay_for(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }
        let exponent = attempt.saturating_sub(1).min(16);
        let scaled = self.base_delay_ms.saturating_mul(1u64 << exponent);
        Duration::from_millis(scaled.min(self.max_delay_ms))
    }

    #[must_use]
    pub const fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }
}

/// İş kiralaması: aynı işi iki süreç aynı anda işlemesin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    pub owner: String,
    pub until: OffsetDateTime,
}

impl Lease {
    #[must_use]
    pub fn acquire(owner: impl Into<String>, now: OffsetDateTime, ttl: Duration) -> Self {
        let until = now + time::Duration::seconds(i64::try_from(ttl.as_secs()).unwrap_or(300));
        Self {
            owner: owner.into(),
            until,
        }
    }

    #[must_use]
    pub fn is_expired(&self, now: OffsetDateTime) -> bool {
        now >= self.until
    }

    /// Uzatma: sahibi olmayan süreç kirayı yenileyemez.
    pub fn renew(
        &mut self,
        owner: &str,
        now: OffsetDateTime,
        ttl: Duration,
    ) -> Result<(), JobError> {
        if self.owner != owner {
            return Err(JobError::LeaseOwnerMismatch);
        }
        if self.is_expired(now) {
            return Err(JobError::LeaseExpired);
        }
        self.until = now + time::Duration::seconds(i64::try_from(ttl.as_secs()).unwrap_or(300));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    /// Şartname §17.2 tablosunun birebir kopyası — kaynak dosya bozulursa ya da
    /// biri "düzeltmeye" kalkarsa test düşer.
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
        assert_eq!(all_states(), SPEC, "durum tablosu şartnameden ayrışmış");
    }

    #[test]
    fn every_transition_is_reachable_and_checked() {
        // 11 × 11 çiftinin tamamı tabloyla karşılaştırılır: eksik ya da fazla
        // bir geçiş sessizce kabul edilmez.
        for (from_name, nexts) in SPEC {
            let from = JobState::from_db(from_name).expect("durum");
            for (to_name, _) in SPEC {
                let to = JobState::from_db(to_name).expect("durum");
                assert_eq!(
                    from.can_transition_to(to),
                    nexts.contains(to_name),
                    "{from_name} -> {to_name}"
                );
            }
        }
    }

    #[test]
    fn terminal_states_have_no_exit() {
        for state in [JobState::Completed, JobState::Canceled, JobState::Partial] {
            assert!(state.is_terminal(), "{} terminal olmalı", state.as_str());
            assert!(!state.can_transition_to(JobState::Running));
        }
        // `failed` terminal sayılmaz: kullanıcı yeniden denemesi `queued` açar.
        assert!(!JobState::Failed.is_terminal());
        assert!(JobState::Failed.can_transition_to(JobState::Queued));
        assert!(!JobState::Failed.can_transition_to(JobState::Running));
    }

    #[test]
    fn spec_named_states_all_exist() {
        // §17.2'nin istediği adlar; 0001'in eski adları artık üretilmiyor.
        for name in [
            "queued",
            "blocked",
            "running",
            "validating",
            "retry_wait",
            "canceling",
            "interrupted",
            "partial",
            "completed",
            "failed",
            "canceled",
        ] {
            assert!(JobState::from_db(name).is_some(), "{name} tanınmalı");
        }
        // Eski satırlar okunabilir ama yeni adda yazılır.
        assert_eq!(JobState::from_db("leased"), Some(JobState::Running));
        assert_eq!(JobState::from_db("succeeded"), Some(JobState::Completed));
        assert_eq!(JobState::from_db("cancelled"), Some(JobState::Canceled));
    }

    #[test]
    fn cancel_paths_follow_the_table() {
        assert!(JobState::Queued.can_transition_to(JobState::Canceled));
        assert!(JobState::Running.can_transition_to(JobState::Canceling));
        assert!(JobState::Validating.can_transition_to(JobState::Canceling));
        assert!(JobState::Canceling.can_transition_to(JobState::Canceled));
        // Yürüyen iş doğrudan `canceled` olamaz: önce `canceling`.
        assert!(!JobState::Running.can_transition_to(JobState::Canceled));
        let err = JobState::Running
            .transition(JobState::Canceled)
            .unwrap_err();
        assert_eq!(err.code(), "E_JOB_TRANSITION");
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        let policy = RetryPolicy {
            max_attempts: 5,
            base_delay_ms: 1_000,
            max_delay_ms: 8_000,
        };
        assert_eq!(policy.delay_for(1), Duration::from_millis(1_000));
        assert_eq!(policy.delay_for(2), Duration::from_millis(2_000));
        assert_eq!(policy.delay_for(3), Duration::from_millis(4_000));
        assert_eq!(policy.delay_for(4), Duration::from_millis(8_000));
        assert_eq!(policy.delay_for(9), Duration::from_millis(8_000));
        assert!(policy.should_retry(4));
        assert!(!policy.should_retry(5));
    }

    #[test]
    fn lease_expires_and_only_owner_can_renew() {
        let now = datetime!(2026-01-01 12:00 UTC);
        let mut lease = Lease::acquire("worker-a", now, Duration::from_secs(60));
        assert!(!lease.is_expired(now + time::Duration::seconds(30)));
        assert!(lease
            .renew("worker-b", now, Duration::from_secs(60))
            .is_err());

        let later = now + time::Duration::seconds(61);
        assert!(lease.is_expired(later));
        assert_eq!(
            lease
                .renew("worker-a", later, Duration::from_secs(60))
                .unwrap_err()
                .code(),
            "E_JOB_LEASE_EXPIRED"
        );
    }
}
