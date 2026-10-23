//! Ayrıştırma koşuları: her metin çıkarımı sürümlü bir `parse_run` üretir.
//! Eski çıkarımın üzerine yazılmaz (INV-01, ADR-09); kaynak konumları hangi
//! koşuya bağlıysa o koşu açılır.

use rusqlite::functions::FunctionFlags;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension};
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
    /// PDF `/PageLabels` ağacından gelen basılı etiket (`iv`, `A-12`).
    /// Tanımsızsa `null`; fiziksel numaradan etiket türetilmez.
    pub page_label: Option<String>,
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
    /// Yapısal motorlarda paragraf/bölüm yolu (`Kira › Ödeme · p/4`).
    /// PDF/görüntüde yoktur; yoksa alan yazılmaz.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// `heading`, `paragraph`, `table`, `header`, `footer`, `annotation`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
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
    /// PDF `/PageLabels` ağacından gelen basılı sayfa etiketi. Tanımsızsa
    /// `None`: fiziksel numaradan etiket türetilmez, arayüz "fiziksel sayfa"
    /// der.
    pub page_label: Option<String>,
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
            // Etiket kırpılır; boş etiket `NULL` olur — fiziksel numaradan
            // etiket türetilmez (uydurma etiket yazılmaz).
            let page_label = page
                .page_label
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty());
            tx.execute(
                "INSERT INTO pages (id,parse_run_id,page_index,page_label,width,height,rotation,coordinate_space,text_origin,quality_flags)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                rusqlite::params![
                    page_id,
                    parse_run_id,
                    page.index,
                    page_label,
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
                    p.text_origin,p.quality_flags,p.width,p.height,p.rotation,p.coordinate_space,p.page_label,
                    (SELECT s.bbox_list FROM text_segments s WHERE s.page_id=p.id LIMIT 1) AS bbox_list
             FROM pages p WHERE p.parse_run_id=?1 ORDER BY p.page_index",
        )?;
        let rows = stmt.query_map([parse_run_id], |row| {
            let raw_blocks: Option<String> = row.get(11)?;
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
                page_label: row.get(10)?,
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

    /// Dosya kapsamlı tam metin araması (filtresiz, ilk sayfa). Geriye dönük
    /// uyumluluk için korunur; filtre/sayfalama `search_segments_page` ile.
    pub fn search_segments(
        &self,
        case_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<SearchHit>, StorageError> {
        Ok(self
            .search_segments_page(
                case_id,
                query,
                &SearchFilters::default(),
                limit,
                None,
                false,
            )?
            .hits)
    }

    /// Filtreli ve **keyset** sayfalı arama (F07-T03, F07-T05). Sorgu kipi
    /// `auto` kabul edilir; kip seçimi için `search_page`.
    pub fn search_segments_page(
        &self,
        case_id: &str,
        query: &str,
        filters: &SearchFilters,
        limit: i64,
        cursor: Option<&str>,
        include_total: bool,
    ) -> Result<SearchPage, StorageError> {
        self.search_page(
            case_id,
            &SearchQuery::auto(query),
            filters,
            limit,
            cursor,
            include_total,
        )
    }

    /// Kip destekli filtreli ve **keyset** sayfalı arama (F07-T02).
    ///
    /// Sıralama `bm25` (daha küçük = daha iyi) ve eşitlikte `segment_id`.
    /// İmleç son dönen sonucun (rank bitleri, segment kimliği) çiftidir; böylece
    /// araya kayıt girse bile sayfa kaymaz, sonuç tekrarlanmaz veya atlanmaz.
    /// `include_total` yalnız ilk sayfada istenmeli: ikinci sayfada sayım
    /// gereksiz maliyettir.
    ///
    /// Kip, adayları FTS'te daraltır; **kesin eşleşme** `dim_search_match`
    /// doğrulamasıyla kurulur (Türkçe `ı/i`, noktalama, baştaki sıfır). FTS'in
    /// diakritik sadeleştirmesi tek başına "tam arama" iddiasını taşıyamaz.
    pub fn search_page(
        &self,
        case_id: &str,
        query: &SearchQuery,
        filters: &SearchFilters,
        limit: i64,
        cursor: Option<&str>,
        include_total: bool,
    ) -> Result<SearchPage, StorageError> {
        let filters = filters.normalized()?;
        let limit = limit.clamp(1, MAX_SEARCH_LIMIT);
        let resolved = query.resolve()?;
        let cursor = match cursor {
            Some(token) if !token.trim().is_empty() => {
                Some(SearchCursor::parse(token.trim()).ok_or(StorageError::SearchCursor)?)
            }
            _ => None,
        };

        let mut page = SearchPage {
            case_id: case_id.to_string(),
            query: query.text.trim().to_string(),
            resolved_query: resolved.text.clone(),
            mode: resolved.mode,
            scope: "case".to_string(),
            limit,
            returned: 0,
            has_more: false,
            next_cursor: None,
            total: None,
            filters: filters.clone(),
            hits: Vec::new(),
        };
        let expression = fts_expression(&resolved);
        if resolved.text.is_empty() || expression.is_empty() {
            if include_total {
                page.total = Some(0);
            }
            return Ok(page);
        }

        let (base_where, base_params) = search_where(case_id, &resolved, &expression, &filters);
        if include_total {
            let count: i64 = self.conn().query_row(
                &format!("SELECT count(*) FROM {SEARCH_FROM} {base_where}"),
                rusqlite::params_from_iter(base_params.iter()),
                |row| row.get(0),
            )?;
            page.total = Some(count);
        }

        // Sonuç sınırı: bir fazla satır çekilir, `has_more` gerçek veriden gelir.
        let mut sql = format!(
            "SELECT s.id, s.page_id, s.raw_text, p.page_index, bm25(text_segments_fts),
                    pr.document_version_id, dv.document_id, d.display_name, d.document_type,
                    d.status, p.text_origin, s.case_id
             FROM {SEARCH_FROM} {base_where}"
        );
        let mut params = base_params;
        if let Some(cursor) = &cursor {
            sql.push_str(
                " AND (bm25(text_segments_fts) > ?
                       OR (bm25(text_segments_fts) = ? AND s.id > ?))",
            );
            let rank = Value::Real(cursor.rank());
            params.push(rank.clone());
            params.push(rank);
            params.push(Value::Text(cursor.segment_id.clone()));
        }
        sql.push_str(" ORDER BY bm25(text_segments_fts), s.id LIMIT ?");
        params.push(Value::Integer(limit + 1));

        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok(SearchHit {
                segment_id: row.get(0)?,
                page_id: row.get(1)?,
                raw_text: row.get(2)?,
                page_index: row.get(3)?,
                rank: row.get(4)?,
                document_version_id: row.get(5)?,
                document_id: row.get(6)?,
                document_name: row.get(7)?,
                document_type: row.get(8)?,
                document_status: row.get(9)?,
                text_origin: row.get(10)?,
                case_id: row.get(11)?,
                tags: Vec::new(),
            })
        })?;
        let mut hits = Vec::new();
        for row in rows {
            hits.push(row?);
        }
        if hits.len() as i64 > limit {
            hits.truncate(limit as usize);
            page.has_more = true;
        }
        if page.has_more {
            if let Some(last) = hits.last() {
                page.next_cursor = Some(
                    SearchCursor {
                        rank_bits: last.rank.to_bits(),
                        segment_id: last.segment_id.clone(),
                    }
                    .encode(),
                );
            }
        }

        let doc_ids: Vec<String> = {
            let mut ids: Vec<String> = hits.iter().filter_map(|h| h.document_id.clone()).collect();
            ids.sort();
            ids.dedup();
            ids
        };
        if !doc_ids.is_empty() {
            let mut by_doc = self.tags_for_documents(case_id, &doc_ids)?;
            for hit in &mut hits {
                if let Some(doc_id) = &hit.document_id {
                    if let Some(tags) = by_doc.remove(doc_id) {
                        hit.tags = tags;
                    }
                }
            }
        }

        page.returned = hits.len() as i64;
        page.hits = hits;
        Ok(page)
    }

    /// Sonuç sayfasındaki belgelerin etiketleri: tek sorgu, N+1 yok.
    fn tags_for_documents(
        &self,
        case_id: &str,
        document_ids: &[String],
    ) -> Result<std::collections::HashMap<String, Vec<String>>, StorageError> {
        let placeholders = vec!["?"; document_ids.len()].join(",");
        let sql = format!(
            "SELECT document_id, tag FROM document_tags
             WHERE case_id=? AND document_id IN ({placeholders}) ORDER BY tag"
        );
        let mut params: Vec<Value> = vec![Value::Text(case_id.to_string())];
        for id in document_ids {
            params.push(Value::Text(id.clone()));
        }
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for row in rows {
            let (doc_id, tag) = row?;
            out.entry(doc_id).or_default().push(tag);
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Sorgu kipi (F07-T02): Türkçe tam/esnek arama, tırnaklı ifade, E./K. numarası
// ---------------------------------------------------------------------------

/// Arama kipi. `Auto` kullanıcının yazımından kipi çıkarır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SearchMode {
    /// Tırnaklı metin → ifade, `2024/123 E.` → esas/karar no, değilse esnek.
    #[default]
    Auto,
    /// Türkçe karakterler korunur; metin bitişik geçmeli (noktalama araya girmez).
    /// Son kelime önek olabilir: "itiraz" → "itirazın".
    Exact,
    /// Türkçe karakterler sadeleşir (ı→i, ş→s…); kelimeler sırayla, noktalama
    /// önemsiz, son kelime önek olabilir.
    Flexible,
    /// Kelimeler aynı sırada; araya noktalama girebilir, son kelime tam olmalı.
    Phrase,
    /// `2024/123 E.` biçimli esas/karar numarası; baştaki sıfırlar yok sayılır.
    Docket,
}

impl SearchMode {
    /// UI'dan gelen metni kipe çevirir; bilinmeyen değer `None`.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Some(Self::Auto),
            "exact" => Some(Self::Exact),
            "flexible" => Some(Self::Flexible),
            "phrase" => Some(Self::Phrase),
            "docket" => Some(Self::Docket),
            _ => None,
        }
    }

    /// SQL doğrulamasında ve arayüzde kullanılan kararlı etiket.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Exact => "exact",
            Self::Flexible => "flexible",
            Self::Phrase => "phrase",
            Self::Docket => "docket",
        }
    }
}

