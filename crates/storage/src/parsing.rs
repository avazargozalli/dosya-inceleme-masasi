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
        let mut stmt = self.conn().prepare(PAGES_FOR_RUN_SQL)?;
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
        self.search_page_windowed(
            case_id,
            query,
            filters,
            limit,
            cursor,
            include_total,
            SEARCH_CANDIDATE_WINDOW,
        )
    }

    /// `search_page`in **pencereli** çekirdeği (F15 kuyruk düzeltmesi).
    ///
    /// Önceki hâlde doğrulama SQL'in `WHERE`'indeydi: SQLite `ORDER BY bm25`
    /// sıralamasından **önce** her FTS adayına `dim_search_verify` uyguluyor,
    /// yani sayfa için 20 sonuç yetse de 50.000 adayın tamamı doğrulanıyordu.
    /// Ölçülen kuyruk (`"numaralı reddine"` 36–54 sn) buradan geliyordu.
    ///
    /// Yeni yol: adaylar `bm25, segment_id` sırasında **akıtılır**, doğrulama
    /// Rust'ta pencere pencere yapılır ve sayfa dolduğu an tarama kesilir.
    /// Sıra ve sonuç kümesi değişmez (doğrulama aynı `SearchMatcher`), yalnız
    /// gereksiz adaylar hiç okunmaz. Ayrıntı: `verification` alanı kaç adayın
    /// gerçekten doğrulandığını dürüstçe taşır.
    ///
    /// `window` testlerde küçük verilir (pencere sınırları zorlanır); üretimde
    /// `SEARCH_CANDIDATE_WINDOW`.
    // Gerekçe: imza `search_page` ile aynı düzeni korur + tek ölçüm parametresi
    // (`window`) ekler; argümanları struct'a sarmak çağrı yerlerini okunaksız
    // kılardı. Testler bu yolu doğrudan çağırır.
    #[allow(clippy::too_many_arguments)]
    fn search_page_windowed(
        &self,
        case_id: &str,
        query: &SearchQuery,
        filters: &SearchFilters,
        limit: i64,
        cursor: Option<&str>,
        include_total: bool,
        window: i64,
    ) -> Result<SearchPage, StorageError> {
        let filters = filters.normalized()?;
        let limit = limit.clamp(1, MAX_SEARCH_LIMIT);
        let window = window.clamp(1, SEARCH_CANDIDATE_WINDOW);
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
            verification: SearchVerification::default(),
            hits: Vec::new(),
        };
        let expression = fts_expression(&resolved);
        if resolved.text.is_empty() || expression.is_empty() {
            if include_total {
                page.total = Some(0);
            }
            return Ok(page);
        }

        // Doğrulayıcı sorgu başına bir kez kurulur (F15 ölçek düzeltmesi).
        let matcher = std::sync::Arc::new(SearchMatcher::new(
            &resolved.text,
            resolved.mode,
            resolved.docket.clone(),
        ));
        let wants_normalized = matcher.wants_normalized();

        // Aday WHERE'i **doğrulama içermez**: doğrulama Rust'ta, sıralamadan
        // sonra ve yalnız sayfanın ihtiyaç duyduğu kadar aday üzerinde koşar.
        let (base_where, base_params) = search_where_candidates(case_id, &expression, &filters);

        // **İki aşamalı tarama (F15).** Ölçüm (3.030 adaylı seyrek eşleşme
        // profili, `docs/releases/F15/bench-scan-2026-10-26.json`): aday başına
        // maliyetin neredeyse tamamı satır hattıydı — dört LEFT JOIN, belge/sayfa
        // yükü ve metin çıkarımı; doğrulamanın kendisi değil. Bu yüzden aday
        // akışı yalnız `text_segments` + FTS üzerinden yürür; **belge üst
        // verisi yalnız kabul edilen sonuçlar için** (tek `IN` sorgusu) okunur.
        //
        // Filtre varsa JOIN'ler WHERE'de gereklidir; o durumda eski FROM
        // kullanılır (davranış aynı, yalnız seçilen sütunlar az).
        let filters_need_join = !filters.document_types.is_empty()
            || filters.ocr_status.is_some()
            || !filters.tags.is_empty()
            || filters.date_role.is_some();
        let scan_from = if filters_need_join {
            SEARCH_FROM
        } else {
            "text_segments_fts JOIN text_segments s ON s.rowid = text_segments_fts.rowid"
        };

        // `LIMIT` yok: adaylar `bm25` sırasında akıtılır; kaç adayın
        // doğrulanacağına sayfanın dolması karar verir (erken kesme).
        let mut sql = format!(
            "SELECT s.id AS segment_id, s.page_id, s.raw_text, s.normalized_text,
                    bm25(text_segments_fts) AS bm25_rank
             FROM {scan_from} {base_where}"
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
        // Pencere sayımı (`count(*) OVER ()`) ölçümde **daha yavaş** çıktı:
        // `bm25` alt sorgu düzleştirmesinde bağlamını kaybettiği için alt sorgu
        // materyalize ediliyor ve sıralama ikinci kez yapılıyor. Bu yüzden sayım
        // ayrı geçişte kalır (doğrulayıcı ucuz).
        sql.push_str(" ORDER BY bm25_rank, segment_id");

        let started = std::time::Instant::now();
        let mut scratch = MatchScratch::default();
        // Doğrulamayı geçen adaylar: kimlik + sıra + metin. Belge üst verisi
        // ikinci aşamada, tek sorguda gelir.
        let mut accepted: Vec<(String, Option<String>, f64, String)> = Vec::new();
        let mut examined: i64 = 0;
        let mut windows: i64 = 0;
        let mut stopped_early = false;
        {
            let mut stmt = self.conn().prepare(&sql)?;
            let mut rows = stmt.query(rusqlite::params_from_iter(params.iter()))?;
            'scan: loop {
                let mut in_window: i64 = 0;
                let mut opened = false;
                let mut exhausted = false;
                while in_window < window {
                    let Some(row) = rows.next()? else {
                        exhausted = true;
                        break;
                    };
                    if !opened {
                        windows += 1;
                        opened = true;
                    }
                    in_window += 1;
                    examined += 1;
                    let raw: String = row.get(2)?;
                    let normalized: Option<String> = row.get(3)?;
                    let matched = if wants_normalized {
                        matcher.matches(None, normalized.as_deref(), &mut scratch)
                    } else {
                        matcher.matches(Some(&raw), None, &mut scratch)
                    };
                    if !matched {
                        continue;
                    }
                    accepted.push((row.get(0)?, row.get(1)?, row.get(4)?, raw));
                    if accepted.len() as i64 > limit {
                        stopped_early = true;
                        break 'scan;
                    }
                }
                if exhausted {
                    break;
                }
            }
        }
        page.verification = SearchVerification {
            candidates_examined: examined,
            windows,
            stopped_early,
            scan_ms: started.elapsed().as_secs_f64() * 1000.0,
        };

        let has_more = accepted.len() as i64 > limit;
        if has_more {
            accepted.truncate(limit as usize);
        }
        // İkinci aşama: yalnız dönen sonuçların belge/sayfa üst verisi.
        let metadata =
            self.hit_metadata(&accepted.iter().map(|row| row.0.clone()).collect::<Vec<_>>())?;
        let mut hits: Vec<SearchHit> = Vec::with_capacity(accepted.len());
        for (segment_id, page_id, rank, raw_text) in accepted {
            let extra = metadata.get(&segment_id);
            hits.push(SearchHit {
                document_version_id: extra.and_then(|meta| meta.document_version_id.clone()),
                document_id: extra.and_then(|meta| meta.document_id.clone()),
                document_name: extra.and_then(|meta| meta.display_name.clone()),
                document_type: extra.and_then(|meta| meta.document_type.clone()),
                document_status: extra.and_then(|meta| meta.status.clone()),
                text_origin: extra.and_then(|meta| meta.text_origin.clone()),
                page_index: extra.and_then(|meta| meta.page_index),
                case_id: extra.map(|meta| meta.case_id.clone()),
                segment_id,
                page_id,
                raw_text,
                rank,
                tags: Vec::new(),
            });
        }
        page.has_more = has_more;
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

        // Toplam: sayfa **bütün** eşleşmeleri döndürdüyse toplam dönen sayıdır;
        // ikinci geçişe gerek yok. İmleçli sayfada bu çıkarım geçersizdir
        // (elde kalan satırlar sayılır), orada tam sayım yapılır. Tam sayım
        // FTS-güdümlü olmayan planla yapılır (`count_where`): 3.000 adaylı
        // sorguda 286 ms → <1 ms (F15 ölçümü).
        if include_total {
            page.total = Some(if !page.has_more && cursor.is_none() {
                // Adaylar tükendi: sayfa **tüm** eşleşmeleri taşıyor, ikinci
                // geçiş gerekmez (erken kesme yoksa doğru).
                hits.len() as i64
            } else {
                // Tam sayım doğrulamayı SQL'de yapar ve bütün adayları gezmek
                // zorundadır — bu yüzden pahalıdır; arayüz onu `search_total`
                // ile ayrı ve sonradan ister. Burada yalnız `include_total`
                // açıkça istendiğinde çalışır.
                let verify = if wants_normalized {
                    "dim_search_verify(NULL, s.normalized_text)"
                } else {
                    "dim_search_verify(s.raw_text, NULL)"
                };
                self.register_search_verifier(std::sync::Arc::clone(&matcher))?;
                let (count_sql, count_params) = count_where(case_id, &expression, &filters, verify);
                self.conn().query_row(
                    &format!("SELECT count(*) FROM {COUNT_FROM}{count_sql}"),
                    rusqlite::params_from_iter(count_params.iter()),
                    |row| row.get(0),
                )?
            });
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

    /// Toplam eşleşme sayısı — **sayfadan ayrı** çağrılabilir (F15 ertelenmiş
    /// sayım).
    ///
    /// Aynı kümeyi `search_page(..., include_total=true)` ile birebir aynı
    /// kuralla sayar (`count_where`: doğrulama geçen satırlar). Fark yalnız
    /// çağrı düzenindedir: arayüz ilk sayfayı gösterdikten **sonra** bu komutu
    /// çalıştırır. Ölçüm (B-01, 1.000 belge / 100.000 segment): sayım sayfa
    /// sorgusuna p50'de **+643 ms (%52)**, p95'te **+1.467 ms** ekliyordu;
    /// sayfa artık bu maliyeti beklemez.
    ///
    /// Sayım **tam**dır: "en çok N" ya da pencere kırpması yoktur. Bu yüzden
    /// ucuz da değildir — çağıran taraf bunu kullanıcıya beklemesiz göstermeli
    /// (arayüz: "sayılıyor…" → sayı), sayfayı bloke etmemelidir.
    pub fn search_total(
        &self,
        case_id: &str,
        query: &SearchQuery,
        filters: &SearchFilters,
    ) -> Result<SearchTotal, StorageError> {
        let filters = filters.normalized()?;
        let resolved = query.resolve()?;
        let mut out = SearchTotal {
            case_id: case_id.to_string(),
            query: query.text.trim().to_string(),
            resolved_query: resolved.text.clone(),
            mode: resolved.mode,
            scope: "case".to_string(),
            total: 0,
            took_ms: 0.0,
            filters: filters.clone(),
        };
        let expression = fts_expression(&resolved);
        if resolved.text.is_empty() || expression.is_empty() {
            return Ok(out);
        }

        let started = std::time::Instant::now();
        let matcher = SearchMatcher::new(&resolved.text, resolved.mode, resolved.docket.clone());
        let verify = if matcher.wants_normalized() {
            "dim_search_verify(NULL, s.normalized_text)"
        } else {
            "dim_search_verify(s.raw_text, NULL)"
        };
        self.register_search_verifier(std::sync::Arc::new(matcher))?;

        let (count_sql, count_params) = count_where(case_id, &expression, &filters, verify);
        out.total = self.conn().query_row(
            &format!("SELECT count(*) FROM {COUNT_FROM}{count_sql}"),
            rusqlite::params_from_iter(count_params.iter()),
            |row| row.get(0),
        )?;
        out.took_ms = started.elapsed().as_secs_f64() * 1000.0;
        Ok(out)
    }

    /// Aday doğrulamasını sorguya bağlar (F15 ölçek düzeltmesi).
    ///
    /// Skaler fonksiyon adı sabittir; her arama kendi matcher'ını kaydeder.
    /// Kayıt bağlantıya bağlıdır ve arama tek iş parçacığında bittiği için
    /// arama sürerken değişmez.
    fn register_search_verifier(
        &self,
        matcher: std::sync::Arc<SearchMatcher>,
    ) -> Result<(), StorageError> {
        let mut scratch = MatchScratch::default();
        self.conn().create_scalar_function(
            "dim_search_verify",
            2,
            FunctionFlags::SQLITE_UTF8,
            move |ctx| {
                let raw: Option<String> = ctx.get::<Option<String>>(0)?;
                let normalized: Option<String> = ctx.get::<Option<String>>(1)?;
                Ok(i64::from(matcher.matches(
                    raw.as_deref(),
                    normalized.as_deref(),
                    &mut scratch,
                )))
            },
        )?;
        Ok(())
    }

    /// İkinci aşama (F15 iki aşamalı tarama): dönen sonuçların belge/sayfa üst
    /// verisi tek `IN` sorgusuyla okunur. Aday akışında JOIN taşınmadığı için
    /// seyrek eşleşen sorguda binlerce aday bu maliyeti ödemez.
    ///
    /// Eksik satır olursa alanlar **boş** kalır; üst veri uydurulmaz.
    fn hit_metadata(
        &self,
        segment_ids: &[String],
    ) -> Result<std::collections::HashMap<String, HitMetadata>, StorageError> {
        let mut out = std::collections::HashMap::with_capacity(segment_ids.len());
        if segment_ids.is_empty() {
            return Ok(out);
        }
        let placeholders = vec!["?"; segment_ids.len()].join(",");
        let sql = format!(
            "SELECT s.id, p.page_index, pr.document_version_id, dv.document_id,
                    d.display_name, d.document_type, d.status, p.text_origin, s.case_id
             FROM text_segments s
             LEFT JOIN pages p ON p.id = s.page_id
             LEFT JOIN parse_runs pr ON pr.id = s.parse_run_id
             LEFT JOIN document_versions dv ON dv.id = pr.document_version_id
             LEFT JOIN documents d ON d.id = dv.document_id
             WHERE s.id IN ({placeholders})"
        );
        let params: Vec<Value> = segment_ids
            .iter()
            .map(|id| Value::Text(id.clone()))
            .collect();
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok(HitMetadata {
                segment_id: row.get(0)?,
                page_index: row.get(1)?,
                document_version_id: row.get(2)?,
                document_id: row.get(3)?,
                display_name: row.get(4)?,
                document_type: row.get(5)?,
                status: row.get(6)?,
                text_origin: row.get(7)?,
                case_id: row.get(8)?,
            })
        })?;
        for row in rows {
            let meta = row?;
            out.insert(meta.segment_id.clone(), meta);
        }
        Ok(out)
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
pub(crate) struct ResolvedQuery {
    pub(crate) mode: SearchMode,
    pub(crate) text: String,
    pub(crate) docket: Option<DocketNumber>,
}

