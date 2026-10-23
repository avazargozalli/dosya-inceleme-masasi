//! F08-T08 — inceleme kararının kalıcı sonucu **tek SQLite işleminde** yazılır:
//! hedef domain kaydı, `review_decisions` satırı, `review_items` durumu, denetim
//! zinciri kaydı ve bildirim outbox olayı.
//!
//! Neden tek işlem: kabul edilen bir öneri ile onun doğurduğu kayıt arasında
//! kopukluk olursa aynı öneri ikinci kez kabul edilip **çift kayıt** üretebilir
//! ya da kayıt oluşup bildirim hiç yazılmaz. Bu modülde hedef kayıtlar mevcut
//! `create_*` fonksiyonlarıyla (aynı bağlantı üzerinde, dolayısıyla açık işlemin
//! içinde) oluşturulur; herhangi bir adım hata verirse `Transaction` düşer ve
//! yarım kalan hedef kayıt, karar, denetim ve outbox satırı birlikte geri alınır.
//!
//! Karar verilmiş öğe yeniden karara bağlanamaz; `target_entity_id` dolduysa da
//! yeni hedef kayıt üretilmez. Outbox olayı karar satırının kimliğini taşır:
//! aynı karar için ikinci olay yazılamaz.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{now_iso, Database, NewClaim, NewEvent, NewNote, NewTask, ReviewItemRow, StorageError};

/// Onaylanan önerinin dönüşeceği kalıcı kayıt. Türü kullanıcı açıkça seçer;
/// gövde alanları hedef kaydın kendi doğrulamasından geçer.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ReviewTarget {
    Claim {
        claim_text: String,
        claim_kind: String,
        #[serde(default)]
        claimant_actor_id: Option<String>,
        #[serde(default)]
        source_anchor_id: Option<String>,
    },
    Note {
        #[serde(default)]
        title: String,
        body: String,
    },
    Task {
        title: String,
        #[serde(default)]
        user_due_date: Option<String>,
        #[serde(default)]
        date_timezone: Option<String>,
        #[serde(default)]
        reminder_policy: Option<String>,
    },
    Event {
        event_kind: String,
        title: String,
        #[serde(default)]
        date_value: Option<String>,
        date_precision: String,
        date_role: String,
        #[serde(default)]
        date_raw: Option<String>,
        #[serde(default)]
        description: String,
        #[serde(default = "default_assertion")]
        assertion_kind: String,
    },
}

fn default_assertion() -> String {
    "user".to_string()
}

impl ReviewTarget {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Claim { .. } => "claim",
            Self::Note { .. } => "note",
            Self::Task { .. } => "task",
            Self::Event { .. } => "event",
        }
    }
}

/// Atomik yazımın sonucu: kabul edilen öğe, üretilen hedef kayıt ve bildirim
/// olayı. `outboxEventId` boş bırakılamaz; outbox satırı yazılmadan işlem
/// tamamlanmaz.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCommitResult {
    pub item: ReviewItemRow,
    pub decision_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub outbox_event_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxEventRow {
    pub id: String,
    pub aggregate_id: String,
    pub case_id: Option<String>,
    pub event_type: String,
    pub payload_version: i64,
    pub payload: Value,
    pub created_at: String,
    pub delivered_at: Option<String>,
    pub attempt_count: i64,
}

/// Bildirim outbox olay türleri. Yeni tür eklemek şema değişikliği gerektirmez
/// ama tüketici tarafın anlaması gerekir.
pub const EVENT_DECISION_COMMITTED: &str = "review.decision_committed";
pub const EVENT_ITEM_DECIDED: &str = "review.item_decided";

