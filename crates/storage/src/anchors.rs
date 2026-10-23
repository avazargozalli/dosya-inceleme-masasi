//! Kaynak anchor'ları: bir alıntının hangi segmentte, hangi Unicode scalar
//! aralığında ve hangi belge sürümünde olduğunu **doğrulayarak** kaydeder.
//!
//! Alıntı segment metninde bulunamazsa kayıt oluşmaz (INV-02, INV-12).
//! Anchor, oluşturulduğu `parse_run`a bağlı kalır; yeni ayrıştırma eski
//! anchor'ı sessizce taşımaz (07.3).

use dim_evidence::{context_around, hash_quote, verify_quote, QuoteMatch};
use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorRow {
    pub id: String,
    pub case_id: String,
    pub document_version_id: String,
    pub parse_run_id: String,
    pub page_id: Option<String>,
    pub segment_id: String,
    pub start_offset: i64,
    pub end_offset: i64,
    pub exact_quote: String,
    pub prefix: String,
    pub suffix: String,
    pub quote_hash: String,
    pub anchor_status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAnchor {
    pub case_id: String,
    pub document_version_id: String,
    pub parse_run_id: String,
    pub segment_id: String,
    /// Unicode scalar konumu (bayt veya UTF-16 değil).
    pub start_offset: i64,
    pub end_offset: i64,
    pub exact_quote: String,
}

/// Bir anchor'ın okuyucuda açılacağı yer: belge sürümü, fiziksel sayfa ve
/// alıntının segmentte hâlâ birebir durup durmadığı (F07 "kaynağa git").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorLocation {
    pub anchor_id: String,
    pub case_id: String,
    pub document_version_id: String,
    pub document_id: Option<String>,
    pub document_name: Option<String>,
    pub parse_run_id: String,
    pub page_id: Option<String>,
    pub page_index: Option<i64>,
    pub segment_id: String,
    pub start_offset: i64,
    pub end_offset: i64,
    pub exact_quote: String,
    pub anchor_status: String,
    /// Segment metni anchor kaydıyla hâlâ birebir eşleşiyor mu (INV-02).
    pub quote_intact: bool,
    /// Alıntının düştüğü blokların kutuları (segmentin koordinat uzayında).
    /// Kutu üretilmemişse boş: konum uydurulmaz.
    pub quads: Vec<[f64; 4]>,
    pub coordinate_space: Option<String>,
    /// Sayfanın gösterim döndürmesi; kutular döndürme öncesi uzaydadır.
    pub page_rotation: i64,
}

