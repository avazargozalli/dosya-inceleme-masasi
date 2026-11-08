//! Gönderim maskesi: metni yer tutucuyla maskeleyip **özgün aralık
//! eşlemesini** birlikte üretir.
//!
//! Şartname §737 (maskeli bağlamın iki aşamalı doğrulaması): «Her gönderim
//! segmenti, gönderilen metin/hash, maskeleme dönüşüm sürümü ve özgün Unicode
//! aralıklarına eşleme taşır. Modelin alıntısı önce gördüğü maskeli kopyada
//! doğrulanır. Sonra yalnız kasada bulunan eşleme, ilgili özgün segment/sürüme
//! bağlanır. Maske hiç uygulanmadıysa eşleme özdeşliktir. Silinen alan, takma
//! ad veya birden fazla kaynak aralığı varsa dönüşüm tipi açık tutulur; geri
//! eşlenemeyen alıntı doğrulanmış sayılmaz.»
//!
//! Bu modül **saf** çalışır (veritabanı yok): segment metni + terim listesi →
//! maskeli metin + aralık haritası. Harita, maskelenen her aralığı da taşır;
//! böylece maskenin içine düşen bir alıntı "geri eşlenemedi" diye açıkça
//! reddedilir (uydurma bir konuma bağlanmaz).

use serde::{Deserialize, Serialize};

/// Gönderilen metinde maskelenen aralığın yerine yazılan **sabit** yer tutucu.
/// Değiştirilebilir olması, aynı metnin iki farklı sürümünü ayırt etmeyi
/// zorlaştırırdı; sürüm `v1` yer tutucusuyla tanımlıdır.
pub const MASK_PLACEHOLDER: &str = "[MASKED]";

/// Terim listesi sınırları. Üst sınır, maskeyi anlamsız kılacak kadar geniş
/// listeleri (ör. tek harfli terimlerle metnin tümünü silmeyi) engeller.
pub const MAX_MASK_TERMS: usize = 500;
pub const MAX_MASK_TERM_CHARS: usize = 200;
pub const MIN_MASK_TERM_CHARS: usize = 2;

/// Maskesi reddedilen girdi. Çağıran bunu kullanıcıya **açık** hata olarak
/// taşır; sessizce maskesiz gönderim yapılmaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskError {
    /// Terim boş, çok kısa/uzun ya da kontrol karakteri taşıyor.
    InvalidTerm,
    /// Terim sayısı sınırın üstünde.
    TooManyTerms,
    /// Segment metni yer tutucunun kendisini içeriyor: maske ile özgün metin
    /// ayırt edilemez.
    PlaceholderPresent,
    /// Aralık haritası okunamadı (bozuk kayıt).
    InvalidMap,
}

impl MaskError {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            MaskError::InvalidTerm => "E_MASK_TERM",
            MaskError::TooManyTerms => "E_MASK_TERMS_LIMIT",
            MaskError::PlaceholderPresent => "E_MASK_PLACEHOLDER",
            MaskError::InvalidMap => "E_MASK_MAP",
        }
    }
}

/// Eşlemenin bir parçası: gönderilen metindeki `out_start..out_end`
/// (Unicode scalar) aralığı, özgün metindeki `source_start..source_end`
/// aralığına karşılık gelir. `masked` ise aradaki metin **yer tutucudur**:
/// bu aralığa düşen alıntı özgün kaynağa geri eşlenemez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaskRun {
    pub out_start: usize,
    pub out_end: usize,
    pub source_start: usize,
    pub source_end: usize,
    #[serde(default)]
    pub masked: bool,
}

/// Maskelenmiş metin ve haritası.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskedText {
    pub text: String,
    /// Metnin **tamamını** kesintisiz kaplayan harita (kopya + maske).
    pub runs: Vec<MaskRun>,
    /// Yazılan yer tutucu sayısı (komşu eşleşmeler tek yer tutucuda birleşir).
    pub masked_runs: usize,
    /// Yer tutucuyla değiştirilen **özgün** karakter sayısı.
    pub masked_chars: usize,
}

