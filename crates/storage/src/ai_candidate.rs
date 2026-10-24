//! F11-T05 — AI adayının **şema doğrulaması** ve **alıntı eşleştirmesi**.
//!
//! İki ayrı ölçüm, tek karar:
//!
//! 1. **Şema** — aday yükü, türüne göre tanımlı alan listesine uyuyor mu?
//!    Doğrulama durumu **hesaplanır**; çağıranın beyanıyla yazılmaz. Modelin
//!    uydurduğu fazladan alan kullanılmaz (`unknown_field`), eksik zorunlu alan
//!    reddedilir.
//! 2. **Alıntı** — adayın taşıdığı her alıntı, belgenin gerçek metninde
//!    `dim_evidence::verify_quote` ile aranır. Bulunmayan alıntı kaynak
//!    sayılmaz (INV-02, INV-12). Alıntı modelin dediği sayfada değilse bu ayrı
//!    durumdur (`page_mismatch`); konum uydurulmaz, bulunduğu yer yazılır.
//!
//! **Kapsam dışı:** maskeli bağlamın özgün kaynağa geri eşlenmesi (F11-T03) ve
//! model çağrısının kendisi (F11-T04). Burada yalnız elde bulunan aday yükü ile
//! kasada duran metin karşılaştırılır; hiçbir ağ isteği yapılmaz.

use std::collections::HashMap;

use dim_evidence::{verify_quote, QuoteMatch};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::parsing::PageRow;
use crate::{now_iso, AiCandidateRow, Database, StorageError};

/// Desteklenen aday şeması sürümü. Bilinmeyen sürüm reddedilir.
pub const CANDIDATE_SCHEMA_VERSION: i64 = 1;
/// Tek adayda taşınabilecek en çok alıntı.
pub const MAX_CANDIDATE_QUOTES: usize = 20;
/// Alıntının anlamlı sayılması için en az karakter (F12 ile aynı eşik).
pub const MIN_QUOTE_CHARS: usize = 8;
/// Raporlanan en çok aday konum.
const MAX_REPORTED_POSITIONS: usize = 5;

/// Aday türleri (F11-T01: olay, kişi, tarih, iddia, özet, yanıt).
///
/// Eski kayıtlarla uyum için `*_candidate` adları da kabul edilir; kuyrukta
/// `review_items.item_kind` olarak yine `*_candidate` yazılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    Event,
    Person,
    Date,
    Claim,
    Summary,
    Answer,
}

impl CandidateKind {
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "event" | "event_candidate" => Some(Self::Event),
            "person" | "person_candidate" => Some(Self::Person),
            "date" | "date_candidate" => Some(Self::Date),
            "claim" | "claim_candidate" => Some(Self::Claim),
            "summary" | "summary_candidate" => Some(Self::Summary),
            "answer" | "answer_candidate" => Some(Self::Answer),
            _ => None,
        }
    }

    /// Şemada ve raporda kullanılan kararlı ad.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Person => "person",
            Self::Date => "date",
            Self::Claim => "claim",
            Self::Summary => "summary",
            Self::Answer => "answer",
        }
    }
}

/// Bir alanın beklenen biçimi. Sınırlar şemada tanımlıdır; modelin serbest
/// metni bu sınırlara göre reddedilir.
#[derive(Debug, Clone, Copy)]
enum FieldKind {
    Text { min: usize, max: usize },
    Enum(&'static [&'static str]),
    TextList { max_items: usize, max_chars: usize },
    IsoDate,
}

#[derive(Debug, Clone, Copy)]
struct FieldSpec {
    name: &'static str,
    required: bool,
    kind: FieldKind,
}

const PERSON_ROLES: &[&str] = &[
    "davaci",
    "davali",
    "vekil",
    "tanik",
    "hakim",
    "bilirkisi",
    "diger",
    "unknown",
];
const DATE_KINDS: &[&str] = &["document", "event", "notification", "hearing", "unknown"];
const DATE_PRECISIONS: &[&str] = &["day", "month", "year", "unknown"];

fn field_specs(kind: CandidateKind) -> &'static [FieldSpec] {
    match kind {
        CandidateKind::Event => &[
            FieldSpec {
                name: "title",
                required: true,
                kind: FieldKind::Text { min: 1, max: 200 },
            },
            FieldSpec {
                name: "date",
                required: false,
                kind: FieldKind::Text { min: 0, max: 120 },
            },
            FieldSpec {
                name: "participants",
                required: false,
                kind: FieldKind::TextList {
                    max_items: 20,
                    max_chars: 120,
                },
            },
            FieldSpec {
                name: "description",
                required: false,
                kind: FieldKind::Text { min: 0, max: 2000 },
            },
        ],
        CandidateKind::Person => &[
            FieldSpec {
                name: "name",
                required: true,
                kind: FieldKind::Text { min: 1, max: 160 },
            },
            FieldSpec {
                name: "role",
                required: false,
                kind: FieldKind::Enum(PERSON_ROLES),
            },
            FieldSpec {
                name: "organization",
                required: false,
                kind: FieldKind::Text { min: 0, max: 200 },
            },
        ],
        CandidateKind::Date => &[
            FieldSpec {
                name: "rawText",
                required: true,
                kind: FieldKind::Text { min: 1, max: 200 },
            },
            FieldSpec {
                name: "kind",
                required: true,
                kind: FieldKind::Enum(DATE_KINDS),
            },
            FieldSpec {
                name: "isoDate",
                required: false,
                kind: FieldKind::IsoDate,
            },
            FieldSpec {
                name: "precision",
                required: false,
                kind: FieldKind::Enum(DATE_PRECISIONS),
            },
        ],
        CandidateKind::Claim => &[
            FieldSpec {
                name: "text",
                required: true,
                kind: FieldKind::Text { min: 1, max: 2000 },
            },
            FieldSpec {
                name: "assertedBy",
                required: false,
                kind: FieldKind::Text { min: 0, max: 200 },
            },
            FieldSpec {
                name: "amountText",
                required: false,
                kind: FieldKind::Text { min: 0, max: 120 },
            },
        ],
        CandidateKind::Summary => &[FieldSpec {
            name: "text",
            required: true,
            kind: FieldKind::Text { min: 1, max: 4000 },
        }],
        CandidateKind::Answer => &[
            FieldSpec {
                name: "text",
                required: true,
                kind: FieldKind::Text { min: 1, max: 4000 },
            },
            FieldSpec {
                name: "question",
                required: false,
                kind: FieldKind::Text { min: 0, max: 500 },
            },
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    /// Kararlı kod (`missing_field`, `quote_too_short`, …).
    pub code: String,
    /// `error` adayı geçersiz kılar, `warning` yalnız `partial` yapar.
    pub severity: String,
    /// Alan yolu (`quotes[0].quote`).
    pub field: String,
    pub detail: String,
}

fn issue(code: &str, severity: &str, field: &str, detail: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        severity: severity.to_string(),
        field: field.to_string(),
        detail: detail.into(),
    }
}

