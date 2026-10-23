//! Belge etiketleri: kullanıcı tarafından verilen kısa serbest etiketler.
//!
//! Etiket bir **belgeye** bağlıdır, segment ya da sürüme değil: yeni sürüm
//! geldiğinde etiket belgede kalır (F07-T03 arama filtresi, belge listesi).
//! Kanonik biçim tek yerde (`normalize_tag`) üretilir; arama filtresi de aynı
//! fonksiyonu kullanır, böylece "Kira" ile "kira" ayrı etiket olmaz.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

pub const MAX_TAG_CHARS: usize = 40;
/// Tek aramada en çok bu kadar etiketle filtre kurulabilir (SQL parametre sınırı
/// ve arayüz karmaşıklığı için).
pub const MAX_FILTER_TAGS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTagRow {
    pub document_id: String,
    pub case_id: String,
    pub tag: String,
    pub created_at: String,
}

/// Dosya genelinde etiket listesi: filtre çubuğunda öneri olarak gösterilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseTagRow {
    pub tag: String,
    pub document_count: i64,
}

/// Etiketi kanonikleştirir: boşluklar sadeleşir, küçük harfe çevrilir.
///
/// Türkçe `İ` küçültmesi `i` + birleşik nokta üretir; birleşik nokta atılmazsa
/// aynı etiket iki farklı bayt dizisi olur ve filtre eşleşmez. Boş ya da çok
/// uzun etiket `None` döner — sessizce kırpılmaz.
pub fn normalize_tag(raw: &str) -> Option<String> {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    let lowered = collapsed.to_lowercase().replace('\u{0307}', "");
    if lowered.chars().count() > MAX_TAG_CHARS {
        return None;
    }
    if lowered.chars().any(char::is_control) {
        return None;
    }
    Some(lowered)
}

impl Database {
    /// Etiketi belgeye ekler. Aynı etiket ikinci kez eklenmez ve ikinci bir
    /// denetim kaydı üretmez; işlem yine de gerçek belgeyi doğrular.
    pub fn add_document_tag(
        &self,
        document_id: &str,
        tag: &str,
    ) -> Result<DocumentTagRow, StorageError> {
        let normalized = normalize_tag(tag).ok_or(StorageError::InvalidTag)?;
        let case_id: String = self
            .conn()
            .query_row(
                "SELECT case_id FROM documents WHERE id=?1 AND deleted_at IS NULL",
                [document_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StorageError::NotFound)?;

        let existing: Option<String> = self
            .conn()
            .query_row(
                "SELECT created_at FROM document_tags WHERE document_id=?1 AND tag=?2",
                rusqlite::params![document_id, normalized],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(created_at) = existing {
            return Ok(DocumentTagRow {
                document_id: document_id.to_string(),
                case_id,
                tag: normalized,
                created_at,
            });
        }

        let now = now_iso();
        self.conn().execute(
            "INSERT INTO document_tags (document_id,case_id,tag,created_at) VALUES (?1,?2,?3,?4)",
            rusqlite::params![document_id, case_id, normalized, now],
        )?;
        self.append_audit(
            "document",
            Some(document_id),
            "tag_add",
            "local-user",
            &serde_json::json!({"tag": normalized}),
        )?;
        Ok(DocumentTagRow {
            document_id: document_id.to_string(),
            case_id,
            tag: normalized,
            created_at: now,
        })
    }

    /// Etiketi kaldırır. Etiket yoksa `false` döner; olmayan etiket için denetim
    /// kaydı yazılmaz.
    pub fn remove_document_tag(&self, document_id: &str, tag: &str) -> Result<bool, StorageError> {
        let normalized = normalize_tag(tag).ok_or(StorageError::InvalidTag)?;
        let removed = self.conn().execute(
            "DELETE FROM document_tags WHERE document_id=?1 AND tag=?2",
            rusqlite::params![document_id, normalized],
        )?;
        if removed == 0 {
            return Ok(false);
        }
        self.append_audit(
            "document",
            Some(document_id),
            "tag_remove",
            "local-user",
            &serde_json::json!({"tag": normalized}),
        )?;
        Ok(true)
    }

    pub fn list_document_tags(&self, document_id: &str) -> Result<Vec<String>, StorageError> {
        let mut stmt = self
            .conn()
            .prepare("SELECT tag FROM document_tags WHERE document_id=?1 ORDER BY tag")?;
        let rows = stmt.query_map([document_id], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Dosyadaki bütün belge etiketleri (belge kimliğiyle); liste ekranı tek
    /// sorguda doldurur.
    pub fn list_case_document_tags(
        &self,
        case_id: &str,
    ) -> Result<Vec<DocumentTagRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT document_id, case_id, tag, created_at FROM document_tags
             WHERE case_id=?1 ORDER BY document_id, tag",
        )?;
        let rows = stmt.query_map([case_id], |row| {
            Ok(DocumentTagRow {
                document_id: row.get(0)?,
                case_id: row.get(1)?,
                tag: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Dosya genelinde etiket önerileri: etiket ve kaç belgede geçtiği.
    pub fn list_case_tags(&self, case_id: &str) -> Result<Vec<CaseTagRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT tag, count(*) FROM document_tags WHERE case_id=?1 GROUP BY tag ORDER BY tag",
        )?;
        let rows = stmt.query_map([case_id], |row| {
            Ok(CaseTagRow {
                tag: row.get(0)?,
                document_count: row.get(1)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};

    fn setup() -> (Database, String, String) {
        let db = Database::open_in_memory(&crate::DatabaseKey::new([31u8; 32])).expect("db");
        let case = db
            .create_case(&NewCase {
                title: "Etiket".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        let doc = db
            .create_document(&case.id, "Belge", "pdf", None, "unknown")
            .expect("belge");
        (db, case.id, doc.id)
    }

    #[test]
    fn tag_normalization_collapses_case_and_turkish_dot() {
        assert_eq!(
            normalize_tag("  Kira   Sözleşmesi ").as_deref(),
            Some("kira sözleşmesi")
        );
        assert_eq!(normalize_tag("İHTAR").as_deref(), Some("ihtar"));
        assert_eq!(normalize_tag("   "), None);
        assert_eq!(normalize_tag(&"a".repeat(MAX_TAG_CHARS + 1)), None);
    }

    #[test]
    fn add_is_idempotent_and_case_scoped() {
        let (db, case_id, doc_id) = setup();
        let first = db.add_document_tag(&doc_id, "Kira").expect("ekle");
        assert_eq!(first.tag, "kira");
        assert_eq!(first.case_id, case_id);
        let second = db.add_document_tag(&doc_id, "kira").expect("tekrar");
        assert_eq!(second.created_at, first.created_at);

        let tags = db.list_document_tags(&doc_id).expect("liste");
        assert_eq!(tags, vec!["kira".to_string()]);
        assert_eq!(
            db.list_case_tags(&case_id).expect("dosya")[0].document_count,
            1
        );

        assert!(db.remove_document_tag(&doc_id, "KİRA").expect("kaldır"));
        assert!(!db.remove_document_tag(&doc_id, "kira").expect("yok"));
        assert!(db.list_document_tags(&doc_id).expect("liste").is_empty());
    }

    #[test]
    fn unknown_document_is_not_tagged() {
        let (db, _case_id, _doc_id) = setup();
        assert!(matches!(
            db.add_document_tag("yok", "kira"),
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            db.add_document_tag("yok-boş", "   "),
            Err(StorageError::InvalidTag)
        ));
    }
}
