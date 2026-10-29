//! Alıntı doğrulama. Modelin döndürdüğü alıntı, gerçek segment metninde
//! bulunmadan kaynak sayılmaz (INV-02, INV-12).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteMatch {
    /// Birebir, aynı konumda.
    Exact,
    /// Aynı konumda ama yalnız normalizasyon farkı var (OCR/boşluk).
    Normalized,
    /// Birden fazla yerde geçiyor; hangisi kastedildiği belirsiz.
    Ambiguous,
    /// Metinde yok.
    NotFound,
    /// Kayıtlı hash ile alıntı uyuşmuyor (kurcalama).
    HashMismatch,
}

impl QuoteMatch {
    #[must_use]
    pub const fn is_usable(self) -> bool {
        matches!(self, Self::Exact | Self::Normalized)
    }
}

#[must_use]
pub fn hash_quote(quote: &str) -> String {
    hex::encode(Sha256::digest(quote.as_bytes()))
}

/// Segment içinde alıntıyı **Unicode scalar** konumunda arar.
/// `start_hint` verilirse önce o konum denenir; tutmazsa tüm metin taranır.
#[must_use]
pub fn verify_quote(
    segment: &str,
    quote: &str,
    expected_hash: Option<&str>,
    start_hint: Option<usize>,
) -> QuoteMatch {
    if let Some(expected) = expected_hash {
        if hash_quote(quote) != expected {
            return QuoteMatch::HashMismatch;
        }
    }
    if quote.is_empty() {
        return QuoteMatch::NotFound;
    }

    let scalars: Vec<char> = segment.chars().collect();
    let q: Vec<char> = quote.chars().collect();

    if let Some(start) = start_hint {
        if start <= scalars.len()
            && scalars.len() - start >= q.len()
            && scalars[start..start + q.len()] == q[..]
        {
            return QuoteMatch::Exact;
        }
    }

    let mut hits = 0usize;
    if scalars.len() >= q.len() {
        for window in scalars.windows(q.len()) {
            if window == q.as_slice() {
                hits += 1;
            }
        }
    }
    match hits {
        0 => {
            if normalize(segment).contains(&normalize(quote)) {
                QuoteMatch::Normalized
            } else {
                QuoteMatch::NotFound
            }
        }
        1 => QuoteMatch::Exact,
        _ => QuoteMatch::Ambiguous,
    }
}

/// Bitişik yazı (ligatür) karakterinin harf karşılığı.
///
/// PDF fontları `fi`/`fl` gibi çiftleri **tek glif** olarak yazar; metin
/// katmanından `U+FB01`/`U+FB02` döner. Bu bir yazım farkı değil **sunum**
/// artefaktıdır: `taraﬂar` ile `taraflar` aynı kelimedir. Ligatür açılmazsa
/// avukatın yazdığı alıntı `fiili`, `tasfiye`, `taraflar` gibi kelimelerde
/// sessizce "bulunamadı" olur (ölçüldü: gerçek PDF, F12).
#[must_use]
pub fn ligature_expansion(ch: char) -> Option<&'static str> {
    match ch {
        '\u{FB00}' => Some("ff"),
        '\u{FB01}' => Some("fi"),
        '\u{FB02}' => Some("fl"),
        '\u{FB03}' => Some("ffi"),
        '\u{FB04}' => Some("ffl"),
        '\u{FB05}' => Some("ft"),
        '\u{FB06}' => Some("st"),
        _ => None,
    }
}