/// Adayın taşıdığı alıntı referansı. `startHint` **kullanılmaz**: modelin
/// işaret ettiği konum güvenilir sayılmaz, alıntı metnin tamamında aranır.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRef {
    /// Yükün `quotes` dizisindeki özgün sıra; rapor bu numarayı taşır.
    pub source_index: usize,
    pub document_version_id: String,
    pub quote: String,
    #[serde(default)]
    pub page_index: Option<i64>,
    #[serde(default)]
    pub label: Option<String>,
}

/// Şema doğrulamasının sonucu (alıntı eşleştirmesinden bağımsız).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateSchema {
    pub kind: String,
    /// `valid` | `partial` | `invalid`
    pub state: String,
    pub issues: Vec<ValidationIssue>,
    /// Yalnız yapısal olarak geçerli alıntılar; geçersizler eşleştirilmez.
    pub quote_refs: Vec<QuoteRef>,
    pub field_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotePosition {
    pub page_index: i64,
    pub char_start: i64,
}

/// Alıntı başına eşleşme raporu. `state`:
/// `exact` · `normalized` · `ambiguous` · `page_mismatch` · `not_found` ·
/// `unverifiable` (`reason`: `version_not_found`, `case_mismatch`, `no_text`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteMatchReport {
    pub index: usize,
    pub document_version_id: String,
    pub quote: String,
    #[serde(default)]
    pub label: Option<String>,
    pub state: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub claimed_page_index: Option<i64>,
    #[serde(default)]
    pub page_index: Option<i64>,
    #[serde(default)]
    pub char_start: Option<i64>,
    pub occurrences: i64,
    #[serde(default)]
    pub candidates: Vec<QuotePosition>,
    #[serde(default)]
    pub text_origin: Option<String>,
    #[serde(default)]
    pub parse_run_id: Option<String>,
    #[serde(default)]
    pub segment_id: Option<String>,
}

/// Adayın bütün doğrulama sonucu: şema + alıntı + birleşik karar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateReport {
    pub kind: String,
    /// `valid` | `partial` | `invalid`
    pub state: String,
    pub schema_state: String,
    /// `verified` | `partial` | `failed` | `no_quotes` | `not_checked`
    pub quote_state: String,
    pub issues: Vec<ValidationIssue>,
    pub quotes: Vec<QuoteMatchReport>,
    pub notes: Vec<String>,
    pub checked_at: String,
}

/// Aday yükünü türüne göre doğrular. **Saf fonksiyon**: veritabanına bakmaz,
/// yalnız JSON yapısını ve alan sınırlarını denetler.
#[must_use]
pub fn validate_candidate_payload(kind_raw: &str, payload: &str) -> CandidateSchema {
    let Some(kind) = CandidateKind::parse(kind_raw) else {
        return CandidateSchema {
            kind: kind_raw.trim().to_string(),
            state: "invalid".into(),
            issues: vec![issue(
                "unknown_candidate_type",
                "error",
                "type",
                "Bilinmeyen aday türü; şema tanımlı değil.",
            )],
            quote_refs: Vec::new(),
            field_count: 0,
        };
    };

    let parsed: Value = match serde_json::from_str(payload) {
        Ok(value) => value,
        Err(err) => {
            return CandidateSchema {
                kind: kind.label().to_string(),
                state: "invalid".into(),
                issues: vec![issue(
                    "invalid_json",
                    "error",
                    "$",
                    format!(
                        "Yük JSON olarak çözülemedi (satır {}, sütun {}).",
                        err.line(),
                        err.column()
                    ),
                )],
                quote_refs: Vec::new(),
                field_count: 0,
            }
        }
    };

    let Value::Object(obj) = parsed else {
        return CandidateSchema {
            kind: kind.label().to_string(),
            state: "invalid".into(),
            issues: vec![issue(
                "not_object",
                "error",
                "$",
                "Aday yükü bir JSON nesnesi olmalı.",
            )],
            quote_refs: Vec::new(),
            field_count: 0,
        };
    };

    let mut issues = Vec::new();
    let field_count = obj.len();

    match obj.get("schemaVersion") {
        None => issues.push(issue(
            "missing_field",
            "error",
            "schemaVersion",
            format!("Şema sürümü zorunlu ({CANDIDATE_SCHEMA_VERSION})."),
        )),
        Some(Value::Number(number)) if number.as_i64() == Some(CANDIDATE_SCHEMA_VERSION) => {}
        Some(_) => issues.push(issue(
            "schema_version_unsupported",
            "error",
            "schemaVersion",
            format!("Yalnız şema sürümü {CANDIDATE_SCHEMA_VERSION} desteklenir."),
        )),
    }

    let specs = field_specs(kind);
    for spec in specs {
        if spec.required && !obj.contains_key(spec.name) {
            issues.push(issue(
                "missing_field",
                "error",
                spec.name,
                "Zorunlu alan yok.",
            ));
        }
    }
    for (key, value) in &obj {
        if key == "schemaVersion" || key == "quotes" {
            continue;
        }
        match specs.iter().find(|spec| spec.name == key.as_str()) {
            Some(spec) => check_field(key, value, &spec.kind, &mut issues),
            None => issues.push(issue(
                "unknown_field",
                "warning",
                key,
                "Şemada tanımlı olmayan alan kullanılmaz.",
            )),
        }
    }

    let quote_refs = match obj.get("quotes") {
        Some(value) => parse_quotes(value, &mut issues),
        None => Vec::new(),
    };
    // Özet cümlesi dayanağına bağlı olmak zorunda (F11-T07).
    if kind == CandidateKind::Summary && quote_refs.is_empty() && !obj.contains_key("quotes") {
        issues.push(issue(
            "missing_field",
            "error",
            "quotes",
            "Özet adayı en az bir alıntı taşımalı.",
        ));
    }

    issues.sort_by(|a, b| (&a.field, &a.code).cmp(&(&b.field, &b.code)));
    issues.dedup();

    // Alıntı yokluğu şema kusuru değildir; `quote_state` ayrı ölçülür.
    let state = if issues.iter().any(|item| item.severity == "error") {
        "invalid"
    } else if issues.is_empty() {
        "valid"
    } else {
        "partial"
    };

    CandidateSchema {
        kind: kind.label().to_string(),
        state: state.into(),
        issues,
        quote_refs,
        field_count,
    }
}