impl MaskedText {
    /// Hiçbir maske uygulanmadı: eşleme **özdeşliktir** (şartname: «Maske hiç
    /// uygulanmadıysa eşleme özdeşliktir»).
    #[must_use]
    pub fn identity(raw: &str) -> Self {
        let len = raw.chars().count();
        Self {
            text: raw.to_string(),
            runs: if len == 0 {
                Vec::new()
            } else {
                vec![MaskRun {
                    out_start: 0,
                    out_end: len,
                    source_start: 0,
                    source_end: len,
                    masked: false,
                }]
            },
            masked_runs: 0,
            masked_chars: 0,
        }
    }

    /// Gönderilen metindeki bir aralığı özgün karakter aralığına çevirir.
    ///
    /// `None` — aralık bir maske parçasına değiyor, metin dışına taşıyor ya da
    /// harita kesintili. Şartname gereği bu alıntı **doğrulanmış sayılmaz**;
    /// çağıran uydurma bir konum üretmemelidir.
    #[must_use]
    pub fn map_span(&self, out_start: usize, out_end: usize) -> Option<(usize, usize)> {
        map_span_in_runs(&self.runs, out_start, out_end)
    }

    /// Haritanın kesintisiz ve artan olduğunu denetler (okunan kayıt için kapı).
    #[must_use]
    pub fn runs_are_sound(runs: &[MaskRun], out_len: usize, source_len: usize) -> bool {
        if runs.is_empty() {
            return out_len == 0 && source_len == 0;
        }
        let mut out_cursor = 0usize;
        let mut source_cursor = 0usize;
        for run in runs {
            if run.out_start != out_cursor || run.source_start != source_cursor {
                return false;
            }
            let Some(out_width) = run.out_end.checked_sub(run.out_start) else {
                return false;
            };
            let Some(source_width) = run.source_end.checked_sub(run.source_start) else {
                return false;
            };
            if out_width == 0 || source_width == 0 {
                return false;
            }
            if !run.masked && out_width != source_width {
                // Kopya parçası birebir olmalı: yoksa eşleme kanıtsızdır.
                return false;
            }
            out_cursor = run.out_end;
            source_cursor = run.source_end;
        }
        out_cursor == out_len && source_cursor == source_len
    }
}

/// Haritadaki bir aralığı özgün karakter aralığına çevirir (`MaskedText`
/// olmadan, yalnız parçalarla çalışır — doğrulama yolu bunu kullanır).
///
/// `None`: aralık maske parçasına değiyor, metin dışına taşıyor ya da harita
/// kesintili. Bu durumda alıntı **doğrulanmış sayılmaz**.
#[must_use]
pub fn map_span_in_runs(
    runs: &[MaskRun],
    out_start: usize,
    out_end: usize,
) -> Option<(usize, usize)> {
    if out_end <= out_start {
        return None;
    }
    let run = runs
        .iter()
        .find(|run| out_start >= run.out_start && out_end <= run.out_end)?;
    if run.masked {
        return None;
    }
    let offset = out_start.checked_sub(run.out_start)?;
    let length = out_end.checked_sub(out_start)?;
    let source_start = run.source_start.checked_add(offset)?;
    let source_end = source_start.checked_add(length)?;
    if source_end > run.source_end {
        return None;
    }
    Some((source_start, source_end))
}

/// Türkçe kurallı, **1:1** harf katlaması.
///
/// `char::to_lowercase` bazı harflerde iki karakter üretir (`İ` → `i` +
/// birleşik nokta) ve aralık eşlemesini kaydırır. Bu yüzden katlama tek
/// karakter eşlemesiyle yapılır: `I`→`ı`, `İ`→`i`, `Ş`→`ş`, `Ğ`→`ğ`, `Ü`→`ü`,
/// `Ö`→`ö`, `Ç`→`ç`, ASCII büyük harfler küçüğe. Diğer karakterler değişmez.
#[must_use]
pub fn fold_char(c: char) -> char {
    match c {
        'I' => 'ı',
        'İ' => 'i',
        'Ş' => 'ş',
        'Ğ' => 'ğ',
        'Ü' => 'ü',
        'Ö' => 'ö',
        'Ç' => 'ç',
        'Â' => 'â',
        'Î' => 'î',
        'Û' => 'û',
        other => other
            .to_ascii_lowercase()
            .to_lowercase()
            .next()
            .unwrap_or(other),
    }
}

