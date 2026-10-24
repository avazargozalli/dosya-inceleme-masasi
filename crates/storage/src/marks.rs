//! Kalıcı işaretler (F07-T09). Metin seçimi **doğrulanmış** bir kaynak
//! anchor'ına, sayfa bölgesi seçimi ise sayfanın kendi koordinat uzayındaki
//! dikdörtgenlere bağlanır.
//!
//! Kurallar:
//! - Alıntı segment metninde birebir bulunamazsa anchor oluşmaz; işaret de
//!   oluşmaz (INV-02). İşaret uydurma metinle kaydedilmez.
//! - Aynı anchor üzerinde tek aktif metin işareti vardır; ikinci çağrı yeni
//!   satır açmaz, var olanı döndürür.
//! - Bölge işareti sayfa/rotasyon/yakınlaştırma değişiminden etkilenmez:
//!   koordinatlar döndürme öncesi sayfa uzayındadır.
//! - Seçimden **not** oluşturmak ayrı eylemdir; anchor + not tek işlemde
//!   yazılır ki yarım kalan kayıt (kaynaksız not) oluşmasın.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::anchors::{AnchorRow, NewAnchor};
use crate::notes::{NewNote, NoteRow};
use crate::{now_iso, Database, StorageError};

/// Bir bölge işaretinde en çok dikdörtgen sayısı (tek seçimden çok parça).
const MAX_REGION_RECTS: usize = 20;
/// İşaret etiketi en fazla bu kadar karakter taşır.
const MAX_LABEL_CHARS: usize = 120;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkRow {
    pub id: String,
    pub case_id: String,
    pub document_version_id: String,
    pub parse_run_id: Option<String>,
    pub anchor_id: Option<String>,
    pub page_id: Option<String>,
    /// `text` | `region`
    pub mark_kind: String,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub quoted_text: String,
    pub rects: Vec<[f64; 4]>,
    pub label: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTextMark {
    pub case_id: String,
    pub anchor_id: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRegionMark {
    pub case_id: String,
    pub document_version_id: String,
    pub page_id: String,
    /// Sayfa koordinat uzayında `[x0, y0, x1, y1]` (döndürme öncesi).
    pub rects: Vec<[f64; 4]>,
    #[serde(default)]
    pub label: String,
}

/// Metin işareti kurulurken anchor'dan kopyalanan alanlar.
struct AnchorSource {
    case_id: String,
    document_version_id: String,
    parse_run_id: String,
    page_id: Option<String>,
    start_offset: i64,
    end_offset: i64,
    exact_quote: String,
    anchor_status: String,
}

fn map_mark(row: &Row<'_>) -> rusqlite::Result<MarkRow> {
    let rects_json: String = row.get("rects_json")?;
    let rects: Vec<[f64; 4]> = serde_json::from_str(&rects_json).unwrap_or_default();
    Ok(MarkRow {
        id: row.get("id")?,
        case_id: row.get("case_id")?,
        document_version_id: row.get("document_version_id")?,
        parse_run_id: row.get("parse_run_id")?,
        anchor_id: row.get("anchor_id")?,
        page_id: row.get("page_id")?,
        mark_kind: row.get("mark_kind")?,
        start_offset: row.get("start_offset")?,
        end_offset: row.get("end_offset")?,
        quoted_text: row.get("quoted_text")?,
        rects,
        label: row.get("label")?,
        created_at: row.get("created_at")?,
    })
}

fn normalize_label(label: &str) -> String {
    label.trim().chars().take(MAX_LABEL_CHARS).collect()
}

impl Database {
    /// Metin seçiminden işaret: anchor doğrulaması çağıran `create_anchor`
    /// tarafından yapılır, burada yalnız bağ kurulur.
    pub fn create_text_mark(&self, new: &NewTextMark) -> Result<MarkRow, StorageError> {
        let anchor = self
            .conn()
            .query_row(
                "SELECT case_id, document_version_id, parse_run_id, page_id, start_offset, end_offset, exact_quote, anchor_status
                 FROM source_anchors WHERE id=?1",
                [&new.anchor_id],
                |row| {
                    Ok(AnchorSource {
                        case_id: row.get(0)?,
                        document_version_id: row.get(1)?,
                        parse_run_id: row.get(2)?,
                        page_id: row.get(3)?,
                        start_offset: row.get(4)?,
                        end_offset: row.get(5)?,
                        exact_quote: row.get(6)?,
                        anchor_status: row.get(7)?,
                    })
                },
            )
            .optional()?
            .ok_or(StorageError::NotFound)?;
        if anchor.case_id != new.case_id {
            return Err(StorageError::NotFound);
        }
        // Çözümlenmemiş/belirsiz anchor işaretlenmez: kaynağı doğrulanmamış
        // bir alıntı "işaretlendi" görüntüsü almaz.
        if anchor.anchor_status != "active" {
            return Err(StorageError::InvalidSetting);
        }
        if let Some(existing) = self.mark_for_anchor(&new.anchor_id)? {
            return Ok(existing);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO source_marks
               (id,case_id,document_version_id,parse_run_id,anchor_id,page_id,mark_kind,start_offset,end_offset,quoted_text,rects_json,label,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,'text',?7,?8,?9,'[]',?10,?11)",
            rusqlite::params![
                id,
                anchor.case_id,
                anchor.document_version_id,
                anchor.parse_run_id,
                new.anchor_id,
                anchor.page_id,
                anchor.start_offset,
                anchor.end_offset,
                anchor.exact_quote,
                normalize_label(&new.label),
                now
            ],
        )?;
        self.append_audit(
            "source_mark",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"kind": "text", "anchorId": new.anchor_id}),
        )?;
        self.get_mark(&id)?.ok_or(StorageError::NotFound)
    }

    /// Sayfa bölgesinden işaret. Dikdörtgenler sayfanın gerçek ölçüsüne karşı
    /// doğrulanır; ölçü yoksa işaret oluşmaz (konum uydurulmaz).
    pub fn create_region_mark(&self, new: &NewRegionMark) -> Result<MarkRow, StorageError> {
        let version = self
            .get_version(&new.document_version_id)?
            .ok_or(StorageError::NotFound)?;
        if version.case_id != new.case_id {
            return Err(StorageError::NotFound);
        }
        let page: Option<(String, Option<f64>, Option<f64>)> = self
            .conn()
            .query_row(
                "SELECT p.parse_run_id, p.width, p.height
                 FROM pages p JOIN parse_runs r ON r.id = p.parse_run_id
                 WHERE p.id=?1 AND r.document_version_id=?2",
                [&new.page_id, &new.document_version_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let (run_id, width, height) = page.ok_or(StorageError::NotFound)?;
        let (Some(width), Some(height)) = (width, height) else {
            // Sayfa geometrisi yoksa piksel/punto uzayı bilinmiyor.
            return Err(StorageError::InvalidSetting);
        };
        if width <= 0.0 || height <= 0.0 {
            return Err(StorageError::InvalidSetting);
        }
        if new.rects.is_empty() || new.rects.len() > MAX_REGION_RECTS {
            return Err(StorageError::InvalidSetting);
        }
        for rect in &new.rects {
            let [x0, y0, x1, y1] = *rect;
            if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
                return Err(StorageError::InvalidSetting);
            }
            if x1 <= x0 || y1 <= y0 {
                return Err(StorageError::InvalidSetting);
            }
            if x0 < 0.0 || y0 < 0.0 || x1 > width || y1 > height {
                return Err(StorageError::InvalidSetting);
            }
        }
        let rects_json =
            serde_json::to_string(&new.rects).map_err(|_| StorageError::InvalidSetting)?;
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO source_marks
               (id,case_id,document_version_id,parse_run_id,anchor_id,page_id,mark_kind,start_offset,end_offset,quoted_text,rects_json,label,created_at)
             VALUES (?1,?2,?3,?4,NULL,?5,'region',NULL,NULL,'',?6,?7,?8)",
            rusqlite::params![
                id,
                new.case_id,
                new.document_version_id,
                run_id,
                new.page_id,
                rects_json,
                normalize_label(&new.label),
                now
            ],
        )?;
        self.append_audit(
            "source_mark",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"kind": "region", "pageId": new.page_id}),
        )?;
        self.get_mark(&id)?.ok_or(StorageError::NotFound)
    }

    /// Metin seçiminden **işaret**: anchor doğrulanır ve işaret aynı işlemde
    /// yazılır. Alıntı doğrulanamazsa hiçbir kayıt oluşmaz.
    pub fn mark_from_selection(
        &self,
        anchor: &NewAnchor,
        label: &str,
    ) -> Result<(AnchorRow, MarkRow), StorageError> {
        let mut outcome: Option<(AnchorRow, MarkRow)> = None;
        self.write_atomically(|| {
            let created = self.create_anchor(anchor)?;
            let mark = self.create_text_mark(&NewTextMark {
                case_id: anchor.case_id.clone(),
                anchor_id: created.id.clone(),
                label: label.to_string(),
            })?;
            outcome = Some((created, mark));
            Ok(())
        })?;
        outcome.ok_or(StorageError::NotFound)
    }

    /// Metin seçiminden **kaynaklı not**: anchor ve not tek işlemde yazılır.
    /// Notun kaynak listesi çağıranın verdikleriyle değil, oluşan anchor'la
    /// kurulur; böylece doğrulanmamış kaynak kimliği notta kalmaz.
    pub fn note_from_selection(
        &self,
        anchor: &NewAnchor,
        note: &NewNote,
    ) -> Result<(AnchorRow, NoteRow), StorageError> {
        if note.body.trim().is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        let mut outcome: Option<(AnchorRow, NoteRow)> = None;
        self.write_atomically(|| {
            let created = self.create_anchor(anchor)?;
            let saved = self.create_note(
                &anchor.case_id,
                &NewNote {
                    title: note.title.clone(),
                    body: note.body.clone(),
                    source_anchor_ids: vec![created.id.clone()],
                },
            )?;
            outcome = Some((created, saved));
            Ok(())
        })?;
        outcome.ok_or(StorageError::NotFound)
    }

    /// Bir sürümün görünür işaretleri (silinmişler döner).
    pub fn marks_for_version(
        &self,
        document_version_id: &str,
    ) -> Result<Vec<MarkRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,document_version_id,parse_run_id,anchor_id,page_id,mark_kind,
                    start_offset,end_offset,quoted_text,rects_json,label,created_at
             FROM source_marks
             WHERE document_version_id=?1 AND deleted_at IS NULL
             ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map([document_version_id], map_mark)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// İşareti siler (geri dönüşümlü: satır kalır, `deleted_at` yazılır).
    pub fn delete_mark(&self, mark_id: &str) -> Result<(), StorageError> {
        let affected = self.conn().execute(
            "UPDATE source_marks SET deleted_at=?2 WHERE id=?1 AND deleted_at IS NULL",
            rusqlite::params![mark_id, now_iso()],
        )?;
        if affected == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit(
            "source_mark",
            Some(mark_id),
            "delete",
            "local-user",
            &serde_json::json!({}),
        )?;
        Ok(())
    }

    pub fn get_mark(&self, mark_id: &str) -> Result<Option<MarkRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,document_version_id,parse_run_id,anchor_id,page_id,mark_kind,
                    start_offset,end_offset,quoted_text,rects_json,label,created_at
             FROM source_marks WHERE id=?1",
        )?;
        Ok(stmt.query_row([mark_id], map_mark).optional()?)
    }

    fn mark_for_anchor(&self, anchor_id: &str) -> Result<Option<MarkRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,document_version_id,parse_run_id,anchor_id,page_id,mark_kind,
                    start_offset,end_offset,quoted_text,rects_json,label,created_at
             FROM source_marks
             WHERE anchor_id=?1 AND mark_kind='text' AND deleted_at IS NULL
             LIMIT 1",
        )?;
        Ok(stmt.query_row([anchor_id], map_mark).optional()?)
    }

    /// Çağıran açık bir işlem içindeyse ona katılır; yoksa kendi işlemini açar.
    /// (Aynı desen `notes::note_write` içinde; iç içe işlem SQLite'ta hata verir.)
    fn write_atomically<F>(&self, run: F) -> Result<(), StorageError>
    where
        F: FnOnce() -> Result<(), StorageError>,
    {
        if self.conn().is_autocommit() {
            let tx = self.conn().unchecked_transaction()?;
            run()?;
            tx.commit()?;
            Ok(())
        } else {
            run()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::{DatabaseKey, NewPage};

    fn setup() -> (Database, String, String, String, String, String) {
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
        let blob = db.create_blob_record("st-mark", 10, "aad").expect("blob");
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
                text: "Mahkeme itirazın reddine karar verdi.".into(),
                width: Some(595.0),
                height: Some(842.0),
                origin: "native".into(),
                warnings: vec![],
                rotation: 0,
                coordinate_space: Some("unrotated_cropbox_points".into()),
                page_label: None,
                blocks: vec![],
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
        let page: String = db
            .conn()
            .query_row(
                "SELECT id FROM pages WHERE parse_run_id=?1",
                [&run.id],
                |r| r.get(0),
            )
            .expect("page");
        (db, case.id, version.id, run.id, segment, page)
    }

    fn anchor_input(case_id: &str, version_id: &str, run_id: &str, segment_id: &str) -> NewAnchor {
        let quote = "itirazın reddine";
        NewAnchor {
            case_id: case_id.to_string(),
            document_version_id: version_id.to_string(),
            parse_run_id: run_id.to_string(),
            segment_id: segment_id.to_string(),
            start_offset: 8,
            end_offset: 8 + i64::try_from(quote.chars().count()).unwrap(),
            exact_quote: quote.to_string(),
        }
    }

    #[test]
    fn text_mark_from_selection_links_verified_anchor() {
        let (db, case_id, version_id, run_id, segment_id) = {
            let (db, c, v, r, s, _p) = setup();
            (db, c, v, r, s)
        };
        let (anchor, mark) = db
            .mark_from_selection(
                &anchor_input(&case_id, &version_id, &run_id, &segment_id),
                "Dikkat",
            )
            .expect("mark");
        assert_eq!(mark.mark_kind, "text");
        assert_eq!(mark.anchor_id.as_deref(), Some(anchor.id.as_str()));
        assert_eq!(mark.quoted_text, "itirazın reddine");
        assert_eq!(mark.label, "Dikkat");
        assert!(mark.page_id.is_some());
        assert_eq!(db.marks_for_version(&version_id).expect("liste").len(), 1);
    }

    #[test]
    fn mark_is_not_created_when_quote_does_not_match() {
        let (db, case_id, version_id, run_id, segment_id, _page) = setup();
        let mut bad = anchor_input(&case_id, &version_id, &run_id, &segment_id);
        bad.exact_quote = "uydurma alıntı".into();
        bad.end_offset = bad.start_offset + 14;
        let err = db.mark_from_selection(&bad, "x").unwrap_err();
        assert_eq!(err.code(), "E_QUOTE_NOT_FOUND");
        // Yarım kayıt kalmaz: ne anchor ne işaret.
        assert_eq!(db.marks_for_version(&version_id).expect("liste").len(), 0);
        assert_eq!(db.list_anchors(&case_id).expect("anchor").len(), 0);
    }

    #[test]
    fn same_anchor_is_marked_once() {
        let (db, case_id, version_id, run_id, segment_id, _page) = setup();
        let anchor = db
            .create_anchor(&anchor_input(&case_id, &version_id, &run_id, &segment_id))
            .expect("anchor");
        let first = db
            .create_text_mark(&NewTextMark {
                case_id: case_id.clone(),
                anchor_id: anchor.id.clone(),
                label: "ilk".into(),
            })
            .expect("mark");
        let second = db
            .create_text_mark(&NewTextMark {
                case_id,
                anchor_id: anchor.id,
                label: "ikinci".into(),
            })
            .expect("mark");
        assert_eq!(first.id, second.id);
        assert_eq!(second.label, "ilk");
    }

    #[test]
    fn region_mark_is_validated_against_page_geometry() {
        let (db, case_id, version_id, _run, _segment, page_id) = setup();
        let ok = db
            .create_region_mark(&NewRegionMark {
                case_id: case_id.clone(),
                document_version_id: version_id.clone(),
                page_id: page_id.clone(),
                rects: vec![[10.0, 20.0, 110.0, 60.0]],
                label: "Bölge".into(),
            })
            .expect("mark");
        assert_eq!(ok.mark_kind, "region");
        assert_eq!(ok.rects, vec![[10.0, 20.0, 110.0, 60.0]]);
        assert!(ok.anchor_id.is_none());
        assert_eq!(ok.quoted_text, "");

        let outside = db.create_region_mark(&NewRegionMark {
            case_id: case_id.clone(),
            document_version_id: version_id.clone(),
            page_id: page_id.clone(),
            rects: vec![[0.0, 0.0, 600.0, 60.0]],
            label: String::new(),
        });
        assert_eq!(outside.unwrap_err().code(), "E_DB_SETTING");

        let empty = db.create_region_mark(&NewRegionMark {
            case_id: case_id.clone(),
            document_version_id: version_id.clone(),
            page_id,
            rects: vec![],
            label: String::new(),
        });
        assert_eq!(empty.unwrap_err().code(), "E_DB_SETTING");
    }

    #[test]
    fn region_mark_requires_page_of_the_same_version() {
        let (db, case_id, version_id, _run, _segment, _page) = setup();
        let err = db
            .create_region_mark(&NewRegionMark {
                case_id,
                document_version_id: version_id,
                page_id: "başka-sayfa".into(),
                rects: vec![[1.0, 1.0, 2.0, 2.0]],
                label: String::new(),
            })
            .unwrap_err();
        assert_eq!(err.code(), "E_NOT_FOUND");
    }

    #[test]
    fn deleted_mark_disappears_but_row_stays() {
        let (db, case_id, version_id, run_id, segment_id, _page) = setup();
        let (_, mark) = db
            .mark_from_selection(
                &anchor_input(&case_id, &version_id, &run_id, &segment_id),
                "",
            )
            .expect("mark");
        db.delete_mark(&mark.id).expect("sil");
        assert_eq!(db.marks_for_version(&version_id).expect("liste").len(), 0);
        assert!(db.get_mark(&mark.id).expect("oku").is_some());
        assert_eq!(db.delete_mark(&mark.id).unwrap_err().code(), "E_NOT_FOUND");
    }

    #[test]
    fn note_from_selection_links_note_to_verified_anchor() {
        let (db, case_id, version_id, run_id, segment_id, _page) = setup();
        let (anchor, note) = db
            .note_from_selection(
                &anchor_input(&case_id, &version_id, &run_id, &segment_id),
                &NewNote {
                    title: "Red gerekçesi".into(),
                    body: "Mahkeme itirazın reddine karar verdi.".into(),
                    source_anchor_ids: vec!["uydurma-id".into()],
                },
            )
            .expect("not");
        assert_eq!(note.source_anchor_ids, vec![anchor.id.clone()]);
        assert_eq!(note.case_id, case_id);
    }

    #[test]
    fn note_from_selection_rejects_empty_body_without_writing_anchor() {
        let (db, case_id, version_id, run_id, segment_id, _page) = setup();
        let err = db
            .note_from_selection(
                &anchor_input(&case_id, &version_id, &run_id, &segment_id),
                &NewNote {
                    title: "Boş".into(),
                    body: "   ".into(),
                    source_anchor_ids: vec![],
                },
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_DB_SETTING");
        assert_eq!(db.list_anchors(&case_id).expect("anchor").len(), 0);
    }
}