fn check_field(name: &str, value: &Value, kind: &FieldKind, issues: &mut Vec<ValidationIssue>) {
    match kind {
        FieldKind::Text { min, max } => match value {
            Value::String(text) => {
                let len = text.trim().chars().count();
                if len < *min {
                    issues.push(issue("empty_text", "error", name, "Alan boş olamaz."));
                } else if len > *max {
                    issues.push(issue(
                        "too_long",
                        "error",
                        name,
                        format!("En çok {max} karakter olabilir."),
                    ));
                }
                if text
                    .chars()
                    .any(|ch| ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t')
                {
                    issues.push(issue(
                        "control_chars",
                        "error",
                        name,
                        "Kontrol karakteri içeriyor.",
                    ));
                }
            }
            _ => issues.push(issue("wrong_type", "error", name, "Metin bekleniyordu.")),
        },
        FieldKind::Enum(values) => match value {
            Value::String(text) if values.contains(&text.as_str()) => {}
            Value::String(text) => issues.push(issue(
                "bad_enum",
                "error",
                name,
                format!("Bilinmeyen değer: {text}."),
            )),
            _ => issues.push(issue("wrong_type", "error", name, "Metin bekleniyordu.")),
        },
        FieldKind::TextList {
            max_items,
            max_chars,
        } => match value {
            Value::Array(items) => {
                if items.len() > *max_items {
                    issues.push(issue(
                        "too_many_items",
                        "error",
                        name,
                        format!("En çok {max_items} öğe olabilir."),
                    ));
                }
                for (index, item) in items.iter().enumerate() {
                    let ok = matches!(item, Value::String(text)
                        if !text.trim().is_empty() && text.chars().count() <= *max_chars);
                    if !ok {
                        issues.push(issue(
                            "wrong_type",
                            "error",
                            &format!("{name}[{index}]"),
                            "Boş olmayan metin bekleniyordu.",
                        ));
                    }
                }
            }
            _ => issues.push(issue(
                "wrong_type",
                "error",
                name,
                "Metin listesi bekleniyordu.",
            )),
        },
        FieldKind::IsoDate => match value {
            Value::String(text) if is_iso_date(text) => {}
            _ => issues.push(issue(
                "bad_date",
                "error",
                name,
                "Tarih YYYY-AA-GG biçiminde ve gerçek bir gün olmalı.",
            )),
        },
    }
}

fn parse_quotes(value: &Value, issues: &mut Vec<ValidationIssue>) -> Vec<QuoteRef> {
    let Value::Array(items) = value else {
        issues.push(issue(
            "wrong_type",
            "error",
            "quotes",
            "Alıntı listesi bekleniyordu.",
        ));
        return Vec::new();
    };
    if items.len() > MAX_CANDIDATE_QUOTES {
        issues.push(issue(
            "too_many_items",
            "error",
            "quotes",
            format!("En çok {MAX_CANDIDATE_QUOTES} alıntı olabilir."),
        ));
    }

    let mut accepted: Vec<(usize, QuoteRef)> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let field = format!("quotes[{index}]");
        let Value::Object(map) = item else {
            issues.push(issue(
                "wrong_type",
                "error",
                &field,
                "Alıntı bir nesne olmalı.",
            ));
            continue;
        };
        for key in map.keys() {
            if !matches!(
                key.as_str(),
                "documentVersionId" | "quote" | "pageIndex" | "startHint" | "label"
            ) {
                issues.push(issue(
                    "unknown_field",
                    "warning",
                    &format!("{field}.{key}"),
                    "Şemada tanımlı olmayan alan kullanılmaz.",
                ));
            }
        }

        let version = map
            .get("documentVersionId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty());
        if version.is_none() {
            issues.push(issue(
                "quote_missing_field",
                "error",
                &format!("{field}.documentVersionId"),
                "Belge sürümü kimliği zorunlu.",
            ));
        }
        let quote = map
            .get("quote")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty());
        if quote.is_none() {
            issues.push(issue(
                "quote_missing_field",
                "error",
                &format!("{field}.quote"),
                "Alıntı metni zorunlu.",
            ));
        }

        let page_index = match map.get("pageIndex") {
            None | Some(Value::Null) => None,
            Some(Value::Number(number)) => match number.as_i64() {
                Some(value) if value >= 0 => Some(value),
                _ => {
                    issues.push(issue(
                        "wrong_type",
                        "error",
                        &format!("{field}.pageIndex"),
                        "Sayfa numarası sıfır veya pozitif tam sayı olmalı.",
                    ));
                    None
                }
            },
            Some(_) => {
                issues.push(issue(
                    "wrong_type",
                    "error",
                    &format!("{field}.pageIndex"),
                    "Sayfa numarası tam sayı olmalı.",
                ));
                None
            }
        };
        // `startHint` şemada kabul edilir ama **eşleştirmede kullanılmaz**;
        // yine de biçimi denetlenir (yanlış tip sessizce yutulmaz).
        match map.get("startHint") {
            None | Some(Value::Null) => {}
            Some(Value::Number(number)) if number.as_i64().is_some_and(|value| value >= 0) => {}
            Some(_) => issues.push(issue(
                "wrong_type",
                "error",
                &format!("{field}.startHint"),
                "İpucu konumu negatif olmayan tam sayı olmalı.",
            )),
        }
        let label = match map.get("label") {
            None | Some(Value::Null) => None,
            Some(Value::String(text)) if text.trim().is_empty() => None,
            Some(Value::String(text)) if text.chars().count() <= 120 => {
                Some(text.trim().to_string())
            }
            Some(Value::String(_)) => {
                issues.push(issue(
                    "too_long",
                    "error",
                    &format!("{field}.label"),
                    "Etiket en çok 120 karakter olabilir.",
                ));
                None
            }
            Some(_) => {
                issues.push(issue(
                    "wrong_type",
                    "error",
                    &format!("{field}.label"),
                    "Etiket metin olmalı.",
                ));
                None
            }
        };

        if let (Some(version), Some(quote)) = (version, quote) {
            let len = quote.chars().count();
            if len < MIN_QUOTE_CHARS {
                issues.push(issue(
                    "quote_too_short",
                    "error",
                    &format!("{field}.quote"),
                    format!("Alıntı en az {MIN_QUOTE_CHARS} karakter olmalı."),
                ));
            } else if len > 1000 {
                issues.push(issue(
                    "too_long",
                    "error",
                    &format!("{field}.quote"),
                    "Alıntı en çok 1000 karakter olabilir.",
                ));
            } else {
                accepted.push((
                    index,
                    QuoteRef {
                        source_index: index,
                        document_version_id: version.to_string(),
                        quote: quote.to_string(),
                        page_index,
                        label,
                    },
                ));
            }
        }
    }

    for (position, (left_index, left)) in accepted.iter().enumerate() {
        for (right_index, right) in accepted.iter().enumerate().skip(position + 1) {
            if left.document_version_id == right.1.document_version_id
                && left.quote == right.1.quote
            {
                issues.push(issue(
                    "duplicate_quote",
                    "error",
                    &format!("quotes[{right_index}]"),
                    format!("Aynı alıntı quotes[{left_index}] ile tekrar ediyor."),
                ));
            }
        }
    }

    accepted.into_iter().map(|(_, quote)| quote).collect()
}