/// Terim listesini doğrular ve **kanonik** biçime getirir: kırpılmış, kontrol
/// karakteri taşımayan, tekilleştirilmiş (katlanmış biçimde), uzundan kısaya
/// sıralı. Sıra, çakışan terimlerde **uzun** olanın kazanmasını sağlar.
pub fn validate_terms(terms: &[String]) -> Result<Vec<String>, MaskError> {
    if terms.len() > MAX_MASK_TERMS {
        return Err(MaskError::TooManyTerms);
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for term in terms {
        let trimmed = term.trim();
        if trimmed.chars().any(|c| c.is_control() || c == '\u{0}') {
            return Err(MaskError::InvalidTerm);
        }
        let length = trimmed.chars().count();
        if !(MIN_MASK_TERM_CHARS..=MAX_MASK_TERM_CHARS).contains(&length) {
            return Err(MaskError::InvalidTerm);
        }
        if trimmed.contains(MASK_PLACEHOLDER) {
            return Err(MaskError::PlaceholderPresent);
        }
        let folded: String = trimmed.chars().map(fold_char).collect();
        if out.iter().any(|(key, _)| key == &folded) {
            continue;
        }
        out.push((folded, trimmed.to_string()));
    }
    if out.is_empty() {
        return Err(MaskError::InvalidTerm);
    }
    out.sort_by(|a, b| {
        b.0.chars()
            .count()
            .cmp(&a.0.chars().count())
            .then_with(|| a.0.cmp(&b.0))
    });
    Ok(out.into_iter().map(|(_, term)| term).collect())
}

/// Metni terimlere göre maskeler ve özgün aralık haritasını üretir.
///
/// `terms` **doğrulanmış** olmalı (`validate_terms`). Boş liste özdeşliktir.
pub fn mask_terms(raw: &str, terms: &[String]) -> Result<MaskedText, MaskError> {
    if raw.contains(MASK_PLACEHOLDER) {
        return Err(MaskError::PlaceholderPresent);
    }
    if terms.is_empty() {
        return Ok(MaskedText::identity(raw));
    }
    let chars: Vec<char> = raw.chars().collect();
    let folded: Vec<char> = chars.iter().copied().map(fold_char).collect();

    // Eşleşmeleri karakter aralığı olarak topla.
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for term in terms {
        let needle: Vec<char> = term.chars().map(fold_char).collect();
        if needle.is_empty() || needle.len() > folded.len() {
            continue;
        }
        let mut index = 0usize;
        while index + needle.len() <= folded.len() {
            if folded[index..index + needle.len()] == needle[..] {
                spans.push((index, index + needle.len()));
                index += needle.len();
            } else {
                index += 1;
            }
        }
    }
    if spans.is_empty() {
        return Ok(MaskedText {
            text: raw.to_string(),
            runs: MaskedText::identity(raw).runs,
            masked_runs: 0,
            masked_chars: 0,
        });
    }
    spans.sort_unstable();
    // Çakışan/komşu aralıkları tek maskeye birleştir: ardışık iki yer tutucu
    // metni okunamaz hâle getirirdi.
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in spans {
        match merged.last_mut() {
            Some(last) if start <= last.1 => {
                if end > last.1 {
                    last.1 = end;
                }
            }
            _ => merged.push((start, end)),
        }
    }

    let mut text = String::with_capacity(raw.len());
    let mut runs: Vec<MaskRun> = Vec::with_capacity(merged.len() * 2 + 1);
    let mut masked_chars = 0usize;
    let mut out_cursor = 0usize;
    let mut source_cursor = 0usize;
    for (start, end) in &merged {
        if *start > source_cursor {
            for c in &chars[source_cursor..*start] {
                text.push(*c);
            }
            runs.push(MaskRun {
                out_start: out_cursor,
                out_end: out_cursor + (start - source_cursor),
                source_start: source_cursor,
                source_end: *start,
                masked: false,
            });
            out_cursor += start - source_cursor;
        }
        text.push_str(MASK_PLACEHOLDER);
        let placeholder_chars = MASK_PLACEHOLDER.chars().count();
        runs.push(MaskRun {
            out_start: out_cursor,
            out_end: out_cursor + placeholder_chars,
            source_start: *start,
            source_end: *end,
            masked: true,
        });
        out_cursor += placeholder_chars;
        masked_chars += end - start;
        source_cursor = *end;
    }
    if source_cursor < chars.len() {
        for c in &chars[source_cursor..] {
            text.push(*c);
        }
        runs.push(MaskRun {
            out_start: out_cursor,
            out_end: out_cursor + (chars.len() - source_cursor),
            source_start: source_cursor,
            source_end: chars.len(),
            masked: false,
        });
        out_cursor += chars.len() - source_cursor;
    }
    let out_len = text.chars().count();
    debug_assert_eq!(out_cursor, out_len);
    debug_assert_eq!(runs.last().map(|run| run.out_end), Some(out_len));
    Ok(MaskedText {
        text,
        runs,
        masked_runs: merged.len(),
        masked_chars,
    })
}

/// Haritayı JSON'dan okur; bozuk/kesintili kayıt `Err` döner (sessizce boş
/// haritaya düşülmez).
pub fn parse_runs(
    json: &str,
    out_len: usize,
    source_len: usize,
) -> Result<Vec<MaskRun>, MaskError> {
    let runs: Vec<MaskRun> = serde_json::from_str(json).map_err(|_| MaskError::InvalidMap)?;
    if !MaskedText::runs_are_sound(&runs, out_len, source_len) {
        return Err(MaskError::InvalidMap);
    }
    Ok(runs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(list: &[&str]) -> Vec<String> {
        validate_terms(&list.iter().map(|t| (*t).to_string()).collect::<Vec<_>>())
            .expect("geçerli terim listesi")
    }

    #[test]
    fn empty_term_list_is_identity() {
        let masked = mask_terms("Kira bedeli ödenmedi.", &[]).expect("maske");
        assert_eq!(masked.text, "Kira bedeli ödenmedi.");
        assert_eq!(masked.masked_runs, 0);
        assert_eq!(masked.masked_chars, 0);
        assert_eq!(masked.runs.len(), 1);
        assert_eq!(masked.map_span(0, 4), Some((0, 4)));
    }

    #[test]
    fn single_term_is_replaced_and_map_stays_verbatim_around_it() {
        let raw = "Ahmet Yılmaz 12.000 TL ödedi.";
        let masked = mask_terms(raw, &terms(&["Ahmet Yılmaz"])).expect("maske");
        assert_eq!(masked.text, "[MASKED] 12.000 TL ödedi.");
        assert_eq!(masked.masked_runs, 1);
        assert_eq!(masked.masked_chars, "Ahmet Yılmaz".chars().count());
        // Maske parçasına düşen aralık geri eşlenemez.
        assert_eq!(masked.map_span(0, 8), None);
        // Maskeden sonraki metin birebir eşlenir (yer tutucu 8 karakter).
        let tail = masked.text.find("ödedi").expect("tail");
        let out_start = masked.text[..tail].chars().count();
        let (source_start, source_end) = masked
            .map_span(out_start, out_start + 5)
            .expect("kopya aralığı eşlenir");
        assert_eq!(
            &raw.chars().collect::<Vec<_>>()[source_start..source_end]
                .iter()
                .collect::<String>(),
            "ödedi"
        );
        // Harita metnin tamamını kesintisiz kaplar.
        assert!(MaskedText::runs_are_sound(
            &masked.runs,
            masked.text.chars().count(),
            raw.chars().count()
        ));
    }

    #[test]
    fn turkish_case_folding_is_one_to_one_and_finds_both_ways() {
        // `İ`/`I` katlaması aralık kaydırmaz: aynı uzunlukta kalır.
        let raw = "İSTANBUL'da ışık yok.";
        let masked = mask_terms(raw, &terms(&["istanbul"])).expect("maske");
        assert_eq!(masked.masked_chars, "İSTANBUL".chars().count());
        assert!(masked.text.starts_with(MASK_PLACEHOLDER));
        let masked2 = mask_terms(raw, &terms(&["IŞIK"])).expect("maske");
        assert_eq!(masked2.masked_chars, 4);
    }

    #[test]
    fn overlapping_terms_merge_into_one_placeholder() {
        let raw = "Ahmet Yılmaz ödedi.";
        let masked = mask_terms(raw, &terms(&["Ahmet", "Ahmet Yılmaz"])).expect("maske");
        // Uzun terim kazanır; tek yer tutucu yazılır.
        assert_eq!(masked.masked_runs, 1);
        assert_eq!(masked.text, "[MASKED] ödedi.");
    }

    #[test]
    fn touching_terms_merge_so_no_double_placeholder() {
        // Bitişik iki terim tek yer tutucuda birleşir: "AhmetYılmaz" →
        // "[MASKED]". Arasında boşluk kalan terimler **ayrı** maskelenir;
        // boşluğu yutmak haritayı kanıtsız bırakırdı.
        let raw = "AhmetYılmaz geldi.";
        let masked = mask_terms(raw, &terms(&["Ahmet", "Yılmaz"])).expect("maske");
        assert_eq!(masked.masked_runs, 1);
        assert_eq!(masked.text, "[MASKED] geldi.");
        let spaced =
            mask_terms("Ahmet Yılmaz geldi.", &terms(&["Ahmet", "Yılmaz"])).expect("maske");
        assert_eq!(spaced.masked_runs, 2);
        assert_eq!(spaced.text, "[MASKED] [MASKED] geldi.");
    }

    #[test]
    fn span_crossing_a_mask_boundary_is_unmappable() {
        let raw = "Ali Veli 5 TL";
        let masked = mask_terms(raw, &terms(&["Veli"])).expect("maske");
        assert_eq!(masked.text, "Ali [MASKED] 5 TL");
        // "Ali " (4) kopya; 4..9 maskeye düşer.
        assert_eq!(masked.map_span(0, 4), Some((0, 4)));
        assert_eq!(masked.map_span(2, 9), None);
        assert_eq!(masked.map_span(4, 12), None);
        // Sondaki kopya parçası doğru kaynaktan eşlenir.
        let (start, end) = masked.map_span(13, 17).expect("son parça");
        assert_eq!(start, "Ali Veli ".chars().count());
        assert_eq!(end - start, 4);
    }

    #[test]
    fn placeholder_in_source_text_is_rejected() {
        let error = mask_terms("Metin [MASKED] içeriyor", &terms(&["Metin"])).expect_err("red");
        assert_eq!(error, MaskError::PlaceholderPresent);
        let error = validate_terms(&["kötü [MASKED] terim".to_string()]).expect_err("red");
        assert_eq!(error, MaskError::PlaceholderPresent);
    }

    #[test]
    fn term_validation_rejects_short_control_and_duplicates() {
        assert_eq!(
            validate_terms(&["a".to_string()]),
            Err(MaskError::InvalidTerm)
        );
        assert_eq!(
            validate_terms(&["iyi".to_string(), "kötü\u{7}".to_string()]),
            Err(MaskError::InvalidTerm)
        );
        let folded = validate_terms(&["Ahmet".to_string(), "AHMET".to_string()]).expect("tek");
        assert_eq!(folded.len(), 1);
        assert_eq!(folded[0], "Ahmet");
        let too_many: Vec<String> = (0..=MAX_MASK_TERMS).map(|i| format!("terim{i}")).collect();
        assert_eq!(validate_terms(&too_many), Err(MaskError::TooManyTerms));
    }

    #[test]
    fn parse_runs_rejects_gaps_and_truncation() {
        let raw = "Ali Veli 5 TL";
        let masked = mask_terms(raw, &terms(&["Veli"])).expect("maske");
        let json = serde_json::to_string(&masked.runs).expect("json");
        let parsed =
            parse_runs(&json, masked.text.chars().count(), raw.chars().count()).expect("okunur");
        assert_eq!(parsed, masked.runs);
        assert_eq!(parse_runs("[]", 5, 5), Err(MaskError::InvalidMap));
        assert_eq!(parse_runs("bozuk", 5, 5), Err(MaskError::InvalidMap));
        // Kopya parçasında genişlik uyuşmazlığı kanıtsızdır.
        let broken = r#"[{"outStart":0,"outEnd":4,"sourceStart":0,"sourceEnd":3}]"#;
        assert_eq!(parse_runs(broken, 4, 3), Err(MaskError::InvalidMap));
    }
}
