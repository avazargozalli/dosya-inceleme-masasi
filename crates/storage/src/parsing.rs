//! Ayrıştırma koşuları: her metin çıkarımı sürümlü bir `parse_run` üretir.
//! Eski çıkarımın üzerine yazılmaz (INV-01, ADR-09); kaynak konumları hangi
//! koşuya bağlıysa o koşu açılır.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{now_iso, Database, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseRun {
    pub id: String,
    pub document_version_id: String,
    pub case_id: String,
    pub engine: String,
    pub engine_version: String,
    pub status: String,
    pub pages_expected: Option<i64>,
    pub pages_processed: i64,
    pub warnings: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageRow {
    pub id: String,
    pub segment_id: Option<String>,
    pub page_index: i64,
    pub text: String,
    pub text_origin: String,
    pub quality_flags: String,
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// Sayfanın gösterim döndürmesi (0/90/180/270). Koordinatlar döndürme
    /// **öncesi** uzaydadır; görüntülemek için bu değer uygulanır.
    pub rotation: i64,
    /// Koordinat uzayı adı (`unrotated_cropbox_points`, `image_pixels`,
    /// `structure`). Bilinmiyorsa `null`: kutu yok demektir, tahmin edilmez.
    pub coordinate_space: Option<String>,
    /// Blok karakter aralıkları ve kutuları; metinle aynı çıkarımdan gelir.
    pub blocks: Vec<SegmentBlock>,
}

/// `text_segments.bbox_list` kaydı: `raw_text` içindeki `start..end` aralığı ve
/// o aralığın koordinatları (`bbox` blok, `lines` satır kutuları).
///
/// Offsetler Unicode scalar birimidir ve segmentin **kendi** metnine göredir;
/// ayrı bir çıkarımdan gelen kutu ile metin karıştırılmaz.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentBlock {
    pub start: i64,
    pub end: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bbox: Option<[f64; 4]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<[f64; 4]>,
}

pub(crate) fn parse_blocks(raw: Option<String>) -> Vec<SegmentBlock> {
    raw.and_then(|text| serde_json::from_str::<Vec<SegmentBlock>>(&text).ok())
        .unwrap_or_default()
}