/// `YYYY-AA-GG` biçiminde **gerçek** bir gün mü? Ay/gün aralığı ve artık yıl
/// denetlenir; yaklaşık tarih bu alana yazılmaz (ham metin `rawText`te durur).
#[must_use]
pub fn is_iso_date(text: &str) -> bool {
    if text.len() != 10 || !text.is_ascii() {
        return false;
    }
    let bytes = text.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        text[0..4].parse::<i32>(),
        text[5..7].parse::<u32>(),
        text[8..10].parse::<u32>(),
    ) else {
        return false;
    };
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            if leap {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    (1..=max_day).contains(&day)
}

struct VersionScope {
    case_id: String,
    parse_run_id: Option<String>,
    pages: Vec<PageRow>,
}

impl Database {
    /// Adayı **yazmadan** doğrular: şema + alıntı eşleştirmesi.
    pub fn validate_ai_candidate(
        &self,
        run_id: &str,
        candidate_type: &str,
        payload: &str,
    ) -> Result<CandidateReport, StorageError> {
        let run = self.get_ai_run(run_id)?.ok_or(StorageError::NotFound)?;
        let schema = validate_candidate_payload(candidate_type, payload);
        let mut quotes = Vec::new();
        let mut notes = Vec::new();

        let quote_state = if schema.state == "invalid" {
            notes.push("Şema geçersiz olduğu için alıntılar eşleştirilmedi.".to_string());
            "not_checked"
        } else if schema.quote_refs.is_empty() {
            notes.push(
                "Aday alıntı taşımıyor; hiçbir kaynak doğrulanamaz, aday yalnız inceleme \
                 kuyruğuna düşebilir."
                    .to_string(),
            );
            "no_quotes"
        } else {
            let mut scopes: HashMap<String, Option<VersionScope>> = HashMap::new();
            for quote in &schema.quote_refs {
                if !scopes.contains_key(&quote.document_version_id) {
                    let loaded = self.load_version_scope(&quote.document_version_id)?;
                    scopes.insert(quote.document_version_id.clone(), loaded);
                }
                let scope = scopes
                    .get(&quote.document_version_id)
                    .and_then(Option::as_ref);
                quotes.push(match_quote(&run.case_id, quote.source_index, quote, scope));
            }
            let state = aggregate_quote_state(&quotes);
            let exact = quotes.iter().filter(|item| item.state == "exact").count();
            notes.push(format!(
                "{} alıntıdan {exact} tanesi belge metninde birebir bulundu.",
                quotes.len()
            ));
            if quotes
                .iter()
                .any(|item| matches!(item.text_origin.as_deref(), Some("ocr" | "mixed")))
            {
                notes.push(
                    "Bazı alıntılar OCR ile çıkarılmış sayfalarda bulundu; OCR yazım hatası \
                     eşleşmeyi etkileyebilir."
                        .to_string(),
                );
            }
            state
        };

        let mut issues = schema.issues.clone();
        issues.extend(quote_issues(&quotes));
        if quote_state == "no_quotes" {
            issues.push(issue(
                "no_quotes",
                "warning",
                "quotes",
                "Aday alıntı taşımıyor; kaynak doğrulanamadı.",
            ));
        }
        issues.sort_by(|a, b| (&a.field, &a.code).cmp(&(&b.field, &b.code)));
        issues.dedup();

        let state = if schema.state == "invalid" || quote_state == "failed" {
            "invalid"
        } else if schema.state == "partial" || quote_state != "verified" {
            "partial"
        } else {
            "valid"
        };

        Ok(CandidateReport {
            kind: schema.kind,
            state: state.to_string(),
            schema_state: schema.state,
            quote_state: quote_state.to_string(),
            issues,
            quotes,
            notes,
            checked_at: now_iso(),
        })
    }

