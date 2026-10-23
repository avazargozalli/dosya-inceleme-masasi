use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::DomainError;

macro_rules! id_type {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Yeni sıralanabilir kimlik (UUIDv7). Kimlikte taraf/esas/belge adı yoktur.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            #[must_use]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            pub fn parse(value: &str) -> Result<Self, DomainError> {
                Uuid::parse_str(value)
                    .map(Self)
                    .map_err(|_| DomainError::InvalidId($label))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = DomainError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::parse(s)
            }
        }
    };
}

id_type!(CaseId, "case_id");
id_type!(DocumentId, "document_id");
id_type!(DocumentVersionId, "document_version_id");
id_type!(ParseRunId, "parse_run_id");
id_type!(SegmentId, "segment_id");
id_type!(JobId, "job_id");
id_type!(ActorId, "actor_id");
id_type!(ClaimId, "claim_id");
id_type!(EventId, "event_id");
id_type!(NoteId, "note_id");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_v7_is_time_sortable() {
        let a = CaseId::new();
        let b = CaseId::new();
        assert!(a < b, "UUIDv7 artan sıralanabilir olmalı: {a} < {b}");
    }

    #[test]
    fn roundtrip_through_string() {
        let id = DocumentVersionId::new();
        let parsed = DocumentVersionId::parse(&id.to_string()).expect("geçerli kimlik");
        assert_eq!(id, parsed);
    }

    #[test]
    fn rejects_garbage_without_leaking_input() {
        let err = CaseId::parse("2024/1234 Esas").expect_err("geçersiz");
        assert_eq!(err.code(), "E_DOMAIN_ID");
        assert!(!err.to_string().contains("2024/1234"));
    }
}
