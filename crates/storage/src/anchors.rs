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
    /// Anchor belgenin **güncel** sürümüne mi bağlı (F07-T12).
    pub is_current_version: bool,
    /// Belgenin güncel sürüm kimliği (belge yoksa `None`).
    pub current_version_id: Option<String>,
    /// Anchor'ın bağlı olduğu sürüm numarası.
    pub version_no: Option<i64>,
    /// Belgenin güncel sürüm numarası.
    pub current_version_no: Option<i64>,
}

/// Güncel sürümde alıntının bulunduğu aday konum (yeniden bağlama önerisi).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebindCandidate {
    pub segment_id: String,
    pub page_id: Option<String>,
    pub page_index: Option<i64>,
    pub start_offset: i64,
    pub end_offset: i64,
}

/// Yeniden bağlama önerisi için anchor + belge sürüm bilgisi.
struct AnchorVersionInfo {
    case_id: String,
    anchor_version_id: String,
    quote: String,
    current_version_id: Option<String>,
    version_no: Option<i64>,
    current_version_no: Option<i64>,
}

/// Kopmuş/eski bağlantı için yeniden bağlama önerisi. `status`:
/// `current` (zaten güncel), `exact` (tek eşleşme), `ambiguous` (birden fazla),
/// `unresolved` (güncel sürümde bulunamadı).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebindSuggestion {
    pub anchor_id: String,
    pub quote: String,
    pub current_version_id: Option<String>,
    pub version_no: Option<i64>,
    pub current_version_no: Option<i64>,
    pub status: String,
    pub candidates: Vec<RebindCandidate>,
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
             FROM source_anchors WHERE case_id=?1 AND anchor_status IN ('active','rebound') ORDER BY created_at DESC",
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
                        a.quote_hash, s.raw_text, s.bbox_list, p.rotation, p.coordinate_space,
                        d.current_version_id, dv.version_no, cv.version_no
                 FROM source_anchors a
                 LEFT JOIN document_versions dv ON dv.id = a.document_version_id
                 LEFT JOIN documents d ON d.id = dv.document_id
                 LEFT JOIN document_versions cv ON cv.id = d.current_version_id
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
                        current_version_id: row.get(18)?,
                        version_no: row.get(19)?,
                        current_version_no: row.get(20)?,
                        is_current_version: row.get::<_, Option<String>>(18)?
                            == Some(row.get::<_, String>(2)?),
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Belgenin güncel sürümünde alıntıyı arar (yeniden bağlama önerisi).
    /// Eşleşme sayısı tek ise `exact`, birden fazlaysa `ambiguous`, hiç yoksa
    /// `unresolved` döner; anchor zaten güncel sürümdeyse `current`.
    pub fn rebind_suggestion(&self, anchor_id: &str) -> Result<RebindSuggestion, StorageError> {
        let row: Option<AnchorVersionInfo> = self
            .conn()
            .query_row(
                "SELECT a.case_id, a.document_version_id, a.exact_quote,
                        d.current_version_id, dv.version_no, cv.version_no
                 FROM source_anchors a
                 JOIN document_versions dv ON dv.id = a.document_version_id
                 JOIN documents d ON d.id = dv.document_id
                 LEFT JOIN document_versions cv ON cv.id = d.current_version_id
                 WHERE a.id = ?1",
                [anchor_id],
                |row| {
                    Ok(AnchorVersionInfo {
                        case_id: row.get(0)?,
                        anchor_version_id: row.get(1)?,
                        quote: row.get(2)?,
                        current_version_id: row.get(3)?,
                        version_no: row.get(4)?,
                        current_version_no: row.get(5)?,
                    })
                },
            )
            .optional()?;
        let info = row.ok_or(StorageError::NotFound)?;
        let mut suggestion = RebindSuggestion {
            anchor_id: anchor_id.to_string(),
            quote: info.quote.clone(),
            current_version_id: info.current_version_id.clone(),
            version_no: info.version_no,
            current_version_no: info.current_version_no,
            status: "unresolved".into(),
            candidates: Vec::new(),
        };
        let Some(current_version_id) = info.current_version_id.clone() else {
            return Ok(suggestion);
        };
        if current_version_id == info.anchor_version_id {
            suggestion.status = "current".into();
            return Ok(suggestion);
        }
        // Güncel sürümün en son başarılı/ksmi ayrıştırması: başka sürümün
        // segmentleri karıştırılmaz.
        let run: Option<String> = self
            .conn()
            .query_row(
                "SELECT id FROM parse_runs
                 WHERE document_version_id=?1 AND status IN ('succeeded','partial')
                 ORDER BY COALESCE(completed_at, created_at) DESC, id DESC LIMIT 1",
                [&current_version_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(run_id) = run else {
            return Ok(suggestion);
        };
        let mut stmt = self.conn().prepare(
            "SELECT s.id, s.page_id, p.page_index, s.raw_text
             FROM text_segments s LEFT JOIN pages p ON p.id = s.page_id
             WHERE s.parse_run_id=?1 AND s.case_id=?2
             ORDER BY p.page_index, s.id",
        )?;
        let rows = stmt.query_map([&run_id, &info.case_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let quote = info.quote.clone();
        for row in rows {
            let (segment_id, page_id, page_index, raw) = row?;
            for (byte_index, _) in raw.match_indices(&quote) {
                let start = raw[..byte_index].chars().count();
                suggestion.candidates.push(RebindCandidate {
                    segment_id: segment_id.clone(),
                    page_id: page_id.clone(),
                    page_index,
                    start_offset: i64::try_from(start).unwrap_or(i64::MAX),
                    end_offset: i64::try_from(start + quote.chars().count()).unwrap_or(i64::MAX),
                });
                if suggestion.candidates.len() >= 20 {
                    break;
                }
            }
            if suggestion.candidates.len() >= 20 {
                break;
            }
        }
        suggestion.status = match suggestion.candidates.len() {
            0 => "unresolved".into(),
            1 => "exact".into(),
            _ => "ambiguous".into(),
        };
        Ok(suggestion)
    }

    /// Anchor'ı güncel sürümdeki doğrulanmış bir konuma yeniden bağlar.
    /// Alıntı yeni segmentte birebir durmuyorsa bağ değişmez.
    pub fn rebind_anchor(
        &self,
        anchor_id: &str,
        segment_id: &str,
        start_offset: i64,
    ) -> Result<AnchorRow, StorageError> {
        let anchor = self.get_anchor(anchor_id)?.ok_or(StorageError::NotFound)?;
        let segment: Option<(String, String, Option<String>, String)> = self
            .conn()
            .query_row(
                "SELECT raw_text, parse_run_id, page_id, case_id FROM text_segments WHERE id=?1",
                [segment_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let (raw, run_id, page_id, segment_case) = segment.ok_or(StorageError::NotFound)?;
        if segment_case != anchor.case_id {
            return Err(StorageError::NotFound);
        }
        let run: Option<(String, String)> = self
            .conn()
            .query_row(
                "SELECT document_version_id, case_id FROM parse_runs WHERE id=?1",
                [&run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (version_id, run_case) = run.ok_or(StorageError::NotFound)?;
        if run_case != anchor.case_id {
            return Err(StorageError::NotFound);
        }
        let start = usize::try_from(start_offset).map_err(|_| StorageError::InvalidSetting)?;
        // `verify_quote` başlangıç ipucunu **öneri** olarak kullanır ve alıntıyı
        // segmentin başka yerinde bulursa yine `Exact` döner. Yeniden bağlamada
        // konum birebir doğrulanır: yanlış offset "bağlandı" görüntüsü almaz.
        let scalars: Vec<char> = raw.chars().collect();
        let quote_chars: Vec<char> = anchor.exact_quote.chars().collect();
        if quote_chars.is_empty()
            || start + quote_chars.len() > scalars.len()
            || scalars[start..start + quote_chars.len()] != quote_chars[..]
        {
            return Err(StorageError::QuoteNotFound);
        }
        match verify_quote(
            &raw,
            &anchor.exact_quote,
            Some(&anchor.quote_hash),
            Some(start),
        ) {
            QuoteMatch::Exact => {}
            QuoteMatch::Ambiguous => return Err(StorageError::QuoteAmbiguous),
            QuoteMatch::NotFound | QuoteMatch::Normalized => {
                return Err(StorageError::QuoteNotFound)
            }
            QuoteMatch::HashMismatch => return Err(StorageError::QuoteHashMismatch),
        }
        let end_offset =
            start_offset + i64::try_from(anchor.exact_quote.chars().count()).unwrap_or(0);
        // Durum değişikliği ve denetim kaydı tek işlemde: denetim yazılamazsa
        // bağ da değişmez (yarım kalan "rebound" görüntüsü olmaz).
        self.write_atomically(|| {
            self.conn().execute(
                "UPDATE source_anchors
                 SET document_version_id=?2, parse_run_id=?3, page_id=?4, segment_id=?5,
                     start_offset=?6, end_offset=?7, anchor_status='rebound', updated_at=?8
                 WHERE id=?1",
                rusqlite::params![
                    anchor_id,
                    version_id,
                    run_id,
                    page_id,
                    segment_id,
                    start_offset,
                    end_offset,
                    now_iso()
                ],
            )?;
            self.append_audit(
                "source_anchor",
                Some(anchor_id),
                "rebind",
                "local-user",
                &serde_json::json!({"segmentId": segment_id, "startOffset": start_offset}),
            )?;
            Ok(())
        })?;
        self.get_anchor(anchor_id)?.ok_or(StorageError::NotFound)
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
                        path: None,
                        kind: None,
                    },
                    crate::SegmentBlock {
                        start: 13,
                        end: 25,
                        bbox: Some([10.0, 40.0, 180.0, 54.0]),
                        lines: vec![],
                        path: None,
                        kind: None,
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

    /// İki sürümlü belge: v1'e anchor bağlanır, v2 belgenin güncel sürümüdür.
    /// `second_pages` v2'nin sayfa metinleridir.
    fn setup_two_versions(
        second_pages: &[&str],
    ) -> (Database, String, String, String, String, String, String) {
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
        let blob1 = db.create_blob_record("st-a1", 10, "aad").expect("blob");
        let doc = db
            .create_document(&case.id, "Belge", "pdf", None, "unknown")
            .expect("doc");
        let v1 = db
            .add_version(
                &doc.id,
                &case.id,
                &blob1,
                &"a".repeat(64),
                10,
                "application/pdf",
            )
            .expect("v1");
        let run1 = db
            .start_parse_run(&v1.id, &case.id, "pymupdf", "1.24", "{}")
            .expect("run1");
        db.store_pages(
            &run1.id,
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
        .expect("pages1");
        db.finish_parse_run(&run1.id, "succeeded", &[])
            .expect("finish1");
        let segment1: String = db
            .conn()
            .query_row(
                "SELECT id FROM text_segments WHERE parse_run_id=?1",
                [&run1.id],
                |r| r.get(0),
            )
            .expect("segment1");
        let anchor = db
            .create_anchor(&NewAnchor {
                case_id: case.id.clone(),
                document_version_id: v1.id.clone(),
                parse_run_id: run1.id.clone(),
                segment_id: segment1,
                start_offset: 8,
                end_offset: 8 + i64::try_from("itirazın reddine".chars().count()).unwrap(),
                exact_quote: "itirazın reddine".into(),
            })
            .expect("anchor");

        let blob2 = db.create_blob_record("st-a2", 11, "aad").expect("blob2");
        let v2 = db
            .add_version(
                &doc.id,
                &case.id,
                &blob2,
                &"b".repeat(64),
                11,
                "application/pdf",
            )
            .expect("v2");
        let run2 = db
            .start_parse_run(&v2.id, &case.id, "pymupdf", "1.24", "{}")
            .expect("run2");
        let pages: Vec<NewPage> = second_pages
            .iter()
            .enumerate()
            .map(|(index, text)| NewPage {
                index: i64::try_from(index).unwrap(),
                text: (*text).to_string(),
                width: None,
                height: None,
                origin: "native".into(),
                warnings: vec![],
                ..Default::default()
            })
            .collect();
        db.store_pages(&run2.id, &case.id, &pages).expect("pages2");
        db.finish_parse_run(&run2.id, "succeeded", &[])
            .expect("finish2");
        (db, case.id, v1.id, run1.id, v2.id, anchor.id, run2.id)
    }

    #[test]
    fn rebind_suggestion_finds_single_match_in_current_version() {
        let (db, _case, _v1, _run1, v2, anchor_id, _run2) =
            setup_two_versions(&["Mahkeme, itirazın reddine karar verdi."]);
        let suggestion = db.rebind_suggestion(&anchor_id).expect("öneri");
        assert_eq!(suggestion.status, "exact");
        assert_eq!(suggestion.current_version_id.as_deref(), Some(v2.as_str()));
        assert_eq!(suggestion.version_no, Some(1));
        assert_eq!(suggestion.current_version_no, Some(2));
        assert_eq!(suggestion.candidates.len(), 1);
        assert_eq!(suggestion.candidates[0].page_index, Some(0));
        assert_eq!(suggestion.candidates[0].start_offset, 9);
    }

    #[test]
    fn rebind_suggestion_reports_ambiguous_and_unresolved() {
        let (db, _c, _v1, _r1, _v2, anchor_id, _r2) = setup_two_versions(&[
            "itirazın reddine karar verildi.",
            "Ayrıca itirazın reddine ilişkin gerekçe yazıldı.",
        ]);
        assert_eq!(
            db.rebind_suggestion(&anchor_id).expect("öneri").status,
            "ambiguous"
        );

        let (db2, _c2, _v12, _r12, _v22, anchor2, _r22) =
            setup_two_versions(&["Metin tamamen değişti."]);
        let suggestion = db2.rebind_suggestion(&anchor2).expect("öneri");
        assert_eq!(suggestion.status, "unresolved");
        assert!(suggestion.candidates.is_empty());
    }

    #[test]
    fn rebind_suggestion_is_current_for_same_version() {
        let (db, case_id, _v1, _run1, v2, _anchor, run2) =
            setup_two_versions(&["Mahkeme, itirazın reddine karar verdi."]);
        let segment: String = db
            .conn()
            .query_row(
                "SELECT id FROM text_segments WHERE parse_run_id=?1",
                [&run2],
                |r| r.get(0),
            )
            .expect("segment");
        let anchor = db
            .create_anchor(&NewAnchor {
                case_id,
                document_version_id: v2,
                parse_run_id: run2,
                segment_id: segment,
                start_offset: 9,
                end_offset: 9 + i64::try_from("itirazın reddine".chars().count()).unwrap(),
                exact_quote: "itirazın reddine".into(),
            })
            .expect("anchor");
        assert_eq!(
            db.rebind_suggestion(&anchor.id).expect("öneri").status,
            "current"
        );
    }

    #[test]
    fn rebind_moves_anchor_to_current_version_after_verification() {
        let (db, case_id, _v1, _run1, v2, anchor_id, _run2) =
            setup_two_versions(&["Mahkeme, itirazın reddine karar verdi."]);
        let suggestion = db.rebind_suggestion(&anchor_id).expect("öneri");
        let candidate = suggestion.candidates[0].clone();
        let updated = db
            .rebind_anchor(&anchor_id, &candidate.segment_id, candidate.start_offset)
            .expect("rebind");
        assert_eq!(updated.document_version_id, v2);
        assert_eq!(updated.anchor_status, "rebound");
        assert_eq!(updated.start_offset, candidate.start_offset);
        let located = db.locate_anchor(&anchor_id).expect("konum").expect("var");
        assert!(located.is_current_version);
        assert!(located.quote_intact);
        // Yeniden bağlanan anchor dosya listesinde görünür kalır.
        assert_eq!(db.list_anchors(&case_id).expect("liste").len(), 1);
        // Denetim kaydı gerçekten yazıldı ve zincir bozulmadı.
        let audited: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM audit_events WHERE entity_id=?1 AND operation='rebind'",
                [&anchor_id],
                |r| r.get(0),
            )
            .expect("denetim");
        assert_eq!(audited, 1);
        assert!(db.verify_audit_chain().expect("zincir") >= 1);
    }

    #[test]
    fn rebind_rejects_wrong_offset() {
        let (db, _c, _v1, _r1, _v2, anchor_id, _r2) =
            setup_two_versions(&["Mahkeme, itirazın reddine karar verdi."]);
        let suggestion = db.rebind_suggestion(&anchor_id).expect("öneri");
        let candidate = suggestion.candidates[0].clone();
        let err = db
            .rebind_anchor(
                &anchor_id,
                &candidate.segment_id,
                candidate.start_offset + 1,
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_QUOTE_NOT_FOUND");
        let located = db.locate_anchor(&anchor_id).expect("konum").expect("var");
        assert!(!located.is_current_version);
    }
}