impl Database {
    /// Alıntıyı segment üzerinde doğrular ve anchor kaydı oluşturur.
    pub fn create_anchor(&self, input: &NewAnchor) -> Result<AnchorRow, StorageError> {
        let quote = input.exact_quote.as_str();
        if quote.trim().is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        let start =
            usize::try_from(input.start_offset).map_err(|_| StorageError::InvalidSetting)?;
        let end = usize::try_from(input.end_offset).map_err(|_| StorageError::InvalidSetting)?;
        if end < start {
            return Err(StorageError::InvalidSetting);
        }

        let (raw, page_id, segment_case, segment_run): (String, Option<String>, String, String) = self
            .conn()
            .query_row(
                "SELECT raw_text, page_id, case_id, parse_run_id FROM text_segments WHERE id=?1",
                [&input.segment_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?
            .ok_or(StorageError::NotFound)?;

        // Dosya kapsamı ve ayrıştırma koşulu tutmalı.
        if segment_case != input.case_id || segment_run != input.parse_run_id {
            return Err(StorageError::NotFound);
        }

        let expected_len = quote.chars().count();
        if end.saturating_sub(start) != expected_len {
            return Err(StorageError::InvalidSetting);
        }

        let hash = hash_quote(quote);
        match verify_quote(&raw, quote, Some(&hash), Some(start)) {
            QuoteMatch::Exact => {}
            QuoteMatch::Ambiguous => return Err(StorageError::QuoteAmbiguous),
            QuoteMatch::NotFound | QuoteMatch::Normalized => {
                return Err(StorageError::QuoteNotFound)
            }
            QuoteMatch::HashMismatch => return Err(StorageError::QuoteHashMismatch),
        }

        let (prefix, suffix) = context_around(&raw, start, end, 24);
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO source_anchors (id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,exact_quote,prefix,suffix,quote_hash,anchor_status,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,'active',?13,?13)",
            rusqlite::params![
                id,
                input.case_id,
                input.document_version_id,
                input.parse_run_id,
                page_id,
                input.segment_id,
                input.start_offset,
                input.end_offset,
                quote,
                prefix,
                suffix,
                hash,
                now
            ],
        )?;
        self.append_audit(
            "source_anchor",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"caseId": input.case_id, "segmentId": input.segment_id}),
        )?;
        self.get_anchor(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_anchor(&self, id: &str) -> Result<Option<AnchorRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,exact_quote,prefix,suffix,quote_hash,anchor_status,created_at
             FROM source_anchors WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_anchor).optional()?)
    }

    pub fn list_anchors(&self, case_id: &str) -> Result<Vec<AnchorRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,document_version_id,parse_run_id,page_id,segment_id,start_offset,end_offset,exact_quote,prefix,suffix,quote_hash,anchor_status,created_at
             FROM source_anchors WHERE case_id=?1 AND anchor_status='active' ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([case_id], map_anchor)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Anchor'ı okuyucuda açılacak yere çözer: fiziksel sayfa indeksi, belge
    /// sürümü ve alıntının segmentte hâlâ birebir durduğu bilgisi.
    ///
    /// Anchor kaydı yoksa `None` döner. Alıntı artık segmentte bulunamıyorsa
    /// kayıt silinmez; `quote_intact=false` ile "kopmuş kaynak" olarak bildirilir.
    pub fn locate_anchor(&self, anchor_id: &str) -> Result<Option<AnchorLocation>, StorageError> {
        let row = self
            .conn()
            .query_row(
                "SELECT a.id, a.case_id, a.document_version_id, dv.document_id, d.display_name,
                        a.parse_run_id, a.page_id, p.page_index, a.segment_id,
                        a.start_offset, a.end_offset, a.exact_quote, a.anchor_status,
                        a.quote_hash, s.raw_text, s.bbox_list, p.rotation, p.coordinate_space
                 FROM source_anchors a
                 LEFT JOIN document_versions dv ON dv.id = a.document_version_id
                 LEFT JOIN documents d ON d.id = dv.document_id
                 LEFT JOIN pages p ON p.id = a.page_id
                 LEFT JOIN text_segments s ON s.id = a.segment_id
                 WHERE a.id = ?1",
                [anchor_id],
                |row| {
                    let start_offset: i64 = row.get(9)?;
                    let end_offset: i64 = row.get(10)?;
                    let exact_quote: String = row.get(11)?;
                    let quote_hash: String = row.get(13)?;
                    let raw: Option<String> = row.get(14)?;
                    let quote_intact = raw.as_deref().is_some_and(|text| {
                        usize::try_from(start_offset).is_ok_and(|start| {
                            verify_quote(text, &exact_quote, Some(&quote_hash), Some(start))
                                == QuoteMatch::Exact
                        })
                    });
                    let blocks = crate::parsing::parse_blocks(row.get(15)?);
                    let quads =
                        crate::parsing::block_quads(&blocks, Some(start_offset), Some(end_offset));
                    Ok(AnchorLocation {
                        anchor_id: row.get(0)?,
                        case_id: row.get(1)?,
                        document_version_id: row.get(2)?,
                        document_id: row.get(3)?,
                        document_name: row.get(4)?,
                        parse_run_id: row.get(5)?,
                        page_id: row.get(6)?,
                        page_index: row.get(7)?,
                        segment_id: row.get(8)?,
                        start_offset,
                        end_offset,
                        exact_quote,
                        anchor_status: row.get(12)?,
                        quote_intact,
                        quads,
                        coordinate_space: row.get(17)?,
                        page_rotation: row.get::<_, Option<i64>>(16)?.unwrap_or(0),
                    })
                },
            )
            .optional()?;
        Ok(row)
    }
}