/// Kullanıcının arama isteği: metin + kip. `mode` verilmezse `auto`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: String,
    #[serde(default)]
    pub mode: SearchMode,
}

impl SearchQuery {
    #[must_use]
    pub fn auto(text: &str) -> Self {
        Self {
            text: text.to_string(),
            mode: SearchMode::Auto,
        }
    }
}

/// Aşırı uzun sorgu FTS ifadesini şişirmesin; sınır arayüzde de görünür.
pub const MAX_SEARCH_QUERY_CHARS: usize = 300;

/// Çözümlenmiş sorgu: kip kesinleşti, tırnaklar atıldı, esas no ayrıştırıldı.
struct ResolvedQuery {
    mode: SearchMode,
    text: String,
    docket: Option<DocketNumber>,
}

/// `2024/123 E.` ayrıştırması. Numara baştaki sıfırlardan arındırılır.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DocketNumber {
    year: String,
    number: String,
    role: Option<char>,
}

impl SearchQuery {
    fn resolve(&self) -> Result<ResolvedQuery, StorageError> {
        let raw = self.text.trim();
        if raw.chars().count() > MAX_SEARCH_QUERY_CHARS {
            return Err(StorageError::SearchQuery(format!(
                "arama metni en çok {MAX_SEARCH_QUERY_CHARS} karakter olabilir"
            )));
        }
        let quoted = strip_wrapping_quotes(raw);
        let (mode, text) = match self.mode {
            SearchMode::Auto => match quoted {
                Some(inner) => (SearchMode::Phrase, inner),
                None => match parse_docket(raw) {
                    Some(_) => (SearchMode::Docket, raw.to_string()),
                    None => (SearchMode::Flexible, raw.to_string()),
                },
            },
            // Kip açıkça seçildiyse tırnak yalnız süslemedir; metne katılmaz.
            SearchMode::Phrase => (
                SearchMode::Phrase,
                quoted.unwrap_or_else(|| raw.to_string()),
            ),
            SearchMode::Docket => (SearchMode::Docket, raw.to_string()),
            other => (other, raw.to_string()),
        };
        let docket = if mode == SearchMode::Docket {
            Some(parse_docket(&text).ok_or_else(|| {
                StorageError::SearchQuery(
                    "esas/karar numarası çözümlenemedi; örnek: 2024/123 E.".into(),
                )
            })?)
        } else {
            None
        };
        Ok(ResolvedQuery { mode, text, docket })
    }
}