impl Database {
    /// İnceleme kararını hedef kayıtla birlikte atomik yazar.
    ///
    /// Sıra: hedef kayıt → onay işaretleri → karar satırı → öğe durumu
    /// (`accepted`/`edited`, `target_entity_id`) → denetim → outbox. Adım
    /// sayısı değil, hepsinin **aynı** işlemde olması önemlidir.
    pub fn commit_review_decision(
        &self,
        item_id: &str,
        decision: &str,
        target: &ReviewTarget,
        reviewer: &str,
        rationale: &str,
    ) -> Result<ReviewCommitResult, StorageError> {
        let next_state = match decision {
            "accept" => "accepted",
            "edit" => "edited",
            _ => return Err(StorageError::InvalidSetting),
        };
        if reviewer.trim().is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        let item = self
            .get_review_item(item_id)?
            .ok_or(StorageError::NotFound)?;
        if !matches!(item.state.as_str(), "pending" | "needs_more_evidence")
            || item.superseded_at.is_some()
            || item.target_entity_id.is_some()
        {
            return Err(StorageError::InvalidSetting);
        }
        // Çelişki adayı bir iddia değildir; yalnız not olarak kayda geçebilir.
        // Aksi hâlde "çelişki var" değerlendirmesi iddia uydurmaya dönüşürdü.
        if item.item_kind == "contradiction_candidate"
            && !matches!(target, ReviewTarget::Note { .. })
        {
            return Err(StorageError::InvalidSetting);
        }

        let anchors = item.source_anchor_ids.clone();
        let first_anchor = anchors.first().cloned();
        let now = now_iso();
        let decision_id = uuid::Uuid::now_v7().to_string();
        let outbox_id = format!("outbox:{decision_id}");
        let target_json = serde_json::to_value(target).map_err(|_| StorageError::InvalidSetting)?;

        let tx = self.conn().unchecked_transaction()?;

        let target_id = match target {
            ReviewTarget::Claim {
                claim_text,
                claim_kind,
                claimant_actor_id,
                source_anchor_id,
            } => {
                let claim = self.create_claim(
                    &item.case_id,
                    &NewClaim {
                        claimant_actor_id: claimant_actor_id.clone(),
                        claim_text: claim_text.clone(),
                        claim_kind: claim_kind.clone(),
                        source_anchor_id: source_anchor_id.clone().or_else(|| first_anchor.clone()),
                    },
                )?;
                // Onaylanan iddia taslak kalmaz.
                tx.execute(
                    "UPDATE claims SET status='asserted', updated_at=?1, row_version=row_version+1
                     WHERE id=?2 AND status='draft'",
                    rusqlite::params![now, claim.id],
                )?;
                claim.id
            }
            ReviewTarget::Note { title, body } => {
                let body_text = body.trim().to_string();
                let title_text = if title.trim().is_empty() {
                    body_text.chars().take(80).collect::<String>()
                } else {
                    title.trim().to_string()
                };
                let note = self.create_note(
                    &item.case_id,
                    &NewNote {
                        title: title_text,
                        body: body_text,
                        source_anchor_ids: anchors.clone(),
                    },
                )?;
                // Not onaylandı: inceleme kararıyla birlikte damgalanır.
                tx.execute(
                    "UPDATE notes SET approved_at=?1, updated_at=?1, row_version=row_version+1 WHERE id=?2",
                    rusqlite::params![now, note.id],
                )?;
                note.id
            }
            ReviewTarget::Task {
                title,
                user_due_date,
                date_timezone,
                reminder_policy,
            } => {
                let task = self.create_task(
                    &item.case_id,
                    &NewTask {
                        title: title.clone(),
                        user_due_date: user_due_date.clone(),
                        date_timezone: date_timezone.clone(),
                        assignee_label: None,
                        reminder_policy: reminder_policy
                            .clone()
                            .unwrap_or_else(|| "none".to_string()),
                    },
                )?;
                if let Some(anchor) = &first_anchor {
                    tx.execute(
                        "UPDATE tasks SET source_anchor_id=?1, updated_at=?2, row_version=row_version+1 WHERE id=?3",
                        rusqlite::params![anchor, now, task.id],
                    )?;
                }
                task.id
            }
            ReviewTarget::Event {
                event_kind,
                title,
                date_value,
                date_precision,
                date_role,
                date_raw,
                description,
                assertion_kind,
            } => {
                let event = self.create_event(
                    &item.case_id,
                    &NewEvent {
                        event_kind: event_kind.clone(),
                        date_value: date_value.clone(),
                        date_precision: date_precision.clone(),
                        date_role: date_role.clone(),
                        date_raw: date_raw.clone(),
                        title: title.clone(),
                        description: description.clone(),
                        assertion_kind: assertion_kind.clone(),
                    },
                )?;
                // Kullanıcı onayı: olay `confirmed` olur ve kaynak bağları
                // onaylı olarak işaretlenir (kaynaksız olay kabul edilmez).
                self.confirm_event(&event.id)?;
                for anchor in &anchors {
                    tx.execute(
                        "INSERT OR IGNORE INTO event_sources (id,event_id,source_anchor_id,relation,user_confirmed)
                         VALUES (?1,?2,?3,'states',1)",
                        rusqlite::params![uuid::Uuid::now_v7().to_string(), event.id, anchor],
                    )?;
                }
                event.id
            }
        };

        let edited_payload = if decision == "edit" {
            Some(target_json.to_string())
        } else {
            None
        };
        tx.execute(
            "INSERT INTO review_decisions (id,review_item_id,decision,edited_payload,reviewer_label,rationale,decided_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![
                decision_id,
                item_id,
                decision,
                edited_payload,
                reviewer.trim(),
                rationale.trim(),
                now
            ],
        )?;
        // Koşullu güncelleme: araya giren bir karar varsa hedef kayıt da geri alınır.
        let changed = tx.execute(
            "UPDATE review_items SET state=?1, target_entity_id=?2, payload=COALESCE(?3,payload),
                    updated_at=?4, row_version=row_version+1
             WHERE id=?5 AND target_entity_id IS NULL AND superseded_at IS NULL
               AND state IN ('pending','needs_more_evidence')",
            rusqlite::params![next_state, target_id, edited_payload, now, item_id],
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
            "commit",
            reviewer.trim(),
            &serde_json::json!({
                "decision": decision,
                "targetKind": target.kind(),
                "targetId": target_id,
                "outboxEventId": outbox_id,
            }),
        )?;
        // Bildirim olayı yalnız kimlik taşır; dosya içeriği outbox'a sızmaz.
        tx.execute(
            "INSERT INTO outbox_events (id,aggregate_id,case_id,event_type,payload_version,payload,created_at,delivered_at,attempt_count)
             VALUES (?1,?2,?3,?4,1,?5,?6,NULL,0)",
            rusqlite::params![
                outbox_id,
                target_id,
                item.case_id,
                EVENT_DECISION_COMMITTED,
                serde_json::json!({
                    "itemId": item_id,
                    "decisionId": decision_id,
                    "decision": decision,
                    "targetKind": target.kind(),
                    "targetId": target_id,
                })
                .to_string(),
                now
            ],
        )?;
        tx.commit()?;

