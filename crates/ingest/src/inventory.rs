//! Belge envanteri (F01-T06): yapı, ekler ve değişiklik izi.
//!
//! Envanter **paket düzeyinde** çıkarılır ve gövde metnini taşımaz: yalnız
//! sayılar, eklerin adı/boyutu/türü, değişiklik sayısı/yazarı/tarihi. Bu
//! yüzden çıktı arayüzde gösterilebilir, denetime yalnız sayıları yazılır.
//!
//! Ayrıştırma tek yerde (`parse_inventory`): worker yanıtı eksik alanla
//! gelirse (eski sürüm) alanlar **varsayılana** düşer, ama `kind` ve `structure`
//! zorunludur — "envanter çıkarıldı" iddiası boş yanıtla kurulmaz.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::worker::WorkerError;

/// Envanteri çıkarılabilen türler. Başka MIME için worker açık hata döner.
pub const SUPPORTED_KINDS: [&str; 2] = ["docx", "udf"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryAttachment {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub detail: Option<String>,
}

/// DOCX/UDF değişiklik izi. UDF'te izlenen değişiklik kavramı yoktur; alanlar
/// sıfır/`null` kalır ve arayüz bunu yazar (uydurulmuş değer yok).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeInventory {
    #[serde(default)]
    pub insertions: u32,
    #[serde(default)]
    pub deletions: u32,
    #[serde(default)]
    pub comments: u32,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub first_date: Option<String>,
    #[serde(default)]
    pub last_date: Option<String>,
    #[serde(default)]
    pub revision: Option<String>,
    #[serde(default)]
    pub last_modified_by: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub modified: Option<String>,
    /// `None` = belge/kaynak bu alanı bildirmiyor (UDF); `Some(false)` = kapalı.
    #[serde(default)]
    pub track_revisions: Option<bool>,
    #[serde(default)]
    pub protection: Option<String>,
    #[serde(default)]
    pub signatures: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInventory {
    pub kind: String,
    #[serde(default)]
    pub structure: BTreeMap<String, u64>,
    #[serde(default)]
    pub attachments: Vec<InventoryAttachment>,
    #[serde(default)]
    pub attachment_count: u32,
    #[serde(default)]
    pub attachment_bytes: u64,
    #[serde(default)]
    pub changes: ChangeInventory,
    #[serde(default)]
    pub external_links: u32,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub truncated: bool,
}

impl DocumentInventory {
    /// Denetime yazılacak **yalnız sayılar**: dosya adı, metin ve yol girmez.
    #[must_use]
    pub fn audit_counts(&self) -> Value {
        serde_json::json!({
            "kind": self.kind,
            "attachments": self.attachment_count,
            "attachmentBytes": self.attachment_bytes,
            "structure": self.structure,
            "insertions": self.changes.insertions,
            "deletions": self.changes.deletions,
            "comments": self.changes.comments,
            "signatures": self.changes.signatures,
            "externalLinks": self.external_links,
            "truncated": self.truncated,
        })
    }

    /// Arayüzün tek satır özeti için toplam öğe sayısı (yapı + ekler).
    #[must_use]
    pub fn total_items(&self) -> u64 {
        self.structure.values().copied().sum::<u64>() + u64::from(self.attachment_count)
    }
}

fn count(value: Option<&Value>) -> u32 {
    value
        .and_then(Value::as_u64)
        .and_then(|raw| u32::try_from(raw).ok())
        .unwrap_or(0)
}

fn text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
}

fn attachment_name_is_safe(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.split('/').any(|segment| segment == "..")
}