    /// Adayı doğrular ve **hesaplanan** durumla kaydeder. Çağıran doğrulama
    /// durumunu seçemez; `valid` yalnız şema geçip her alıntı birebir
    /// bulunduğunda yazılır.
    pub fn record_ai_candidate(
        &self,
        run_id: &str,
        candidate_type: &str,
        payload: &str,
    ) -> Result<AiCandidateRow, StorageError> {
        let report = self.validate_ai_candidate(run_id, candidate_type, payload)?;
        let id = uuid::Uuid::now_v7().to_string();
        let errors = serde_json::to_string(&report.issues).unwrap_or_else(|_| "[]".to_string());
        let quote_report =
            serde_json::to_string(&report.quotes).unwrap_or_else(|_| "[]".to_string());
        self.conn().execute(
            "INSERT INTO ai_candidates (id,ai_run_id,type,structured_payload,validation_state,validation_errors,schema_state,quote_state,quote_report,checked_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            rusqlite::params![
                id,
                run_id,
                candidate_type,
                payload,
                report.state,
                errors,
                report.schema_state,
                report.quote_state,
                quote_report,
                report.checked_at
            ],
        )?;
        self.append_audit(
            "ai_candidate",
            Some(&id),
            "record",
            "local-system",
            &serde_json::json!({
                "kind": candidate_type,
                "state": report.state,
                "quotes": report.quotes.len(),
            }),
        )?;
        self.get_ai_candidate(&id)?.ok_or(StorageError::NotFound)
    }

    /// Metin değişmiş olabilir (yeni ayrıştırma): adayı yeniden doğrular.
    pub fn revalidate_ai_candidate(
        &self,
        candidate_id: &str,
    ) -> Result<AiCandidateRow, StorageError> {
        let row = self
            .conn()
            .query_row(
                "SELECT type,structured_payload,ai_run_id FROM ai_candidates WHERE id=?1",
                [candidate_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(StorageError::NotFound)?;
        let (kind, payload, run_id) = row;
        let report = self.validate_ai_candidate(&run_id, &kind, &payload)?;
        let errors = serde_json::to_string(&report.issues).unwrap_or_else(|_| "[]".to_string());
        let quote_report =
            serde_json::to_string(&report.quotes).unwrap_or_else(|_| "[]".to_string());
        self.conn().execute(
            "UPDATE ai_candidates SET validation_state=?1, validation_errors=?2, schema_state=?3, quote_state=?4, quote_report=?5, checked_at=?6 WHERE id=?7",
            rusqlite::params![
                report.state,
                errors,
                report.schema_state,
                report.quote_state,
                quote_report,
                report.checked_at,
                candidate_id
            ],
        )?;
        self.append_audit(
            "ai_candidate",
            Some(candidate_id),
            "revalidate",
            "local-user",
            &serde_json::json!({
                "state": report.state,
                "quotes": report.quotes.len(),
            }),
        )?;
        self.get_ai_candidate(candidate_id)?
            .ok_or(StorageError::NotFound)
    }

    pub fn get_ai_candidate(&self, id: &str) -> Result<Option<AiCandidateRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,ai_run_id,type,structured_payload,validation_state,validation_errors,
                    schema_state,quote_state,quote_report,checked_at
             FROM ai_candidates WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], crate::ai::map_candidate).optional()?)
    }

    fn load_version_scope(&self, version_id: &str) -> Result<Option<VersionScope>, StorageError> {
        let case_id: Option<String> = self
            .conn()
            .query_row(
                "SELECT d.case_id FROM document_versions v JOIN documents d ON d.id=v.document_id
                 WHERE v.id=?1",
                [version_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(case_id) = case_id else {
            return Ok(None);
        };
        let parse_run_id: Option<String> = self
            .conn()
            .query_row(
                "SELECT id FROM parse_runs
                 WHERE document_version_id=?1 AND status IN ('succeeded','partial')
                 ORDER BY COALESCE(completed_at, created_at) DESC, id DESC LIMIT 1",
                [version_id],
                |row| row.get(0),
            )
            .optional()?;
        let pages = match parse_run_id.as_deref() {
            Some(run_id) => self.pages_for_run(run_id)?,
            None => Vec::new(),
        };
        Ok(Some(VersionScope {
            case_id,
            parse_run_id,
            pages,
        }))
    }
}

fn match_quote(
    case_id: &str,
    index: usize,
    quote: &QuoteRef,
    scope: Option<&VersionScope>,
) -> QuoteMatchReport {
    let mut report = QuoteMatchReport {
        index,
        document_version_id: quote.document_version_id.clone(),
        quote: quote.quote.clone(),
        label: quote.label.clone(),
        state: "unverifiable".into(),
        reason: None,
        claimed_page_index: quote.page_index,
        page_index: None,
        char_start: None,
        occurrences: 0,
        candidates: Vec::new(),
        text_origin: None,
        parse_run_id: None,
        segment_id: None,
    };
    let Some(scope) = scope else {
        report.reason = Some("version_not_found".into());
        return report;
    };
    // Başka dosyanın belgesi kaynak olamaz: bu bir uyuşmazlık, "bulunamadı" değil.
    if scope.case_id != case_id {
        report.reason = Some("case_mismatch".into());
        return report;
    }
    let Some(parse_run_id) = scope.parse_run_id.as_deref() else {
        report.reason = Some("no_text".into());
        return report;
    };
    report.parse_run_id = Some(parse_run_id.to_string());
    if scope.pages.is_empty() {
        report.reason = Some("no_text".into());
        return report;
    }

    let mut hits: Vec<QuotePosition> = Vec::new();
    let mut origins: Vec<Option<String>> = Vec::new();
    let mut segments: Vec<Option<String>> = Vec::new();
    let mut ambiguous_pages = 0usize;
    let mut normalized: Option<(i64, Option<String>)> = None;

    for page in &scope.pages {
        let outcome = verify_quote(&page.text, &quote.quote, None, None);
        match outcome {
            QuoteMatch::Exact | QuoteMatch::Ambiguous => {
                if outcome == QuoteMatch::Ambiguous {
                    ambiguous_pages += 1;
                }
                for (byte, _) in page.text.match_indices(&quote.quote) {
                    hits.push(QuotePosition {
                        page_index: page.page_index,
                        char_start: byte_to_char(&page.text, byte),
                    });
                    origins.push(Some(page.text_origin.clone()));
                    segments.push(page.segment_id.clone());
                }
            }
            QuoteMatch::Normalized => {
                normalized = Some((page.page_index, Some(page.text_origin.clone())));
            }
            QuoteMatch::NotFound | QuoteMatch::HashMismatch => {}
        }
    }

    if hits.len() == 1 && ambiguous_pages == 0 {
        let position = &hits[0];
        report.state = if quote
            .page_index
            .is_some_and(|claimed| claimed != position.page_index)
        {
            "page_mismatch".to_string()
        } else {
            "exact".to_string()
        };
        report.page_index = Some(position.page_index);
        report.char_start = Some(position.char_start);
        report.occurrences = 1;
        report.text_origin = origins.first().cloned().flatten();
        report.segment_id = segments.first().cloned().flatten();
    } else if hits.len() > 1 || ambiguous_pages > 0 {
        report.state = "ambiguous".into();
        report.occurrences = i64::try_from(hits.len()).unwrap_or(i64::MAX);
        report.candidates = hits.iter().take(MAX_REPORTED_POSITIONS).cloned().collect();
        if let Some(first) = hits.first() {
            report.page_index = Some(first.page_index);
            report.char_start = Some(first.char_start);
            report.text_origin = origins.first().cloned().flatten();
            report.segment_id = segments.first().cloned().flatten();
        }
    } else if let Some((page_index, origin)) = normalized {
        report.state = "normalized".into();
        report.page_index = Some(page_index);
        report.text_origin = origin;
    } else {
        report.state = "not_found".into();
    }
    report
}

fn aggregate_quote_state(quotes: &[QuoteMatchReport]) -> &'static str {
    if quotes.is_empty() {
        return "no_quotes";
    }
    let mut failed = false;
    let mut imprecise = false;
    for quote in quotes {
        match quote.state.as_str() {
            "exact" => {}
            "not_found" => failed = true,
            "unverifiable"
                if matches!(
                    quote.reason.as_deref(),
                    Some("version_not_found" | "case_mismatch")
                ) =>
            {
                failed = true;
            }
            _ => imprecise = true,
        }
    }
    if failed {
        "failed"
    } else if imprecise {
        "partial"
    } else {
        "verified"
    }
}