        let item = self
            .get_review_item(item_id)?
            .ok_or(StorageError::NotFound)?;
        Ok(ReviewCommitResult {
            item,
            decision_id,
            target_kind: target.kind().to_string(),
            target_id,
            outbox_event_id: outbox_id,
        })
    }

    /// Bekleyen/tüm bildirim olayları. Tüketici (yerel hatırlatıcı) hazır
    /// olana kadar olaylar `delivered_at IS NULL` olarak birikir.
    pub fn list_outbox_events(
        &self,
        case_id: Option<&str>,
        only_pending: bool,
    ) -> Result<Vec<OutboxEventRow>, StorageError> {
        let mut sql = String::from(
            "SELECT id,aggregate_id,case_id,event_type,payload_version,payload,created_at,delivered_at,attempt_count
             FROM outbox_events WHERE 1=1",
        );
        if case_id.is_some() {
            sql.push_str(" AND case_id=?1");
        }
        if only_pending {
            sql.push_str(" AND delivered_at IS NULL");
        }
        sql.push_str(" ORDER BY created_at ASC");
        let mut stmt = self.conn().prepare(&sql)?;
        let map = |row: &rusqlite::Row<'_>| -> rusqlite::Result<OutboxEventRow> {
            Ok(OutboxEventRow {
                id: row.get(0)?,
                aggregate_id: row.get(1)?,
                case_id: row.get(2)?,
                event_type: row.get(3)?,
                payload_version: row.get(4)?,
                payload: serde_json::from_str(&row.get::<_, String>(5)?)
                    .unwrap_or(serde_json::Value::Null),
                created_at: row.get(6)?,
                delivered_at: row.get(7)?,
                attempt_count: row.get(8)?,
            })
        };
        let mut out = Vec::new();
        if let Some(case_id) = case_id {
            for row in stmt.query_map([case_id], map)? {
                out.push(row?);
            }
        } else {
            for row in stmt.query_map([], map)? {
                out.push(row?);
            }
        }
        Ok(out)
    }

    /// Bir inceleme öğesinin ürettiği hedef kaydı türüyle birlikte çözer.
    /// UI'da "→ kayıt" bağlantısı için kullanılır; kayıt silinmişse `None`.
    pub fn review_target_exists(&self, item_id: &str) -> Result<bool, StorageError> {
        let row: Option<(Option<String>, Option<String>)> = self
            .conn()
            .query_row(
                "SELECT target_entity_id, superseded_at FROM review_items WHERE id=?1",
                [item_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (target, _) = row.ok_or(StorageError::NotFound)?;
        let Some(target) = target else {
            return Ok(false);
        };
        let found: i64 = self.conn().query_row(
            "SELECT (SELECT count(*) FROM claims WHERE id=?1)
                  + (SELECT count(*) FROM notes WHERE id=?1)
                  + (SELECT count(*) FROM tasks WHERE id=?1)
                  + (SELECT count(*) FROM events WHERE id=?1)",
            [&target],
            |r| r.get(0),
        )?;
        Ok(found > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CaseKind, DatabaseKey, NewAnchor, NewCase, NewPage, NewReviewItem};

    struct Setup {
        db: Database,
        case_id: String,
        anchor_id: String,
        anchor_id2: String,
    }

    fn setup() -> Setup {
        let db = Database::open_in_memory(&DatabaseKey::new([37u8; 32])).expect("db");
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
        let blob = db.create_blob_record("st-review", 10, "aad").expect("blob");
        let doc = db
            .create_document(&case.id, "Belge", "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &doc.id,
                &case.id,
                &blob,
                &"e".repeat(64),
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
                document_version_id: version.id.clone(),
                parse_run_id: run.id.clone(),
                segment_id: segment.clone(),
                start_offset: 0,
                end_offset: 16,
                exact_quote: "Ödeme yapılmadı.".into(),
            })
            .expect("anchor");
        let anchor2 = db
            .create_anchor(&NewAnchor {
                case_id: case.id.clone(),
                document_version_id: version.id,
                parse_run_id: run.id,
                segment_id: segment,
                start_offset: 0,
                end_offset: 5,
                exact_quote: "Ödeme".into(),
            })
            .expect("anchor2");
        Setup {
            db,
            case_id: case.id,
            anchor_id: anchor.id,
            anchor_id2: anchor2.id,
        }
    }

    fn pending_item(s: &Setup, kind: &str, anchors: Vec<String>) -> String {
        s.db.create_review_item(
            &s.case_id,
            &NewReviewItem {
                item_kind: kind.into(),
                payload: serde_json::json!({"text": "Kira ödenmedi"}),
                source_anchor_ids: anchors,
                ai_run_id: None,
            },
        )
        .expect("öğe")
        .id
    }

    fn count(s: &Setup, sql: &str, param: &str) -> i64 {
        s.db.conn()
            .query_row(sql, [param], |r| r.get(0))
            .expect("sayım")
    }

    fn audit_total(s: &Setup) -> i64 {
        s.db.conn()
            .query_row("SELECT count(*) FROM audit_events", [], |r| r.get(0))
            .expect("denetim sayımı")
    }

    fn claim_target() -> ReviewTarget {
        ReviewTarget::Claim {
            claim_text: "Kira ödenmedi".into(),
            claim_kind: "allegation".into(),
            claimant_actor_id: None,
            source_anchor_id: None,
        }
    }

    #[test]
    fn commit_writes_target_decision_audit_and_outbox_together() {
        let s = setup();
        let item_id = pending_item(&s, "claim_candidate", vec![s.anchor_id.clone()]);
        let audit_before = audit_total(&s);

        let result =
            s.db.commit_review_decision(
                &item_id,
                "accept",
                &claim_target(),
                "avukat",
                "kaynak yeterli",
            )
            .expect("commit");

        assert_eq!(result.item.state, "accepted");
        assert_eq!(result.target_kind, "claim");
        assert_eq!(
            result.item.target_entity_id.as_deref(),
            Some(result.target_id.as_str())
        );
        // Hedef kayıt gerçekten var, onaylı durumda ve kaynağını taşıyor.
        let claims = s.db.list_claims(&s.case_id).expect("iddialar");
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].id, result.target_id);
        assert_eq!(claims[0].status, "asserted");
        assert_eq!(
            claims[0].source_anchor_id.as_deref(),
            Some(s.anchor_id.as_str())
        );
        assert_eq!(s.db.review_history(&item_id).expect("geçmiş").len(), 1);

        // Denetim zinciri: yeni kayıtlar zincire girdi, zincir doğrulandı.
        let audit_after = audit_total(&s);
        assert!(audit_after > audit_before);
        assert_eq!(
            s.db.verify_audit_chain().expect("zincir") as i64,
            audit_after
        );

        // Bildirim olayı: tek, teslim edilmemiş, yalnız kimlik taşıyor.
        let outbox =
            s.db.list_outbox_events(Some(&s.case_id), true)
                .expect("outbox");
        assert_eq!(outbox.len(), 1);
        assert_eq!(outbox[0].event_type, EVENT_DECISION_COMMITTED);
        assert_eq!(outbox[0].aggregate_id, result.target_id);
        assert!(outbox[0].delivered_at.is_none());
        assert_eq!(outbox[0].attempt_count, 0);
        let payload = outbox[0].payload.to_string();
        assert!(payload.contains(&item_id));
        assert!(
            !payload.contains("Kira ödenmedi"),
            "outbox dosya içeriği taşımamalı"
        );

        // Aynı öğe ikinci kez karara bağlanamaz: ne yeni kayıt ne yeni olay.
        assert!(s
            .db
            .commit_review_decision(&item_id, "accept", &claim_target(), "avukat", "")
            .is_err());
        assert_eq!(s.db.list_claims(&s.case_id).expect("iddialar").len(), 1);
        assert_eq!(
            s.db.list_outbox_events(Some(&s.case_id), true)
                .expect("outbox")
                .len(),
            1
        );
        assert_eq!(s.db.review_history(&item_id).expect("geçmiş").len(), 1);
    }

    #[test]
    fn failed_outbox_leaves_no_target_record_and_no_decision() {
        let s = setup();
        let item_id = pending_item(&s, "claim_candidate", vec![s.anchor_id.clone()]);
        let audit_before = audit_total(&s);
        let claims_before = count(
            &s,
            "SELECT count(*) FROM claims WHERE case_id=?1",
            &s.case_id,
        );

        // Outbox yazımını zorla düşür: hedef kayıt çoktan yazılmış olacak.
        s.db.conn()
            .execute_batch(
                "CREATE TRIGGER outbox_fail BEFORE INSERT ON outbox_events
                 BEGIN SELECT RAISE(ABORT, 'outbox kapalı'); END;",
            )
            .expect("trigger");

        let err =
            s.db.commit_review_decision(
                &item_id,
                "accept",
                &claim_target(),
                "avukat",
                "kaynak yeterli",
            )
            .expect_err("outbox düşünce commit düşmeli");
        assert!(matches!(err, StorageError::Sql(_)));

        // Hiçbir iz kalmadı: hedef kayıt, karar, denetim, outbox.
        assert_eq!(
            count(
                &s,
                "SELECT count(*) FROM claims WHERE case_id=?1",
                &s.case_id
            ),
            claims_before
        );
        assert_eq!(
            count(
                &s,
                "SELECT count(*) FROM review_decisions WHERE review_item_id=?1",
                &item_id
            ),
            0
        );
        assert_eq!(
            count(
                &s,
                "SELECT count(*) FROM outbox_events WHERE case_id=?1",
                &s.case_id
            ),
            0
        );
        assert_eq!(audit_total(&s), audit_before);
        let item = s.db.get_review_item(&item_id).expect("öğe").expect("var");
        assert_eq!(item.state, "pending");
        assert!(item.target_entity_id.is_none());

        // Trigger kaldırılınca aynı öğe temiz biçimde kabul edilebilir.
        s.db.conn()
            .execute_batch("DROP TRIGGER outbox_fail;")
            .expect("drop");
        let ok =
            s.db.commit_review_decision(&item_id, "accept", &claim_target(), "avukat", "")
                .expect("ikinci deneme");
        assert_eq!(ok.item.state, "accepted");
        assert_eq!(s.db.list_claims(&s.case_id).expect("iddialar").len(), 1);
    }

    #[test]
    fn contradiction_candidate_can_only_be_committed_as_note() {
        let s = setup();
        let item_id = pending_item(
            &s,
            "contradiction_candidate",
            vec![s.anchor_id.clone(), s.anchor_id2.clone()],
        );

        // Çelişki adayı iddia olarak kayda geçmez: iddia uydurulmaz.
        assert!(s
            .db
            .commit_review_decision(&item_id, "accept", &claim_target(), "avukat", "")
            .is_err());
        assert_eq!(s.db.list_claims(&s.case_id).expect("iddialar").len(), 0);

        let note = ReviewTarget::Note {
            title: "Çelişki notu".into(),
            body: "Ödeme beyanı iki kaynakta farklı.".into(),
        };
        let result =
            s.db.commit_review_decision(&item_id, "accept", &note, "avukat", "fark kayda geçti")
                .expect("not");
        assert_eq!(result.item.state, "accepted");
        let notes = s.db.list_notes(&s.case_id).expect("notlar");
        assert_eq!(notes.len(), 1);
        // Not iki kaynağı da taşır ve onay damgası vardır.
        assert_eq!(notes[0].source_anchor_ids.len(), 2);
        assert!(notes[0].approved_at.is_some());
    }

    #[test]
    fn commit_event_keeps_unknown_precision_without_invented_day() {
        let s = setup();
        let item_id = pending_item(&s, "event_candidate", vec![s.anchor_id.clone()]);
        let target = ReviewTarget::Event {
            event_kind: "notice".into(),
            title: "Tebligat".into(),
            date_value: None,
            date_precision: "unknown".into(),
            date_role: "notification".into(),
            date_raw: Some("tarih okunamadı".into()),
            description: String::new(),
            assertion_kind: "document".into(),
        };
        let result =
            s.db.commit_review_decision(&item_id, "accept", &target, "avukat", "")
                .expect("olay");
        let event =
            s.db.get_event(&result.target_id)
                .expect("olay okuma")
                .expect("var");
        assert_eq!(event.date_precision, "unknown");
        assert!(event.sortable_date.is_none(), "uydurulmuş gün yazılmamalı");
        assert_eq!(event.status, "confirmed");
        let sources = s.db.list_event_sources(&event.id).expect("kaynaklar");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source_anchor_id, s.anchor_id);
        assert!(sources[0].user_confirmed);
    }

    #[test]
    fn reject_decision_writes_outbox_atomically() {
        let s = setup();
        let item_id = pending_item(&s, "note_candidate", vec![s.anchor_id.clone()]);
        let decided =
            s.db.decide_review_item(&item_id, "reject", None, "avukat", "kaynak yetersiz")
                .expect("ret");
        assert_eq!(decided.state, "rejected");
        assert!(decided.target_entity_id.is_none());
        let outbox =
            s.db.list_outbox_events(Some(&s.case_id), true)
                .expect("outbox");
        assert_eq!(outbox.len(), 1);
        assert_eq!(outbox[0].event_type, EVENT_ITEM_DECIDED);
        assert!(s.db.verify_audit_chain().is_ok());
        // Ret edilen öğe hedef kayıt üretmez.
        assert_eq!(s.db.list_notes(&s.case_id).expect("notlar").len(), 0);
    }

    #[test]
    fn review_target_exists_reports_created_record() {
        let s = setup();
        let item_id = pending_item(&s, "claim_candidate", vec![s.anchor_id.clone()]);
        assert!(!s.db.review_target_exists(&item_id).expect("sorgu"));
        s.db.commit_review_decision(&item_id, "accept", &claim_target(), "avukat", "")
            .expect("commit");
        assert!(s.db.review_target_exists(&item_id).expect("sorgu"));
    }

    #[test]
    fn edit_decision_stores_target_as_edited_payload() {
        let s = setup();
        let item_id = pending_item(&s, "claim_candidate", vec![s.anchor_id.clone()]);
        let target = ReviewTarget::Claim {
            claim_text: "Kira iki aydır ödenmedi".into(),
            claim_kind: "defense".into(),
            claimant_actor_id: None,
            source_anchor_id: None,
        };
        let result =
            s.db.commit_review_decision(&item_id, "edit", &target, "avukat", "tarih eklendi")
                .expect("düzenleme");
        assert_eq!(result.item.state, "edited");
        let history = s.db.review_history(&item_id).expect("geçmiş");
        assert_eq!(history.len(), 1);
        let edited = history[0]
            .edited_payload
            .clone()
            .expect("düzenlenmiş içerik");
        assert_eq!(edited["kind"], "claim");
        assert_eq!(edited["claimText"], "Kira iki aydır ödenmedi");
    }

    /// Hedef kaydın kendi doğrulaması düşerse karar da yazılmaz.
    #[test]
    fn invalid_target_keeps_item_pending() {
        let s = setup();
        let item_id = pending_item(&s, "claim_candidate", vec![s.anchor_id.clone()]);
        let audit_before = audit_total(&s);
        let bad = ReviewTarget::Claim {
            claim_text: "   ".into(),
            claim_kind: "allegation".into(),
            claimant_actor_id: None,
            source_anchor_id: None,
        };
        assert!(s
            .db
            .commit_review_decision(&item_id, "accept", &bad, "avukat", "")
            .is_err());
        let item = s.db.get_review_item(&item_id).expect("öğe").expect("var");
        assert_eq!(item.state, "pending");
        assert_eq!(
            count(
                &s,
                "SELECT count(*) FROM review_decisions WHERE review_item_id=?1",
                &item_id
            ),
            0
        );
        assert_eq!(
            count(
                &s,
                "SELECT count(*) FROM outbox_events WHERE case_id=?1",
                &s.case_id
            ),
            0
        );
        assert_eq!(audit_total(&s), audit_before);
    }

    /// `decide_review_item` da outbox'ı aynı işlemde yazar; başarısız outbox
    /// kararı bırakmaz.
    #[test]
    fn failed_outbox_rolls_back_plain_decision() {
        let s = setup();
        let item_id = pending_item(&s, "note_candidate", vec![s.anchor_id.clone()]);
        let audit_before = audit_total(&s);
        s.db.conn()
            .execute_batch(
                "CREATE TRIGGER outbox_fail BEFORE INSERT ON outbox_events
                 BEGIN SELECT RAISE(ABORT, 'outbox kapalı'); END;",
            )
            .expect("trigger");
        assert!(s
            .db
            .decide_review_item(&item_id, "reject", None, "avukat", "")
            .is_err());
        let item = s.db.get_review_item(&item_id).expect("öğe").expect("var");
        assert_eq!(item.state, "pending");
        assert_eq!(
            count(
                &s,
                "SELECT count(*) FROM review_decisions WHERE review_item_id=?1",
                &item_id
            ),
            0
        );
        assert_eq!(audit_total(&s), audit_before);
    }
}
