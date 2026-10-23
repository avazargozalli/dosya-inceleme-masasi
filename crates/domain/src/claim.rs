use serde::{Deserialize, Serialize};
use thiserror::Error;

/// İddia ile belge arasındaki ilişki türü. İlişkinin gerekçesi zorunludur;
/// "destekliyor" etiketi tek başına hukuki sonuç değildir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRelation {
    SubmittedFor,
    ResponseTo,
    SupportsCandidate,
    ContradictsCandidate,
    Context,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Pending,
    Accepted,
    Rejected,
    Edited,
    NeedsMoreEvidence,
}

/// İnceleme kararı geçişleri. Geçmiş eklemeli tutulur; kabul edilmiş bir kayıt
/// sessizce "pending"e döndürülemez.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransitionError {
    #[error("geçiş geçersiz: {0:?} -> {1:?}")]
    Invalid(ReviewState, ReviewState),
}

impl ReviewState {
    pub fn can_transition_to(self, next: Self) -> bool {
        use ReviewState::{Accepted, Edited, NeedsMoreEvidence, Pending, Rejected};
        matches!(
            (self, next),
            (Pending, Accepted | Rejected | Edited | NeedsMoreEvidence)
                | (NeedsMoreEvidence, Pending | Accepted | Rejected | Edited)
                | (Edited, Pending | Accepted | Rejected)
                | (Accepted, Edited)
                | (Rejected, Pending)
        )
    }

    pub fn transition(self, next: Self) -> Result<Self, TransitionError> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(TransitionError::Invalid(self, next))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_cannot_return_to_pending_silently() {
        assert!(ReviewState::Accepted.transition(ReviewState::Pending).is_err());
        assert_eq!(ReviewState::Accepted.transition(ReviewState::Edited).unwrap(), ReviewState::Edited);
    }

    #[test]
    fn pending_can_be_accepted() {
        assert_eq!(ReviewState::Pending.transition(ReviewState::Accepted).unwrap(), ReviewState::Accepted);
    }
}
