//! Lisanslı kontrol listeleri (F08-T11).
//!
//! Şablonlar uygulamayla birlikte gelir ve **içerik lisansı** yazılım
//! lisansından ayrıdır (`CC-BY-4.0`, bkz. `docs/checklists/README.md`).
//! Katalog veritabanına yazılırken sürüm, yazar, gözden geçirme tarihi, lisans
//! ve içerik hash'i birlikte saklanır; dosyaya kopyalanan liste bu kaynağı
//! `template_version_id` ile taşır. İçerik hash'i dosyadaki `contentSha256`
//! ile eşleşmezse katalog **yüklenmez** — sessizce farklı metin kopyalanmaz.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{now_iso, Database, StorageError};

/// Gömülü katalog. Her dosya kendi lisans/sürüm bilgisini taşır.
const CATALOG: &[(&str, &str)] = &[
    (
        "kira-hukuku-v1",
        include_str!("../assets/checklists/kira-hukuku-v1.json"),
    ),
    (
        "is-hukuku-v1",
        include_str!("../assets/checklists/is-hukuku-v1.json"),
    ),
];

const MAX_NOTE_CHARS: usize = 2_000;
const MAX_LINKS: usize = 50;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundledTemplate {
    id: String,
    title: String,
    practice_area: String,
    version: u32,
    author: String,
    review_date: String,
    license_id: String,
    license_url: String,
    source_url: String,
    is_community: bool,
    content_sha256: String,
    items: Vec<BundledItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct BundledItem {
    category: String,
    text: String,
    #[serde(default)]
    required: bool,
}

/// Katalogdaki bir şablonun veritabanı satırı + kopya sayısı.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistTemplateRow {
    pub id: String,
    pub title: String,
    pub practice_area: Option<String>,
    pub version: i64,
    pub author: String,
    pub review_date: Option<String>,
    pub license_id: Option<String>,
    pub license_url: Option<String>,
    pub source_url: Option<String>,
    pub content_hash: String,
    pub is_community: bool,
    pub item_count: i64,
    /// Bu şablon sürümünden dosyaya kopyalanmış öğe sayısı.
    pub copied_item_count: i64,
}