/// Tırnakla sarılmış metni içeriğe çevirir: `"kira bedeli"` → `kira bedeli`.
/// Eşleşmeyen tırnak yok sayılmaz; metin olduğu gibi kalır.
fn strip_wrapping_quotes(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let count = trimmed.chars().count();
    if count < 3 {
        return None;
    }
    let first = trimmed.chars().next()?;
    let last = trimmed.chars().last()?;
    let pairs = [('"', '"'), ('\u{201c}', '\u{201d}')];
    for (open, close) in pairs {
        if first == open && last == close {
            let inner: String = trimmed.chars().skip(1).take(count - 2).collect();
            let inner = inner.trim().to_string();
            if !inner.is_empty() {
                return Some(inner);
            }
        }
    }
    None
}

/// `2024/123 E.` metninden yıl, numara ve varsa E./K. rolünü çıkarır.
fn parse_docket(text: &str) -> Option<DocketNumber> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        if i - start != 4 {
            continue;
        }
        let year: String = chars[start..i].iter().collect();
        let mut j = i;
        while j < chars.len() && (chars[j].is_whitespace() || chars[j] == '.') {
            j += 1;
        }
        if j >= chars.len() || chars[j] != '/' {
            continue;
        }
        j += 1;
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
        let number_start = j;
        while j < chars.len() && chars[j].is_ascii_digit() {
            j += 1;
        }
        if j == number_start {
            continue;
        }
        let raw_number: String = chars[number_start..j].iter().collect();
        let trimmed = raw_number.trim_start_matches('0');
        let number = if trimmed.is_empty() { "0" } else { trimmed };
        let mut k = j;
        while k < chars.len()
            && (chars[k].is_whitespace() || matches!(chars[k], '.' | ',' | '-' | '/'))
        {
            k += 1;
        }
        let role = if k < chars.len() && chars[k].is_ascii_alphabetic() {
            let candidate = chars[k].to_ascii_uppercase();
            (candidate == 'E' || candidate == 'K').then_some(candidate)
        } else {
            None
        };
        return Some(DocketNumber {
            year,
            number: number.to_string(),
            role,
        });
    }
    None
}

/// FTS5 sözdizimine gömülecek metin: `"` boşluğa çevrilir, boşluklar sadeleşir.
/// Sorgu **parametre** olarak bağlanır; kullanıcı metni hiçbir zaman SQL'e
/// dizge birleştirmesiyle girmez.
fn escape_fts(text: &str) -> String {
    text.replace('"', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Kipin FTS aday ifadesi. Adaylar geniştir; kesin eşleşmeyi
/// `dim_search_match` doğrular (FTS diakritikleri kendiliğinden sadeleştirir).
fn fts_expression(query: &ResolvedQuery) -> String {
    match query.mode {
        // Yıl tek başına aday: `2024/0123` baştaki sıfır yüzünden `123`
        // belirteciyle eşleşmez; numara doğrulamada kanonikleştirilir.
        SearchMode::Docket => match query.docket.as_ref() {
            Some(docket) => format!("raw_text : \"{}\"", escape_fts(&docket.year)),
            None => String::new(),
        },
        SearchMode::Exact => {
            let needle = escape_fts(&query.text);
            if needle.is_empty() {
                String::new()
            } else {
                format!("raw_text : \"{needle}\"*")
            }
        }
        SearchMode::Phrase => exact_tokens(&query.text)
            .iter()
            .map(|token| format!("raw_text : \"{}\"", escape_fts(token)))
            .collect::<Vec<_>>()
            .join(" AND "),
        SearchMode::Flexible => {
            let tokens = fold_tokens(&query.text);
            let Some((last, head)) = tokens.split_last() else {
                return String::new();
            };
            let mut parts: Vec<String> = head
                .iter()
                .map(|token| format!("normalized_text : \"{}\"", escape_fts(token)))
                .collect();
            parts.push(format!("normalized_text : \"{}\"*", escape_fts(last)));
            parts.join(" AND ")
        }
        SearchMode::Auto => String::new(),
    }
}

/// Boşlukları tek boşluğa indirir (karşılaştırma için).
fn collapse_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Türkçe küçük harf: `İ`→`i`, `I`→`ı`. Tam arama `ı` ile `i`yi ayırır.
fn turkish_lower(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            'İ' => out.push('i'),
            'I' => out.push('ı'),
            _ => out.extend(ch.to_lowercase()),
        }
    }
    out
}

/// Kelime dizisi: Türkçe tam (`fold=false`) ya da sadeleşmiş (`fold=true`).
/// Noktalama ayırıcıdır; `normalized_text` sütunuyla aynı sadeleştirme
/// (`dim_evidence::normalize`) kullanılır ki aday üretimi ile doğrulama
/// birbirinden kopmasın.
fn tokens_with(text: &str, fold: bool) -> Vec<String> {
    let prepared = if fold {
        dim_evidence::normalize(text)
    } else {
        turkish_lower(text)
    };
    prepared
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn exact_tokens(text: &str) -> Vec<String> {
    tokens_with(text, false)
}

fn fold_tokens(text: &str) -> Vec<String> {
    tokens_with(text, true)
}

/// İğne kelimeleri samanlıkta **bitişik** geçiyor mu? `prefix_last` ise son
/// iğne kelimesi samanlık kelimesinin öneki olabilir (Türkçe ekler).
fn seq_matches(haystack: &[String], needle: &[String], prefix_last: bool) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|window| {
        window
            .iter()
            .zip(needle.iter())
            .enumerate()
            .all(|(index, (hay, want))| {
                if prefix_last && index == needle.len() - 1 {
                    hay.starts_with(want.as_str())
                } else {
                    hay == want
                }
            })
    })
}