/// `2024/123 E.` ayrıştırması. Numara baştaki sıfırlardan arındırılır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DocketNumber {
    pub(crate) year: String,
    pub(crate) number: String,
    pub(crate) role: Option<char>,
}

impl SearchQuery {
    /// Emsal kütüphanesi araması da **aynı** çözümlemeyi kullanır (F12-T10);
    /// kip ve tırnak kuralı iki yerde ayrışmasın.
    pub(crate) fn resolve(&self) -> Result<ResolvedQuery, StorageError> {
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
        // Esas/karar numarası: yıl + numara belirteçleri (sıfır varyantlarıyla).
        SearchMode::Docket => match query.docket.as_ref() {
            Some(docket) => docket_expression(docket),
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

/// Esas/karar numarası adayları: **yıl ve numara** belirteçleri birlikte.
///
/// Önceki hâlde yalnız yıl aranıyordu; yoğun bir yılda bu, doğrulamaya binlerce
/// aday bırakıyordu. Numara, baştaki sıfır varyantlarıyla ve **önek** olarak
/// aranır (`2024/0123` → `0123`, `2024/123E` → `123e`). Eşleşme kuralı
/// değişmez: kesin doğrulamayı yine `docket_match` yapar, burada yalnız aday
/// daraltılır. Yıl ile numara arasında yalnız boşluk/nokta ve `/` olabildiği
/// için ikisi FTS'te her zaman bitişik belirteçlerdir.
fn docket_expression(docket: &DocketNumber) -> String {
    // Numara metinde baştaki sıfırlarla yazılmış olabilir; doğrulama sıfırları
    // atar, aday üretimi de aynı biçimleri aramalı.
    const MAX_DIGITS: usize = 10;
    let digits = docket.number.chars().count();
    (0..=MAX_DIGITS.saturating_sub(digits))
        .map(|zeros| {
            format!(
                "raw_text : \"{} {}{}\"*",
                escape_fts(&docket.year),
                "0".repeat(zeros),
                escape_fts(&docket.number)
            )
        })
        .collect::<Vec<_>>()
        .join(" OR ")
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
pub(crate) fn docket_match(text: &str, docket: &DocketNumber) -> bool {
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

/// Türkçe küçük harf eşlemesi (tek karakter). `İ`→`i`, `I`→`ı`; diğerleri
/// Unicode küçük harf. Çok karaktere açılan tek eşleme `İ`'dir ve burada özel
/// olarak ele alındığı için tek karakter döndürmek `turkish_lower` ile aynıdır.
fn fold_char(ch: char) -> char {
    match ch {
        'İ' => 'i',
        'I' => 'ı',
        _ => ch.to_lowercase().next().unwrap_or(ch),
    }
}

/// `turkish_lower(collapse_ws(text))` çıktısını `out` içine yazar (ayırma yok).
fn fold_ws_into(text: &str, out: &mut String) {
    out.clear();
    let mut last_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if out.is_empty() || last_space {
                continue;
            }
            last_space = true;
            out.push(' ');
        } else {
            last_space = false;
            out.push(fold_char(ch));
        }
    }
}

/// `turkish_lower(collapse_ws(haystack)).contains(needle)` ile birebir aynı
/// sonuç; `folded` yeniden kullanılan tampondur (satır başına ayırma yok).
fn contains_folded_ws(haystack: &str, needle: &str, folded: &mut String) -> bool {
    if needle.is_empty() {
        return false;
    }
    fold_ws_into(haystack, folded);
    folded.contains(needle)
}

/// Metnin alfanümerik belirteç aralıkları (bayt). `turkish_lower` belirteç
/// sınırlarını değiştirmez; bu yüzden ham metin üzerinde hesaplanabilir.
fn token_spans(text: &str, out: &mut Vec<(usize, usize)>) {
    out.clear();
    let mut start: Option<usize> = None;
    for (index, ch) in text.char_indices() {
        if ch.is_alphanumeric() {
            if start.is_none() {
                start = Some(index);
            }
        } else if let Some(begin) = start.take() {
            out.push((begin, index));
        }
    }
    if let Some(begin) = start {
        out.push((begin, text.len()));
    }
}

/// Bir belirteç iğne belirteciyle eşleşiyor mu? `prefix` ise samanlık belirteci
/// iğne belirtecinden **uzun** olabilir (son kelime öneki).
fn token_equals(text: &str, span: (usize, usize), needle: &str, prefix: bool) -> bool {
    let mut hay = text[span.0..span.1].chars();
    let mut want = needle.chars();
    loop {
        match (hay.next(), want.next()) {
            (None, None) => return true,
            (Some(h), Some(w)) => {
                if fold_char(h) != w {
                    return false;
                }
            }
            (Some(_), None) => return prefix,
            (None, Some(_)) => return false,
        }
    }
}

/// Belirteç aralıkları üzerinde bitişik dizi eşleşmesi (ayırma yok).
fn seq_matches_spans(
    text: &str,
    spans: &[(usize, usize)],
    needle: &[String],
    prefix_last: bool,
) -> bool {
    if needle.is_empty() || needle.len() > spans.len() {
        return false;
    }
    let last = needle.len() - 1;
    for start in 0..=(spans.len() - needle.len()) {
        let mut all = true;
        for (offset, want) in needle.iter().enumerate() {
            if !token_equals(
                text,
                spans[start + offset],
                want,
                prefix_last && offset == last,
            ) {
                all = false;
                break;
            }
        }
        if all {
            return true;
        }
    }
    false
}

/// `i` konumundan başlayarak boşluk/nokta atlar (Unicode boşluk dahil).
fn skip_ws_dots(text: &str, mut j: usize) -> usize {
    while j < text.len() {
        let ch = text[j..].chars().next().unwrap_or(' ');
        if ch.is_whitespace() || ch == '.' {
            j += ch.len_utf8();
        } else {
            break;
        }
    }
    j
}

/// `i` konumundan başlayarak yalnız boşluk atlar.
fn skip_ws(text: &str, mut j: usize) -> usize {
    while j < text.len() {
        let ch = text[j..].chars().next().unwrap_or(' ');
        if ch.is_whitespace() {
            j += ch.len_utf8();
        } else {
            break;
        }
    }
    j
}

/// `docket_match` ile aynı kural, ayırma yapmadan (bayt taraması).
fn docket_match_bytes(text: &str, docket: &DocketNumber) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if &text[start..i] != docket.year.as_str() {
            continue;
        }
        let mut j = skip_ws_dots(text, i);
        if j >= bytes.len() || bytes[j] != b'/' {
            continue;
        }
        j = skip_ws(text, j + 1);
        let number_start = j;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j == number_start {
            continue;
        }
        let raw_number = &text[number_start..j];
        let trimmed = raw_number.trim_start_matches('0');
        let number = if trimmed.is_empty() { "0" } else { trimmed };
        if number != docket.number {
            continue;
        }
        let Some(expected) = docket.role else {
            return true;
        };
        let mut k = j;
        while k < text.len() {
            let ch = text[k..].chars().next().unwrap_or(' ');
            if ch.is_whitespace() || matches!(ch, '.' | ',' | '-' | '/') {
                k += ch.len_utf8();
            } else {
                break;
            }
        }
        if k < text.len() && text[k..].starts_with(expected) {
            return true;
        }
    }
    false
}

/// Aday satır doğrulaması için yeniden kullanılan tamponlar.
#[derive(Default)]
pub(crate) struct MatchScratch {
    folded: String,
    spans: Vec<(usize, usize)>,
}

/// Sorgu başına **bir kez** hazırlanan doğrulayıcı (F15 ölçek düzeltmesi).
///
/// Önceki hâlde SQL skaler fonksiyonu her aday satır için iğneyi yeniden
/// tokenize ediyor ve segment metnini yeniden normalleştiriyordu; 1.000 belge
/// ölçümünde gecikmenin ana nedeni buydu. Burada iğne bir kez hazırlanır,
/// satır başına yalnız samanlık taranır; `flexible` kipinde zaten
/// `dim_evidence::normalize` çıktısı olan `normalized_text` kullanılır.
pub(crate) struct SearchMatcher {
    mode: SearchMode,
    /// `exact`: `turkish_lower(collapse_ws(iğne))`.
    exact: String,
    /// `phrase`: Türkçe küçük harfli belirteçler.
    phrase: Vec<String>,
    /// `flexible`: sadeleştirilmiş belirteçler.
    folded: Vec<String>,
    /// `docket`: ayrıştırılmış numara (yoksa kip eşleşmez).
    docket: Option<DocketNumber>,
}

impl SearchMatcher {
    pub(crate) fn new(text: &str, mode: SearchMode, docket: Option<DocketNumber>) -> Self {
        Self {
            mode,
            exact: turkish_lower(&collapse_ws(text)),
            phrase: exact_tokens(text),
            folded: fold_tokens(text),
            docket,
        }
    }

