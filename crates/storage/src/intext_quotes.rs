//! Metin içi alıntı denetimi (F12-T14).
//!
//! Serbest metindeki (iddia, özet, not) çift tırnaklı pasajları **çıkarır** ve
//! dosyadaki belgelerin güncel sürümlerinin metin katmanında doğrular.
//!
//! Üç eşleşme katmanı ayrı raporlanır: `exact` (birebir), `whitespace` (yalnız
//! boşluk/satır farkı) ve `turkish_fold` (harf sadeleştirmesi). Katmanlar
//! sırayla denenir; üst katman bulunduysa alt katman aranmaz.
//!
//! `not_found` sonucu "uydurma" hükmü değildir: yalnız taranan metin
//! katmanında bulunmadığını söyler. Taranmış (OCR'sız) sayfada görünen metin
//! bu denetimde bulunamaz.
//!
//! Tek sayfa taramasından sonra, hâlâ bulunamayan alıntılar için **sayfa
//! sınırını aşan** eşleşme aranır: ardışık sayfalar bir pencerede birleştirilir,
//! yalnız iki sayfaya da gerçekten yayılan eşleşmeler kaydedilir. Böyle bir
//! eşleşme `spansPages` ile işaretlenir; tek sayfalık anchor'a bağlanmaz.
//!
//! Güncel sürümde bulunamayan alıntı için **belgenin eski sürümleri** de
//! taranır (en çok `MAX_HISTORICAL_VERSIONS_PER_DOCUMENT`, yeniden yeniye).
//! Eski sürümde bulunan konum `isCurrentVersion=false` taşır: alıntı güncel
//! metinde geçmiyor, belge güncellenmiş olabilir. Anchor sürümü zaten kaydettiği
//! için bağlanabilir, ama arayüz bunu ayrıca yazar.

use std::collections::BTreeSet;

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::precedents::{
    char_offset, collapse_ws_with_map, normalize_with_map, MappedText, MIN_QUOTE_CHARS,
};
use crate::{now_iso, Database, StorageError};

/// Denetlenecek serbest metnin üst sınırı (karakter).
pub const MAX_IN_TEXT_CHECK_CHARS: usize = 50_000;
/// Tek metinde denetlenecek alıntı sayısı üst sınırı.
pub const MAX_IN_TEXT_QUOTES: usize = 30;
/// Tek alıntının üst sınırı: daha uzunu büyük olasılıkla tırnak hatasıdır.
const MAX_SPAN_CHARS: usize = 1_000;
/// Bulunulan konumdan rapora taşınacak en çok aday sayısı.
const MAX_CANDIDATES: usize = 8;
/// Tek denetimde taranacak sayfa üst sınırı; aşılırsa belge seçilmelidir.
const MAX_SCOPE_PAGES: i64 = 20_000;
/// Belge başına taranacak **eski** sürüm sayısı üst sınırı. Güncel sürüm her
/// zaman taranır; sınırın ötesindeki daha eski sürümler taranmaz ve durum
/// notlarda yazılır (sessiz kesme yok).
const MAX_HISTORICAL_VERSIONS_PER_DOCUMENT: usize = 5;

const SCOPE_PAGE_SQL: &str = "SELECT p.id,p.page_index,p.text_origin,
        (SELECT s.id FROM text_segments s WHERE s.page_id=p.id LIMIT 1),
        COALESCE((SELECT s.raw_text FROM text_segments s WHERE s.page_id=p.id LIMIT 1),'')
     FROM pages p WHERE p.parse_run_id=?1 ORDER BY p.page_index";

/// Metinden çıkarılmış tek alıntı. `start`/`end`, alıntı **içeriğinin**
/// (tırnaklar hariç) karakter aralığıdır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedQuote {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub style: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct ExtractedQuotes {
    pub quotes: Vec<SpannedQuote>,
    pub skipped_short: usize,
    pub skipped_long: usize,
    pub unclosed: usize,
}

/// Serbest metindeki çift tırnak ailesini çıkarır: `“…”`, `„…“`, `«…»`, `"…"`.
/// Tek tırnak ve apostrof alıntı sayılmaz (Türkçe'de `İstanbul'da` alıntı
/// değildir); bu yüzden `’`/`'` sınır kabul edilmez.
#[must_use]
pub fn extract_quoted_spans(text: &str) -> ExtractedQuotes {
    const OPENERS: [(char, &str); 4] = [
        ('\u{201C}', "typographic"),
        ('\u{201E}', "german"),
        ('\u{00AB}', "guillemet"),
        ('"', "straight"),
    ];
    const CLOSERS: [char; 4] = ['\u{201D}', '\u{201C}', '\u{00BB}', '"'];

    let chars: Vec<char> = text.chars().collect();
    let mut out = ExtractedQuotes::default();
    let mut index = 0usize;
    while index < chars.len() {
        let Some((_, style)) = OPENERS.iter().find(|(mark, _)| *mark == chars[index]) else {
            index += 1;
            continue;
        };
        let mut close = None;
        let mut probe = index + 1;
        while probe < chars.len() {
            if CLOSERS.contains(&chars[probe]) {
                close = Some(probe);
                break;
            }
            probe += 1;
        }
        let Some(close) = close else {
            // Kapanmayan tırnak: metnin geri kalanı alıntı sayılmaz.
            out.unclosed += 1;
            break;
        };
        let raw: String = chars[index + 1..close].iter().collect();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            out.skipped_short += 1;
            index = close + 1;
            continue;
        }
        // Kırpılan boşlukları karakter konumundan düş.
        let lead = raw.chars().take_while(|c| c.is_whitespace()).count();
        let text_len = trimmed.chars().count();
        if text_len < MIN_QUOTE_CHARS {
            out.skipped_short += 1;
        } else if text_len > MAX_SPAN_CHARS {
            out.skipped_long += 1;
        } else {
            out.quotes.push(SpannedQuote {
                start: index + 1 + lead,
                end: index + 1 + lead + text_len,
                text: trimmed.to_string(),
                style,
            });
        }
        index = close + 1;
    }
    out
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InTextQuoteInput {
    pub text: String,
    /// Verilirse yalnız bu belgenin **güncel** sürümü taranır.
    #[serde(default)]
    pub document_id: Option<String>,
}