/// Dosyadaki kontrol listesi öğesinin taşıdığı şablon künyesi.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistTemplateRef {
    pub id: String,
    pub title: String,
    pub version: i64,
    pub license_id: Option<String>,
    pub license_url: Option<String>,
    pub author: String,
    pub review_date: Option<String>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedDocumentRow {
    pub id: String,
    /// Belge sonradan silinmişse adı yine gösterilir; bağ tarihsel kalır.
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseChecklistItemRow {
    pub id: String,
    pub case_id: String,
    pub template_version_id: Option<String>,
    pub item_text: String,
    pub state: String,
    pub linked_document_ids: Vec<String>,
    pub linked_documents: Vec<LinkedDocumentRow>,
    pub note: String,
    pub ordinal: i64,
    pub category: String,
    pub created_at: String,
    pub updated_at: String,
    pub row_version: i64,
    pub template: Option<ChecklistTemplateRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistCopyResult {
    pub created: bool,
    pub item_count: usize,
    pub template_id: String,
    pub template_title: String,
    pub template_version: i64,
    pub license_id: Option<String>,
    pub content_hash: String,
}

impl Database {
    /// Gömülü kataloğu veritabanına yazar. Var olan kayıt aynı sürüm ve
    /// içerik hash'ini taşımıyorsa durur: şablon sessizce değiştirilmez.
    /// Dönen değer bu çağrıda eklenen şablon sayısıdır.
    pub fn ensure_checklist_catalog(&self) -> Result<usize, StorageError> {
        let tx = self.conn().unchecked_transaction()?;
        let mut inserted = 0usize;
        for (_, raw) in CATALOG {
            let template = parse_bundled(raw)?;
            let computed = content_hash(&template);
            if !computed.eq_ignore_ascii_case(&template.content_sha256) {
                return Err(StorageError::Integrity(format!(
                    "kontrol listesi içerik hash'i uyuşmuyor: {}",
                    template.id
                )));
            }
            let existing: Option<(i64, String)> = tx
                .query_row(
                    "SELECT version, content_hash FROM checklist_templates WHERE id=?1",
                    [&template.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;

            match existing {
                Some((version, stored_hash)) => {
                    let expected_version = i64::from(template.version);
                    if version != expected_version || !stored_hash.eq_ignore_ascii_case(&computed) {
                        return Err(StorageError::Integrity(format!(
                            "kontrol listesi sürümü çakışıyor: {}",
                            template.id
                        )));
                    }
                    verify_template_items(&tx, &template)?;
                }
                None => {
                    let now = now_iso();
                    let template_id = template.id.as_str();
                    tx.execute(
                        "INSERT INTO checklist_templates
                           (id,title,practice_area,version,author,review_date,license_id,content_hash,is_community,created_at,source_url,license_url)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                        rusqlite::params![
                            template_id,
                            template.title,
                            template.practice_area,
                            i64::from(template.version),
                            template.author,
                            template.review_date,
                            template.license_id,
                            computed,
                            if template.is_community { 1 } else { 0 },
                            now,
                            template.source_url,
                            template.license_url,
                        ],
                    )?;
                    for (ordinal, item) in template.items.iter().enumerate() {
                        tx.execute(
                            "INSERT INTO checklist_template_items
                               (id,template_id,ordinal,category,item_text,required)
                             VALUES (?1,?2,?3,?4,?5,?6)",
                            rusqlite::params![
                                format!("{template_id}:item:{ordinal}"),
                                template_id,
                                i64::try_from(ordinal).unwrap_or(i64::MAX),
                                item.category,
                                item.text,
                                if item.required { 1 } else { 0 },
                            ],
                        )?;
                    }
                    inserted += 1;
                }
            }
        }
        tx.commit()?;
        Ok(inserted)
    }

    /// Katalogdaki şablonlar; her biri için dosyaya kopyalanmış öğe sayısı.
    pub fn list_checklist_templates(
        &self,
        case_id: &str,
    ) -> Result<Vec<ChecklistTemplateRow>, StorageError> {
        self.ensure_checklist_catalog()?;
        let mut stmt = self.conn().prepare(
            "SELECT t.id,t.title,t.practice_area,t.version,t.author,t.review_date,
                    t.license_id,t.license_url,t.source_url,t.content_hash,t.is_community,
                    (SELECT count(*) FROM checklist_template_items i WHERE i.template_id=t.id),
                    (SELECT count(*) FROM case_checklist_items c
                      WHERE c.template_version_id=t.id AND c.case_id=?1)
             FROM checklist_templates t
             ORDER BY (t.practice_area IS NULL), t.practice_area, t.title, t.version DESC",
        )?;
        let rows = stmt.query_map([case_id], |row| {
            Ok(ChecklistTemplateRow {
                id: row.get(0)?,
                title: row.get(1)?,
                practice_area: row.get(2)?,
                version: row.get(3)?,
                author: row.get(4)?,
                review_date: row.get(5)?,
                license_id: row.get(6)?,
                license_url: row.get(7)?,
                source_url: row.get(8)?,
                content_hash: row.get(9)?,
                is_community: row.get::<_, i64>(10)? != 0,
                item_count: row.get(11)?,
                copied_item_count: row.get(12)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Şablonu dosyaya kopyalar. Kopya **idempotenttir**: aynı şablon sürümü
    /// ikinci kez kopyalanmaz, mevcut öğe sayısı döner. Böylece çift tıklama
    /// veya yeniden deneme dosyada yinelenen liste üretmez.
    pub fn copy_checklist_to_case(
        &self,
        case_id: &str,
        template_id: &str,
    ) -> Result<ChecklistCopyResult, StorageError> {
        self.ensure_checklist_catalog()?;
        let case_exists: i64 = self.conn().query_row(
            "SELECT count(*) FROM cases WHERE id=?1 AND deleted_at IS NULL",
            [case_id],
            |row| row.get(0),
        )?;
        if case_exists == 0 {
            return Err(StorageError::NotFound);
        }

        let template: ChecklistTemplateRow = self
            .list_checklist_templates(case_id)?
            .into_iter()
            .find(|t| t.id == template_id)
            .ok_or(StorageError::NotFound)?;

        let existing: i64 = self.conn().query_row(
            "SELECT count(*) FROM case_checklist_items WHERE case_id=?1 AND template_version_id=?2",
            rusqlite::params![case_id, template_id],
            |row| row.get(0),
        )?;
        if existing > 0 {
            return Ok(ChecklistCopyResult {
                created: false,
                item_count: usize::try_from(existing).unwrap_or(0),
                template_id: template.id,
                template_title: template.title,
                template_version: template.version,
                license_id: template.license_id,
                content_hash: template.content_hash,
            });
        }

        let items = load_template_items(self, template_id)?;
        if items.is_empty() {
            return Err(StorageError::Integrity(
                "kontrol listesi şablonu boş".to_string(),
            ));
        }
        let tx = self.conn().unchecked_transaction()?;
        let now = now_iso();
        for (ordinal, category, text) in &items {
            tx.execute(
                "INSERT INTO case_checklist_items
                   (id,case_id,template_version_id,item_text,state,linked_document_ids,note,created_at,updated_at,row_version,ordinal,category)
                 VALUES (?1,?2,?3,?4,'open','[]','',?5,?5,1,?6,?7)",
                rusqlite::params![
                    uuid::Uuid::now_v7().to_string(),
                    case_id,
                    template_id,
                    text,
                    now,
                    ordinal,
                    category,
                ],
            )?;
        }
        self.append_audit(
            "checklist",
            Some(template_id),
            "copy",
            "local-user",
            &serde_json::json!({
                "caseId": case_id,
                "version": template.version,
                "licenseId": template.license_id,
                "contentHash": template.content_hash,
                "itemCount": items.len(),
            }),
        )?;
        tx.commit()?;

        Ok(ChecklistCopyResult {
            created: true,
            item_count: items.len(),
            template_id: template.id,
            template_title: template.title,
            template_version: template.version,
            license_id: template.license_id,
            content_hash: template.content_hash,
        })
    }

    /// Dosyadaki kontrol listesi; şablon sırası korunur, şablon silinmiş olsa
    /// bile metin ve künye kopyalama anındaki hâliyle okunur.
    pub fn list_case_checklist(
        &self,
        case_id: &str,
    ) -> Result<Vec<CaseChecklistItemRow>, StorageError> {
        self.ensure_checklist_catalog()?;
        let mut stmt = self.conn().prepare(&format!(
            "{CASE_ITEM_SELECT} WHERE c.case_id=?1
             ORDER BY COALESCE(t.practice_area,''), COALESCE(t.title,''), c.ordinal, c.created_at"
        ))?;
        let rows = stmt.query_map([case_id], map_case_item)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        drop(stmt);
        for item in &mut out {
            item.linked_documents =
                self.linked_document_names(&item.case_id, &item.linked_document_ids)?;
        }
        Ok(out)
    }

    pub fn get_case_checklist_item(
        &self,
        item_id: &str,
    ) -> Result<Option<CaseChecklistItemRow>, StorageError> {
        let mut stmt = self
            .conn()
            .prepare(&format!("{CASE_ITEM_SELECT} WHERE c.id=?1"))?;
        let mut item = stmt.query_row([item_id], map_case_item).optional()?;
        drop(stmt);
        if let Some(found) = item.as_mut() {
            found.linked_documents =
                self.linked_document_names(&found.case_id, &found.linked_document_ids)?;
        }
        Ok(item)
    }

    /// Liste öğesinin durumunu yazar. Kapılar:
    /// - `present` en az bir **aynı dosyaya ait** belge bağı ister;
    /// - `missing` ve `not_applicable` gerekçe/inceleme kapsamı notu ister:
    ///   "bulunamadı" yalnız incelenen kapsamı anlatır, "dosyada yok" demez;
    /// - `open` durumuna dönerken belge bağları kaldırılır;
    /// - yazma `row_version` ile korunur (F08-T09 deseni).
    pub fn set_case_checklist_state(
        &self,
        item_id: &str,
        new_state: &str,
        linked_document_ids: &[String],
        note: &str,
        expected_row_version: i64,
    ) -> Result<CaseChecklistItemRow, StorageError> {
        let current = self
            .get_case_checklist_item(item_id)?
            .ok_or(StorageError::NotFound)?;
        let state = match new_state {
            "open" | "present" | "missing" | "not_applicable" => new_state,
            _ => {
                return Err(StorageError::ChecklistRule(
                    "Bilinmeyen kontrol listesi durumu.".to_string(),
                ))
            }
        };

        let mut unique: Vec<String> = Vec::new();
        for id in linked_document_ids {
            let trimmed = id.trim();
            if !trimmed.is_empty() && !unique.iter().any(|seen| seen == trimmed) {
                unique.push(trimmed.to_string());
            }
        }
        if unique.len() > MAX_LINKS {
            return Err(StorageError::ChecklistRule(format!(
                "En fazla {MAX_LINKS} belge bağlanabilir."
            )));
        }
        let note = note.trim();
        if note.chars().count() > MAX_NOTE_CHARS {
            return Err(StorageError::ChecklistRule(format!(
                "Not en fazla {MAX_NOTE_CHARS} karakter olabilir."
            )));
        }

        match state {
            "present" => {
                if unique.is_empty() {
                    return Err(StorageError::ChecklistRule(
                        "“Bulundu” durumu en az bir belge bağı ister.".to_string(),
                    ));
                }
                for document_id in &unique {
                    let found: i64 = self.conn().query_row(
                        "SELECT count(*) FROM documents
                         WHERE id=?1 AND case_id=?2 AND deleted_at IS NULL AND status<>'deleted'",
                        rusqlite::params![document_id, current.case_id],
                        |row| row.get(0),
                    )?;
                    if found == 0 {
                        return Err(StorageError::ChecklistRule(
                            "Belge bağı bu dosyada bulunamadı.".to_string(),
                        ));
                    }
                }
            }
            "missing" => {
                if !unique.is_empty() {
                    return Err(StorageError::ChecklistRule(
                        "Belge bağı yalnız “Bulundu” durumunda kullanılır.".to_string(),
                    ));
                }
                if note.is_empty() {
                    return Err(StorageError::ChecklistRule(
                        "“İncelenen kapsamda bulunamadı” için incelenen kapsamı yazın.".to_string(),
                    ));
                }
            }
            "not_applicable" => {
                if !unique.is_empty() {
                    return Err(StorageError::ChecklistRule(
                        "Belge bağı yalnız “Bulundu” durumunda kullanılır.".to_string(),
                    ));
                }
                if note.is_empty() {
                    return Err(StorageError::ChecklistRule(
                        "“Uygulanamaz” için gerekçe yazın.".to_string(),
                    ));
                }
            }
            "open" => {
                if !unique.is_empty() {
                    return Err(StorageError::ChecklistRule(
                        "Açık duruma dönerken belge bağları kaldırılmalı.".to_string(),
                    ));
                }
            }
            _ => unreachable!("durum yukarıda doğrulandı"),
        }

        let linked_json = serde_json::to_string(&unique)
            .map_err(|_| StorageError::Integrity("belge bağları yazılamadı".to_string()))?;
        let now = now_iso();
        let tx = self.conn().unchecked_transaction()?;
        let changed = tx.execute(
            "UPDATE case_checklist_items
             SET state=?1, linked_document_ids=?2, note=?3, updated_at=?4, row_version=row_version+1
             WHERE id=?5 AND row_version=?6",
            rusqlite::params![state, linked_json, note, now, item_id, expected_row_version],
        )?;
        if changed == 0 {
            let actual: Option<i64> = tx
                .query_row(
                    "SELECT row_version FROM case_checklist_items WHERE id=?1",
                    [item_id],
                    |row| row.get(0),
                )
                .optional()?;
            return match actual {
                Some(actual) => Err(StorageError::Conflict {
                    expected: expected_row_version,
                    actual,
                }),
                None => Err(StorageError::NotFound),
            };
        }
        // Denetim kaydı aynı işlemde: durum yazıldıysa izi de yazılır.
        self.append_audit(
            "checklist_item",
            Some(item_id),
            "state",
            "local-user",
            &serde_json::json!({
                "caseId": current.case_id,
                "state": state,
                "documentCount": unique.len(),
            }),
        )?;
        tx.commit()?;
        self.get_case_checklist_item(item_id)?
            .ok_or(StorageError::NotFound)
    }

    fn linked_document_names(
        &self,
        case_id: &str,
        ids: &[String],
    ) -> Result<Vec<LinkedDocumentRow>, StorageError> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let name: Option<String> = self
                .conn()
                .query_row(
                    "SELECT display_name FROM documents WHERE id=?1 AND case_id=?2",
                    rusqlite::params![id, case_id],
                    |row| row.get(0),
                )
                .optional()?;
            out.push(LinkedDocumentRow {
                id: id.clone(),
                display_name: name,
            });
        }
        Ok(out)
    }
}

const CASE_ITEM_SELECT: &str = "SELECT c.id,c.case_id,c.template_version_id,c.item_text,c.state,
        c.linked_document_ids,c.note,c.created_at,c.updated_at,c.row_version,c.ordinal,c.category,
        t.id,t.title,t.version,t.license_id,t.license_url,t.author,t.review_date,t.content_hash
   FROM case_checklist_items c
   LEFT JOIN checklist_templates t ON t.id=c.template_version_id";

fn map_case_item(row: &Row<'_>) -> rusqlite::Result<CaseChecklistItemRow> {
    let raw_links: String = row.get(5)?;
    let linked_document_ids: Vec<String> = serde_json::from_str(&raw_links).map_err(|source| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(source))
    })?;
    let template = match row.get::<_, Option<String>>(12)? {
        Some(id) => Some(ChecklistTemplateRef {
            id,
            title: row.get::<_, Option<String>>(13)?.unwrap_or_default(),
            version: row.get::<_, Option<i64>>(14)?.unwrap_or_default(),
            license_id: row.get(15)?,
            license_url: row.get(16)?,
            author: row.get::<_, Option<String>>(17)?.unwrap_or_default(),
            review_date: row.get(18)?,
            content_hash: row.get::<_, Option<String>>(19)?.unwrap_or_default(),
        }),
        None => None,
    };
    Ok(CaseChecklistItemRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        template_version_id: row.get(2)?,
        item_text: row.get(3)?,
        state: row.get(4)?,
        linked_document_ids,
        linked_documents: Vec::new(),
        note: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        row_version: row.get(9)?,
        ordinal: row.get(10)?,
        category: row.get(11)?,
        template,
    })
}

fn parse_bundled(raw: &str) -> Result<BundledTemplate, StorageError> {
    serde_json::from_str(raw).map_err(|_| {
        StorageError::Integrity("gömülü kontrol listesi kataloğu okunamadı".to_string())
    })
}

/// İçerik hash'i: sürüm, künye ve maddeler kanonik sırayla. URL'ler kapsam
/// dışıdır; lisans kimliği (`licenseId`) kapsam içindedir.
fn content_hash(template: &BundledTemplate) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"dimvault/checklist-content/v1");
    for part in [
        template.id.as_str(),
        template.title.as_str(),
        template.practice_area.as_str(),
        template.author.as_str(),
        template.review_date.as_str(),
        template.license_id.as_str(),
    ] {
        hasher.update([0x1f]);
        hasher.update(part.as_bytes());
    }
    hasher.update([0x1f]);
    hasher.update(template.version.to_string().as_bytes());
    hasher.update([0x1f]);
    hasher.update(if template.is_community { b"1" } else { b"0" });
    for (ordinal, item) in template.items.iter().enumerate() {
        hasher.update([0x1e]);
        hasher.update(ordinal.to_string().as_bytes());
        hasher.update([0x00]);
        hasher.update(item.category.as_bytes());
        hasher.update([0x00]);
        hasher.update(item.text.as_bytes());
        hasher.update([0x00]);
        hasher.update(if item.required { b"1" } else { b"0" });
    }
    hex::encode(hasher.finalize())
}

/// Şablon maddelerini sırayla okur: `(ordinal, kategori, metin)`.
fn load_template_items(
    db: &Database,
    template_id: &str,
) -> Result<Vec<(i64, String, String)>, StorageError> {
    let mut stmt = db.conn().prepare(
        "SELECT ordinal,category,item_text FROM checklist_template_items
         WHERE template_id=?1 ORDER BY ordinal",
    )?;
    let rows = stmt.query_map([template_id], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Veritabanındaki maddeler gömülü katalogla birebir aynı mı? Fark varsa
/// şablon kurcalanmış demektir; kopyalama durur.
fn verify_template_items(
    conn: &rusqlite::Connection,
    template: &BundledTemplate,
) -> Result<(), StorageError> {
    let mut stmt = conn.prepare(
        "SELECT ordinal,category,item_text,required FROM checklist_template_items
         WHERE template_id=?1 ORDER BY ordinal",
    )?;
    let rows = stmt.query_map([&template.id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)? != 0,
        ))
    })?;
    let mut stored = Vec::new();
    for row in rows {
        stored.push(row?);
    }
    let template_id = template.id.as_str();
    if stored.len() != template.items.len() {
        return Err(StorageError::Integrity(format!(
            "kontrol listesi maddeleri eksik: {template_id}"
        )));
    }
    for (index, item) in template.items.iter().enumerate() {
        let ordinal = i64::try_from(index).unwrap_or(i64::MAX);
        let Some((stored_ordinal, category, text, required)) = stored.get(index) else {
            return Err(StorageError::Integrity(format!(
                "kontrol listesi maddeleri eksik: {template_id}"
            )));
        };
        if *stored_ordinal != ordinal
            || category != &item.category
            || text != &item.text
            || *required != item.required
        {
            return Err(StorageError::Integrity(format!(
                "kontrol listesi maddesi kurcalanmış: {template_id} #{ordinal}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn setup() -> (Database, String) {
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
        (db, case.id)
    }

    fn add_doc(db: &Database, case_id: &str, name: &str) -> String {
        db.create_document(case_id, name, "other", None, "unknown")
            .expect("belge")
            .id
    }

    fn first_item(db: &Database, case_id: &str) -> CaseChecklistItemRow {
        db.list_case_checklist(case_id)
            .expect("liste")
            .into_iter()
            .next()
            .expect("öğe")
    }

    #[test]
    fn bundled_catalog_hashes_are_pinned() {
        for (_, raw) in CATALOG {
            let template = parse_bundled(raw).expect("katalog");
            assert_eq!(
                content_hash(&template),
                template.content_sha256,
                "{}: dosyadaki contentSha256 güncel değil",
                template.id
            );
            assert!(!template.items.is_empty());
            assert!(template.license_id.starts_with("CC-BY"));
        }
    }

    #[test]
    fn catalog_seeds_license_source_and_hash() {
        let (db, case_id) = setup();
        assert_eq!(db.ensure_checklist_catalog().expect("katalog"), 2);
        assert_eq!(
            db.ensure_checklist_catalog().expect("ikinci çağrı"),
            0,
            "katalog ikinci kez eklenmez"
        );
        let templates = db.list_checklist_templates(&case_id).expect("liste");
        assert_eq!(templates.len(), 2);
        for template in &templates {
            assert_eq!(template.license_id.as_deref(), Some("CC-BY-4.0"));
            assert!(template.license_url.is_some());
            assert!(template.source_url.is_some(), "kaynak URL'si saklanmalı");
            assert_eq!(template.content_hash.len(), 64);
            assert!(template.item_count > 0);
            assert_eq!(template.copied_item_count, 0);
            assert!(!template.is_community);
        }
    }

    #[test]
    fn copy_preserves_version_license_and_order() {
        let (db, case_id) = setup();
        let result = db
            .copy_checklist_to_case(&case_id, "tpl:kira-hukuku:v1")
            .expect("kopyala");
        assert!(result.created);
        assert_eq!(result.template_version, 1);
        assert_eq!(result.license_id.as_deref(), Some("CC-BY-4.0"));
        assert_eq!(result.item_count, 10);

        let items = db.list_case_checklist(&case_id).expect("liste");
        assert_eq!(items.len(), 10);
        assert_eq!(items[0].ordinal, 0);
        assert_eq!(items[9].ordinal, 9);
        for item in &items {
            assert_eq!(item.case_id, case_id);
            assert_eq!(
                item.template_version_id.as_deref(),
                Some("tpl:kira-hukuku:v1")
            );
            let template = item.template.as_ref().expect("künye");
            assert_eq!(template.version, 1);
            assert_eq!(template.license_id.as_deref(), Some("CC-BY-4.0"));
            assert_eq!(template.content_hash, result.content_hash);
            assert_eq!(item.state, "open");
            assert!(item.linked_document_ids.is_empty());
        }
        // Şablon sırası ve kategorisi kopyada korunur.
        assert_eq!(items[0].category, "Sözleşme");
        assert!(items[0].item_text.contains("Kira sözleşmesi"));

        let templates = db.list_checklist_templates(&case_id).expect("katalog");
        let kira = templates
            .iter()
            .find(|t| t.id == "tpl:kira-hukuku:v1")
            .expect("kira");
        assert_eq!(kira.copied_item_count, 10);
    }

    #[test]
    fn copy_is_idempotent_and_audited_once() {
        let (db, case_id) = setup();
        db.copy_checklist_to_case(&case_id, "tpl:kira-hukuku:v1")
            .expect("ilk");
        let second = db
            .copy_checklist_to_case(&case_id, "tpl:kira-hukuku:v1")
            .expect("ikinci");
        assert!(!second.created);
        assert_eq!(second.item_count, 10);
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM case_checklist_items WHERE case_id=?1",
                [&case_id],
                |row| row.get(0),
            )
            .expect("say");
        assert_eq!(count, 10, "ikinci kopya yinelenen öğe üretmemeli");
        let audits: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM audit_events WHERE category='checklist' AND operation='copy'",
                [],
                |row| row.get(0),
            )
            .expect("audit");
        assert_eq!(audits, 1);
    }

    #[test]
    fn copy_rejects_missing_case_or_template() {
        let (db, case_id) = setup();
        assert_eq!(
            db.copy_checklist_to_case(&case_id, "tpl:yok:v1")
                .expect_err("bilinmeyen şablon")
                .code(),
            "E_NOT_FOUND"
        );
        assert_eq!(
            db.copy_checklist_to_case("olmayan-dosya", "tpl:kira-hukuku:v1")
                .expect_err("bilinmeyen dosya")
                .code(),
            "E_NOT_FOUND"
        );
    }

    #[test]
    fn present_needs_document_from_same_case() {
        let (db, case_id) = setup();
        db.copy_checklist_to_case(&case_id, "tpl:kira-hukuku:v1")
            .expect("kopyala");
        let item = first_item(&db, &case_id);

        let empty = db
            .set_case_checklist_state(&item.id, "present", &[], "", item.row_version)
            .expect_err("belge bağı olmadan bulundu denemez");
        assert_eq!(empty.code(), "E_CHECKLIST_RULE");

        let other_case = db
            .create_case(&NewCase {
                title: "Başka".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("diğer dosya");
        let foreign_doc = add_doc(&db, &other_case.id, "Başka dosyanın belgesi");
        let cross = db
            .set_case_checklist_state(
                &item.id,
                "present",
                std::slice::from_ref(&foreign_doc),
                "",
                item.row_version,
            )
            .expect_err("başka dosyanın belgesi bağlanamaz");
        assert_eq!(cross.code(), "E_CHECKLIST_RULE");

        let doc = add_doc(&db, &case_id, "Kira sözleşmesi 2024.pdf");
        let updated = db
            .set_case_checklist_state(
                &item.id,
                "present",
                std::slice::from_ref(&doc),
                "",
                item.row_version,
            )
            .expect("bulundu");
        assert_eq!(updated.state, "present");
        assert_eq!(updated.linked_document_ids, vec![doc.clone()]);
        assert_eq!(
            updated.linked_documents[0].display_name.as_deref(),
            Some("Kira sözleşmesi 2024.pdf")
        );
        assert_eq!(updated.row_version, item.row_version + 1);
    }

    #[test]
    fn missing_requires_examined_scope_note() {
        let (db, case_id) = setup();
        db.copy_checklist_to_case(&case_id, "tpl:kira-hukuku:v1")
            .expect("kopyala");
        let item = first_item(&db, &case_id);

        let no_note = db
            .set_case_checklist_state(&item.id, "missing", &[], "   ", item.row_version)
            .expect_err("kapsamsız bulunamadı");
        assert_eq!(no_note.code(), "E_CHECKLIST_RULE");

        let doc = add_doc(&db, &case_id, "Taranmış belge.pdf");
        let with_link = db
            .set_case_checklist_state(
                &item.id,
                "missing",
                std::slice::from_ref(&doc),
                "İncelendi",
                item.row_version,
            )
            .expect_err("bulunamadı belge bağı taşımaz");
        assert_eq!(with_link.code(), "E_CHECKLIST_RULE");

        let updated = db
            .set_case_checklist_state(
                &item.id,
                "missing",
                &[],
                "2020-2024 arası kira sözleşmeleri incelendi",
                item.row_version,
            )
            .expect("bulunamadı");
        assert_eq!(updated.state, "missing");
        assert!(updated.note.contains("2020-2024"));
        // Not metni audit'e sızmaz; yalnız durum ve belge sayısı yazılır.
        let metadata: String = db
            .conn()
            .query_row(
                "SELECT metadata_allowlist FROM audit_events WHERE category='checklist_item' ORDER BY sequence DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("audit");
        assert!(!metadata.contains("2020-2024"), "not audit'e sızmamalı");
        assert!(metadata.contains("missing"));
    }

    #[test]
    fn not_applicable_requires_reason_and_open_clears_links() {
        let (db, case_id) = setup();
        db.copy_checklist_to_case(&case_id, "tpl:kira-hukuku:v1")
            .expect("kopyala");
        let item = first_item(&db, &case_id);

        assert_eq!(
            db.set_case_checklist_state(&item.id, "not_applicable", &[], "", item.row_version)
                .expect_err("gerekçesiz")
                .code(),
            "E_CHECKLIST_RULE"
        );
        let na = db
            .set_case_checklist_state(
                &item.id,
                "not_applicable",
                &[],
                "Dosyada kira sözleşmesi yok; uyuşmazlık ortak alan kullanımına ilişkin",
                item.row_version,
            )
            .expect("uygulanamaz");
        assert_eq!(na.state, "not_applicable");

        let doc = add_doc(&db, &case_id, "Sözleşme.pdf");
        let present = db
            .set_case_checklist_state(
                &na.id,
                "present",
                std::slice::from_ref(&doc),
                "",
                na.row_version,
            )
            .expect("bulundu");
        assert_eq!(
            db.set_case_checklist_state(
                &present.id,
                "open",
                std::slice::from_ref(&doc),
                "",
                present.row_version,
            )
            .expect_err("bağlı belgeyle açığa dönülemez")
            .code(),
            "E_CHECKLIST_RULE"
        );
        let reopened = db
            .set_case_checklist_state(&present.id, "open", &[], "", present.row_version)
            .expect("açık");
        assert_eq!(reopened.state, "open");
        assert!(reopened.linked_document_ids.is_empty());
    }

    #[test]
    fn stale_row_version_conflicts() {
        let (db, case_id) = setup();
        db.copy_checklist_to_case(&case_id, "tpl:kira-hukuku:v1")
            .expect("kopyala");
        let item = first_item(&db, &case_id);
        let updated = db
            .set_case_checklist_state(
                &item.id,
                "not_applicable",
                &[],
                "Kapsam dışı",
                item.row_version,
            )
            .expect("ilk yazma");
        let conflict = db
            .set_case_checklist_state(&item.id, "open", &[], "", item.row_version)
            .expect_err("eski sürüm");
        assert_eq!(conflict.code(), "E_ROW_VERSION");
        match conflict {
            StorageError::Conflict { expected, actual } => {
                assert_eq!(expected, item.row_version);
                assert_eq!(actual, updated.row_version);
            }
            other => panic!("beklenen çatışma, gelen {other:?}"),
        }
    }

    #[test]
    fn tampered_template_blocks_copy() {
        let (db, case_id) = setup();
        db.ensure_checklist_catalog().expect("katalog");
        db.conn()
            .execute(
                "UPDATE checklist_template_items SET item_text='başka metin'
                 WHERE template_id='tpl:kira-hukuku:v1' AND ordinal=0",
                [],
            )
            .expect("kurcala");
        let err = db
            .copy_checklist_to_case(&case_id, "tpl:kira-hukuku:v1")
            .expect_err("kurcalanmış şablon kopyalanmaz");
        assert_eq!(err.code(), "E_DB_INTEGRITY");
        let count: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM case_checklist_items", [], |row| {
                row.get(0)
            })
            .expect("say");
        assert_eq!(count, 0, "yarım kopya yazılmamalı");
    }

    #[test]
    fn copied_text_survives_template_deletion() {
        let (db, case_id) = setup();
        db.copy_checklist_to_case(&case_id, "tpl:is-hukuku:v1")
            .expect("kopyala");
        let before = db.list_case_checklist(&case_id).expect("liste");
        db.conn()
            .execute(
                "DELETE FROM checklist_templates WHERE id='tpl:is-hukuku:v1'",
                [],
            )
            .expect("şablonu sil");
        let after = db.list_case_checklist(&case_id).expect("liste");
        assert_eq!(after.len(), before.len());
        assert_eq!(after[0].item_text, before[0].item_text);
        assert_eq!(after[0].ordinal, before[0].ordinal);
        // Şablon kimliği FK ile NULL'a düşer; metin ve sıra dosyada kalır.
        assert!(after[0].template_version_id.is_none());
        assert!(after[0].template.is_none());
    }
}
