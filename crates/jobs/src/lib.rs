//! İş kuyruğu kuralları. Veritabanı erişimi yoktur; `jobs` tablosundaki
//! geçişler burada doğrulanır. Tek yazıcı ilkesi çağıran katmandadır.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Leased,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    RetryWait,
}

impl JobState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Leased => "leased",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::RetryWait => "retry_wait",
        }
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }

    /// İptal her aktif durumdan mümkündür; terminal durumdan geri dönüş yoktur.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        use JobState::{Cancelled, Failed, Leased, Queued, RetryWait, Running, Succeeded};
        matches!(
            (self, next),
            (Queued, Leased | Cancelled)
                | (Leased, Running | Queued | RetryWait | Cancelled)
                | (Running, Succeeded | Failed | RetryWait | Cancelled)
                | (RetryWait, Queued | Cancelled)
                | (Failed, RetryWait)
        )
    }

    pub fn transition(self, next: Self) -> Result<Self, JobError> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(JobError::InvalidTransition { from: self, to: next })
        }
    }
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
        Self { max_attempts: 3, base_delay_ms: 2_000, max_delay_ms: 300_000 }
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
        Self { owner: owner.into(), until }
    }

    #[must_use]
    pub fn is_expired(&self, now: OffsetDateTime) -> bool {
        now >= self.until
    }

    /// Uzatma: sahibi olmayan süreç kirayı yenileyemez.
    pub fn renew(&mut self, owner: &str, now: OffsetDateTime, ttl: Duration) -> Result<(), JobError> {
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

    #[test]
    fn terminal_states_do_not_move() {
        for state in [JobState::Succeeded, JobState::Failed, JobState::Cancelled] {
            assert!(state.is_terminal());
            assert!(!state.can_transition_to(JobState::Running));
        }
    }

    #[test]
    fn cancel_is_allowed_from_running() {
        assert_eq!(JobState::Running.transition(JobState::Cancelled).unwrap(), JobState::Cancelled);
    }

    #[test]
    fn succeeded_cannot_be_retried() {
        let err = JobState::Succeeded.transition(JobState::RetryWait).unwrap_err();
        assert_eq!(err.code(), "E_JOB_TRANSITION");
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        let policy = RetryPolicy { max_attempts: 5, base_delay_ms: 1_000, max_delay_ms: 8_000 };
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
        assert!(lease.renew("worker-b", now, Duration::from_secs(60)).is_err());

        let later = now + time::Duration::seconds(61);
        assert!(lease.is_expired(later));
        assert_eq!(lease.renew("worker-a", later, Duration::from_secs(60)).unwrap_err().code(), "E_JOB_LEASE_EXPIRED");
    }
}