/// Alıntının bulunduğu gerçek konum. `sourceText`, kaynak metnin o aralıktaki
/// **ham** hâlidir; `normalized` eşleşmede yazılan alıntıdan farklı olabilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InTextQuotePosition {
    pub document_id: String,
    pub document_name: String,
    pub document_version_id: String,
    pub parse_run_id: String,
    pub page_id: Option<String>,
    pub segment_id: Option<String>,
    pub page_index: i64,
    /// Unicode scalar konumu (bayt değil). Sayfa sınırını aşan eşleşmede
    /// `char_start` **ilk** sayfadaki, `char_end` **son** sayfadaki konumdur
    /// (`end_page_index`).
    pub char_start: i64,
    pub char_end: i64,
    /// Eşleşmenin bittiği sayfa. Sayfa sınırını aşmayan eşleşmede
    /// `page_index` ile aynıdır.
    pub end_page_index: i64,
    /// Eşleşme sayfa sınırını aşıyor mu (kaynakta sayfa/satır sonu).
    pub spans_pages: bool,
    /// Kaynağın sürüm numarası (`document_versions.version_no`).
    pub version_no: i64,
    /// Konum **güncel** sürümde mi? `false` ise alıntı yalnız eski bir sürümde
    /// bulundu; belge güncellenmiş olabilir.
    pub is_current_version: bool,
    /// `exact` | `whitespace` | `turkish_fold`
    pub match_kind: String,
    pub text_origin: String,
    pub source_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InTextQuoteFinding {
    pub index: usize,
    pub quote: String,
    /// `exact` | `normalized` | `ambiguous` | `not_found` | `unverifiable`
    pub state: String,
    /// `no_text` (taranacak metin yok) — başka neden uydurulmaz.
    pub reason: Option<String>,
    pub occurrences: i64,
    pub distinct_documents: i64,
    /// Yalnız tek konum varsa dolu.
    pub position: Option<InTextQuotePosition>,
    /// Birden fazla konum varsa ilk adaylar (en çok 8).
    pub candidates: Vec<InTextQuotePosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InTextQuoteCheck {
    pub case_id: String,
    pub scope: String,
    pub scope_document_id: Option<String>,
    pub scope_document_name: Option<String>,
    pub text_chars: i64,
    pub spans_found: usize,
    pub skipped_short: usize,
    pub skipped_long: usize,
    pub unclosed: usize,
    pub quotes: Vec<InTextQuoteFinding>,
    pub exact: usize,
    pub normalized: usize,
    pub ambiguous: usize,
    pub not_found: usize,
    pub unverifiable: usize,
    pub documents_total: usize,
    pub documents_with_text: usize,
    pub documents_without_text: usize,
    pub pages_scanned: usize,
    /// Taranan sürüm sayısı (güncel + eski).
    pub versions_scanned: usize,
    /// Bunlardan **eski** sürüm sayısı.
    pub historical_versions_scanned: usize,
    /// Yalnız eski sürümde bulunan alıntı sayısı.
    pub found_in_old_version: usize,
    pub notes: Vec<String>,
    pub checked_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchTier {
    Exact,
    Whitespace,
    Fold,
}

impl MatchTier {
    const fn kind(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Whitespace => "whitespace",
            Self::Fold => "turkish_fold",
        }
    }
}

struct VersionScope {
    document_id: String,
    document_name: String,
    version_id: Option<String>,
    /// `document_versions.version_no`; metin yoksa 0.
    version_no: i64,
    /// Bu kapsam belgenin **güncel** sürümü mü?
    is_current: bool,
    parse_run_id: Option<String>,
}

/// `resolve_historical_scopes` içinde belge başına sürüm toplama torbası.
struct HistoryBucket {
    document_id: String,
    document_name: String,
    /// Yeniden eskiye `(version_id, version_no)`.
    versions: Vec<(String, i64)>,
}

#[derive(Clone)]
struct RawHit {
    document_id: String,
    document_name: String,
    version_id: String,
    parse_run_id: String,
    page_id: Option<String>,
    segment_id: Option<String>,
    page_index: i64,
    char_start: i64,
    /// Eşleşmenin bittiği sayfa ve o sayfadaki bitiş konumu.
    end_page_index: i64,
    char_end: i64,
    spans_pages: bool,
    version_no: i64,
    is_current_version: bool,
    match_kind: &'static str,
    text_origin: String,
    source_text: String,
}

impl RawHit {
    fn position(&self) -> InTextQuotePosition {
        InTextQuotePosition {
            document_id: self.document_id.clone(),
            document_name: self.document_name.clone(),
            document_version_id: self.version_id.clone(),
            parse_run_id: self.parse_run_id.clone(),
            page_id: self.page_id.clone(),
            segment_id: self.segment_id.clone(),
            page_index: self.page_index,
            char_start: self.char_start,
            char_end: self.char_end,
            end_page_index: self.end_page_index,
            spans_pages: self.spans_pages,
            version_no: self.version_no,
            is_current_version: self.is_current_version,
            match_kind: self.match_kind.to_string(),
            text_origin: self.text_origin.clone(),
            source_text: self.source_text.clone(),
        }
    }
}

#[derive(Default)]
struct QuoteAccum {
    hits: Vec<RawHit>,
    occurrences: i64,
    documents: BTreeSet<String>,
}

impl QuoteAccum {
    fn record(&mut self, hit: RawHit) {
        self.occurrences += 1;
        self.documents.insert(hit.document_id.clone());
        if self.hits.len() < MAX_CANDIDATES {
            self.hits.push(hit);
        }
    }
}

impl Database {
    /// Serbest metindeki alıntıları dosyanın belgelerinde doğrular.
    ///
    /// Kapsam: `document_id` verilirse o belgenin güncel sürümü; verilmezse
    /// dosyadaki bütün belgelerin güncel sürümleri. Her sürümün **en yeni**
    /// başarılı/kısmi ayrıştırması taranır.
    ///
    /// Güncel sürümde bulunamayan alıntı için belgenin **eski sürümleri** de
    /// taranır (belge başına en çok `MAX_HISTORICAL_VERSIONS_PER_DOCUMENT`,
    /// yeniden yeniye). Eski sürümde bulunan konum `is_current_version=false`
    /// taşır; anchor sürümü kaydettiği için bağlanabilir, arayüz bunu yazar.
    pub fn check_in_text_quotes(
        &self,
        case_id: &str,
        input: &InTextQuoteInput,
    ) -> Result<InTextQuoteCheck, StorageError> {
        let text_chars = input.text.chars().count();
        if text_chars > MAX_IN_TEXT_CHECK_CHARS {
            return Err(StorageError::QuoteTextTooLong);
        }
        let extracted = extract_quoted_spans(&input.text);
        if extracted.quotes.len() > MAX_IN_TEXT_QUOTES {
            return Err(StorageError::QuoteTooMany);
        }
        if self.get_case(case_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let scopes = self.resolve_quote_scopes(case_id, input.document_id.as_deref())?;
        let documents_total = scopes.len();
        let documents_with_text = scopes
            .iter()
            .filter(|scope| scope.parse_run_id.is_some())
            .count();
        let documents_without_text = documents_total - documents_with_text;

        let mut notes = vec![
            "Alıntı çıkarımı yalnız çift tırnak ailesini tanır (“…” · „…“ · «…» · \"…\"); \
             tek tırnak ve apostrof alıntı sayılmaz."
                .to_string(),
            "Bulunamadı sonucu “uydurma” hükmü değildir: yalnız taranan metin katmanında \
             bulunmadığını söyler."
                .to_string(),
        ];
        if let Some(document_id) = input.document_id.as_deref() {
            let name = scopes
                .iter()
                .find(|scope| scope.document_id == document_id)
                .map(|scope| scope.document_name.clone())
                .unwrap_or_default();
            notes.push(format!(
                "Yalnız “{name}” belgesinin güncel sürümü tarandı; alıntı başka belgede geçebilir."
            ));
        }
        if documents_without_text > 0 {
            notes.push(format!(
                "{documents_without_text} belgenin güncel sürümünde ayrıştırılmış metin yok \
                 (bu belgeler taranamadı)."
            ));
        }

        let mut accum = (0..extracted.quotes.len())
            .map(|_| QuoteAccum::default())
            .collect::<Vec<_>>();
        let mut pages_scanned = 0usize;
        let mut versions_scanned = 0usize;
        let mut historical_versions_scanned = 0usize;
        let mut historical_truncated_documents = 0usize;
        let mut historical_skipped_for_pages = false;
        if extracted.quotes.is_empty() {
            notes.push("Metinde çift tırnaklı alıntı bulunamadı.".to_string());
        } else {
            let max_pages = usize::try_from(MAX_SCOPE_PAGES).unwrap_or(usize::MAX);
            pages_scanned = self.count_scope_pages(&scopes)?;
            versions_scanned = scopes
                .iter()
                .filter(|scope| scope.parse_run_id.is_some())
                .count();
            if pages_scanned > max_pages {
                return Err(StorageError::QuoteScopeTooLarge);
            }
            // Eski sürüm kapsamları yalnız güncel sürümde bir şey bulunamazsa
            // çözülür ve taranır: sık durumda ek maliyet yok.
            let mut historical: Option<Vec<VersionScope>> = None;
            let mut active: Vec<usize> = (0..extracted.quotes.len()).collect();
            // Katman sırası korunur: üst katman bulduysa alt kata inilmez.
            // Her katmanda önce güncel sürümün tek sayfası, sonra sayfa sınırı,
            // ardından **eski sürümler** taranır; böylece eski sürümdeki birebir
            // eşleşme, güncel sürümdeki normalize eşleşmeden üstün kalır.
            for tier in [MatchTier::Exact, MatchTier::Whitespace, MatchTier::Fold] {
                if active.is_empty() {
                    break;
                }
                self.scan_quote_tier(&scopes, &extracted.quotes, &active, &mut accum, tier)?;
                active = pending_quotes(&accum);
                if active.is_empty() {
                    break;
                }
                self.scan_page_break_tier(&scopes, &extracted.quotes, &active, &mut accum, tier)?;
                active = pending_quotes(&accum);
                if active.is_empty() {
                    break;
                }
                if historical.is_none() {
                    let (historic, truncated) =
                        self.resolve_historical_scopes(case_id, input.document_id.as_deref())?;
                    historical_truncated_documents = truncated;
                    let extra_pages = self.count_scope_pages(&historic)?;
                    if extra_pages > 0 && pages_scanned.saturating_add(extra_pages) > max_pages {
                        // Sessizce kesmek yerine hiç taranmaz ve nedeni yazılır.
                        historical_skipped_for_pages = true;
                        historical = Some(Vec::new());
                    } else {
                        pages_scanned = pages_scanned.saturating_add(extra_pages);
                        historical_versions_scanned = historic
                            .iter()
                            .filter(|scope| scope.parse_run_id.is_some())
                            .count();
                        versions_scanned =
                            versions_scanned.saturating_add(historical_versions_scanned);
                        historical = Some(historic);
                    }
                }
                let historic = historical.as_deref().unwrap_or(&[]);
                if !historic.is_empty() {
                    self.scan_quote_tier(historic, &extracted.quotes, &active, &mut accum, tier)?;
                    active = pending_quotes(&accum);
                    if !active.is_empty() {
                        self.scan_page_break_tier(
                            historic,
                            &extracted.quotes,
                            &active,
                            &mut accum,
                            tier,
                        )?;
                        active = pending_quotes(&accum);
                    }
                }
            }
        }
        if historical_versions_scanned > 0 {
            notes.push(format!(
                "Güncel sürümde bulunamayan alıntılar için belgelerin eski sürümleri de \
                 tarandı ({historical_versions_scanned} sürüm)."
            ));
        }
        if historical_truncated_documents > 0 {
            notes.push(format!(
                "{historical_truncated_documents} belgede en çok \
                 {MAX_HISTORICAL_VERSIONS_PER_DOCUMENT} eski sürüm tarandı; daha eski \
                 sürümler taranmadı."
            ));
        }
        if historical_skipped_for_pages {
            notes.push(format!(
                "Eski sürüm taraması {MAX_SCOPE_PAGES} sayfa sınırı nedeniyle yapılmadı; \
                 kapsamı belge seçerek daraltın."
            ));
        }

        let mut counts = [0usize; 5];
        let mut spanning = 0usize;
        let mut bindable = 0usize;
        let mut found_in_old_version = 0usize;
        let mut quotes = Vec::with_capacity(extracted.quotes.len());
        for (index, span) in extracted.quotes.iter().enumerate() {
            let found = &accum[index];
            let mut finding = InTextQuoteFinding {
                index,
                quote: span.text.clone(),
                state: "not_found".into(),
                reason: None,
                occurrences: found.occurrences,
                distinct_documents: i64::try_from(found.documents.len()).unwrap_or(i64::MAX),
                position: None,
                candidates: Vec::new(),
            };
            if pages_scanned == 0 {
                finding.state = "unverifiable".into();
                finding.reason = Some("no_text".into());
            } else if found.occurrences == 0 {
                finding.state = "not_found".into();
            } else if found.occurrences == 1 {
                let hit = found.hits.first().ok_or(StorageError::NotFound)?;
                finding.state = if hit.match_kind == MatchTier::Exact.kind() {
                    "exact".into()
                } else {
                    "normalized".into()
                };
                finding.position = Some(hit.position());
            } else {
                finding.state = "ambiguous".into();
                finding.candidates = found.hits.iter().map(RawHit::position).collect();
            }
            match finding.state.as_str() {
                "exact" => counts[0] += 1,
                "normalized" => counts[1] += 1,
                "ambiguous" => counts[2] += 1,
                "not_found" => counts[3] += 1,
                _ => counts[4] += 1,
            }
            let hit_spans = finding
                .position
                .as_ref()
                .is_some_and(|position| position.spans_pages)
                || finding
                    .candidates
                    .iter()
                    .any(|candidate| candidate.spans_pages);
            if hit_spans {
                spanning += 1;
            } else if finding.state == "exact" {
                // Tek sayfada birebir: tek sayfalık anchor'a bağlanabilir.
                bindable += 1;
            }
            if finding
                .position
                .as_ref()
                .is_some_and(|position| !position.is_current_version)
            {
                found_in_old_version += 1;
            }
            quotes.push(finding);
        }
        if found_in_old_version > 0 {
            notes.push(format!(
                "{found_in_old_version} alıntı yalnız **eski** bir sürümde bulundu; güncel \
                 sürümde geçmiyor (belge güncellenmiş olabilir). Kaynak, alıntının bulunduğu \
                 sürüme bağlanır."
            ));
        }
        if bindable > 0 {
            notes.push(format!(
                "{bindable} alıntı birebir tek konumda bulundu; kaynak bağlanabilir."
            ));
        }
        if spanning > 0 {
            notes.push(format!(
                "{spanning} alıntı yalnız sayfa sınırını aşarak bulundu (kaynakta sayfa \
                 sonu/satır sonu var); tek sayfalık anchor'a bağlanmaz, kaynak metin \
                 gösterilir."
            ));
        }
        if counts[1] > 0 {
            notes.push(
                "Birebir olmayan (boşluk veya harf farkı) eşleşmeler kaynak metniyle gösterilir; \
                 anchor'a bağlanmaz."
                    .to_string(),
            );
        }
        if counts[2] > 0 {
            notes.push(
                "Birden fazla konumda geçen alıntı tek kaynağa bağlanmaz; konumu kullanıcı seçmelidir."
                    .to_string(),
            );
        }
        if pages_scanned == 0 && !extracted.quotes.is_empty() {
            notes.push(
                "Taranacak ayrıştırılmış metin bulunamadı; alıntılar bu denetimde \
                 doğrulanamadı."
                    .to_string(),
            );
        }

        let scope_document = input.document_id.as_deref().and_then(|id| {
            scopes
                .iter()
                .find(|scope| scope.document_id == id)
                .map(|scope| (scope.document_id.clone(), scope.document_name.clone()))
        });
        Ok(InTextQuoteCheck {
            case_id: case_id.to_string(),
            scope: if input.document_id.is_some() {
                "document".into()
            } else {
                "case".into()
            },
            scope_document_id: scope_document.as_ref().map(|(id, _)| id.clone()),
            scope_document_name: scope_document.map(|(_, name)| name),
            text_chars: i64::try_from(text_chars).unwrap_or(i64::MAX),
            spans_found: extracted.quotes.len(),
            skipped_short: extracted.skipped_short,
            skipped_long: extracted.skipped_long,
            unclosed: extracted.unclosed,
            quotes,
            exact: counts[0],
            normalized: counts[1],
            ambiguous: counts[2],
            not_found: counts[3],
            unverifiable: counts[4],
            documents_total,
            documents_with_text,
            documents_without_text,
            pages_scanned,
            versions_scanned,
            historical_versions_scanned,
            found_in_old_version,
            notes,
            checked_at: now_iso(),
        })
    }

    fn resolve_quote_scopes(
        &self,
        case_id: &str,
        document_id: Option<&str>,
    ) -> Result<Vec<VersionScope>, StorageError> {
        let mut scopes = Vec::new();
        if let Some(document_id) = document_id {
            let row: Option<(String, String, Option<String>, i64)> = self
                .conn()
                .query_row(
                    "SELECT d.id,d.display_name,d.current_version_id,
                            COALESCE((SELECT v.version_no FROM document_versions v
                                      WHERE v.id=d.current_version_id),0)
                     FROM documents d
                     WHERE d.id=?1 AND d.case_id=?2 AND d.deleted_at IS NULL",
                    [document_id, case_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()?;
            let Some((id, name, version_id, version_no)) = row else {
                return Err(StorageError::NotFound);
            };
            let run_id = match version_id.as_deref() {
                Some(version) => self.latest_text_run(version)?,
                None => None,
            };
            scopes.push(VersionScope {
                document_id: id,
                document_name: name,
                version_id,
                version_no,
                is_current: true,
                parse_run_id: run_id,
            });
            return Ok(scopes);
        }
        let mut stmt = self.conn().prepare(
            "SELECT d.id,d.display_name,d.current_version_id,
                    COALESCE((SELECT v.version_no FROM document_versions v
                              WHERE v.id=d.current_version_id),0)
             FROM documents d
             WHERE d.case_id=?1 AND d.deleted_at IS NULL ORDER BY d.created_at DESC, d.id DESC",
        )?;
        let rows = stmt.query_map([case_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        let mut docs = Vec::new();
        for row in rows {
            docs.push(row?);
        }
        for (id, name, version_id, version_no) in docs {
            let run_id = match version_id.as_deref() {
                Some(version) => self.latest_text_run(version)?,
                None => None,
            };
            scopes.push(VersionScope {
                document_id: id,
                document_name: name,
                version_id,
                version_no,
                is_current: true,
                parse_run_id: run_id,
            });
        }
        Ok(scopes)
    }

    /// Güncel sürümde bulunamayan alıntılar için **eski sürüm** kapsamları.
    ///
    /// Her belge için güncel sürüm dışındaki sürümler yeniden eskiye alınır ve
    /// `MAX_HISTORICAL_VERSIONS_PER_DOCUMENT` ile sınırlanır; sınırın ötesinde
    /// kalan sürüm varsa ikinci dönüş değeri (kesilen belge sayısı) artar.
    /// Metni olmayan sürümler de kapsama girer: `documents_without_text`
    /// sayımı güncel sürüm üzerinden yapıldığı için burada ayrıca sayılmaz.
    fn resolve_historical_scopes(
        &self,
        case_id: &str,
        document_id: Option<&str>,
    ) -> Result<(Vec<VersionScope>, usize), StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT v.document_id,d.display_name,v.id,v.version_no
             FROM document_versions v JOIN documents d ON d.id=v.document_id
             WHERE d.case_id=?1 AND d.deleted_at IS NULL
               AND (?2 IS NULL OR d.id=?2)
               AND (d.current_version_id IS NULL OR v.id<>d.current_version_id)
             ORDER BY v.document_id, v.version_no DESC, v.id DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![case_id, document_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        let mut buckets: Vec<HistoryBucket> = Vec::new();
        for row in rows {
            let (doc_id, name, version_id, version_no) = row?;
            match buckets.last_mut() {
                Some(bucket) if bucket.document_id == doc_id => {
                    bucket.versions.push((version_id, version_no));
                }
                _ => buckets.push(HistoryBucket {
                    document_id: doc_id,
                    document_name: name,
                    versions: vec![(version_id, version_no)],
                }),
            }
        }
        let mut scopes = Vec::new();
        let mut truncated_documents = 0usize;
        for bucket in buckets {
            if bucket.versions.len() > MAX_HISTORICAL_VERSIONS_PER_DOCUMENT {
                truncated_documents += 1;
            }
            for (version_id, version_no) in bucket
                .versions
                .into_iter()
                .take(MAX_HISTORICAL_VERSIONS_PER_DOCUMENT)
            {
                let run_id = self.latest_text_run(&version_id)?;
                scopes.push(VersionScope {
                    document_id: bucket.document_id.clone(),
                    document_name: bucket.document_name.clone(),
                    version_id: Some(version_id),
                    version_no,
                    is_current: false,
                    parse_run_id: run_id,
                });
            }
        }
        Ok((scopes, truncated_documents))
    }

    fn latest_text_run(&self, version_id: &str) -> Result<Option<String>, StorageError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT id FROM parse_runs
                 WHERE document_version_id=?1 AND status IN ('succeeded','partial')
                 ORDER BY COALESCE(completed_at, created_at) DESC, id DESC LIMIT 1",
                [version_id],
                |row| row.get(0),
            )
            .optional()?)
    }

    fn count_scope_pages(&self, scopes: &[VersionScope]) -> Result<usize, StorageError> {
        let mut total: i64 = 0;
        for scope in scopes {
            if let Some(run_id) = scope.parse_run_id.as_deref() {
                total += self.conn().query_row(
                    "SELECT count(*) FROM pages WHERE parse_run_id=?1",
                    [run_id],
                    |row| row.get::<_, i64>(0),
                )?;
            }
        }
        Ok(usize::try_from(total).unwrap_or(usize::MAX))
    }

    /// Tek bir eşleşme katmanını tarar. `active` yalnız o kata bakılacak
    /// alıntı indeksleridir; üst katmanda bulunan alıntı alt kata inmez.
    fn scan_quote_tier(
        &self,
        scopes: &[VersionScope],
        spans: &[SpannedQuote],
        active: &[usize],
        accum: &mut [QuoteAccum],
        tier: MatchTier,
    ) -> Result<(), StorageError> {
        if active.is_empty() {
            return Ok(());
        }
        let needles: Vec<Option<MappedText>> = spans
            .iter()
            .map(|span| match tier {
                MatchTier::Exact => None,
                MatchTier::Whitespace => Some(collapse_ws_with_map(&span.text)),
                MatchTier::Fold => Some(normalize_with_map(&span.text)),
            })
            .collect();
        let needle_chars: Vec<usize> = needles
            .iter()
            .map(|needle| {
                needle
                    .as_ref()
                    .map(|mapped| mapped.text.chars().count())
                    .unwrap_or(0)
            })
            .collect();

        let connection = self.conn();
        let mut stmt = connection.prepare(SCOPE_PAGE_SQL)?;
        for scope in scopes {
            let Some(run_id) = scope.parse_run_id.as_deref() else {
                continue;
            };
            let rows = stmt.query_map([run_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?;
            for row in rows {
                let (page_id, page_index, text_origin, segment_id, text) = row?;
                if text.is_empty() {
                    continue;
                }
                match tier {
                    MatchTier::Exact => {
                        for &quote_index in active {
                            let quote = &spans[quote_index].text;
                            for (byte, _) in text.match_indices(quote) {
                                let start = char_offset(&text, byte);
                                accum[quote_index].record(RawHit {
                                    document_id: scope.document_id.clone(),
                                    document_name: scope.document_name.clone(),
                                    version_id: scope.version_id.clone().unwrap_or_default(),
                                    parse_run_id: run_id.to_string(),
                                    page_id: Some(page_id.clone()),
                                    segment_id: segment_id.clone(),
                                    page_index,
                                    char_start: start,
                                    end_page_index: page_index,
                                    char_end: start
                                        + i64::try_from(quote.chars().count()).unwrap_or(i64::MAX),
                                    spans_pages: false,
                                    version_no: scope.version_no,
                                    is_current_version: scope.is_current,
                                    match_kind: tier.kind(),
                                    text_origin: text_origin.clone(),
                                    source_text: quote.clone(),
                                });
                            }
                        }
                    }
                    MatchTier::Whitespace | MatchTier::Fold => {
                        let mapped = if tier == MatchTier::Whitespace {
                            collapse_ws_with_map(&text)
                        } else {
                            normalize_with_map(&text)
                        };
                        for &quote_index in active {
                            let Some(needle) = needles[quote_index].as_ref() else {
                                continue;
                            };
                            if needle.text.is_empty() || needle_chars[quote_index] == 0 {
                                continue;
                            }
                            let mut from = 0usize;
                            while let Some(offset) = mapped.text[from..].find(&needle.text) {
                                let at = from + offset;
                                if let Some((start, end)) =
                                    source_span(&mapped, at, needle_chars[quote_index])
                                {
                                    let source_text: String = text
                                        .chars()
                                        .skip(usize::try_from(start).unwrap_or(usize::MAX))
                                        .take(usize::try_from(end - start).unwrap_or(0))
                                        .collect();
                                    accum[quote_index].record(RawHit {
                                        document_id: scope.document_id.clone(),
                                        document_name: scope.document_name.clone(),
                                        version_id: scope.version_id.clone().unwrap_or_default(),
                                        parse_run_id: run_id.to_string(),
                                        page_id: Some(page_id.clone()),
                                        segment_id: segment_id.clone(),
                                        page_index,
                                        char_start: start,
                                        char_end: end,
                                        end_page_index: page_index,
                                        spans_pages: false,
                                        version_no: scope.version_no,
                                        is_current_version: scope.is_current,
                                        match_kind: tier.kind(),
                                        text_origin: text_origin.clone(),
                                        source_text,
                                    });
                                }
                                from = at + needle.text.len().max(1);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// **Sayfa sınırını aşan** eşleşmeleri arar (F12-T14 genişletmesi).
    ///
    /// Ardışık sayfalar tek pencerede birleştirilir (araya bir ayırıcı konur);
    /// yalnız iki sayfaya da gerçekten yayılan eşleşmeler kaydedilir, çünkü tek
    /// sayfa içinde kalanlar birinci taramada zaten bulunmuştur. Katman
    /// kuralları `scan_quote_tier` ile aynıdır; `spansPages=true` yazılır ve
    /// böyle bir konum tek sayfalık anchor'a bağlanamaz.
    fn scan_page_break_tier(
        &self,
        scopes: &[VersionScope],
        spans: &[SpannedQuote],
        active: &[usize],
        accum: &mut [QuoteAccum],
        tier: MatchTier,
    ) -> Result<(), StorageError> {
        if active.is_empty() {
            return Ok(());
        }
        let needles: Vec<Option<MappedText>> = spans
            .iter()
            .map(|span| match tier {
                MatchTier::Exact => None,
                MatchTier::Whitespace => Some(collapse_ws_with_map(&span.text)),
                MatchTier::Fold => Some(normalize_with_map(&span.text)),
            })
            .collect();
        let needle_chars: Vec<usize> = needles
            .iter()
            .map(|needle| {
                needle
                    .as_ref()
                    .map(|mapped| mapped.text.chars().count())
                    .unwrap_or(0)
            })
            .collect();

        // Taşınan kuyruk en uzun alıntıdan (+ayırıcılar) büyük olmalı; daha
        // uzun bir alıntı çok kısa sayfalar üzerinden üç sayfaya yayılırsa
        // pencere başlangıcı yetmeyebilir (dürüst sınır, README'de yazılı).
        let carry_limit = MAX_SPAN_CHARS + 64;
        let connection = self.conn();
        let mut stmt = connection.prepare(SCOPE_PAGE_SQL)?;
        for scope in scopes {
            let Some(run_id) = scope.parse_run_id.as_deref() else {
                continue;
            };
            let rows = stmt.query_map([run_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?;
            let mut carry: Vec<WindowChar> = Vec::new();
            let mut metas: Vec<PageMeta> = Vec::new();
            for row in rows {
                let (page_id, page_index, text_origin, segment_id, text) = row?;
                if text.is_empty() {
                    continue;
                }
                let boundary = carry.len();
                let mut window: Vec<WindowChar> =
                    Vec::with_capacity(boundary + 1 + text.chars().count());
                window.extend(carry.iter().cloned());
                window.push(WindowChar {
                    page_index: None,
                    char_in_page: 0,
                    ch: '\n',
                });
                for (index, ch) in text.chars().enumerate() {
                    window.push(WindowChar {
                        page_index: Some(page_index),
                        char_in_page: i64::try_from(index).unwrap_or(i64::MAX),
                        ch,
                    });
                }
                // Pencerede artık bulunmayan sayfaların üst verisi düşer;
                // böylece kuyruk sınırlı kalır (sayfa sayısıyla büyümez).
                metas.retain(|meta| {
                    window
                        .iter()
                        .any(|item| item.page_index == Some(meta.page_index))
                });
                metas.push(PageMeta {
                    page_index,
                    page_id: page_id.clone(),
                    segment_id: segment_id.clone(),
                    text_origin: text_origin.clone(),
                });

                let joined: String = window.iter().map(|item| item.ch).collect();
                match tier {
                    MatchTier::Exact => {
                        for &quote_index in active {
                            let quote = &spans[quote_index].text;
                            for (byte, _) in joined.match_indices(quote) {
                                let start = joined[..byte].chars().count();
                                let end = start + quote.chars().count();
                                if let Some(hit) = page_break_hit(
                                    scope,
                                    run_id,
                                    &metas,
                                    &window,
                                    boundary,
                                    start,
                                    end,
                                    tier.kind(),
                                ) {
                                    accum[quote_index].record(hit);
                                }
                            }
                        }
                    }
                    MatchTier::Whitespace | MatchTier::Fold => {
                        let mapped = if tier == MatchTier::Whitespace {
                            collapse_ws_with_map(&joined)
                        } else {
                            normalize_with_map(&joined)
                        };
                        for &quote_index in active {
                            let Some(needle) = needles[quote_index].as_ref() else {
                                continue;
                            };
                            if needle.text.is_empty() || needle_chars[quote_index] == 0 {
                                continue;
                            }
                            let mut from = 0usize;
                            while let Some(offset) = mapped.text[from..].find(&needle.text) {
                                let at = from + offset;
                                if let Some((start, end)) =
                                    source_span(&mapped, at, needle_chars[quote_index])
                                {
                                    if let Some(hit) = page_break_hit(
                                        scope,
                                        run_id,
                                        &metas,
                                        &window,
                                        boundary,
                                        usize::try_from(start).unwrap_or(usize::MAX),
                                        usize::try_from(end).unwrap_or(usize::MAX),
                                        tier.kind(),
                                    ) {
                                        accum[quote_index].record(hit);
                                    }
                                }
                                from = at + needle.text.len().max(1);
                            }
                        }
                    }
                }

                // Kuyruğu pencere sonundan sınırla taşı.
                let keep = window.len().min(carry_limit);
                carry = window.split_off(window.len() - keep);
            }
        }
        Ok(())
    }
}

/// Pencerede taşınan tek karakter: hangi sayfanın kaçıncı karakteri olduğu.
/// `page_index` `None` ise araya konan sayfa ayırıcısıdır (hiçbir sayfaya ait
/// değildir, eşleşmenin ucu olamaz).
#[derive(Clone)]
struct WindowChar {
    page_index: Option<i64>,
    char_in_page: i64,
    ch: char,
}

/// Pencerede tutulan sayfanın kimlikleri: sınırı aşan eşleşmenin **ilk**
/// sayfasının `page_id`/`segment_id` değerleri gerekir.
struct PageMeta {
    page_index: i64,
    page_id: String,
    segment_id: Option<String>,
    text_origin: String,
}

/// Pencere içindeki eşleşmeyi sayfa sınırını aşan bir konuma çevirir.
///
/// Yalnız **iki tarafında da** gerçek sayfa karakteri bulunan eşleşmeler
/// kabul edilir: `start` ayırıcıdan önce, `end` ayırıcıdan en az bir karakter
/// sonra olmalıdır. Aksi hâlde eşleşme tek sayfa içindedir ve birinci
/// taramada bulunmuştur (yinelenmez).
// Gerekçe: pencere bağlamı (kapsam, kimlik, sınır, eşleşme) tek çağrıda
// taşınıyor; ara yapı kurmak kuralı iki yere kopyalardı.
#[allow(clippy::too_many_arguments)]
fn page_break_hit(
    scope: &VersionScope,
    run_id: &str,
    metas: &[PageMeta],
    window: &[WindowChar],
    boundary: usize,
    start: usize,
    end: usize,
    match_kind: &'static str,
) -> Option<RawHit> {
    if start >= boundary || end <= boundary + 1 || end > window.len() {
        return None;
    }
    let first = window.get(start)?;
    let last = window.get(end - 1)?;
    let start_page = first.page_index?;
    let end_page = last.page_index?;
    if start_page >= end_page {
        return None;
    }
    let meta = metas.iter().find(|meta| meta.page_index == start_page)?;
    let source_text: String = window[start..end].iter().map(|item| item.ch).collect();
    Some(RawHit {
        document_id: scope.document_id.clone(),
        document_name: scope.document_name.clone(),
        version_id: scope.version_id.clone().unwrap_or_default(),
        parse_run_id: run_id.to_string(),
        page_id: Some(meta.page_id.clone()),
        segment_id: meta.segment_id.clone(),
        page_index: start_page,
        char_start: first.char_in_page,
        end_page_index: end_page,
        char_end: last.char_in_page.checked_add(1)?,
        spans_pages: true,
        version_no: scope.version_no,
        is_current_version: scope.is_current,
        match_kind,
        text_origin: meta.text_origin.clone(),
        source_text,
    })
}

fn pending_quotes(accum: &[QuoteAccum]) -> Vec<usize> {
    accum
        .iter()
        .enumerate()
        .filter(|(_, found)| found.occurrences == 0)
        .map(|(index, _)| index)
        .collect()
}

/// Sadeleştirilmiş metindeki eşleşmenin kaynak metindeki karakter aralığını
/// verir. `needle_chars`, eşleşen iğnenin **sadeleştirilmiş** karakter sayısıdır.
fn source_span(mapped: &MappedText, at_byte: usize, needle_chars: usize) -> Option<(i64, i64)> {
    if needle_chars == 0 {
        return None;
    }
    let start_index = mapped.text[..at_byte].chars().count();
    let end_index = start_index.checked_add(needle_chars)?;
    let first = *mapped.map.get(start_index)?;
    let last = *mapped.map.get(end_index - 1)?;
    Some((
        i64::try_from(first).ok()?,
        i64::try_from(last.checked_add(1)?).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::{DatabaseKey, NewAnchor, NewPage};

    fn db() -> Database {
        Database::open_in_memory(&DatabaseKey::new([21u8; 32])).expect("db")
    }

    fn new_case(db: &Database, title: &str) -> String {
        db.create_case(&NewCase {
            title: title.into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: crate::Sensitivity::Normal,
            default_ai_policy: crate::AiPolicy::Off,
        })
        .expect("case")
        .id
    }

    fn page(index: i64, text: &str) -> NewPage {
        NewPage {
            index,
            text: text.into(),
            width: Some(595.0),
            height: Some(842.0),
            origin: "native".into(),
            warnings: Vec::new(),
            rotation: 0,
            coordinate_space: Some("unrotated_cropbox_points".into()),
            page_label: None,
            blocks: Vec::new(),
        }
    }

    /// Belge + sürüm + başarılı ayrıştırma + sayfalar. Dönüş: (belge, sürüm).
    fn add_document(db: &Database, case_id: &str, name: &str, pages: &[&str]) -> (String, String) {
        let blob = db
            .create_blob_record(&uuid::Uuid::now_v7().to_string(), 10, "aad")
            .expect("blob");
        let document = db
            .create_document(case_id, name, "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &document.id,
                case_id,
                &blob,
                &"d".repeat(64),
                10,
                "application/pdf",
            )
            .expect("version");
        let run = db
            .start_parse_run(&version.id, case_id, "pymupdf", "1.24", "pdf|ocr=false")
            .expect("run");
        let stored: Vec<NewPage> = pages
            .iter()
            .enumerate()
            .map(|(index, text)| page(i64::try_from(index).unwrap_or(i64::MAX), text))
            .collect();
        db.store_pages(&run.id, case_id, &stored).expect("pages");
        db.finish_parse_run(&run.id, "succeeded", &[])
            .expect("finish");
        (document.id, version.id)
    }

    /// Var olan belgeye yeni sürüm + başarılı ayrıştırma ekler (sürüm numarası
    /// veritabanında artar; `documents.current_version_id` yeni sürüme döner).
    fn add_version_with_pages(
        db: &Database,
        case_id: &str,
        document_id: &str,
        pages: &[&str],
    ) -> String {
        let blob = db
            .create_blob_record(&uuid::Uuid::now_v7().to_string(), 10, "aad")
            .expect("blob");
        let version = db
            .add_version(
                document_id,
                case_id,
                &blob,
                &"f".repeat(64),
                10,
                "application/pdf",
            )
            .expect("version");
        let run = db
            .start_parse_run(&version.id, case_id, "pymupdf", "1.24", "pdf|ocr=false")
            .expect("run");
        let stored: Vec<NewPage> = pages
            .iter()
            .enumerate()
            .map(|(index, text)| page(i64::try_from(index).unwrap_or(i64::MAX), text))
            .collect();
        db.store_pages(&run.id, case_id, &stored).expect("pages");
        db.finish_parse_run(&run.id, "succeeded", &[])
            .expect("finish");
        version.id
    }

    fn add_empty_document(db: &Database, case_id: &str, name: &str) -> String {
        let blob = db
            .create_blob_record(&uuid::Uuid::now_v7().to_string(), 10, "aad")
            .expect("blob");
        let document = db
            .create_document(case_id, name, "pdf", None, "unknown")
            .expect("doc");
        db.add_version(
            &document.id,
            case_id,
            &blob,
            &"e".repeat(64),
            10,
            "application/pdf",
        )
        .expect("version");
        document.id
    }

    fn check(
        db: &Database,
        case_id: &str,
        text: &str,
        document_id: Option<&str>,
    ) -> InTextQuoteCheck {
        db.check_in_text_quotes(
            case_id,
            &InTextQuoteInput {
                text: text.into(),
                document_id: document_id.map(str::to_string),
            },
        )
        .expect("check")
    }

    #[test]
    fn extracts_typographic_quotes_with_content_positions() {
        let out = extract_quoted_spans("Davacı “kira bedelini ödemediğini” söyledi.");
        assert_eq!(out.quotes.len(), 1);
        let quote = &out.quotes[0];
        assert_eq!(quote.text, "kira bedelini ödemediğini");
        assert_eq!(quote.style, "typographic");
        let chars: Vec<char> = "Davacı “kira bedelini ödemediğini” söyledi."
            .chars()
            .collect();
        let sliced: String = chars[quote.start..quote.end].iter().collect();
        assert_eq!(sliced, quote.text);
    }

    #[test]
    fn extracts_straight_guillemet_and_german_quotes() {
        let out = extract_quoted_spans(
            "Bir \"düz tırnak alıntısı\" ve «köşeli alıntı metni» ile „alman tipi alıntı“ var.",
        );
        assert_eq!(out.quotes.len(), 3);
        let styles: Vec<&str> = out.quotes.iter().map(|q| q.style).collect();
        assert_eq!(styles, vec!["straight", "guillemet", "german"]);
        assert_eq!(out.quotes[2].text, "alman tipi alıntı");
    }

    #[test]
    fn apostrophes_are_not_quote_delimiters() {
        let out = extract_quoted_spans("İstanbul'da “kira ödenmedi” denildi; avukat’ın notu.");
        assert_eq!(out.quotes.len(), 1);
        assert_eq!(out.quotes[0].text, "kira ödenmedi");
    }

    #[test]
    fn short_unclosed_and_long_spans_are_counted_not_returned() {
        let short = extract_quoted_spans("“kısa” ve “az”.");
        assert!(short.quotes.is_empty());
        assert_eq!(short.skipped_short, 2);

        let unclosed = extract_quoted_spans("“kapanmayan uzun bir alıntı metni");
        assert!(unclosed.quotes.is_empty());
        assert_eq!(unclosed.unclosed, 1);

        let long_text = "a".repeat(MAX_SPAN_CHARS + 1);
        let long = extract_quoted_spans(&format!("“{long_text}”"));
        assert!(long.quotes.is_empty());
        assert_eq!(long.skipped_long, 1);
    }

    #[test]
    fn exact_quote_is_found_once_and_position_binds_an_anchor() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        let (_, version_id) = add_document(
            &db,
            &case_id,
            "Dava dilekçesi",
            &[
                "Mahkeme, kiracının kira bedelini ödemediğini tespit etti.",
                "İkinci sayfa başka bir metindir.",
            ],
        );
        let text = "Davacı “kira bedelini ödemediğini tespit etti” iddiasındadır.";
        let report = check(&db, &case_id, text, None);
        assert_eq!(report.spans_found, 1);
        assert_eq!(report.exact, 1);
        assert_eq!(report.ambiguous, 0);
        assert_eq!(report.pages_scanned, 2);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "exact");
        assert_eq!(finding.occurrences, 1);
        assert_eq!(finding.distinct_documents, 1);
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.document_version_id, version_id);
        assert_eq!(position.page_index, 0);
        assert_eq!(position.end_page_index, 0);
        assert!(!position.spans_pages);
        assert_eq!(position.match_kind, "exact");
        assert_eq!(
            position.source_text,
            "kira bedelini ödemediğini tespit etti"
        );
        assert_eq!(position.char_start, 19);

        // Konum doğrudan anchor'a bağlanabilir: aynı doğrulama yeniden geçer.
        let anchor = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: position.document_version_id.clone(),
                parse_run_id: position.parse_run_id.clone(),
                segment_id: position.segment_id.clone().expect("segment"),
                start_offset: position.char_start,
                end_offset: position.char_end,
                exact_quote: position.source_text.clone(),
            })
            .expect("anchor");
        assert_eq!(anchor.exact_quote, finding.quote);
        assert_eq!(anchor.start_offset, 19);
        assert_eq!(anchor.end_offset, 19 + 37);
        let location = db.locate_anchor(&anchor.id).expect("locate").expect("row");
        assert!(location.quote_intact);
    }

    #[test]
    fn quote_in_two_documents_is_ambiguous_with_distinct_documents() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Dilekçe",
            &["Kiracı kira bedelini ödemedi ve temerrüde düştü."],
        );
        add_document(
            &db,
            &case_id,
            "Bilirkişi raporu",
            &["Kiracı kira bedelini ödemedi; bu nedenle tahliye istendi."],
        );
        let report = check(
            &db,
            &case_id,
            "“kira bedelini ödemedi” ifadesi geçiyor.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "ambiguous");
        assert_eq!(finding.occurrences, 2);
        assert_eq!(finding.distinct_documents, 2);
        assert!(finding.position.is_none());
        assert_eq!(finding.candidates.len(), 2);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("Birden fazla konumda")));
    }

    #[test]
    fn repeated_quote_on_one_page_is_ambiguous() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Tutanak",
            &["Kira bedeli ödenmedi. Kira bedeli ödenmedi. Kira bedeli ödenmedi."],
        );
        let report = check(&db, &case_id, "“kira bedeli ödenmedi” yazıyor.", None);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "ambiguous");
        assert_eq!(finding.occurrences, 3);
        assert_eq!(finding.distinct_documents, 1);
        assert_eq!(finding.candidates.len(), 3);
    }

    #[test]
    fn whitespace_only_difference_is_reported_as_normalized_with_source_text() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Karar",
            &["Mahkeme ödeme   yapılmadığını ve temerrüdün oluştuğunu tespit etti."],
        );
        let report = check(&db, &case_id, "“ödeme yapılmadığını” tespiti.", None);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "whitespace");
        assert_eq!(position.source_text, "ödeme   yapılmadığını");
        assert_eq!(
            normalize_with_map(&position.source_text).text,
            normalize_with_map(&finding.quote).text
        );
        // Birebir olmayan eşleşme anchor'a bağlanmaz: çekirdek reddeder.
        let error = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: position.document_version_id.clone(),
                parse_run_id: position.parse_run_id.clone(),
                segment_id: position.segment_id.clone().expect("segment"),
                start_offset: position.char_start,
                end_offset: position.char_start
                    + i64::try_from(finding.quote.chars().count()).unwrap_or(i64::MAX),
                exact_quote: finding.quote.clone(),
            })
            .expect_err("normalized quote must not bind");
        assert_eq!(error.code(), "E_QUOTE_NOT_FOUND");
    }

    #[test]
    fn turkish_fold_is_a_separate_weaker_tier() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(&db, &case_id, "Karar", &["ÖDEME YAPILMADIĞI sabittir."]);
        let report = check(&db, &case_id, "“ödeme yapılmadığı” sabittir.", None);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "turkish_fold");
        assert_eq!(position.source_text, "ÖDEME YAPILMADIĞI");
    }

    #[test]
    fn missing_quote_is_not_found_without_position() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(&db, &case_id, "Dilekçe", &["Kiracı kira bedelini ödedi."]);
        let report = check(
            &db,
            &case_id,
            "“tamamen uydurma bir pasaj burada” deniyor.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "not_found");
        assert_eq!(finding.occurrences, 0);
        assert!(finding.position.is_none());
        assert_eq!(report.not_found, 1);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("“uydurma” hükmü değildir")));
    }

    #[test]
    fn document_scope_excludes_other_documents_and_says_so() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Dilekçe",
            &["Kiracı kira bedelini ödemediğini beyan etti."],
        );
        let (other_document, _) = add_document(
            &db,
            &case_id,
            "Rapor",
            &["Raporda ödeme yapıldığı belirtilmiştir."],
        );
        let report = check(
            &db,
            &case_id,
            "“kira bedelini ödemediğini” beyanı.",
            Some(&other_document),
        );
        assert_eq!(report.scope, "document");
        assert_eq!(
            report.scope_document_id.as_deref(),
            Some(other_document.as_str())
        );
        assert_eq!(report.quotes[0].state, "not_found");
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("Yalnız “Rapor” belgesinin güncel sürümü tarandı")));
    }

    #[test]
    fn document_from_another_case_is_rejected() {
        let db = db();
        let case_a = new_case(&db, "A");
        let case_b = new_case(&db, "B");
        let (document, _) = add_document(&db, &case_a, "Belge", &["metin"]);

        let error = db
            .check_in_text_quotes(
                &case_b,
                &InTextQuoteInput {
                    text: "“uzun bir alıntı metni” var.".into(),
                    document_id: Some(document),
                },
            )
            .expect_err("foreign document");
        assert_eq!(error.code(), "E_NOT_FOUND");
    }

    #[test]
    fn document_without_parse_is_counted_and_scan_still_works() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Dilekçe",
            &["Kiracı kira bedelini ödemedi ve temerrüt oluştu."],
        );
        add_empty_document(&db, &case_id, "Taranmamış belge");
        let report = check(&db, &case_id, "“kira bedelini ödemedi” ifadesi.", None);
        assert_eq!(report.documents_total, 2);
        assert_eq!(report.documents_with_text, 1);
        assert_eq!(report.documents_without_text, 1);
        assert_eq!(report.quotes[0].state, "exact");
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("1 belgenin güncel sürümünde ayrıştırılmış metin yok")));
    }

    #[test]
    fn no_text_scope_is_unverifiable_not_not_found() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_empty_document(&db, &case_id, "Taranmamış belge");
        let report = check(&db, &case_id, "“uzun bir alıntı metni” var.", None);
        assert_eq!(report.pages_scanned, 0);
        assert_eq!(report.quotes[0].state, "unverifiable");
        assert_eq!(report.quotes[0].reason.as_deref(), Some("no_text"));
        assert_eq!(report.unverifiable, 1);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("Taranacak ayrıştırılmış metin bulunamadı")));
    }

    #[test]
    fn quote_spanning_two_pages_is_found_with_its_page_range() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Karar",
            &[
                "Kiracı kira bedelini",
                "ödemediği için tahliyeye karar verildi.",
            ],
        );
        let report = check(&db, &case_id, "“kira bedelini ödemediği” ifadesi.", None);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        assert_eq!(finding.occurrences, 1);
        assert_eq!(report.normalized, 1);
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.page_index, 0);
        assert_eq!(position.end_page_index, 1);
        assert!(position.spans_pages);
        assert_eq!(position.match_kind, "whitespace");
        assert_eq!(position.char_start, 7);
        assert_eq!(position.char_end, 9);
        assert_eq!(position.source_text, "kira bedelini\nödemediği");
        assert!(report
            .notes
            .iter()
            .any(|note| note.contains("sayfa sınırını aşarak bulundu")));
        // Sayfa sınırını aşan konum tek sayfalık anchor'a bağlanamaz: çekirdek
        // reddeder (arayüz de düğmeyi kapatır).
        let error = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: position.document_version_id.clone(),
                parse_run_id: position.parse_run_id.clone(),
                segment_id: position.segment_id.clone().expect("segment"),
                start_offset: position.char_start,
                // Arayüz `charEnd` gönderir (son sayfadaki konum): tek segmentte
                // uzunluk tutmaz, çekirdek aralığı reddeder. Yani sayfa sınırını
                // aşan alıntı **tek segmentlik anchor olarak ifade edilemez**.
                end_offset: position.char_end,
                exact_quote: finding.quote.clone(),
            })
            .expect_err("sayfa sınırını aşan alıntı bağlanmamalı");
        assert_eq!(error.code(), "E_DB_SETTING");
    }

    #[test]
    fn exact_quote_that_carries_the_page_break_stays_exact_but_is_not_bindable() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(&db, &case_id, "Karar", &["Karar gerekçesi", "şöyledir"]);
        // Alıntı sayfa sonundaki satır sonunu **taşıyor**: katman `exact`.
        let report = check(&db, &case_id, "“Karar gerekçesi\nşöyledir” denildi.", None);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "exact");
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "exact");
        assert_eq!(position.page_index, 0);
        assert_eq!(position.end_page_index, 1);
        assert!(position.spans_pages);
        // Birebir olsa da tek sayfaya sığmadığı için bağlanamaz; "birebir tek
        // konumda" notu bu alıntıyı saymaz.
        assert!(!report
            .notes
            .iter()
            .any(|note| note.contains("kaynak bağlanabilir")));
        assert!(report
            .notes
            .iter()
            .any(|note| note.contains("tek sayfalık anchor'a bağlanmaz")));
    }

    #[test]
    fn cross_page_match_is_ambiguous_when_it_repeats_over_two_breaks() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Tutanak",
            &[
                "kira bedelini",
                "ödemediği birinci kayıt",
                "kira bedelini",
                "ödemediği ikinci kayıt",
            ],
        );
        let report = check(&db, &case_id, "“kira bedelini ödemediği” yazıyor.", None);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "ambiguous");
        assert_eq!(finding.occurrences, 2);
        assert_eq!(finding.candidates.len(), 2);
        assert!(finding
            .candidates
            .iter()
            .all(|candidate| candidate.spans_pages));
        assert_eq!(finding.candidates[0].page_index, 0);
        assert_eq!(finding.candidates[1].page_index, 2);
    }

    #[test]
    fn cross_page_match_works_in_the_fold_tier() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Karar",
            &["ÖDEME YAPILMADIĞINI", "tespit etti."],
        );
        let report = check(
            &db,
            &case_id,
            "“ödeme yapılmadığını tespit” sabittir.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "turkish_fold");
        assert!(position.spans_pages);
        assert_eq!(position.page_index, 0);
        assert_eq!(position.end_page_index, 1);
        assert_eq!(position.source_text, "ÖDEME YAPILMADIĞINI\ntespit");
    }

    #[test]
    fn text_without_quotes_returns_empty_report() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(&db, &case_id, "Belge", &["metin"]);
        let report = check(&db, &case_id, "Tırnaksız bir iddia metni.", None);
        assert_eq!(report.spans_found, 0);
        assert!(report.quotes.is_empty());
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("çift tırnaklı alıntı bulunamadı")));
    }

    #[test]
    fn too_many_quotes_and_too_long_text_are_rejected() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        let many = vec!["“alıntı metni bir”"; MAX_IN_TEXT_QUOTES + 1].join(" ");
        let error = db
            .check_in_text_quotes(
                &case_id,
                &InTextQuoteInput {
                    text: many,
                    document_id: None,
                },
            )
            .expect_err("too many");
        assert_eq!(error.code(), "E_QUOTE_TOO_MANY");

        let long = "a".repeat(MAX_IN_TEXT_CHECK_CHARS + 1);
        let error = db
            .check_in_text_quotes(
                &case_id,
                &InTextQuoteInput {
                    text: long,
                    document_id: None,
                },
            )
            .expect_err("too long");
        assert_eq!(error.code(), "E_QUOTE_TEXT");
    }

    #[test]
    fn unknown_case_is_not_found() {
        let db = db();
        let error = db
            .check_in_text_quotes(
                "bilinmeyen",
                &InTextQuoteInput {
                    text: "“uzun bir alıntı metni”".into(),
                    document_id: None,
                },
            )
            .expect_err("unknown case");
        assert_eq!(error.code(), "E_NOT_FOUND");
    }

    #[test]
    fn quote_missing_from_the_current_version_is_found_in_history() {
        let db = db();
        let case_id = new_case(&db, "Bilirkişi");
        let (document_id, first_version) = add_document(
            &db,
            &case_id,
            "Bilirkişi raporu",
            &["Raporun eski sürümünde kalan cümle burada."],
        );
        let current_version = add_version_with_pages(
            &db,
            &case_id,
            &document_id,
            &["Güncel metin, cümle çıkarıldı."],
        );

        let report = check(&db, &case_id, "Raporda “kalan cümle burada” yazıyor.", None);

        assert_eq!(report.exact, 1, "alıntı eski sürümde birebir bulunmalı");
        assert_eq!(report.not_found, 0);
        assert_eq!(report.found_in_old_version, 1);
        assert_eq!(report.historical_versions_scanned, 1);
        assert_eq!(report.versions_scanned, 2);
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert_eq!(position.document_version_id, first_version);
        assert_ne!(position.document_version_id, current_version);
        assert_eq!(position.version_no, 1);
        assert!(!position.is_current_version);
        assert!(report
            .notes
            .iter()
            .any(|note| note.contains("yalnız **eski** bir sürümde bulundu")));
    }

    #[test]
    fn historical_quote_binds_an_anchor_to_the_old_version() {
        let db = db();
        let case_id = new_case(&db, "Bilirkişi");
        let (document_id, first_version) = add_document(
            &db,
            &case_id,
            "Bilirkişi raporu",
            &["Raporun eski sürümünde kalan cümle burada."],
        );
        add_version_with_pages(
            &db,
            &case_id,
            &document_id,
            &["Güncel metin, cümle çıkarıldı."],
        );

        let report = check(&db, &case_id, "“kalan cümle burada”", None);
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert!(!position.is_current_version);

        // Eski sürümün segmenti üzerinden anchor kurulabilir: kayıt hangi
        // sürümde doğrulandığını taşır, sessizce güncel sürüme yazılmaz.
        let anchor = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: position.document_version_id.clone(),
                parse_run_id: position.parse_run_id.clone(),
                segment_id: position.segment_id.clone().expect("segment"),
                start_offset: position.char_start,
                end_offset: position.char_end,
                exact_quote: position.source_text.clone(),
            })
            .expect("anchor");
        assert_eq!(anchor.document_version_id, first_version);
        assert_eq!(anchor.exact_quote, "kalan cümle burada");
        let location = db.locate_anchor(&anchor.id).expect("locate").expect("row");
        assert!(location.quote_intact);
    }

    #[test]
    fn current_version_wins_over_history_and_history_is_not_scanned() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        let (document_id, _) =
            add_document(&db, &case_id, "Sözleşme", &["Karar gerekçesi şöyledir."]);
        add_version_with_pages(&db, &case_id, &document_id, &["Karar gerekçesi şöyledir."]);

        let report = check(
            &db,
            &case_id,
            "Metinde “Karar gerekçesi şöyledir” deniyor.",
            None,
        );

        assert_eq!(report.exact, 1);
        assert_eq!(
            report.quotes[0].occurrences, 1,
            "güncel sürüm kazandı, yinelenmez"
        );
        assert_eq!(report.found_in_old_version, 0);
        assert_eq!(
            report.historical_versions_scanned, 0,
            "güncel sürümde bulunan alıntı için eski sürümler taranmaz"
        );
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert!(position.is_current_version);
        assert_eq!(position.version_no, 2);
    }

    #[test]
    fn exact_history_beats_normalized_current_version() {
        let db = db();
        let case_id = new_case(&db, "Karar");
        let (document_id, first_version) =
            add_document(&db, &case_id, "Karar metni", &["Karar gerekçesi şöyledir."]);
        // Güncel sürümde yalnız **boşluk farkıyla** geçiyor: `whitespace` katmanı.
        add_version_with_pages(
            &db,
            &case_id,
            &document_id,
            &["Karar  gerekçesi    şöyledir."],
        );

        let report = check(&db, &case_id, "“Karar gerekçesi şöyledir”", None);

        assert_eq!(report.exact, 1, "eski sürümdeki birebir eşleşme üstündür");
        assert_eq!(report.normalized, 0);
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert_eq!(position.match_kind, "exact");
        assert_eq!(position.version_no, 1);
        assert_eq!(position.document_version_id, first_version);
        assert!(!position.is_current_version);
    }

    #[test]
    fn document_scope_does_not_scan_another_documents_history() {
        let db = db();
        let case_id = new_case(&db, "Dosya");
        let (first_document, _) =
            add_document(&db, &case_id, "Dilekçe", &["Dilekçe güncel metni."]);
        let (second_document, _) = add_document(
            &db,
            &case_id,
            "Ek rapor",
            &["Ek raporda kalan cümle burada."],
        );
        add_version_with_pages(&db, &case_id, &second_document, &["Ek rapor güncel metni."]);

        let scoped = check(&db, &case_id, "“kalan cümle burada”", Some(&first_document));
        assert_eq!(scoped.not_found, 1, "başka belgenin eski sürümü taranmaz");
        assert_eq!(scoped.found_in_old_version, 0);

        let whole_case = check(&db, &case_id, "“kalan cümle burada”", None);
        assert_eq!(whole_case.exact, 1);
        assert_eq!(whole_case.found_in_old_version, 1);
    }

    #[test]
    fn old_versions_beyond_the_limit_are_reported_not_silently_skipped() {
        let db = db();
        let case_id = new_case(&db, "Dosya");
        let (document_id, _) = add_document(
            &db,
            &case_id,
            "Sözleşme",
            &["En eski sürümde kalan cümle burada."],
        );
        // v2..v6 eski sürümler (sınır 5), v7 güncel; alıntı yalnız v1'de.
        for index in 2..=MAX_HISTORICAL_VERSIONS_PER_DOCUMENT + 2 {
            let body = format!("Sürüm {index} gövde metni.");
            add_version_with_pages(&db, &case_id, &document_id, &[body.as_str()]);
        }

        let report = check(&db, &case_id, "“kalan cümle burada”", None);

        assert_eq!(report.not_found, 1, "v1 sınırın ötesinde: taranmadı");
        assert_eq!(
            report.historical_versions_scanned,
            MAX_HISTORICAL_VERSIONS_PER_DOCUMENT
        );
        assert!(report
            .notes
            .iter()
            .any(|note| note.contains("en çok 5 eski sürüm tarandı")));
    }
}
