//! Karartma planları. Plan, çıktının hangi kaynak sürüme ve hangi render
//! sürümüne ait olduğunu sabitler; başarısız doğrulama planı `applied` yapmaz.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactionPlanRow {
    pub id: String,
    pub case_id: String,
    pub document_version_id: String,
    pub parse_run_id: Option<String>,
    pub render_hash: String,
    pub coordinate_space: String,
    pub plan_version: i64,
    pub state: String,
    pub reviewer: Option<String>,
    pub output_profile: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRedactionPlan {
    pub document_version_id: String,
    #[serde(default)]
    pub parse_run_id: Option<String>,
    pub render_hash: String,
    pub output_profile: String,
}

#[derive(Debug, Clone)]
pub struct RedactionRegionInput {
    pub page_index: i64,
    pub rect: [f64; 4],
    pub term_hash: String,
}

impl Database {
    pub fn create_redaction_plan(
        &self,
        case_id: &str,
        new: &NewRedactionPlan,
    ) -> Result<RedactionPlanRow, StorageError> {
        let version = self
            .get_version(&new.document_version_id)?
            .ok_or(StorageError::NotFound)?;
        if version.case_id != case_id {
            return Err(StorageError::NotFound);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO redaction_plans (id,case_id,document_version_id,parse_run_id,render_hash,coordinate_space,plan_version,state,output_profile,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,'pdf_page_points',1,'draft',?6,?7,?7)",
            rusqlite::params![
                id,
                case_id,
                new.document_version_id,
                new.parse_run_id,
                new.render_hash,
                new.output_profile,
                now
            ],
        )?;
        self.append_audit(
            "redaction_plan",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"caseId": case_id}),
        )?;
        self.get_redaction_plan(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn add_redaction_regions(
        &self,
        plan_id: &str,
        regions: &[RedactionRegionInput],
    ) -> Result<usize, StorageError> {
        if self.get_redaction_plan(plan_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let tx = self.conn().unchecked_transaction()?;
        for region in regions {
            tx.execute(
                "INSERT INTO redaction_regions (id,plan_id,page_index,quads,coordinate_space,category,detected_text_hash,user_decision)
                 VALUES (?1,?2,?3,?4,'pdf_page_points','user_term',?5,'redact')",
                rusqlite::params![
                    uuid::Uuid::now_v7().to_string(),
                    plan_id,
                    region.page_index,
                    serde_json::to_string(&vec![region.rect]).unwrap_or_else(|_| "[]".into()),
                    region.term_hash
                ],
            )?;
        }
        tx.execute(
            "UPDATE redaction_plans SET state='applied', updated_at=?1 WHERE id=?2",
            rusqlite::params![now_iso(), plan_id],
        )?;
        tx.commit()?;
        self.append_audit(
            "redaction_plan",
            Some(plan_id),
            "applied",
            "local-worker",
            &serde_json::json!({"regions": regions.len()}),
        )?;
        Ok(regions.len())
    }

    pub fn get_redaction_plan(&self, id: &str) -> Result<Option<RedactionPlanRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,document_version_id,parse_run_id,render_hash,coordinate_space,plan_version,state,reviewer,output_profile,created_at,updated_at
             FROM redaction_plans WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_plan).optional()?)
    }

    pub fn list_redaction_plans(
        &self,
        case_id: &str,
    ) -> Result<Vec<RedactionPlanRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,document_version_id,parse_run_id,render_hash,coordinate_space,plan_version,state,reviewer,output_profile,created_at,updated_at
             FROM redaction_plans WHERE case_id=?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([case_id], map_plan)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

fn map_plan(row: &Row<'_>) -> rusqlite::Result<RedactionPlanRow> {
    Ok(RedactionPlanRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        document_version_id: row.get(2)?,
        parse_run_id: row.get(3)?,
        render_hash: row.get(4)?,
        coordinate_space: row.get(5)?,
        plan_version: row.get(6)?,
        state: row.get(7)?,
        reviewer: row.get(8)?,
        output_profile: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn setup() -> (Database, String, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([41u8; 32])).expect("db");
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
        let blob = db.create_blob_record("st-redact", 10, "aad").expect("blob");
        let doc = db
            .create_document(&case.id, "Belge", "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &doc.id,
                &case.id,
                &blob,
                &"a".repeat(64),
                10,
                "application/pdf",
            )
            .expect("v");
        (db, case.id, version.id)
    }

    #[test]
    fn plan_lifecycle_and_region_count() {
        let (db, case_id, version_id) = setup();
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id,
                    parse_run_id: None,
                    render_hash: "b".repeat(64),
                    output_profile: "share".into(),
                },
            )
            .expect("plan");
        assert_eq!(plan.state, "draft");

        let regions = vec![
            RedactionRegionInput {
                page_index: 0,
                rect: [1.0, 2.0, 3.0, 4.0],
                term_hash: "h1".into(),
            },
            RedactionRegionInput {
                page_index: 1,
                rect: [5.0, 6.0, 7.0, 8.0],
                term_hash: "h2".into(),
            },
        ];
        assert_eq!(
            db.add_redaction_regions(&plan.id, &regions).expect("bölge"),
            2
        );
        let applied = db.get_redaction_plan(&plan.id).expect("get").expect("var");
        assert_eq!(applied.state, "applied");
        assert_eq!(db.list_redaction_plans(&case_id).expect("liste").len(), 1);
    }

    #[test]
    fn cross_case_version_rejected() {
        let (db, _case_id, version_id) = setup();
        let other = db
            .create_case(&NewCase {
                title: "Başka".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        let err = db
            .create_redaction_plan(
                &other.id,
                &NewRedactionPlan {
                    document_version_id: version_id,
                    parse_run_id: None,
                    render_hash: "c".repeat(64),
                    output_profile: "share".into(),
                },
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_NOT_FOUND");
    }
}