fn quote_issues(quotes: &[QuoteMatchReport]) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    for quote in quotes {
        let field = format!("quotes[{}]", quote.index);
        match quote.state.as_str() {
            "exact" => {}
            "not_found" => issues.push(issue(
                "quote_not_found",
                "error",
                &field,
                "Alıntı belge metninde bulunamadı; kaynak sayılmaz.",
            )),
            "unverifiable" => match quote.reason.as_deref() {
                Some("case_mismatch") => issues.push(issue(
                    "quote_case_mismatch",
                    "error",
                    &field,
                    "Alıntının belgesi bu dosyaya ait değil.",
                )),
                Some("version_not_found") => issues.push(issue(
                    "quote_version_not_found",
                    "error",
                    &field,
                    "Belirtilen belge sürümü bulunamadı.",
                )),
                _ => issues.push(issue(
                    "quote_unverifiable",
                    "warning",
                    &field,
                    "Belgenin ayrıştırılmış metni yok; alıntı doğrulanamadı.",
                )),
            },
            "ambiguous" => issues.push(issue(
                "quote_ambiguous",
                "warning",
                &field,
                "Alıntı birden fazla yerde geçiyor; hangisi kastedildiği belirsiz.",
            )),
            "page_mismatch" => issues.push(issue(
                "quote_page_mismatch",
                "warning",
                &field,
                "Alıntı bulundu ama modelin bildirdiği sayfada değil.",
            )),
            "normalized" => issues.push(issue(
                "quote_normalized",
                "warning",
                &field,
                "Alıntı yalnız boşluk/karakter normalizasyonuyla eşleşti.",
            )),
            _ => {}
        }
    }
    issues
}