/// Verilen karakter aralığıyla kesişen blokların kutuları.
///
/// Blok sınırları karakter aralığı taşıdığı için alıntının hangi bloklara
/// düştüğü kesin bilinir; satır düzeyinde bölme yapılmaz (satır offsetleri
/// kayıtlı değildir) ve bu sınır çağırana `covered_blocks` ile bildirilir.
pub(crate) fn block_quads(
    blocks: &[SegmentBlock],
    start: Option<i64>,
    end: Option<i64>,
) -> Vec<[f64; 4]> {
    let mut quads = Vec::new();
    for block in blocks {
        let overlaps = match (start, end) {
            (Some(range_start), Some(range_end)) => {
                block.end > range_start && block.start < range_end
            }
            _ => true,
        };
        if !overlaps {
            continue;
        }
        if let Some(bbox) = block.bbox {
            quads.push(bbox);
        }
    }
    quads
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobRef {
    pub blob_id: String,
    pub storage_id: String,
    pub aad_ref: String,
    pub sha256: String,
    pub mime: String,
    pub byte_size: i64,
}

#[derive(Debug, Clone, Default)]
pub struct NewPage {
    pub index: i64,
    pub text: String,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub origin: String,
    pub warnings: Vec<String>,
    /// Gösterim döndürmesi (0/90/180/270); koordinatlar döndürme öncesidir.
    pub rotation: i64,
    pub coordinate_space: Option<String>,
    pub blocks: Vec<SegmentBlock>,
}

#[must_use]
pub fn settings_hash(engine: &str, engine_version: &str, settings: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(engine.as_bytes());
    hasher.update(b"|");
    hasher.update(engine_version.as_bytes());
    hasher.update(b"|");
    hasher.update(settings.as_bytes());
    hex::encode(hasher.finalize())
}

impl Database {
    /// Sürümün şifreli blob referansı (çözme AAD'si dahil).
    pub fn blob_for_version(&self, version_id: &str) -> Result<Option<BlobRef>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT b.id,b.storage_id,b.wrapped_key_ref,v.original_sha256,v.detected_mime,v.byte_size
             FROM document_versions v JOIN blobs b ON b.id = v.blob_id WHERE v.id = ?1",
        )?;
        Ok(stmt
            .query_row([version_id], |row| {
                Ok(BlobRef {
                    blob_id: row.get(0)?,
                    storage_id: row.get(1)?,
                    aad_ref: row.get(2)?,
                    sha256: row.get(3)?,
                    mime: row.get(4)?,
                    byte_size: row.get(5)?,
                })
            })
            .optional()?)
    }

    /// Yeni ayrıştırma koşusu başlatır. Aynı sürüm için birden çok koşu olabilir;
    /// hangisinin güncel olduğu `created_at` ile bellidir.
    pub fn start_parse_run(
        &self,
        version_id: &str,
        case_id: &str,
        engine: &str,
        engine_version: &str,
        settings: &str,
    ) -> Result<ParseRun, StorageError> {
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO parse_runs (id,document_version_id,case_id,engine,engine_version,settings_hash,status,pages_processed,warnings,started_at,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,'running',0,'[]',?7,?7)",
            rusqlite::params![id, version_id, case_id, engine, engine_version, settings_hash(engine, engine_version, settings), now],
        )?;
        self.get_parse_run(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_parse_run(&self, id: &str) -> Result<Option<ParseRun>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,document_version_id,case_id,engine,engine_version,status,pages_expected,pages_processed,warnings,started_at,completed_at,created_at
             FROM parse_runs WHERE id=?1",
        )?;
        Ok(stmt
            .query_row([id], |row| {
                Ok(ParseRun {
                    id: row.get(0)?,
                    document_version_id: row.get(1)?,
                    case_id: row.get(2)?,
                    engine: row.get(3)?,
                    engine_version: row.get(4)?,
                    status: row.get(5)?,
                    pages_expected: row.get(6)?,
                    pages_processed: row.get(7)?,
                    warnings: row.get(8)?,
                    started_at: row.get(9)?,
                    completed_at: row.get(10)?,
                    created_at: row.get(11)?,
                })
            })
            .optional()?)
    }

    /// Sayfaları ve metin segmentlerini tek transaction'da yazar. Çökme
    /// durumunda yarım ayrıştırma "tamamlandı" görünmez: koşu `running` kalır
    /// ve yeniden denenir.
    pub fn store_pages(
        &self,
        parse_run_id: &str,
        case_id: &str,
        pages: &[NewPage],
    ) -> Result<usize, StorageError> {
        let tx = self.conn().unchecked_transaction()?;
        let now = now_iso();
        let mut stored = 0usize;
        for page in pages {
            let page_id = uuid::Uuid::now_v7().to_string();
            let flags = serde_json::to_string(&page.warnings).unwrap_or_else(|_| "[]".into());
            tx.execute(
                "INSERT INTO pages (id,parse_run_id,page_index,width,height,rotation,coordinate_space,text_origin,quality_flags)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                rusqlite::params![
                    page_id,
                    parse_run_id,
                    page.index,
                    page.width,
                    page.height,
                    page.rotation,
                    page.coordinate_space,
                    page.origin,
                    flags
                ],
            )?;
            if !page.text.trim().is_empty() {
                let geometry = serde_json::to_string(&page.blocks).unwrap_or_else(|_| "[]".into());
                tx.execute(
                    "INSERT INTO text_segments (id,parse_run_id,page_id,case_id,structural_path,raw_text,normalized_text,offset_map,bbox_list,created_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,'[]',?8,?9)",
                    rusqlite::params![
                        uuid::Uuid::now_v7().to_string(),
                        parse_run_id,
                        page_id,
                        case_id,
                        format!("page/{}", page.index),
                        page.text,
                        dim_evidence::normalize(&page.text),
                        geometry,
                        now,
                    ],
                )?;
                stored += 1;
            }
        }
        tx.execute(
            "UPDATE parse_runs SET pages_expected=?1, pages_processed=?1 WHERE id=?2",
            rusqlite::params![i64::try_from(pages.len()).unwrap_or(i64::MAX), parse_run_id],
        )?;
        tx.commit()?;
        Ok(stored)
    }

    pub fn finish_parse_run(
        &self,
        parse_run_id: &str,
        status: &str,
        warnings: &[String],
    ) -> Result<(), StorageError> {
        if !matches!(status, "succeeded" | "partial" | "failed" | "cancelled") {
            return Err(StorageError::InvalidSetting);
        }
        let now = now_iso();
        self.conn().execute(
            "UPDATE parse_runs SET status=?1, warnings=?2, completed_at=?3 WHERE id=?4",
            rusqlite::params![
                status,
                serde_json::to_string(warnings).unwrap_or_else(|_| "[]".into()),
                now,
                parse_run_id
            ],
        )?;
        self.append_audit(
            "parse_run",
            Some(parse_run_id),
            status,
            "local-worker",
            &serde_json::json!({"warnings": warnings.len()}),
        )?;
        Ok(())
    }

    pub fn list_parse_runs(&self, version_id: &str) -> Result<Vec<ParseRun>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,document_version_id,case_id,engine,engine_version,status,pages_expected,pages_processed,warnings,started_at,completed_at,created_at
             FROM parse_runs WHERE document_version_id=?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([version_id], |row| {
            Ok(ParseRun {
                id: row.get(0)?,
                document_version_id: row.get(1)?,
                case_id: row.get(2)?,
                engine: row.get(3)?,
                engine_version: row.get(4)?,
                status: row.get(5)?,
                pages_expected: row.get(6)?,
                pages_processed: row.get(7)?,
                warnings: row.get(8)?,
                started_at: row.get(9)?,
                completed_at: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn pages_for_run(&self, parse_run_id: &str) -> Result<Vec<PageRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT p.id,
                    (SELECT s.id FROM text_segments s WHERE s.page_id=p.id LIMIT 1) AS segment_id,
                    p.page_index,
                    COALESCE((SELECT s.raw_text FROM text_segments s WHERE s.page_id=p.id LIMIT 1),'') AS text,
                    p.text_origin,p.quality_flags,p.width,p.height,p.rotation,p.coordinate_space,
                    (SELECT s.bbox_list FROM text_segments s WHERE s.page_id=p.id LIMIT 1) AS bbox_list
             FROM pages p WHERE p.parse_run_id=?1 ORDER BY p.page_index",
        )?;
        let rows = stmt.query_map([parse_run_id], |row| {
            let raw_blocks: Option<String> = row.get(10)?;
            Ok(PageRow {
                id: row.get(0)?,
                segment_id: row.get(1)?,
                page_index: row.get(2)?,
                text: row.get(3)?,
                text_origin: row.get(4)?,
                quality_flags: row.get(5)?,
                width: row.get(6)?,
                height: row.get(7)?,
                rotation: row.get(8)?,
                coordinate_space: row.get(9)?,
                blocks: parse_blocks(raw_blocks),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Segmentin koordinat geometrisi: hangi uzayda, hangi kutular.
    ///
    /// `start`/`end` verilirse yalnız kesişen bloklar döner; böylece alıntının
    /// sayfadaki yeri blok düzeyinde gösterilebilir (F06-T08). Kutu yoksa
    /// `quads` boş kalır — konum uydurulmaz.
    pub fn segment_geometry(
        &self,
        segment_id: &str,
        start: Option<i64>,
        end: Option<i64>,
    ) -> Result<Option<SegmentGeometry>, StorageError> {
        let row = self
            .conn()
            .query_row(
                "SELECT s.id, s.bbox_list, p.rotation, p.coordinate_space, p.width, p.height
                 FROM text_segments s LEFT JOIN pages p ON p.id = s.page_id
                 WHERE s.id = ?1",
                [segment_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<f64>>(4)?,
                        row.get::<_, Option<f64>>(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((id, raw_blocks, rotation, coordinate_space, width, height)) = row else {
            return Ok(None);
        };
        let blocks = parse_blocks(raw_blocks);
        let total_blocks = blocks.len();
        let quads = block_quads(&blocks, start, end);
        Ok(Some(SegmentGeometry {
            segment_id: id,
            coordinate_space,
            page_rotation: rotation.unwrap_or(0),
            page_width: width,
            page_height: height,
            covered_blocks: quads.len(),
            total_blocks,
            quads,
        }))
    }

    /// Dosya kapsamlı tam metin araması. `case_id` zorunludur; başka dosyanın
    /// segmenti sonuca sızmaz. Sorgu parametre olarak bağlanır (SQL birleştirme yok).
    pub fn search_segments(
        &self,
        case_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<SearchHit>, StorageError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let escaped = trimmed.replace('"', " ");
        let mut stmt = self.conn().prepare(
            "SELECT s.id, s.page_id, s.raw_text, p.page_index, bm25(text_segments_fts),
                    pr.document_version_id, dv.document_id, d.display_name
             FROM text_segments_fts
             JOIN text_segments s ON s.rowid = text_segments_fts.rowid
             LEFT JOIN pages p ON p.id = s.page_id
             LEFT JOIN parse_runs pr ON pr.id = s.parse_run_id
             LEFT JOIN document_versions dv ON dv.id = pr.document_version_id
             LEFT JOIN documents d ON d.id = dv.document_id
             WHERE text_segments_fts MATCH ?1 AND s.case_id = ?2
             ORDER BY bm25(text_segments_fts) LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![format!("\"{escaped}\"*"), case_id, limit],
            |row| {
                Ok(SearchHit {
                    segment_id: row.get(0)?,
                    page_id: row.get(1)?,
                    document_version_id: row.get(5)?,
                    document_id: row.get(6)?,
                    document_name: row.get(7)?,
                    raw_text: row.get(2)?,
                    page_index: row.get(3)?,
                    rank: row.get(4)?,
                })
            },
        )?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentGeometry {
    pub segment_id: String,
    pub coordinate_space: Option<String>,
    pub page_rotation: i64,
    pub page_width: Option<f64>,
    pub page_height: Option<f64>,
    pub quads: Vec<[f64; 4]>,
    pub covered_blocks: usize,
    pub total_blocks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub segment_id: String,
    pub page_id: Option<String>,
    /// Sonucun hangi belge sürümünden geldiği; okuyucu bu sürümü açar (F07).
    pub document_version_id: Option<String>,
    pub document_id: Option<String>,
    pub document_name: Option<String>,
    pub raw_text: String,
    pub page_index: Option<i64>,
    pub rank: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn setup() -> (Database, String, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([13u8; 32])).expect("db");
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
        let blob = db.create_blob_record("st1", 10, "aad").expect("blob");
        let doc = db
            .create_document(&case.id, "Belge", "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &doc.id,
                &case.id,
                &blob,
                &"d".repeat(64),
                10,
                "application/pdf",
            )
            .expect("v");
        (db, case.id, version.id)
    }

    #[test]
    fn parse_run_stores_pages_and_search_finds_them() {
        let (db, case_id, version_id) = setup();
        let run = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        let stored = db
            .store_pages(
                &run.id,
                &case_id,
                &[
                    NewPage {
                        index: 0,
                        text: "İtirazın reddine karar verildi".into(),
                        width: Some(595.0),
                        height: Some(842.0),
                        origin: "native".into(),
                        warnings: vec![],
                        ..Default::default()
                    },
                    NewPage {
                        index: 1,
                        text: "   ".into(),
                        width: Some(595.0),
                        height: Some(842.0),
                        origin: "none".into(),
                        warnings: vec!["metin katmanı yok".into()],
                        ..Default::default()
                    },
                ],
            )
            .expect("sayfalar");
        assert_eq!(stored, 1);
        db.finish_parse_run(&run.id, "partial", &["sayfa 2: metin yok".into()])
            .expect("bitir");

        let pages = db.pages_for_run(&run.id).expect("sayfalar");
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[1].text_origin, "none");

        let hits = db.search_segments(&case_id, "itiraz", 10).expect("arama");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].page_index, Some(0));
        // Sonuç, doğru sürümü açabilmek için belge kimliğini taşır (F07 kabul testi).
        assert_eq!(
            hits[0].document_version_id.as_deref(),
            Some(version_id.as_str())
        );
        assert_eq!(hits[0].document_name.as_deref(), Some("Belge"));
        assert!(hits[0].document_id.is_some());
    }

    #[test]
    fn search_hit_points_at_its_own_version_not_the_latest() {
        let (db, case_id, version_id) = setup();
        let first = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        db.store_pages(
            &first.id,
            &case_id,
            &[NewPage {
                index: 0,
                text: "eski sürümde kira kaydı".into(),
                width: None,
                height: None,
                origin: "native".into(),
                warnings: vec![],
                ..Default::default()
            }],
        )
        .expect("sayfa");
        db.finish_parse_run(&first.id, "succeeded", &[])
            .expect("bitir");

        let doc_id = db
            .list_documents(&case_id)
            .expect("belgeler")
            .into_iter()
            .next()
            .expect("belge")
            .id;
        let blob = db.create_blob_record("st-v2", 20, "aad-v2").expect("blob");
        let second_version = db
            .add_version(
                &doc_id,
                &case_id,
                &blob,
                &"f".repeat(64),
                20,
                "application/pdf",
            )
            .expect("sürüm");
        let second = db
            .start_parse_run(&second_version.id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        db.store_pages(
            &second.id,
            &case_id,
            &[NewPage {
                index: 0,
                text: "yeni sürümde kira kaydı".into(),
                width: None,
                height: None,
                origin: "native".into(),
                warnings: vec![],
                ..Default::default()
            }],
        )
        .expect("sayfa");

        let hits = db.search_segments(&case_id, "kira", 10).expect("arama");
        assert_eq!(hits.len(), 2);
        let old = hits
            .iter()
            .find(|h| h.document_version_id.as_deref() == Some(first.document_version_id.as_str()))
            .expect("eski sürüm sonucu");
        assert!(old.raw_text.contains("eski sürümde"));
        let new = hits
            .iter()
            .find(|h| h.document_version_id.as_deref() == Some(second_version.id.as_str()))
            .expect("yeni sürüm sonucu");
        assert!(new.raw_text.contains("yeni sürümde"));
    }

    #[test]
    fn search_never_crosses_case_boundary() {
        let (db, case_id, version_id) = setup();
        let run = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        db.store_pages(
            &run.id,
            &case_id,
            &[NewPage {
                index: 0,
                text: "gizli kira metni".into(),
                width: None,
                height: None,
                origin: "native".into(),
                warnings: vec![],
                ..Default::default()
            }],
        )
        .expect("sayfa");

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
        assert!(db
            .search_segments(&other.id, "kira", 10)
            .expect("arama")
            .is_empty());
    }

    #[test]
    fn blob_reference_resolves_aad() {
        let (db, _case_id, version_id) = setup();
        let blob = db
            .blob_for_version(&version_id)
            .expect("blob")
            .expect("var");
        assert_eq!(blob.storage_id, "st1");
        assert_eq!(blob.aad_ref, "aad");
        assert_eq!(blob.mime, "application/pdf");
    }

    #[test]
    fn invalid_finish_status_rejected() {
        let (db, case_id, version_id) = setup();
        let run = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        assert!(db.finish_parse_run(&run.id, "bitti", &[]).is_err());
    }

    #[test]
    fn page_geometry_roundtrips_and_range_selects_overlapping_blocks() {
        let (db, case_id, version_id) = setup();
        let run = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        let text = "Birinci blok\nİkinci blok\n";
        let first_bbox = [10.0, 10.0, 200.0, 24.0];
        let second_bbox = [10.0, 40.0, 180.0, 54.0];
        db.store_pages(
            &run.id,
            &case_id,
            &[NewPage {
                index: 0,
                text: text.into(),
                width: Some(300.0),
                height: Some(400.0),
                origin: "native".into(),
                warnings: vec![],
                rotation: 90,
                coordinate_space: Some("unrotated_cropbox_points".into()),
                blocks: vec![
                    SegmentBlock {
                        start: 0,
                        end: 13,
                        bbox: Some(first_bbox),
                        lines: vec![[10.0, 10.0, 200.0, 24.0]],
                    },
                    SegmentBlock {
                        start: 13,
                        end: 25,
                        bbox: Some(second_bbox),
                        lines: vec![],
                    },
                ],
            }],
        )
        .expect("sayfa");

        let pages = db.pages_for_run(&run.id).expect("sayfalar");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].rotation, 90);
        assert_eq!(
            pages[0].coordinate_space.as_deref(),
            Some("unrotated_cropbox_points")
        );
        assert_eq!(pages[0].blocks.len(), 2);
        assert_eq!(pages[0].blocks[1].bbox, Some(second_bbox));
        // Blok aralıkları segment metniyle birebir aynı yerleri gösterir.
        let chars: Vec<char> = text.chars().collect();
        let block_text: String = chars[13..25].iter().collect();
        assert_eq!(block_text, "İkinci blok\n");

        let segment_id = pages[0].segment_id.clone().expect("segment");
        let geometry = db
            .segment_geometry(&segment_id, Some(13), Some(25))
            .expect("geometri")
            .expect("kayıt");
        assert_eq!(geometry.total_blocks, 2);
        assert_eq!(geometry.covered_blocks, 1);
        assert_eq!(geometry.quads, vec![second_bbox]);
        assert_eq!(geometry.page_rotation, 90);
        assert_eq!(geometry.page_width, Some(300.0));

        let all = db
            .segment_geometry(&segment_id, None, None)
            .expect("hepsi")
            .expect("kayıt");
        assert_eq!(all.quads.len(), 2);
        assert!(db
            .segment_geometry("olmayan-segment", None, None)
            .expect("arama")
            .is_none());
    }

    #[test]
    fn malformed_geometry_record_does_not_crash_and_is_not_invented() {
        let (db, case_id, version_id) = setup();
        let run = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        db.store_pages(
            &run.id,
            &case_id,
            &[NewPage {
                index: 0,
                text: "Metin".into(),
                width: None,
                height: None,
                origin: "native".into(),
                warnings: vec![],
                ..Default::default()
            }],
        )
        .expect("sayfa");
        db.conn()
            .execute(
                "UPDATE text_segments SET bbox_list='bozuk-json' WHERE parse_run_id=?1",
                [&run.id],
            )
            .expect("güncelle");

        let pages = db.pages_for_run(&run.id).expect("sayfalar");
        assert!(pages[0].blocks.is_empty());
        let geometry = db
            .segment_geometry(pages[0].segment_id.as_deref().unwrap(), None, None)
            .expect("geometri")
            .expect("kayıt");
        assert!(geometry.quads.is_empty());
        assert_eq!(geometry.total_blocks, 0);
    }
}