/// Metinde `yıl/numara` geçiyor mu? Numara baştaki sıfırlardan arındırılmış
/// karşılaştırılır; rol (E./K.) verildiyse numaradan sonra o harf aranır.
fn docket_match(text: &str, docket: &DocketNumber) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        let year: String = chars[start..i].iter().collect();
        if year != docket.year {
            continue;
        }
        let mut j = i;
        while j < chars.len() && (chars[j].is_whitespace() || chars[j] == '.') {
            j += 1;
        }
        if j >= chars.len() || chars[j] != '/' {
            continue;
        }
        j += 1;
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
        let number_start = j;
        while j < chars.len() && chars[j].is_ascii_digit() {
            j += 1;
        }
        if j == number_start {
            continue;
        }
        let raw_number: String = chars[number_start..j].iter().collect();
        let trimmed = raw_number.trim_start_matches('0');
        let number = if trimmed.is_empty() { "0" } else { trimmed };
        if number != docket.number {
            continue;
        }
        let Some(expected) = docket.role else {
            return true;
        };
        let mut k = j;
        while k < chars.len()
            && (chars[k].is_whitespace() || matches!(chars[k], '.' | ',' | '-' | '/'))
        {
            k += 1;
        }
        if k < chars.len() && chars[k].to_ascii_uppercase() == expected {
            return true;
        }
    }
    false
}

/// SQL doğrulaması: `mode` etiketiyle gelen kipin kesin eşleşme kuralı.
fn search_match(text: &str, needle: &str, mode: &str) -> bool {
    match mode {
        "exact" => {
            let haystack = turkish_lower(&collapse_ws(text));
            let wanted = turkish_lower(&collapse_ws(needle));
            !wanted.is_empty() && haystack.contains(&wanted)
        }
        "flexible" => seq_matches(&fold_tokens(text), &fold_tokens(needle), true),
        "phrase" => seq_matches(&exact_tokens(text), &exact_tokens(needle), false),
        "docket" => parse_docket(needle).is_some_and(|docket| docket_match(text, &docket)),
        _ => false,
    }
}

/// `dim_search_match(text, needle, mode)` SQL fonksiyonunu bağlar.
///
/// Kural tek yerde (Rust) yaşar; sayım ve sayfa aynı doğrulamayı kullansın diye
/// SQLite'a skaler fonksiyon olarak verilir. Fonksiyon deterministiktir.
pub(crate) fn register_search_functions(conn: &Connection) -> Result<(), StorageError> {
    conn.create_scalar_function(
        "dim_search_match",
        3,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let text: Option<String> = ctx.get(0)?;
            let needle: Option<String> = ctx.get(1)?;
            let mode: Option<String> = ctx.get(2)?;
            Ok(match (text, needle, mode) {
                (Some(text), Some(needle), Some(mode)) => {
                    i64::from(search_match(&text, &needle, &mode))
                }
                _ => 0,
            })
        },
    )?;
    Ok(())
}

/// Arama sorgusunun ortak FROM'u. Segment → koşu → sürüm → belge yolu tek
/// yerde; sayaç ve sayfa sorgusu aynı kapsamı kullanır.
const SEARCH_FROM: &str = "text_segments_fts
     JOIN text_segments s ON s.rowid = text_segments_fts.rowid
     LEFT JOIN pages p ON p.id = s.page_id
     LEFT JOIN parse_runs pr ON pr.id = s.parse_run_id
     LEFT JOIN document_versions dv ON dv.id = pr.document_version_id
     LEFT JOIN documents d ON d.id = dv.document_id";

/// WHERE'i ve parametrelerini (önce MATCH, sonra `case_id`, kip doğrulaması,
/// ardından filtreler) üretir.
///
/// Etiket filtresi **VE** semantiği taşır: verilen bütün etiketler aynı belgede
/// olmalı. Tarih rolü, belgeye *kaynaklı* olayın rolüdür — belgeye bağlanmamış
/// olay bu belgeyi sonuca sokmaz.
fn search_where(
    case_id: &str,
    query: &ResolvedQuery,
    expression: &str,
    filters: &SearchFilters,
) -> (String, Vec<Value>) {
    let mut sql = String::from(
        " WHERE text_segments_fts MATCH ? AND s.case_id = ?
          AND dim_search_match(s.raw_text, ?, ?) = 1",
    );
    let mut params: Vec<Value> = vec![
        Value::Text(expression.to_string()),
        Value::Text(case_id.to_string()),
        Value::Text(query.text.clone()),
        Value::Text(query.mode.tag().to_string()),
    ];

    if !filters.document_types.is_empty() {
        let placeholders = vec!["?"; filters.document_types.len()].join(",");
        sql.push_str(&format!(" AND d.document_type IN ({placeholders})"));
        for kind in &filters.document_types {
            params.push(Value::Text(kind.clone()));
        }
    }

    match filters.ocr_status.as_deref() {
        Some("ocr") => sql.push_str(" AND p.text_origin IN ('ocr','mixed')"),
        Some("native") => sql.push_str(" AND p.text_origin = 'native'"),
        _ => {}
    }

    if !filters.tags.is_empty() {
        let placeholders = vec!["?"; filters.tags.len()].join(",");
        sql.push_str(&format!(
            " AND (SELECT count(*) FROM document_tags t
                    WHERE t.document_id = d.id AND t.tag IN ({placeholders})) = {}",
            filters.tags.len()
        ));
        for tag in &filters.tags {
            params.push(Value::Text(tag.clone()));
        }
    }

    if let Some(role) = &filters.date_role {
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM events e
                 JOIN event_sources es ON es.event_id = e.id
                 JOIN source_anchors sa ON sa.id = es.source_anchor_id
                 JOIN document_versions sdv ON sdv.id = sa.document_version_id
                 WHERE e.case_id = ? AND e.deleted_at IS NULL AND e.date_role = ?
                   AND sdv.document_id = d.id)",
        );
        params.push(Value::Text(case_id.to_string()));
        params.push(Value::Text(role.clone()));
    }

    (sql, params)
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
    /// Sonucun kapsamını arayüzde görünür kılan üst veri (F07-T05): belge türü,
    /// durumu ve sayfanın metin kaynağı (OCR mı).
    pub document_type: Option<String>,
    pub document_status: Option<String>,
    pub text_origin: Option<String>,
    /// Sonucun hangi dosyaya ait olduğu; kapsam dışı dosya sonuca giremez.
    pub case_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub raw_text: String,
    pub page_index: Option<i64>,
    pub rank: f64,
}

/// Bir aramanın filtre kümesi (F07-T03). Boş alan = filtre yok.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchFilters {
    /// `documents.document_type` değerleri; birden fazlaysa **VEYA**.
    pub document_types: Vec<String>,
    /// Belgeye kaynaklı olayın `date_role` değeri (tebliğ, sözleşme, duruşma…).
    pub date_role: Option<String>,
    /// Belgedeki etiketler; birden fazlaysa **VE** (hepsi aynı belgede).
    pub tags: Vec<String>,
    /// `any` | `ocr` | `native`.
    pub ocr_status: Option<String>,
}