fn byte_to_char(text: &str, byte: usize) -> i64 {
    i64::try_from(text[..byte].chars().count()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::{AiPolicy, DatabaseKey, NewAiRun, NewPage, NewTransferConsent, Sensitivity};

    const PAGE_ZERO: &str =
        "Kiracı, 12 Mart 2024 tarihli dilekçesinde ödeme yapılmadığını belirtmiştir.";
    const PAGE_ONE: &str =
        "Mahkeme, itirazın reddine karar verdi. Gerekçede sözleşmenin feshedildiği yazılıdır.";

    struct Fixture {
        db: Database,
        case_id: String,
        version_id: String,
        run_id: String,
        parse_run_id: String,
    }

    fn setup(pages: &[&str]) -> Fixture {
        let db = Database::open_in_memory(&DatabaseKey::new([73u8; 32])).expect("db");
        let case = db
            .create_case(&NewCase {
                title: "Kira".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: Sensitivity::Normal,
                default_ai_policy: AiPolicy::Off,
            })
            .expect("case");
        db.conn()
            .execute(
                "INSERT INTO provider_profiles (id,provider_type,adapter_id,policy_version,created_at,updated_at)
                 VALUES ('p1','cli','claude-code','1','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                [],
            )
            .expect("provider");
        let consent = db
            .create_transfer_consent(&NewTransferConsent {
                provider_snapshot: "claude-code@2.1".into(),
                case_scope: case.id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![],
                expires_at: None,
                consent_text_version: "1".into(),
            })
            .expect("consent");
        let run = db
            .start_ai_run(
                &case.id,
                &NewAiRun {
                    task_type: "summarize".into(),
                    provider_id: "p1".into(),
                    model_snapshot: "unknown".into(),
                    input_manifest_hash: "h".into(),
                    prompt_version: "1".into(),
                    output_schema_version: "1".into(),
                    consent_id: consent.id,
                },
            )
            .expect("run");
        let blob = db
            .create_blob_record(&format!("st-{}", uuid::Uuid::now_v7()), 10, "aad")
            .expect("blob");
        let document = db
            .create_document(&case.id, "Dilekçe.pdf", "petition", None, "unknown")
            .expect("document");
        let version = db
            .add_version(
                &document.id,
                &case.id,
                &blob,
                &"a".repeat(64),
                10,
                "application/pdf",
            )
            .expect("version");
        let parse_run = db
            .start_parse_run(&version.id, &case.id, "plain", "2", "{}")
            .expect("parse");
        let new_pages: Vec<NewPage> = pages
            .iter()
            .enumerate()
            .map(|(index, text)| NewPage {
                index: i64::try_from(index).expect("index"),
                text: (*text).to_string(),
                width: Some(595.0),
                height: Some(842.0),
                origin: "native".into(),
                warnings: vec![],
                rotation: 0,
                coordinate_space: Some("unrotated_cropbox_points".into()),
                page_label: None,
                blocks: vec![],
            })
            .collect();
        db.store_pages(&parse_run.id, &case.id, &new_pages)
            .expect("pages");
        db.finish_parse_run(&parse_run.id, "succeeded", &[])
            .expect("finish");
        Fixture {
            db,
            case_id: case.id,
            version_id: version.id,
            run_id: run.id,
            parse_run_id: parse_run.id,
        }
    }

    fn candidate(version_id: &str, quote: &str, page_index: Option<i64>) -> String {
        let mut quote_ref = serde_json::json!({
            "documentVersionId": version_id,
            "quote": quote,
        });
        if let Some(index) = page_index {
            quote_ref["pageIndex"] = serde_json::json!(index);
        }
        serde_json::json!({
            "schemaVersion": 1,
            "text": "Kiracı ödeme yapılmadığını belirtmiştir.",
            "quotes": [quote_ref],
        })
        .to_string()
    }

    #[test]
    fn schema_rejects_missing_required_field_and_bad_version() {
        let schema = validate_candidate_payload("event", "{\"schemaVersion\":1}");
        assert_eq!(schema.state, "invalid");
        assert!(schema
            .issues
            .iter()
            .any(|i| i.code == "missing_field" && i.field == "title" && i.severity == "error"));

        let schema =
            validate_candidate_payload("event", "{\"schemaVersion\":9,\"title\":\"Ödeme\"}");
        assert_eq!(schema.state, "invalid");
        assert!(schema
            .issues
            .iter()
            .any(|i| i.code == "schema_version_unsupported"));

        let schema = validate_candidate_payload("olay", "{\"schemaVersion\":1,\"title\":\"x\"}");
        assert_eq!(schema.state, "invalid");
        assert!(schema
            .issues
            .iter()
            .any(|i| i.code == "unknown_candidate_type"));
    }

    #[test]
    fn unknown_field_is_warning_but_wrong_type_is_error() {
        let schema = validate_candidate_payload(
            "event",
            "{\"schemaVersion\":1,\"title\":\"Ödeme\",\"modelNote\":\"ekstra\"}",
        );
        assert_eq!(schema.state, "partial");
        assert!(schema
            .issues
            .iter()
            .any(|i| i.code == "unknown_field" && i.severity == "warning"));

        let schema = validate_candidate_payload("event", "{\"schemaVersion\":1,\"title\":42}");
        assert_eq!(schema.state, "invalid");
        assert!(schema
            .issues
            .iter()
            .any(|i| i.code == "wrong_type" && i.field == "title"));
    }

    #[test]
    fn summary_requires_a_quote_and_short_quotes_are_rejected() {
        let schema =
            validate_candidate_payload("summary", "{\"schemaVersion\":1,\"text\":\"Özet\"}");
        assert_eq!(schema.state, "invalid");
        assert!(schema
            .issues
            .iter()
            .any(|i| i.code == "missing_field" && i.field == "quotes"));

        let schema = serde_json::json!({
            "schemaVersion": 1,
            "text": "Özet",
            "quotes": [{"documentVersionId": "v1", "quote": "kısa"}],
        })
        .to_string();
        let schema = validate_candidate_payload("summary", &schema);
        assert_eq!(schema.state, "invalid");
        assert!(schema.issues.iter().any(|i| i.code == "quote_too_short"));

        let duplicate = serde_json::json!({
            "schemaVersion": 1,
            "text": "Özet",
            "quotes": [
                {"documentVersionId": "v1", "quote": "ödeme yapılmadığı belirtildi"},
                {"documentVersionId": "v1", "quote": "ödeme yapılmadığı belirtildi"},
            ],
        })
        .to_string();
        let schema = validate_candidate_payload("summary", &duplicate);
        assert_eq!(schema.state, "invalid");
        assert!(schema.issues.iter().any(|i| i.code == "duplicate_quote"));
    }

    #[test]
    fn quote_label_and_start_hint_types_are_checked() {
        let payload = serde_json::json!({
            "schemaVersion": 1,
            "text": "İddia",
            "quotes": [{
                "documentVersionId": "v1",
                "quote": "ödeme yapılmadığı belirtildi",
                "startHint": "burada",
            }],
        })
        .to_string();
        let schema = validate_candidate_payload("claim", &payload);
        assert_eq!(schema.state, "invalid");
        assert!(schema
            .issues
            .iter()
            .any(|i| i.code == "wrong_type" && i.field == "quotes[0].startHint"));

        let payload = serde_json::json!({
            "schemaVersion": 1,
            "text": "İddia",
            "quotes": [{
                "documentVersionId": "v1",
                "quote": "ödeme yapılmadığı belirtildi",
                "label": "x".repeat(121),
            }],
        })
        .to_string();
        let schema = validate_candidate_payload("claim", &payload);
        assert_eq!(schema.state, "invalid");
        assert!(schema
            .issues
            .iter()
            .any(|i| i.code == "too_long" && i.field == "quotes[0].label"));
    }

    #[test]
    fn iso_date_requires_a_real_calendar_day() {
        assert!(is_iso_date("2024-03-12"));
        assert!(is_iso_date("2024-02-29"));
        assert!(!is_iso_date("2023-02-29"));
        assert!(!is_iso_date("2024-13-01"));
        assert!(!is_iso_date("2024-04-31"));
        assert!(!is_iso_date("12.03.2024"));
        assert!(!is_iso_date("2024-3-12"));
        assert!(!is_iso_date("2024-03-12 "));
    }

    #[test]
    fn exact_quote_is_located_and_candidate_becomes_valid() {
        let fixture = setup(&[PAGE_ZERO, PAGE_ONE]);
        let payload = candidate(
            &fixture.version_id,
            "ödeme yapılmadığını belirtmiştir",
            Some(0),
        );
        let report = fixture
            .db
            .validate_ai_candidate(&fixture.run_id, "claim", &payload)
            .expect("rapor");
        assert_eq!(report.state, "valid", "{:?}", report.issues);
        assert_eq!(report.quote_state, "verified");
        let quote = report.quotes.first().expect("alıntı");
        assert_eq!(quote.state, "exact");
        assert_eq!(quote.page_index, Some(0));
        let expected = PAGE_ZERO
            .find("ödeme yapılmadığını belirtmiştir")
            .expect("konum");
        assert_eq!(
            quote.char_start,
            Some(i64::try_from(PAGE_ZERO[..expected].chars().count()).expect("karakter"))
        );
        assert!(quote.segment_id.is_some());
        assert_eq!(quote.occurrences, 1);
    }

    #[test]
    fn fabricated_quote_is_invalid_and_blocks_promotion() {
        let fixture = setup(&[PAGE_ZERO]);
        let payload = candidate(
            &fixture.version_id,
            "kiracı kira bedelini peşin ödemiştir",
            None,
        );
        let candidate_row = fixture
            .db
            .record_ai_candidate(&fixture.run_id, "claim", &payload)
            .expect("kayıt");
        assert_eq!(candidate_row.validation_state, "invalid");
        assert_eq!(candidate_row.quote_state, "failed");
        assert!(candidate_row
            .quote_report
            .iter()
            .any(|q| q.state == "not_found"));

        let error = fixture
            .db
            .promote_candidate(&candidate_row.id, &fixture.case_id)
            .expect_err("uydurma alıntı kuyruğa girmemeli");
        assert_eq!(error.code(), "E_AI_QUOTE");
        let pending: i64 = fixture
            .db
            .conn()
            .query_row(
                "SELECT count(*) FROM review_items WHERE case_id=?1",
                [&fixture.case_id],
                |row| row.get(0),
            )
            .expect("sayım");
        assert_eq!(pending, 0);
    }

    #[test]
    fn page_mismatch_is_flagged_but_promotable() {
        let fixture = setup(&[PAGE_ZERO, PAGE_ONE]);
        let payload = candidate(&fixture.version_id, "itirazın reddine karar verdi", Some(0));
        let report = fixture
            .db
            .validate_ai_candidate(&fixture.run_id, "claim", &payload)
            .expect("rapor");
        assert_eq!(report.state, "partial");
        let quote = report.quotes.first().expect("alıntı");
        assert_eq!(quote.state, "page_mismatch");
        assert_eq!(quote.claimed_page_index, Some(0));
        assert_eq!(quote.page_index, Some(1));
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "quote_page_mismatch"));
    }

    #[test]
    fn verified_quote_creates_anchor_and_reuses_it_on_repeat_promotion() {
        let fixture = setup(&[PAGE_ZERO]);
        let payload = candidate(
            &fixture.version_id,
            "ödeme yapılmadığını belirtmiştir",
            Some(0),
        );
        let candidate_row = fixture
            .db
            .record_ai_candidate(&fixture.run_id, "claim", &payload)
            .expect("kayıt");
        assert_eq!(candidate_row.validation_state, "valid");

        let review_id = fixture
            .db
            .promote_candidate(&candidate_row.id, &fixture.case_id)
            .expect("terfi");
        let item = fixture
            .db
            .get_review_item(&review_id)
            .expect("get")
            .expect("var");
        assert_eq!(item.source_anchor_ids.len(), 1);
        let anchor = fixture
            .db
            .get_anchor(&item.source_anchor_ids[0])
            .expect("anchor")
            .expect("var");
        assert_eq!(anchor.exact_quote, "ödeme yapılmadığını belirtmiştir");
        assert_eq!(anchor.parse_run_id, fixture.parse_run_id);
        assert_eq!(anchor.anchor_status, "active");

        let second = fixture
            .db
            .promote_candidate(&candidate_row.id, &fixture.case_id)
            .expect("ikinci terfi");
        let second_item = fixture
            .db
            .get_review_item(&second)
            .expect("get")
            .expect("var");
        assert_eq!(second_item.source_anchor_ids, item.source_anchor_ids);
    }

    #[test]
    fn quote_from_another_case_is_rejected() {
        let fixture = setup(&[PAGE_ZERO]);
        let other_case = fixture
            .db
            .create_case(&NewCase {
                title: "Başka dosya".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: Sensitivity::Normal,
                default_ai_policy: AiPolicy::Off,
            })
            .expect("case");
        let blob = fixture
            .db
            .create_blob_record(&format!("st-{}", uuid::Uuid::now_v7()), 10, "aad")
            .expect("blob");
        let document = fixture
            .db
            .create_document(&other_case.id, "Başka.pdf", "petition", None, "unknown")
            .expect("document");
        let version = fixture
            .db
            .add_version(
                &document.id,
                &other_case.id,
                &blob,
                &"b".repeat(64),
                10,
                "application/pdf",
            )
            .expect("version");
        let payload = candidate(&version.id, "ödeme yapılmadığını belirtmiştir", None);
        let report = fixture
            .db
            .validate_ai_candidate(&fixture.run_id, "claim", &payload)
            .expect("rapor");
        assert_eq!(report.state, "invalid");
        assert_eq!(report.quote_state, "failed");
        let quote = report.quotes.first().expect("alıntı");
        assert_eq!(quote.state, "unverifiable");
        assert_eq!(quote.reason.as_deref(), Some("case_mismatch"));
    }

    #[test]
    fn version_without_parsed_text_is_unverifiable_not_valid() {
        let fixture = setup(&[PAGE_ZERO]);
        let blob = fixture
            .db
            .create_blob_record(&format!("st-{}", uuid::Uuid::now_v7()), 10, "aad")
            .expect("blob");
        let document = fixture
            .db
            .create_document(
                &fixture.case_id,
                "Taranmamış.pdf",
                "petition",
                None,
                "unknown",
            )
            .expect("document");
        let version = fixture
            .db
            .add_version(
                &document.id,
                &fixture.case_id,
                &blob,
                &"c".repeat(64),
                10,
                "application/pdf",
            )
            .expect("version");
        let payload = candidate(&version.id, "ödeme yapılmadığını belirtmiştir", None);
        let report = fixture
            .db
            .validate_ai_candidate(&fixture.run_id, "claim", &payload)
            .expect("rapor");
        assert_eq!(report.state, "partial");
        assert_eq!(report.quote_state, "partial");
        let quote = report.quotes.first().expect("alıntı");
        assert_eq!(quote.state, "unverifiable");
        assert_eq!(quote.reason.as_deref(), Some("no_text"));
    }

    #[test]
    fn revalidation_follows_the_newest_parse_run() {
        let fixture = setup(&[PAGE_ZERO]);
        let payload = candidate(
            &fixture.version_id,
            "ödeme yapılmadığını belirtmiştir",
            Some(0),
        );
        let candidate_row = fixture
            .db
            .record_ai_candidate(&fixture.run_id, "claim", &payload)
            .expect("kayıt");
        assert_eq!(candidate_row.validation_state, "valid");

        // İlk koşuyu eskiye sabitle ki yeni koşu kesin olarak "en yeni" olsun.
        fixture
            .db
            .conn()
            .execute(
                "UPDATE parse_runs SET completed_at='2026-01-01T00:00:00Z' WHERE id=?1",
                [&fixture.parse_run_id],
            )
            .expect("eski koşu");
        let new_run = fixture
            .db
            .start_parse_run(&fixture.version_id, &fixture.case_id, "plain", "3", "{}")
            .expect("yeni koşu");
        fixture
            .db
            .store_pages(
                &new_run.id,
                &fixture.case_id,
                &[NewPage {
                    index: 0,
                    text: "Bu sayfa yeniden ayrıştırıldı ve eski cümle kaldırıldı.".into(),
                    width: Some(595.0),
                    height: Some(842.0),
                    origin: "native".into(),
                    warnings: vec![],
                    rotation: 0,
                    coordinate_space: Some("unrotated_cropbox_points".into()),
                    page_label: None,
                    blocks: vec![],
                }],
            )
            .expect("pages");
        fixture
            .db
            .finish_parse_run(&new_run.id, "succeeded", &[])
            .expect("finish");

        let refreshed = fixture
            .db
            .revalidate_ai_candidate(&candidate_row.id)
            .expect("yeniden doğrula");
        assert_eq!(refreshed.validation_state, "invalid");
        assert_eq!(refreshed.quote_state, "failed");
        assert_eq!(refreshed.quote_report[0].state, "not_found");
    }

    #[test]
    fn legacy_candidate_must_be_revalidated_before_promotion() {
        let fixture = setup(&[PAGE_ZERO]);
        let id = uuid::Uuid::now_v7().to_string();
        fixture
            .db
            .conn()
            .execute(
                "INSERT INTO ai_candidates (id,ai_run_id,type,structured_payload,validation_state,validation_errors)
                 VALUES (?1,?2,'claim_candidate',?3,'valid','[]')",
                rusqlite::params![
                    id,
                    fixture.run_id,
                    candidate(
                        &fixture.version_id,
                        "ödeme yapılmadığını belirtmiştir",
                        Some(0)
                    )
                ],
            )
            .expect("legacy kayıt");
        let row = fixture.db.get_ai_candidate(&id).expect("get").expect("var");
        assert_eq!(row.schema_state, "legacy");
        assert_eq!(row.quote_state, "legacy");
        assert_eq!(
            fixture
                .db
                .promote_candidate(&id, &fixture.case_id)
                .unwrap_err()
                .code(),
            "E_AI_SCHEMA"
        );
        let refreshed = fixture
            .db
            .revalidate_ai_candidate(&id)
            .expect("yeniden doğrula");
        assert_eq!(refreshed.validation_state, "valid");
        assert_eq!(refreshed.quote_state, "verified");
        assert!(fixture.db.promote_candidate(&id, &fixture.case_id).is_ok());
    }

    #[test]
    fn recording_invalid_payload_cannot_be_claimed_as_valid() {
        let fixture = setup(&[PAGE_ZERO]);
        let payload = serde_json::json!({"schemaVersion": 1, "title": ""}).to_string();
        let row = fixture
            .db
            .record_ai_candidate(&fixture.run_id, "event", &payload)
            .expect("kayıt");
        assert_eq!(row.validation_state, "invalid");
        assert_eq!(row.schema_state, "invalid");
        assert!(
            fixture
                .db
                .promote_candidate(&row.id, &fixture.case_id)
                .unwrap_err()
                .code()
                == "E_AI_SCHEMA"
        );
    }
}
