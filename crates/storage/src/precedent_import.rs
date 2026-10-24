//! F12-T06 — emsal **toplu içe alma**.
//!
//! Akış: kullanıcı birden çok karar dosyası ve isteğe bağlı bir künye listesi
//! (CSV) seçer. Metin çıkarma belge worker'ında yapılır (çağıran katman);
//! bu modül **yalnız** künye/metin eşlemesini ve kasaya yazımı yönetir.
//!
//! Kurallar:
//! - Aynı künye + **aynı** metin: yeni sürüm açılmaz (`duplicate`).
//! - Aynı künye + **farklı** metin: sessiz birleştirme yok; yeni nüsha
//!   `identity_conflict=1` ile eklenir ve incelemeye sunulur (`conflict`).
//! - Farklı mahkeme/daire aynı numarayı kullansa bile ayrı emsaldir.
//! - Künye listesindeki bir dosya adı iki kez geçerse içe alma **durmaz**,
//!   liste reddedilir: hangi satırın geçerli olduğu tahmin edilmez.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{now_iso, Database, NewPrecedent, PrecedentVersionRow, StorageError};

/// Künye listesi üst sınırı. Daha büyük dosya bir avukatın elle hazırladığı
/// liste değildir; sessizce kırpmak yerine açıkça reddedilir.
pub const MANIFEST_MAX_BYTES: usize = 4 * 1024 * 1024;

/// Künye listesindeki bir satır. Yalnız `file` zorunludur; boş alanlar
/// varsayılan künyeden tamamlanır.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRow {
    /// Dosya adı (yol değil). Eşleşme büyük/küçük harf duyarsızdır.
    pub file: String,
    pub court: Option<String>,
    pub chamber: Option<String>,
    pub docket_no: Option<String>,
    pub decision_no: Option<String>,
    pub decision_date: Option<String>,
    pub title: Option<String>,
    pub source_url: Option<String>,
    pub user_tags: Vec<String>,
    pub legal_issue: Option<String>,
}

impl ManifestRow {
    /// Dosya adı ya da uzantısız gövde eşleşir; ikisi de yoksa `false`.
    fn matches(&self, file_name: &str) -> bool {
        let listed = self.file.trim();
        if listed.eq_ignore_ascii_case(file_name) {
            return true;
        }
        let stem = file_name
            .rsplit_once('.')
            .map_or(file_name, |(base, _)| base);
        listed.eq_ignore_ascii_case(stem)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Liste dosyasının SHA-256'sı; hangi listenin kullanıldığı denetimde izlenir.
    pub hash: String,
    pub rows: Vec<ManifestRow>,
    /// Yorumlanamayan ama içe almayı durdurmayan satırlar (ör. boş dosya adı).
    pub warnings: Vec<String>,
}

impl Manifest {
    #[must_use]
    pub fn row_for(&self, file_name: &str) -> Option<&ManifestRow> {
        self.rows.iter().find(|row| row.matches(file_name))
    }
}

fn manifest_error(message: impl Into<String>) -> StorageError {
    StorageError::Manifest(message.into())
}

fn header_key(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('\u{feff}')
        .trim()
        .to_lowercase()
}

/// RFC 4180'e yakın, bağımlılıksız CSV ayrıştırıcı: tırnaklı alan, kaçırılmış
/// tırnak (`""`), alan içinde satır sonu, CRLF. Fazlası (ayırıcı seçimi,
/// yorum satırı) desteklenmez ve sessizce yorumlanmaz.
fn parse_records(text: &str) -> Result<Vec<Vec<String>>, StorageError> {
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut record: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut quoted_field = false;
    let mut line = 1usize;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            }
            '"' => {
                // Tırnak yalnız alanın başında anlamlıdır; ortada ise düz karakter.
                if field.trim().is_empty() {
                    field.clear();
                    in_quotes = true;
                    quoted_field = true;
                } else {
                    field.push('"');
                }
            }
            ',' if !in_quotes => {
                record.push(std::mem::take(&mut field));
                quoted_field = false;
            }
            '\r' if !in_quotes => {}
            '\n' if !in_quotes => {
                record.push(std::mem::take(&mut field));
                quoted_field = false;
                let blank = record.len() == 1 && record[0].trim().is_empty();
                if blank {
                    record.clear();
                } else {
                    records.push(std::mem::take(&mut record));
                }
                line += 1;
            }
            _ => {
                if ch == '\n' {
                    line += 1;
                }
                field.push(ch);
            }
        }
    }
    if in_quotes {
        return Err(manifest_error(format!(
            "{line}. satırda kapanmamış tırnak var"
        )));
    }
    // `quoted_field` yalnız son alanın tırnaklı olduğunu izler; boş satır
    // ayıklaması için son alanın boşluk durumu yeterlidir.
    let _ = quoted_field;
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        if !(record.len() == 1 && record[0].trim().is_empty()) {
            records.push(record);
        }
    }
    Ok(records)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Column {
    File,
    Court,
    Chamber,
    Docket,
    Decision,
    Date,
    Title,
    SourceUrl,
    Tags,
    Issue,
}