impl SearchFilters {
    /// Filtreleri doğrular ve kanonikleştirir. Bilinmeyen değer sessizce
    /// yok sayılmaz: `E_SEARCH_FILTER` döner, yoksa kullanıcı filtresiz sonuç
    /// gördüğünü sanır.
    fn normalized(&self) -> Result<SearchFilters, StorageError> {
        let mut out = SearchFilters::default();
        for raw in &self.document_types {
            let kind = raw.trim();
            if kind.is_empty() {
                continue;
            }
            if kind.chars().count() > 40 {
                return Err(StorageError::SearchFilter("belge türü çok uzun".into()));
            }
            if !out.document_types.iter().any(|existing| existing == kind) {
                out.document_types.push(kind.to_string());
            }
        }
        if out.document_types.len() > 20 {
            return Err(StorageError::SearchFilter(
                "en çok 20 belge türü seçilebilir".into(),
            ));
        }

        let role = self
            .date_role
            .as_deref()
            .map(str::trim)
            .filter(|role| !role.is_empty());
        if let Some(role) = role {
            if !SEARCH_DATE_ROLES.contains(&role) {
                return Err(StorageError::SearchFilter(format!(
                    "bilinmeyen tarih rolü: {role}"
                )));
            }
            out.date_role = Some(role.to_string());
        }

        for raw in &self.tags {
            let tag = crate::tags::normalize_tag(raw).ok_or_else(|| {
                StorageError::SearchFilter("geçersiz etiket (boş veya 40 karakterden uzun)".into())
            })?;
            if !out.tags.contains(&tag) {
                out.tags.push(tag);
            }
        }
        if out.tags.len() > crate::tags::MAX_FILTER_TAGS {
            return Err(StorageError::SearchFilter(format!(
                "en çok {} etiketle filtre kurulabilir",
                crate::tags::MAX_FILTER_TAGS
            )));
        }
        // Parametre sırası deterministik olsun: aynı filtre aynı SQL'i üretir.
        out.tags.sort();

        match self.ocr_status.as_deref().map(str::trim) {
            None | Some("") | Some("any") => {}
            Some(value @ ("ocr" | "native")) => out.ocr_status = Some(value.to_string()),
            Some(other) => {
                return Err(StorageError::SearchFilter(format!(
                    "bilinmeyen OCR durumu: {other}"
                )))
            }
        }
        Ok(out)
    }
}

/// `events.date_role` ile aynı küme; bilinmeyen rol filtresi reddedilir.
pub const SEARCH_DATE_ROLES: &[&str] = &[
    "event",
    "document",
    "notification",
    "contract",
    "hearing",
    "deadline_candidate",
    "unknown",
];

/// Tek aramada dönebilecek en fazla sonuç (arayüz de aynı sınırı gösterir).
pub const MAX_SEARCH_LIMIT: i64 = 200;

/// Keyset imleci: son dönen sonucun `bm25` bitleri + segment kimliği.
///
/// Rank `f64` olarak karşılaştırılır; ondalık metne çevirmek eşit skorlarda
/// kayma üretirdi, bu yüzden bit temsili taşınır.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchCursor {
    pub rank_bits: u64,
    pub segment_id: String,
}

impl SearchCursor {
    pub fn encode(&self) -> String {
        format!("v1.{:016x}.{}", self.rank_bits, self.segment_id)
    }

    pub fn parse(token: &str) -> Option<SearchCursor> {
        let mut parts = token.splitn(3, '.');
        if parts.next()? != "v1" {
            return None;
        }
        let rank_bits = u64::from_str_radix(parts.next()?, 16).ok()?;
        let segment_id = parts.next()?;
        if segment_id.is_empty() || segment_id.len() > 64 {
            return None;
        }
        Some(SearchCursor {
            rank_bits,
            segment_id: segment_id.to_string(),
        })
    }

    fn rank(&self) -> f64 {
        f64::from_bits(self.rank_bits)
    }
}

