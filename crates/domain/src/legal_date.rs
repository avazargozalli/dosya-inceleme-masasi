use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};

use crate::error::DomainError;

/// Hukuki tarihin rolü. Bir olay tarihi ile belge tarihi ve tebliğ tarihi
/// birbirinin yerine geçmez (REQ-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateRole {
    Event,
    Document,
    Notification,
    Contract,
    Hearing,
    DeadlineCandidate,
    Unknown,
}

/// Tarihin ne kadar kesin olduğu. Eksik gün **uydurulmaz**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatePrecision {
    Day,
    Month,
    Year,
    Range,
    Approximate,
    Unknown,
}

/// Hukuki tarih değeri.
///
/// `date_only` UTC'ye çevrilmez; gün kayması üretilmemesi için `Date` olarak tutulur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LegalDate {
    DateOnly { value: Date },
    DateTime { value: OffsetDateTime },
    Range { from: Date, to: Date },
    Approximate { value: Date, note: Option<String> },
    Unknown,
}

impl LegalDate {
    /// Sıralama için türev değer. Bilinmeyen tarih `None` döner ve sıralamaya
    /// zorla sokulmaz; arayüzde ayrı grupta gösterilir.
    #[must_use]
    pub fn sortable_date(&self) -> Option<Date> {
        match self {
            Self::DateOnly { value } | Self::Approximate { value, .. } => Some(*value),
            Self::DateTime { value } => Some(value.date()),
            Self::Range { from, .. } => Some(*from),
            Self::Unknown => None,
        }
    }

    #[must_use]
    pub const fn precision(&self) -> DatePrecision {
        match self {
            Self::DateOnly { .. } => DatePrecision::Day,
            Self::DateTime { .. } => DatePrecision::Day,
            Self::Range { .. } => DatePrecision::Range,
            Self::Approximate { .. } => DatePrecision::Approximate,
            Self::Unknown => DatePrecision::Unknown,
        }
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        if let Self::Range { from, to } = self {
            if from > to {
                return Err(DomainError::ReversedRange);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn date_only_keeps_calendar_day() {
        let d = LegalDate::DateOnly { value: date!(2024 - 03 - 31) };
        assert_eq!(d.sortable_date(), Some(date!(2024 - 03 - 31)));
        assert_eq!(d.precision(), DatePrecision::Day);
    }

    #[test]
    fn unknown_is_not_sorted_into_timeline() {
        assert_eq!(LegalDate::Unknown.sortable_date(), None);
        assert_eq!(LegalDate::Unknown.precision(), DatePrecision::Unknown);
    }

    #[test]
    fn reversed_range_rejected() {
        let d = LegalDate::Range { from: date!(2025 - 01 - 02), to: date!(2025 - 01 - 01) };
        assert_eq!(d.validate().unwrap_err().code(), "E_DATE_RANGE_REVERSED");
    }
}
