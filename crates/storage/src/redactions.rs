//! Karartma planları. Plan, çıktının hangi kaynak sürüme ve hangi render
//! sürümüne ait olduğunu sabitler; başarısız doğrulama planı `applied` yapmaz.
//!
//! Plan ve bölgeler **denetim sicili**dir: her maskelemede yazılır. Okuma
//! yolu (`list_redaction_plans` / `list_redaction_regions`) satırı belge adı ve
//! sürüm numarasıyla birlikte verir; avukat "hangi belgede hangi sayfalarda
//! maskeleme yapıldı" sorusunu kasadan yanıtlayabilir.

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
    /// Belgenin okunabilir adı (`documents.display_name`). Sürüm/belge
    /// bulunamazsa `None`: kimlik dururken **ad uydurulmaz**. `redaction_plans`
    /// satırı sürüme yabancı anahtarla bağlı değil — denetim satırı yaşar.
    pub document_title: Option<String>,
    /// Sürüm numarası (`document_versions.version_no`); bulunamazsa `None`.
    pub version_no: Option<i64>,
    /// Bu plana yazılmış bölge sayısı (tek sorguda alt sorgu).
    pub region_count: i64,
}

/// Planın bölge satırı — **okuma** modeli. `quads` alanı yazılırken JSON
/// dizisi olarak saklanır; bozuk JSON `None` döner (geometri uydurulmaz),
/// satır yine listelenir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactionRegionRow {
    pub id: String,
    pub page_index: i64,
    pub rects: Option<Vec<[f64; 4]>>,
    pub coordinate_space: String,
    pub category: String,
    pub detected_text_hash: Option<String>,
    pub user_decision: String,
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
            "SELECT p.id,p.case_id,p.document_version_id,p.parse_run_id,p.render_hash,p.coordinate_space,p.plan_version,p.state,p.reviewer,p.output_profile,p.created_at,p.updated_at,
                    d.display_name,v.version_no,
                    (SELECT COUNT(*) FROM redaction_regions rr WHERE rr.plan_id=p.id)
             FROM redaction_plans p
             LEFT JOIN document_versions v ON v.id = p.document_version_id
             LEFT JOIN documents d ON d.id = v.document_id
             WHERE p.id=?1",
        )?;
        Ok(stmt.query_row([id], map_plan).optional()?)
    }

    /// Dosyanın maskeleme planları — en yeni önce. Sıra `created_at DESC,
    /// rowid DESC`: aynı damgaya düşen iki plan bile yazılma sırasını korur.
    pub fn list_redaction_plans(
        &self,
        case_id: &str,
    ) -> Result<Vec<RedactionPlanRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT p.id,p.case_id,p.document_version_id,p.parse_run_id,p.render_hash,p.coordinate_space,p.plan_version,p.state,p.reviewer,p.output_profile,p.created_at,p.updated_at,
                    d.display_name,v.version_no,
                    (SELECT COUNT(*) FROM redaction_regions rr WHERE rr.plan_id=p.id)
             FROM redaction_plans p
             LEFT JOIN document_versions v ON v.id = p.document_version_id
             LEFT JOIN documents d ON d.id = v.document_id
             WHERE p.case_id=?1 ORDER BY p.created_at DESC, p.rowid DESC",
        )?;
        let rows = stmt.query_map([case_id], map_plan)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Planın bölgeleri — **yazılma sırası** (`rowid ASC`): aynı sayfadaki
    /// bölgeler bile seçildikleri sırayı korur.
    pub fn list_redaction_regions(
        &self,
        plan_id: &str,
    ) -> Result<Vec<RedactionRegionRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,page_index,quads,coordinate_space,category,detected_text_hash,user_decision
             FROM redaction_regions WHERE plan_id=?1 ORDER BY rowid ASC",
        )?;
        let rows = stmt.query_map([plan_id], |row| {
            let quads: String = row.get(2)?;
            Ok(RedactionRegionRow {
                id: row.get(0)?,
                page_index: row.get(1)?,
                // Bozuk `quads` **gizlenmez**: `None` "geometri okunamadı"
                // demektir; bölge satırı yine listelenir (sayım kaybolmaz).
                rects: serde_json::from_str::<Vec<[f64; 4]>>(&quads).ok(),
                coordinate_space: row.get(3)?,
                category: row.get(4)?,
                detected_text_hash: row.get(5)?,
                user_decision: row.get(6)?,
            })
        })?;
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
        document_title: row.get(12)?,
        version_no: row.get(13)?,
        region_count: row.get(14)?,
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
    fn plan_list_carries_document_title_version_and_region_count() {
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
        // Bölgeler yazılmadan önce plan satırı da okunabilir; sayı 0'dır.
        let before = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].region_count, 0);
        assert_eq!(before[0].document_title.as_deref(), Some("Belge"));
        assert_eq!(before[0].version_no, Some(1));

        db.add_redaction_regions(
            &plan.id,
            &[
                RedactionRegionInput {
                    page_index: 0,
                    rect: [1.0, 2.0, 3.0, 4.0],
                    term_hash: "h1".into(),
                },
                RedactionRegionInput {
                    page_index: 2,
                    rect: [5.0, 6.0, 7.0, 8.0],
                    term_hash: "h2".into(),
                },
            ],
        )
        .expect("bölge");

        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows.len(), 1);
        // Ad/sürüm **sorgudan** okunur, satırda saklanmaz: plan hangi belgeye
        // aitse onu gösterir.
        assert_eq!(rows[0].document_title.as_deref(), Some("Belge"));
        assert_eq!(rows[0].version_no, Some(1));
        assert_eq!(rows[0].region_count, 2);
        assert_eq!(rows[0].state, "applied");
    }

    #[test]
    fn plan_list_keeps_row_when_version_is_missing() {
        let (db, case_id, version_id) = setup();
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id,
                    parse_run_id: None,
                    render_hash: "c".repeat(64),
                    output_profile: "share".into(),
                },
            )
            .expect("plan");
        // Sürüm bağı yabancı anahtar değil: satır silinse/taşınsa bile denetim
        // kaydı **düşmez**, ad uydurulmaz (None).
        db.conn()
            .execute(
                "UPDATE redaction_plans SET document_version_id='yok' WHERE id=?1",
                [&plan.id],
            )
            .expect("güncelle");
        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].document_title.is_none());
        assert!(rows[0].version_no.is_none());
        assert_eq!(rows[0].region_count, 0);
    }

    #[test]
    fn plans_with_equal_timestamp_keep_insertion_order() {
        let (db, case_id, version_id) = setup();
        let mut ids = Vec::new();
        for hash in ["d", "e"] {
            ids.push(
                db.create_redaction_plan(
                    &case_id,
                    &NewRedactionPlan {
                        document_version_id: version_id.clone(),
                        parse_run_id: None,
                        render_hash: hash.repeat(64),
                        output_profile: "share".into(),
                    },
                )
                .expect("plan")
                .id,
            );
        }
        // Damga eşitlenince sıra tek başına `rowid`e düşer: sonra yazılan üstte.
        db.conn()
            .execute(
                "UPDATE redaction_plans SET created_at='2026-01-01T00:00:00.000000Z' \
                 WHERE case_id=?1",
                [&case_id],
            )
            .expect("damga");
        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, ids[1]);
        assert_eq!(rows[1].id, ids[0]);
    }

    #[test]
    fn regions_list_in_insertion_order_and_bad_quads_are_reported() {
        let (db, case_id, version_id) = setup();
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id,
                    parse_run_id: None,
                    render_hash: "f".repeat(64),
                    output_profile: "share".into(),
                },
            )
            .expect("plan");
        db.add_redaction_regions(
            &plan.id,
            &[
                RedactionRegionInput {
                    page_index: 3,
                    rect: [1.0, 2.0, 3.0, 4.0],
                    term_hash: "h1".into(),
                },
                RedactionRegionInput {
                    page_index: 1,
                    rect: [5.0, 6.0, 7.0, 8.0],
                    term_hash: "h2".into(),
                },
                RedactionRegionInput {
                    page_index: 3,
                    rect: [9.0, 10.0, 11.0, 12.0],
                    term_hash: "h3".into(),
                },
            ],
        )
        .expect("bölge");

        let rows = db.list_redaction_regions(&plan.id).expect("bölgeler");
        // Sıra **yazılma** sırasıdır (sayfaya göre sıralanmaz): seçim sırası
        // denetim bilgisidir.
        assert_eq!(
            rows.iter().map(|r| r.page_index).collect::<Vec<_>>(),
            vec![3, 1, 3]
        );
        assert_eq!(rows[0].rects.as_deref(), Some(&[[1.0, 2.0, 3.0, 4.0]][..]));
        assert_eq!(rows[0].user_decision, "redact");
        assert_eq!(rows[0].category, "user_term");
        assert_eq!(rows[0].coordinate_space, "pdf_page_points");

        // Bozuk geometri gizlenmez: satır durur, `rects` `None` olur.
        db.conn()
            .execute(
                "UPDATE redaction_regions SET quads='bozuk' WHERE page_index=1",
                [],
            )
            .expect("boz");
        let rows = db.list_redaction_regions(&plan.id).expect("bölgeler");
        assert_eq!(rows.len(), 3);
        assert!(rows[1].rects.is_none());
        assert!(rows[0].rects.is_some());
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
