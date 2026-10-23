use thiserror::Error;

/// Alan katmanı hataları. Her varyant kararlı bir `code()` taşır; arayüz ve
/// günlükler bu kodu kullanır, ham hata metni kullanıcıya gösterilmez.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("kimlik biçimi geçersiz: {0}")]
    InvalidId(&'static str),

    #[error("alıntı hash'i içerikle uyuşmuyor")]
    QuoteHashMismatch,

    #[error("metin aralığı alıntı uzunluğuyla uyuşmuyor: beklenen {expected}, bulunan {actual}")]
    TextRangeMismatch { expected: usize, actual: usize },

    #[error("kaynak konumu bu belge türü için geçersiz: {0}")]
    InvalidLocation(&'static str),

    #[error("tarih değeri geçersiz: {0}")]
    InvalidDate(&'static str),

    #[error("tarih aralığı ters: başlangıç bitişten sonra")]
    ReversedRange,

    #[error("bilinmeyen tarih için sıralama değeri üretilemez")]
    UnknownDateNotSortable,

    #[error("kaynak başka bir dosyaya ait")]
    CrossCaseReference,

    #[error("durum geçişi geçersiz: {from} -> {to}")]
    InvalidTransition { from: &'static str, to: &'static str },
}

impl DomainError {
    /// Kararlı hata kodu (teşhis ve i18n anahtarı).
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidId(_) => "E_DOMAIN_ID",
            Self::QuoteHashMismatch => "E_QUOTE_HASH",
            Self::TextRangeMismatch { .. } => "E_QUOTE_RANGE",
            Self::InvalidLocation(_) => "E_LOCATION",
            Self::InvalidDate(_) => "E_DATE",
            Self::ReversedRange => "E_DATE_RANGE_REVERSED",
            Self::UnknownDateNotSortable => "E_DATE_UNKNOWN",
            Self::CrossCaseReference => "E_CASE_SCOPE",
            Self::InvalidTransition { .. } => "E_TRANSITION",
        }
    }
}

pub type DomainResult<T> = Result<T, DomainError>;
