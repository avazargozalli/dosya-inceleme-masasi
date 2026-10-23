//! Emsal kütüphanesi ve atıf kontrolü.
//!
//! Kural: “bulunamadı” ile “uydurulmuş” birbirine dönüştürülmez. Bir kararın
//! künyesi eksikse `incomplete` işaretlenir; kullanıcı notu hukuki uygunluk
//! değerlendirmesidir, sistem hükmü değildir.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

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
    pub reason_codes: Vec<String>,
    pub created_at: String,
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
            "SELECT id,court,chamber,docket_no,decision_no,application_no,decision_date,title,source_url,user_tags,legal_issue,completeness,created_at
             FROM precedents WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_precedent).optional()?)
    }

    pub fn list_precedents(&self) -> Result<Vec<PrecedentRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,court,chamber,docket_no,decision_no,application_no,decision_date,title,source_url,user_tags,legal_issue,completeness,created_at
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
            "SELECT id,input_document_version_id,extracted_citation,candidate_precedent_id,identity_match,quote_match,meaning_review,reason_codes,created_at
             FROM citation_checks WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_check).optional()?)
    }

    pub fn list_citation_checks(
        &self,
        version_id: &str,
    ) -> Result<Vec<CitationCheckRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,input_document_version_id,extracted_citation,candidate_precedent_id,identity_match,quote_match,meaning_review,reason_codes,created_at
             FROM citation_checks WHERE input_document_version_id=?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([version_id], map_check)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
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
    })
}

fn map_check(row: &Row<'_>) -> rusqlite::Result<CitationCheckRow> {
    let reasons: String = row.get(7)?;
    Ok(CitationCheckRow {
        id: row.get(0)?,
        input_document_version_id: row.get(1)?,
        extracted_citation: row.get(2)?,
        candidate_precedent_id: row.get(3)?,
        identity_match: row.get(4)?,
        quote_match: row.get(5)?,
        meaning_review: row.get(6)?,
        reason_codes: serde_json::from_str(&reasons).unwrap_or_default(),
        created_at: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

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
}