    /// Bu kipin doğrulaması ham metni mi yoksa `normalized_text` sütununu mu
    /// istediğini söyler (SQL tarafında yalnız gereken sütun okunur).
    fn wants_normalized(&self) -> bool {
        self.mode == SearchMode::Flexible
    }

    pub(crate) fn matches(
        &self,
        raw: Option<&str>,
        normalized: Option<&str>,
        scratch: &mut MatchScratch,
    ) -> bool {
        match self.mode {
            SearchMode::Exact => {
                raw.is_some_and(|raw| contains_folded_ws(raw, &self.exact, &mut scratch.folded))
            }
            SearchMode::Phrase => {
                let Some(raw) = raw else { return false };
                token_spans(raw, &mut scratch.spans);
                seq_matches_spans(raw, &scratch.spans, &self.phrase, false)
            }
            SearchMode::Flexible => {
                let Some(normalized) = normalized else {
                    return false;
                };
                token_spans(normalized, &mut scratch.spans);
                seq_matches_spans(normalized, &scratch.spans, &self.folded, true)
            }
            SearchMode::Docket => self
                .docket
                .as_ref()
                .is_some_and(|docket| docket_match_bytes(raw.unwrap_or(""), docket)),
            SearchMode::Auto => false,
        }
    }
}

/// SQL doğrulaması: `mode` etiketiyle gelen kipin kesin eşleşme kuralı.
/// Yeni kod `SearchMatcher` kullanır; bu fonksiyon referans uygulama olarak
/// (ve eşdeğerlik testi için) kalır.
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

/// Sayımın FROM'u. FTS tablosu **yok**: sürücü tablo `text_segments`.
/// (Gerekçe `count_where` belgesinde — F15 ölçümü.)
const COUNT_FROM: &str = "text_segments s
     LEFT JOIN pages p ON p.id = s.page_id
     LEFT JOIN parse_runs pr ON pr.id = s.parse_run_id
     LEFT JOIN document_versions dv ON dv.id = pr.document_version_id
     LEFT JOIN documents d ON d.id = dv.document_id";

/// Filtre koşullarını (belge türü, OCR durumu, etiketler, tarih rolü) ekler.
///
/// Etiket filtresi **VE** semantiği taşır: verilen bütün etiketler aynı belgede
/// olmalı. Tarih rolü, belgeye *kaynaklı* olayın rolüdür — belgeye bağlanmamış
/// olay bu belgeyi sonuca sokmaz.
fn push_filters(sql: &mut String, params: &mut Vec<Value>, case_id: &str, filters: &SearchFilters) {
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
}

/// Aday sorgusunun WHERE'i (parametre sırası: MATCH, `case_id`, filtreler).
///
/// **Doğrulama yok** (F15 kuyruk düzeltmesi): `dim_search_verify` WHERE'e
/// konduğunda SQLite onu `ORDER BY bm25` sıralamasından önce her adaya
/// uyguluyordu. Doğrulama artık Rust'ta, sıralanmış akış üzerinde ve yalnız
/// sayfanın ihtiyaç duyduğu adaylar için yapılır (`search_page_windowed`).
fn search_where_candidates(
    case_id: &str,
    expression: &str,
    filters: &SearchFilters,
) -> (String, Vec<Value>) {
    let mut sql = " WHERE text_segments_fts MATCH ? AND s.case_id = ?".to_string();
    let mut params: Vec<Value> = vec![
        Value::Text(expression.to_string()),
        Value::Text(case_id.to_string()),
    ];
    push_filters(&mut sql, &mut params, case_id, filters);
    (sql, params)
}

/// Sayım sorgusunun WHERE'i (parametre sırası: `case_id`, MATCH, filtreler).
///
/// F15 ölçümü: FTS tablosu FROM'da olduğunda SQLite her aday için **rastgele**
/// birincil anahtar araması yapıyor; 3.000 adaylı bir kelime sorgusunda sayım
/// 286 ms sürüyordu. Aynı kümeyi `rowid IN (SELECT … MATCH …)` ile vermek
/// planlayıcıyı `text_segments` üzerinde sıralı taramaya geçiriyor: **<1 ms**.
/// Kapsam aynı (aynı filtreler, aynı doğrulama), yalnız plan değişiyor.
fn count_where(
    case_id: &str,
    expression: &str,
    filters: &SearchFilters,
    verify: &str,
) -> (String, Vec<Value>) {
    let mut sql = format!(
        " WHERE s.case_id = ?
          AND s.rowid IN (SELECT rowid FROM text_segments_fts
                           WHERE text_segments_fts MATCH ?)
          AND {verify} = 1"
    );
    let mut params: Vec<Value> = vec![
        Value::Text(case_id.to_string()),
        Value::Text(expression.to_string()),
    ];
    push_filters(&mut sql, &mut params, case_id, filters);
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

/// İkinci aşamada okunan belge/sayfa üst verisi (F15 iki aşamalı tarama).
struct HitMetadata {
    segment_id: String,
    page_index: Option<i64>,
    document_version_id: Option<String>,
    document_id: Option<String>,
    display_name: Option<String>,
    document_type: Option<String>,
    status: Option<String>,
    text_origin: Option<String>,
    case_id: String,
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

/// Aday penceresi (F15 kuyruk düzeltmesi): sayfa sorgusu adayları `bm25`
/// sırasında bu kadar satırlık pencereler hâlinde **doğrular**; sayfa dolduğu
/// an tarama kesilir. 200, `MAX_SEARCH_LIMIT`in üstünde kalır ki tek pencere
/// tipik bir sayfayı taşıyabilsin.
pub const SEARCH_CANDIDATE_WINDOW: i64 = 200;

/// `Database::pages_for_run` sorgusu. Testler **bu metnin** planını denetler;
/// kopya bir sorgu yazılsaydı indeks denetimi sessizce eskiyebilirdi.
///
/// Üç korelasyonlu alt sorgu da `text_segments.page_id` üzerinden arar; bu
/// sütunun indeksi (migration 0018) yokken her sayfa okuması tam tablo taraması
/// yapıyordu (F15 ölçümü, `docs/releases/F15/BENCHMARK.md`).
pub(crate) const PAGES_FOR_RUN_SQL: &str = "SELECT p.id,
                    (SELECT s.id FROM text_segments s WHERE s.page_id=p.id LIMIT 1) AS segment_id,
                    p.page_index,
                    COALESCE((SELECT s.raw_text FROM text_segments s WHERE s.page_id=p.id LIMIT 1),'') AS text,
                    p.text_origin,p.quality_flags,p.width,p.height,p.rotation,p.coordinate_space,p.page_label,
                    (SELECT s.bbox_list FROM text_segments s WHERE s.page_id=p.id LIMIT 1) AS bbox_list
             FROM pages p WHERE p.parse_run_id=?1 ORDER BY p.page_index";

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
    /// Sayfanın doğrulama maliyeti (F15 kuyruk düzeltmesi) — kaç adayın
    /// gerçekten doğrulandığını **dürüstçe** taşır; arayüz bunu gösterir.
    #[serde(default)]
    pub verification: SearchVerification,
    pub hits: Vec<SearchHit>,
}

/// Sayfa doğrulamasının gerçek maliyeti (F15). "Hızlı" iddiası buradan
/// okunur: erken kesme olmuşsa adayların tamamı hiç doğrulanmamıştır.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchVerification {
    /// Doğrulanan (metni okunan) FTS adayı sayısı.
    pub candidates_examined: i64,
    /// Gezilen aday penceresi sayısı.
    pub windows: i64,
    /// Sayfa dolduğu için taramanın erken kesilip kesilmediği. `false` ise
    /// adayların **tamamı** doğrulandı (sayfa eksik değil, aday bitti).
    pub stopped_early: bool,
    /// Aday taraması + doğrulamanın çekirdekteki süresi (ms).
    pub scan_ms: f64,
}

/// Ertelenmiş toplam sayımın sonucu (F15). `search_page` ile **aynı** sorgu
/// çözümünü taşır ki arayüz sayının hangi sorguya ait olduğunu doğrulayabilsin
/// (sayım sürerken yeni arama başlarsa eski sayı gösterilmez).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchTotal {
    pub case_id: String,
    pub query: String,
    pub resolved_query: String,
    pub mode: SearchMode,
    /// `case` = yalnız bu dosya.
    pub scope: String,
    /// Doğrulamayı geçen **tam** eşleşme sayısı.
    pub total: i64,
    /// Sayımın çekirdekteki süresi (ms) — arayüzde "ne kadar sürdü" olarak
    /// gösterilir; ölçüm iddiası buradan gelir.
    pub took_ms: f64,
    pub filters: SearchFilters,
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