fn column_for(header: &str) -> Option<Column> {
    match header {
        "dosya" | "dosya adı" | "dosya adi" | "file" | "file name" | "filename" => {
            Some(Column::File)
        }
        "mahkeme" | "court" => Some(Column::Court),
        "daire" | "kurul" | "chamber" => Some(Column::Chamber),
        "esas" | "esas no" | "docket" | "docket no" => Some(Column::Docket),
        "karar" | "karar no" | "decision" | "decision no" => Some(Column::Decision),
        "tarih" | "karar tarihi" | "date" | "decision date" => Some(Column::Date),
        "başlık" | "baslik" | "title" => Some(Column::Title),
        "kaynak" | "kaynak url" | "url" | "source url" | "source_url" => Some(Column::SourceUrl),
        "etiket" | "etiketler" | "tags" => Some(Column::Tags),
        "konu" | "hukuki konu" | "issue" | "legal issue" => Some(Column::Issue),
        _ => None,
    }
}

fn non_empty(value: Option<&String>) -> Option<String> {
    value
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// Künye listesini ayrıştırır. `dosya` sütunu zorunludur; bilinmeyen sütun
/// adı sessizce yok sayılmaz (yanlış başlıkla içe alma fark edilmesin diye).
pub fn parse_manifest(text: &str) -> Result<Manifest, StorageError> {
    let records = parse_records(text)?;
    let mut iter = records.into_iter();
    let header = iter
        .next()
        .ok_or_else(|| manifest_error("künye listesi boş"))?;
    let mut columns: Vec<(usize, Column)> = Vec::new();
    for (index, raw) in header.iter().enumerate() {
        let key = header_key(raw);
        if key.is_empty() {
            continue;
        }
        let column = column_for(&key).ok_or_else(|| {
            manifest_error(format!(
                "bilinmeyen sütun “{key}”; beklenen sütunlar: dosya, mahkeme, daire, esas, karar, tarih, başlık, kaynak url, etiketler, konu"
            ))
        })?;
        if columns.iter().any(|(_, existing)| *existing == column) {
            return Err(manifest_error(format!("sütun iki kez tanımlı: {key}")));
        }
        columns.push((index, column));
    }
    if !columns.iter().any(|(_, c)| *c == Column::File) {
        return Err(manifest_error("“dosya” sütunu zorunlu"));
    }

    let mut rows: Vec<ManifestRow> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    for (offset, record) in iter.enumerate() {
        let line_no = offset + 2;
        let cell = |column: Column| -> Option<&String> {
            columns
                .iter()
                .find(|(_, c)| *c == column)
                .and_then(|(index, _)| record.get(*index))
        };
        let Some(file) = non_empty(cell(Column::File)) else {
            warnings.push(format!("{line_no}. satırda dosya adı yok; atlandı"));
            continue;
        };
        if rows.iter().any(|row| row.matches(&file)) {
            return Err(manifest_error(format!(
                "“{file}” listede iki kez geçiyor; hangi satırın geçerli olduğu tahmin edilmez"
            )));
        }
        let tags = non_empty(cell(Column::Tags))
            .map(|value| {
                value
                    .split([',', ';'])
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        rows.push(ManifestRow {
            file,
            court: non_empty(cell(Column::Court)),
            chamber: non_empty(cell(Column::Chamber)),
            docket_no: non_empty(cell(Column::Docket)),
            decision_no: non_empty(cell(Column::Decision)),
            decision_date: non_empty(cell(Column::Date)),
            title: non_empty(cell(Column::Title)),
            source_url: non_empty(cell(Column::SourceUrl)),
            user_tags: tags,
            legal_issue: non_empty(cell(Column::Issue)),
        });
    }
    if rows.is_empty() {
        return Err(manifest_error("künye listesinde işlenecek satır yok"));
    }
    let hash = hex::encode(Sha256::digest(text.as_bytes()));
    Ok(Manifest {
        hash,
        rows,
        warnings,
    })
}

/// Tek nüshanın içe alma girdisi. `citation.court` boşsa varsayılanlar
/// uygulanmış olmalıdır; bu modül künye **tamamlamaz**.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentCopyInput {
    pub citation: NewPrecedent,
    pub text: String,
    /// `file` (varsayılan) | `document` | `paste`.
    #[serde(default)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub document_version_id: Option<String>,
}

/// `outcome`: `created` (yeni emsal), `duplicate` (aynı nüsha zaten var),
/// `conflict` (aynı künye, farklı metin — inceleme gerekir).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentCopyOutcome {
    pub outcome: String,
    pub precedent_id: String,
    pub version_id: Option<String>,
    pub body_hash: String,
    pub char_count: i64,
    pub conflict_with_version_id: Option<String>,
    pub needs_review: bool,
}

fn norm(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_lowercase)
        .unwrap_or_default()
}

