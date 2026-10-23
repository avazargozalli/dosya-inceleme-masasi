//! Çekirdek alan modeli.
//!
//! Bu crate hiçbir G/Ç yapmaz: kimlik, tarih, kaynak konumu ve iddia kuralları
//! burada saf fonksiyonlar olarak durur ve birim testleriyle doğrulanır.

pub mod claim;
pub mod error;
pub mod ids;
pub mod legal_date;
pub mod source;

pub use error::{DomainError, DomainResult};
pub use ids::{CaseId, DocumentId, DocumentVersionId, JobId, ParseRunId, SegmentId};
pub use legal_date::{DatePrecision, DateRole, LegalDate};
pub use source::{Location, SourceRef, TextRange};