/// Worker'ın `inventory` yanıtını çözer.
///
/// Zorunlu: `kind` (docx|udf) ve `structure` nesnesi. Ek adı paket içi göreli
/// yol olmalıdır; `..`, `\` veya mutlak yol içeren ad **reddedilir** (arayüzde
/// yol gösterimi karışmasın, dosya sistemi yolu sanılmasın).
pub fn parse_inventory(value: &Value) -> Result<DocumentInventory, WorkerError> {
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or(WorkerError::BadResponse)?
        .to_string();
    if !SUPPORTED_KINDS.contains(&kind.as_str()) {
        return Err(WorkerError::BadResponse);
    }
    let structure_value = value
        .get("structure")
        .ok_or(WorkerError::BadResponse)?
        .as_object()
        .ok_or(WorkerError::BadResponse)?;
    let mut structure = BTreeMap::new();
    for (key, raw) in structure_value {
        let number = raw.as_u64().ok_or(WorkerError::BadResponse)?;
        structure.insert(key.clone(), number);
    }

    let mut attachments = Vec::new();
    if let Some(list) = value.get("attachments") {
        let items = list.as_array().ok_or(WorkerError::BadResponse)?;
        for item in items {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .ok_or(WorkerError::BadResponse)?;
            if !attachment_name_is_safe(name) {
                return Err(WorkerError::BadResponse);
            }
            let item_kind = item
                .get("kind")
                .and_then(Value::as_str)
                .ok_or(WorkerError::BadResponse)?;
            attachments.push(InventoryAttachment {
                name: name.to_string(),
                kind: item_kind.to_string(),
                size: item.get("size").and_then(Value::as_u64).unwrap_or(0),
                detail: text(item.get("detail")),
            });
        }
    }

    let changes_value = value.get("changes").cloned().unwrap_or(Value::Null);
    let changes: ChangeInventory = if changes_value.is_null() {
        ChangeInventory::default()
    } else {
        serde_json::from_value(changes_value).map_err(|_| WorkerError::BadResponse)?
    };

    let notes: Vec<String> = match value.get("notes") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect(),
        Some(_) => return Err(WorkerError::BadResponse),
        None => Vec::new(),
    };

    let attachment_count = count(value.get("attachmentCount"))
        .max(u32::try_from(attachments.len()).unwrap_or(u32::MAX));

    Ok(DocumentInventory {
        kind,
        structure,
        attachments,
        attachment_count,
        attachment_bytes: value
            .get("attachmentBytes")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        changes,
        external_links: count(value.get("externalLinks")),
        notes,
        truncated: value
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload() -> Value {
        json!({
            "kind": "docx",
            "structure": {"paragraphs": 9, "tables": 1, "textBoxes": 1},
            "attachments": [
                {"name": "word/embeddings/oleObject1.bin", "kind": "ole_object", "size": 84,
                 "detail": "Excel.Sheet.12"},
                {"name": "word/media/image1.png", "kind": "image", "size": 82}
            ],
            "attachmentCount": 2,
            "attachmentBytes": 166,
            "changes": {
                "insertions": 1, "deletions": 1, "comments": 1,
                "authors": ["Av. Deneme", "Karsi Taraf"],
                "firstDate": "2026-10-01T09:00:00Z",
                "lastDate": "2026-10-02T09:00:00Z",
                "revision": "12",
                "lastModifiedBy": "Av. Karsi",
                "creator": "Av. Deneme",
                "created": "2026-09-01T09:00:00Z",
                "modified": "2026-10-02T10:00:00Z",
                "trackRevisions": true,
                "protection": "readOnly",
                "signatures": 1
            },
            "externalLinks": 1,
            "notes": ["1 gömülü nesne: içeriği çıkarılmıyor"],
            "truncated": false
        })
    }

    #[test]
    fn parses_a_full_inventory() {
        let inventory = parse_inventory(&payload()).expect("yanıt çözülmeli");
        assert_eq!(inventory.kind, "docx");
        assert_eq!(inventory.structure["paragraphs"], 9);
        assert_eq!(inventory.structure["textBoxes"], 1);
        assert_eq!(inventory.attachments.len(), 2);
        assert_eq!(inventory.attachments[0].kind, "ole_object");
        assert_eq!(
            inventory.attachments[0].detail.as_deref(),
            Some("Excel.Sheet.12")
        );
        assert_eq!(inventory.attachment_bytes, 166);
        assert_eq!(inventory.changes.insertions, 1);
        assert_eq!(inventory.changes.authors.len(), 2);
        assert_eq!(inventory.changes.track_revisions, Some(true));
        assert_eq!(inventory.changes.protection.as_deref(), Some("readOnly"));
        assert_eq!(inventory.changes.signatures, 1);
        assert_eq!(inventory.external_links, 1);
        assert_eq!(inventory.total_items(), 9 + 1 + 1 + 2);
    }

    #[test]
    fn audit_metadata_carries_numbers_only() {
        let inventory = parse_inventory(&payload()).expect("yanıt çözülmeli");
        let audit = inventory.audit_counts().to_string();
        for secret in [
            "oleObject1.bin",
            "image1.png",
            "Av. Deneme",
            "Excel.Sheet.12",
        ] {
            assert!(!audit.contains(secret), "denetime {secret} yazılmamalı");
        }
        assert!(audit.contains("\"attachments\":2"));
    }

    #[test]
    fn tolerates_an_old_worker_without_optional_fields() {
        let minimal = json!({"kind": "udf", "structure": {"paragraphs": 3}});
        let inventory = parse_inventory(&minimal).expect("eski worker yanıtı çözülmeli");
        assert_eq!(inventory.kind, "udf");
        assert!(inventory.attachments.is_empty());
        assert_eq!(inventory.changes, ChangeInventory::default());
        assert!(inventory.notes.is_empty());
        assert!(!inventory.truncated);
    }

    #[test]
    fn rejects_missing_or_unknown_kind() {
        assert!(parse_inventory(&json!({"structure": {}})).is_err());
        assert!(parse_inventory(&json!({"kind": "pdf", "structure": {}})).is_err());
    }

    #[test]
    fn rejects_missing_structure_and_wrong_types() {
        assert!(parse_inventory(&json!({"kind": "docx"})).is_err());
        assert!(parse_inventory(&json!({"kind": "docx", "structure": []})).is_err());
        assert!(parse_inventory(&json!({"kind": "docx", "structure": {"p": -1}})).is_err());
        assert!(parse_inventory(&json!({"kind": "docx", "structure": {"p": "3"}})).is_err());
        assert!(parse_inventory(&json!({"kind": "docx", "structure": {}, "notes": "x"})).is_err());
    }

    #[test]
    fn rejects_attachment_names_that_look_like_filesystem_paths() {
        for name in [
            "../evil.xml",
            "/etc/passwd",
            "word\\..\\evil",
            "word/../../x",
        ] {
            let bad = json!({
                "kind": "docx",
                "structure": {},
                "attachments": [{"name": name, "kind": "image", "size": 1}]
            });
            assert!(parse_inventory(&bad).is_err(), "{name} reddedilmeli");
        }
    }

    #[test]
    fn attachment_count_never_understates_the_list() {
        let value = json!({
            "kind": "docx",
            "structure": {},
            "attachments": [
                {"name": "word/media/a.png", "kind": "image", "size": 1},
                {"name": "word/media/b.png", "kind": "image", "size": 1}
            ],
            "attachmentCount": 0
        });
        assert_eq!(parse_inventory(&value).unwrap().attachment_count, 2);
    }
}