fn norm_some(value: &Option<String>) -> String {
    norm(value.as_deref())
}

impl Database {
    /// Künye eşleşmesi: mahkeme + daire + esas/karar/başvuru numarası **birebir**
    /// (küçük harfe indirgenmiş). En az bir numara dolu olmalıdır; yalnız
    /// mahkeme adıyla eşleştirmek farklı kararları birleştirirdi.
    pub fn find_precedent_by_identity(
        &self,
        citation: &NewPrecedent,
    ) -> Result<Option<String>, StorageError> {
        let docket = norm_some(&citation.docket_no);
        let decision = norm_some(&citation.decision_no);
        let application = norm_some(&citation.application_no);
        if docket.is_empty() && decision.is_empty() && application.is_empty() {
            return Ok(None);
        }
        let mut stmt = self.conn().prepare(
            "SELECT id FROM precedents
             WHERE lower(trim(court))=?1
               AND coalesce(lower(trim(chamber)),'')=?2
               AND coalesce(lower(trim(docket_no)),'')=?3
               AND coalesce(lower(trim(decision_no)),'')=?4
               AND coalesce(lower(trim(application_no)),'')=?5
             ORDER BY created_at ASC, id ASC LIMIT 1",
        )?;
        Ok(stmt
            .query_row(
                rusqlite::params![
                    norm(Some(&citation.court)),
                    norm_some(&citation.chamber),
                    docket,
                    decision,
                    application
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()?)
    }

    /// Bir karar nüshasını kasaya yazar (F12-T06). Metin çıkarımı çağıranın
    /// işidir; burada ağ, dosya okuma veya worker çağrısı yoktur.
    pub fn import_precedent_copy(
        &self,
        input: &PrecedentCopyInput,
    ) -> Result<PrecedentCopyOutcome, StorageError> {
        let text = input.text.trim();
        if text.is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        if input.citation.court.trim().is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        let source_kind = input
            .source_kind
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("file");
        if !matches!(source_kind, "paste" | "document" | "file") {
            return Err(StorageError::InvalidSetting);
        }
        if let Some(version_id) = input
            .document_version_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            let count: i64 = self.conn().query_row(
                "SELECT count(*) FROM document_versions WHERE id=?1",
                [version_id],
                |r| r.get(0),
            )?;
            if count == 0 {
                return Err(StorageError::NotFound);
            }
        }
        let body_hash = hex::encode(Sha256::digest(text.as_bytes()));
        let char_count = i64::try_from(text.chars().count()).unwrap_or(i64::MAX);

        // Aynı nüsha (aynı hash) daha önce yazılmışsa tekrar yazılmaz.
        let existing_precedent = self.find_precedent_by_identity(&input.citation)?;
        if let Some(precedent_id) = existing_precedent.as_deref() {
            let duplicate: Option<String> = self
                .conn()
                .query_row(
                    "SELECT id FROM precedent_versions
                     WHERE precedent_id=?1 AND body_hash=?2
                     ORDER BY acquired_at DESC, id DESC LIMIT 1",
                    rusqlite::params![precedent_id, body_hash],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(version_id) = duplicate {
                self.append_audit(
                    "precedent",
                    Some(precedent_id),
                    "import_duplicate",
                    "local-user",
                    &serde_json::json!({"bodyHash": body_hash, "charCount": char_count}),
                )?;
                return Ok(PrecedentCopyOutcome {
                    outcome: "duplicate".into(),
                    precedent_id: precedent_id.to_string(),
                    version_id: Some(version_id),
                    body_hash,
                    char_count,
                    conflict_with_version_id: None,
                    needs_review: false,
                });
            }
        }

        let latest_version: Option<String> = match existing_precedent.as_deref() {
            Some(precedent_id) => self
                .conn()
                .query_row(
                    "SELECT id FROM precedent_versions WHERE precedent_id=?1
                     ORDER BY acquired_at DESC, id DESC LIMIT 1",
                    [precedent_id],
                    |row| row.get(0),
                )
                .optional()?,
            None => None,
        };

        let mut outcome: Option<PrecedentCopyOutcome> = None;
        self.write_atomically(|| {
            let precedent_id = match existing_precedent.clone() {
                Some(id) => id,
                None => self.create_precedent(&input.citation)?.id,
            };
            let conflict_with = latest_version.clone();
            let version = self.insert_precedent_version(
                &precedent_id,
                text,
                &body_hash,
                char_count,
                source_kind,
                input
                    .document_version_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty()),
                conflict_with.as_deref(),
            )?;
            outcome = Some(PrecedentCopyOutcome {
                outcome: if conflict_with.is_some() {
                    "conflict".into()
                } else {
                    "created".into()
                },
                precedent_id,
                version_id: Some(version.id),
                body_hash: body_hash.clone(),
                char_count,
                conflict_with_version_id: conflict_with,
                needs_review: false,
            });
            Ok(())
        })?;
        let mut value = outcome.ok_or(StorageError::NotFound)?;
        // `needs_review` yalnız gerçek çakışmada true olur (yeni emsalde değil).
        value.needs_review = value.outcome == "conflict";
        Ok(value)
    }

    /// Sürüm satırı yazımı. `add_precedent_text` ile toplu içe alma **aynı**
    /// yolu kullanır; `conflict_with` yalnız toplu içe almada doludur.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn insert_precedent_version(
        &self,
        precedent_id: &str,
        text: &str,
        body_hash: &str,
        char_count: i64,
        source_kind: &str,
        document_version_id: Option<&str>,
        conflict_with: Option<&str>,
    ) -> Result<PrecedentVersionRow, StorageError> {
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        let conflict = i64::from(conflict_with.is_some());
        self.conn().execute(
            "INSERT INTO precedent_versions
               (id,precedent_id,document_version_id,acquired_at,source_kind,body_hash,
                verification_state,full_text,text_char_count,identity_conflict,conflict_with_version_id)
             VALUES (?1,?2,?3,?4,?5,?6,'text_verified',?7,?8,?9,?10)",
            rusqlite::params![
                id,
                precedent_id,
                document_version_id,
                now,
                source_kind,
                body_hash,
                text,
                char_count,
                conflict,
                conflict_with
            ],
        )?;
        self.append_audit(
            "precedent_version",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({
                "sourceKind": source_kind,
                "charCount": char_count,
                "bodyHash": body_hash,
                "identityConflict": conflict_with.is_some(),
                "conflictWithVersionId": conflict_with,
            }),
        )?;
        self.get_precedent_version(&id)?
            .ok_or(StorageError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseKey;

    fn db() -> Database {
        Database::open_in_memory(&DatabaseKey::new([21u8; 32])).expect("db")
    }

    fn citation(court: &str, docket: Option<&str>, decision: Option<&str>) -> NewPrecedent {
        NewPrecedent {
            court: court.into(),
            chamber: Some("1. Hukuk Dairesi".into()),
            docket_no: docket.map(str::to_string),
            decision_no: decision.map(str::to_string),
            application_no: None,
            decision_date: Some("2025-05-02".into()),
            title: Some("Kira alacağı".into()),
            source_url: None,
            user_tags: vec![],
            legal_issue: None,
        }
    }

    fn copy(
        court: &str,
        docket: Option<&str>,
        decision: Option<&str>,
        text: &str,
    ) -> PrecedentCopyInput {
        PrecedentCopyInput {
            citation: citation(court, docket, decision),
            text: text.into(),
            source_kind: Some("file".into()),
            document_version_id: None,
        }
    }

    #[test]
    fn manifest_parses_quoted_fields_and_turkish_headers() {
        let csv = "\u{feff}Dosya,Mahkeme,Daire,Esas,Karar,Tarih,Başlık,Etiketler\n\
                   \"2024-123 karar.pdf\",Yargıtay,1. Hukuk Dairesi,2024/123,2025/456,2025-05-02,\"Kira, \"\"temerrüt\"\"\",kira; temerrüt\n";
        let manifest = parse_manifest(csv).expect("liste");
        assert_eq!(manifest.rows.len(), 1);
        let row = manifest.rows.first().expect("satır");
        assert_eq!(row.file, "2024-123 karar.pdf");
        assert_eq!(row.court.as_deref(), Some("Yargıtay"));
        assert_eq!(row.docket_no.as_deref(), Some("2024/123"));
        // Tırnak içindeki virgül alanı bölmez; kaçırılmış tırnak korunur.
        assert_eq!(row.title.as_deref(), Some("Kira, \"temerrüt\""));
        assert_eq!(
            row.user_tags,
            vec!["kira".to_string(), "temerrüt".to_string()]
        );
        assert!(!manifest.hash.is_empty());
        assert!(manifest.warnings.is_empty());
    }

    #[test]
    fn manifest_rejects_bad_headers_and_duplicate_files() {
        let unknown =
            parse_manifest("dosya,mahkeme,okul\nx.pdf,Yargıtay,abc\n").expect_err("sütun");
        assert_eq!(unknown.code(), "E_MANIFEST");

        let twice =
            parse_manifest("dosya,mahkeme\na.pdf,Yargıtay\nA.PDF,Hukuk\n").expect_err("çift kayıt");
        assert_eq!(twice.code(), "E_MANIFEST");

        assert_eq!(
            parse_manifest("mahkeme\na, Yargıtay\n")
                .expect_err("dosya sütunu")
                .code(),
            "E_MANIFEST"
        );
    }

    #[test]
    fn import_creates_precedent_with_hashed_text() {
        let db = db();
        let result = db
            .import_precedent_copy(&copy(
                "Yargıtay",
                Some("2024/123"),
                Some("2025/456"),
                "Karar metni.",
            ))
            .expect("içe alındı");
        assert_eq!(result.outcome, "created");
        assert!(!result.needs_review);
        assert_eq!(result.char_count, 12);
        assert_eq!(
            result.body_hash,
            hex::encode(Sha256::digest(b"Karar metni."))
        );
        let versions = db.precedent_versions(&result.precedent_id).expect("sürüm");
        assert_eq!(versions.len(), 1);
        assert!(versions[0].has_text);
        assert!(!versions[0].identity_conflict);
        assert!(versions[0].conflict_with_version_id.is_none());
    }

    #[test]
    fn same_copy_is_duplicate_and_not_written_twice() {
        let db = db();
        let input = copy(
            "Yargıtay",
            Some("2024/123"),
            Some("2025/456"),
            "Aynı metin.",
        );
        let first = db.import_precedent_copy(&input).expect("ilk");
        let second = db.import_precedent_copy(&input).expect("ikinci");
        assert_eq!(second.outcome, "duplicate");
        assert_eq!(second.precedent_id, first.precedent_id);
        assert_eq!(second.version_id, first.version_id);
        assert_eq!(
            db.precedent_versions(&first.precedent_id)
                .expect("sürüm")
                .len(),
            1
        );
        assert_eq!(db.list_precedents().expect("liste").len(), 1);
    }

    #[test]
    fn same_identity_different_text_is_conflict_for_review() {
        let db = db();
        let first = db
            .import_precedent_copy(&copy(
                "Yargıtay",
                Some("2024/123"),
                Some("2025/456"),
                "Birinci nüsha.",
            ))
            .expect("ilk");
        let second = db
            .import_precedent_copy(&copy(
                "Yargıtay",
                Some("2024/123"),
                Some("2025/456"),
                "İkinci nüsha farklı.",
            ))
            .expect("ikinci");
        assert_eq!(second.outcome, "conflict");
        assert!(second.needs_review);
        assert_eq!(second.precedent_id, first.precedent_id);
        assert_eq!(second.conflict_with_version_id, first.version_id);
        // Sessiz birleştirme yok: iki nüsha ayrı sürüm olarak durur.
        let versions = db.precedent_versions(&first.precedent_id).expect("sürüm");
        assert_eq!(versions.len(), 2);
        let newest = versions.first().expect("en yeni");
        assert!(newest.identity_conflict);
        assert_eq!(newest.conflict_with_version_id, first.version_id);
    }

    #[test]
    fn different_court_same_number_stays_separate() {
        let db = db();
        let yargitay = db
            .import_precedent_copy(&copy(
                "Yargıtay",
                Some("2024/123"),
                Some("2025/456"),
                "Yargıtay metni.",
            ))
            .expect("yargıtay");
        let danistay = db
            .import_precedent_copy(&copy(
                "Danıştay",
                Some("2024/123"),
                Some("2025/456"),
                "Danıştay metni.",
            ))
            .expect("danıştay");
        assert_eq!(danistay.outcome, "created");
        assert_ne!(danistay.precedent_id, yargitay.precedent_id);
        assert_eq!(db.list_precedents().expect("liste").len(), 2);
    }

    #[test]
    fn copies_without_identifier_never_merge() {
        let db = db();
        let first = db
            .import_precedent_copy(&copy("Yargıtay", None, None, "Künyesiz nüsha."))
            .expect("ilk");
        let second = db
            .import_precedent_copy(&copy("Yargıtay", None, None, "Künyesiz nüsha."))
            .expect("ikinci");
        // Numara yoksa aynı metin bile birleştirilmez: hangi karar olduğu bilinmiyor.
        assert_eq!(second.outcome, "created");
        assert_ne!(second.precedent_id, first.precedent_id);
    }

    #[test]
    fn empty_text_is_rejected() {
        let db = db();
        assert_eq!(
            db.import_precedent_copy(&copy("Yargıtay", Some("2024/1"), None, "   "))
                .expect_err("boş")
                .code(),
            "E_DB_SETTING"
        );
    }

    #[test]
    fn audit_records_hash_not_decision_text() {
        let db = db();
        let secret = "Müvekkilin adı geçen gizli karar metni.";
        let result = db
            .import_precedent_copy(&copy("Yargıtay", Some("2024/9"), Some("2025/9"), secret))
            .expect("içe alındı");
        let payload: String = db
            .conn()
            .query_row(
                "SELECT metadata_allowlist FROM audit_events WHERE entity_id=?1",
                [result.version_id.clone().unwrap_or_default()],
                |row| row.get(0),
            )
            .expect("denetim");
        assert!(payload.contains(&result.body_hash));
        assert!(!payload.contains("Müvekkilin"));
    }
}
