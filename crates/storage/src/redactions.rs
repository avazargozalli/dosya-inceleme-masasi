//! Karartma planları. Plan, çıktının hangi kaynak sürüme ve hangi render
//! sürümüne ait olduğunu sabitler; başarısız doğrulama planı `applied` yapmaz.
//!
//! Plan ve bölgeler **denetim sicili**dir: her maskelemede yazılır. Okuma
//! yolu (`list_redaction_plans` / `list_redaction_regions`) satırı belge adı ve
//! sürüm numarasıyla birlikte verir; avukat "hangi belgede hangi sayfalarda
//! maskeleme yapıldı" sorusunu kasadan yanıtlayabilir.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::outbound_mask::validate_terms;
use crate::parsing::{parse_blocks, SegmentBlock};
use crate::{now_iso, Database, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactionPlanRow {
    pub id: String,
    pub case_id: String,
    pub document_version_id: String,
    pub parse_run_id: Option<String>,
    pub render_hash: String,
    pub coordinate_space: String,
    pub plan_version: i64,
    pub state: String,
    pub reviewer: Option<String>,
    pub output_profile: String,
    pub created_at: String,
    pub updated_at: String,
    /// Belgenin okunabilir adı (`documents.display_name`). Sürüm/belge
    /// bulunamazsa `None`: kimlik dururken **ad uydurulmaz**. `redaction_plans`
    /// satırı sürüme yabancı anahtarla bağlı değil — denetim satırı yaşar.
    pub document_title: Option<String>,
    /// Sürüm numarası (`document_versions.version_no`); bulunamazsa `None`.
    pub version_no: Option<i64>,
    /// Bu plana yazılmış bölge sayısı (tek sorguda alt sorgu).
    pub region_count: i64,
    /// Bu planın ürettiği **çıktı sürümü** (`redaction_plans.output_version_id`).
    ///
    /// Bu sütun eklenmeden önce yazılmış planlarda `None` — bu "çıktı yok"
    /// demek **değildir**, "bağ kayıtlı değil" demektir; arayüz ikisini ayrı
    /// yazar. Çıktı sürümü silinirse bağ `ON DELETE SET NULL` ile çözülür.
    pub output_version_id: Option<String>,
    /// Çıktı sürümünün numarası (`document_versions.version_no`); bağ yoksa
    /// ya da sürüm/satır bulunamazsa `None` (numara uydurulmaz).
    pub output_version_no: Option<i64>,
    /// Çıktı belgesinin adı (`documents.display_name`); bulunamazsa `None`.
    pub output_document_title: Option<String>,
    /// Çıktı bağının **durumu** — bağın yazılıp yazılmadığı ile bugünkü
    /// hâli ayrılır:
    ///
    /// - `none` — bağ **hiç yazılmadı** (v28'den önce yazılmış plan),
    /// - `linked` — bağ yazıldı, çıktı sürümü bugün de kasada,
    /// - `removed` — bağ yazıldı, çıktı **artık çözülemiyor**: sürüm silinmiş
    ///   ya da belge yumuşak-silinmiş.
    ///
    /// `none` ile `removed` ayrılmadan arayüz "kayıtlı değil" deyip silinmiş
    /// çıktıyı gizliyordu; ikisi aynı cümleyi hak etmiyor.
    pub output_link_state: String,
    /// Bağ **yazılırken** kaydedilen çıktı sürümü kimliği
    /// (`redaction_plans.output_version_ref`). Yabancı anahtar **değildir**:
    /// çıktı silinince `output_version_id` NULL'a döner ama iz kalır. `none`
    /// durumunda `None`.
    pub output_version_ref: Option<String>,
    /// Belgenin **güncel** sürüm numarası (`documents.current_version_id`).
    ///
    /// Planın kaynağı güncel sürüm değilse avukat "hangi sürümden sonra
    /// değişti" sorusunu bu satırdan yanıtlar. Sürüm satırı yoksa ya da belge
    /// güncel sürüm taşımıyorsa `None` (numara uydurulmaz).
    pub current_version_no: Option<i64>,
    /// **Yeniden inceleme nedeni** — kararlı kod (F13-T01: "yerleşim
    /// değişince yeniden incelet").
    ///
    /// `None` = planın kaynağı bugünkü belge hâliyle tutuyor. Dolu = plan
    /// yürürlükte (`approved`/`applied`) ama bağlandığı kanıt artık tutmuyor;
    /// sicil bunu yazmadan plan "uygulandı" görünürdü. Planın **durumu
    /// değişmez**: maskeleme geçmişte gerçekten uygulandı — değişen, o
    /// geometrinin bugünkü belgeyi anlatmamasıdır.
    ///
    /// Kodlar:
    /// - `version_missing` — planın sürümü/belgesi kasadan silinmiş,
    /// - `version_superseded` — belgenin daha yeni bir sürümü var,
    /// - `render_changed` — planın render özeti sürümün özetiyle eşleşmiyor,
    /// - `parse_run_newer` — plan bir ayrıştırmaya bağlıyken sürüm yeniden
    ///   ayrıştırıldı (yerleşim değişmiş olabilir).
    ///
    /// Ölçülemeyen kapılar (sayfa döndürmesi, kesişen blok metni) **burada
    /// iddia edilmez** — maske çalışırken yine hatayla kapanır.
    pub reinspect_reason: Option<String>,
}

/// Planın bölge satırı — **okuma** modeli. `quads` alanı yazılırken JSON
/// dizisi olarak saklanır; bozuk JSON `None` döner (geometri uydurulmaz),
/// satır yine listelenir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactionRegionRow {
    pub id: String,
    pub page_index: i64,
    pub rects: Option<Vec<[f64; 4]>>,
    pub coordinate_space: String,
    pub category: String,
    pub detected_text_hash: Option<String>,
    pub user_decision: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRedactionPlan {
    pub document_version_id: String,
    #[serde(default)]
    pub parse_run_id: Option<String>,
    pub render_hash: String,
    pub output_profile: String,
    /// Maskelemenin ürettiği sürüm. Ürün yolu (`commit_redaction`) her zaman
    /// doldurur; bağ **doğrulanmadan** yazılmaz: çıktı, kaynak sürümün alt
    /// sürümü olmalı ve aynı dosyaya ait olmalıdır. `None` yalnız bu sütundan
    /// önce yazılmış planları temsil eder.
    #[serde(default)]
    pub output_version_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RedactionRegionInput {
    pub page_index: i64,
    pub rect: [f64; 4],
    pub term_hash: String,
}

impl Database {
    /// Maskelemenin ürettiği **çıktı sürümleri**. Dışa aktarma kaydı bir sürümün
    /// maskeli kopya olup olmadığını belge adından değil bu izden **ölçer**
    /// (`export_items.redaction_state`). Bağ `output_version_id`den okunur;
    /// çıktısı silinmiş planlar zaten `NULL` taşır ve o sürüm yoktur.
    pub fn masked_output_version_ids(&self, case_id: &str) -> Result<Vec<String>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT DISTINCT output_version_id FROM redaction_plans
             WHERE case_id=?1 AND output_version_id IS NOT NULL",
        )?;
        let rows = stmt.query_map([case_id], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn create_redaction_plan(
        &self,
        case_id: &str,
        new: &NewRedactionPlan,
    ) -> Result<RedactionPlanRow, StorageError> {
        let version = self
            .get_version(&new.document_version_id)?
            .ok_or(StorageError::NotFound)?;
        if version.case_id != case_id {
            return Err(StorageError::NotFound);
        }
        // Çıktı bağı **doğrulanmadan** yazılmaz: plan, kendi ürettiği sürümü
        // göstermeli. Çıktı kaynak sürümün alt sürümü değilse (ya da başka
        // dosyaya aitse) sicile yanlış iz düşerdi — izi hiç yazmamak, yanlış
        // yazmaktan iyidir.
        if let Some(output_id) = new.output_version_id.as_deref() {
            if output_id == new.document_version_id {
                return Err(StorageError::RedactionPlanInvalid {
                    reason: "çıktı sürümü kaynak sürümün kendisi olamaz".into(),
                });
            }
            let output =
                self.get_version(output_id)?
                    .ok_or_else(|| StorageError::RedactionPlanInvalid {
                        reason: format!("çıktı sürümü bulunamadı: {output_id}"),
                    })?;
            if output.case_id != case_id {
                return Err(StorageError::RedactionPlanInvalid {
                    reason: "çıktı sürümü başka dosyaya ait".into(),
                });
            }
            let parent: Option<String> = self.conn().query_row(
                "SELECT parent_version_id FROM document_versions WHERE id=?1",
                [output_id],
                |row| row.get(0),
            )?;
            if parent.as_deref() != Some(new.document_version_id.as_str()) {
                return Err(StorageError::RedactionPlanInvalid {
                    reason: "çıktı sürümü kaynak sürümün alt sürümü değil".into(),
                });
            }
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO redaction_plans (id,case_id,document_version_id,parse_run_id,render_hash,coordinate_space,plan_version,state,output_profile,created_at,updated_at,output_version_id,output_version_ref)
             VALUES (?1,?2,?3,?4,?5,'pdf_page_points',1,'draft',?6,?7,?7,?8,?8)",
            rusqlite::params![
                id,
                case_id,
                new.document_version_id,
                new.parse_run_id,
                new.render_hash,
                new.output_profile,
                now,
                new.output_version_id
            ],
        )?;
        self.append_audit(
            "redaction_plan",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({
                "caseId": case_id,
                // Bağın **varlığı** denetime yazılır; çıktı kimliği plan
                // satırındadır (denetim kaydı kimlik taşımaz).
                "outputLinked": new.output_version_id.is_some(),
            }),
        )?;
        self.get_redaction_plan(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn add_redaction_regions(
        &self,
        plan_id: &str,
        regions: &[RedactionRegionInput],
    ) -> Result<usize, StorageError> {
        if self.get_redaction_plan(plan_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let tx = self.conn().unchecked_transaction()?;
        for region in regions {
            tx.execute(
                "INSERT INTO redaction_regions (id,plan_id,page_index,quads,coordinate_space,category,detected_text_hash,user_decision)
                 VALUES (?1,?2,?3,?4,'pdf_page_points','user_term',?5,'redact')",
                rusqlite::params![
                    uuid::Uuid::now_v7().to_string(),
                    plan_id,
                    region.page_index,
                    serde_json::to_string(&vec![region.rect]).unwrap_or_else(|_| "[]".into()),
                    region.term_hash
                ],
            )?;
        }
        tx.execute(
            "UPDATE redaction_plans SET state='applied', updated_at=?1 WHERE id=?2",
            rusqlite::params![now_iso(), plan_id],
        )?;
        tx.commit()?;
        self.append_audit(
            "redaction_plan",
            Some(plan_id),
            "applied",
            "local-worker",
            &serde_json::json!({"regions": regions.len()}),
        )?;
        Ok(regions.len())
    }

    pub fn get_redaction_plan(&self, id: &str) -> Result<Option<RedactionPlanRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT p.id,p.case_id,p.document_version_id,p.parse_run_id,p.render_hash,p.coordinate_space,p.plan_version,p.state,p.reviewer,p.output_profile,p.created_at,p.updated_at,
                    d.display_name,v.version_no,
                    (SELECT COUNT(*) FROM redaction_regions rr WHERE rr.plan_id=p.id),
                    p.output_version_id,ov.id,ov.version_no,od.display_name,
                    p.output_version_ref,od.deleted_at,
                    cv.version_no,
                    CASE
                      WHEN p.state NOT IN ('approved','applied') THEN NULL
                      WHEN v.id IS NULL OR d.id IS NULL OR d.deleted_at IS NOT NULL THEN 'version_missing'
                      WHEN cv.version_id IS NULL OR cv.version_id <> p.document_version_id THEN 'version_superseded'
                      WHEN v.original_sha256 <> p.render_hash THEN 'render_changed'
                      WHEN p.parse_run_id IS NOT NULL AND p.parse_run_id <> COALESCE((
                          SELECT pr.id FROM parse_runs pr
                           WHERE pr.document_version_id = p.document_version_id
                             AND EXISTS (SELECT 1 FROM pages pg WHERE pg.parse_run_id = pr.id)
                           ORDER BY pr.created_at DESC, pr.id DESC LIMIT 1
                      ), '') THEN 'parse_run_newer'
                      ELSE NULL
                    END
             FROM redaction_plans p
             LEFT JOIN document_versions v ON v.id = p.document_version_id
             LEFT JOIN documents d ON d.id = v.document_id
             LEFT JOIN current_document_versions cv ON cv.document_id = d.id
             LEFT JOIN document_versions ov ON ov.id = p.output_version_id
             LEFT JOIN documents od ON od.id = ov.document_id
             WHERE p.id=?1",
        )?;
        Ok(stmt.query_row([id], map_plan).optional()?)
    }

    /// Dosyanın maskeleme planları — en yeni önce. Sıra `created_at DESC,
    /// rowid DESC`: aynı damgaya düşen iki plan bile yazılma sırasını korur.
    pub fn list_redaction_plans(
        &self,
        case_id: &str,
    ) -> Result<Vec<RedactionPlanRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT p.id,p.case_id,p.document_version_id,p.parse_run_id,p.render_hash,p.coordinate_space,p.plan_version,p.state,p.reviewer,p.output_profile,p.created_at,p.updated_at,
                    d.display_name,v.version_no,
                    (SELECT COUNT(*) FROM redaction_regions rr WHERE rr.plan_id=p.id),
                    p.output_version_id,ov.id,ov.version_no,od.display_name,
                    p.output_version_ref,od.deleted_at,
                    cv.version_no,
                    CASE
                      WHEN p.state NOT IN ('approved','applied') THEN NULL
                      WHEN v.id IS NULL OR d.id IS NULL OR d.deleted_at IS NOT NULL THEN 'version_missing'
                      WHEN cv.version_id IS NULL OR cv.version_id <> p.document_version_id THEN 'version_superseded'
                      WHEN v.original_sha256 <> p.render_hash THEN 'render_changed'
                      WHEN p.parse_run_id IS NOT NULL AND p.parse_run_id <> COALESCE((
                          SELECT pr.id FROM parse_runs pr
                           WHERE pr.document_version_id = p.document_version_id
                             AND EXISTS (SELECT 1 FROM pages pg WHERE pg.parse_run_id = pr.id)
                           ORDER BY pr.created_at DESC, pr.id DESC LIMIT 1
                      ), '') THEN 'parse_run_newer'
                      ELSE NULL
                    END
             FROM redaction_plans p
             LEFT JOIN document_versions v ON v.id = p.document_version_id
             LEFT JOIN documents d ON d.id = v.document_id
             LEFT JOIN current_document_versions cv ON cv.document_id = d.id
             LEFT JOIN document_versions ov ON ov.id = p.output_version_id
             LEFT JOIN documents od ON od.id = ov.document_id
             WHERE p.case_id=?1 ORDER BY p.created_at DESC, p.rowid DESC",
        )?;
        let rows = stmt.query_map([case_id], map_plan)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Planın bölgeleri — **yazılma sırası** (`rowid ASC`): aynı sayfadaki
    /// bölgeler bile seçildikleri sırayı korur.
    pub fn list_redaction_regions(
        &self,
        plan_id: &str,
    ) -> Result<Vec<RedactionRegionRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,page_index,quads,coordinate_space,category,detected_text_hash,user_decision
             FROM redaction_regions WHERE plan_id=?1 ORDER BY rowid ASC",
        )?;
        let rows = stmt.query_map([plan_id], |row| {
            let quads: String = row.get(2)?;
            Ok(RedactionRegionRow {
                id: row.get(0)?,
                page_index: row.get(1)?,
                // Bozuk `quads` **gizlenmez**: `None` "geometri okunamadı"
                // demektir; bölge satırı yine listelenir (sayım kaybolmaz).
                rects: serde_json::from_str::<Vec<[f64; 4]>>(&quads).ok(),
                coordinate_space: row.get(3)?,
                category: row.get(4)?,
                detected_text_hash: row.get(5)?,
                user_decision: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

/// Karartma planından türetilen gönderim maskesi.
///
/// Terimler **saklanmaz**: planda yalnız bölge geometrisi ve terim özeti
/// (hash) vardır. Metin, planın bağlı olduğu sürümün güncel ayrıştırmasındaki
/// bloklardan, bölgeyle kesişen kutular üzerinden çıkarılır — bu yüzden dönen
/// liste yalnız çağrı süresince yaşar ve kasaya yazılmaz.
#[derive(Debug, Clone)]
pub struct PlanMaskTerms {
    pub plan_id: String,
    pub document_version_id: String,
    pub terms: Vec<String>,
    pub regions_total: usize,
    pub regions_used: usize,
    pub blocks: usize,
    pub short_skipped: usize,
}

struct PlanPageText {
    page_index: i64,
    rotation: i64,
    coordinate_space: Option<String>,
    segments: Vec<(String, Vec<SegmentBlock>)>,
}

/// İki kutu kesişiyor mu? **Kenara değmek de kesişmedir**: sıfır alanlı
/// boşluk daha çok yuvarlama artığıdır; eksik maske (ad sızması) fazla
/// maskelemekten daha ağır olduğu için sınırda kutu da alınır.
fn rects_overlap(first: [f64; 4], second: [f64; 4]) -> bool {
    let (ax0, ax1) = (first[0].min(first[2]), first[0].max(first[2]));
    let (ay0, ay1) = (first[1].min(first[3]), first[1].max(first[3]));
    let (bx0, bx1) = (second[0].min(second[2]), second[0].max(second[2]));
    let (by0, by1) = (second[1].min(second[3]), second[1].max(second[3]));
    ax0 <= bx1 && ax1 >= bx0 && ay0 <= by1 && ay1 >= by0
}

/// Plan bölgeleri `pdf_page_points` uzayında yazılır; ayrıştırma sayfaları
/// aynı PDF noktası uzayını `unrotated_cropbox_points` adıyla taşır.
/// `image_pixels`/`structure` **çevrilemez** (ölçek saklanmıyor), bu yüzden
/// kabul edilmez — tahminle dönüşüm yapılmaz.
fn plan_space_matches_page(page_space: Option<&str>) -> bool {
    matches!(page_space, Some("unrotated_cropbox_points"))
}

/// Bölgeyle kesişen blokların metnini terimlere çevirir.
///
/// Kontrol karakteri taşıyan parçalar (ör. blok içi satır sonu) ayrı satırlara
/// bölünür; her parça kırpılır. 1 karakterlik parçalar maskelenemez
/// (`validate_terms` alt sınırı 2) ve **sessizce** kaybolmaz: sayılır.
fn block_terms(
    chars: &[char],
    blocks: &[SegmentBlock],
    rects: &[[f64; 4]],
    blocks_hit: &mut usize,
    short_skipped: &mut usize,
) -> Vec<String> {
    let mut out = Vec::new();
    for block in blocks {
        let Some(bbox) = block.bbox else { continue };
        if !rects.iter().any(|rect| rects_overlap(bbox, *rect)) {
            continue;
        }
        *blocks_hit += 1;
        if block.start < 0 || block.end <= block.start {
            continue;
        }
        let start = usize::try_from(block.start).unwrap_or(usize::MAX);
        let end = usize::try_from(block.end)
            .unwrap_or(usize::MAX)
            .min(chars.len());
        if start >= end {
            continue;
        }
        let text: String = chars[start..end].iter().collect();
        for piece in text.split('\n') {
            let trimmed = piece.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.chars().any(char::is_control) {
                *short_skipped += 1;
                continue;
            }
            let length = trimmed.chars().count();
            if (2..=200).contains(&length) {
                out.push(trimmed.to_string());
            } else {
                *short_skipped += 1;
            }
        }
    }
    out
}

impl Database {
    /// Karartma planının `redact` bölgelerinden **gönderim maskesi terimleri**
    /// üretir. Terimler kasaya yazılmaz; çağıran maskeyi çekirdekte uygular.
    ///
    /// Kapılar (hepsi hatayla kapanır — sessiz eksik maske yok):
    /// - plan bu dosyaya ve **güncel** sürüme ait olmalı,
    /// - planın bağlandığı render özeti sürümün özetiyle aynı olmalı,
    /// - koordinat uzayı PDF noktası olmalı, sayfa döndürmesi 0 olmalı,
    /// - her `redact` bölgesi en az bir kullanılabilir terim üretmeli;
    ///   geometrisi okunamayan/kesişen metni olmayan bölge **maskesiz kalmaz**.
    pub fn outbound_terms_from_plan(
        &self,
        case_id: &str,
        plan_id: &str,
    ) -> Result<PlanMaskTerms, StorageError> {
        let plan = self
            .get_redaction_plan(plan_id)?
            .ok_or(StorageError::NotFound)?;
        if plan.case_id != case_id {
            return Err(StorageError::NotFound);
        }
        let version = self
            .get_version(&plan.document_version_id)?
            .ok_or(StorageError::NotFound)?;
        if version.case_id != case_id {
            return Err(StorageError::NotFound);
        }
        if plan.render_hash != version.original_sha256 {
            return Err(StorageError::PlanMaskInvalid {
                reason:
                    "plan başka bir dosya sürümüne ait (render özeti eşleşmiyor); maske uygulanmadı"
                        .into(),
            });
        }
        let current: Option<String> = self
            .conn()
            .query_row(
                "SELECT (SELECT cv.version_id FROM current_document_versions cv
                          WHERE cv.document_id=d.id)
                 FROM documents d
                 WHERE d.id=?1 AND d.case_id=?2 AND d.deleted_at IS NULL",
                rusqlite::params![version.document_id, case_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        if current.as_deref() != Some(plan.document_version_id.as_str()) {
            return Err(StorageError::PlanMaskInvalid {
                reason:
                    "plan güncel sürüme ait değil; bölge geometrisi gönderilecek metinle eşleşmez"
                        .into(),
            });
        }
        if plan.coordinate_space != "pdf_page_points" {
            return Err(StorageError::PlanMaskInvalid {
                reason: format!(
                    "plan koordinat uzayı «{}»; gönderim maskesi yalnız PDF noktası uzayında çözülür",
                    plan.coordinate_space
                ),
            });
        }
        let latest_run: Option<String> = self
            .conn()
            .query_row(
                "SELECT pr.id FROM parse_runs pr
                 WHERE pr.document_version_id=?1
                   AND EXISTS (SELECT 1 FROM pages p WHERE p.parse_run_id=pr.id)
                 ORDER BY pr.created_at DESC, pr.id DESC LIMIT 1",
                [&plan.document_version_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(run_id) = latest_run else {
            return Err(StorageError::PlanMaskInvalid {
                reason: "planın sürümü ayrıştırılmamış; bölge metni çıkarılamaz".into(),
            });
        };
        if let Some(declared) = plan.parse_run_id.as_deref() {
            if declared != run_id {
                return Err(StorageError::PlanMaskInvalid {
                    reason:
                        "plan eski bir ayrıştırmaya ait; bölge geometrisi güncel metinle eşleşmiyor"
                            .into(),
                });
            }
        }
        let mut stmt = self.conn().prepare(
            "SELECT p.page_index,p.rotation,p.coordinate_space,s.raw_text,s.bbox_list
             FROM text_segments s JOIN pages p ON p.id=s.page_id
             WHERE p.parse_run_id=?1
             ORDER BY p.page_index, s.rowid",
        )?;
        let rows = stmt.query_map([&run_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;
        let mut pages: Vec<PlanPageText> = Vec::new();
        for row in rows {
            let (page_index, rotation, coordinate_space, raw, bbox_raw) = row?;
            let blocks = parse_blocks(bbox_raw);
            match pages.last_mut() {
                Some(page) if page.page_index == page_index => {
                    page.segments.push((raw, blocks));
                }
                _ => pages.push(PlanPageText {
                    page_index,
                    rotation: rotation.unwrap_or(0),
                    coordinate_space,
                    segments: vec![(raw, blocks)],
                }),
            }
        }
        if pages.is_empty() {
            return Err(StorageError::PlanMaskInvalid {
                reason: "planın sürümünde ayrıştırılmış metin yok; bölge metni çıkarılamaz".into(),
            });
        }

        let regions = self.list_redaction_regions(plan_id)?;
        let redact: Vec<&RedactionRegionRow> = regions
            .iter()
            .filter(|region| region.user_decision == "redact")
            .collect();
        if redact.is_empty() {
            return Err(StorageError::PlanMaskInvalid {
                reason: "planda karartılacak bölge yok".into(),
            });
        }

        let mut terms: Vec<String> = Vec::new();
        let mut regions_used = 0usize;
        let mut blocks_hit = 0usize;
        let mut short_skipped = 0usize;
        for (index, region) in redact.iter().enumerate() {
            let Some(page) = pages
                .iter()
                .find(|page| page.page_index == region.page_index)
            else {
                return Err(StorageError::PlanMaskInvalid {
                    reason: format!(
                        "bölge {}: sayfa {} ayrıştırmada yok",
                        index + 1,
                        region.page_index
                    ),
                });
            };
            if page.rotation % 360 != 0 {
                return Err(StorageError::PlanMaskInvalid {
                    reason: format!(
                        "bölge {}: sayfa {} döndürülmüş ({}°); bölge kutusu döndürme öncesi uzayda çözülemiyor",
                        index + 1,
                        region.page_index,
                        page.rotation
                    ),
                });
            }
            if !plan_space_matches_page(page.coordinate_space.as_deref()) {
                return Err(StorageError::PlanMaskInvalid {
                    reason: format!(
                        "bölge {}: sayfa {} koordinat uzayı «{}»; PDF noktası uzayı değil",
                        index + 1,
                        region.page_index,
                        page.coordinate_space.as_deref().unwrap_or("bilinmiyor")
                    ),
                });
            }
            let Some(rects) = region.rects.as_ref() else {
                return Err(StorageError::PlanMaskInvalid {
                    reason: format!("bölge {}: geometri okunamadı", index + 1),
                });
            };
            if rects.is_empty() {
                return Err(StorageError::PlanMaskInvalid {
                    reason: format!("bölge {}: boş geometri", index + 1),
                });
            }
            let mut region_terms = Vec::new();
            for (raw, blocks) in &page.segments {
                let chars: Vec<char> = raw.chars().collect();
                region_terms.extend(block_terms(
                    &chars,
                    blocks,
                    rects,
                    &mut blocks_hit,
                    &mut short_skipped,
                ));
            }
            if region_terms.is_empty() {
                return Err(StorageError::PlanMaskInvalid {
                    reason: format!(
                        "bölge {} (sayfa {}): kesişen metin bloğu yok; maske kanıtlanamaz",
                        index + 1,
                        region.page_index
                    ),
                });
            }
            regions_used += 1;
            terms.extend(region_terms);
        }
        let terms = validate_terms(&terms).map_err(|error| StorageError::PlanMaskInvalid {
            reason: format!("plan terimleri maskeye uygun değil: {}", error.code()),
        })?;
        Ok(PlanMaskTerms {
            plan_id: plan.id,
            document_version_id: plan.document_version_id,
            terms,
            regions_total: redact.len(),
            regions_used,
            blocks: blocks_hit,
            short_skipped,
        })
    }
}

fn map_plan(row: &Row<'_>) -> rusqlite::Result<RedactionPlanRow> {
    Ok(RedactionPlanRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        document_version_id: row.get(2)?,
        parse_run_id: row.get(3)?,
        render_hash: row.get(4)?,
        coordinate_space: row.get(5)?,
        plan_version: row.get(6)?,
        state: row.get(7)?,
        reviewer: row.get(8)?,
        output_profile: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        document_title: row.get(12)?,
        version_no: row.get(13)?,
        region_count: row.get(14)?,
        output_version_id: row.get(15)?,
        output_version_no: row.get(17)?,
        output_document_title: row.get(18)?,
        output_link_state: {
            // Bağın bugünkü hâli **ölçülür**, varsayılmaz: `linked` için
            // çıktı sürümünün satırı gerçekten okunmuş ve belgesi
            // yumuşak-silinmemiş olmalı. Aksi hâlde bağ yazılmışsa `removed`,
            // hiç yazılmamışsa `none`.
            let version_row: Option<String> = row.get(16)?;
            let document_deleted: Option<String> = row.get(20)?;
            let recorded: Option<String> = row.get(19)?;
            if version_row.is_some() && document_deleted.is_none() {
                "linked"
            } else if recorded.is_some() || row.get::<_, Option<String>>(15)?.is_some() {
                "removed"
            } else {
                "none"
            }
            .to_string()
        },
        output_version_ref: row.get(19)?,
        current_version_no: row.get(21)?,
        reinspect_reason: row.get(22)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn setup() -> (Database, String, String) {
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
        let blob = db.create_blob_record("st-redact", 10, "aad").expect("blob");
        let doc = db
            .create_document(&case.id, "Belge", "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &doc.id,
                &case.id,
                &blob,
                &"a".repeat(64),
                10,
                "application/pdf",
            )
            .expect("v");
        (db, case.id, version.id)
    }

    /// Dışa aktarma kaydı bir sürümün maskeli kopya olup olmadığını **izden**
    /// ölçer (`redaction_plans.output_version_id`), belge adına bakmaz.
    #[test]
    fn masked_output_version_ids_reads_plan_link() {
        let (db, case_id, source_version) = setup();
        // Plan yokken hiçbir sürüm maskeli değil — "belki" denmez.
        assert!(db
            .masked_output_version_ids(&case_id)
            .expect("liste")
            .is_empty());
        let blob = db
            .create_blob_record("st-masked-out", 10, "aad")
            .expect("blob");
        let document_id: String = db
            .conn()
            .query_row(
                "SELECT document_id FROM document_versions WHERE id=?1",
                [&source_version],
                |row| row.get(0),
            )
            .expect("belge");
        let output = db
            .add_version_with_parent(
                &document_id,
                &case_id,
                &blob,
                &"b".repeat(64),
                10,
                "application/pdf",
                Some(&source_version),
            )
            .expect("çıktı");
        db.create_redaction_plan(
            &case_id,
            &NewRedactionPlan {
                document_version_id: source_version.clone(),
                parse_run_id: None,
                render_hash: "a".repeat(64),
                output_profile: "masked_pdf".into(),
                output_version_id: Some(output.id.clone()),
            },
        )
        .expect("plan");
        assert_eq!(
            db.masked_output_version_ids(&case_id).expect("liste"),
            vec![output.id.clone()]
        );
        // Başka dosya bu izi görmez: kapsam dosyaya bağlıdır.
        let other = db
            .create_case(&crate::cases::NewCase {
                title: "Başka".into(),
                case_kind: crate::cases::CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("dosya");
        assert!(db
            .masked_output_version_ids(&other.id)
            .expect("liste")
            .is_empty());
    }

    #[test]
    fn plan_lifecycle_and_region_count() {
        let (db, case_id, version_id) = setup();
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id,
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        assert_eq!(plan.state, "draft");

        let regions = vec![
            RedactionRegionInput {
                page_index: 0,
                rect: [1.0, 2.0, 3.0, 4.0],
                term_hash: "h1".into(),
            },
            RedactionRegionInput {
                page_index: 1,
                rect: [5.0, 6.0, 7.0, 8.0],
                term_hash: "h2".into(),
            },
        ];
        assert_eq!(
            db.add_redaction_regions(&plan.id, &regions).expect("bölge"),
            2
        );
        let applied = db.get_redaction_plan(&plan.id).expect("get").expect("var");
        assert_eq!(applied.state, "applied");
        assert_eq!(db.list_redaction_plans(&case_id).expect("liste").len(), 1);
    }

    #[test]
    fn plan_list_carries_document_title_version_and_region_count() {
        let (db, case_id, version_id) = setup();
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id,
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        // Bölgeler yazılmadan önce plan satırı da okunabilir; sayı 0'dır.
        let before = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].region_count, 0);
        assert_eq!(before[0].document_title.as_deref(), Some("Belge"));
        assert_eq!(before[0].version_no, Some(1));

        db.add_redaction_regions(
            &plan.id,
            &[
                RedactionRegionInput {
                    page_index: 0,
                    rect: [1.0, 2.0, 3.0, 4.0],
                    term_hash: "h1".into(),
                },
                RedactionRegionInput {
                    page_index: 2,
                    rect: [5.0, 6.0, 7.0, 8.0],
                    term_hash: "h2".into(),
                },
            ],
        )
        .expect("bölge");

        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows.len(), 1);
        // Ad/sürüm **sorgudan** okunur, satırda saklanmaz: plan hangi belgeye
        // aitse onu gösterir.
        assert_eq!(rows[0].document_title.as_deref(), Some("Belge"));
        assert_eq!(rows[0].version_no, Some(1));
        assert_eq!(rows[0].region_count, 2);
        assert_eq!(rows[0].state, "applied");
    }

    #[test]
    fn plan_list_keeps_row_when_version_is_missing() {
        let (db, case_id, version_id) = setup();
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id,
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        // Sürüm bağı yabancı anahtar değil: satır silinse/taşınsa bile denetim
        // kaydı **düşmez**, ad uydurulmaz (None).
        //
        // v31 kapsam kapısı bu durumu **artık üretmez** (plan, var olmayan bir
        // sürüme bağlanamaz). Test, eski/bozuk bir kasadan gelebilecek satırın
        // görüntüsünü ölçtüğü için kapılar burada bilinçli düşürülür (v51 kanıt
        // kapısı da sürüm bağını dondurur).
        db.conn()
            .execute_batch(
                "DROP TRIGGER scope_guard_redaction_plans_bu;
                 DROP TRIGGER redaction_plan_evidence_guard_bu;",
            )
            .expect("kapı düşür");
        db.conn()
            .execute(
                "UPDATE redaction_plans SET document_version_id='yok' WHERE id=?1",
                [&plan.id],
            )
            .expect("güncelle");
        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].document_title.is_none());
        assert!(rows[0].version_no.is_none());
        assert_eq!(rows[0].region_count, 0);
    }

    #[test]
    fn plans_with_equal_timestamp_keep_insertion_order() {
        let (db, case_id, version_id) = setup();
        let mut ids = Vec::new();
        for _ in 0..2 {
            ids.push(
                db.create_redaction_plan(
                    &case_id,
                    &NewRedactionPlan {
                        document_version_id: version_id.clone(),
                        parse_run_id: None,
                        render_hash: "a".repeat(64),
                        output_profile: "share".into(),
                        output_version_id: None,
                    },
                )
                .expect("plan")
                .id,
            );
        }
        // Damga eşitlenince sıra tek başına `rowid`e düşer: sonra yazılan üstte.
        //
        // `created_at` üretimde **donar** (v51): eşitleme bir test kurgusudur,
        // sıra okumasını sınamak için kapı bilinçli düşürülür.
        db.conn()
            .execute_batch("DROP TRIGGER redaction_plan_evidence_guard_bu;")
            .expect("kapı düşür");
        db.conn()
            .execute(
                "UPDATE redaction_plans SET created_at='2026-01-01T00:00:00.000000Z' \
                 WHERE case_id=?1",
                [&case_id],
            )
            .expect("damga");
        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, ids[1]);
        assert_eq!(rows[1].id, ids[0]);
    }

    #[test]
    fn regions_list_in_insertion_order_and_bad_quads_are_reported() {
        let (db, case_id, version_id) = setup();
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id,
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        db.add_redaction_regions(
            &plan.id,
            &[
                RedactionRegionInput {
                    page_index: 3,
                    rect: [1.0, 2.0, 3.0, 4.0],
                    term_hash: "h1".into(),
                },
                RedactionRegionInput {
                    page_index: 1,
                    rect: [5.0, 6.0, 7.0, 8.0],
                    term_hash: "h2".into(),
                },
                RedactionRegionInput {
                    page_index: 3,
                    rect: [9.0, 10.0, 11.0, 12.0],
                    term_hash: "h3".into(),
                },
            ],
        )
        .expect("bölge");

        let rows = db.list_redaction_regions(&plan.id).expect("bölgeler");
        // Sıra **yazılma** sırasıdır (sayfaya göre sıralanmaz): seçim sırası
        // denetim bilgisidir.
        assert_eq!(
            rows.iter().map(|r| r.page_index).collect::<Vec<_>>(),
            vec![3, 1, 3]
        );
        assert_eq!(rows[0].rects.as_deref(), Some(&[[1.0, 2.0, 3.0, 4.0]][..]));
        assert_eq!(rows[0].user_decision, "redact");
        assert_eq!(rows[0].category, "user_term");
        assert_eq!(rows[0].coordinate_space, "pdf_page_points");

        // Bozuk geometri gizlenmez: satır durur, `rects` `None` olur.
        //
        // v50 kapısı böyle bir satırı **doğuşta** reddeder ve sonradan
        // değiştirmeyi dondurur. Buradaki amaç **okuma** davranışını ölçmek
        // (v50 öncesi kasadan gelmiş bozuk satırın simülasyonu), o yüzden
        // değişmezlik tetikleyicisi bilinçli olarak düşürülür; test kendi
        // bellek içi kasasında koşar, başka testi etkilemez.
        db.conn()
            .execute_batch(
                "DROP TRIGGER redaction_region_evidence_guard_bu;
                 UPDATE redaction_regions SET quads='bozuk' WHERE page_index=1;",
            )
            .expect("boz");
        let rows = db.list_redaction_regions(&plan.id).expect("bölgeler");
        assert_eq!(rows.len(), 3);
        assert!(rows[1].rects.is_none());
        assert!(rows[0].rects.is_some());
    }

    #[test]
    fn cross_case_version_rejected() {
        let (db, _case_id, version_id) = setup();
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
            .create_redaction_plan(
                &other.id,
                &NewRedactionPlan {
                    document_version_id: version_id,
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_NOT_FOUND");
    }

    fn block(start: i64, end: i64, bbox: [f64; 4]) -> SegmentBlock {
        SegmentBlock {
            start,
            end,
            bbox: Some(bbox),
            ..Default::default()
        }
    }

    fn seed_page(
        db: &Database,
        case_id: &str,
        version_id: &str,
        text: &str,
        blocks: Vec<SegmentBlock>,
        rotation: i64,
        coordinate_space: Option<&str>,
    ) {
        let run = db
            .start_parse_run(version_id, case_id, "text", "1", "{}")
            .expect("run");
        db.store_pages(
            &run.id,
            case_id,
            &[crate::parsing::NewPage {
                index: 0,
                text: text.to_string(),
                width: Some(595.0),
                height: Some(842.0),
                origin: "native".into(),
                warnings: Vec::new(),
                rotation,
                coordinate_space: coordinate_space.map(str::to_string),
                page_label: None,
                blocks,
            }],
        )
        .expect("pages");
    }

    fn plan_with_regions(
        db: &Database,
        case_id: &str,
        version_id: &str,
        regions: &[(i64, [f64; 4])],
    ) -> RedactionPlanRow {
        let version = db.get_version(version_id).expect("v").expect("var");
        let plan = db
            .create_redaction_plan(
                case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.to_string(),
                    parse_run_id: None,
                    render_hash: version.original_sha256,
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        let inputs: Vec<RedactionRegionInput> = regions
            .iter()
            .map(|(page_index, rect)| RedactionRegionInput {
                page_index: *page_index,
                rect: *rect,
                term_hash: "h".into(),
            })
            .collect();
        db.add_redaction_regions(&plan.id, &inputs)
            .expect("regions");
        plan
    }

    const THREE_LINES: &str = "Ahmet Yılmaz\nkira bedeli\nİstanbul";

    #[test]
    fn plan_terms_come_from_blocks_inside_regions() {
        let (db, case_id, version_id) = setup();
        seed_page(
            &db,
            &case_id,
            &version_id,
            THREE_LINES,
            vec![
                block(0, 12, [10.0, 10.0, 200.0, 30.0]),
                block(13, 24, [10.0, 40.0, 200.0, 60.0]),
                block(25, 33, [10.0, 70.0, 80.0, 90.0]),
            ],
            0,
            Some("unrotated_cropbox_points"),
        );
        // Bölge yalnız ilk satırı kapsıyor: komşu satır maskelenmez.
        let plan = plan_with_regions(&db, &case_id, &version_id, &[(0, [5.0, 5.0, 210.0, 35.0])]);
        let mask = db
            .outbound_terms_from_plan(&case_id, &plan.id)
            .expect("mask");
        assert_eq!(mask.terms, vec!["Ahmet Yılmaz".to_string()]);
        assert_eq!(mask.regions_total, 1);
        assert_eq!(mask.regions_used, 1);
        assert_eq!(mask.blocks, 1);
        assert_eq!(mask.short_skipped, 0);
        assert_eq!(mask.document_version_id, version_id);
        // Terim gerçekten gönderilecek metni maskeler.
        let masked = crate::outbound_mask::mask_terms("Ahmet Yılmaz ödedi.", &mask.terms)
            .expect("mask_terms");
        assert!(!masked.text.contains("Ahmet"));
        assert!(masked.text.contains(crate::outbound_mask::MASK_PLACEHOLDER));
    }

    #[test]
    fn overlapping_regions_and_duplicate_blocks_are_deduplicated() {
        let (db, case_id, version_id) = setup();
        seed_page(
            &db,
            &case_id,
            &version_id,
            THREE_LINES,
            vec![
                block(0, 12, [10.0, 10.0, 200.0, 30.0]),
                block(13, 24, [10.0, 40.0, 200.0, 60.0]),
                block(25, 33, [10.0, 70.0, 80.0, 90.0]),
            ],
            0,
            Some("unrotated_cropbox_points"),
        );
        // İki bölge de ilk iki satırı kapsıyor; terimler tekilleşir.
        let plan = plan_with_regions(
            &db,
            &case_id,
            &version_id,
            &[(0, [0.0, 0.0, 300.0, 65.0]), (0, [0.0, 0.0, 300.0, 65.0])],
        );
        let mask = db
            .outbound_terms_from_plan(&case_id, &plan.id)
            .expect("mask");
        assert_eq!(
            mask.terms,
            vec!["Ahmet Yılmaz".to_string(), "kira bedeli".to_string()]
        );
        assert_eq!(mask.regions_total, 2);
        assert_eq!(mask.regions_used, 2);
        // Aynı blok iki bölgede de sayılır (blok sayısı ölçüm dürüstlüğü).
        assert_eq!(mask.blocks, 4);
    }

    #[test]
    fn short_fragments_are_counted_not_silently_dropped() {
        let (db, case_id, version_id) = setup();
        let text = "·\nAhmet Yılmaz";
        seed_page(
            &db,
            &case_id,
            &version_id,
            text,
            vec![
                block(0, 1, [10.0, 10.0, 20.0, 30.0]),
                block(2, 14, [10.0, 40.0, 200.0, 60.0]),
            ],
            0,
            Some("unrotated_cropbox_points"),
        );
        let plan = plan_with_regions(&db, &case_id, &version_id, &[(0, [0.0, 0.0, 300.0, 70.0])]);
        let mask = db
            .outbound_terms_from_plan(&case_id, &plan.id)
            .expect("mask");
        assert_eq!(mask.terms, vec!["Ahmet Yılmaz".to_string()]);
        assert_eq!(mask.short_skipped, 1);
    }

    #[test]
    fn stale_version_plan_is_rejected() {
        let (db, case_id, version_id) = setup();
        seed_page(
            &db,
            &case_id,
            &version_id,
            THREE_LINES,
            vec![block(0, 12, [10.0, 10.0, 200.0, 30.0])],
            0,
            Some("unrotated_cropbox_points"),
        );
        let plan = plan_with_regions(&db, &case_id, &version_id, &[(0, [5.0, 5.0, 210.0, 35.0])]);
        let version = db.get_version(&version_id).expect("v").expect("var");
        let blob = db
            .create_blob_record("st-redact-2", 10, "aad")
            .expect("blob");
        let newer = db
            .add_version(
                &version.document_id,
                &case_id,
                &blob,
                &"b".repeat(64),
                10,
                "application/pdf",
            )
            .expect("v2");
        db.conn()
            .execute(
                "UPDATE documents SET current_version_id=?1 WHERE id=?2",
                rusqlite::params![newer.id, version.document_id],
            )
            .expect("current");
        let err = db.outbound_terms_from_plan(&case_id, &plan.id).unwrap_err();
        assert_eq!(err.code(), "E_MASK_PLAN");
        assert!(err.to_string().contains("güncel sürüme"));
    }

    #[test]
    fn plan_from_other_case_is_not_found() {
        let (db, case_id, version_id) = setup();
        seed_page(
            &db,
            &case_id,
            &version_id,
            THREE_LINES,
            vec![block(0, 12, [10.0, 10.0, 200.0, 30.0])],
            0,
            Some("unrotated_cropbox_points"),
        );
        let plan = plan_with_regions(&db, &case_id, &version_id, &[(0, [5.0, 5.0, 210.0, 35.0])]);
        let err = db
            .outbound_terms_from_plan("başka-dosya", &plan.id)
            .unwrap_err();
        assert_eq!(err.code(), "E_NOT_FOUND");
    }

    #[test]
    fn region_without_intersecting_text_blocks_before_sending() {
        let (db, case_id, version_id) = setup();
        seed_page(
            &db,
            &case_id,
            &version_id,
            THREE_LINES,
            vec![block(0, 12, [10.0, 10.0, 200.0, 30.0])],
            0,
            Some("unrotated_cropbox_points"),
        );
        // Bölge sayfada ama metin bloğu yok: maske kanıtlanamaz, gönderim durur.
        let plan = plan_with_regions(
            &db,
            &case_id,
            &version_id,
            &[(0, [10.0, 400.0, 200.0, 430.0])],
        );
        let err = db.outbound_terms_from_plan(&case_id, &plan.id).unwrap_err();
        assert_eq!(err.code(), "E_MASK_PLAN");
        assert!(err.to_string().contains("kesişen metin bloğu yok"));
    }

    #[test]
    fn rotated_page_is_rejected() {
        let (db, case_id, version_id) = setup();
        seed_page(
            &db,
            &case_id,
            &version_id,
            THREE_LINES,
            vec![block(0, 12, [10.0, 10.0, 200.0, 30.0])],
            90,
            Some("unrotated_cropbox_points"),
        );
        let plan = plan_with_regions(&db, &case_id, &version_id, &[(0, [5.0, 5.0, 210.0, 35.0])]);
        let err = db.outbound_terms_from_plan(&case_id, &plan.id).unwrap_err();
        assert_eq!(err.code(), "E_MASK_PLAN");
        assert!(err.to_string().contains("döndürülmüş"));
    }

    #[test]
    fn non_pdf_coordinate_space_is_rejected() {
        let (db, case_id, version_id) = setup();
        seed_page(
            &db,
            &case_id,
            &version_id,
            THREE_LINES,
            vec![block(0, 12, [10.0, 10.0, 200.0, 30.0])],
            0,
            Some("image_pixels"),
        );
        let plan = plan_with_regions(&db, &case_id, &version_id, &[(0, [5.0, 5.0, 210.0, 35.0])]);
        let err = db.outbound_terms_from_plan(&case_id, &plan.id).unwrap_err();
        assert_eq!(err.code(), "E_MASK_PLAN");
        assert!(err.to_string().contains("koordinat uzayı"));
    }

    #[test]
    fn plan_without_sent_version_text_is_rejected() {
        let (db, case_id, version_id) = setup();
        let plan = plan_with_regions(&db, &case_id, &version_id, &[(0, [5.0, 5.0, 210.0, 35.0])]);
        let err = db.outbound_terms_from_plan(&case_id, &plan.id).unwrap_err();
        assert_eq!(err.code(), "E_MASK_PLAN");
        assert!(err.to_string().contains("ayrıştırılmamış"));
    }

    #[test]
    fn tampered_plan_case_scope_is_not_trusted() {
        let (db, case_id, version_id) = setup();
        seed_page(
            &db,
            &case_id,
            &version_id,
            THREE_LINES,
            vec![block(0, 12, [10.0, 10.0, 200.0, 30.0])],
            0,
            Some("unrotated_cropbox_points"),
        );
        let plan = plan_with_regions(&db, &case_id, &version_id, &[(0, [5.0, 5.0, 210.0, 35.0])]);
        // Plan satırındaki dosya kapsamı bozulursa **sürüm** bağı hâlâ doğru
        // olsa bile plan yabancı sayılır: satır kendi başına yetki vermez.
        //
        // v31 kapsam kapısı ve v51 kanıt kapısı bu bozulmayı SQL üzerinden
        // engeller; test, elle kurcalanmış/eski bir kasadan gelebilecek satırı
        // ölçtüğü için kapılar burada bilinçli düşürülür (okuma yolu hâlâ
        // kendi doğrulamasını yapar).
        db.conn()
            .execute_batch(
                "DROP TRIGGER scope_guard_redaction_plans_bu;
                 DROP TRIGGER redaction_plan_evidence_guard_bu;",
            )
            .expect("kapı düşür");
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
        db.conn()
            .execute(
                "UPDATE redaction_plans SET case_id=?1 WHERE id=?2",
                rusqlite::params![other.id, plan.id],
            )
            .expect("tamper");
        let err = db.outbound_terms_from_plan(&case_id, &plan.id).unwrap_err();
        assert_eq!(err.code(), "E_NOT_FOUND");
    }

    #[test]
    fn plan_bound_to_another_render_is_rejected() {
        let (db, case_id, version_id) = setup();
        seed_page(
            &db,
            &case_id,
            &version_id,
            THREE_LINES,
            vec![block(0, 12, [10.0, 10.0, 200.0, 30.0])],
            0,
            Some("unrotated_cropbox_points"),
        );
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.clone(),
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        db.add_redaction_regions(
            &plan.id,
            &[RedactionRegionInput {
                page_index: 0,
                rect: [5.0, 5.0, 210.0, 35.0],
                term_hash: "h".into(),
            }],
        )
        .expect("regions");
        // Plan doğuşta sürümün özetine bağlanır (v51); başka render'a bağlı
        // plan ancak **sürüm özeti değişerek** oluşabilir. Maske yolu yine
        // kapanmalı: geometri hangi dosyaya ait olduğunu kanıtlamıyor.
        //
        // v52 sürümün özetini **dondurur**: bu geçiş üretimde artık yoktur
        // (yalnız v52 öncesi/onarılmamış kasalarda görülebilir). Okuma
        // davranışını ölçebilmek için kapı bilinçli düşürülür.
        db.conn()
            .execute_batch("DROP TRIGGER document_version_identity_guard_bu;")
            .expect("kapı düşürüldü");
        db.conn()
            .execute(
                "UPDATE document_versions SET original_sha256=?1 WHERE id=?2",
                rusqlite::params!["f".repeat(64), version_id],
            )
            .expect("sürüm özeti değişti");
        let err = db.outbound_terms_from_plan(&case_id, &plan.id).unwrap_err();
        assert_eq!(err.code(), "E_MASK_PLAN");
        assert!(err.to_string().contains("render özeti"));
    }

    /// Maskeleme çıktısı yeni bir belge + sürümdür (`parent_version_id` kaynağa
    /// bağlanır). Plan bu çıktıyı **göstermelidir** — aynı kaynak sürümden iki
    /// maskeleme yapıldığında hangi çıktının hangi plandan geldiği sicilden
    /// okunabilsin.
    fn add_output_document(
        db: &Database,
        case_id: &str,
        source_version_id: &str,
        name: &str,
        sha: &str,
    ) -> crate::documents::DocumentVersion {
        let blob = db
            .create_blob_record(&format!("st-out-{sha}"), 12, "aad")
            .expect("blob");
        let doc = db
            .create_document(case_id, name, "pdf", None, "unknown")
            .expect("çıktı belgesi");
        db.add_version_with_parent(
            &doc.id,
            case_id,
            &blob,
            sha,
            12,
            "application/pdf",
            Some(source_version_id),
        )
        .expect("çıktı sürümü")
    }

    #[test]
    fn plan_records_output_version_and_reads_it_back() {
        let (db, case_id, version_id) = setup();
        let output = add_output_document(
            &db,
            &case_id,
            &version_id,
            "Belge (maskeli)",
            &"c".repeat(64),
        );
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.clone(),
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: Some(output.id.clone()),
                },
            )
            .expect("plan");
        // `get` yolu: bağ kimlik, çıktı sürüm numarası ve belge adıyla döner.
        assert_eq!(plan.output_version_id.as_deref(), Some(output.id.as_str()));
        assert_eq!(plan.output_version_no, Some(1));
        assert_eq!(
            plan.output_document_title.as_deref(),
            Some("Belge (maskeli)")
        );
        // Bağ **yazıldı**: hem canlı kimlik hem silinmeye dayanan iz taşınır.
        assert_eq!(plan.output_link_state, "linked");
        assert_eq!(plan.output_version_ref.as_deref(), Some(output.id.as_str()));
        // Kaynak sürüm alanı **değişmez**: plan kaynağa bağlıdır, çıktı ayrı
        // sütundadır (ikisi karışmaz).
        assert_eq!(plan.document_version_id, version_id);

        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].output_version_id.as_deref(),
            Some(output.id.as_str())
        );
        assert_eq!(rows[0].output_version_no, Some(1));
        assert_eq!(
            rows[0].output_document_title.as_deref(),
            Some("Belge (maskeli)")
        );
        assert_eq!(rows[0].output_link_state, "linked");
        assert_eq!(
            rows[0].output_version_ref.as_deref(),
            Some(output.id.as_str())
        );
        assert_eq!(rows[0].document_title.as_deref(), Some("Belge"));
    }

    #[test]
    fn plan_without_output_stays_unlinked_and_is_not_disguised() {
        let (db, case_id, version_id) = setup();
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id,
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        // Sütun eklenmeden önce yazılmış planlar: bağ **yok**, çıktı varmış
        // gibi gösterilmez.
        assert!(plan.output_version_id.is_none());
        assert!(plan.output_version_no.is_none());
        assert!(plan.output_document_title.is_none());
        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert!(rows[0].output_version_id.is_none());
        assert!(rows[0].output_version_no.is_none());
        assert!(rows[0].output_document_title.is_none());
        // Bağ **hiç yazılmadı**: durum `none`. `removed` (bağ vardı, çıktı
        // silindi) ile karıştırılmaz — arayüz ikisine ayrı cümle yazar.
        assert_eq!(plan.output_link_state, "none");
        assert!(plan.output_version_ref.is_none());
        assert_eq!(rows[0].output_link_state, "none");
        assert!(rows[0].output_version_ref.is_none());
    }

    /// Çıktı sürümü kasadan silinirse bağ **izi** kalır: `output_version_id`
    /// yabancı anahtarla (`ON DELETE SET NULL`) NULL'a döner, ama
    /// `output_version_ref` bağ yazılırken kaydedilen kimliği taşır. Sicil
    /// "kayıtlı değil" demez — "artık yok" der; ikisi ayrı gerçektir.
    #[test]
    fn deleted_output_version_keeps_link_trace() {
        let (db, case_id, version_id) = setup();
        let output = add_output_document(
            &db,
            &case_id,
            &version_id,
            "Belge (maskeli)",
            &"c".repeat(64),
        );
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.clone(),
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: Some(output.id.clone()),
                },
            )
            .expect("plan");
        db.conn()
            .execute("DELETE FROM document_versions WHERE id=?1", [&output.id])
            .expect("çıktı sürümü silindi");

        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows[0].id, plan.id);
        // Canlı bağ çözüldü, **iz** duruyor.
        assert!(rows[0].output_version_id.is_none());
        assert_eq!(
            rows[0].output_version_ref.as_deref(),
            Some(output.id.as_str())
        );
        assert_eq!(rows[0].output_link_state, "removed");
        // Silinen çıktının adı/sürüm numarası **uydurulmaz**.
        assert!(rows[0].output_version_no.is_none());
        assert!(rows[0].output_document_title.is_none());
        // Kaynak satır etkilenmez: plan hâlâ kaynağa bağlı okunur.
        assert_eq!(rows[0].document_title.as_deref(), Some("Belge"));
        assert_eq!(rows[0].document_version_id, version_id);
        // `get` yolu aynı gerçeği yazar (iki sorgu ayrışmaz).
        let got = db
            .get_redaction_plan(&plan.id)
            .expect("oku")
            .expect("plan var");
        assert_eq!(got.output_link_state, "removed");
        assert_eq!(got.output_version_ref.as_deref(), Some(output.id.as_str()));
    }

    /// Çıktı **belgesi** yumuşak-silinirse sürüm satırı yerinde kalır ama çıktı
    /// artık kullanılamaz: sicil bağı canlı saymaz (`removed`), "çıktı var"
    /// diye göstermez.
    #[test]
    fn soft_deleted_output_document_is_removed_not_linked() {
        let (db, case_id, version_id) = setup();
        let output = add_output_document(
            &db,
            &case_id,
            &version_id,
            "Belge (maskeli)",
            &"c".repeat(64),
        );
        db.create_redaction_plan(
            &case_id,
            &NewRedactionPlan {
                document_version_id: version_id,
                parse_run_id: None,
                render_hash: "a".repeat(64),
                output_profile: "share".into(),
                output_version_id: Some(output.id.clone()),
            },
        )
        .expect("plan");
        db.conn()
            .execute(
                "UPDATE documents SET deleted_at=?1 WHERE id=?2",
                rusqlite::params![now_iso(), output.document_id],
            )
            .expect("çıktı belgesi yumuşak-silindi");

        let rows = db.list_redaction_plans(&case_id).expect("liste");
        // Sürüm satırı duruyor (yabancı anahtar çözülmedi) ama belge silinmiş:
        // bağ canlı sayılmaz.
        assert_eq!(
            rows[0].output_version_id.as_deref(),
            Some(output.id.as_str())
        );
        assert_eq!(rows[0].output_link_state, "removed");
        assert_eq!(
            rows[0].output_version_ref.as_deref(),
            Some(output.id.as_str())
        );
    }

    #[test]
    fn plan_rejects_output_that_is_not_child_of_source() {
        let (db, case_id, version_id) = setup();
        // Ebeveynsiz sürüm: kaynağın alt sürümü değil.
        let blob = db.create_blob_record("st-orphan", 12, "aad").expect("blob");
        let doc = db
            .create_document(&case_id, "Başka belge", "pdf", None, "unknown")
            .expect("doc");
        let orphan = db
            .add_version(
                &doc.id,
                &case_id,
                &blob,
                &"d".repeat(64),
                12,
                "application/pdf",
            )
            .expect("sürüm");
        let err = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id,
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: Some(orphan.id),
                },
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_REDACTION_PLAN");
        assert!(err.to_string().contains("alt sürümü değil"));
        // Yanlış bağ sicile **yazılmadı**.
        assert!(db.list_redaction_plans(&case_id).expect("liste").is_empty());
    }

    #[test]
    fn plan_rejects_output_from_another_case() {
        let (db, case_id, version_id) = setup();
        let other = db
            .create_case(&NewCase {
                title: "Başka dosya".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        let other_blob = db
            .create_blob_record("st-other-src", 12, "aad")
            .expect("blob");
        let other_doc = db
            .create_document(&other.id, "Yabancı", "pdf", None, "unknown")
            .expect("doc");
        let other_src = db
            .add_version(
                &other_doc.id,
                &other.id,
                &other_blob,
                &"f".repeat(64),
                12,
                "application/pdf",
            )
            .expect("yabancı kaynak sürüm");
        let output = add_output_document(
            &db,
            &other.id,
            &other_src.id,
            "Yabancı (maskeli)",
            &"e".repeat(64),
        );
        let err = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id,
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: Some(output.id),
                },
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_REDACTION_PLAN");
        assert!(err.to_string().contains("başka dosyaya ait"));
        assert!(db.list_redaction_plans(&case_id).expect("liste").is_empty());
    }

    #[test]
    fn plan_rejects_output_equal_to_source() {
        let (db, case_id, version_id) = setup();
        let err = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.clone(),
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: Some(version_id),
                },
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_REDACTION_PLAN");
        assert!(err.to_string().contains("kendisi olamaz"));
    }

    /// Kaynağının **kendi** özetine bağlı, bölgeleri yazılmış plan: sicilde
    /// "uygulandı" görünen planların normal hâli (`setup()` sürümü `"a"*64`).
    fn applied_plan(db: &Database, case_id: &str, version_id: &str) -> RedactionPlanRow {
        let plan = db
            .create_redaction_plan(
                case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.to_string(),
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        db.add_redaction_regions(
            &plan.id,
            &[RedactionRegionInput {
                page_index: 0,
                rect: [1.0, 2.0, 3.0, 4.0],
                term_hash: "h1".into(),
            }],
        )
        .expect("bölge");
        db.get_redaction_plan(&plan.id)
            .expect("get")
            .expect("plan var")
    }

    fn document_of(db: &Database, version_id: &str) -> String {
        db.conn()
            .query_row(
                "SELECT document_id FROM document_versions WHERE id=?1",
                [version_id],
                |row| row.get(0),
            )
            .expect("belge")
    }

    /// F13-T01: "yerleşim değişince yeniden incelet". Belgeye yeni sürüm
    /// gelince planın geometrisi güncel belgeyi anlatmaz; sicil bunu **yazmak
    /// zorunda** — yoksa plan sonsuza dek "uygulandı" görünür.
    #[test]
    fn superseded_source_version_requires_reinspection() {
        let (db, case_id, version_id) = setup();
        let plan = applied_plan(&db, &case_id, &version_id);
        // Kendi sürümü güncelken sapma yok: bayrak her plana vurulmaz.
        assert_eq!(plan.reinspect_reason, None);
        assert_eq!(plan.current_version_no, Some(1));

        let blob = db
            .create_blob_record("st-redact-v2", 11, "aad")
            .expect("blob");
        let doc = document_of(&db, &version_id);
        db.add_version_with_parent(
            &doc,
            &case_id,
            &blob,
            &"c".repeat(64),
            11,
            "application/pdf",
            Some(&version_id),
        )
        .expect("v2");

        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].reinspect_reason.as_deref(),
            Some("version_superseded")
        );
        assert_eq!(rows[0].current_version_no, Some(2));
        // Geçmiş **yeniden yazılmaz**: maskeleme gerçekten uygulandı.
        assert_eq!(rows[0].state, "applied");
        // Sicil ile maske kapısı aynı gerçeği söyler (bayrak uydurma değil).
        let err = db
            .outbound_terms_from_plan(&case_id, &plan.id)
            .expect_err("maske kapısı kapanmalı");
        assert!(err.to_string().contains("güncel sürüme ait değil"));
    }

    /// Yürürlükte olmayan plan (`draft`) için "yeniden inceleme" istenmez:
    /// ortada uygulanmış bir geometri yok.
    #[test]
    fn draft_plan_does_not_ask_for_reinspection() {
        let (db, case_id, version_id) = setup();
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.clone(),
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        let blob = db
            .create_blob_record("st-draft-v2", 11, "aad")
            .expect("blob");
        let doc = document_of(&db, &version_id);
        db.add_version_with_parent(
            &doc,
            &case_id,
            &blob,
            &"c".repeat(64),
            11,
            "application/pdf",
            Some(&version_id),
        )
        .expect("v2");

        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows[0].id, plan.id);
        assert_eq!(rows[0].state, "draft");
        assert_eq!(rows[0].reinspect_reason, None);
        // Güncel sürüm yine **yazılır**: bilgi planın durumuna bağlı değil.
        assert_eq!(rows[0].current_version_no, Some(2));
    }

    /// Planın bağlandığı render özeti sürümün özetiyle tutmuyorsa geometri
    /// hangi dosyaya ait olduğunu kanıtlamaz.
    #[test]
    fn render_mismatch_requires_reinspection() {
        let (db, case_id, version_id) = setup();
        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.clone(),
                    parse_run_id: None,
                    // Plan doğarken sürümün özetine (`"a"*64`) bağlanır; v51
                    // doğuşta başka özete izin vermez (kapı).
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        db.add_redaction_regions(
            &plan.id,
            &[RedactionRegionInput {
                page_index: 0,
                rect: [1.0, 2.0, 3.0, 4.0],
                term_hash: "h1".into(),
            }],
        )
        .expect("bölge");
        // Uyuşmazlığın **tek** meşru kaynağı sürüm tarafıdır: sürümün özeti
        // değişir (yeniden render/aktarım), planınki donmuştur. v52 sürüm
        // özetini dondurur → uyuşmazlık yalnız **v52 öncesi** satırlarda
        // doğabilir; okuma yolu ölçülsün diye kapı bilinçli düşürülür.
        db.conn()
            .execute_batch("DROP TRIGGER document_version_identity_guard_bu;")
            .expect("kapı düşürüldü");
        db.conn()
            .execute(
                "UPDATE document_versions SET original_sha256=?1 WHERE id=?2",
                rusqlite::params!["b".repeat(64), version_id],
            )
            .expect("sürüm özeti değişti");

        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows[0].reinspect_reason.as_deref(), Some("render_changed"));
    }

    /// Sürüm yeniden ayrıştırıldıysa planın bağlı olduğu ayrıştırma artık en
    /// yeni koşu değildir: yerleşim değişmiş olabilir → yeniden inceleme.
    #[test]
    fn newer_parse_run_requires_reinspection() {
        let (db, case_id, version_id) = setup();
        let page = |text: &str| crate::parsing::NewPage {
            index: 0,
            text: text.to_string(),
            width: Some(595.0),
            height: Some(842.0),
            origin: "native".into(),
            warnings: Vec::new(),
            rotation: 0,
            coordinate_space: Some("unrotated_cropbox_points".into()),
            page_label: None,
            blocks: Vec::new(),
        };
        let run1 = db
            .start_parse_run(&version_id, &case_id, "pdfium", "1.0", "{}")
            .expect("koşu");
        db.store_pages(&run1.id, &case_id, &[page("Ahmet Yılmaz")])
            .expect("sayfa");

        let plan = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.clone(),
                    parse_run_id: Some(run1.id.clone()),
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        db.add_redaction_regions(
            &plan.id,
            &[RedactionRegionInput {
                page_index: 0,
                rect: [1.0, 2.0, 3.0, 4.0],
                term_hash: "h1".into(),
            }],
        )
        .expect("bölge");
        // Bağlı koşu hâlâ en yeniyse sapma yok.
        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows[0].reinspect_reason, None);

        let run2 = db
            .start_parse_run(&version_id, &case_id, "pdfium", "1.0", "{}")
            .expect("koşu 2");
        db.store_pages(&run2.id, &case_id, &[page("Ahmet Yılmaz (yeni yerleşim)")])
            .expect("sayfa 2");

        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows[0].reinspect_reason.as_deref(), Some("parse_run_newer"));
    }

    /// Plan satırı sürümünden uzun yaşar (`document_version_id` yabancı
    /// anahtar değil): sürüm satırı yoksa okuma **çöker değil**, nedeni yazar.
    #[test]
    fn missing_source_version_requires_reinspection() {
        let (db, case_id, version_id) = setup();
        let plan = applied_plan(&db, &case_id, &version_id);
        db.conn()
            .execute("DELETE FROM document_versions WHERE id=?1", [&version_id])
            .expect("sürüm silindi");

        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(rows[0].id, plan.id);
        assert_eq!(rows[0].reinspect_reason.as_deref(), Some("version_missing"));
        // Ad/sürüm uydurulmaz.
        assert!(rows[0].document_title.is_none());
        assert!(rows[0].version_no.is_none());
        assert!(rows[0].current_version_no.is_none());
    }
    /// Maskeleme bölgesi **kanıttır**: hangi sayfada hangi dikdörtgenin
    /// karartıldığı. Çekirdek bölgeyi yalnız plan `draft`ken INSERT eder
    /// (`add_redaction_regions` sonra planı `applied` yapar); düzeltme, taşıma
    /// veya silme yolu yoktur. v50 kapısı doğuşta biçimi, sonrasında **her
    /// sütunu** denetler.
    #[test]
    fn redaction_region_evidence_is_frozen_and_well_formed() {
        let (db, case_id, version_id) = setup();
        let region_error = |result: Result<usize, StorageError>, label: &str| match result {
            Err(StorageError::Integrity(message)) => assert!(
                message.contains("redaction_regions"),
                "{label}: mesaj tabloyu adlandırmıyor: {message}"
            ),
            other => panic!("{label}: beklenen E_DB_INTEGRITY, ölçülen: {other:?}"),
        };
        let new_plan = || {
            db.create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.clone(),
                    parse_run_id: None,
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan")
        };
        // Kapanmış plan + içindeki bölge (çekirdek sırası).
        let closed = new_plan();
        db.add_redaction_regions(
            &closed.id,
            &[RedactionRegionInput {
                page_index: 2,
                rect: [10.0, 20.0, 30.0, 40.0],
                term_hash: "h".into(),
            }],
        )
        .expect("bölge");
        let region_id: String = db
            .conn()
            .query_row(
                "SELECT id FROM redaction_regions WHERE plan_id=?1",
                [&closed.id],
                |row| row.get(0),
            )
            .expect("bölge kimliği");
        // Açık (draft) plan: doğuş kusurları ona yazılır.
        let open = new_plan();

        // --- doğuş: her satır **tek** bir kusur taşır ----------------------
        let write = |id: &str,
                     plan: &str,
                     page: i64,
                     quads: &str,
                     space: &str,
                     category: &str,
                     hash: Option<&str>,
                     decision: &str| {
            db.conn()
                .execute(
                    "INSERT INTO redaction_regions
                       (id,plan_id,page_index,quads,coordinate_space,category,
                        detected_text_hash,user_decision)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    rusqlite::params![id, plan, page, quads, space, category, hash, decision],
                )
                .map_err(StorageError::from)
        };
        let good = "[[1.0,2.0,3.0,4.0]]";
        region_error(
            write(
                "",
                &open.id,
                0,
                good,
                "pdf_page_points",
                "user_term",
                None,
                "redact",
            ),
            "boş kimlik",
        );
        region_error(
            write(
                "x-ghost",
                "yok",
                0,
                good,
                "pdf_page_points",
                "user_term",
                None,
                "redact",
            ),
            "olmayan plan",
        );
        region_error(
            write(
                "x-closed",
                &closed.id,
                0,
                good,
                "pdf_page_points",
                "user_term",
                None,
                "redact",
            ),
            "kapanmış plan",
        );
        region_error(
            write(
                "x-page",
                &open.id,
                -1,
                good,
                "pdf_page_points",
                "user_term",
                None,
                "redact",
            ),
            "negatif sayfa",
        );
        region_error(
            write(
                "x-json",
                &open.id,
                0,
                "bozuk",
                "pdf_page_points",
                "user_term",
                None,
                "redact",
            ),
            "geçersiz JSON",
        );
        region_error(
            write(
                "x-object",
                &open.id,
                0,
                r#"{"a":1}"#,
                "pdf_page_points",
                "user_term",
                None,
                "redact",
            ),
            "nesne",
        );
        region_error(
            write(
                "x-empty",
                &open.id,
                0,
                "[]",
                "pdf_page_points",
                "user_term",
                None,
                "redact",
            ),
            "boş dizi",
        );
        region_error(
            write(
                "x-scalar",
                &open.id,
                0,
                "[1,2,3,4]",
                "pdf_page_points",
                "user_term",
                None,
                "redact",
            ),
            "düz sayı elemanı",
        );
        region_error(
            write(
                "x-arity",
                &open.id,
                0,
                "[[1,2,3]]",
                "pdf_page_points",
                "user_term",
                None,
                "redact",
            ),
            "üçlü dikdörtgen",
        );
        region_error(
            write(
                "x-text",
                &open.id,
                0,
                r#"[[1,2,3,"a"]]"#,
                "pdf_page_points",
                "user_term",
                None,
                "redact",
            ),
            "metinli koordinat",
        );
        region_error(
            write(
                "x-space",
                &open.id,
                0,
                good,
                "image_pixels",
                "user_term",
                None,
                "redact",
            ),
            "yanlış koordinat uzayı",
        );
        region_error(
            write(
                "x-cat",
                &open.id,
                0,
                good,
                "pdf_page_points",
                "",
                None,
                "redact",
            ),
            "boş kategori",
        );
        region_error(
            write(
                "x-cat2",
                &open.id,
                0,
                good,
                "pdf_page_points",
                "   ",
                None,
                "redact",
            ),
            "boşluk kategori",
        );
        region_error(
            write(
                "x-hash",
                &open.id,
                0,
                good,
                "pdf_page_points",
                "user_term",
                Some(""),
                "redact",
            ),
            "boş terim özeti",
        );
        region_error(
            write(
                "x-keep",
                &open.id,
                0,
                good,
                "pdf_page_points",
                "user_term",
                None,
                "keep",
            ),
            "doğuştan keep",
        );
        region_error(
            write(
                "x-pending",
                &open.id,
                0,
                good,
                "pdf_page_points",
                "user_term",
                None,
                "pending",
            ),
            "doğuştan pending",
        );
        // Meşru doğum (çekirdeğin yazdığı biçim) takılmaz.
        write(
            "ok-multi",
            &open.id,
            1,
            "[[1,2,3,4],[5,6,7,8]]",
            "pdf_page_points",
            "user_term",
            None,
            "redact",
        )
        .expect("çok kutulu bölge");
        write(
            "ok-null-hash",
            &open.id,
            1,
            good,
            "pdf_page_points",
            "user_term",
            None,
            "redact",
        )
        .expect("NULL özet");

        // --- sonrası: sekiz sütunun hepsi donar ----------------------------
        let updates: Vec<(String, String)> = vec![
            (
                "başka plana taşı".into(),
                format!("UPDATE redaction_regions SET plan_id='{}' WHERE id='{region_id}'", open.id),
            ),
            (
                "sayfa".into(),
                format!("UPDATE redaction_regions SET page_index=0 WHERE id='{region_id}'"),
            ),
            (
                "geometri".into(),
                format!(
                    "UPDATE redaction_regions SET quads='[[9.0,9.0,9.0,9.0]]' WHERE id='{region_id}'"
                ),
            ),
            (
                "karar".into(),
                format!("UPDATE redaction_regions SET user_decision='keep' WHERE id='{region_id}'"),
            ),
            (
                "kategori".into(),
                format!("UPDATE redaction_regions SET category='' WHERE id='{region_id}'"),
            ),
            (
                "koordinat uzayı".into(),
                format!(
                    "UPDATE redaction_regions SET coordinate_space='image_pixels' WHERE id='{region_id}'"
                ),
            ),
            (
                "terim özeti".into(),
                format!(
                    "UPDATE redaction_regions SET detected_text_hash='zz' WHERE id='{region_id}'"
                ),
            ),
            (
                "kimlik".into(),
                format!("UPDATE redaction_regions SET id='{region_id}-b' WHERE id='{region_id}'"),
            ),
        ];
        for (label, sql) in updates {
            region_error(
                db.conn().execute(&sql, []).map_err(StorageError::from),
                &label,
            );
        }
        // Değişmeyen yazma takılmaz (kapı yalnız değişimi durdurur).
        db.conn()
            .execute(
                "UPDATE redaction_regions SET quads=quads WHERE id=?1",
                [&region_id],
            )
            .expect("değişmeyen yazma");
        // Satır hâlâ okunur ve eski değerlerini taşır.
        let rows = db.list_redaction_regions(&closed.id).expect("bölgeler");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].page_index, 2);
        assert_eq!(
            rows[0].rects.as_deref(),
            Some(&[[10.0, 20.0, 30.0, 40.0]][..])
        );
        assert_eq!(rows[0].user_decision, "redact");
    }

    /// v54 — bölge **sayfanın kanıtıdır**: planın maskede kullanılacak
    /// ayrıştırma koşusunda **var olan** bir sayfayı göstermeli ve dikdörtgeni
    /// o sayfanın kırpma kutusunda kalmalı. 0050 biçimi ve değişmezliği
    /// kapıladı; sayfa sınırını bilinçli olarak açık bıraktı (kapanış sınırı c).
    /// Kusur sessiz sızıntı değil **kanıt yalanıydı**: plan «sayfa 5'i
    /// maskeledim» diyordu, belge 2 sayfa; ya da kutu sayfa dışındaydı ve
    /// maskeleme «kesişen metin bloğu yok» ile düşüyordu.
    #[test]
    fn redaction_region_must_lie_on_a_parsed_page_inside_its_box() {
        let (db, case_id, version_id) = setup();
        let seed_page =
            |index: i64, width: Option<f64>, height: Option<f64>| crate::parsing::NewPage {
                index,
                text: format!("sayfa {index}"),
                width,
                height,
                origin: "native".into(),
                warnings: Vec::new(),
                rotation: 0,
                coordinate_space: Some("unrotated_cropbox_points".into()),
                page_label: None,
                blocks: Vec::new(),
            };
        let run1 = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu 1");
        db.store_pages(
            &run1.id,
            &case_id,
            &[
                seed_page(0, Some(595.0), Some(842.0)),
                seed_page(1, None, None),
            ],
        )
        .expect("sayfalar");
        db.finish_parse_run(&run1.id, "succeeded", &[])
            .expect("koşu 1 bitti");
        // Daha yeni koşu: fazladan sayfa 2 taşır. Plan run1'i **bildirirse**
        // maskede run1 kullanılır; sayfa 2 orada yoktur.
        let run2 = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu 2");
        db.store_pages(
            &run2.id,
            &case_id,
            &[
                seed_page(0, Some(595.0), Some(842.0)),
                seed_page(1, None, None),
                seed_page(2, Some(595.0), Some(842.0)),
            ],
        )
        .expect("sayfa 2");
        db.finish_parse_run(&run2.id, "succeeded", &[])
            .expect("koşu 2 bitti");
        let new_plan = |run: Option<&str>| {
            db.create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.clone(),
                    parse_run_id: run.map(str::to_string),
                    render_hash: "a".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan")
        };
        let declared = new_plan(Some(&run1.id));
        let latest = new_plan(None);
        let write = |plan: &str, id: &str, page: i64, quads: &str| {
            db.conn()
                .execute(
                    "INSERT INTO redaction_regions
                       (id,plan_id,page_index,quads,coordinate_space,category,
                        detected_text_hash,user_decision)
                     VALUES (?1,?2,?3,?4,'pdf_page_points','user_term',NULL,'redact')",
                    rusqlite::params![id, plan, page, quads],
                )
                .map_err(StorageError::from)
        };
        let region_error = |result: Result<usize, StorageError>, label: &str| match result {
            Err(StorageError::Integrity(message)) => assert!(
                message.contains("redaction_regions"),
                "{label}: mesaj tabloyu adlandırmıyor: {message}"
            ),
            other => panic!("{label}: beklenen E_DB_INTEGRITY, ölçülen: {other:?}"),
        };

        // --- sayfa varlığı: bildirilen koşu esastır -------------------------
        region_error(
            write(&declared.id, "g-page", 2, "[[10,10,50,50]]"),
            "bildirilen koşuda olmayan sayfa (yalnız en yeni koşuda var)",
        );
        region_error(
            write(&declared.id, "g-page-hi", 9, "[[10,10,50,50]]"),
            "koşuda hiç olmayan sayfa",
        );
        write(&latest.id, "ok-latest", 2, "[[10,10,50,50]]").expect("en yeni koşuda sayfa 2 meşru");

        // --- biçim: pozitif alanlı dikdörtgen ------------------------------
        for (id, quads, label) in [
            ("g-inv", "[[100,100,50,50]]", "ters dikdörtgen"),
            ("g-zero-w", "[[10,10,10,50]]", "sıfır genişlik"),
            ("g-zero-h", "[[10,10,50,10]]", "sıfır yükseklik"),
        ] {
            region_error(write(&declared.id, id, 0, quads), label);
        }

        // --- sınır: kutu sayfanın kırpma kutusunda -------------------------
        for (id, quads, label) in [
            ("g-wide", "[[10,10,900,50]]", "genişliği aşan kutu"),
            ("g-neg", "[[-50,10,100,50]]", "negatif x kutusu"),
            ("g-neg-y", "[[10,-50,50,100]]", "negatif y kutusu"),
            ("g-tall", "[[10,10,100,900]]", "yüksekliği aşan kutu"),
            (
                "g-second",
                "[[10,10,50,50],[600,10,700,50]]",
                "ikinci dörtlü taşıyor",
            ),
            ("g-over", "[[0,0,597,842]]", "2 punto taşma (pay 1)"),
            (
                "g-neg-over",
                "[[-1.5,0,100,100]]",
                "1,5 punto negatif taşma (pay 1)",
            ),
        ] {
            region_error(write(&declared.id, id, 0, quads), label);
        }
        // Meşru: kenara dayanan kutu yuvarlama payı içinde kalır.
        write(&latest.id, "ok-edge", 0, "[[0,0,595.5,842.5]]").expect("kenara dayanan kutu meşru");
        // Geometrisi NULL sayfada kutu biçimi denetlenir, sınır denetlenmez.
        region_error(
            write(&latest.id, "g-null-shape", 1, "[[10,10,10,50]]"),
            "geometrisi NULL sayfada da ters kutu reddedilir",
        );
        write(&latest.id, "ok-null-bound", 1, "[[0,0,99999,99999]]")
            .expect("geometrisi bilinmeyen sayfada sınır boşta kalır");

        // --- çekirdeğin yolu aynı kapıdan geçer ----------------------------
        match db.add_redaction_regions(
            &declared.id,
            &[RedactionRegionInput {
                page_index: 9,
                rect: [10.0, 10.0, 50.0, 50.0],
                term_hash: "h".into(),
            }],
        ) {
            Err(StorageError::Integrity(message)) => {
                assert!(message.contains("redaction_regions"), "çekirdek: {message}")
            }
            other => panic!("çekirdek sayfa dışı bölgeyi reddetmeli: {other:?}"),
        }
        db.add_redaction_regions(
            &declared.id,
            &[RedactionRegionInput {
                page_index: 0,
                rect: [0.0, 0.0, 595.5, 842.5],
                term_hash: "h".into(),
            }],
        )
        .expect("çekirdeğin meşru bölgesi");

        // Yarım kalmış (`partial`) koşuda sayfa kümesi **belgelenmiş** sayılmaz:
        // eksik sayfa «yalan» değildir, yazım durmaz (maske PDF'i maskeler).
        let run3 = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu 3");
        db.store_pages(
            &run3.id,
            &case_id,
            &[seed_page(0, Some(595.0), Some(842.0))],
        )
        .expect("sayfa 3");
        db.finish_parse_run(&run3.id, "partial", &[])
            .expect("koşu 3 bitti");
        let partial = new_plan(None);
        write(&partial.id, "ok-partial", 5, "[[10,10,50,50]]")
            .expect("yarım koşuda eksik sayfa yalan sayılmaz");

        // Tamamlanmış ama **hiç sayfası olmayan** koşu: sayfa kümesi yoktur,
        // varlık cümlesi boşta kalır (maske zaten «metin yok» diye reddeder).
        let empty_run = db
            .start_parse_run(&version_id, &case_id, "pymupdf", "1.24", "{}")
            .expect("boş koşu");
        db.finish_parse_run(&empty_run.id, "succeeded", &[])
            .expect("boş koşu bitti");
        let empty_plan = new_plan(Some(&empty_run.id));
        write(&empty_plan.id, "ok-empty-run", 0, "[[10,10,50,50]]")
            .expect("sayfasız tamamlanmış koşuda varlık cümlesi boşta");
    }

    /// v51 — plan **künyesi** doğuşta tutarlı olmalı ve sonra donmalı:
    /// `render_hash` kaynak sürümün özeti, çıktı izi bağın kimliği,
    /// kapanmış (`applied`) plan kapalı.
    ///
    /// Her satır **tek** bir kusur taşır ve kendi satırına yazar: kimliği
    /// değiştiren bir yazım sonrakileri 0 satırla eşleştirip sahte «geçti»
    /// verdirirdi (v49'un ölçüm dersi).
    #[test]
    fn redaction_plan_evidence_is_bound_at_birth_and_frozen() {
        let (db, case_id, version_id) = setup();
        let output_same = add_output_document(
            &db,
            &case_id,
            &version_id,
            "Belge (maskeli)",
            &"c".repeat(64),
        );
        let output_other = add_output_document(
            &db,
            &case_id,
            &version_id,
            "Belge (maskeli 2)",
            &"d".repeat(64),
        );
        // Aynı dosyada **başka** belgenin sürümü + ayrıştırması: kapsam kapısı
        // bunları serbest bırakır, kanıt kapısı dondurmalıdır.
        let foreign_blob = db
            .create_blob_record("st-other-doc", 10, "aad")
            .expect("blob");
        let other_doc = db
            .create_document(&case_id, "Başka belge", "pdf", None, "unknown")
            .expect("belge");
        let other_version = db
            .add_version(
                &other_doc.id,
                &case_id,
                &foreign_blob,
                &"e".repeat(64),
                10,
                "application/pdf",
            )
            .expect("sürüm");
        db.conn()
            .execute(
                "INSERT INTO parse_runs
                   (id,document_version_id,case_id,engine,engine_version,
                    settings_hash,status,created_at)
                 VALUES ('pr-other',?1,?2,'pdfium','1','s','succeeded',
                         '2026-01-01T00:00:00Z')",
                rusqlite::params![other_version.id, case_id],
            )
            .expect("ayrıştırma");

        let plan_error = |result: Result<usize, StorageError>, label: &str| match result {
            Err(StorageError::Integrity(message)) => assert!(
                message.contains("redaction_plans"),
                "{label}: mesaj tabloyu adlandırmıyor: {message}"
            ),
            other => panic!("{label}: beklenen E_DB_INTEGRITY, ölçülen: {other:?}"),
        };
        let now = "2026-06-01T00:00:00Z";
        #[allow(clippy::too_many_arguments)]
        let write = |id: &str,
                     doc: &str,
                     render: &str,
                     space: &str,
                     profile: &str,
                     plan_version: i64,
                     created: &str,
                     updated: &str,
                     out: Option<&str>,
                     trace: Option<&str>| {
            db.conn()
                .execute(
                    "INSERT INTO redaction_plans
                       (id,case_id,document_version_id,parse_run_id,render_hash,
                        coordinate_space,plan_version,state,output_profile,
                        created_at,updated_at,output_version_id,output_version_ref)
                     VALUES (?1,?2,?3,NULL,?4,?5,?6,'draft',?7,?8,?9,?10,?11)",
                    rusqlite::params![
                        id,
                        case_id,
                        doc,
                        render,
                        space,
                        plan_version,
                        profile,
                        created,
                        updated,
                        out,
                        trace
                    ],
                )
                .map_err(StorageError::from)
        };
        let good_render = "a".repeat(64);
        // --- doğuş kusurları ----------------------------------------------
        plan_error(
            write(
                "",
                &version_id,
                &good_render,
                "pdf_page_points",
                "share",
                1,
                now,
                now,
                None,
                None,
            ),
            "boş kimlik",
        );
        plan_error(
            write(
                "x-h",
                &version_id,
                "h",
                "pdf_page_points",
                "share",
                1,
                now,
                now,
                None,
                None,
            ),
            "render özeti başka ('h')",
        );
        plan_error(
            write(
                "x-hex",
                &version_id,
                &"b".repeat(64),
                "pdf_page_points",
                "share",
                1,
                now,
                now,
                None,
                None,
            ),
            "render özeti başka 64 hex",
        );
        plan_error(
            write(
                "x-space",
                &version_id,
                &good_render,
                "page",
                "share",
                1,
                now,
                now,
                None,
                None,
            ),
            "koordinat uzayı 'page'",
        );
        plan_error(
            write(
                "x-prof",
                &version_id,
                &good_render,
                "pdf_page_points",
                "",
                1,
                now,
                now,
                None,
                None,
            ),
            "boş çıktı profili",
        );
        plan_error(
            write(
                "x-prof2",
                &version_id,
                &good_render,
                "pdf_page_points",
                "   ",
                1,
                now,
                now,
                None,
                None,
            ),
            "boşluk çıktı profili",
        );
        plan_error(
            write(
                "x-ver",
                &version_id,
                &good_render,
                "pdf_page_points",
                "share",
                0,
                now,
                now,
                None,
                None,
            ),
            "plan sürümü 0",
        );
        plan_error(
            write(
                "x-created",
                &version_id,
                &good_render,
                "pdf_page_points",
                "share",
                1,
                "",
                now,
                None,
                None,
            ),
            "boş created_at",
        );
        plan_error(
            write(
                "x-updated",
                &version_id,
                &good_render,
                "pdf_page_points",
                "share",
                1,
                now,
                "",
                None,
                None,
            ),
            "boş updated_at",
        );
        plan_error(
            write(
                "x-trace",
                &version_id,
                &good_render,
                "pdf_page_points",
                "share",
                1,
                now,
                now,
                Some(&output_same.id),
                Some(&output_other.id),
            ),
            "çıktı izi başka kimlik",
        );
        plan_error(
            write(
                "x-trace2",
                &version_id,
                &good_render,
                "pdf_page_points",
                "share",
                1,
                now,
                now,
                Some(&output_same.id),
                None,
            ),
            "çıktı izi NULL",
        );
        plan_error(
            write(
                "x-self",
                &version_id,
                &good_render,
                "pdf_page_points",
                "share",
                1,
                now,
                now,
                Some(&version_id),
                Some(&version_id),
            ),
            "çıktı bağı kaynağın kendisi",
        );
        plan_error(
            write(
                "x-faketrace",
                &version_id,
                &good_render,
                "pdf_page_points",
                "share",
                1,
                now,
                now,
                None,
                Some(&output_same.id),
            ),
            "bağsız uydurma iz",
        );
        // Meşru doğuş (çekirdeğin yolu) takılmaz.
        write(
            "ok-birth",
            &version_id,
            &good_render,
            "pdf_page_points",
            "share",
            1,
            now,
            now,
            Some(&output_same.id),
            Some(&output_same.id),
        )
        .expect("meşru plan doğuşu");

        // --- güncelleme: kanıt bağı donar --------------------------------
        let new_plan = || {
            db.create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.clone(),
                    parse_run_id: None,
                    render_hash: good_render.clone(),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan")
        };
        let update = |id: &str, sets: &str| {
            db.conn()
                .execute(
                    &format!("UPDATE redaction_plans SET {sets} WHERE id='{id}'"),
                    [],
                )
                .map_err(StorageError::from)
        };
        let p = new_plan();
        plan_error(
            update(&p.id, &format!("render_hash='{}'", "b".repeat(64))),
            "render özetini çevir",
        );
        let p = new_plan();
        plan_error(
            update(&p.id, "coordinate_space='image_pixels'"),
            "koordinat uzayını çevir",
        );
        let p = new_plan();
        plan_error(
            update(&p.id, "output_profile='other'"),
            "çıktı profilini çevir",
        );
        let p = new_plan();
        plan_error(update(&p.id, "plan_version=7"), "plan sürümünü çevir");
        let p = new_plan();
        plan_error(
            update(&p.id, "created_at='2020-01-01T00:00:00Z'"),
            "yazılma zamanını geriye al",
        );
        let p = new_plan();
        plan_error(
            update(
                &p.id,
                &format!("document_version_id='{}'", other_version.id),
            ),
            "aynı dosyada başka belgeye taşı",
        );
        let p = new_plan();
        plan_error(
            update(&p.id, "parse_run_id='pr-other'"),
            "başka belgenin ayrıştırmasına bağla",
        );
        let p = new_plan();
        plan_error(
            update(
                &p.id,
                &format!(
                    "output_version_id='{}',output_version_ref='{}'",
                    output_same.id, output_same.id
                ),
            ),
            "çıktı bağını sonradan doldur",
        );
        let linked = db
            .create_redaction_plan(
                &case_id,
                &NewRedactionPlan {
                    document_version_id: version_id.clone(),
                    parse_run_id: None,
                    render_hash: good_render.clone(),
                    output_profile: "share".into(),
                    output_version_id: Some(output_same.id.clone()),
                },
            )
            .expect("bağlı plan");
        plan_error(
            update(
                &linked.id,
                &format!(
                    "output_version_id='{}',output_version_ref='{}'",
                    output_other.id, output_other.id
                ),
            ),
            "çıktı bağını başka sürüme çevir",
        );
        let applied = new_plan();
        db.add_redaction_regions(
            &applied.id,
            &[RedactionRegionInput {
                page_index: 0,
                rect: [1.0, 2.0, 3.0, 4.0],
                term_hash: "h".into(),
            }],
        )
        .expect("bölge");
        plan_error(update(&applied.id, "state='draft'"), "kapanmış planı aç");
        let applied2 = new_plan();
        db.add_redaction_regions(
            &applied2.id,
            &[RedactionRegionInput {
                page_index: 0,
                rect: [1.0, 2.0, 3.0, 4.0],
                term_hash: "h".into(),
            }],
        )
        .expect("bölge");
        plan_error(
            update(&applied2.id, "state='approved'"),
            "kapanmış planı approved'a çevir",
        );
        // Değişmeyen yazma ve serbest sütunlar takılmaz.
        update(&applied.id, "state='applied',reviewer='av. A'").expect("değişmeyen yazma");
        update(&applied.id, "updated_at='2026-07-01T00:00:00Z'").expect("updated_at serbest");
        let rows = db.list_redaction_plans(&case_id).expect("liste");
        assert_eq!(
            rows.iter().filter(|r| r.id == applied.id).count(),
            1,
            "deneme satırı hâlâ tek"
        );
    }
}
