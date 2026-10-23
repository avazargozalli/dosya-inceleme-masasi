//! İddia–delil matrisi ve inceleme kuyruğu.
//!
//! İki kural: (1) iddianın doğruluğu sistemce kesinleştirilmez, yalnız ilişki
//! kaydedilir; (2) inceleme öğesi kullanıcı kararı olmadan kalıcı kayda dönüşmez.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{now_iso, Database, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimRow {
    pub id: String,
    pub case_id: String,
    pub claimant_actor_id: Option<String>,
    pub claim_text: String,
    pub claim_kind: String,
    pub status: String,
    pub source_anchor_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub row_version: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewClaim {
    #[serde(default)]
    pub claimant_actor_id: Option<String>,
    pub claim_text: String,
    pub claim_kind: String,
    #[serde(default)]
    pub source_anchor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceLinkRow {
    pub id: String,
    pub case_id: String,
    pub claim_id: String,
    pub source_anchor_id: String,
    pub relation: String,
    pub rationale: String,
    pub review_state: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewEvidenceLink {
    pub claim_id: String,
    pub source_anchor_id: String,
    pub relation: String,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimRelationRow {
    pub id: String,
    pub case_id: String,
    pub claim_id: String,
    pub related_claim_id: String,
    pub relation: String,
    pub rationale: String,
    pub created_at: String,
}

/// İki iddia arasındaki ilişki. Tür **zorunlu** ve açık seçilir; boş bırakılırsa
/// kayıt oluşmaz (F08-T05). Yanıt ilişkisi belgeye değil iddiaya bağlanır.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewClaimRelation {
    pub related_claim_id: String,
    pub relation: String,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItemRow {
    pub id: String,
    pub case_id: String,
    pub item_kind: String,
    pub payload_schema_version: i64,
    pub payload: Value,
    pub source_anchor_ids: Vec<String>,
    pub ai_run_id: Option<String>,
    pub state: String,
    pub target_entity_id: Option<String>,
    pub superseded_by_item_id: Option<String>,
    pub superseded_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub row_version: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewReviewItem {
    pub item_kind: String,
    pub payload: Value,
    pub source_anchor_ids: Vec<String>,
    #[serde(default)]
    pub ai_run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDecisionRow {
    pub id: String,
    pub review_item_id: String,
    pub decision: String,
    pub edited_payload: Option<Value>,
    pub reviewer_label: String,
    pub rationale: String,
    pub decided_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContradictionAssessmentRow {
    pub id: String,
    pub case_id: String,
    pub review_item_id: String,
    pub verdict: String,
    pub rationale: String,
    pub reviewer_label: String,
    pub assessed_at: String,
}

fn valid_claim_kind(value: &str) -> bool {
    matches!(
        value,
        "allegation" | "defense" | "admission" | "court_finding" | "unknown"
    )
}

/// Belge bağı ilişkisi. `response_to` burada **yok**: yanıt, belgeye değil
/// yanıtlanan iddiaya bağlanır (`claim_relations`). Böylece belge bağlantısında
/// anlamsız "yanıt" seçimi engellenir.
fn valid_document_relation(value: &str) -> bool {
    matches!(
        value,
        "submitted_for" | "supports_candidate" | "contradicts_candidate" | "context"
    )
}

fn valid_claim_relation(value: &str) -> bool {
    matches!(
        value,
        "response_to" | "supports" | "contradicts" | "context"
    )
}

fn valid_review_state(value: &str) -> bool {
    matches!(
        value,
        "pending" | "accepted" | "rejected" | "edited" | "needs_more_evidence" | "superseded"
    )
}

impl Database {
    fn case_exists(&self, case_id: &str) -> Result<bool, StorageError> {
        let count: i64 = self.conn().query_row(
            "SELECT count(*) FROM cases WHERE id=?1 AND deleted_at IS NULL",
            [case_id],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    fn anchor_in_case(&self, anchor_id: &str, case_id: &str) -> Result<bool, StorageError> {
        let count: i64 = self.conn().query_row(
            "SELECT count(*) FROM source_anchors WHERE id=?1 AND case_id=?2 AND anchor_status='active'",
            rusqlite::params![anchor_id, case_id],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn create_claim(&self, case_id: &str, new: &NewClaim) -> Result<ClaimRow, StorageError> {
        if new.claim_text.trim().is_empty() || !valid_claim_kind(&new.claim_kind) {
            return Err(StorageError::InvalidSetting);
        }
        if !self.case_exists(case_id)? {
            return Err(StorageError::NotFound);
        }
        if let Some(anchor) = &new.source_anchor_id {
            if !self.anchor_in_case(anchor, case_id)? {
                return Err(StorageError::NotFound);
            }
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO claims (id,case_id,claimant_actor_id,claim_text,claim_kind,status,source_anchor_id,created_at,updated_at,row_version)
             VALUES (?1,?2,?3,?4,?5,'draft',?6,?7,?7,1)",
            rusqlite::params![
                id,
                case_id,
                new.claimant_actor_id,
                new.claim_text.trim(),
                new.claim_kind,
                new.source_anchor_id,
                now
            ],
        )?;
        self.append_audit(
            "claim",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"caseId": case_id}),
        )?;
        self.get_claim(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_claim(&self, id: &str) -> Result<Option<ClaimRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,claimant_actor_id,claim_text,claim_kind,status,source_anchor_id,created_at,updated_at,row_version
             FROM claims WHERE id=?1 AND deleted_at IS NULL",
        )?;
        Ok(stmt.query_row([id], map_claim).optional()?)
    }

    pub fn list_claims(&self, case_id: &str) -> Result<Vec<ClaimRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,claimant_actor_id,claim_text,claim_kind,status,source_anchor_id,created_at,updated_at,row_version
             FROM claims WHERE case_id=?1 AND deleted_at IS NULL ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([case_id], map_claim)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn link_evidence(
        &self,
        case_id: &str,
        new: &NewEvidenceLink,
    ) -> Result<EvidenceLinkRow, StorageError> {
        if !valid_document_relation(&new.relation) {
            return Err(StorageError::InvalidSetting);
        }
        let claim = self
            .get_claim(&new.claim_id)?
            .ok_or(StorageError::NotFound)?;
        if claim.case_id != case_id {
            return Err(StorageError::NotFound);
        }
        if !self.anchor_in_case(&new.source_anchor_id, case_id)? {
            return Err(StorageError::NotFound);
        }
        if let Some(existing) =
            self.find_evidence_link(&new.claim_id, &new.source_anchor_id, &new.relation)?
        {
            return Ok(existing);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO evidence_links (id,case_id,claim_id,source_anchor_id,relation,rationale,review_state,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,'accepted',?7,?7)",
            rusqlite::params![id, case_id, new.claim_id, new.source_anchor_id, new.relation, new.rationale.trim(), now],
        )?;
        self.append_audit(
            "evidence_link",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"claimId": new.claim_id, "relation": new.relation}),
        )?;
        self.get_evidence_link(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn find_evidence_link(
        &self,
        claim_id: &str,
        anchor_id: &str,
        relation: &str,
    ) -> Result<Option<EvidenceLinkRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,claim_id,source_anchor_id,relation,rationale,review_state,created_at
             FROM evidence_links WHERE claim_id=?1 AND source_anchor_id=?2 AND relation=?3",
        )?;
        Ok(stmt
            .query_row(rusqlite::params![claim_id, anchor_id, relation], map_link)
            .optional()?)
    }

    pub fn get_evidence_link(&self, id: &str) -> Result<Option<EvidenceLinkRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,claim_id,source_anchor_id,relation,rationale,review_state,created_at
             FROM evidence_links WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_link).optional()?)
    }

    pub fn list_evidence_links(
        &self,
        claim_id: &str,
    ) -> Result<Vec<EvidenceLinkRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,claim_id,source_anchor_id,relation,rationale,review_state,created_at
             FROM evidence_links WHERE claim_id=?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([claim_id], map_link)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// İki iddia arasında ilişki kurar. İlişki türü whitelist'te olmalıdır;
    /// kendine bağ kurulamaz; iki iddia da aynı dosyada ve silinmemiş olmalıdır.
    /// `response_to` yalnız bir iddia/karar tespitine yanıt olarak kurulur ve
    /// yanıtlanan iddianın durumunu `responded` yapar.
    pub fn link_claim_relation(
        &self,
        case_id: &str,
        claim_id: &str,
        new: &NewClaimRelation,
    ) -> Result<ClaimRelationRow, StorageError> {
        if !valid_claim_relation(&new.relation) {
            return Err(StorageError::InvalidSetting);
        }
        if claim_id == new.related_claim_id {
            return Err(StorageError::InvalidSetting);
        }
        let claim = self.get_claim(claim_id)?.ok_or(StorageError::NotFound)?;
        let related = self
            .get_claim(&new.related_claim_id)?
            .ok_or(StorageError::NotFound)?;
        if claim.case_id != case_id || related.case_id != case_id {
            return Err(StorageError::NotFound);
        }
        if new.relation == "response_to"
            && !matches!(related.claim_kind.as_str(), "allegation" | "court_finding")
        {
            return Err(StorageError::InvalidSetting);
        }
        if let Some(existing) =
            self.find_claim_relation(claim_id, &new.related_claim_id, &new.relation)?
        {
            return Ok(existing);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "INSERT INTO claim_relations (id,case_id,claim_id,related_claim_id,relation,rationale,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?7)",
            rusqlite::params![
                id,
                case_id,
                claim_id,
                new.related_claim_id,
                new.relation,
                new.rationale.trim(),
                now
            ],
        )?;
        if new.relation == "response_to" {
            tx.execute(
                "UPDATE claims SET status='responded', updated_at=?1, row_version=row_version+1
                 WHERE id=?2 AND status IN ('draft','asserted')",
                rusqlite::params![now, new.related_claim_id],
            )?;
        }
        tx.commit()?;
        self.append_audit(
            "claim_relation",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({
                "claimId": claim_id,
                "relatedClaimId": new.related_claim_id,
                "relation": new.relation
            }),
        )?;
        self.get_claim_relation(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn find_claim_relation(
        &self,
        claim_id: &str,
        related_claim_id: &str,
        relation: &str,
    ) -> Result<Option<ClaimRelationRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,claim_id,related_claim_id,relation,rationale,created_at
             FROM claim_relations WHERE claim_id=?1 AND related_claim_id=?2 AND relation=?3",
        )?;
        Ok(stmt
            .query_row(
                rusqlite::params![claim_id, related_claim_id, relation],
                map_claim_relation,
            )
            .optional()?)
    }

    pub fn get_claim_relation(&self, id: &str) -> Result<Option<ClaimRelationRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,claim_id,related_claim_id,relation,rationale,created_at
             FROM claim_relations WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_claim_relation).optional()?)
    }

    /// Verilen iddiaya **her iki yönde** bağlı ilişkiler (giden + gelen).
    pub fn list_claim_relations(
        &self,
        claim_id: &str,
    ) -> Result<Vec<ClaimRelationRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,claim_id,related_claim_id,relation,rationale,created_at
             FROM claim_relations WHERE claim_id=?1 OR related_claim_id=?1
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([claim_id], map_claim_relation)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// İnceleme kuyruğuna öğe ekler. `source_anchor_ids` boş olamaz: kaynağı
    /// olmayan öneri kuyruğa giremez (INV-02).
    pub fn create_review_item(
        &self,
        case_id: &str,
        new: &NewReviewItem,
    ) -> Result<ReviewItemRow, StorageError> {
        if new.source_anchor_ids.is_empty() || !self.case_exists(case_id)? {
            return Err(StorageError::InvalidSetting);
        }
        if new.item_kind == "contradiction_candidate" {
            let mut anchors = new.source_anchor_ids.clone();
            anchors.sort();
            anchors.dedup();
            if anchors.len() != 2 {
                return Err(StorageError::InvalidSetting);
            }
        }
        for anchor in &new.source_anchor_ids {
            if !self.anchor_in_case(anchor, case_id)? {
                return Err(StorageError::NotFound);
            }
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO review_items (id,case_id,item_kind,payload_schema_version,payload,source_anchor_ids,ai_run_id,state,created_at,updated_at,row_version)
             VALUES (?1,?2,?3,1,?4,?5,?6,'pending',?7,?7,1)",
            rusqlite::params![
                id,
                case_id,
                new.item_kind,
                serde_json::to_string(&new.payload).unwrap_or_else(|_| "{}".into()),
                serde_json::to_string(&new.source_anchor_ids).unwrap_or_else(|_| "[]".into()),
                new.ai_run_id,
                now
            ],
        )?;
        self.append_audit(
            "review_item",
            Some(&id),
            "create",
            "system",
            &serde_json::json!({"kind": new.item_kind}),
        )?;
        self.get_review_item(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_review_item(&self, id: &str) -> Result<Option<ReviewItemRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,item_kind,payload_schema_version,payload,source_anchor_ids,ai_run_id,state,target_entity_id,superseded_by_item_id,superseded_at,created_at,updated_at,row_version
             FROM review_items WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_item).optional()?)
    }

    pub fn list_review_items(
        &self,
        case_id: &str,
        state: Option<&str>,
    ) -> Result<Vec<ReviewItemRow>, StorageError> {
        let base = "SELECT id,case_id,item_kind,payload_schema_version,payload,source_anchor_ids,ai_run_id,state,target_entity_id,superseded_by_item_id,superseded_at,created_at,updated_at,row_version FROM review_items WHERE case_id=?1";
        let (sql, only_superseded) = match state {
            Some("superseded") => (
                format!("{base} AND superseded_at IS NOT NULL ORDER BY created_at ASC"),
                true,
            ),
            Some(_) => (
                format!("{base} AND state=?2 AND superseded_at IS NULL ORDER BY created_at ASC"),
                false,
            ),
            None => (format!("{base} ORDER BY created_at ASC"), false),
        };
        let mut stmt = self.conn().prepare(&sql)?;
        let mut out = Vec::new();
        if let Some(state) = state {
            if !valid_review_state(state) {
                return Err(StorageError::InvalidSetting);
            }
            if only_superseded {
                for row in stmt.query_map([case_id], map_item)? {
                    out.push(row?);
                }
            } else {
                for row in stmt.query_map(rusqlite::params![case_id, state], map_item)? {
                    out.push(row?);
                }
            }
        } else {
            for row in stmt.query_map([case_id], map_item)? {
                out.push(row?);
            }
        }
        Ok(out)
    }

    /// İnceleme kararı: geçmiş eklemeli tutulur; karar verilmiş öğe yeniden
    /// "pending" yapılamaz. Kalıcı domain kaydı bu fonksiyonda oluşturulmaz
    /// (hedef kayıt için `commit_review_decision`).
    ///
    /// F08-T08: karar satırı, öğe durumu, denetim kaydı ve bildirim outbox
    /// olayı **aynı işlemde** yazılır; biri düşerse hiçbiri kalmaz.
    pub fn decide_review_item(
        &self,
        item_id: &str,
        decision: &str,
        edited_payload: Option<&Value>,
        reviewer: &str,
        rationale: &str,
    ) -> Result<ReviewItemRow, StorageError> {
        let next_state = match decision {
            "accept" => "accepted",
            "reject" => "rejected",
            "edit" => "edited",
            "request_evidence" => "needs_more_evidence",
            _ => return Err(StorageError::InvalidSetting),
        };
        let item = self
            .get_review_item(item_id)?
            .ok_or(StorageError::NotFound)?;
        if item.state != "pending" && item.state != "needs_more_evidence" {
            return Err(StorageError::InvalidSetting);
        }
        if item.superseded_at.is_some() {
            return Err(StorageError::InvalidSetting);
        }
        if reviewer.trim().is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        let now = now_iso();
        let decision_id = uuid::Uuid::now_v7().to_string();
        let outbox_id = format!("outbox:{decision_id}");
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "INSERT INTO review_decisions (id,review_item_id,decision,edited_payload,reviewer_label,rationale,decided_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![
                decision_id,
                item_id,
                decision,
                edited_payload.map(|v| v.to_string()),
                reviewer.trim(),
                rationale.trim(),
                now
            ],
        )?;
        let changed = tx.execute(
            "UPDATE review_items SET state=?1, payload=COALESCE(?2, payload), updated_at=?3, row_version=row_version+1
             WHERE id=?4 AND superseded_at IS NULL AND target_entity_id IS NULL
               AND state IN ('pending','needs_more_evidence')",
            rusqlite::params![next_state, edited_payload.map(|v| v.to_string()), now, item_id],
        )?;
        if changed == 0 {
            return Err(StorageError::Conflict {
                expected: item.row_version,
                actual: item.row_version,
            });
        }
        self.append_audit(
            "review_item",
            Some(item_id),
            decision,
            reviewer,
            &serde_json::json!({ "outboxEventId": outbox_id }),
        )?;
        // Bildirim olayı: tüketici hazır olana kadar kuyrukta bekler.
        tx.execute(
            "INSERT INTO outbox_events (id,aggregate_id,case_id,event_type,payload_version,payload,created_at,delivered_at,attempt_count)
             VALUES (?1,?2,?3,?4,1,?5,?6,NULL,0)",
            rusqlite::params![
                outbox_id,
                item_id,
                item.case_id,
                crate::review_commit::EVENT_ITEM_DECIDED,
                serde_json::json!({
                    "itemId": item_id,
                    "decisionId": decision_id,
                    "decision": decision,
                })
                .to_string(),
                now
            ],
        )?;
        tx.commit()?;
        self.get_review_item(item_id)?.ok_or(StorageError::NotFound)
    }

    /// Bir inceleme öğesini daha yeni bir adayla değiştirir. Eski öğe silinmez,
    /// `superseded` olarak işaretlenir ve yerine geçen öğe kimliği saklanır;
    /// kabul/ret kararı geçmişi olduğu gibi kalır. Zaten karara bağlanmış
    /// (kabul/düzeltme) kayıt da yeni adayla değiştirilebilir; ret edilmiş ya da
    /// daha önce değiştirilmiş kayıt değiştirilemez.
    pub fn supersede_review_item(
        &self,
        item_id: &str,
        superseded_by_item_id: &str,
    ) -> Result<ReviewItemRow, StorageError> {
        if item_id == superseded_by_item_id {
            return Err(StorageError::InvalidSetting);
        }
        let item = self
            .get_review_item(item_id)?
            .ok_or(StorageError::NotFound)?;
        let replacement = self
            .get_review_item(superseded_by_item_id)?
            .ok_or(StorageError::NotFound)?;
        if item.case_id != replacement.case_id {
            return Err(StorageError::NotFound);
        }
        if !matches!(
            item.state.as_str(),
            "pending" | "needs_more_evidence" | "accepted" | "edited"
        ) {
            return Err(StorageError::InvalidSetting);
        }
        if !matches!(replacement.state.as_str(), "pending" | "accepted") {
            return Err(StorageError::InvalidSetting);
        }
        let now = now_iso();
        self.conn().execute(
            "UPDATE review_items SET superseded_by_item_id=?1, superseded_at=?2, updated_at=?2, row_version=row_version+1
             WHERE id=?3 AND superseded_at IS NULL",
            rusqlite::params![superseded_by_item_id, now, item_id],
        )?;
        let updated = self
            .get_review_item(item_id)?
            .ok_or(StorageError::NotFound)?;
        if updated.superseded_at.is_none() {
            return Err(StorageError::Conflict {
                expected: item.row_version,
                actual: updated.row_version,
            });
        }
        self.append_audit(
            "review_item",
            Some(item_id),
            "supersede",
            "local-user",
            &serde_json::json!({ "supersededBy": superseded_by_item_id }),
        )?;
        self.get_review_item(item_id)?.ok_or(StorageError::NotFound)
    }

    /// Çelişki incelemesinde kullanıcının değerlendirmesini kaydeder. İki
    /// kaynağı olmayan öğe değerlendirilemez; değerlendirme inceleme kararından
    /// ayrıdır ve eklemelidir (en yenisi gösterilir).
    pub fn assess_contradiction(
        &self,
        case_id: &str,
        item_id: &str,
        verdict: &str,
        rationale: &str,
        reviewer: &str,
    ) -> Result<ContradictionAssessmentRow, StorageError> {
        if !matches!(
            verdict,
            "conflict" | "explained" | "not_conflict" | "needs_more_evidence"
        ) || reviewer.trim().is_empty()
        {
            return Err(StorageError::InvalidSetting);
        }
        let item = self
            .get_review_item(item_id)?
            .ok_or(StorageError::NotFound)?;
        if item.case_id != case_id {
            return Err(StorageError::NotFound);
        }
        if item.item_kind != "contradiction_candidate" {
            return Err(StorageError::InvalidSetting);
        }
        let mut anchors = item.source_anchor_ids.clone();
        anchors.sort();
        anchors.dedup();
        if anchors.len() != 2 {
            return Err(StorageError::InvalidSetting);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO contradiction_assessments (id,case_id,review_item_id,verdict,rationale,reviewer_label,assessed_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![
                id,
                case_id,
                item_id,
                verdict,
                rationale.trim(),
                reviewer.trim(),
                now
            ],
        )?;
        self.append_audit(
            "review_item",
            Some(item_id),
            "contradiction_assess",
            reviewer,
            &serde_json::json!({ "verdict": verdict }),
        )?;
        self.get_contradiction_assessment(&id)?
            .ok_or(StorageError::NotFound)
    }

    pub fn get_contradiction_assessment(
        &self,
        id: &str,
    ) -> Result<Option<ContradictionAssessmentRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,review_item_id,verdict,rationale,reviewer_label,assessed_at
             FROM contradiction_assessments WHERE id=?1",
        )?;
        Ok(stmt
            .query_row([id], map_contradiction_assessment)
            .optional()?)
    }

    pub fn list_contradiction_assessments(
        &self,
        item_id: &str,
    ) -> Result<Vec<ContradictionAssessmentRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,review_item_id,verdict,rationale,reviewer_label,assessed_at
             FROM contradiction_assessments WHERE review_item_id=?1 ORDER BY assessed_at ASC",
        )?;
        let rows = stmt.query_map([item_id], map_contradiction_assessment)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn review_history(&self, item_id: &str) -> Result<Vec<ReviewDecisionRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,review_item_id,decision,edited_payload,reviewer_label,rationale,decided_at
             FROM review_decisions WHERE review_item_id=?1 ORDER BY decided_at ASC",
        )?;
        let rows = stmt.query_map([item_id], |row| {
            Ok(ReviewDecisionRow {
                id: row.get(0)?,
                review_item_id: row.get(1)?,
                decision: row.get(2)?,
                edited_payload: row
                    .get::<_, Option<String>>(3)?
                    .and_then(|s| serde_json::from_str(&s).ok()),
                reviewer_label: row.get(4)?,
                rationale: row.get(5)?,
                decided_at: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

fn map_claim(row: &Row<'_>) -> rusqlite::Result<ClaimRow> {
    Ok(ClaimRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        claimant_actor_id: row.get(2)?,
        claim_text: row.get(3)?,
        claim_kind: row.get(4)?,
        status: row.get(5)?,
        source_anchor_id: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        row_version: row.get(9)?,
    })
}

fn map_link(row: &Row<'_>) -> rusqlite::Result<EvidenceLinkRow> {
    Ok(EvidenceLinkRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        claim_id: row.get(2)?,
        source_anchor_id: row.get(3)?,
        relation: row.get(4)?,
        rationale: row.get(5)?,
        review_state: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn map_claim_relation(row: &Row<'_>) -> rusqlite::Result<ClaimRelationRow> {
    Ok(ClaimRelationRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        claim_id: row.get(2)?,
        related_claim_id: row.get(3)?,
        relation: row.get(4)?,
        rationale: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn map_item(row: &Row<'_>) -> rusqlite::Result<ReviewItemRow> {
    let payload: String = row.get(4)?;
    let anchors: String = row.get(5)?;
    let stored_state: String = row.get(7)?;
    let superseded_by_item_id: Option<String> = row.get(9)?;
    let superseded_at: Option<String> = row.get(10)?;
    let state = if superseded_at.is_some() {
        "superseded".to_string()
    } else {
        stored_state
    };
    Ok(ReviewItemRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        item_kind: row.get(2)?,
        payload_schema_version: row.get(3)?,
        payload: serde_json::from_str(&payload).unwrap_or(Value::Null),
        source_anchor_ids: serde_json::from_str(&anchors).unwrap_or_default(),
        ai_run_id: row.get(6)?,
        state,
        target_entity_id: row.get(8)?,
        superseded_by_item_id,
        superseded_at,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        row_version: row.get(13)?,
    })
}

fn map_contradiction_assessment(row: &Row<'_>) -> rusqlite::Result<ContradictionAssessmentRow> {
    Ok(ContradictionAssessmentRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        review_item_id: row.get(2)?,
        verdict: row.get(3)?,
        rationale: row.get(4)?,
        reviewer_label: row.get(5)?,
        assessed_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::{DatabaseKey, NewAnchor, NewPage};

    struct Setup {
        db: Database,
        case_id: String,
        anchor_id: String,
    }

    fn setup() -> Setup {
        let db = Database::open_in_memory(&DatabaseKey::new([31u8; 32])).expect("db");
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
        let blob = db.create_blob_record("st-claim", 10, "aad").expect("blob");
        let doc = db
            .create_document(&case.id, "Belge", "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &doc.id,
                &case.id,
                &blob,
                &"f".repeat(64),
                10,
                "application/pdf",
            )
            .expect("v");
        let run = db
            .start_parse_run(&version.id, &case.id, "pymupdf", "1.24", "{}")
            .expect("run");
        db.store_pages(
            &run.id,
            &case.id,
            &[NewPage {
                index: 0,
                text: "Ödeme yapılmadı.".into(),
                width: None,
                height: None,
                origin: "native".into(),
                warnings: vec![],
                ..Default::default()
            }],
        )
        .expect("pages");
        let segment: String = db
            .conn()
            .query_row(
                "SELECT id FROM text_segments WHERE parse_run_id=?1",
                [&run.id],
                |r| r.get(0),
            )
            .expect("segment");
        let anchor = db
            .create_anchor(&NewAnchor {
                case_id: case.id.clone(),
                document_version_id: version.id,
                parse_run_id: run.id,
                segment_id: segment,
                start_offset: 0,
                end_offset: 16,
                exact_quote: "Ödeme yapılmadı.".into(),
            })
            .expect("anchor");
        Setup {
            db,
            case_id: case.id,
            anchor_id: anchor.id,
        }
    }

    fn claim(text: &str) -> NewClaim {
        NewClaim {
            claimant_actor_id: None,
            claim_text: text.into(),
            claim_kind: "allegation".into(),
            source_anchor_id: None,
        }
    }

    #[test]
    fn claim_lifecycle() {
        let s = setup();
        let c =
            s.db.create_claim(&s.case_id, &claim("Kira ödenmedi"))
                .expect("claim");
        assert_eq!(c.status, "draft");
        assert_eq!(s.db.list_claims(&s.case_id).expect("liste").len(), 1);
    }

    #[test]
    fn invalid_claim_kind_rejected() {
        let s = setup();
        let mut c = claim("X");
        c.claim_kind = "kesin_hukum".into();
        assert!(s.db.create_claim(&s.case_id, &c).is_err());
    }

    #[test]
    fn evidence_link_is_idempotent_and_case_scoped() {
        let s = setup();
        let c =
            s.db.create_claim(&s.case_id, &claim("Kira ödenmedi"))
                .expect("claim");
        let input = NewEvidenceLink {
            claim_id: c.id.clone(),
            source_anchor_id: s.anchor_id.clone(),
            relation: "supports_candidate".into(),
            rationale: "makbuz yok".into(),
        };
        let first = s.db.link_evidence(&s.case_id, &input).expect("bağ");
        let second = s.db.link_evidence(&s.case_id, &input).expect("tekrar");
        assert_eq!(first.id, second.id);
        assert_eq!(s.db.list_evidence_links(&c.id).expect("liste").len(), 1);

        let other =
            s.db.create_case(&NewCase {
                title: "Başka".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        assert!(s.db.link_evidence(&other.id, &input).is_err());
    }

    #[test]
    fn review_item_requires_source_and_is_decided_once() {
        let s = setup();
        let item =
            s.db.create_review_item(
                &s.case_id,
                &NewReviewItem {
                    item_kind: "claim_candidate".into(),
                    payload: serde_json::json!({"text": "Kira ödenmedi"}),
                    source_anchor_ids: vec![s.anchor_id.clone()],
                    ai_run_id: None,
                },
            )
            .expect("öğe");
        assert_eq!(item.state, "pending");

        let decided =
            s.db.decide_review_item(&item.id, "accept", None, "avukat", "kaynak yeterli")
                .expect("karar");
        assert_eq!(decided.state, "accepted");
        assert!(s
            .db
            .decide_review_item(&item.id, "reject", None, "avukat", "")
            .is_err());
        assert_eq!(s.db.review_history(&item.id).expect("geçmiş").len(), 1);
        assert!(s.db.verify_audit_chain().expect("zincir") >= 2);
    }

    #[test]
    fn review_item_without_anchor_rejected() {
        let s = setup();
        let err =
            s.db.create_review_item(
                &s.case_id,
                &NewReviewItem {
                    item_kind: "note_candidate".into(),
                    payload: serde_json::json!({}),
                    source_anchor_ids: vec![],
                    ai_run_id: None,
                },
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_DB_SETTING");
    }

    #[test]
    fn edit_decision_replaces_payload() {
        let s = setup();
        let item =
            s.db.create_review_item(
                &s.case_id,
                &NewReviewItem {
                    item_kind: "claim_candidate".into(),
                    payload: serde_json::json!({"text": "eski"}),
                    source_anchor_ids: vec![s.anchor_id.clone()],
                    ai_run_id: None,
                },
            )
            .expect("öğe");
        let edited = serde_json::json!({"text": "düzeltilmiş"});
        let result =
            s.db.decide_review_item(
                &item.id,
                "edit",
                Some(&edited),
                "avukat",
                "ifade düzeltildi",
            )
            .expect("karar");
        assert_eq!(result.state, "edited");
        assert_eq!(result.payload["text"], "düzeltilmiş");
    }

    fn claim_of(text: &str, kind: &str) -> NewClaim {
        NewClaim {
            claimant_actor_id: None,
            claim_text: text.into(),
            claim_kind: kind.into(),
            source_anchor_id: None,
        }
    }

    #[test]
    fn claim_relation_requires_explicit_type() {
        let s = setup();
        let a =
            s.db.create_claim(&s.case_id, &claim_of("Kira ödenmedi", "allegation"))
                .expect("a");
        let b =
            s.db.create_claim(&s.case_id, &claim_of("Ödedim", "defense"))
                .expect("b");
        for relation in ["", "response", "supports_candidate"] {
            let err =
                s.db.link_claim_relation(
                    &s.case_id,
                    &b.id,
                    &NewClaimRelation {
                        related_claim_id: a.id.clone(),
                        relation: relation.into(),
                        rationale: String::new(),
                    },
                )
                .unwrap_err();
            assert_eq!(
                err.code(),
                "E_DB_SETTING",
                "ilişki {relation:?} reddedilmeli"
            );
        }
        assert!(s.db.list_claim_relations(&a.id).expect("liste").is_empty());
    }

    #[test]
    fn claim_relation_rejects_self_and_cross_case() {
        let s = setup();
        let a =
            s.db.create_claim(&s.case_id, &claim_of("Kira ödenmedi", "allegation"))
                .expect("a");
        let self_link =
            s.db.link_claim_relation(
                &s.case_id,
                &a.id,
                &NewClaimRelation {
                    related_claim_id: a.id.clone(),
                    relation: "supports".into(),
                    rationale: String::new(),
                },
            )
            .unwrap_err();
        assert_eq!(self_link.code(), "E_DB_SETTING");

        let other_case =
            s.db.create_case(&NewCase {
                title: "Başka".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        let foreign =
            s.db.create_claim(
                &other_case.id,
                &claim_of("Başka dosya iddiası", "allegation"),
            )
            .expect("foreign");
        let cross =
            s.db.link_claim_relation(
                &s.case_id,
                &a.id,
                &NewClaimRelation {
                    related_claim_id: foreign.id,
                    relation: "supports".into(),
                    rationale: String::new(),
                },
            )
            .unwrap_err();
        assert_eq!(cross.code(), "E_NOT_FOUND");
    }

    #[test]
    fn response_to_marks_original_and_is_idempotent() {
        let s = setup();
        let allegation =
            s.db.create_claim(&s.case_id, &claim_of("Kira ödenmedi", "allegation"))
                .expect("allegation");
        let defense =
            s.db.create_claim(&s.case_id, &claim_of("Kira ödendi", "defense"))
                .expect("defense");
        assert_eq!(allegation.status, "draft");

        let input = NewClaimRelation {
            related_claim_id: allegation.id.clone(),
            relation: "response_to".into(),
            rationale: "banka dekontu".into(),
        };
        let first =
            s.db.link_claim_relation(&s.case_id, &defense.id, &input)
                .expect("bağ");
        let second =
            s.db.link_claim_relation(&s.case_id, &defense.id, &input)
                .expect("tekrar");
        assert_eq!(first.id, second.id);

        let updated = s.db.get_claim(&allegation.id).expect("oku").expect("var");
        assert_eq!(updated.status, "responded");
        assert_eq!(updated.row_version, allegation.row_version + 1);

        let both = s.db.list_claim_relations(&allegation.id).expect("liste");
        assert_eq!(both.len(), 1, "gelen ilişki de görünmeli");
        assert_eq!(both[0].claim_id, defense.id);
        assert!(s.db.verify_audit_chain().expect("zincir") >= 3);
    }

    #[test]
    fn response_to_requires_allegation_target() {
        let s = setup();
        let first_defense =
            s.db.create_claim(&s.case_id, &claim_of("Ödedim", "defense"))
                .expect("defense");
        let second_defense =
            s.db.create_claim(&s.case_id, &claim_of("Yine ödedim", "defense"))
                .expect("defense2");
        let err =
            s.db.link_claim_relation(
                &s.case_id,
                &second_defense.id,
                &NewClaimRelation {
                    related_claim_id: first_defense.id,
                    relation: "response_to".into(),
                    rationale: String::new(),
                },
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_DB_SETTING");
    }

    #[test]
    fn supersede_marks_item_and_keeps_decision_history() {
        let s = setup();
        let old =
            s.db.create_review_item(
                &s.case_id,
                &NewReviewItem {
                    item_kind: "claim_candidate".into(),
                    payload: serde_json::json!({"text": "eski"}),
                    source_anchor_ids: vec![s.anchor_id.clone()],
                    ai_run_id: None,
                },
            )
            .expect("eski");
        s.db.decide_review_item(&old.id, "accept", None, "avukat", "kabul")
            .expect("karar");
        let new =
            s.db.create_review_item(
                &s.case_id,
                &NewReviewItem {
                    item_kind: "claim_candidate".into(),
                    payload: serde_json::json!({"text": "yeni"}),
                    source_anchor_ids: vec![s.anchor_id.clone()],
                    ai_run_id: None,
                },
            )
            .expect("yeni");

        let before = s.db.get_review_item(&old.id).expect("oku").expect("var");
        let result =
            s.db.supersede_review_item(&old.id, &new.id)
                .expect("değiştir");
        assert_eq!(result.state, "superseded");
        assert_eq!(
            result.superseded_by_item_id.as_deref(),
            Some(new.id.as_str())
        );
        assert_eq!(result.row_version, before.row_version + 1);
        assert_eq!(
            s.db.review_history(&old.id).expect("geçmiş").len(),
            1,
            "kabul kararı korunmalı"
        );
        assert!(s
            .db
            .decide_review_item(&old.id, "reject", None, "avukat", "")
            .is_err());
        let superseded =
            s.db.list_review_items(&s.case_id, Some("superseded"))
                .expect("liste");
        assert_eq!(superseded.len(), 1);
        assert_eq!(superseded[0].id, old.id);
        assert!(s.db.verify_audit_chain().expect("zincir") >= 4);
    }

    #[test]
    fn supersede_rejects_self_cross_case_and_rejected() {
        let s = setup();
        let a =
            s.db.create_review_item(
                &s.case_id,
                &NewReviewItem {
                    item_kind: "note_candidate".into(),
                    payload: serde_json::json!({"text": "a"}),
                    source_anchor_ids: vec![s.anchor_id.clone()],
                    ai_run_id: None,
                },
            )
            .expect("a");
        assert_eq!(
            s.db.supersede_review_item(&a.id, &a.id).unwrap_err().code(),
            "E_DB_SETTING"
        );

        let other_case =
            s.db.create_case(&NewCase {
                title: "Başka".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        let foreign = s.db.create_review_item(
            &other_case.id,
            &NewReviewItem {
                item_kind: "note_candidate".into(),
                payload: serde_json::json!({}),
                source_anchor_ids: vec![s.anchor_id.clone()],
                ai_run_id: None,
            },
        );
        assert!(foreign.is_err(), "başka dosyanın anchor'ı reddedilmeli");

        let rejected =
            s.db.create_review_item(
                &s.case_id,
                &NewReviewItem {
                    item_kind: "note_candidate".into(),
                    payload: serde_json::json!({"text": "ret"}),
                    source_anchor_ids: vec![s.anchor_id.clone()],
                    ai_run_id: None,
                },
            )
            .expect("ret");
        s.db.decide_review_item(&rejected.id, "reject", None, "avukat", "")
            .expect("ret kararı");
        let target =
            s.db.create_review_item(
                &s.case_id,
                &NewReviewItem {
                    item_kind: "note_candidate".into(),
                    payload: serde_json::json!({"text": "aday"}),
                    source_anchor_ids: vec![s.anchor_id.clone()],
                    ai_run_id: None,
                },
            )
            .expect("aday");
        assert_eq!(
            s.db.supersede_review_item(&rejected.id, &target.id)
                .unwrap_err()
                .code(),
            "E_DB_SETTING"
        );
    }

    #[test]
    fn evidence_link_rejects_response_relation() {
        let s = setup();
        let c =
            s.db.create_claim(&s.case_id, &claim_of("Kira ödenmedi", "allegation"))
                .expect("claim");
        let err =
            s.db.link_evidence(
                &s.case_id,
                &NewEvidenceLink {
                    claim_id: c.id,
                    source_anchor_id: s.anchor_id.clone(),
                    relation: "response_to".into(),
                    rationale: String::new(),
                },
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_DB_SETTING");
    }

    fn second_anchor(s: &Setup) -> crate::AnchorRow {
        let (segment, run, version) = s
            .db
            .conn()
            .query_row(
                "SELECT segment_id, parse_run_id, document_version_id FROM source_anchors WHERE id=?1",
                [&s.anchor_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )
            .expect("anchor");
        s.db.create_anchor(&NewAnchor {
            case_id: s.case_id.clone(),
            document_version_id: version,
            parse_run_id: run,
            segment_id: segment,
            start_offset: 0,
            end_offset: 6,
            exact_quote: "Ödeme ".into(),
        })
        .expect("ikinci anchor")
    }

    #[test]
    fn contradiction_requires_two_distinct_anchors() {
        let s = setup();
        let err =
            s.db.create_review_item(
                &s.case_id,
                &NewReviewItem {
                    item_kind: "contradiction_candidate".into(),
                    payload: serde_json::json!({}),
                    source_anchor_ids: vec![s.anchor_id.clone()],
                    ai_run_id: None,
                },
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_DB_SETTING");

        let dup =
            s.db.create_review_item(
                &s.case_id,
                &NewReviewItem {
                    item_kind: "contradiction_candidate".into(),
                    payload: serde_json::json!({}),
                    source_anchor_ids: vec![s.anchor_id.clone(), s.anchor_id.clone()],
                    ai_run_id: None,
                },
            )
            .unwrap_err();
        assert_eq!(dup.code(), "E_DB_SETTING", "aynı anchor iki kez sayılmaz");
    }

    #[test]
    fn contradiction_assessment_records_verdict() {
        let s = setup();
        let other = second_anchor(&s);
        let item =
            s.db.create_review_item(
                &s.case_id,
                &NewReviewItem {
                    item_kind: "contradiction_candidate".into(),
                    payload: serde_json::json!({"note": "iki ifade farklı"}),
                    source_anchor_ids: vec![s.anchor_id.clone(), other.id.clone()],
                    ai_run_id: None,
                },
            )
            .expect("çelişki");
        assert_eq!(item.source_anchor_ids.len(), 2);

        let first =
            s.db.assess_contradiction(&s.case_id, &item.id, "explained", "farklı tarih", "avukat")
                .expect("değerlendirme");
        assert_eq!(first.verdict, "explained");
        let second =
            s.db.assess_contradiction(
                &s.case_id,
                &item.id,
                "conflict",
                "çelişki sürüyor",
                "avukat",
            )
            .expect("ikinci");
        let list =
            s.db.list_contradiction_assessments(&item.id)
                .expect("liste");
        assert_eq!(list.len(), 2, "değerlendirmeler eklemeli");
        assert_eq!(list[1].id, second.id);

        let invalid =
            s.db.assess_contradiction(&s.case_id, &item.id, "belki", "", "avukat")
                .unwrap_err();
        assert_eq!(invalid.code(), "E_DB_SETTING");

        let note_item =
            s.db.create_review_item(
                &s.case_id,
                &NewReviewItem {
                    item_kind: "note_candidate".into(),
                    payload: serde_json::json!({}),
                    source_anchor_ids: vec![s.anchor_id.clone()],
                    ai_run_id: None,
                },
            )
            .expect("not");
        assert_eq!(
            s.db.assess_contradiction(&s.case_id, &note_item.id, "conflict", "", "avukat")
                .unwrap_err()
                .code(),
            "E_DB_SETTING",
            "yalnız çelişki öğesi değerlendirilir"
        );
        assert!(s.db.verify_audit_chain().expect("zincir") >= 4);
    }
}