    /// F15-T05 kusuru: `text_segments.page_id` indekssizken `pages_for_run`
    /// her sayfa için üç tam tablo taraması yapıyordu. Plan **gerçek** sorgu
    /// metninden okunur (`PAGES_FOR_RUN_SQL`), kopyasından değil.
    #[test]
    fn pages_for_run_plan_uses_indexes() {
        let (db, _, _) = setup();
        let mut stmt = db
            .conn()
            .prepare(&format!("EXPLAIN QUERY PLAN {PAGES_FOR_RUN_SQL}"))
            .expect("plan hazırlanır");
        let details: Vec<String> = stmt
            .query_map(["run-1"], |row| row.get::<_, String>(3))
            .expect("plan satırları")
            .collect::<Result<_, _>>()
            .expect("plan okunur");
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("idx_text_segments_page")),
            "segment araması page_id indeksini kullanmalı: {details:?}"
        );
        assert!(
            !details
                .iter()
                .any(|detail| detail.starts_with("SCAN s") || detail.contains("SCAN text_segments")),
            "segment tablosu taranmamalı: {details:?}"
        );
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("SEARCH p USING INDEX")
                    && detail.contains("parse_run_id=?")),
            "sayfa araması parse_run_id indeksiyle olmalı: {details:?}"
        );
    }

    /// Sayfa okuması yalnız kendi ayrıştırma koşusunun sayfalarını döndürür;
    /// komşu koşunun çok sayıda sayfası sonucu değiştirmez.
    #[test]
    fn pages_for_run_reads_only_its_own_run() {
        let (db, case_id, version_id) = setup();
        let bulk = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("komşu koşu");
        let bulk_pages: Vec<NewPage> = (0..500)
            .map(|index| NewPage {
                index,
                text: format!("Komşu koşu sayfası {index}"),
                origin: "native".into(),
                ..Default::default()
            })
            .collect();
        db.store_pages(&bulk.id, &case_id, &bulk_pages)
            .expect("komşu sayfalar");

        let own = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("kendi koşusu");
        db.store_pages(
            &own.id,
            &case_id,
            &[
                NewPage {
                    index: 0,
                    text: "Kendi ilk sayfası".into(),
                    origin: "native".into(),
                    ..Default::default()
                },
                NewPage {
                    index: 1,
                    text: "Kendi ikinci sayfası".into(),
                    origin: "native".into(),
                    ..Default::default()
                },
            ],
        )
        .expect("kendi sayfaları");

        let pages = db.pages_for_run(&own.id).expect("sayfalar");
        assert_eq!(pages.len(), 2, "yalnız kendi koşusunun sayfaları");
        assert_eq!(pages[0].text, "Kendi ilk sayfası");
        assert_eq!(pages[1].text, "Kendi ikinci sayfası");
        assert_eq!(db.pages_for_run(&bulk.id).expect("komşu").len(), 500);
    }

    /// Bir sayfada birden çok segment varsa sayfanın **ilk** segmenti döner
    /// (rowid sırası). İndeks eklendikten sonra da bu davranış korunur.
    #[test]
    fn pages_for_run_returns_first_segment_of_a_page() {
        let (db, case_id, version_id) = setup();
        let run = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        db.store_pages(
            &run.id,
            &case_id,
            &[NewPage {
                index: 0,
                text: "İlk segment metni".into(),
                origin: "native".into(),
                ..Default::default()
            }],
        )
        .expect("sayfa");
        let page_id: String = db
            .conn()
            .query_row(
                "SELECT id FROM pages WHERE parse_run_id = ?1",
                [&run.id],
                |row| row.get(0),
            )
            .expect("sayfa kimliği");
        let second = "İkinci segment metni";
        db.conn()
            .execute(
                "INSERT INTO text_segments
                 (id,parse_run_id,page_id,case_id,structural_path,raw_text,normalized_text,offset_map,bbox_list,created_at)
                 VALUES (?1,?2,?3,?4,'page/0/seg1',?5,?6,'[]','[]',?7)",
                rusqlite::params![
                    uuid::Uuid::now_v7().to_string(),
                    &run.id,
                    &page_id,
                    &case_id,
                    second,
                    dim_evidence::normalize(second),
                    crate::now_iso(),
                ],
            )
            .expect("ikinci segment");
        let pages = db.pages_for_run(&run.id).expect("sayfalar");
        assert_eq!(pages.len(), 1, "sayfa başına tek satır");
        assert_eq!(pages[0].text, "İlk segment metni");
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

    /// İki aşamalı tarama (F15): üst veri ikinci aşamada, ayrı sorguyla gelir.
    /// Sonucun bütün alanları **dolu** olmalı ve filtreli aramada da eksilmemeli;
    /// eksik satır uydurma değerle doldurulmaz.
    #[test]
    fn two_stage_scan_keeps_full_hit_metadata() {
        let (db, case_id, _version_id) = setup();
        let (document_id, version_id, run_id, segment_id) = add_document_with_text(
            &db,
            &case_id,
            "Kira sözleşmesi",
            "contract",
            "İtirazın reddine karar verildi",
            "ocr",
        );
        assert!(!run_id.is_empty(), "koşu kimliği dolu olmalı");
        db.add_document_tag(&document_id, "kira").expect("etiket");
        db.add_document_tag(&document_id, "2024").expect("etiket");

        let page = db
            .search_page(
                &case_id,
                &SearchQuery::auto("itirazın reddine"),
                &SearchFilters::default(),
                10,
                None,
                false,
            )
            .expect("arama");
        assert_eq!(page.hits.len(), 1);
        let hit = &page.hits[0];
        assert_eq!(hit.segment_id, segment_id);
        assert_eq!(
            hit.document_version_id.as_deref(),
            Some(version_id.as_str())
        );
        assert_eq!(hit.document_id.as_deref(), Some(document_id.as_str()));
        assert_eq!(hit.document_name.as_deref(), Some("Kira sözleşmesi"));
        assert_eq!(hit.document_type.as_deref(), Some("contract"));
        assert!(hit.document_status.is_some(), "belge durumu taşınmalı");
        assert_eq!(hit.text_origin.as_deref(), Some("ocr"));
        assert_eq!(hit.case_id.as_deref(), Some(case_id.as_str()));
        assert_eq!(hit.page_index, Some(0));
        assert_eq!(hit.tags, vec!["2024".to_string(), "kira".to_string()]);

        // Filtreli aramada JOIN'ler WHERE için devrede; sonuç yine tam olmalı.
        let filtered = db
            .search_page(
                &case_id,
                &SearchQuery::auto("itirazın reddine"),
                &SearchFilters {
                    document_types: vec!["contract".into()],
                    ocr_status: Some("ocr".into()),
                    tags: vec!["kira".into()],
                    ..Default::default()
                },
                10,
                None,
                false,
            )
            .expect("filtreli arama");
        assert_eq!(filtered.hits.len(), 1);
        assert_eq!(
            filtered.hits[0].document_name.as_deref(),
            Some("Kira sözleşmesi")
        );
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

    /// F15 kuyruk düzeltmesi: aday penceresi **sonucu değiştirmez** — aynı
    /// sorgu, aynı sıra, aynı toplam. Pencere yalnız kaç adayın doğrulandığını
    /// belirler.
    #[test]
    fn candidate_window_does_not_change_results() {
        let (db, case_id, version_id) = setup();
        let mut texts: Vec<String> = (0..12)
            .map(|i| format!("reddine numaralı {i} sayfa")) // aday, eşleşme değil
            .collect();
        texts.push("numaralı reddine kararı".into());
        texts.push("NUMARALI REDDINE büyük harf".into());
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        store_texts(&db, &case_id, &version_id, &refs);

        let query = SearchQuery {
            text: "numaralı reddine".into(),
            mode: SearchMode::Flexible,
        };
        let base = db
            .search_page_windowed(
                &case_id,
                &query,
                &SearchFilters::default(),
                5,
                None,
                true,
                64,
            )
            .expect("geniş pencere");
        assert_eq!(base.hits.len(), 2, "iki gerçek eşleşme");
        assert_eq!(base.total, Some(2), "sayı aday sayısı değil eşleşme sayısı");

        for window in 1..=4 {
            let page = db
                .search_page_windowed(
                    &case_id,
                    &query,
                    &SearchFilters::default(),
                    5,
                    None,
                    true,
                    window,
                )
                .expect("küçük pencere");
            let ids: Vec<&str> = page.hits.iter().map(|h| h.segment_id.as_str()).collect();
            let base_ids: Vec<&str> = base.hits.iter().map(|h| h.segment_id.as_str()).collect();
            assert_eq!(ids, base_ids, "pencere {window} sonucu değiştirmemeli");
            assert_eq!(
                page.total, base.total,
                "pencere {window} sayıyı değiştirmemeli"
            );
            assert_eq!(page.has_more, base.has_more);
            assert_eq!(page.next_cursor, base.next_cursor);
        }

        // Küçük pencerede sayfa dolmadığı için adayların **tamamı** gezilir:
        // 14 aday / 2'lik pencere = 7 pencere, erken kesme yok.
        let small = db
            .search_page_windowed(
                &case_id,
                &query,
                &SearchFilters::default(),
                5,
                None,
                false,
                2,
            )
            .expect("pencere 2");
        assert!(!small.verification.stopped_early);
        assert_eq!(small.verification.candidates_examined, 14);
        assert_eq!(small.verification.windows, 7);
    }

    /// Sayfa dolduğu an tarama kesilir: 30 eşleşen aday varken 3 sonuç için
    /// **limit+1** aday doğrulanır, gerisi hiç okunmaz (kuyruğun kaynağı buydu).
    #[test]
    fn page_stops_scanning_when_it_fills() {
        let (db, case_id, version_id) = setup();
        let texts: Vec<String> = (0..30).map(|i| format!("karar metni {i}")).collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        store_texts(&db, &case_id, &version_id, &refs);

        let query = SearchQuery {
            text: "karar".into(),
            mode: SearchMode::Exact,
        };
        let page = db
            .search_page_windowed(
                &case_id,
                &query,
                &SearchFilters::default(),
                3,
                None,
                true,
                200,
            )
            .expect("sayfa");
        assert_eq!(page.hits.len(), 3);
        assert!(page.has_more);
        assert_eq!(page.total, Some(30), "toplam tam sayıdır");
        assert!(page.verification.stopped_early);
        assert_eq!(
            page.verification.candidates_examined, 4,
            "limit+1 aday doğrulanır, 30 adayın tamamı değil"
        );
        assert_eq!(page.verification.windows, 1);
    }

    /// İlk pencerede eşleşme yoksa tarama **devam eder**: eksik sayfa
    /// döndürmek yerine sonraki adaylar doğrulanır.
    #[test]
    fn sparse_matches_after_non_matching_candidates_are_found() {
        let (db, case_id, version_id) = setup();
        let mut texts: Vec<String> = (0..8)
            .map(|i| format!("reddine {i} numaralı")) // ters sıra: aday, eşleşme değil
            .collect();
        texts.push("tam eşleşme: numaralı reddine".into());
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        store_texts(&db, &case_id, &version_id, &refs);

        let query = SearchQuery {
            text: "numaralı reddine".into(),
            mode: SearchMode::Flexible,
        };
        let page = db
            .search_page_windowed(
                &case_id,
                &query,
                &SearchFilters::default(),
                5,
                None,
                true,
                1,
            )
            .expect("pencere 1");
        assert_eq!(page.hits.len(), 1, "eşleşme pencere sınırında kaybolmamalı");
        assert_eq!(page.hits[0].raw_text, "tam eşleşme: numaralı reddine");
        assert_eq!(page.total, Some(1));
        assert!(
            page.verification.candidates_examined >= 2,
            "ilk pencere eşleşmediği için tarama devam etmeli"
        );
        assert!(page.verification.windows >= 2);
        assert!(!page.verification.stopped_early);
    }

    /// Keyset sayfalama pencere boyutundan bağımsız: her pencere boyutunda
    /// aynı segment kimlikleri, tekrar yok, atlama yok.
    #[test]
    fn keyset_pagination_is_identical_for_every_window_size() {
        let (db, case_id, version_id) = setup();
        let texts: Vec<String> = (0..25)
            .map(|i| format!("kira sözleşmesi {i} maddesi"))
            .collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        store_texts(&db, &case_id, &version_id, &refs);

        let query = SearchQuery {
            text: "kira".into(),
            mode: SearchMode::Exact,
        };
        let mut sequences: Vec<Vec<String>> = Vec::new();
        for window in [1i64, 3, 200] {
            let mut ids: Vec<String> = Vec::new();
            let mut cursor: Option<String> = None;
            loop {
                let page = db
                    .search_page_windowed(
                        &case_id,
                        &query,
                        &SearchFilters::default(),
                        4,
                        cursor.as_deref(),
                        false,
                        window,
                    )
                    .expect("sayfa");
                ids.extend(page.hits.iter().map(|h| h.segment_id.clone()));
                cursor = page.next_cursor.clone();
                if cursor.is_none() {
                    break;
                }
            }
            assert_eq!(ids.len(), 25, "pencere {window}: tüm eşleşmeler bir kez");
            let unique: std::collections::BTreeSet<&String> = ids.iter().collect();
            assert_eq!(unique.len(), 25, "pencere {window}: tekrar yok");
            sequences.push(ids);
        }
        assert_eq!(sequences[0], sequences[1], "pencere 1 ↔ 3 aynı sıra");
        assert_eq!(sequences[0], sequences[2], "pencere 1 ↔ 200 aynı sıra");
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

    /// F15 ölçek düzeltmesi: hızlı doğrulayıcı, referans uygulamayla **birebir**
    /// aynı sonucu vermeli. Aday daraltması değişse bile kesin eşleşme kuralı
    /// tek yerde kalır; bu test kuralın sessizce kaymasını engeller.
    #[test]
    fn fast_matcher_agrees_with_reference_implementation() {
        let texts = [
            "Mahkeme, itirazın reddine karar verdi. Esas 2024/0123 E.",
            "ISLAH edilen tutar 1.234,56 TL; Irak'ın ısrarı",
            "Kira  sözleşmesi\n\nfeshedildi",
            "2024 / 123 sayılı karar",
            "2024/123E karar",
            "2024-123 numaralı",
            "ĞÜŞİÖÇ ğüşiöç",
            "İİİ ııı iıi",
            "x123y 00123",
            "   ",
            "",
            "itirazın itiraz itiraz",
        ];
        let needles = [
            "ıslah",
            "ISLAH",
            "itiraz",
            "kira sözleşmesi",
            "2024/123 E.",
            "2024/123",
            "123",
            "ğüş",
            "Islah",
            "i",
        ];
        let mut scratch = MatchScratch::default();
        for mode in [
            SearchMode::Exact,
            SearchMode::Flexible,
            SearchMode::Phrase,
            SearchMode::Docket,
        ] {
            for needle in needles {
                let matcher = SearchMatcher::new(needle, mode, parse_docket(needle));
                for text in texts {
                    let reference = search_match(text, needle, mode.tag());
                    // `flexible` doğrulaması `normalized_text` sütununu kullanır;
                    // testte o sütunun üretimi aynen çağrılır.
                    let normalized = dim_evidence::normalize(text);
                    let fast = matcher.matches(Some(text), Some(&normalized), &mut scratch);
                    assert_eq!(
                        fast, reference,
                        "kip {mode:?}, iğne {needle:?}, metin {text:?}"
                    );
                }
            }
        }
    }

    /// Aday daraltması gerçek eşleşmeleri kaybetmemeli: numara sıfırla,
    /// boşlukla ve harfe yapışık yazılmış olabilir.
    #[test]
    fn search_docket_narrowing_keeps_real_matches() {
        let (db, case_id, version_id) = setup();
        store_texts(
            &db,
            &case_id,
            &version_id,
            &[
                "Esas 2024/0123 E. ile açıldı",
                "2024 / 123 E. sayılı karar",
                "2024/123E numaralı karar",
                "2024/12345 E. başka dosya",
                "2023/123 E. eski yıl",
            ],
        );
        let page = db
            .search_segments_page(
                &case_id,
                "2024/123 E.",
                &SearchFilters::default(),
                20,
                None,
                true,
            )
            .expect("arama");
        assert_eq!(
            page.total,
            Some(3),
            "sıfırlı, boşluklu ve harfe yapışık yazım eşleşmeli; yanlış yıl/numara değil"
        );
        let mut pages: Vec<i64> = page.hits.iter().filter_map(|hit| hit.page_index).collect();
        pages.sort_unstable();
        assert_eq!(pages, vec![0, 1, 2]);
    }

    /// Pencere sayımı ile sayfa aynı sonucu vermeli: ilk sayfada toplam tam,
    /// imleçli sayfada sayım istenmez.
    #[test]
    fn search_total_from_single_pass_matches_verified_hits() {
        let (db, case_id, _version_id) = setup();
        for index in 0..4 {
            add_document_with_text(
                &db,
                &case_id,
                &format!("Belge {index}"),
                "notice",
                "kira bedeli ödenmedi",
                "native",
            );
        }
        // Eşleşmeyen aday: FTS iki kelimeyi de bulur, doğrulama **sırayı** arar.
        add_document_with_text(
            &db,
            &case_id,
            "Ters sıra",
            "notice",
            "bedeli kira olarak ödendi",
            "native",
        );
        let page = db
            .search_segments_page(
                &case_id,
                "kira bedeli",
                &SearchFilters::default(),
                2,
                None,
                true,
            )
            .expect("arama");
        assert_eq!(page.total, Some(4), "sayım doğrulanmış eşleşmeleri sayar");
        assert_eq!(page.returned, 2);
        assert!(page.has_more);
    }

    // -----------------------------------------------------------------------
    // F15 — ertelenmiş toplam sayım (`search_total`)
    // -----------------------------------------------------------------------

    /// Sayfa toplamı ile ayrı sayım **aynı** sayıyı vermeli; ayrı sayım önce
    /// çağrıldığında da (doğrulayıcı kendisi kaydeder) doğru olmalı.
    #[test]
    fn deferred_total_matches_page_total_without_prior_search() {
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
        add_document_with_text(
            &db,
            &case_id,
            "Ters sıra",
            "notice",
            "bedeli kira olarak ödendi",
            "native",
        );

        // Önce ayrı sayım: hiç sayfa araması yapılmadı.
        let total = db
            .search_total(
                &case_id,
                &SearchQuery::auto("kira bedeli"),
                &SearchFilters::default(),
            )
            .expect("sayım");
        assert_eq!(total.total, 5, "doğrulamayı geçen 5 eşleşme");
        assert_eq!(total.scope, "case");
        assert!(total.took_ms >= 0.0);

        let page = db
            .search_segments_page(
                &case_id,
                "kira bedeli",
                &SearchFilters::default(),
                50,
                None,
                true,
            )
            .expect("arama");
        assert_eq!(
            page.total,
            Some(total.total),
            "sayfa toplamı ayrı sayımla aynı"
        );

        // Sayımdan sonra yapılan arama eski doğrulayıcıyı kullanmamalı.
        let other = db
            .search_total(
                &case_id,
                &SearchQuery::auto("kira"),
                &SearchFilters::default(),
            )
            .expect("ikinci sayım");
        assert_eq!(other.total, 6, "tek kelime altı belgede de geçiyor");
    }

    /// Sayım filtreleri sayfayla aynı uygulamalı (etiket = VE).
    #[test]
    fn deferred_total_respects_filters() {
        let (db, case_id, _version_id) = setup();
        let (tagged_doc, _, _, _) = add_document_with_text(
            &db,
            &case_id,
            "Etiketli",
            "notice",
            "kira bedeli ödenmedi",
            "native",
        );
        add_document_with_text(
            &db,
            &case_id,
            "Etiketsiz",
            "notice",
            "kira bedeli ödenmedi",
            "native",
        );
        db.add_document_tag(&tagged_doc, "kritik").expect("etiket");

        let filters = SearchFilters {
            tags: vec!["kritik".to_string()],
            ..Default::default()
        };
        let filtered = db
            .search_total(&case_id, &SearchQuery::auto("kira bedeli"), &filters)
            .expect("filtreli sayım");
        assert_eq!(filtered.total, 1, "yalnız etiketli belge sayılır");
        assert_eq!(filtered.filters.tags, vec!["kritik".to_string()]);

        let unfiltered = db
            .search_total(
                &case_id,
                &SearchQuery::auto("kira bedeli"),
                &SearchFilters::default(),
            )
            .expect("filtresiz sayım");
        assert_eq!(unfiltered.total, 2);

        // Sayfa da aynı filtreyle aynı sayıyı vermeli.
        let page = db
            .search_segments_page(&case_id, "kira bedeli", &filters, 50, None, true)
            .expect("filtreli arama");
        assert_eq!(page.total, Some(filtered.total));
    }

    /// Boş sorgu: hata değil, sıfır — ve sayım sorgusu hiç çalışmaz.
    #[test]
    fn deferred_total_of_blank_query_is_zero() {
        let (db, case_id, _version_id) = setup();
        for query in ["", "   ", "\"\""] {
            let total = db
                .search_total(
                    &case_id,
                    &SearchQuery::auto(query),
                    &SearchFilters::default(),
                )
                .expect("boş sorgu sayımı");
            assert_eq!(total.total, 0, "sorgu {query:?} için sıfır");
            assert_eq!(total.took_ms, 0.0, "boş sorguda sayım yapılmaz");
        }
    }

    /// Sayım sonucu hangi sorguya ait olduğunu taşır (arayüz bayat sayıyı
    /// göstermesin): kip + çözümlenmiş metin sayfayla birebir aynı.
    #[test]
    fn deferred_total_carries_resolved_query_and_mode() {
        let (db, case_id, _version_id) = setup();
        add_document_with_text(
            &db,
            &case_id,
            "Bir",
            "notice",
            "kira bedeli ödenmedi, kira artışı yapıldı",
            "native",
        );
        let query = SearchQuery {
            text: "\"kira bedeli\"".to_string(),
            mode: SearchMode::Auto,
        };
        let total = db
            .search_total(&case_id, &query, &SearchFilters::default())
            .expect("sayım");
        assert_eq!(total.mode, SearchMode::Phrase, "tırnak ifade kipini seçer");
        assert_eq!(total.resolved_query, "kira bedeli");
        assert_eq!(total.query, "\"kira bedeli\"");

        let page = db
            .search_page(&case_id, &query, &SearchFilters::default(), 10, None, true)
            .expect("arama");
        assert_eq!(page.total, Some(total.total));
        assert_eq!(page.resolved_query, total.resolved_query);
        assert_eq!(page.mode, total.mode);
    }
}