/// Bir arama sayfası ve kapsam bilgisi (F07-T05).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub case_id: String,
    /// Kullanıcının yazdığı metin (kırpılmış hâli).
    pub query: String,
    /// Çözümlenmiş metin: tırnaklar atılmış, kip `auto` ise kipe göre
    /// sadeleştirilmiş hâli. Arayüz alıntı işaretlemesini bununla yapar.
    pub resolved_query: String,
    /// `auto` çözümlendikten sonraki kesin kip.
    pub mode: SearchMode,
    /// `case` = yalnız bu dosya.
    pub scope: String,
    pub limit: i64,
    pub returned: i64,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    /// Yalnız `include_total` istendiğinde dolar.
    pub total: Option<i64>,
    pub filters: SearchFilters,
    pub hits: Vec<SearchHit>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::{DatabaseKey, NewAnchor, NewEvent};

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
    fn parse_run_settings_hash_is_canonical_and_not_double_hashed() {
        let (db, case_id, version_id) = setup();
        let settings = "application/pdf|ocr=false|lang=eng|tessdata=unknown";
        let run = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", settings)
            .expect("koşu");
        let stored: String = db
            .conn()
            .query_row(
                "SELECT settings_hash FROM parse_runs WHERE id = ?1",
                [&run.id],
                |row| row.get(0),
            )
            .expect("ayar özeti");
        let expected = settings_hash("pymupdf", "1.24", settings);
        assert_eq!(
            stored, expected,
            "ayar özeti engine|sürüm|ayarlar üzerinden hesaplanmalı"
        );
        assert_ne!(
            stored,
            settings_hash("pymupdf", "1.24", &expected),
            "ayar özeti ikinci kez hash'lenmemeli"
        );
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
    fn page_labels_are_stored_verbatim_and_never_invented() {
        let (db, case_id, version_id) = setup();
        let run = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        db.store_pages(
            &run.id,
            &case_id,
            &[
                NewPage {
                    index: 0,
                    text: "Kapak sayfası".into(),
                    origin: "native".into(),
                    page_label: Some(" iv ".into()),
                    ..Default::default()
                },
                NewPage {
                    index: 1,
                    text: "Boş etiketli sayfa".into(),
                    origin: "native".into(),
                    page_label: Some("   ".into()),
                    ..Default::default()
                },
                NewPage {
                    index: 2,
                    text: "Etiketsiz sayfa".into(),
                    origin: "native".into(),
                    ..Default::default()
                },
            ],
        )
        .expect("sayfalar");

        let pages = db.pages_for_run(&run.id).expect("sayfalar");
        assert_eq!(pages[0].page_label.as_deref(), Some("iv"));
        assert!(
            pages[1].page_label.is_none(),
            "boşluktan oluşan etiket NULL olmalı"
        );
        assert!(
            pages[2].page_label.is_none(),
            "etiket yoksa fiziksel numaradan üretilmemeli"
        );
    }

    #[test]
    fn structure_block_paths_survive_the_round_trip() {
        let (db, case_id, version_id) = setup();
        let run = db
            .start_parse_run(&version_id, &case_id, "python-docx", "1.1", "{}")
            .expect("koşu");
        db.store_pages(
            &run.id,
            &case_id,
            &[NewPage {
                index: 0,
                text: "Kira Sözleşmesi\n\nÖdeme · p/2".into(),
                origin: "native".into(),
                coordinate_space: Some("structure".into()),
                blocks: vec![
                    SegmentBlock {
                        start: 0,
                        end: 16,
                        path: Some("Kira Sözleşmesi".into()),
                        kind: Some("heading".into()),
                        ..Default::default()
                    },
                    SegmentBlock {
                        start: 18,
                        end: 30,
                        path: Some("Kira Sözleşmesi · p/2".into()),
                        kind: Some("paragraph".into()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
        )
        .expect("sayfalar");

        let pages = db.pages_for_run(&run.id).expect("sayfalar");
        let blocks = &pages[0].blocks;
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].path.as_deref(), Some("Kira Sözleşmesi"));
        assert_eq!(blocks[0].kind.as_deref(), Some("heading"));
        assert_eq!(blocks[1].path.as_deref(), Some("Kira Sözleşmesi · p/2"));
        assert_eq!(blocks[1].kind.as_deref(), Some("paragraph"));
        // Yapısal bloklarda kutu yoktur; olmayan kutu uydurulmaz.
        assert!(blocks[1].bbox.is_none());
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

    /// Yeni belge + sürüm + koşu + tek sayfa. Testler arama filtrelerini bu
    /// gerçek kayıtlar üzerinde kurar; segment kimliği de döner (anchor için).
    fn add_document_with_text(
        db: &Database,
        case_id: &str,
        name: &str,
        document_type: &str,
        text: &str,
        origin: &str,
    ) -> (String, String, String, String) {
        let tag = uuid::Uuid::now_v7().to_string();
        let blob = db
            .create_blob_record(&format!("st-{tag}"), 10, "aad")
            .expect("blob");
        let doc = db
            .create_document(case_id, name, document_type, None, "unknown")
            .expect("belge");
        let version = db
            .add_version(
                &doc.id,
                case_id,
                &blob,
                &"a".repeat(64),
                10,
                "application/pdf",
            )
            .expect("sürüm");
        let run = db
            .start_parse_run(&version.id, case_id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        db.store_pages(
            &run.id,
            case_id,
            &[NewPage {
                index: 0,
                text: text.into(),
                width: None,
                height: None,
                origin: origin.into(),
                warnings: vec![],
                ..Default::default()
            }],
        )
        .expect("sayfa");
        let segment_id = db
            .pages_for_run(&run.id)
            .expect("sayfalar")
            .first()
            .and_then(|page| page.segment_id.clone())
            .expect("segment");
        (doc.id, version.id, run.id, segment_id)
    }

    #[test]
    fn search_filters_by_document_type_and_text_origin() {
        let (db, case_id, _version_id) = setup();
        let (contract_doc, ..) = add_document_with_text(
            &db,
            &case_id,
            "Kira Sözleşmesi",
            "contract",
            "kira ödemesi aylık yapılır",
            "native",
        );
        let (report_doc, ..) = add_document_with_text(
            &db,
            &case_id,
            "Bilirkişi Raporu",
            "report",
            "kira ödemesi yapılmamıştır",
            "ocr",
        );

        let by_type = db
            .search_segments_page(
                &case_id,
                "kira",
                &SearchFilters {
                    document_types: vec!["contract".into()],
                    ..Default::default()
                },
                10,
                None,
                true,
            )
            .expect("tür filtresi");
        assert_eq!(by_type.total, Some(1));
        assert_eq!(by_type.hits.len(), 1);
        assert_eq!(
            by_type.hits[0].document_id.as_deref(),
            Some(contract_doc.as_str())
        );
        assert_eq!(by_type.hits[0].document_type.as_deref(), Some("contract"));
        assert_eq!(by_type.hits[0].text_origin.as_deref(), Some("native"));

        let ocr_only = db
            .search_segments_page(
                &case_id,
                "kira",
                &SearchFilters {
                    ocr_status: Some("ocr".into()),
                    ..Default::default()
                },
                10,
                None,
                true,
            )
            .expect("OCR filtresi");
        assert_eq!(ocr_only.hits.len(), 1);
        assert_eq!(
            ocr_only.hits[0].document_id.as_deref(),
            Some(report_doc.as_str())
        );
        assert_eq!(ocr_only.hits[0].text_origin.as_deref(), Some("ocr"));

        let native_only = db
            .search_segments_page(
                &case_id,
                "kira",
                &SearchFilters {
                    ocr_status: Some("native".into()),
                    ..Default::default()
                },
                10,
                None,
                false,
            )
            .expect("yerel metin filtresi");
        assert_eq!(native_only.hits.len(), 1);
        assert_eq!(
            native_only.hits[0].document_id.as_deref(),
            Some(contract_doc.as_str())
        );

        let unknown = db.search_segments_page(
            &case_id,
            "kira",
            &SearchFilters {
                ocr_status: Some("taranmış".into()),
                ..Default::default()
            },
            10,
            None,
            false,
        );
        assert!(matches!(unknown, Err(StorageError::SearchFilter(_))));
    }

    #[test]
    fn search_tag_filter_requires_every_tag() {
        let (db, case_id, _version_id) = setup();
        let (both, ..) = add_document_with_text(
            &db,
            &case_id,
            "İhtarname",
            "notice",
            "kira bedeli ödenmedi",
            "native",
        );
        let (single, ..) = add_document_with_text(
            &db,
            &case_id,
            "Makbuz",
            "receipt",
            "kira bedeli ödendi",
            "native",
        );
        db.add_document_tag(&both, "Kira").expect("etiket");
        db.add_document_tag(&both, "temerrüt").expect("etiket");
        db.add_document_tag(&single, "kira").expect("etiket");

        let one = db
            .search_segments_page(
                &case_id,
                "kira",
                &SearchFilters {
                    tags: vec!["KİRA".into()],
                    ..Default::default()
                },
                10,
                None,
                true,
            )
            .expect("tek etiket");
        assert_eq!(one.total, Some(2));

        let two = db
            .search_segments_page(
                &case_id,
                "kira",
                &SearchFilters {
                    tags: vec!["kira".into(), "Temerrüt".into()],
                    ..Default::default()
                },
                10,
                None,
                true,
            )
            .expect("iki etiket");
        assert_eq!(two.total, Some(1));
        assert_eq!(two.hits[0].document_id.as_deref(), Some(both.as_str()));
        assert_eq!(
            two.hits[0].tags,
            vec!["kira".to_string(), "temerrüt".to_string()]
        );

        let unmatched = db
            .search_segments_page(
                &case_id,
                "kira",
                &SearchFilters {
                    tags: vec!["kira".into(), "yok".into()],
                    ..Default::default()
                },
                10,
                None,
                true,
            )
            .expect("eşleşmeyen");
        assert_eq!(unmatched.total, Some(0));
    }

    #[test]
    fn search_date_role_filter_uses_documents_linked_events() {
        let (db, case_id, _version_id) = setup();
        let (notified, version_id, run_id, segment_id) = add_document_with_text(
            &db,
            &case_id,
            "Tebligat",
            "notice",
            "kira artışı tebliğ edildi",
            "native",
        );
        let (other, ..) = add_document_with_text(
            &db,
            &case_id,
            "Dilekçe",
            "petition",
            "kira artışı itirazı",
            "native",
        );
        let anchor = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: version_id,
                parse_run_id: run_id,
                segment_id,
                start_offset: 0,
                end_offset: "kira artışı".chars().count() as i64,
                exact_quote: "kira artışı".into(),
            })
            .expect("anchor");
        let event = db
            .create_event(
                &case_id,
                &NewEvent {
                    event_kind: "notice".into(),
                    date_value: Some("2026-03-01".into()),
                    date_precision: "day".into(),
                    date_role: "notification".into(),
                    date_raw: Some("01.03.2026".into()),
                    title: "Tebligat".into(),
                    description: String::new(),
                    assertion_kind: "document".into(),
                },
            )
            .expect("olay");
        db.link_event_source(&event.id, &anchor.id, "states")
            .expect("bağ");

        let filtered = db
            .search_segments_page(
                &case_id,
                "kira",
                &SearchFilters {
                    date_role: Some("notification".into()),
                    ..Default::default()
                },
                10,
                None,
                true,
            )
            .expect("rol filtresi");
        assert_eq!(filtered.total, Some(1));
        assert_eq!(
            filtered.hits[0].document_id.as_deref(),
            Some(notified.as_str())
        );

        // Aynı rol başka bir olayda olsa da belgeye bağlı değilse süzülür.
        let hearing = db
            .search_segments_page(
                &case_id,
                "kira",
                &SearchFilters {
                    date_role: Some("hearing".into()),
                    ..Default::default()
                },
                10,
                None,
                true,
            )
            .expect("duruşma rolü");
        assert_eq!(hearing.total, Some(0));

        // Diğer belge kaynak bağı taşımadığı için sonuçta yok.
        assert!(!filtered
            .hits
            .iter()
            .any(|hit| hit.document_id.as_deref() == Some(other.as_str())));
    }

    #[test]
    fn search_keyset_pages_cover_every_hit_once() {
        let (db, case_id, _version_id) = setup();
        for index in 0..5 {
            add_document_with_text(
                &db,
                &case_id,
                &format!("Belge {index}"),
                "notice",
                "kira bedeli ödenmedi",
                "native",
            );
        }
        let mut seen: Vec<String> = Vec::new();
        let mut cursor: Option<String> = None;
        let mut pages = 0;
        loop {
            let page = db
                .search_segments_page(
                    &case_id,
                    "kira",
                    &SearchFilters::default(),
                    2,
                    cursor.as_deref(),
                    pages == 0,
                )
                .expect("sayfa");
            if pages == 0 {
                assert_eq!(page.total, Some(5), "ilk sayfa toplamı tam olmalı");
            } else {
                assert_eq!(page.total, None, "sonraki sayfada sayım istenmez");
            }
            assert!(page.returned <= 2);
            seen.extend(page.hits.iter().map(|hit| hit.segment_id.clone()));
            pages += 1;
            match page.next_cursor {
                Some(next) if page.has_more => cursor = Some(next),
                _ => break,
            }
            assert!(pages < 10, "sayfalama döngüye girdi");
        }
        seen.sort();
        seen.dedup();
        // Beş belge, her biri tek segment: her sonuç tam bir kez dönmeli.
        assert_eq!(seen.len(), 5, "her sonuç tam bir kez dönmeli");
        assert_eq!(pages, 3, "2'şerlik sayfalarda 5 sonuç 3 sayfa");
    }

    #[test]
    fn search_rejects_foreign_cursor_and_clamps_limit() {
        let (db, case_id, _version_id) = setup();
        let invalid = db.search_segments_page(
            &case_id,
            "kira",
            &SearchFilters::default(),
            10,
            Some("v1.zzzz."),
            false,
        );
        assert!(matches!(invalid, Err(StorageError::SearchCursor)));

        let page = db
            .search_segments_page(
                &case_id,
                "kira",
                &SearchFilters::default(),
                10_000,
                None,
                true,
            )
            .expect("sayfa");
        assert_eq!(page.limit, MAX_SEARCH_LIMIT);
        assert_eq!(page.scope, "case");
        assert_eq!(page.case_id, case_id);
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
                page_label: None,
                blocks: vec![
                    SegmentBlock {
                        start: 0,
                        end: 13,
                        bbox: Some(first_bbox),
                        lines: vec![[10.0, 10.0, 200.0, 24.0]],
                        path: None,
                        kind: None,
                    },
                    SegmentBlock {
                        start: 13,
                        end: 25,
                        bbox: Some(second_bbox),
                        lines: vec![],
                        path: None,
                        kind: None,
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

    /// Sayfa metinlerini tek koşuda yazar; arama testleri aynı dosyada
    /// karşılaştırma yapabilsin diye segment kimlikleri dönmez.
    fn store_texts(db: &Database, case_id: &str, version_id: &str, texts: &[&str]) {
        let run = db
            .start_parse_run(version_id, case_id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        let pages: Vec<NewPage> = texts
            .iter()
            .enumerate()
            .map(|(index, text)| NewPage {
                index: i64::try_from(index).expect("sıra"),
                text: (*text).to_string(),
                width: None,
                height: None,
                origin: "native".into(),
                warnings: vec![],
                ..Default::default()
            })
            .collect();
        db.store_pages(&run.id, case_id, &pages).expect("sayfalar");
        db.finish_parse_run(&run.id, "succeeded", &[])
            .expect("bitir");
    }

    fn search_with(db: &Database, case_id: &str, text: &str, mode: SearchMode) -> SearchPage {
        db.search_page(
            case_id,
            &SearchQuery {
                text: text.into(),
                mode,
            },
            &SearchFilters::default(),
            20,
            None,
            true,
        )
        .expect("arama")
    }

    #[test]
    fn search_exact_preserves_turkish_letters_flexible_folds_them() {
        let (db, case_id, version_id) = setup();
        store_texts(
            &db,
            &case_id,
            &version_id,
            &["ıslah beyanı verildi", "islah edilen tutar"],
        );

        // Tam arama `ı` ile `i`yi ayırır: yalnız gerçekten "ıslah" yazan sayfa.
        let exact = search_with(&db, &case_id, "ıslah", SearchMode::Exact);
        assert_eq!(exact.mode, SearchMode::Exact);
        assert_eq!(exact.total, Some(1));
        assert_eq!(exact.hits[0].page_index, Some(0));

        // Esnek arama Türkçe karakterleri sadeleştirir: iki sayfa da gelir.
        let flexible = search_with(&db, &case_id, "islah", SearchMode::Flexible);
        assert_eq!(flexible.mode, SearchMode::Flexible);
        assert_eq!(flexible.total, Some(2), "esnek arama ı/i ayrımını kaldırır");
        assert_eq!(flexible.resolved_query, "islah");
    }

    #[test]
    fn search_phrase_allows_punctuation_between_words_exact_does_not() {
        let (db, case_id, version_id) = setup();
        store_texts(
            &db,
            &case_id,
            &version_id,
            &["kira, sözleşmesi imzalandı", "kira sözleşmesi feshedildi"],
        );

        let phrase = search_with(&db, &case_id, "kira sözleşmesi", SearchMode::Phrase);
        assert_eq!(phrase.total, Some(2), "ifade araması noktalamayı yutar");
        let exact = search_with(&db, &case_id, "kira sözleşmesi", SearchMode::Exact);
        assert_eq!(exact.total, Some(1), "tam arama bitişik metin ister");
        assert_eq!(exact.hits[0].page_index, Some(1));
    }

    #[test]
    fn search_exact_matches_turkish_suffix_prefix() {
        let (db, case_id, version_id) = setup();
        store_texts(
            &db,
            &case_id,
            &version_id,
            &["itirazın reddine karar verildi"],
        );
        let exact = search_with(&db, &case_id, "itiraz", SearchMode::Exact);
        assert_eq!(exact.total, Some(1));
        // "itiraz" öneki "itirazın"ı bulur; ayrı kelime "itirazname" değil.
        let wrong = search_with(&db, &case_id, "itirazname", SearchMode::Exact);
        assert_eq!(wrong.total, Some(0));
    }

    #[test]
    fn search_auto_treats_quoted_text_as_phrase() {
        let (db, case_id, version_id) = setup();
        store_texts(
            &db,
            &case_id,
            &version_id,
            &["kira bedeli ödendi", "kira ve bedeli ayrı"],
        );

        let auto = db
            .search_segments_page(
                &case_id,
                "\"kira bedeli\"",
                &SearchFilters::default(),
                20,
                None,
                true,
            )
            .expect("arama");
        assert_eq!(auto.mode, SearchMode::Phrase, "tırnak ifade kipini seçer");
        assert_eq!(auto.resolved_query, "kira bedeli");
        assert_eq!(auto.total, Some(1));
        assert_eq!(auto.hits[0].page_index, Some(0));

        let plain = db
            .search_segments_page(
                &case_id,
                "kira bedeli",
                &SearchFilters::default(),
                20,
                None,
                true,
            )
            .expect("arama");
        assert_eq!(plain.mode, SearchMode::Flexible);
        assert_eq!(plain.total, Some(1), "tırnaksız da bitişik arar");
    }

    #[test]
    fn search_docket_matches_leading_zeros_and_role() {
        let (db, case_id, version_id) = setup();
        store_texts(
            &db,
            &case_id,
            &version_id,
            &[
                "2024/0123 E. sayılı dosya",
                "2024/123 K. kararı",
                "2024/1234 E. başka dosya",
            ],
        );

        let esas = db
            .search_segments_page(
                &case_id,
                "2024/123 E.",
                &SearchFilters::default(),
                20,
                None,
                true,
            )
            .expect("arama");
        assert_eq!(esas.mode, SearchMode::Docket, "E./K. yazımı tanınır");
        assert_eq!(esas.total, Some(1), "baştaki sıfır ve rol eşleşmeli");
        assert_eq!(esas.hits[0].page_index, Some(0));

        let any_role = db
            .search_segments_page(
                &case_id,
                "2024/123",
                &SearchFilters::default(),
                20,
                None,
                true,
            )
            .expect("arama");
        assert_eq!(any_role.mode, SearchMode::Docket);
        assert_eq!(any_role.total, Some(2), "rol verilmezse E. ve K. birlikte");
    }

    #[test]
    fn search_docket_mode_rejects_text_without_number() {
        let (db, case_id, _version_id) = setup();
        let err = db
            .search_page(
                &case_id,
                &SearchQuery {
                    text: "esas numarası".into(),
                    mode: SearchMode::Docket,
                },
                &SearchFilters::default(),
                10,
                None,
                false,
            )
            .expect_err("numarasız metin reddedilmeli");
        assert!(matches!(err, StorageError::SearchQuery(_)));
        assert_eq!(err.code(), "E_SEARCH_QUERY");
    }
}