fn map_anchor(row: &Row<'_>) -> rusqlite::Result<AnchorRow> {
    Ok(AnchorRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        document_version_id: row.get(2)?,
        parse_run_id: row.get(3)?,
        page_id: row.get(4)?,
        segment_id: row.get(5)?,
        start_offset: row.get(6)?,
        end_offset: row.get(7)?,
        exact_quote: row.get(8)?,
        prefix: row.get(9)?,
        suffix: row.get(10)?,
        quote_hash: row.get(11)?,
        anchor_status: row.get(12)?,
        created_at: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::{DatabaseKey, NewPage};

    fn setup() -> (Database, String, String, String, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([29u8; 32])).expect("db");
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
        let blob = db.create_blob_record("st-anchor", 10, "aad").expect("blob");
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
                text: "Mahkeme itirazın reddine karar verdi.".into(),
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
        (db, case.id, version.id, run.id, segment)
    }

    #[test]
    fn exact_quote_creates_anchor_with_context() {
        let (db, case_id, version_id, run_id, segment_id) = setup();
        let quote = "itirazın reddine";
        let anchor = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: version_id,
                parse_run_id: run_id,
                segment_id,
                start_offset: 8,
                end_offset: 8 + i64::try_from(quote.chars().count()).unwrap(),
                exact_quote: quote.into(),
            })
            .expect("anchor");
        assert_eq!(anchor.exact_quote, quote);
        assert!(anchor.prefix.contains("Mahkeme"));
        assert_eq!(anchor.quote_hash, hash_quote(quote));
        assert_eq!(db.list_anchors(&case_id).expect("liste").len(), 1);
    }

    #[test]
    fn wrong_quote_is_rejected() {
        let (db, case_id, version_id, run_id, segment_id) = setup();
        let err = db
            .create_anchor(&NewAnchor {
                case_id,
                document_version_id: version_id,
                parse_run_id: run_id,
                segment_id,
                start_offset: 8,
                end_offset: 22,
                exact_quote: "uydurma alıntı".into(),
            })
            .unwrap_err();
        assert_eq!(err.code(), "E_QUOTE_NOT_FOUND");
    }

    #[test]
    fn wrong_offset_is_rejected() {
        let (db, case_id, version_id, run_id, segment_id) = setup();
        let err = db
            .create_anchor(&NewAnchor {
                case_id,
                document_version_id: version_id,
                parse_run_id: run_id,
                segment_id,
                start_offset: 0,
                end_offset: 8,
                exact_quote: "itirazın reddine".into(),
            })
            .unwrap_err();
        // Uzunluk uyuşmazlığı önce yakalanır.
        assert_eq!(err.code(), "E_DB_SETTING");
    }

    #[test]
    fn locate_anchor_returns_page_and_intact_quote() {
        let (db, case_id, version_id, run_id, segment_id) = setup();
        let quote = "itirazın reddine";
        let anchor = db
            .create_anchor(&NewAnchor {
                case_id,
                document_version_id: version_id.clone(),
                parse_run_id: run_id,
                segment_id: segment_id.clone(),
                start_offset: 8,
                end_offset: 8 + i64::try_from(quote.chars().count()).unwrap(),
                exact_quote: quote.into(),
            })
            .expect("anchor");

        let location = db.locate_anchor(&anchor.id).expect("konum").expect("kayıt");
        assert_eq!(location.document_version_id, version_id);
        assert_eq!(location.document_name.as_deref(), Some("Belge"));
        assert_eq!(location.page_index, Some(0));
        assert_eq!(location.segment_id, segment_id);
        assert_eq!(location.start_offset, 8);
        assert!(location.quote_intact);
        assert_eq!(location.anchor_status, "active");

        // Segment metni anchor kaydından koparsa kaynak "kopmuş" bildirilir.
        db.conn()
            .execute(
                "UPDATE text_segments SET raw_text='Mahkeme tamamen farklı bir metin yazdı.' WHERE id=?1",
                [&segment_id],
            )
            .expect("güncelle");
        let drifted = db.locate_anchor(&anchor.id).expect("konum").expect("kayıt");
        assert!(!drifted.quote_intact);
    }

    #[test]
    fn locate_missing_anchor_returns_none() {
        let (db, _case_id, _version_id, _run_id, _segment_id) = setup();
        assert!(db
            .locate_anchor("yok-boyle-anchor")
            .expect("konum")
            .is_none());
    }

    #[test]
    fn locate_anchor_returns_quote_quads_in_segment_space() {
        let db = Database::open_in_memory(&DatabaseKey::new([31u8; 32])).expect("db");
        let case = db
            .create_case(&NewCase {
                title: "Konum".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        let blob = db.create_blob_record("st-quad", 10, "aad").expect("blob");
        let doc = db
            .create_document(&case.id, "Belge", "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &doc.id,
                &case.id,
                &blob,
                &"b".repeat(64),
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
                text: "Birinci blok\nİkinci blok\n".into(),
                width: Some(300.0),
                height: Some(400.0),
                origin: "native".into(),
                warnings: vec![],
                rotation: 90,
                coordinate_space: Some("unrotated_cropbox_points".into()),
                page_label: None,
                blocks: vec![
                    crate::SegmentBlock {
                        start: 0,
                        end: 13,
                        bbox: Some([10.0, 10.0, 200.0, 24.0]),
                        lines: vec![],
                    },
                    crate::SegmentBlock {
                        start: 13,
                        end: 25,
                        bbox: Some([10.0, 40.0, 180.0, 54.0]),
                        lines: vec![],
                    },
                ],
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
                segment_id: segment,
                start_offset: 13,
                end_offset: 24,
                exact_quote: "İkinci blok".into(),
            })
            .expect("anchor");

        let location = db.locate_anchor(&anchor.id).expect("konum").expect("kayıt");
        assert!(location.quote_intact);
        assert_eq!(location.page_rotation, 90);
        assert_eq!(
            location.coordinate_space.as_deref(),
            Some("unrotated_cropbox_points")
        );
        // Alıntı yalnız ikinci blokta; kutu o bloğun kutusudur, uydurulmaz.
        assert_eq!(location.quads, vec![[10.0, 40.0, 180.0, 54.0]]);
    }

    #[test]
    fn cross_case_anchor_rejected() {
        let (db, _case_id, version_id, run_id, segment_id) = setup();
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
            .create_anchor(&NewAnchor {
                case_id: other.id,
                document_version_id: version_id,
                parse_run_id: run_id,
                segment_id,
                start_offset: 8,
                end_offset: 24,
                exact_quote: "itirazın reddine".into(),
            })
            .unwrap_err();
        assert_eq!(err.code(), "E_NOT_FOUND");
    }
}
