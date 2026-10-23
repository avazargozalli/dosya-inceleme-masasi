//! Dosya (case) kayıtları: oluşturma, listeleme, güncelleme, arşiv ve
//! geri dönüşümlü silme. Her yazma denetim zincirine işlenir.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseKind {
    Review,
    ReferenceArchive,
}

impl CaseKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::ReferenceArchive => "reference_archive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    Active,
    Archived,
    Deleted,
}

impl CaseStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Deleted => "deleted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    Normal,
    High,
    ClientConfidential,
}

impl Sensitivity {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::High => "high",
            Self::ClientConfidential => "client_confidential",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiPolicy {
    Off,
    LocalOnly,
    CliAllowed,
    ApiAllowed,
}

impl AiPolicy {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::LocalOnly => "local_only",
            Self::CliAllowed => "cli_allowed",
            Self::ApiAllowed => "api_allowed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Case {
    pub id: String,
    pub title: String,
    pub case_kind: String,
    pub reference_code: Option<String>,
    pub jurisdiction_label: Option<String>,
    pub practice_area: Option<String>,
    pub status: String,
    pub sensitivity: String,
    pub default_ai_policy: String,
    pub archived_at: Option<String>,
    pub retention_hold: bool,
    pub created_at: String,
    pub updated_at: String,
    pub row_version: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCase {
    pub title: String,
    #[serde(default = "default_kind")]
    pub case_kind: CaseKind,
    pub reference_code: Option<String>,
    pub jurisdiction_label: Option<String>,
    pub practice_area: Option<String>,
    #[serde(default = "default_sensitivity")]
    pub sensitivity: Sensitivity,
    #[serde(default = "default_policy")]
    pub default_ai_policy: AiPolicy,
}

fn default_kind() -> CaseKind {
    CaseKind::Review
}
fn default_sensitivity() -> Sensitivity {
    Sensitivity::Normal
}
fn default_policy() -> AiPolicy {
    AiPolicy::Off
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CasePatch {
    pub title: Option<String>,
    pub reference_code: Option<String>,
    pub jurisdiction_label: Option<String>,
    pub practice_area: Option<String>,
    pub sensitivity: Option<Sensitivity>,
    pub default_ai_policy: Option<AiPolicy>,
    pub retention_hold: Option<bool>,
}

impl NewCase {
    fn validate(&self) -> Result<(), StorageError> {
        let title = self.title.trim();
        if title.is_empty() || title.chars().count() > 200 {
            return Err(StorageError::InvalidSetting);
        }
        Ok(())
    }
}

impl Database {
    pub fn create_case(&self, new: &NewCase) -> Result<Case, StorageError> {
        new.validate()?;
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO cases (id,title,case_kind,reference_code,jurisdiction_label,practice_area,status,sensitivity,default_ai_policy,retention_hold,created_at,updated_at,row_version)
             VALUES (?1,?2,?3,?4,?5,?6,'active',?7,?8,0,?9,?9,1)",
            rusqlite::params![
                id,
                new.title.trim(),
                new.case_kind.as_str(),
                new.reference_code.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                new.jurisdiction_label.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                new.practice_area.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                new.sensitivity.as_str(),
                new.default_ai_policy.as_str(),
                now,
            ],
        )?;
        self.append_audit("case", Some(&id), "create", "local-user", &serde_json::json!({"kind": new.case_kind.as_str()}))?;
        self.get_case(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn list_cases(&self, include_archived: bool) -> Result<Vec<Case>, StorageError> {
        let sql = if include_archived {
            "SELECT id,title,case_kind,reference_code,jurisdiction_label,practice_area,status,sensitivity,default_ai_policy,archived_at,retention_hold,created_at,updated_at,row_version
             FROM cases WHERE deleted_at IS NULL ORDER BY updated_at DESC"
        } else {
            "SELECT id,title,case_kind,reference_code,jurisdiction_label,practice_area,status,sensitivity,default_ai_policy,archived_at,retention_hold,created_at,updated_at,row_version
             FROM cases WHERE deleted_at IS NULL AND status = 'active' ORDER BY updated_at DESC"
        };
        let mut stmt = self.conn().prepare(sql)?;
        let rows = stmt.query_map([], map_case)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_case(&self, id: &str) -> Result<Option<Case>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,title,case_kind,reference_code,jurisdiction_label,practice_area,status,sensitivity,default_ai_policy,archived_at,retention_hold,created_at,updated_at,row_version
             FROM cases WHERE id = ?1",
        )?;
        Ok(stmt.query_row([id], map_case).optional()?)
    }

    /// İyimser eşzamanlılık: `expected_row_version` uyuşmazsa `Conflict` döner,
    /// eski görünüm yeni kaydın üzerine yazılmaz (13.3).
    pub fn update_case(&self, id: &str, patch: &CasePatch, expected_row_version: i64) -> Result<Case, StorageError> {
        let mut current = self.get_case(id)?.ok_or(StorageError::NotFound)?;
        if current.row_version != expected_row_version {
            return Err(StorageError::Conflict { expected: expected_row_version, actual: current.row_version });
        }
        if let Some(title) = &patch.title {
            let trimmed = title.trim();
            if trimmed.is_empty() || trimmed.chars().count() > 200 {
                return Err(StorageError::InvalidSetting);
            }
            current.title = trimmed.to_string();
        }
        if let Some(v) = &patch.reference_code {
            current.reference_code = Some(v.trim().to_string()).filter(|s| !s.is_empty());
        }
        if let Some(v) = &patch.jurisdiction_label {
            current.jurisdiction_label = Some(v.trim().to_string()).filter(|s| !s.is_empty());
        }
        if let Some(v) = &patch.practice_area {
            current.practice_area = Some(v.trim().to_string()).filter(|s| !s.is_empty());
        }
        if let Some(v) = patch.sensitivity {
            current.sensitivity = v.as_str().to_string();
        }
        if let Some(v) = patch.default_ai_policy {
            current.default_ai_policy = v.as_str().to_string();
        }
        if let Some(v) = patch.retention_hold {
            current.retention_hold = v;
        }
        current.updated_at = now_iso();
        current.row_version += 1;

        let changed = self.conn().execute(
            "UPDATE cases SET title=?1, reference_code=?2, jurisdiction_label=?3, practice_area=?4,
                    sensitivity=?5, default_ai_policy=?6, retention_hold=?7, updated_at=?8, row_version=?9
             WHERE id=?10 AND row_version=?11",
            rusqlite::params![
                current.title,
                current.reference_code,
                current.jurisdiction_label,
                current.practice_area,
                current.sensitivity,
                current.default_ai_policy,
                i64::from(current.retention_hold),
                current.updated_at,
                current.row_version,
                id,
                expected_row_version,
            ],
        )?;
        if changed == 0 {
            return Err(StorageError::Conflict { expected: expected_row_version, actual: -1 });
        }
        self.append_audit("case", Some(id), "update", "local-user", &serde_json::json!({"rowVersion": current.row_version}))?;
        Ok(current)
    }

    pub fn archive_case(&self, id: &str) -> Result<Case, StorageError> {
        let now = now_iso();
        let changed = self.conn().execute(
            "UPDATE cases SET status='archived', archived_at=?1, updated_at=?1, row_version=row_version+1
             WHERE id=?2 AND status='active'",
            rusqlite::params![now, id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit("case", Some(id), "archive", "local-user", &serde_json::json!({}))?;
        self.get_case(id)?.ok_or(StorageError::NotFound)
    }

    /// Geri dönüşümlü silme: kayıt korunur, listede görünmez. Saklama kilidi
    /// (`retention_hold`) varsa silme reddedilir.
    pub fn soft_delete_case(&self, id: &str) -> Result<(), StorageError> {
        let case = self.get_case(id)?.ok_or(StorageError::NotFound)?;
        if case.retention_hold {
            return Err(StorageError::InvalidSetting);
        }
        let now = now_iso();
        self.conn().execute(
            "UPDATE cases SET status='deleted', deleted_at=?1, updated_at=?1, row_version=row_version+1 WHERE id=?2",
            rusqlite::params![now, id],
        )?;
        self.append_audit("case", Some(id), "soft_delete", "local-user", &serde_json::json!({}))?;
        Ok(())
    }

    pub fn restore_case(&self, id: &str) -> Result<Case, StorageError> {
        let now = now_iso();
        let changed = self.conn().execute(
            "UPDATE cases SET status='active', deleted_at=NULL, archived_at=NULL, updated_at=?1, row_version=row_version+1
             WHERE id=?2 AND status IN ('deleted','archived')",
            rusqlite::params![now, id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit("case", Some(id), "restore", "local-user", &serde_json::json!({}))?;
        self.get_case(id)?.ok_or(StorageError::NotFound)
    }
}

fn map_case(row: &Row<'_>) -> rusqlite::Result<Case> {
    Ok(Case {
        id: row.get(0)?,
        title: row.get(1)?,
        case_kind: row.get(2)?,
        reference_code: row.get(3)?,
        jurisdiction_label: row.get(4)?,
        practice_area: row.get(5)?,
        status: row.get(6)?,
        sensitivity: row.get(7)?,
        default_ai_policy: row.get(8)?,
        archived_at: row.get(9)?,
        retention_hold: row.get::<_, i64>(10)? != 0,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        row_version: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseKey;

    fn db() -> Database {
        Database::open_in_memory(&DatabaseKey::new([3u8; 32])).expect("db")
    }

    fn new_case(title: &str) -> NewCase {
        NewCase {
            title: title.to_string(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: AiPolicy::Off,
        }
    }

    #[test]
    fn create_and_list_case() {
        let db = db();
        let case = db.create_case(&new_case("Kira davası")).expect("oluştur");
        assert_eq!(case.status, "active");
        assert_eq!(case.row_version, 1);

        let list = db.list_cases(false).expect("liste");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Kira davası");
    }

    #[test]
    fn empty_title_rejected() {
        let db = db();
        assert!(db.create_case(&new_case("   ")).is_err());
    }

    #[test]
    fn stale_row_version_is_conflict() {
        let db = db();
        let case = db.create_case(&new_case("Kira")).expect("oluştur");
        let patch = CasePatch { title: Some("Kira II".into()), ..Default::default() };
        let updated = db.update_case(&case.id, &patch, 1).expect("güncelle");
        assert_eq!(updated.row_version, 2);

        let err = db.update_case(&case.id, &patch, 1).expect_err("çakışma");
        assert_eq!(err.code(), "E_ROW_VERSION");
    }

    #[test]
    fn archive_hides_from_active_list_but_keeps_row() {
        let db = db();
        let case = db.create_case(&new_case("Arşiv")).expect("oluştur");
        db.archive_case(&case.id).expect("arşivle");
        assert!(db.list_cases(false).expect("liste").is_empty());
        assert_eq!(db.list_cases(true).expect("liste").len(), 1);
        assert_eq!(db.get_case(&case.id).expect("get").expect("var").status, "archived");
    }

    #[test]
    fn soft_delete_and_restore() {
        let db = db();
        let case = db.create_case(&new_case("Sil")).expect("oluştur");
        db.soft_delete_case(&case.id).expect("sil");
        assert!(db.list_cases(true).expect("liste").is_empty());
        db.restore_case(&case.id).expect("geri al");
        assert_eq!(db.list_cases(false).expect("liste").len(), 1);
    }

    #[test]
    fn retention_hold_blocks_delete() {
        let db = db();
        let case = db.create_case(&new_case("Saklama")).expect("oluştur");
        db.update_case(&case.id, &CasePatch { retention_hold: Some(true), ..Default::default() }, 1).expect("güncelle");
        assert!(db.soft_delete_case(&case.id).is_err());
    }

    #[test]
    fn audit_chain_records_each_mutation() {
        let db = db();
        let case = db.create_case(&new_case("Denetim")).expect("oluştur");
        db.archive_case(&case.id).expect("arşivle");
        assert_eq!(db.verify_audit_chain().expect("zincir"), 2);
    }
}
