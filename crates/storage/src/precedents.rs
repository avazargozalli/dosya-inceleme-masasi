//! Emsal kütüphanesi ve atıf kontrolü.
//!
//! Kural: “bulunamadı” ile “uydurulmuş” birbirine dönüştürülmez. Bir kararın
//! künyesi eksikse `incomplete` işaretlenir; kullanıcı notu hukuki uygunluk
//! değerlendirmesidir, sistem hükmü değildir.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parsing::{docket_match, DocketNumber, MatchScratch, SearchMatcher, SearchQuery};
use crate::{now_iso, Database, SearchMode, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentRow {
    pub id: String,
    pub court: String,
    pub chamber: Option<String>,
    pub docket_no: Option<String>,
    pub decision_no: Option<String>,
    pub application_no: Option<String>,
    pub decision_date: Option<String>,
    pub title: Option<String>,
    pub source_url: Option<String>,
    pub user_tags: Vec<String>,
    pub legal_issue: Option<String>,
    pub completeness: String,
    pub created_at: String,
    /// F12-T11 — kullanım dayanağı (`official_public|licence|permission|internal_only|unknown`).
    pub usage_basis: String,
    /// F12-T11 — yeniden dağıtım izni (`allowed|not_allowed|unknown`).
    pub redistribution: String,
    /// Koşul kaydının alındığı zaman; yoksa `None` (kayıt yok).
    pub terms_recorded_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewPrecedent {
    pub court: String,
    #[serde(default)]
    pub chamber: Option<String>,
    #[serde(default)]
    pub docket_no: Option<String>,
    #[serde(default)]
    pub decision_no: Option<String>,
    #[serde(default)]
    pub application_no: Option<String>,
    #[serde(default)]
    pub decision_date: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub user_tags: Vec<String>,
    #[serde(default)]
    pub legal_issue: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentNoteRow {
    pub id: String,
    pub precedent_id: String,
    pub case_id_optional: Option<String>,
    pub useful_passage_anchor: Option<String>,
    pub similarity_note: String,
    pub distinguishing_note: String,
    pub user_conclusion: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewPrecedentNote {
    #[serde(default)]
    pub case_id_optional: Option<String>,
    #[serde(default)]
    pub useful_passage_anchor: Option<String>,
    #[serde(default)]
    pub similarity_note: String,
    #[serde(default)]
    pub distinguishing_note: String,
    #[serde(default)]
    pub user_conclusion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationCheckRow {
    pub id: String,
    pub input_document_version_id: String,
    pub extracted_citation: String,
    pub candidate_precedent_id: Option<String>,
    pub identity_match: String,
    pub quote_match: String,
    pub meaning_review: String,
    pub meaning_note: String,
    pub meaning_reviewed_at: Option<String>,
    pub reason_codes: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationContext {
    pub found: bool,
    pub before: String,
    pub quote: String,
    pub after: String,
    /// Alıntının tam metindeki karakter konumu (bulunamadıysa `None`).
    pub char_position: Option<i64>,
    pub state: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentBibliography {
    pub precedent_id: String,
    /// Kopyalanabilir bibliyografik künye metni.
    pub text: String,
    /// Bulunmayan künye alanları (etiket). Eksik alan **tahmin edilmez**.
    pub missing: Vec<String>,
    pub warnings: Vec<String>,
    pub source_url: Option<String>,
    /// `dd.mm.yyyy` biçiminde erişim/edinim tarihi.
    pub accessed_at: String,
    /// `version` = en yeni tam metin sürümünün edinim zamanı,
    /// `record` = künye kaydının oluşturulma zamanı.
    pub access_basis: String,
    pub has_full_text: bool,
}

/// Emsal kütüphanesi araması (F12-T10). Kapsam **açıkça** kütüphanedir; aktif
/// dosyanın belge araması bu sorguya girmez ve sonuçlar oraya karışmaz.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentSearchQuery {
    /// Aranan metin; dosya aramasıyla **aynı** kip/tırnak kuralına tabidir.
    pub text: String,
    #[serde(default)]
    pub mode: SearchMode,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentSearchHit {
    pub precedent_id: String,
    pub court: String,
    pub chamber: Option<String>,
    pub docket_no: Option<String>,
    pub decision_no: Option<String>,
    pub decision_date: Option<String>,
    pub title: Option<String>,
    /// Eşleşmenin bulunduğu tam metin sürümü; metin gövdesi dönmez.
    pub version_id: String,
    pub acquired_at: String,
    pub source_kind: String,
    /// Eşleşme çevresinden pencere (en çok 240 karakter, “…” ile kırpılır).
    pub snippet: String,
    /// Pencerenin tam metindeki başlangıç karakteri.
    pub snippet_start: i64,
    /// Eşleşmenin tam metindeki karakter konumu; yalnız birebir/boşluk/katlama
    /// ile bulunabildiyse dolar. Bulunamazsa `None` — konum uydurulmaz.
    pub match_start: Option<i64>,
    pub match_length: Option<i64>,
    /// `full_text` = eşleşme kararın tam metninde, `metadata` = yalnız künye
    /// numaralarında (esas/karar/başvuru no).
    pub match_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentSearchPage {
    /// Her zaman `"precedent_library"`; arayüz kapsamı bununla yazar.
    pub scope: String,
    /// Çözümlenmiş kip (`auto` dönmez).
    pub mode: String,
    pub query: String,
    /// Tırnaklar atılmış, kipe göre çözümlenmiş metin.
    pub resolved_query: String,
    pub hits: Vec<PrecedentSearchHit>,
    /// Taranan tam metin sayısı.
    pub scanned: i64,
    /// Eşleşen emsal sayısı (gösterilenden fazla olabilir).
    pub matched: i64,
    /// Tam metni olmadığı için **aranamayan** emsal sayısı.
    pub without_text: i64,
    pub limit: u32,
    pub truncated: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCitationCheck {
    pub input_document_version_id: String,
    pub extracted_citation: String,
    #[serde(default)]
    pub candidate_precedent_id: Option<String>,
    #[serde(default = "match_unknown")]
    pub identity_match: String,
    #[serde(default = "match_unknown")]
    pub quote_match: String,
    #[serde(default = "meaning_unreviewed")]
    pub meaning_review: String,
    #[serde(default)]
    pub reason_codes: Vec<String>,
}

fn match_unknown() -> String {
    "unknown".to_string()
}
fn meaning_unreviewed() -> String {
    "not_reviewed".to_string()
}

/// Kararın **tam metin** sürümü (F12-T05). Metnin kendisi UI'ya gönderilmez;
/// yalnız hash ve karakter sayısı taşınır.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentVersionRow {
    pub id: String,
    pub precedent_id: String,
    pub document_version_id: Option<String>,
    pub acquired_at: String,
    pub source_kind: String,
    pub body_hash: String,
    /// `text_verified` = tam metin elimizde ve hash'i kayıtlı. Künye
    /// doğruluğu ayrı boyuttur; bu durum künyenin doğru olduğunu söylemez.
    pub verification_state: String,
    pub char_count: i64,
    /// Tam metin elde mi. `false` ise alıntı doğrulaması yapılamaz.
    pub has_text: bool,
    /// Aynı künyeyle **farklı** metin geldi (F12-T06). Sessiz birleştirme
    /// yapılmadı; kullanıcı iki nüshayı karşılaştırmalı.
    pub identity_conflict: bool,
    /// Çakışmanın karşılaştırılacağı önceki sürüm.
    pub conflict_with_version_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewPrecedentText {
    /// `paste` (kullanıcı yapıştırdı), `document` (kasadaki belge sürümü) veya
    /// `file` (dosyadan okundu).
    pub source_kind: String,
    pub text: String,
    #[serde(default)]
    pub document_version_id: Option<String>,
}

/// Kasadaki bir belge sürümünden emsale tam metin aktarmanın sonucu (F12-T13).
///
/// `version` yazılan (ya da aynı metin zaten varsa **var olan**) sürümdür.
/// `page_texts` **çıkarılmış** metindir: PDF/DOCX dosyasının kendisi kopyalanmaz,
/// sayfa metinleri birleştirilir. Bu yüzden OCR/boş sayfa sayıları ayrıca döner.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentDocumentText {
    pub version: PrecedentVersionRow,
    /// Metnin alındığı ayrıştırma koşusu.
    pub parse_run_id: String,
    pub engine: String,
    /// `succeeded` | `partial` — kısmi ayrıştırmada eksik sayfa olabilir.
    pub parse_status: String,
    pub pages: i64,
    /// `text_origin IN ('ocr','mixed')` olan sayfa sayısı.
    pub ocr_pages: i64,
    /// Metni boş olduğu için birleştirmeye girmeyen sayfa sayısı.
    pub empty_pages: i64,
    pub char_count: i64,
    /// `false` ise aynı SHA-256 zaten kayıtlıydı; yeni sürüm yazılmadı.
    pub written: bool,
    pub notes: Vec<String>,
}

/// Kasadaki belge sürümünden üretilecek tam metnin üst sınırı. Aşılırsa metin
/// **yazılmaz**; sessizce kırpılmaz.
pub const MAX_PRECEDENT_DOCUMENT_TEXT_BYTES: usize = 16 * 1024 * 1024;

/// Metin içi alıntı doğrulaması (F12-T05). `state`:
/// - `match`: alıntı metinde bulundu; `method` farkın türünü söyler.
/// - `mismatch`: tam metin elimizde ama alıntı bulunamadı.
/// - `not_checked`: metin yok ya da alıntı anlamlı doğrulama için çok kısa.
///
/// Hiçbir durum tek başına “uydurulmuş” hükmü değildir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteVerification {
    pub precedent_id: String,
    pub version_id: Option<String>,
    pub state: String,
    /// `exact` | `whitespace` | `turkish_fold`
    pub method: Option<String>,
    /// Alıntının **kaynak metindeki** karakter konumu (0 tabanlı).
    pub matched_start: Option<i64>,
    pub matched_length: Option<i64>,
    pub occurrences: i64,
    /// `no_full_text` | `quote_too_short` | `not_found`
    pub reason: Option<String>,
    pub quote: String,
    pub checked_at: String,
}

/// Alıntı doğrulaması için en kısa anlamlı alıntı. Daha kısa alıntı yanlış
/// kesinlik üretir: “reddine” gibi tek kelime hemen her kararda geçer.
const MIN_QUOTE_CHARS: usize = 8;

struct MappedText {
    text: String,
    /// `text` içindeki karakter indeksinin kaynak metindeki karakter indeksi.
    map: Vec<usize>,
}

fn fold_char(ch: char) -> char {
    match ch {
        'İ' | 'I' => 'i',
        'ı' => 'i',
        'Ş' | 'ş' => 's',
        'Ğ' | 'ğ' => 'g',
        'Ü' | 'ü' => 'u',
        'Ö' | 'ö' => 'o',
        'Ç' | 'ç' => 'c',
        'Â' | 'â' => 'a',
        'Î' | 'î' => 'i',
        'Û' | 'û' => 'u',
        c => c.to_lowercase().next().unwrap_or(c),
    }
}

/// `dim_evidence::normalize` ile **aynı** sadeleştirme, ama konum eşlemesiyle.
/// İkisi ayrışırsa `fold_matches_evidence_normalize` testi kırılır.
fn normalize_with_map(input: &str) -> MappedText {
    let mut out = String::with_capacity(input.len());
    let mut map = Vec::with_capacity(input.len());
    let mut last_space = false;
    for (index, ch) in input.chars().enumerate() {
        let mapped = fold_char(ch);
        if mapped.is_whitespace() {
            if !last_space && !out.is_empty() {
                out.push(' ');
                map.push(index);
            }
            last_space = true;
        } else {
            out.push(mapped);
            map.push(index);
            last_space = false;
        }
    }
    while out.ends_with(' ') {
        out.pop();
        map.pop();
    }
    MappedText { text: out, map }
}

/// Yalnız boşluk sadeleştirmesi; harfler **korunur** (birebir olmayan ama
/// yalnız satır/biçim farkından kaynaklanan eşleşme için).
fn collapse_ws_with_map(input: &str) -> MappedText {
    let mut out = String::with_capacity(input.len());
    let mut map = Vec::with_capacity(input.len());
    let mut last_space = false;
    for (index, ch) in input.chars().enumerate() {
        if ch.is_whitespace() {
            if !last_space && !out.is_empty() {
                out.push(' ');
                map.push(index);
            }
            last_space = true;
        } else {
            out.push(ch);
            map.push(index);
            last_space = false;
        }
    }
    while out.ends_with(' ') {
        out.pop();
        map.pop();
    }
    MappedText { text: out, map }
}

fn char_offset(text: &str, byte: usize) -> i64 {
    i64::try_from(text[..byte].chars().count()).unwrap_or(i64::MAX)
}

fn count_occurrences(haystack: &str, needle: &str) -> i64 {
    if needle.is_empty() {
        return 0;
    }
    let mut count = 0i64;
    let mut start = 0usize;
    while let Some(pos) = haystack[start..].find(needle) {
        count += 1;
        start += pos + needle.len();
        if start >= haystack.len() {
            break;
        }
    }
    count
}

fn mapped_start(mapped: &MappedText, at_byte: usize) -> Option<i64> {
    let index = mapped.text[..at_byte].chars().count();
    mapped
        .map
        .get(index)
        .map(|i| i64::try_from(*i).unwrap_or(i64::MAX))
}

fn valid_identity(value: &str) -> bool {
    matches!(value, "match" | "mismatch" | "unknown")
}
fn valid_quote(value: &str) -> bool {
    matches!(value, "match" | "mismatch" | "not_checked" | "unknown")
}

impl Database {
    pub fn create_precedent(&self, new: &NewPrecedent) -> Result<PrecedentRow, StorageError> {
        if new.court.trim().is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        let complete = new
            .docket_no
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty())
            && new
                .decision_no
                .as_deref()
                .is_some_and(|v| !v.trim().is_empty());
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO precedents (id,court,chamber,docket_no,decision_no,application_no,decision_date,title,source_url,user_tags,legal_issue,completeness,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            rusqlite::params![
                id,
                new.court.trim(),
                new.chamber.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                new.docket_no.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                new.decision_no.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                new.application_no.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                new.decision_date.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                new.title.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                new.source_url.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                serde_json::to_string(&new.user_tags).unwrap_or_else(|_| "[]".into()),
                new.legal_issue.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                if complete { "complete" } else { "incomplete" },
                now
            ],
        )?;
        self.append_audit(
            "precedent",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"court": new.court}),
        )?;
        self.get_precedent(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_precedent(&self, id: &str) -> Result<Option<PrecedentRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,court,chamber,docket_no,decision_no,application_no,decision_date,title,source_url,user_tags,legal_issue,completeness,created_at,usage_basis,redistribution,terms_recorded_at
             FROM precedents WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_precedent).optional()?)
    }

    pub fn list_precedents(&self) -> Result<Vec<PrecedentRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,court,chamber,docket_no,decision_no,application_no,decision_date,title,source_url,user_tags,legal_issue,completeness,created_at,usage_basis,redistribution,terms_recorded_at
             FROM precedents ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], map_precedent)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Emsal notu. `case_id_optional` verilirse o dosyaya özeldir; verilmezse
    /// kütüphane notudur. Başka dosyanın özel notu bu yolla taşınmaz.
    pub fn create_precedent_note(
        &self,
        precedent_id: &str,
        new: &NewPrecedentNote,
    ) -> Result<PrecedentNoteRow, StorageError> {
        if self.get_precedent(precedent_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        if let Some(case_id) = &new.case_id_optional {
            let count: i64 = self.conn().query_row(
                "SELECT count(*) FROM cases WHERE id=?1 AND deleted_at IS NULL",
                [case_id],
                |r| r.get(0),
            )?;
            if count == 0 {
                return Err(StorageError::NotFound);
            }
        }
        let id = uuid::Uuid::now_v7().to_string();
        self.conn().execute(
            "INSERT INTO precedent_notes (id,precedent_id,case_id_optional,useful_passage_anchor,similarity_note,distinguishing_note,user_conclusion,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            rusqlite::params![
                id,
                precedent_id,
                new.case_id_optional,
                new.useful_passage_anchor,
                new.similarity_note.trim(),
                new.distinguishing_note.trim(),
                new.user_conclusion.trim(),
                now_iso()
            ],
        )?;
        self.get_precedent_note(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_precedent_note(&self, id: &str) -> Result<Option<PrecedentNoteRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,precedent_id,case_id_optional,useful_passage_anchor,similarity_note,distinguishing_note,user_conclusion,created_at
             FROM precedent_notes WHERE id=?1",
        )?;
        Ok(stmt
            .query_row([id], |row| {
                Ok(PrecedentNoteRow {
                    id: row.get(0)?,
                    precedent_id: row.get(1)?,
                    case_id_optional: row.get(2)?,
                    useful_passage_anchor: row.get(3)?,
                    similarity_note: row.get(4)?,
                    distinguishing_note: row.get(5)?,
                    user_conclusion: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .optional()?)
    }

    pub fn list_precedent_notes(
        &self,
        precedent_id: &str,
    ) -> Result<Vec<PrecedentNoteRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,precedent_id,case_id_optional,useful_passage_anchor,similarity_note,distinguishing_note,user_conclusion,created_at
             FROM precedent_notes WHERE precedent_id=?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([precedent_id], |row| {
            Ok(PrecedentNoteRow {
                id: row.get(0)?,
                precedent_id: row.get(1)?,
                case_id_optional: row.get(2)?,
                useful_passage_anchor: row.get(3)?,
                similarity_note: row.get(4)?,
                distinguishing_note: row.get(5)?,
                user_conclusion: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Atıf kontrolü kaydı. `identity_match`/`quote_match` ayrı boyutlardır;
    /// hiçbiri tek başına “sahte” hükmü üretmez.
    pub fn record_citation_check(
        &self,
        new: &NewCitationCheck,
    ) -> Result<CitationCheckRow, StorageError> {
        if new.extracted_citation.trim().is_empty()
            || !valid_identity(&new.identity_match)
            || !valid_quote(&new.quote_match)
        {
            return Err(StorageError::InvalidSetting);
        }
        if let Some(precedent_id) = &new.candidate_precedent_id {
            if self.get_precedent(precedent_id)?.is_none() {
                return Err(StorageError::NotFound);
            }
        }
        let id = uuid::Uuid::now_v7().to_string();
        self.conn().execute(
            "INSERT INTO citation_checks (id,input_document_version_id,extracted_citation,candidate_precedent_id,identity_match,quote_match,meaning_review,reason_codes,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            rusqlite::params![
                id,
                new.input_document_version_id,
                new.extracted_citation.trim(),
                new.candidate_precedent_id,
                new.identity_match,
                new.quote_match,
                new.meaning_review,
                serde_json::to_string(&new.reason_codes).unwrap_or_else(|_| "[]".into()),
                now_iso()
            ],
        )?;
        self.append_audit(
            "citation_check",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"identity": new.identity_match, "quote": new.quote_match}),
        )?;
        self.get_citation_check(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_citation_check(&self, id: &str) -> Result<Option<CitationCheckRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,input_document_version_id,extracted_citation,candidate_precedent_id,identity_match,quote_match,meaning_review,meaning_note,meaning_reviewed_at,reason_codes,created_at
             FROM citation_checks WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_check).optional()?)
    }

    pub fn list_citation_checks(
        &self,
        version_id: &str,
    ) -> Result<Vec<CitationCheckRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,input_document_version_id,extracted_citation,candidate_precedent_id,identity_match,quote_match,meaning_review,meaning_note,meaning_reviewed_at,reason_codes,created_at
             FROM citation_checks WHERE input_document_version_id=?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([version_id], map_check)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Alıntının anlamını avukat değerlendirir. Künye ve birebir eşleşme
    /// ayrı boyutlardır; burada yalnız "kararın anlamını yansıtıyor mu?"
    /// sorusu kaydedilir. `misleading`/`off_context` için gerekçe zorunludur —
    /// gerekçesiz "yanlış" damgası basılamaz.
    pub fn review_citation_meaning(
        &self,
        id: &str,
        review: &str,
        note: &str,
    ) -> Result<CitationCheckRow, StorageError> {
        if !matches!(review, "accurate" | "misleading" | "off_context") {
            return Err(StorageError::InvalidSetting);
        }
        let note = note.trim();
        if review != "accurate" && note.is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        let changed = self.conn().execute(
            "UPDATE citation_checks SET meaning_review=?1, meaning_note=?2, meaning_reviewed_at=?3 WHERE id=?4",
            rusqlite::params![review, note, now_iso(), id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit(
            "citation_check",
            Some(id),
            "meaning_review",
            "local-user",
            &serde_json::json!({"review": review}),
        )?;
        self.get_citation_check(id)?.ok_or(StorageError::NotFound)
    }

    /// Alıntının tam metindeki çevresini döndürür: önce birebir doğrulama
    /// yapılır, sonra `radius` karakter öncesi/sonrası alınır. Metin yoksa
    /// `found=false` ve neden döner; uydurma bağlam üretilmez.
    pub fn citation_context(
        &self,
        check_id: &str,
        radius: usize,
    ) -> Result<CitationContext, StorageError> {
        let check = self
            .get_citation_check(check_id)?
            .ok_or(StorageError::NotFound)?;
        let mut context = CitationContext {
            found: false,
            before: String::new(),
            quote: check.extracted_citation.clone(),
            after: String::new(),
            char_position: None,
            state: "not_checked".into(),
            reason: None,
        };
        let Some(precedent_id) = check.candidate_precedent_id else {
            context.reason = Some("no_candidate".into());
            return Ok(context);
        };
        let verification = self.verify_precedent_quote(&precedent_id, &check.extracted_citation)?;
        context.state = verification.state.clone();
        context.reason = verification.reason.clone();
        let (Some(version_id), Some(start)) =
            (verification.version_id.clone(), verification.matched_start)
        else {
            return Ok(context);
        };
        let text: Option<String> = self
            .conn()
            .query_row(
                "SELECT full_text FROM precedent_versions WHERE id=?1",
                [&version_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(text) = text else {
            context.reason = Some("no_full_text".into());
            return Ok(context);
        };
        let start = usize::try_from(start).unwrap_or(0);
        let chars: Vec<char> = text.chars().collect();
        if start > chars.len() {
            return Ok(context);
        }
        let quote_len = check.extracted_citation.chars().count();
        let before_start = start.saturating_sub(radius.max(1));
        let after_end = (start + quote_len + radius.max(1)).min(chars.len());
        context.found = true;
        context.char_position = Some(i64::try_from(start).unwrap_or(i64::MAX));
        context.before = chars[before_start..start].iter().collect();
        context.after = chars[(start + quote_len).min(chars.len())..after_end]
            .iter()
            .collect();
        Ok(context)
    }

    /// Karar **tam metnini** kasaya kaydeder (F12-T05). Metin şifreli
    /// veritabanında kalır; dışarıya yalnız hash ve karakter sayısı çıkar.
    /// Kaynak türü ve varsa kasadaki belge sürümü denetlenir.
    pub fn add_precedent_text(
        &self,
        precedent_id: &str,
        new: &NewPrecedentText,
    ) -> Result<PrecedentVersionRow, StorageError> {
        if self.get_precedent(precedent_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let text = new.text.trim();
        if text.is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        let source_kind = new.source_kind.trim();
        if !matches!(source_kind, "paste" | "document" | "file") {
            return Err(StorageError::InvalidSetting);
        }
        let document_version_id = new
            .document_version_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        if let Some(version_id) = document_version_id {
            let count: i64 = self.conn().query_row(
                "SELECT count(*) FROM document_versions WHERE id=?1",
                [version_id],
                |r| r.get(0),
            )?;
            if count == 0 {
                return Err(StorageError::NotFound);
            }
        }
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        let body_hash = hex::encode(hasher.finalize());
        let char_count = i64::try_from(text.chars().count()).unwrap_or(i64::MAX);
        // Sürüm yazımı toplu içe alma ile **aynı** yoldan geçer (F12-T06);
        // yapıştırma yolunda çakışma işareti konmaz.
        self.insert_precedent_version(
            precedent_id,
            text,
            &body_hash,
            char_count,
            source_kind,
            document_version_id,
            None,
        )
    }

    /// Başarılı ya da kısmi son ayrıştırma koşusunun sayfa metinlerinden emsal
    /// tam metni üretir (F12-T13). Dosyanın kendisi kopyalanmaz: metin
    /// **çıkarılmış** hâldir ve hangi koşudan geldiği sonuçta yazılır.
    ///
    /// - Ayrıştırma yoksa / metin boşsa metin yazılmaz (`E_PRECEDENT_SOURCE`).
    /// - Sayfa metinleri çift satır sonuyla birleştirilir; boş sayfalar atlanır
    ///   ve sayısı raporlanır. Sayfa numarası metne **eklenmez** (uydurma künye
    ///   olurdu).
    /// - Aynı SHA-256 bu emsalde zaten varsa yeni sürüm yazılmaz; var olan
    ///   sürüm `written=false` ile döner.
    pub fn precedent_text_from_document(
        &self,
        precedent_id: &str,
        document_version_id: &str,
    ) -> Result<PrecedentDocumentText, StorageError> {
        if self.get_precedent(precedent_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let version_id = document_version_id.trim();
        if version_id.is_empty() || self.get_version(version_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let run = self
            .list_parse_runs(version_id)?
            .into_iter()
            .find(|r| matches!(r.status.as_str(), "succeeded" | "partial"))
            .ok_or_else(|| {
                StorageError::PrecedentSource(
                    "belge sürümünün başarılı ya da kısmi ayrıştırması yok".into(),
                )
            })?;
        let pages = self.pages_for_run(&run.id)?;
        if pages.is_empty() {
            return Err(StorageError::PrecedentSource(
                "ayrıştırılmış sayfa bulunamadı".into(),
            ));
        }
        let mut body = String::new();
        let mut ocr_pages = 0i64;
        let mut empty_pages = 0i64;
        for page in &pages {
            if matches!(page.text_origin.as_str(), "ocr" | "mixed") {
                ocr_pages += 1;
            }
            let text = page.text.trim();
            if text.is_empty() {
                empty_pages += 1;
                continue;
            }
            if !body.is_empty() {
                body.push_str("\n\n");
            }
            body.push_str(text);
        }
        if body.trim().is_empty() {
            return Err(StorageError::PrecedentSource(
                "ayrıştırılmış sayfaların metni boş".into(),
            ));
        }
        if body.len() > MAX_PRECEDENT_DOCUMENT_TEXT_BYTES {
            return Err(StorageError::PrecedentSource(format!(
                "çıkarılan metin {MAX_PRECEDENT_DOCUMENT_TEXT_BYTES} bayt sınırını aşıyor"
            )));
        }
        let mut hasher = Sha256::new();
        hasher.update(body.as_bytes());
        let body_hash = hex::encode(hasher.finalize());
        let char_count = i64::try_from(body.chars().count()).unwrap_or(i64::MAX);
        let mut notes = vec![
            "Metin kasadaki belgeden çıkarıldı (dosyanın kopyası değil); sayfa sınırları çift satır sonuyla ayrıldı."
                .to_string(),
        ];
        if ocr_pages > 0 {
            notes.push(format!(
                "{ocr_pages} sayfa OCR ile çıkarıldı; alıntı doğrulaması bu metne göre yapılır."
            ));
        }
        if empty_pages > 0 {
            notes.push(format!(
                "{empty_pages} sayfa metinsiz; birleştirmeye alınmadı."
            ));
        }
        if run.status == "partial" {
            notes.push("Ayrıştırma kısmi (partial); eksik sayfa olabilir.".to_string());
        }

        let existing: Option<String> = self
            .conn()
            .query_row(
                "SELECT id FROM precedent_versions WHERE precedent_id=?1 AND body_hash=?2 LIMIT 1",
                rusqlite::params![precedent_id, body_hash],
                |row| row.get(0),
            )
            .optional()?;
        let written = existing.is_none();
        if !written {
            notes.push(
                "Aynı metin (aynı SHA-256) bu emsalde zaten kayıtlı; yeni sürüm yazılmadı."
                    .to_string(),
            );
        }
        let version = if let Some(id) = existing {
            self.get_precedent_version(&id)?
                .ok_or(StorageError::NotFound)?
        } else {
            let inserted = self.insert_precedent_version(
                precedent_id,
                &body,
                &body_hash,
                char_count,
                "document",
                Some(version_id),
                None,
            )?;
            // Hangi koşudan geldiği denetimde kalsın; metin denetime yazılmaz.
            self.append_audit(
                "precedent_version",
                Some(&inserted.id),
                "text_from_document",
                "local-user",
                &serde_json::json!({
                    "documentVersionId": version_id,
                    "parseRunId": run.id,
                    "pages": i64::try_from(pages.len()).unwrap_or(i64::MAX),
                    "ocrPages": ocr_pages,
                    "emptyPages": empty_pages,
                    "charCount": char_count,
                }),
            )?;
            inserted
        };
        Ok(PrecedentDocumentText {
            version,
            parse_run_id: run.id,
            engine: run.engine,
            parse_status: run.status,
            pages: i64::try_from(pages.len()).unwrap_or(i64::MAX),
            ocr_pages,
            empty_pages,
            char_count,
            written,
            notes,
        })
    }

    pub fn get_precedent_version(
        &self,
        id: &str,
    ) -> Result<Option<PrecedentVersionRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,precedent_id,document_version_id,acquired_at,source_kind,body_hash,verification_state,text_char_count,
                    (full_text IS NOT NULL AND length(full_text)>0),identity_conflict,conflict_with_version_id
             FROM precedent_versions WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_version).optional()?)
    }

    /// Emsalin tam metin sürümleri; en yeni önce. Metin gövdesi dönmez.
    pub fn precedent_versions(
        &self,
        precedent_id: &str,
    ) -> Result<Vec<PrecedentVersionRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,precedent_id,document_version_id,acquired_at,source_kind,body_hash,verification_state,text_char_count,
                    (full_text IS NOT NULL AND length(full_text)>0),identity_conflict,conflict_with_version_id
             FROM precedent_versions WHERE precedent_id=?1
             ORDER BY acquired_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([precedent_id], map_version)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Metin içi alıntıyı emsalin **en yeni tam metninde** doğrular
    /// (F12-T05). Yalnız okuma yapar; sonuç kaydı ayrı atıf kontrolüdür.
    /// Tam metin yoksa `not_checked` döner — bu “uydurulmuş” demek değildir.
    pub fn verify_precedent_quote(
        &self,
        precedent_id: &str,
        quote: &str,
    ) -> Result<QuoteVerification, StorageError> {
        if self.get_precedent(precedent_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let quote = quote.trim();
        let quote_chars = quote.chars().count();
        let mut result = QuoteVerification {
            precedent_id: precedent_id.to_string(),
            version_id: None,
            state: "not_checked".into(),
            method: None,
            matched_start: None,
            matched_length: None,
            occurrences: 0,
            reason: None,
            quote: quote.to_string(),
            checked_at: now_iso(),
        };
        if quote_chars < MIN_QUOTE_CHARS {
            result.reason = Some("quote_too_short".into());
            return Ok(result);
        }
        let latest: Option<(String, String)> = self
            .conn()
            .query_row(
                "SELECT id, full_text FROM precedent_versions
                  WHERE precedent_id=?1 AND full_text IS NOT NULL AND length(trim(full_text))>0
                  ORDER BY acquired_at DESC, id DESC LIMIT 1",
                [precedent_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((version_id, text)) = latest else {
            result.reason = Some("no_full_text".into());
            return Ok(result);
        };
        result.version_id = Some(version_id);
        let length = i64::try_from(quote_chars).unwrap_or(i64::MAX);
        if let Some(byte) = text.find(quote) {
            result.state = "match".into();
            result.method = Some("exact".into());
            result.matched_start = Some(char_offset(&text, byte));
            result.matched_length = Some(length);
            result.occurrences = count_occurrences(&text, quote);
            return Ok(result);
        }
        let haystack = collapse_ws_with_map(&text);
        let needle = collapse_ws_with_map(quote);
        if !needle.text.is_empty() {
            if let Some(at) = haystack.text.find(&needle.text) {
                result.state = "match".into();
                result.method = Some("whitespace".into());
                result.matched_start = mapped_start(&haystack, at);
                result.matched_length = Some(length);
                result.occurrences = count_occurrences(&haystack.text, &needle.text);
                return Ok(result);
            }
        }
        let haystack = normalize_with_map(&text);
        let needle = normalize_with_map(quote);
        if !needle.text.is_empty() {
            if let Some(at) = haystack.text.find(&needle.text) {
                result.state = "match".into();
                result.method = Some("turkish_fold".into());
                result.matched_start = mapped_start(&haystack, at);
                result.matched_length = Some(length);
                result.occurrences = count_occurrences(&haystack.text, &needle.text);
                return Ok(result);
            }
        }
        result.state = "mismatch".into();
        result.reason = Some("not_found".into());
        Ok(result)
    }

    /// Bibliyografik künye (F12-T12): künye alanları, kaynak bağlantısı ve
    /// erişim tarihi. Eksik alan **tahmin edilmez**; `missing` listesinde
    /// görünür. Erişim tarihi varsa en yeni tam metin sürümünün edinim
    /// zamanından, yoksa künye kaydının oluşturulma zamanından alınır ve
    /// hangisinin kullanıldığı `access_basis` ile söylenir.
    pub fn precedent_bibliography(
        &self,
        precedent_id: &str,
    ) -> Result<PrecedentBibliography, StorageError> {
        let precedent = self
            .get_precedent(precedent_id)?
            .ok_or(StorageError::NotFound)?;
        let mut missing: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        let mut head = precedent.court.trim().to_string();
        if let Some(chamber) = non_empty(precedent.chamber.as_deref()) {
            head = format!("{head} {chamber}");
        }
        let mut parts = vec![head];

        match non_empty(precedent.decision_date.as_deref()) {
            Some(raw) => match iso_to_tr_date(raw) {
                Some(short) => parts.push(short),
                None => {
                    parts.push(raw.to_string());
                    warnings.push(format!(
                        "Tarih ISO (YYYY-AA-GG) biçiminde değil: “{raw}” olduğu gibi yazıldı."
                    ));
                }
            },
            None => missing.push("karar tarihi".into()),
        }
        match non_empty(precedent.docket_no.as_deref()) {
            Some(docket) => parts.push(format!("E. {docket}")),
            None => missing.push("esas numarası".into()),
        }
        match non_empty(precedent.decision_no.as_deref()) {
            Some(decision) => parts.push(format!("K. {decision}")),
            None => missing.push("karar numarası".into()),
        }
        if let Some(application) = non_empty(precedent.application_no.as_deref()) {
            parts.push(format!("B. No: {application}"));
        }
        match non_empty(precedent.title.as_deref()) {
            Some(title) => parts.push(format!("“{title}”")),
            None => missing.push("başlık".into()),
        }

        let (accessed_raw, access_basis) = match self
            .conn()
            .query_row(
                "SELECT acquired_at FROM precedent_versions
                  WHERE precedent_id=?1 AND full_text IS NOT NULL AND length(trim(full_text))>0
                  ORDER BY acquired_at DESC, id DESC LIMIT 1",
                [precedent_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            Some(stamp) => (stamp, "version"),
            None => (precedent.created_at.clone(), "record"),
        };
        let accessed_at = iso_to_tr_date(&accessed_raw)
            .unwrap_or_else(|| accessed_raw.chars().take(10).collect::<String>());
        if access_basis == "record" {
            warnings.push(
                "Tam metin edinimi yok; erişim tarihi olarak künye kaydının oluşturulma zamanı kullanıldı."
                    .into(),
            );
        }

        let source_url = non_empty(precedent.source_url.as_deref()).map(str::to_string);
        let mut text = parts.join(", ");
        match &source_url {
            Some(url) => text.push_str(&format!(". Kaynak: {url} (erişim: {accessed_at})")),
            None => missing.push("kaynak bağlantısı".into()),
        }
        if !missing.is_empty() {
            warnings.insert(
                0,
                format!(
                    "Künye eksik: {}. Eksik alan tahmin edilmedi.",
                    missing.join(", ")
                ),
            );
        }
        let with_text: i64 = self.conn().query_row(
            "SELECT count(*) FROM precedent_versions
              WHERE precedent_id=?1 AND full_text IS NOT NULL AND length(trim(full_text))>0",
            [precedent_id],
            |row| row.get(0),
        )?;
        Ok(PrecedentBibliography {
            precedent_id: precedent.id,
            text,
            missing,
            warnings,
            source_url,
            accessed_at,
            access_basis: access_basis.into(),
            has_full_text: with_text > 0,
        })
    }

    /// Emsal kütüphanesi araması (F12-T10).
    ///
    /// **Kapsam ayrımı:** yalnız emsal kayıtlarının en yeni tam metni ve künye
    /// numaraları taranır. Aktif dosyanın belge segmentleri (`text_segments`)
    /// bu sorguya hiç girmez; dönen `scope` her zaman `precedent_library` olur.
    /// Metin gövdesi dışarı çıkmaz — yalnız eşleşme çevresinden pencere.
    pub fn search_precedents(
        &self,
        query: &PrecedentSearchQuery,
    ) -> Result<PrecedentSearchPage, StorageError> {
        let resolved = SearchQuery {
            text: query.text.clone(),
            mode: query.mode,
        }
        .resolve()?;
        if resolved.text.trim().is_empty() {
            return Err(StorageError::SearchQuery("arama metni boş".into()));
        }
        let limit = query
            .limit
            .unwrap_or(DEFAULT_PRECEDENT_SEARCH_LIMIT)
            .clamp(1, MAX_PRECEDENT_SEARCH_LIMIT);
        let matcher = SearchMatcher::new(&resolved.text, resolved.mode, resolved.docket.clone());
        // Künye alanı rolü adıyla belli eder (esas/karar/başvuru); metadata
        // karşılaştırmasında E./K. rolü aranmaz — yalnız yıl + numara.
        let metadata_docket = resolved.docket.as_ref().map(|docket| DocketNumber {
            year: docket.year.clone(),
            number: docket.number.clone(),
            role: None,
        });
        // Tam metni olmayan kayıt **sessizce atlanmaz**; sayısı ayrıca bildirilir.
        let without_text: i64 = self.conn().query_row(
            "SELECT count(*) FROM precedents p
              WHERE NOT EXISTS (
                    SELECT 1 FROM precedent_versions v
                     WHERE v.precedent_id=p.id
                       AND v.full_text IS NOT NULL AND length(trim(v.full_text))>0)",
            [],
            |row| row.get(0),
        )?;
        let mut stmt = self.conn().prepare(
            "SELECT p.id,p.court,p.chamber,p.docket_no,p.decision_no,p.application_no,
                    p.decision_date,p.title,v.id,v.acquired_at,v.source_kind,v.full_text
               FROM precedents p
               JOIN precedent_versions v ON v.id = (
                    SELECT id FROM precedent_versions
                     WHERE precedent_id=p.id
                       AND full_text IS NOT NULL AND length(trim(full_text))>0
                     ORDER BY acquired_at DESC, id DESC LIMIT 1)
              ORDER BY p.court, p.chamber, p.decision_date DESC, p.id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                PrecedentSearchSeed {
                    precedent_id: row.get(0)?,
                    court: row.get(1)?,
                    chamber: row.get(2)?,
                    docket_no: row.get(3)?,
                    decision_no: row.get(4)?,
                    application_no: row.get(5)?,
                    decision_date: row.get(6)?,
                    title: row.get(7)?,
                    version_id: row.get(8)?,
                    acquired_at: row.get(9)?,
                    source_kind: row.get(10)?,
                },
                row.get::<_, String>(11)?,
            ))
        })?;
        let mut hits: Vec<PrecedentSearchHit> = Vec::new();
        let mut scanned = 0i64;
        let mut matched = 0i64;
        let mut scratch = MatchScratch::default();
        for row in rows {
            let (seed, full_text) = row?;
            scanned += 1;
            let normalized = dim_evidence::normalize(&full_text);
            let text_match = matcher.matches(Some(&full_text), Some(&normalized), &mut scratch);
            let metadata_match = resolved.mode == SearchMode::Docket
                && metadata_docket.as_ref().is_some_and(|docket| {
                    [
                        seed.docket_no.as_deref(),
                        seed.decision_no.as_deref(),
                        seed.application_no.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    .any(|value| docket_match(value, docket))
                });
            if !(text_match || metadata_match) {
                continue;
            }
            matched += 1;
            if hits.len() >= usize::try_from(limit).unwrap_or(usize::MAX) {
                continue;
            }
            let position = if text_match {
                locate_in_text(&full_text, &resolved.text)
            } else {
                None
            };
            let (snippet, snippet_start) =
                snippet_window(&full_text, position.map(|(start, _)| start));
            hits.push(PrecedentSearchHit {
                precedent_id: seed.precedent_id,
                court: seed.court,
                chamber: seed.chamber,
                docket_no: seed.docket_no,
                decision_no: seed.decision_no,
                decision_date: seed.decision_date,
                title: seed.title,
                version_id: seed.version_id,
                acquired_at: seed.acquired_at,
                source_kind: seed.source_kind,
                snippet,
                snippet_start,
                match_start: position.map(|(start, _)| start),
                match_length: position.map(|(_, length)| length),
                match_source: if text_match { "full_text" } else { "metadata" }.into(),
            });
        }
        let truncated = matched > i64::from(limit);
        let mut notes = vec![
            "Kapsam yalnız emsal kütüphanesidir; aktif dosyanın belgeleri bu aramada taranmaz."
                .to_string(),
        ];
        if without_text > 0 {
            notes.push(format!(
                "{without_text} emsalin tam metni yok; bu kayıtlar metin aramasında taranmadı. \
                 Esas/karar numarasıyla arayın."
            ));
        }
        if truncated {
            notes.push(format!(
                "{matched} eşleşmeden ilk {limit} tanesi gösteriliyor."
            ));
        }
        if matched == 0 {
            notes.push(
                "Emsal kütüphanesinde eşleşme yok. Bu bir “karar yok” hükmü değildir; \
                 dosya belgelerinde aramak için kapsamı değiştirin."
                    .into(),
            );
        }
        Ok(PrecedentSearchPage {
            scope: PRECEDENT_SEARCH_SCOPE.to_string(),
            mode: resolved.mode.tag().to_string(),
            query: query.text.trim().to_string(),
            resolved_query: resolved.text.clone(),
            hits,
            scanned,
            matched,
            without_text,
            limit,
            truncated,
            notes,
        })
    }
}

/// Kütüphane aramasının kapsam etiketi; arayüz ve testler bunu kullanır.
pub const PRECEDENT_SEARCH_SCOPE: &str = "precedent_library";
pub const MAX_PRECEDENT_SEARCH_LIMIT: u32 = 50;
const DEFAULT_PRECEDENT_SEARCH_LIMIT: u32 = 25;
/// Eşleşme penceresi: 240 karakter, eşleşmeden önce en çok 60 karakter bağlam.
const SNIPPET_WINDOW: usize = 240;
const SNIPPET_LEAD: usize = 60;

struct PrecedentSearchSeed {
    precedent_id: String,
    court: String,
    chamber: Option<String>,
    docket_no: Option<String>,
    decision_no: Option<String>,
    application_no: Option<String>,
    decision_date: Option<String>,
    title: Option<String>,
    version_id: String,
    acquired_at: String,
    source_kind: String,
}

/// Eşleşmenin metindeki yerini bulur: birebir → boşluk sadeleştirmesi →
/// Türkçe katlama. Bulunamazsa `None`; **konum uydurulmaz**.
fn locate_in_text(text: &str, needle: &str) -> Option<(i64, i64)> {
    let needle = needle.trim();
    if needle.is_empty() {
        return None;
    }
    let length = i64::try_from(needle.chars().count()).unwrap_or(i64::MAX);
    if let Some(byte) = text.find(needle) {
        return Some((char_offset(text, byte), length));
    }
    let haystack = collapse_ws_with_map(text);
    let wanted = collapse_ws_with_map(needle);
    if !wanted.text.is_empty() {
        if let Some(at) = haystack.text.find(&wanted.text) {
            if let Some(start) = mapped_start(&haystack, at) {
                return Some((
                    start,
                    i64::try_from(wanted.text.chars().count()).unwrap_or(length),
                ));
            }
        }
    }
    let haystack = normalize_with_map(text);
    let wanted = normalize_with_map(needle);
    if !wanted.text.is_empty() {
        if let Some(at) = haystack.text.find(&wanted.text) {
            if let Some(start) = mapped_start(&haystack, at) {
                return Some((
                    start,
                    i64::try_from(wanted.text.chars().count()).unwrap_or(length),
                ));
            }
        }
    }
    None
}

/// Eşleşme çevresinden pencere. Metnin tamamı arayüze gönderilmez.
fn snippet_window(text: &str, start: Option<i64>) -> (String, i64) {
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    let position = start
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value < total);
    let from = position.map_or(0, |pos| pos.saturating_sub(SNIPPET_LEAD));
    let to = (from + SNIPPET_WINDOW).min(total);
    let mut out = String::new();
    if from > 0 {
        out.push('…');
    }
    out.extend(chars[from..to].iter());
    if to < total {
        out.push('…');
    }
    (out, i64::try_from(from).unwrap_or(0))
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

/// `YYYY-AA-GG` → `GG.AA.YYYY`. Biçim tutmuyorsa `None` — tarih uydurulmaz.
fn iso_to_tr_date(raw: &str) -> Option<String> {
    let date = raw.trim().get(0..10)?;
    let bytes = date.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year: u32 = date.get(0..4)?.parse().ok()?;
    let month: u32 = date.get(5..7)?.parse().ok()?;
    let day: u32 = date.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(format!("{day:02}.{month:02}.{year:04}"))
}

fn map_precedent(row: &Row<'_>) -> rusqlite::Result<PrecedentRow> {
    let tags: String = row.get(9)?;
    Ok(PrecedentRow {
        id: row.get(0)?,
        court: row.get(1)?,
        chamber: row.get(2)?,
        docket_no: row.get(3)?,
        decision_no: row.get(4)?,
        application_no: row.get(5)?,
        decision_date: row.get(6)?,
        title: row.get(7)?,
        source_url: row.get(8)?,
        user_tags: serde_json::from_str(&tags).unwrap_or_default(),
        legal_issue: row.get(10)?,
        completeness: row.get(11)?,
        created_at: row.get(12)?,
        usage_basis: row.get(13)?,
        redistribution: row.get(14)?,
        terms_recorded_at: row.get(15)?,
    })
}

fn map_check(row: &Row<'_>) -> rusqlite::Result<CitationCheckRow> {
    let reasons: String = row.get(9)?;
    Ok(CitationCheckRow {
        id: row.get(0)?,
        input_document_version_id: row.get(1)?,
        extracted_citation: row.get(2)?,
        candidate_precedent_id: row.get(3)?,
        identity_match: row.get(4)?,
        quote_match: row.get(5)?,
        meaning_review: row.get(6)?,
        meaning_note: row.get(7)?,
        meaning_reviewed_at: row.get(8)?,
        reason_codes: serde_json::from_str(&reasons).unwrap_or_default(),
        created_at: row.get(10)?,
    })
}

fn map_version(row: &Row<'_>) -> rusqlite::Result<PrecedentVersionRow> {
    Ok(PrecedentVersionRow {
        id: row.get(0)?,
        precedent_id: row.get(1)?,
        document_version_id: row.get(2)?,
        acquired_at: row.get(3)?,
        source_kind: row.get(4)?,
        body_hash: row.get(5)?,
        verification_state: row.get(6)?,
        char_count: row.get(7)?,
        has_text: row.get::<_, i64>(8)? != 0,
        identity_conflict: row.get::<_, i64>(9)? != 0,
        conflict_with_version_id: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::{DatabaseKey, SearchFilters};

    fn db() -> Database {
        Database::open_in_memory(&DatabaseKey::new([37u8; 32])).expect("db")
    }

    fn precedent(court: &str, docket: Option<&str>, decision: Option<&str>) -> NewPrecedent {
        NewPrecedent {
            court: court.into(),
            chamber: Some("1. Hukuk Dairesi".into()),
            docket_no: docket.map(str::to_string),
            decision_no: decision.map(str::to_string),
            application_no: None,
            decision_date: Some("2025-06-01".into()),
            title: Some("Kira alacağı".into()),
            source_url: Some("https://example.invalid/karar".into()),
            user_tags: vec!["kira".into(), "temerrut".into()],
            legal_issue: Some("Temerrüt".into()),
        }
    }

    #[test]
    fn completeness_depends_on_docket_and_decision() {
        let db = db();
        let complete = db
            .create_precedent(&precedent("Yargıtay", Some("2024/123"), Some("2025/456")))
            .expect("emsal");
        assert_eq!(complete.completeness, "complete");
        assert_eq!(complete.user_tags.len(), 2);

        let incomplete = db
            .create_precedent(&precedent("Yargıtay", Some("2024/123"), None))
            .expect("emsal");
        assert_eq!(incomplete.completeness, "incomplete");
    }

    #[test]
    fn court_is_required() {
        let db = db();
        assert!(db.create_precedent(&precedent("   ", None, None)).is_err());
    }

    #[test]
    fn case_specific_note_requires_existing_case() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/1"), Some("2025/2")))
            .expect("emsal");
        let note = db
            .create_precedent_note(
                &p.id,
                &NewPrecedentNote {
                    case_id_optional: None,
                    useful_passage_anchor: None,
                    similarity_note: "benzer".into(),
                    distinguishing_note: "farklı".into(),
                    user_conclusion: "kullanılabilir".into(),
                },
            )
            .expect("not");
        assert!(note.case_id_optional.is_none());

        assert!(db
            .create_precedent_note(
                &p.id,
                &NewPrecedentNote {
                    case_id_optional: Some("olmayan-dosya".into()),
                    useful_passage_anchor: None,
                    similarity_note: String::new(),
                    distinguishing_note: String::new(),
                    user_conclusion: String::new(),
                },
            )
            .is_err());
    }

    #[test]
    fn citation_check_keeps_dimensions_separate() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/1"), Some("2025/2")))
            .expect("emsal");
        let check = db
            .record_citation_check(&NewCitationCheck {
                input_document_version_id: "version-1".into(),
                extracted_citation: "Yargıtay 1. HD 2024/1".into(),
                candidate_precedent_id: Some(p.id.clone()),
                identity_match: "unknown".into(),
                quote_match: "not_checked".into(),
                meaning_review: "not_reviewed".into(),
                reason_codes: vec!["text_missing".into()],
            })
            .expect("kontrol");
        assert_eq!(check.identity_match, "unknown");
        assert_eq!(check.quote_match, "not_checked");
        assert_ne!(check.identity_match, "mismatch");
        assert_eq!(
            db.list_citation_checks("version-1").expect("liste").len(),
            1
        );
    }

    #[test]
    fn meaning_review_requires_note_for_negative_verdicts() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/9"), Some("2025/9")))
            .expect("emsal");
        let check = db
            .record_citation_check(&NewCitationCheck {
                input_document_version_id: "v-meaning".into(),
                extracted_citation: "Yargıtay 1. HD 2024/9".into(),
                candidate_precedent_id: Some(p.id.clone()),
                identity_match: "unknown".into(),
                quote_match: "not_checked".into(),
                meaning_review: "not_reviewed".into(),
                reason_codes: vec![],
            })
            .expect("kontrol");

        assert_eq!(
            db.review_citation_meaning(&check.id, "misleading", "   ")
                .unwrap_err()
                .code(),
            "E_DB_SETTING"
        );
        assert_eq!(
            db.review_citation_meaning(&check.id, "sahte", "not")
                .unwrap_err()
                .code(),
            "E_DB_SETTING"
        );

        let accurate = db
            .review_citation_meaning(&check.id, "accurate", "")
            .expect("doğru");
        assert_eq!(accurate.meaning_review, "accurate");
        assert!(accurate.meaning_reviewed_at.is_some());

        let misleading = db
            .review_citation_meaning(&check.id, "misleading", "Kararın sonucu tersine çevrilmiş.")
            .expect("yanıltıcı");
        assert_eq!(misleading.meaning_note, "Kararın sonucu tersine çevrilmiş.");
    }

    #[test]
    fn citation_context_returns_quote_surroundings() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/7"), Some("2025/8")))
            .expect("emsal");
        let quote = "kiracı temerrüde düşmedikçe tahliye istenemez";
        let body = format!("Öncesi metin. {quote} Sonrası metin.");
        db.add_precedent_text(
            &p.id,
            &NewPrecedentText {
                source_kind: "paste".into(),
                text: body,
                document_version_id: None,
            },
        )
        .expect("metin");
        let check = db
            .record_citation_check(&NewCitationCheck {
                input_document_version_id: "v-context".into(),
                extracted_citation: quote.into(),
                candidate_precedent_id: Some(p.id.clone()),
                identity_match: "unknown".into(),
                quote_match: "not_checked".into(),
                meaning_review: "not_reviewed".into(),
                reason_codes: vec![],
            })
            .expect("kontrol");

        let context = db.citation_context(&check.id, 12).expect("bağlam");
        assert!(context.found, "{context:?}");
        assert_eq!(context.quote, quote);
        assert!(context.before.ends_with("metin. "), "{context:?}");
        assert!(context.after.starts_with(" Sonrası"), "{context:?}");
        assert_eq!(context.state, "match");
    }

    #[test]
    fn citation_context_is_honest_without_full_text_or_candidate() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/3"), None))
            .expect("emsal");
        let no_text = db
            .record_citation_check(&NewCitationCheck {
                input_document_version_id: "v".into(),
                extracted_citation: "uzun bir alıntı metni burada".into(),
                candidate_precedent_id: Some(p.id),
                identity_match: "unknown".into(),
                quote_match: "not_checked".into(),
                meaning_review: "not_reviewed".into(),
                reason_codes: vec![],
            })
            .expect("kontrol");
        let context = db.citation_context(&no_text.id, 10).expect("bağlam");
        assert!(!context.found);
        assert_eq!(context.state, "not_checked");

        let no_candidate = db
            .record_citation_check(&NewCitationCheck {
                input_document_version_id: "v2".into(),
                extracted_citation: "başka alıntı".into(),
                candidate_precedent_id: None,
                identity_match: "unknown".into(),
                quote_match: "not_checked".into(),
                meaning_review: "not_reviewed".into(),
                reason_codes: vec![],
            })
            .expect("kontrol");
        let context = db.citation_context(&no_candidate.id, 10).expect("bağlam");
        assert!(!context.found);
        assert_eq!(context.reason.as_deref(), Some("no_candidate"));
    }

    #[test]
    fn invalid_match_values_rejected() {
        let db = db();
        let mut check = NewCitationCheck {
            input_document_version_id: "v".into(),
            extracted_citation: "x".into(),
            candidate_precedent_id: None,
            identity_match: "sahte".into(),
            quote_match: "unknown".into(),
            meaning_review: "not_reviewed".into(),
            reason_codes: vec![],
        };
        assert!(db.record_citation_check(&check).is_err());
        check.identity_match = "unknown".into();
        check.quote_match = "belki".into();
        assert!(db.record_citation_check(&check).is_err());
    }

    #[test]
    fn unknown_case_still_works_without_case_scope() {
        let db = db();
        let case = db
            .create_case(&NewCase {
                title: "Dosya".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/9"), Some("2025/9")))
            .expect("emsal");
        let note = db
            .create_precedent_note(
                &p.id,
                &NewPrecedentNote {
                    case_id_optional: Some(case.id.clone()),
                    useful_passage_anchor: None,
                    similarity_note: "aynı konu".into(),
                    distinguishing_note: String::new(),
                    user_conclusion: String::new(),
                },
            )
            .expect("not");
        assert_eq!(note.case_id_optional.as_deref(), Some(case.id.as_str()));
    }

    // ---- F12-T05: tam metin ve metin içi alıntı doğrulama ----

    const KARAR: &str = "Yargıtay 1. Hukuk Dairesi\n\nMahkeme, itirazın reddine karar verdi.\nGerekçede ödeme yapılmadığı belirtildi.";

    fn text_of(text: &str) -> NewPrecedentText {
        NewPrecedentText {
            source_kind: "paste".into(),
            text: text.into(),
            document_version_id: None,
        }
    }

    #[test]
    fn text_acquisition_records_hash_and_char_count() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/1"), Some("2025/2")))
            .expect("emsal");
        let version = db
            .add_precedent_text(&p.id, &text_of(KARAR))
            .expect("metin");
        assert_eq!(version.verification_state, "text_verified");
        assert!(version.has_text);
        assert_eq!(
            version.char_count,
            i64::try_from(KARAR.chars().count()).unwrap()
        );
        assert_eq!(version.body_hash.len(), 64);
        assert_eq!(version.source_kind, "paste");

        let listed = db.precedent_versions(&p.id).expect("sürümler");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, version.id);

        // Boş metin ve bilinmeyen kaynak türü reddedilir.
        assert!(db.add_precedent_text(&p.id, &text_of("   ")).is_err());
        let mut bad = text_of("kısa metin");
        bad.source_kind = "clipboard".into();
        assert!(db.add_precedent_text(&p.id, &bad).is_err());
    }

    #[test]
    fn quote_without_full_text_is_not_checked() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/2"), Some("2025/3")))
            .expect("emsal");
        let result = db
            .verify_precedent_quote(&p.id, "itirazın reddine karar verdi")
            .expect("doğrulama");
        assert_eq!(result.state, "not_checked");
        assert_eq!(result.reason.as_deref(), Some("no_full_text"));
        assert!(result.version_id.is_none());
        // "Doğrulanamadı" hiçbir zaman "uydu" hükmü üretmez.
        assert_ne!(result.state, "mismatch");
    }

    #[test]
    fn exact_quote_reports_position_in_original_text() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/3"), Some("2025/4")))
            .expect("emsal");
        db.add_precedent_text(&p.id, &text_of(KARAR))
            .expect("metin");
        let result = db
            .verify_precedent_quote(&p.id, "itirazın reddine karar verdi")
            .expect("doğrulama");
        assert_eq!(result.state, "match");
        assert_eq!(result.method.as_deref(), Some("exact"));
        let expected = KARAR.find("itirazın reddine karar verdi").unwrap();
        assert_eq!(result.matched_start, Some(char_offset(KARAR, expected)));
        assert_eq!(result.occurrences, 1);
    }

    #[test]
    fn line_break_difference_matches_loosely_with_method() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/4"), Some("2025/5")))
            .expect("emsal");
        db.add_precedent_text(&p.id, &text_of(KARAR))
            .expect("metin");
        // Alıntı kısa: yanlış kesinlik üretmemek için doğrulanmaz.
        let result = db
            .verify_precedent_quote(&p.id, "reddine")
            .expect("doğrulama");
        assert_eq!(result.state, "not_checked");
        assert_eq!(result.reason.as_deref(), Some("quote_too_short"));

        // Birebir geçen alıntı.
        let result = db
            .verify_precedent_quote(&p.id, "Gerekçede ödeme yapılmadığı belirtildi")
            .expect("doğrulama");
        assert_eq!(result.state, "match");
        assert_eq!(result.method.as_deref(), Some("exact"));

        let spaced = "itirazın reddine karar\nverdi. Gerekçede";
        let result = db.verify_precedent_quote(&p.id, spaced).expect("doğrulama");
        assert_eq!(result.state, "match");
        assert_eq!(result.method.as_deref(), Some("whitespace"));
        let expected = KARAR.find("itirazın reddine karar").unwrap();
        assert_eq!(result.matched_start, Some(char_offset(KARAR, expected)));
    }

    #[test]
    fn altered_quote_is_mismatch_not_fake_decision() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/5"), Some("2025/6")))
            .expect("emsal");
        db.add_precedent_text(&p.id, &text_of(KARAR))
            .expect("metin");
        let result = db
            .verify_precedent_quote(&p.id, "itirazın kabulüne karar verdi")
            .expect("doğrulama");
        assert_eq!(result.state, "mismatch");
        assert_eq!(result.reason.as_deref(), Some("not_found"));
        assert!(result.matched_start.is_none());
        // Sonuç yalnız "bulunamadı"dır; "uydurulmuş" damgası yok.
        assert!(!result.state.contains("fake"));
    }

    #[test]
    fn turkish_diacritics_fold_is_reported_as_loose_match() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/6"), Some("2025/7")))
            .expect("emsal");
        db.add_precedent_text(&p.id, &text_of(KARAR))
            .expect("metin");
        let result = db
            .verify_precedent_quote(&p.id, "itirazin reddine karar verdi")
            .expect("doğrulama");
        assert_eq!(result.state, "match");
        assert_eq!(result.method.as_deref(), Some("turkish_fold"));
        assert!(result.matched_start.is_some());
    }

    #[test]
    fn newest_text_version_is_used() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/7"), Some("2025/8")))
            .expect("emsal");
        db.add_precedent_text(
            &p.id,
            &text_of("İlk sürümde bu cümle vardı: itiraz reddedildi."),
        )
        .expect("v1");
        let newest = db
            .add_precedent_text(
                &p.id,
                &text_of("Yeni sürüm: itirazın reddine karar verildi."),
            )
            .expect("v2");
        let result = db
            .verify_precedent_quote(&p.id, "itirazın reddine karar verildi")
            .expect("doğrulama");
        assert_eq!(result.state, "match");
        assert_eq!(result.version_id.as_deref(), Some(newest.id.as_str()));
        // Eski sürümde olan ama yenisinde olmayan alıntı artık bulunamaz.
        let old = db
            .verify_precedent_quote(&p.id, "bu cümle vardı")
            .expect("doğrulama");
        assert_eq!(old.state, "mismatch");
    }

    #[test]
    fn fold_matches_evidence_normalize() {
        for sample in [
            "İtirazın REDDİNE karar",
            "Şişli'deki ğ harfi",
            "  ÖDEME   yapılmadı \n",
            "Âdet üzere îkâz",
        ] {
            assert_eq!(
                normalize_with_map(sample).text,
                dim_evidence::normalize(sample),
                "sadeleştirme ayrıştı: {sample}"
            );
        }
        let mapped = normalize_with_map("AB cd");
        assert_eq!(mapped.map.len(), mapped.text.chars().count());
        assert_eq!(mapped.text, "ab cd");
    }

    #[test]
    fn bibliography_formats_complete_record_with_version_access_date() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/123"), Some("2025/456")))
            .expect("emsal");
        db.add_precedent_text(&p.id, &text_of("Karar metni burada."))
            .expect("metin");

        let bib = db.precedent_bibliography(&p.id).expect("künye");
        assert!(bib.missing.is_empty(), "eksik: {:?}", bib.missing);
        assert!(bib.warnings.is_empty(), "uyarı: {:?}", bib.warnings);
        assert!(
            bib.text.starts_with(
                "Yargıtay 1. Hukuk Dairesi, 01.06.2025, E. 2024/123, K. 2025/456, “Kira alacağı”. \
             Kaynak: https://example.invalid/karar (erişim: "
            ),
            "metin: {}",
            bib.text
        );
        assert_eq!(bib.access_basis, "version");
        assert!(bib.has_full_text);
        assert_eq!(
            bib.source_url.as_deref(),
            Some("https://example.invalid/karar")
        );
        // Erişim tarihi tam metnin edinim zamanından gelir (bugün, GG.AA.YYYY).
        assert_eq!(bib.accessed_at.len(), 10);
        assert_eq!(&bib.accessed_at[2..3], ".");
        assert_eq!(&bib.accessed_at[5..6], ".");
    }

    #[test]
    fn bibliography_lists_missing_fields_without_inventing() {
        let db = db();
        let bare = NewPrecedent {
            court: "Danıştay".into(),
            chamber: None,
            docket_no: None,
            decision_no: None,
            application_no: None,
            decision_date: None,
            title: None,
            source_url: None,
            user_tags: vec![],
            legal_issue: None,
        };
        let p = db.create_precedent(&bare).expect("emsal");

        let bib = db.precedent_bibliography(&p.id).expect("künye");
        assert_eq!(bib.text, "Danıştay");
        assert_eq!(
            bib.missing,
            vec![
                "karar tarihi",
                "esas numarası",
                "karar numarası",
                "başlık",
                "kaynak bağlantısı",
            ]
        );
        assert_eq!(bib.access_basis, "record");
        assert!(!bib.has_full_text);
        assert!(bib.warnings[0].starts_with("Künye eksik: "));
        assert!(bib
            .warnings
            .iter()
            .any(|w| w.contains("Tam metin edinimi yok")));
        // Tarih/numara uydurulmadı.
        assert!(!bib.text.contains("E. "));
        assert!(!bib.text.contains("Kaynak:"));
    }

    #[test]
    fn bibliography_keeps_non_iso_date_verbatim_and_warns() {
        let db = db();
        let mut input = precedent("Yargıtay", Some("2024/9"), Some("2025/9"));
        input.decision_date = Some("01.06.2025".into());
        let p = db.create_precedent(&input).expect("emsal");

        let bib = db.precedent_bibliography(&p.id).expect("künye");
        assert!(bib
            .text
            .starts_with("Yargıtay 1. Hukuk Dairesi, 01.06.2025, E. 2024/9"));
        assert!(bib.warnings.iter().any(|w| w.contains("ISO (YYYY-AA-GG)")));
    }

    #[test]
    fn bibliography_unknown_precedent_is_not_found() {
        let db = db();
        assert_eq!(
            db.precedent_bibliography("yok-boyle-emsal")
                .unwrap_err()
                .code(),
            "E_NOT_FOUND"
        );
    }

    #[test]
    fn iso_to_tr_date_rejects_malformed_values() {
        assert_eq!(iso_to_tr_date("2025-06-01").as_deref(), Some("01.06.2025"));
        assert_eq!(
            iso_to_tr_date("2025-06-01T10:00:00Z").as_deref(),
            Some("01.06.2025")
        );
        assert!(iso_to_tr_date("2025-13-01").is_none());
        assert!(iso_to_tr_date("2025-06-32").is_none());
        assert!(iso_to_tr_date("01.06.2025").is_none());
        assert!(iso_to_tr_date("2025-6-1").is_none());
        assert!(iso_to_tr_date("").is_none());
    }

    fn library_query(text: &str, mode: SearchMode, limit: Option<u32>) -> PrecedentSearchQuery {
        PrecedentSearchQuery {
            text: text.into(),
            mode,
            limit,
        }
    }

    fn case_for_search(db: &Database) -> String {
        db.create_case(&NewCase {
            title: "Kira".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: crate::Sensitivity::Normal,
            default_ai_policy: crate::AiPolicy::Off,
        })
        .expect("dosya")
        .id
    }

    /// F12-T10 kabul testi: kütüphane araması **yalnız** emsalleri tarar;
    /// aynı belirteç dosya belgelerinde bulunmaz ve emsal metni segment
    /// tablosuna hiç yazılmaz.
    #[test]
    fn library_search_is_scoped_and_never_reads_case_documents() {
        let db = db();
        let kira = db
            .create_precedent(&precedent("Yargıtay", Some("2024/123"), Some("2025/456")))
            .expect("emsal");
        db.add_precedent_text(
            &kira.id,
            &text_of("Gerekçede ÖZEL-BELİRTEÇ-9137 geçer ve kira bedeli tartışılır."),
        )
        .expect("metin");
        let idari = db
            .create_precedent(&precedent("Danıştay", Some("2024/9"), Some("2025/9")))
            .expect("emsal");
        db.add_precedent_text(&idari.id, &text_of("İdari işlemin iptaline karar verildi."))
            .expect("metin");

        let page = db
            .search_precedents(&library_query("ÖZEL-BELİRTEÇ-9137", SearchMode::Auto, None))
            .expect("kütüphane araması");
        assert_eq!(page.scope, PRECEDENT_SEARCH_SCOPE);
        assert_eq!(page.scope, "precedent_library");
        assert_eq!(page.scanned, 2);
        assert_eq!(page.matched, 1);
        assert_eq!(page.hits.len(), 1);
        assert_eq!(page.hits[0].precedent_id, kira.id);
        assert_eq!(page.hits[0].match_source, "full_text");
        assert!(page.hits[0].snippet.contains("ÖZEL-BELİRTEÇ-9137"));
        assert!(page.hits[0].match_start.is_some());
        // Snippet tam metni taşımaz: kararın tamamı değil, pencere döner.
        assert!(page.hits[0].snippet.chars().count() <= 242);

        let case_id = case_for_search(&db);
        let documents = db
            .search_page(
                &case_id,
                &SearchQuery::auto("ÖZEL-BELİRTEÇ-9137"),
                &SearchFilters::default(),
                20,
                None,
                true,
            )
            .expect("dosya araması");
        assert_eq!(documents.scope, "case");
        assert_eq!(documents.hits.len(), 0);
        let leaked: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM text_segments WHERE raw_text LIKE '%ÖZEL-BELİRTEÇ-9137%'",
                [],
                |row| row.get(0),
            )
            .expect("sayım");
        assert_eq!(leaked, 0, "emsal metni dosya aramasına sızmamalı");
    }

    /// Tam metni olmayan kayıt sessizce atlanmaz; sayısı ve nedeni bildirilir.
    #[test]
    fn library_search_reports_records_without_full_text() {
        let db = db();
        let with_text = db
            .create_precedent(&precedent("Yargıtay", Some("2024/1"), Some("2025/2")))
            .expect("emsal");
        db.add_precedent_text(&with_text.id, &text_of("Kira bedeli ödenmedi."))
            .expect("metin");
        db.create_precedent(&precedent("Yargıtay", Some("2024/55"), Some("2025/55")))
            .expect("metinsiz emsal");

        let page = db
            .search_precedents(&library_query("2024/55", SearchMode::Docket, None))
            .expect("arama");
        assert_eq!(page.mode, "docket");
        assert_eq!(page.scanned, 1, "yalnız tam metni olan kayıt taranır");
        assert_eq!(page.without_text, 1);
        assert_eq!(page.matched, 0);
        assert!(page.notes.iter().any(|n| n.contains("tam metni yok")));
        assert!(page
            .notes
            .iter()
            .any(|n| n.contains("yönünde") || n.contains("hükmü değildir")));
    }

    /// Türkçe katlama ile eşleşme + eşleşme çevresinden pencere.
    #[test]
    fn library_search_folds_turkish_and_keeps_context_window() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/3"), Some("2025/4")))
            .expect("emsal");
        let mut body = String::new();
        for _ in 0..40 {
            body.push_str("dolgu metni ");
        }
        body.push_str("İtirazın reddine karar verildi.");
        for _ in 0..40 {
            body.push_str(" sonrası");
        }
        db.add_precedent_text(&p.id, &text_of(&body))
            .expect("metin");

        let folded = db
            .search_precedents(&library_query(
                "itirazin reddine",
                SearchMode::Flexible,
                None,
            ))
            .expect("katlamalı arama");
        assert_eq!(folded.hits.len(), 1);
        let hit = &folded.hits[0];
        assert!(hit.snippet.contains("İtirazın reddine"));
        assert!(hit.snippet.starts_with('…'), "pencere metnin ortasından");
        let start = hit.match_start.expect("konum") as usize;
        assert!(start > 0);
        // Konum gerçekten eşleşmenin başını gösteriyor.
        let at: String = body.chars().skip(start).take(8).collect();
        assert!(at.starts_with("İtirazın"));

        // Birebir arama Türkçe karakterle de aynı yeri bulur.
        let exact = db
            .search_precedents(&library_query("İtirazın reddine", SearchMode::Exact, None))
            .expect("birebir arama");
        assert_eq!(exact.hits.len(), 1);
    }

    /// Esas/karar numarası künyeden de bulunur; metinde numara geçmese bile
    /// sonuç `metadata` olarak işaretlenir ve konum uydurulmaz.
    #[test]
    fn library_search_docket_mode_matches_metadata_only() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/777"), Some("2025/888")))
            .expect("emsal");
        db.add_precedent_text(
            &p.id,
            &text_of("Metinde numara geçmiyor; yalnız gerekçe var."),
        )
        .expect("metin");

        let page = db
            .search_precedents(&library_query("2024/777 E.", SearchMode::Docket, None))
            .expect("arama");
        assert_eq!(page.hits.len(), 1);
        assert_eq!(page.hits[0].match_source, "metadata");
        assert!(page.hits[0].match_start.is_none());

        let other_year = db
            .search_precedents(&library_query("2023/777 E.", SearchMode::Docket, None))
            .expect("arama");
        assert_eq!(other_year.matched, 0);
    }

    #[test]
    fn library_search_limits_results_but_reports_total_matches() {
        let db = db();
        for index in 0..4 {
            let p = db
                .create_precedent(&precedent(
                    "Yargıtay",
                    Some(&format!("2024/{index}")),
                    Some(&format!("2025/{index}")),
                ))
                .expect("emsal");
            db.add_precedent_text(
                &p.id,
                &text_of(&format!("Karar {index} ORTAKKELIME içerir.")),
            )
            .expect("metin");
        }
        let page = db
            .search_precedents(&library_query("ORTAKKELIME", SearchMode::Auto, Some(2)))
            .expect("arama");
        assert_eq!(page.matched, 4);
        assert_eq!(page.hits.len(), 2);
        assert!(page.truncated);
        assert!(page.notes.iter().any(|n| n.contains("ilk 2")));

        let clamped = db
            .search_precedents(&library_query("ORTAKKELIME", SearchMode::Auto, Some(9_999)))
            .expect("arama");
        assert_eq!(clamped.limit, MAX_PRECEDENT_SEARCH_LIMIT);
        assert_eq!(clamped.hits.len(), 4);
        assert!(!clamped.truncated);
    }

    #[test]
    fn library_search_rejects_empty_query() {
        let db = db();
        assert_eq!(
            db.search_precedents(&library_query("   ", SearchMode::Auto, None))
                .unwrap_err()
                .code(),
            "E_SEARCH_QUERY"
        );
    }

    /// Belge sürümü + ayrıştırma koşusu + sayfalar kurar (F12-T13 testleri).
    /// `pages` = (metin, `text_origin`) çiftleri.
    fn document_version_with_pages(db: &Database, pages: &[(&str, &str)], status: &str) -> String {
        let case = db
            .create_case(&NewCase {
                title: "Kira".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        let blob = db
            .create_blob_record(&format!("st-f12-{}", uuid::Uuid::now_v7()), 10, "aad")
            .expect("blob");
        let doc = db
            .create_document(&case.id, "Karar", "pdf", None, "unknown")
            .expect("belge");
        let version = db
            .add_version(
                &doc.id,
                &case.id,
                &blob,
                &"e".repeat(64),
                10,
                "application/pdf",
            )
            .expect("sürüm");
        let run = db
            .start_parse_run(&version.id, &case.id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        let new_pages: Vec<crate::NewPage> = pages
            .iter()
            .enumerate()
            .map(|(index, (text, origin))| crate::NewPage {
                index: i64::try_from(index).unwrap_or(0),
                text: (*text).to_string(),
                origin: (*origin).to_string(),
                ..Default::default()
            })
            .collect();
        if !new_pages.is_empty() {
            db.store_pages(&run.id, &case.id, &new_pages)
                .expect("sayfalar");
        }
        db.finish_parse_run(&run.id, status, &[]).expect("bitir");
        version.id
    }

    #[test]
    fn document_text_transfer_joins_pages_and_verifies_quotes() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/71"), Some("2025/72")))
            .expect("emsal");
        let version_id = document_version_with_pages(
            &db,
            &[
                ("Birinci sayfa metni.", "native"),
                ("İkinci sayfa metni.", "native"),
            ],
            "succeeded",
        );

        let result = db
            .precedent_text_from_document(&p.id, &version_id)
            .expect("aktarım");
        assert!(result.written);
        assert_eq!(result.pages, 2);
        assert_eq!(result.ocr_pages, 0);
        assert_eq!(result.empty_pages, 0);
        assert_eq!(result.engine, "pymupdf");
        assert_eq!(result.parse_status, "succeeded");
        assert_eq!(result.version.source_kind, "document");
        assert_eq!(
            result.version.document_version_id.as_deref(),
            Some(version_id.as_str())
        );
        assert_eq!(
            result.char_count,
            i64::try_from(
                "Birinci sayfa metni.\n\nİkinci sayfa metni."
                    .chars()
                    .count()
            )
            .expect("sayı")
        );
        assert!(result.version.has_text);

        // Alıntı doğrulaması gerçek metin üzerinde çalışır: sayfa sınırı aşılsa da
        // (çift satır sonu) ikinci sayfadaki ifade bulunur.
        let verification = db
            .verify_precedent_quote(&p.id, "İkinci sayfa metni")
            .expect("doğrulama");
        assert_eq!(verification.state, "match");

        // Denetim: hangi koşudan geldiği yazılır, metin yazılmaz.
        let metadata: String = db
            .conn()
            .query_row(
                "SELECT metadata_allowlist FROM audit_events WHERE operation='text_from_document' ORDER BY sequence DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("denetim");
        assert!(metadata.contains(&result.parse_run_id));
        assert!(!metadata.contains("Birinci sayfa metni"));
    }

    #[test]
    fn document_text_transfer_reports_ocr_and_empty_pages() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/81"), Some("2025/82")))
            .expect("emsal");
        let version_id = document_version_with_pages(
            &db,
            &[
                ("Taranmış sayfa.", "ocr"),
                ("   ", "native"),
                ("", "native"),
            ],
            "partial",
        );
        let result = db
            .precedent_text_from_document(&p.id, &version_id)
            .expect("aktarım");
        assert_eq!(result.pages, 3);
        assert_eq!(result.ocr_pages, 1);
        assert_eq!(result.empty_pages, 2);
        assert!(
            result.notes.iter().any(|n| n.contains("1 sayfa OCR")),
            "OCR notu yok: {:?}",
            result.notes
        );
        assert!(result.notes.iter().any(|n| n.contains("2 sayfa metinsiz")));
        assert!(result.notes.iter().any(|n| n.contains("kısmi")));
        // Boş sayfa metne girmedi; metin yalnız OCR sayfasından.
        assert_eq!(
            result.char_count,
            i64::try_from("Taranmış sayfa.".chars().count()).unwrap()
        );
    }

    #[test]
    fn document_text_transfer_refuses_without_parseable_source() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/91"), Some("2025/92")))
            .expect("emsal");

        // Hiç ayrıştırma yok.
        let case = db
            .create_case(&NewCase {
                title: "Kira".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        let blob = db.create_blob_record("st-none", 10, "aad").expect("blob");
        let doc = db
            .create_document(&case.id, "Ham", "pdf", None, "unknown")
            .expect("belge");
        let unparsed = db
            .add_version(
                &doc.id,
                &case.id,
                &blob,
                &"f".repeat(64),
                10,
                "application/pdf",
            )
            .expect("sürüm");
        assert_eq!(
            db.precedent_text_from_document(&p.id, &unparsed.id)
                .unwrap_err()
                .code(),
            "E_PRECEDENT_SOURCE"
        );

        // Yalnız **başarısız** koşu var: metin yok sayılır, yazılmaz.
        let failed = document_version_with_pages(&db, &[("Metin", "native")], "failed");
        assert_eq!(
            db.precedent_text_from_document(&p.id, &failed)
                .unwrap_err()
                .code(),
            "E_PRECEDENT_SOURCE"
        );

        // Sayfalar var ama hepsi boş.
        let empty = document_version_with_pages(&db, &[("  ", "native")], "succeeded");
        assert_eq!(
            db.precedent_text_from_document(&p.id, &empty)
                .unwrap_err()
                .code(),
            "E_PRECEDENT_SOURCE"
        );

        assert!(
            db.precedent_versions(&p.id).expect("sürümler").is_empty(),
            "başarısız aktarım sürüm yazmamalı"
        );
    }

    #[test]
    fn document_text_transfer_does_not_duplicate_same_hash() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/101"), Some("2025/102")))
            .expect("emsal");
        let version_id =
            document_version_with_pages(&db, &[("Aynı karar metni.", "native")], "succeeded");

        let first = db
            .precedent_text_from_document(&p.id, &version_id)
            .expect("ilk");
        assert!(first.written);
        let second = db
            .precedent_text_from_document(&p.id, &version_id)
            .expect("ikinci");
        assert!(!second.written);
        assert_eq!(second.version.id, first.version.id);
        assert!(second.notes.iter().any(|n| n.contains("zaten kayıtlı")));
        assert_eq!(db.precedent_versions(&p.id).expect("sürümler").len(), 1);
    }

    #[test]
    fn document_text_transfer_rejects_unknown_ids() {
        let db = db();
        let p = db
            .create_precedent(&precedent("Yargıtay", Some("2024/111"), Some("2025/112")))
            .expect("emsal");
        let version_id = document_version_with_pages(&db, &[("Metin.", "native")], "succeeded");
        assert_eq!(
            db.precedent_text_from_document("yok", &version_id)
                .unwrap_err()
                .code(),
            "E_NOT_FOUND"
        );
        assert_eq!(
            db.precedent_text_from_document(&p.id, "yok")
                .unwrap_err()
                .code(),
            "E_NOT_FOUND"
        );
        assert_eq!(
            db.precedent_text_from_document(&p.id, "   ")
                .unwrap_err()
                .code(),
            "E_NOT_FOUND"
        );
    }
}
