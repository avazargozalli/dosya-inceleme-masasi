//! AI çalıştırma ve aktarım sicili.
//!
//! Değişmez kural: geçerli bir aktarım onayı (`transfer_consents`) olmadan
//! `ai_runs` başlatılamaz. Onay sağlayıcı, dosya ve belge listesine bağlıdır;
//! iptal edilirse yeni çalıştırma açılamaz. Gönderilen içerik `ai_context_items`
//! ve `transfer_receipts` ile denetlenebilir.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRunRow {
    pub id: String,
    pub case_id: String,
    pub task_type: String,
    pub provider_id: Option<String>,
    pub model_snapshot: String,
    pub input_manifest_hash: String,
    pub prompt_version: String,
    pub output_schema_version: String,
    pub consent_id: Option<String>,
    pub status: String,
    pub usage_report: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAiRun {
    pub task_type: String,
    pub provider_id: String,
    pub model_snapshot: String,
    pub input_manifest_hash: String,
    pub prompt_version: String,
    pub output_schema_version: String,
    pub consent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferConsentRow {
    pub id: String,
    pub provider_snapshot: String,
    pub case_scope: String,
    pub purpose: String,
    pub selected_document_ids: Vec<String>,
    pub expires_at: Option<String>,
    pub consent_text_version: String,
    pub approved_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTransferConsent {
    pub provider_snapshot: String,
    pub case_scope: String,
    pub purpose: String,
    #[serde(default)]
    pub selected_document_ids: Vec<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    pub consent_text_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContextItemRow {
    pub id: String,
    pub ai_run_id: String,
    pub document_version_id: String,
    pub source_anchor_id: Option<String>,
    pub inclusion_reason: String,
    pub bytes_sent: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAiContextItem {
    pub document_version_id: String,
    #[serde(default)]
    pub source_anchor_id: Option<String>,
    pub inclusion_reason: String,
    pub bytes_sent: i64,
}

impl Database {
    pub fn create_transfer_consent(
        &self,
        new: &NewTransferConsent,
    ) -> Result<TransferConsentRow, StorageError> {
        if new.provider_snapshot.trim().is_empty()
            || new.case_scope.trim().is_empty()
            || new.purpose.trim().is_empty()
        {
            return Err(StorageError::InvalidSetting);
        }
        let exists: i64 = self.conn().query_row(
            "SELECT count(*) FROM cases WHERE id=?1 AND deleted_at IS NULL",
            [&new.case_scope],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(StorageError::NotFound);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO transfer_consents (id,provider_snapshot,case_scope,purpose,selected_document_ids,expires_at,consent_text_version,approved_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            rusqlite::params![
                id,
                new.provider_snapshot.trim(),
                new.case_scope,
                new.purpose.trim(),
                serde_json::to_string(&new.selected_document_ids).unwrap_or_else(|_| "[]".into()),
                new.expires_at,
                new.consent_text_version,
                now
            ],
        )?;
        self.append_audit(
            "transfer_consent",
            Some(&id),
            "approve",
            "local-user",
            &serde_json::json!({"provider": new.provider_snapshot, "documents": new.selected_document_ids.len()}),
        )?;
        self.get_transfer_consent(&id)?
            .ok_or(StorageError::NotFound)
    }

    pub fn get_transfer_consent(
        &self,
        id: &str,
    ) -> Result<Option<TransferConsentRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,provider_snapshot,case_scope,purpose,selected_document_ids,expires_at,consent_text_version,approved_at,revoked_at
             FROM transfer_consents WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_consent).optional()?)
    }

    pub fn revoke_transfer_consent(&self, id: &str) -> Result<(), StorageError> {
        let changed = self.conn().execute(
            "UPDATE transfer_consents SET revoked_at=?1 WHERE id=?2 AND revoked_at IS NULL",
            rusqlite::params![now_iso(), id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit(
            "transfer_consent",
            Some(id),
            "revoke",
            "local-user",
            &serde_json::json!({}),
        )?;
        Ok(())
    }

    /// Onay geçerli mi? İptal edilmiş veya süresi dolmuş onay yeni çalıştırma açamaz.
    pub fn consent_is_active(&self, id: &str) -> Result<bool, StorageError> {
        let consent = self
            .get_transfer_consent(id)?
            .ok_or(StorageError::NotFound)?;
        if consent.revoked_at.is_some() {
            return Ok(false);
        }
        if let Some(expires) = consent.expires_at {
            if expires <= now_iso() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn start_ai_run(&self, case_id: &str, new: &NewAiRun) -> Result<AiRunRow, StorageError> {
        if !self.consent_is_active(&new.consent_id)? {
            return Err(StorageError::AiConsent);
        }
        let consent = self
            .get_transfer_consent(&new.consent_id)?
            .ok_or(StorageError::NotFound)?;
        if consent.case_scope != case_id {
            return Err(StorageError::AiConsent);
        }
        let provider_exists: i64 = self.conn().query_row(
            "SELECT count(*) FROM provider_profiles WHERE id=?1",
            [&new.provider_id],
            |r| r.get(0),
        )?;
        if provider_exists == 0 {
            return Err(StorageError::AiProvider);
        }
        let id = uuid::Uuid::now_v7().to_string();
        self.conn().execute(
            "INSERT INTO ai_runs (id,case_id,task_type,provider_id,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,consent_id,status,usage_report,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'running','{\"billing\":\"unknown\"}',?10)",
            rusqlite::params![
                id,
                case_id,
                new.task_type,
                new.provider_id,
                new.model_snapshot,
                new.input_manifest_hash,
                new.prompt_version,
                new.output_schema_version,
                new.consent_id,
                now_iso()
            ],
        )?;
        self.append_audit(
            "ai_run",
            Some(&id),
            "start",
            "local-user",
            &serde_json::json!({"caseId": case_id}),
        )?;
        self.get_ai_run(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_ai_run(&self, id: &str) -> Result<Option<AiRunRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,task_type,provider_id,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,consent_id,status,usage_report,created_at,completed_at
             FROM ai_runs WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_run).optional()?)
    }

    pub fn list_ai_runs(&self, case_id: &str) -> Result<Vec<AiRunRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,task_type,provider_id,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,consent_id,status,usage_report,created_at,completed_at
             FROM ai_runs WHERE case_id=?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([case_id], map_run)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Gönderilen içeriğin denetlenebilir sicili. `bytes_sent` gerçek dışa giden
    /// bayt sayısıdır; tahmini değer yazılmaz.
    pub fn record_context_items(
        &self,
        run_id: &str,
        items: &[NewAiContextItem],
    ) -> Result<usize, StorageError> {
        if self.get_ai_run(run_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let tx = self.conn().unchecked_transaction()?;
        for item in items {
            if item.bytes_sent < 0 {
                return Err(StorageError::InvalidSetting);
            }
            tx.execute(
                "INSERT INTO ai_context_items (id,ai_run_id,document_version_id,source_anchor_id,segment_ids,inclusion_reason,bytes_sent)
                 VALUES (?1,?2,?3,?4,'[]',?5,?6)",
                rusqlite::params![
                    uuid::Uuid::now_v7().to_string(),
                    run_id,
                    item.document_version_id,
                    item.source_anchor_id,
                    item.inclusion_reason,
                    item.bytes_sent
                ],
            )?;
        }
        tx.commit()?;
        Ok(items.len())
    }

    pub fn list_context_items(&self, run_id: &str) -> Result<Vec<AiContextItemRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,ai_run_id,document_version_id,source_anchor_id,inclusion_reason,bytes_sent
             FROM ai_context_items WHERE ai_run_id=?1",
        )?;
        let rows = stmt.query_map([run_id], |row| {
            Ok(AiContextItemRow {
                id: row.get(0)?,
                ai_run_id: row.get(1)?,
                document_version_id: row.get(2)?,
                source_anchor_id: row.get(3)?,
                inclusion_reason: row.get(4)?,
                bytes_sent: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn finish_ai_run(
        &self,
        run_id: &str,
        status: &str,
        usage_report: &str,
    ) -> Result<(), StorageError> {
        if !matches!(status, "succeeded" | "failed" | "cancelled" | "partial") {
            return Err(StorageError::InvalidSetting);
        }
        let changed = self.conn().execute(
            "UPDATE ai_runs SET status=?1, usage_report=?2, completed_at=?3 WHERE id=?4",
            rusqlite::params![status, usage_report, now_iso(), run_id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit(
            "ai_run",
            Some(run_id),
            status,
            "local-system",
            &serde_json::json!({}),
        )?;
        Ok(())
    }

    pub fn record_transfer_receipt(
        &self,
        run_id: &str,
        destination_label: &str,
        outbound_hash: &str,
        item_count: i64,
        byte_count: i64,
    ) -> Result<String, StorageError> {
        if self.get_ai_run(run_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let id = uuid::Uuid::now_v7().to_string();
        self.conn().execute(
            "INSERT INTO transfer_receipts (id,ai_run_id,destination_label,outbound_hash,item_count,byte_count,status,dispatched_at)
             VALUES (?1,?2,?3,?4,?5,?6,'dispatched',?7)",
            rusqlite::params![id, run_id, destination_label, outbound_hash, item_count, byte_count, now_iso()],
        )?;
        self.append_audit(
            "transfer_receipt",
            Some(&id),
            "dispatch",
            "local-system",
            &serde_json::json!({"destination": destination_label}),
        )?;
        Ok(id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfileRow {
    pub id: String,
    pub provider_type: String,
    pub adapter_id: String,
    pub execution_environment: String,
    pub policy_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCandidateRow {
    pub id: String,
    pub ai_run_id: String,
    pub candidate_type: String,
    pub structured_payload: String,
    pub validation_state: String,
    pub validation_errors: String,
}

/// Kapsamlılık raporu: hangi belge gerçekten ayrıştırıldı, hangi sayfa boş,
/// nerede OCR kullanıldı. "İncelenmedi" ile "temiz" karıştırılmaz.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageReport {
    pub documents: i64,
    pub versions: i64,
    pub parse_runs: i64,
    pub pages: i64,
    pub ocr_pages: i64,
    pub segments: i64,
    pub empty_pages: i64,
    pub unparsed_documents: i64,
    pub failed_parse_runs: i64,
}

impl Database {
    pub fn record_ai_candidate(
        &self,
        run_id: &str,
        candidate_type: &str,
        structured_payload: &str,
        validation_state: &str,
        validation_errors: &str,
    ) -> Result<AiCandidateRow, StorageError> {
        if !matches!(validation_state, "valid" | "invalid" | "partial") {
            return Err(StorageError::InvalidSetting);
        }
        if self.get_ai_run(run_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let id = uuid::Uuid::now_v7().to_string();
        self.conn().execute(
            "INSERT INTO ai_candidates (id,ai_run_id,type,structured_payload,validation_state,validation_errors)
             VALUES (?1,?2,?3,?4,?5,?6)",
            rusqlite::params![id, run_id, candidate_type, structured_payload, validation_state, validation_errors],
        )?;
        Ok(AiCandidateRow {
            id,
            ai_run_id: run_id.to_string(),
            candidate_type: candidate_type.to_string(),
            structured_payload: structured_payload.to_string(),
            validation_state: validation_state.to_string(),
            validation_errors: validation_errors.to_string(),
        })
    }

    pub fn list_ai_candidates(&self, run_id: &str) -> Result<Vec<AiCandidateRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,ai_run_id,type,structured_payload,validation_state,validation_errors
             FROM ai_candidates WHERE ai_run_id=?1 ORDER BY rowid",
        )?;
        let rows = stmt.query_map([run_id], |row| {
            Ok(AiCandidateRow {
                id: row.get(0)?,
                ai_run_id: row.get(1)?,
                candidate_type: row.get(2)?,
                structured_payload: row.get(3)?,
                validation_state: row.get(4)?,
                validation_errors: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Adayı inceleme kuyruğuna taşır. Aday doğrudan hukuki kayda dönüşmez;
    /// avukat kabul edene kadar `review_items` içinde `pending` kalır.
    pub fn promote_candidate(
        &self,
        candidate_id: &str,
        case_id: &str,
    ) -> Result<String, StorageError> {
        let row = self
            .conn()
            .query_row(
                "SELECT c.type,c.structured_payload,c.ai_run_id,r.case_id
                 FROM ai_candidates c JOIN ai_runs r ON r.id=c.ai_run_id WHERE c.id=?1",
                [candidate_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        let (kind, payload, run_id, run_case) = row.ok_or(StorageError::NotFound)?;
        if run_case != case_id {
            return Err(StorageError::NotFound);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO review_items (id,case_id,item_kind,payload_schema_version,payload,source_anchor_ids,ai_run_id,state,created_at,updated_at,row_version)
             VALUES (?1,?2,?3,1,?4,'[]',?5,'pending',?6,?6,1)",
            rusqlite::params![id, case_id, kind, payload, run_id, now],
        )?;
        self.append_audit(
            "review_item",
            Some(&id),
            "promote",
            "local-user",
            &serde_json::json!({"candidateId": candidate_id}),
        )?;
        Ok(id)
    }

    pub fn list_provider_profiles(&self) -> Result<Vec<ProviderProfileRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,provider_type,adapter_id,execution_environment,policy_version
             FROM provider_profiles ORDER BY provider_type, adapter_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ProviderProfileRow {
                id: row.get(0)?,
                provider_type: row.get(1)?,
                adapter_id: row.get(2)?,
                execution_environment: row.get(3)?,
                policy_version: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// `off` profili yoksa oluşturur; AI varsayılan olarak kapalıdır.
    pub fn ensure_off_provider(&self) -> Result<ProviderProfileRow, StorageError> {
        if let Some(row) = self
            .list_provider_profiles()?
            .into_iter()
            .find(|p| p.provider_type == "off")
        {
            return Ok(row);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO provider_profiles (id,provider_type,adapter_id,execution_environment,policy_version,created_at,updated_at)
             VALUES (?1,'off','off','local','1',?2,?2)",
            rusqlite::params![id, now],
        )?;
        Ok(ProviderProfileRow {
            id,
            provider_type: "off".into(),
            adapter_id: "off".into(),
            execution_environment: "local".into(),
            policy_version: "1".into(),
        })
    }

    pub fn coverage_report(&self, case_id: &str) -> Result<CoverageReport, StorageError> {
        let q = |sql: &str| -> Result<i64, StorageError> {
            Ok(self.conn().query_row(sql, [case_id], |r| r.get(0))?)
        };
        Ok(CoverageReport {
            documents: q("SELECT count(*) FROM documents WHERE case_id=?1 AND deleted_at IS NULL")?,
            versions: q("SELECT count(*) FROM document_versions WHERE case_id=?1")?,
            parse_runs: q("SELECT count(*) FROM parse_runs WHERE case_id=?1")?,
            pages: q("SELECT count(*) FROM pages p JOIN parse_runs r ON r.id=p.parse_run_id WHERE r.case_id=?1")?,
            ocr_pages: q("SELECT count(*) FROM pages p JOIN parse_runs r ON r.id=p.parse_run_id WHERE r.case_id=?1 AND p.ocr_used=1")?,
            segments: q("SELECT count(*) FROM segments s JOIN pages p ON p.id=s.page_id JOIN parse_runs r ON r.id=p.parse_run_id WHERE r.case_id=?1")?,
            empty_pages: q("SELECT count(*) FROM pages p JOIN parse_runs r ON r.id=p.parse_run_id LEFT JOIN segments s ON s.page_id=p.id WHERE r.case_id=?1 AND s.id IS NULL")?,
            unparsed_documents: q("SELECT count(*) FROM documents d WHERE d.case_id=?1 AND d.deleted_at IS NULL AND NOT EXISTS (SELECT 1 FROM parse_runs r WHERE r.document_id=d.id)")?,
            failed_parse_runs: q("SELECT count(*) FROM parse_runs WHERE case_id=?1 AND status='failed'")?,
        })
    }
}

fn map_run(row: &Row<'_>) -> rusqlite::Result<AiRunRow> {
    Ok(AiRunRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        task_type: row.get(2)?,
        provider_id: row.get(3)?,
        model_snapshot: row.get(4)?,
        input_manifest_hash: row.get(5)?,
        prompt_version: row.get(6)?,
        output_schema_version: row.get(7)?,
        consent_id: row.get(8)?,
        status: row.get(9)?,
        usage_report: row.get(10)?,
        created_at: row.get(11)?,
        completed_at: row.get(12)?,
    })
}

fn map_consent(row: &Row<'_>) -> rusqlite::Result<TransferConsentRow> {
    let ids: String = row.get(4)?;
    Ok(TransferConsentRow {
        id: row.get(0)?,
        provider_snapshot: row.get(1)?,
        case_scope: row.get(2)?,
        purpose: row.get(3)?,
        selected_document_ids: serde_json::from_str(&ids).unwrap_or_default(),
        expires_at: row.get(5)?,
        consent_text_version: row.get(6)?,
        approved_at: row.get(7)?,
        revoked_at: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn setup() -> (Database, String, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([47u8; 32])).expect("db");
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
        db.conn()
            .execute(
                "INSERT INTO provider_profiles (id,provider_type,adapter_id,policy_version,created_at,updated_at)
                 VALUES ('p1','cli','claude-code','1','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                [],
            )
            .expect("provider");
        (db, case.id, "p1".to_string())
    }

    fn consent(db: &Database, case_id: &str) -> TransferConsentRow {
        db.create_transfer_consent(&NewTransferConsent {
            provider_snapshot: "claude-code@2.1".into(),
            case_scope: case_id.into(),
            purpose: "ozet".into(),
            selected_document_ids: vec!["d1".into()],
            expires_at: None,
            consent_text_version: "1".into(),
        })
        .expect("onay")
    }

    fn run(case_id: &str, consent_id: &str) -> NewAiRun {
        NewAiRun {
            task_type: "summarize".into(),
            provider_id: "p1".into(),
            model_snapshot: "claude-sonnet".into(),
            input_manifest_hash: "h".into(),
            prompt_version: "p1".into(),
            output_schema_version: "1".into(),
            consent_id: consent_id.into(),
        }
    }

    #[test]
    fn run_requires_active_consent() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let started = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        assert_eq!(started.status, "running");

        db.revoke_transfer_consent(&c.id).expect("iptal");
        let err = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .unwrap_err();
        assert_eq!(err.code(), "E_AI_CONSENT");
    }

    #[test]
    fn expired_consent_blocks_run() {
        let (db, case_id, _) = setup();
        let mut c = consent(&db, &case_id);
        c.expires_at = Some("2000-01-01T00:00:00Z".into());
        let row = db
            .create_transfer_consent(&NewTransferConsent {
                provider_snapshot: c.provider_snapshot.clone(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![],
                expires_at: c.expires_at.clone(),
                consent_text_version: "1".into(),
            })
            .expect("onay");
        assert_eq!(
            db.start_ai_run(&case_id, &run(&case_id, &row.id))
                .unwrap_err()
                .code(),
            "E_AI_CONSENT"
        );
    }

    #[test]
    fn consent_is_case_scoped() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
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
        assert_eq!(
            db.start_ai_run(&other.id, &run(&other.id, &c.id))
                .unwrap_err()
                .code(),
            "E_AI_CONSENT"
        );
    }

    #[test]
    fn unknown_provider_rejected() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let mut r = run(&case_id, &c.id);
        r.provider_id = "yok".into();
        assert_eq!(
            db.start_ai_run(&case_id, &r).unwrap_err().code(),
            "E_AI_PROVIDER"
        );
    }

    #[test]
    fn candidate_promotes_to_pending_review_item() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let candidate = db
            .record_ai_candidate(
                &r.id,
                "event_candidate",
                "{\"title\":\"Ödeme\"}",
                "valid",
                "[]",
            )
            .expect("aday");
        assert_eq!(db.list_ai_candidates(&r.id).expect("liste").len(), 1);
        let review_id = db
            .promote_candidate(&candidate.id, &case_id)
            .expect("terfi");
        let item = db.get_review_item(&review_id).expect("get").expect("var");
        assert_eq!(item.state, "pending");
        assert_eq!(item.item_kind, "event_candidate");
    }

    #[test]
    fn coverage_report_counts_empty_pages() {
        let (db, case_id, _) = setup();
        let report = db.coverage_report(&case_id).expect("rapor");
        assert_eq!(report.documents, 0);
        assert_eq!(report.empty_pages, 0);
    }

    #[test]
    fn context_items_and_receipt_are_recorded() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let items = vec![NewAiContextItem {
            document_version_id: "v1".into(),
            source_anchor_id: None,
            inclusion_reason: "seçili paragraf".into(),
            bytes_sent: 42,
        }];
        assert_eq!(db.record_context_items(&r.id, &items).expect("bağlam"), 1);
        assert_eq!(db.list_context_items(&r.id).expect("liste").len(), 1);
        db.record_transfer_receipt(&r.id, "claude-code", &"a".repeat(64), 1, 42)
            .expect("makbuz");
        db.finish_ai_run(&r.id, "succeeded", "{\"billing\":\"unknown\"}")
            .expect("bitir");
        let finished = db.get_ai_run(&r.id).expect("get").expect("var");
        assert_eq!(finished.status, "succeeded");
    }
}