/// Metni karşılaştırma için sadeleştirir: Türkçe harfleri ASCII'ye indirger,
/// şapkalı harfleri sadeleştirir, boşlukları tekler ve **ligatürleri açar**.
/// Kaynak alıntı gösterimi ham metinden yapılır; bu fonksiyon yalnız
/// eşleştirme içindir.
#[must_use]
pub fn normalize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_space = false;
    for ch in input.chars() {
        if let Some(expansion) = ligature_expansion(ch) {
            for letter in expansion.chars() {
                out.push(letter);
            }
            last_space = false;
            continue;
        }
        let mapped = match ch {
            'İ' | 'I' => 'i',
            'ı' => 'i',
            'Ş' | 'ş' => 's',
            'Ğ' | 'ğ' => 'g',
            'Ü' | 'ü' => 'u',
            'Ö' | 'ö' => 'o',
            'Ç' | 'ç' => 'c',
            'Â' | 'â' | 'Î' | 'î' | 'Û' | 'û' => {
                // Şapkalı harfler sadeleşir; ayrı karakter üretilmez.
                match ch {
                    'Â' | 'â' => 'a',
                    'Î' | 'î' => 'i',
                    _ => 'u',
                }
            }
            c => c.to_lowercase().next().unwrap_or(c),
        };
        if mapped.is_whitespace() {
            if !last_space && !out.is_empty() {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(mapped);
            last_space = false;
        }
    }
    out.trim_end().to_string()
}

/// Kaynaklanan alıntının çevre metnini üretir (yeniden bağlama için).
#[must_use]
pub fn context_around(segment: &str, start: usize, end: usize, radius: usize) -> (String, String) {
    let scalars: Vec<char> = segment.chars().collect();
    let s = start.min(scalars.len());
    let e = end.min(scalars.len());
    let pre_start = s.saturating_sub(radius);
    let post_end = (e + radius).min(scalars.len());
    (
        scalars[pre_start..s].iter().collect(),
        scalars[e..post_end].iter().collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEG: &str =
        "Mahkeme, itirazın reddine karar verdi. Gerekçede ödeme yapılmadığı belirtildi.";

    #[test]
    fn exact_quote_matches_at_hint() {
        let quote = "itirazın reddine karar verdi";
        assert_eq!(verify_quote(SEG, quote, None, Some(9)), QuoteMatch::Exact);
    }

    #[test]
    fn repeated_quote_is_ambiguous() {
        let text = "ödeme yapılmadı. ödeme yapılmadı.";
        assert_eq!(
            verify_quote(text, "ödeme yapılmadı", None, None),
            QuoteMatch::Ambiguous
        );
    }

    #[test]
    fn missing_quote_is_not_found() {
        assert_eq!(
            verify_quote(SEG, "tamamen uydurma bir cümle", None, None),
            QuoteMatch::NotFound
        );
    }

    #[test]
    fn hash_mismatch_wins_over_text_match() {
        let quote = "itirazın reddine karar verdi";
        assert_eq!(
            verify_quote(SEG, quote, Some(&hash_quote("başka metin")), None),
            QuoteMatch::HashMismatch
        );
        assert!(verify_quote(SEG, quote, Some(&hash_quote(quote)), None).is_usable());
    }

    #[test]
    fn whitespace_and_case_normalization_is_accepted() {
        let text = "ÖDEME   YAPILMADI";
        assert_eq!(
            verify_quote(text, "ödeme yapılmadı", None, None),
            QuoteMatch::Normalized
        );
    }

    #[test]
    fn turkish_diacritics_fold_for_search() {
        assert_eq!(
            normalize("İtirazın REDDİNE karar"),
            "itirazin reddine karar"
        );
        assert_eq!(normalize("Şişli'deki ğ harfi"), "sisli'deki g harfi");
    }

    #[test]
    fn ligatures_from_the_pdf_font_are_expanded() {
        // Font `fl`/`fi` çiftini tek glif yazar; metin katmanından U+FB01/02
        // döner. Bu bir yazım farkı değildir: eşleştirmede açılmalı.
        assert_eq!(normalize("taraﬂar"), "taraflar");
        assert_eq!(normalize("ﬁili tasfiye"), "fiili tasfiye");
        assert_eq!(normalize("aﬀedilen"), "affedilen");
        // Uçtan uca: alıntı düz harflerle yazılmış, kaynak ligatürlü.
        assert_eq!(
            verify_quote(
                "Sözleşme taraﬂar arasında kurulmuştur",
                "taraflar arasında kurulmuştur",
                None,
                None
            ),
            QuoteMatch::Normalized
        );
    }

    #[test]
    fn context_is_unicode_safe() {
        let (before, after) = context_around("abcdef", 2, 3, 1);
        assert_eq!(before, "b");
        assert_eq!(after, "d");
    }
}
