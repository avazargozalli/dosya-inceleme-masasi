use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::DomainError;
use crate::ids::{CaseId, DocumentVersionId, ParseRunId, SegmentId};

/// Metin aralığı **Unicode scalar** birimindedir; JavaScript UTF-16 seçimi
/// dönüşüm tablosuyla çevrilir (şartname 07.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Location {
    Pdf {
        page_index: u32,
        page_label: Option<String>,
        quads: Vec<[f32; 4]>,
        coordinate_space: CoordinateSpace,
    },
    Structure {
        part: String,
        element_path: String,
        paragraph_index: Option<u32>,
    },
    Image {
        frame_index: u32,
        quads: Vec<[f32; 4]>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateSpace {
    /// Döndürülmemiş sayfa noktası; render türevi değişse de kaynak sürüme bağlıdır.
    UnrotatedPagePoints,
}

/// Kaynak referansı: bir alıntının hangi sürümde, nerede ve hangi metinde
/// olduğunu taşır. Model bu yapıyı tahminle kuramaz; çekirdek doğrular.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub case_id: CaseId,
    pub document_version_id: DocumentVersionId,
    pub parse_run_id: ParseRunId,
    pub segment_id: SegmentId,
    pub location: Location,
    pub text_range: TextRange,
    pub exact_quote: String,
    pub quote_hash: String,
    pub prefix: String,
    pub suffix: String,
}

impl SourceRef {
    /// `start`, alıntının segment içindeki Unicode scalar başlangıcıdır.
    /// Bitiş, alıntının kendi uzunluğundan hesaplanır; çağıran farklı bir
    /// uzunluk veremez (aralık-alıntı tutarsızlığı yapısal olarak imkânsız).
    pub fn new(
        case_id: CaseId,
        document_version_id: DocumentVersionId,
        parse_run_id: ParseRunId,
        segment_id: SegmentId,
        location: Location,
        start: usize,
        exact_quote: String,
    ) -> Self {
        let quote_hash = hash_quote(&exact_quote);
        let text_range = TextRange {
            start,
            end: start + exact_quote.chars().count(),
        };
        Self {
            case_id,
            document_version_id,
            parse_run_id,
            segment_id,
            location,
            text_range,
            exact_quote,
            quote_hash,
            prefix: String::new(),
            suffix: String::new(),
        }
    }

    /// Yapısal doğrulama: hash içerikle, aralık uzunluğu alıntıyla uyuşmalı.
    pub fn validate(&self) -> Result<(), DomainError> {
        if hash_quote(&self.exact_quote) != self.quote_hash {
            return Err(DomainError::QuoteHashMismatch);
        }
        let chars = self.exact_quote.chars().count();
        if self.text_range.len() != chars {
            return Err(DomainError::TextRangeMismatch {
                expected: chars,
                actual: self.text_range.len(),
            });
        }
        match &self.location {
            Location::Pdf { quads, .. } => {
                if quads
                    .iter()
                    .any(|q| q.iter().any(|v| !v.is_finite() || *v < 0.0))
                {
                    return Err(DomainError::InvalidLocation("pdf_quad"));
                }
            }
            Location::Structure { element_path, .. } if element_path.is_empty() => {
                return Err(DomainError::InvalidLocation("empty_element_path"));
            }
            Location::Structure { .. } | Location::Image { .. } => {}
        }
        Ok(())
    }
}

/// Alıntı hash'i: SHA-256, küçük harf hex. Normalizasyon **yapılmaz**;
/// hash her zaman ham `exact_quote` üzerinden hesaplanır.
#[must_use]
pub fn hash_quote(quote: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(quote.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(quote: &str) -> SourceRef {
        SourceRef::new(
            CaseId::new(),
            DocumentVersionId::new(),
            ParseRunId::new(),
            SegmentId::new(),
            Location::Structure {
                part: "body".into(),
                element_path: "p[3]".into(),
                paragraph_index: Some(3),
            },
            0,
            quote.to_string(),
        )
    }

    #[test]
    fn turkish_characters_count_as_single_scalars() {
        let q = "İtirazın reddine karar verildi.";
        let r = sample(q);
        r.validate().expect("geçerli");
        assert_eq!(r.text_range.len(), q.chars().count());
        assert_ne!(r.text_range.len(), q.len(), "offset birimi bayt değildir");
    }

    #[test]
    fn tampered_quote_is_rejected() {
        let mut r = sample("Ödeme yapılmadı.");
        r.exact_quote = "Ödeme yapıldı.".into();
        assert_eq!(r.validate().unwrap_err().code(), "E_QUOTE_HASH");
    }

    #[test]
    fn range_must_match_quote_length() {
        let mut r = sample("Kira bedeli");
        r.text_range.end += 1;
        assert_eq!(r.validate().unwrap_err().code(), "E_QUOTE_RANGE");
    }

    #[test]
    fn non_finite_quad_rejected() {
        let mut r = sample("kira");
        r.location = Location::Pdf {
            page_index: 0,
            page_label: None,
            quads: vec![[0.0, f32::NAN, 1.0, 1.0]],
            coordinate_space: CoordinateSpace::UnrotatedPagePoints,
        };
        assert_eq!(r.validate().unwrap_err().code(), "E_LOCATION");
    }

    #[test]
    fn emoji_and_ligature_offsets_are_scalars() {
        let q = "Kanıt: 👩‍⚖️ ve ﬁş";
        sample(q).validate().expect("geçerli");
    }
}
