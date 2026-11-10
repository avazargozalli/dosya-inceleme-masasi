//! Dışa aktarma kayıtları. Bir dosyanın verisini kasa dışına çıkaran her eylem
//! ayrı kayda alınır; **kaynak sürümler**, şablon sürümü ve çıktı hash'i korunur
//! (şartname §16.3, §433: `exports` + `export_items` "kullanılan şablonu, kaynak
//! sürümlerini, çıktının hash'ini ve maskeleme durumunu kaydeder").
//!
//! Sözleşme: kaynak izi **ölçülür**, tahmin edilmez. Kalem bilinmeyen ya da
//! başka dosyaya ait bir sürümü gösteriyorsa kayıt **hiç yazılmaz**
//! (`E_EXPORT_SOURCE`) — yanlış iz, izsizlikten kötüdür.

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

/// Şablon sürümü alanının üst sınırı (kısa etiket: `case-report@1`).
pub const MAX_TEMPLATE_VERSION_CHARS: usize = 64;
/// Sayfa aralığı alanının üst sınırı (`12-14` gibi kısa aralık).
pub const MAX_PAGE_RANGE_CHARS: usize = 32;
/// Atıf etiketi alanının üst sınırı.
pub const MAX_CITATION_LABEL_CHARS: usize = 64;
/// Kapalı maskeleme durumu kümesi: sürüm maskeli bir kopya mı, değil mi?
/// (Üçüncü bir değer uydurulmaz; şüphe varsa kayıt reddedilir.)
pub const REDACTION_STATES: [&str; 2] = ["none", "masked"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRow {
    pub id: String,
    pub case_id: String,
    pub export_type: String,
    pub status: String,
    pub destination_kind: String,
    /// Kullanıcının seçtiği **gerçek** hedef (dosya ya da paket yolu). 24. şema
    /// sütunu; eski kayıtlarda `null` — uydurulmaz.
    pub destination_path: Option<String>,
    pub output_hash: Option<String>,
    /// Çıktıyı üreten biçim/şablon sürümü (`case-report@1` gibi). Biçim sürümü
    /// taşımayan eski kayıtlarda `null`.
    pub template_version: Option<String>,
    /// Bu kayda yazılmış **kaynak sürüm kalemi** sayısı. `0` ise kaynak izi
    /// yoktur (bu sütunlar kullanılmaya başlamadan önce yazılmış kayıt) —
    /// "kaynak yok" diye uydurulmaz, ekran bunu ayrı söyler.
    pub item_count: i64,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// Dışa aktarmanın **kaynak sürüm** kalemi (şartname §433).
///
/// `output_page_range` ve `citation_label` bugün üretilemediği için `None`
/// bırakılır: raporun hangi sayfasına hangi belgenin düştüğü worker çıktısında
/// eşlenmiyor; uydurma numara yazılmaz.
#[derive(Debug, Clone)]
pub struct NewExportItem {
    pub source_version_id: String,
    pub output_page_range: Option<String>,
    pub citation_label: Option<String>,
    /// `REDACTION_STATES` içinde olmalı.
    pub redaction_state: String,
}

/// Kaynak izi okuma satırı: sürüm kimliği + belge künyesi.
///
/// Şemada `export_items.source_version_id` **FK değildir** (denetim izi sürüm
/// silinse de kalır); silinmiş sürümde belge adı/sürüm no `null` döner —
/// kimlik uydurulmaz.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportItemRow {
    pub id: String,
    pub export_id: String,
    pub source_version_id: String,
    pub document_id: Option<String>,
    pub document_title: Option<String>,
    pub version_no: Option<i64>,
    pub output_page_range: Option<String>,
    pub citation_label: Option<String>,
    pub redaction_state: String,
}

/// Kırpılmış meta alanı. Boş dize `None`'dır (boş alan bir değer değildir);
/// kontrol karakteri ve sınır aşımı **reddedilir** (sessiz kırpma yok).
fn normalize_meta(
    value: Option<&str>,
    max_chars: usize,
    label: &'static str,
) -> Result<Option<String>, StorageError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > max_chars {
        return Err(StorageError::ExportSourceInvalid {
            reason: format!("{label} {max_chars} karakteri aşıyor"),
        });
    }
    if trimmed.chars().any(char::is_control) {
        return Err(StorageError::ExportSourceInvalid {
            reason: format!("{label} kontrol karakteri taşıyor"),
        });
    }
    Ok(Some(trimmed.to_string()))
}

impl Database {
    /// Dışa aktarmayı **kaynak iziyle** kaydeder.
    ///
    /// `items` boş olabilir (kaynağı olmayan çıktı), ama doluysa her kalem
    /// doğrulanır: sürüm var olmalı, **aynı dosyaya** ait olmalı, mükerrer
    /// olmamalı ve `redaction_state` kapalı kümede olmalı. Doğrulama düşerse
    /// `exports` satırı da yazılmaz (tek işlem).
    #[allow(clippy::too_many_arguments)] // alanlar tek tek anlamlı; struct sarmak çağrıyı gizler
    pub fn record_export(
        &self,
        case_id: &str,
        export_type: &str,
        template_version: Option<&str>,
        items: &[NewExportItem],
        destination_kind: &str,
        destination_path: &str,
        output_hash: &str,
    ) -> Result<ExportRow, StorageError> {
        let exists: i64 = self.conn().query_row(
            "SELECT count(*) FROM cases WHERE id=?1 AND deleted_at IS NULL",
            [case_id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(StorageError::NotFound);
        }
        let template_version = normalize_meta(
            template_version,
            MAX_TEMPLATE_VERSION_CHARS,
            "şablon sürümü",
        )?;

        // Kalemler **yazmadan önce** doğrulanır: yanlış iz sicile düşmez.
        let mut source_ids: Vec<String> = Vec::with_capacity(items.len());
        for item in items {
            let version_id = item.source_version_id.trim();
            if version_id.is_empty() {
                return Err(StorageError::ExportSourceInvalid {
                    reason: "kaynak sürüm kimliği boş".into(),
                });
            }
            if source_ids.iter().any(|seen| seen == version_id) {
                return Err(StorageError::ExportSourceInvalid {
                    reason: format!("aynı kaynak sürüm iki kez yazılmış: {version_id}"),
                });
            }
            let owner: Option<String> = self
                .conn()
                .query_row(
                    "SELECT case_id FROM document_versions WHERE id=?1",
                    [version_id],
                    |row| row.get(0),
                )
                .optional()?;
            match owner {
                None => {
                    return Err(StorageError::ExportSourceInvalid {
                        reason: format!("kaynak sürüm bulunamadı: {version_id}"),
                    })
                }
                Some(owner_case) if owner_case != case_id => {
                    return Err(StorageError::ExportSourceInvalid {
                        reason: "kaynak sürüm başka dosyaya ait".into(),
                    })
                }
                Some(_) => {}
            }
            if !REDACTION_STATES.contains(&item.redaction_state.as_str()) {
                return Err(StorageError::ExportSourceInvalid {
                    reason: format!("bilinmeyen maskeleme durumu: {}", item.redaction_state),
                });
            }
            // Meta alanları burada doğrulanır ki hata, alanı adlandırsın.
            normalize_meta(
                item.output_page_range.as_deref(),
                MAX_PAGE_RANGE_CHARS,
                "sayfa aralığı",
            )?;
            normalize_meta(
                item.citation_label.as_deref(),
                MAX_CITATION_LABEL_CHARS,
                "atıf etiketi",
            )?;
            source_ids.push(version_id.to_string());
        }

        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        // Boş hedef yol kayda `NULL` olarak girer: "" bir konum değildir ve
        // arayüzde boş bir "hedef" satırı uydurulmuş olurdu.
        let destination = destination_path.trim();
        let destination_path = if destination.is_empty() {
            None
        } else {
            Some(destination)
        };
        // `included_ids` **kalemlerden** türetilir: iki liste ayrışamaz. Eski
        // sürüm boş `[]` yazıyordu ve sicil "hangi sürümler" sorusunu
        // yanıtlayamıyordu (ölçülen kusur, 2026-11-10).
        let included = serde_json::to_string(&source_ids).unwrap_or_else(|_| "[]".into());

        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "INSERT INTO exports (id,case_id,type,included_ids,template_version,status,destination_kind,destination_path,output_hash,created_at,completed_at)
             VALUES (?1,?2,?3,?4,?5,'ready',?6,?7,?8,?9,?9)",
            params![
                id,
                case_id,
                export_type,
                included,
                template_version,
                destination_kind,
                destination_path,
                output_hash,
                now
            ],
        )?;
        for item in items {
            tx.execute(
                "INSERT INTO export_items (id,export_id,source_version_id,output_page_range,citation_label,redaction_state)
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    uuid::Uuid::now_v7().to_string(),
                    id,
                    item.source_version_id.trim(),
                    normalize_meta(
                        item.output_page_range.as_deref(),
                        MAX_PAGE_RANGE_CHARS,
                        "sayfa aralığı"
                    )?,
                    normalize_meta(
                        item.citation_label.as_deref(),
                        MAX_CITATION_LABEL_CHARS,
                        "atıf etiketi"
                    )?,
                    item.redaction_state,
                ],
            )?;
        }
        tx.commit()?;

        self.append_audit(
            "export",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({
                "type": export_type,
                "destination": destination_kind,
                "path": destination_path,
                "template": template_version,
                // Kaynak **kimlikleri** denetim kaydına yazılmaz (sicil zaten
                // tutar); yalnız ölçüm taşınır.
                "sourceItems": items.len(),
            }),
        )?;
        self.get_export(&id)?.ok_or(StorageError::NotFound)
    }

    /// Dışa aktarma **tamamlanamadı**: kayıt `ready` kalırsa "paket hazır" der
    /// ama ortada dosya yoktur.
    ///
    /// Sıra bilinçlidir — kayıt **önce** yazılır (denetlenemeyen aktarım
    /// yapılmaz), bu yüzden yazma sonrası düşen adım kaydı `failed` işaretler.
    /// Durum yalnız `ready`den düşer: tamamlanmış bir kayıt geriye alınmaz.
    pub fn fail_export(&self, id: &str, reason: &str) -> Result<(), StorageError> {
        let changed = self.conn().execute(
            "UPDATE exports SET status='failed' WHERE id=?1 AND status='ready'",
            [id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit(
            "export",
            Some(id),
            "fail",
            "local-user",
            &serde_json::json!({"reason": reason}),
        )?;
        Ok(())
    }

    pub fn get_export(&self, id: &str) -> Result<Option<ExportRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT e.id,e.case_id,e.type,e.status,e.destination_kind,e.destination_path,e.output_hash,e.template_version,e.created_at,e.completed_at,
                    (SELECT count(*) FROM export_items x WHERE x.export_id=e.id)
             FROM exports e WHERE e.id=?1",
        )?;
        Ok(stmt.query_row([id], map_export).optional()?)
    }

    /// Dosyanın dışa aktarma kayıtları, **en yeni önce**. Aynı milisaniyede
    /// yazılan iki kayıt `created_at` ile ayırt edilemez; `rowid` ikinci anahtar
    /// olarak kullanılır (ekranda sıra kararlı olsun, son kayıt üstte dursun).
    pub fn list_exports(&self, case_id: &str) -> Result<Vec<ExportRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT e.id,e.case_id,e.type,e.status,e.destination_kind,e.destination_path,e.output_hash,e.template_version,e.created_at,e.completed_at,
                    (SELECT count(*) FROM export_items x WHERE x.export_id=e.id)
             FROM exports e WHERE e.case_id=?1 ORDER BY e.created_at DESC, e.rowid DESC",
        )?;
        let rows = stmt.query_map([case_id], map_export)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Bir çıktının **kaynak sürüm kalemleri**, yazılma sırasında.
    ///
    /// Sürüm silinmişse (şemada FK yok, iz kalır) belge adı/sürüm no `null`
    /// döner; kimlik korunur.
    pub fn list_export_items(&self, export_id: &str) -> Result<Vec<ExportItemRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT ei.id,ei.export_id,ei.source_version_id,dv.document_id,doc.display_name,dv.version_no,
                    ei.output_page_range,ei.citation_label,ei.redaction_state
             FROM export_items ei
             LEFT JOIN document_versions dv ON dv.id=ei.source_version_id
             LEFT JOIN documents doc ON doc.id=dv.document_id
             WHERE ei.export_id=?1 ORDER BY ei.rowid ASC",
        )?;
        let rows = stmt.query_map([export_id], |row| {
            Ok(ExportItemRow {
                id: row.get(0)?,
                export_id: row.get(1)?,
                source_version_id: row.get(2)?,
                document_id: row.get(3)?,
                document_title: row.get(4)?,
                version_no: row.get(5)?,
                output_page_range: row.get(6)?,
                citation_label: row.get(7)?,
                redaction_state: row.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

fn map_export(row: &Row<'_>) -> rusqlite::Result<ExportRow> {
    Ok(ExportRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        export_type: row.get(2)?,
        status: row.get(3)?,
        destination_kind: row.get(4)?,
        destination_path: row.get(5)?,
        output_hash: row.get(6)?,
        template_version: row.get(7)?,
        created_at: row.get(8)?,
        completed_at: row.get(9)?,
        item_count: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn db_with_case() -> (Database, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([43u8; 32])).expect("db");
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

    /// Belge + sürüm: kaynak izi testlerinin girdisi.
    fn seed_version(db: &Database, case_id: &str, name: &str) -> String {
        // `storage_id` UNIQUE: her fikstür kendi kimliğini üretir.
        let storage_id = format!("st-export-{}", uuid::Uuid::now_v7());
        let blob = db.create_blob_record(&storage_id, 16, "aad").expect("blob");
        let doc = db
            .create_document(case_id, name, "pdf", None, "unknown")
            .expect("doc");
        db.add_version(
            &doc.id,
            case_id,
            &blob,
            &"9".repeat(64),
            16,
            "application/pdf",
        )
        .expect("version")
        .id
    }

    fn item(version_id: &str, redaction_state: &str) -> NewExportItem {
        NewExportItem {
            source_version_id: version_id.to_string(),
            output_page_range: None,
            citation_label: None,
            redaction_state: redaction_state.to_string(),
        }
    }

    #[test]
    fn export_is_recorded_with_hash() {
        let (db, case_id) = db_with_case();
        let row = db
            .record_export(
                &case_id,
                "json",
                Some("dim-case-export@1"),
                &[],
                "user_selected_path",
                "/Users/avukat/Cikti/kira.json",
                &"a".repeat(64),
            )
            .expect("kayıt");
        assert_eq!(row.status, "ready");
        assert_eq!(row.output_hash.as_deref(), Some("a".repeat(64).as_str()));
        assert_eq!(row.template_version.as_deref(), Some("dim-case-export@1"));
        assert_eq!(row.item_count, 0, "kaynak kalemi yok — uydurulmaz");
        assert_eq!(db.list_exports(&case_id).expect("liste").len(), 1);
    }

    /// **Ölçülen kusur (2026-11-10):** `exports.included_ids` her çağrıda `[]`
    /// yazılıyor, `export_items` hiç yazılmıyordu → "bu rapor hangi belge
    /// sürümlerinden üretildi" yanıtsızdı. Şimdi kalemler yazılır ve liste
    /// belge künyesiyle okunur.
    #[test]
    fn export_records_source_versions_with_document_titles() {
        let (db, case_id) = db_with_case();
        let first = seed_version(&db, &case_id, "Kira sözleşmesi");
        let second = seed_version(&db, &case_id, "İhtarname");
        let row = db
            .record_export(
                &case_id,
                "pdf",
                Some("case-report@1"),
                &[item(&first, "none"), item(&second, "masked")],
                "user_selected_path",
                "/tmp/rapor.pdf",
                &"b".repeat(64),
            )
            .expect("kayıt");
        assert_eq!(row.item_count, 2);
        let items = db.list_export_items(&row.id).expect("kalemler");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].source_version_id, first);
        assert_eq!(items[0].document_title.as_deref(), Some("Kira sözleşmesi"));
        assert_eq!(items[0].version_no, Some(1));
        assert_eq!(items[0].redaction_state, "none");
        assert_eq!(items[1].redaction_state, "masked");
        // `included_ids` kalemlerden türetilir: iki liste ayrışamaz.
        let included_raw: String = db
            .conn()
            .query_row(
                "SELECT included_ids FROM exports WHERE id=?1",
                [&row.id],
                |r| r.get(0),
            )
            .expect("included");
        let included: Vec<String> = serde_json::from_str(&included_raw).expect("json");
        assert_eq!(included, vec![first, second]);
    }

    /// Bilinmeyen / başka dosyaya ait sürüm **hiçbir satır yazdırmaz**: yanlış
    /// iz, izsizlikten kötüdür. Hata kodu kararlıdır (`E_EXPORT_SOURCE`).
    #[test]
    fn foreign_or_unknown_source_version_is_rejected_without_rows() {
        let (db, case_id) = db_with_case();
        let mine = seed_version(&db, &case_id, "Benim");
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
            .expect("dosya");
        let foreign = seed_version(&db, &other.id, "Yabancı");

        for (bad, expected) in [
            ("yok-boyle-surum", "kaynak sürüm bulunamadı"),
            (foreign.as_str(), "başka dosyaya ait"),
        ] {
            let error = db
                .record_export(
                    &case_id,
                    "json",
                    None,
                    &[item(&mine, "none"), item(bad, "none")],
                    "user_selected_path",
                    "/tmp/kira.json",
                    &"c".repeat(64),
                )
                .expect_err("reddedilmeli");
            assert_eq!(error.code(), "E_EXPORT_SOURCE");
            assert!(
                error.to_string().contains(expected),
                "neden alanı adlandırmalı: {error}"
            );
            assert!(
                db.list_exports(&case_id).expect("liste").is_empty(),
                "doğrulama düşünce exports satırı da yazılmaz"
            );
            let items: i64 = db
                .conn()
                .query_row("SELECT count(*) FROM export_items", [], |r| r.get(0))
                .expect("sayım");
            assert_eq!(items, 0, "yarım kaynak izi kalmaz");
        }
    }

    #[test]
    fn duplicate_source_version_and_unknown_state_are_rejected() {
        let (db, case_id) = db_with_case();
        let version = seed_version(&db, &case_id, "Belge");
        let duplicate = db
            .record_export(
                &case_id,
                "json",
                None,
                &[item(&version, "none"), item(&version, "none")],
                "user_selected_path",
                "/tmp/a.json",
                &"d".repeat(64),
            )
            .expect_err("mükerrer reddedilir");
        assert_eq!(duplicate.code(), "E_EXPORT_SOURCE");

        let unknown = db
            .record_export(
                &case_id,
                "json",
                None,
                &[item(&version, "belki")],
                "user_selected_path",
                "/tmp/a.json",
                &"e".repeat(64),
            )
            .expect_err("bilinmeyen durum reddedilir");
        assert_eq!(unknown.code(), "E_EXPORT_SOURCE");
        assert!(db.list_exports(&case_id).expect("liste").is_empty());
    }

    /// Meta alanı sınırı ve kontrol karakteri: sessiz kırpma yok, kayıt düşer.
    #[test]
    fn overlong_template_or_page_range_is_rejected() {
        let (db, case_id) = db_with_case();
        let version = seed_version(&db, &case_id, "Belge");
        let long_template = "t".repeat(MAX_TEMPLATE_VERSION_CHARS + 1);
        assert_eq!(
            db.record_export(
                &case_id,
                "json",
                Some(&long_template),
                &[],
                "user_selected_path",
                "/tmp/a.json",
                &"f".repeat(64),
            )
            .expect_err("şablon sürümü sınırı")
            .code(),
            "E_EXPORT_SOURCE"
        );
        let mut weird = item(&version, "none");
        weird.output_page_range = Some("12-\u{7}14".into());
        assert_eq!(
            db.record_export(
                &case_id,
                "json",
                None,
                &[weird],
                "user_selected_path",
                "/tmp/a.json",
                &"f".repeat(64),
            )
            .expect_err("kontrol karakteri")
            .code(),
            "E_EXPORT_SOURCE"
        );
    }

    /// Boş meta alanları `NULL`'dır (boş dize bir değer değil) — uydurma yok.
    #[test]
    fn blank_meta_fields_become_null() {
        let (db, case_id) = db_with_case();
        let version = seed_version(&db, &case_id, "Belge");
        let mut only_blank = item(&version, "none");
        only_blank.output_page_range = Some("   ".into());
        only_blank.citation_label = Some("".into());
        let row = db
            .record_export(
                &case_id,
                "json",
                Some("   "),
                &[only_blank],
                "user_selected_path",
                "/tmp/a.json",
                &"a".repeat(64),
            )
            .expect("kayıt");
        assert_eq!(row.template_version, None);
        let items = db.list_export_items(&row.id).expect("kalem");
        assert_eq!(items[0].output_page_range, None);
        assert_eq!(items[0].citation_label, None);
    }

    /// Eski kayıtlar (kalemsiz) "kaynak var" gibi sunulmaz: `item_count` 0 ve
    /// liste boş döner — ekran bunu ayrı söyler.
    #[test]
    fn legacy_export_without_items_reports_zero() {
        let (db, case_id) = db_with_case();
        let row = db
            .record_export(
                &case_id,
                "csv",
                None,
                &[],
                "user_selected_path",
                "/tmp/a.csv",
                &"b".repeat(64),
            )
            .expect("kayıt");
        assert_eq!(row.item_count, 0);
        assert!(db.list_export_items(&row.id).expect("kalem").is_empty());
    }

    /// Denetim defteri **nereye** yazıldığını taşımalı: tür + hedef türü + zaman
    /// avukatın "hangi klasöre çıktı" sorusunu yanıtlamaz. Boş yol `null`'dır —
    /// uydurulmuş bir konum yazılmaz.
    #[test]
    fn export_destination_path_is_stored_and_blank_is_null() {
        let (db, case_id) = db_with_case();
        let with_path = db
            .record_export(
                &case_id,
                "pdf",
                None,
                &[],
                "user_selected_path",
                "  /Users/avukat/Masaüstü/kira.pdf  ",
                &"d".repeat(64),
            )
            .expect("kayıt");
        assert_eq!(
            with_path.destination_path.as_deref(),
            Some("/Users/avukat/Masaüstü/kira.pdf"),
            "yol kırpılarak saklanır"
        );

        let blank = db
            .record_export(
                &case_id,
                "csv",
                None,
                &[],
                "user_selected_path",
                "   ",
                &"e".repeat(64),
            )
            .expect("kayıt");
        assert_eq!(blank.destination_path, None, "boş yol konum değildir");
        // Okuma yolu da aynı değeri döndürmeli (INSERT ile SELECT ayrışmasın).
        let reread = db.get_export(&blank.id).expect("oku").expect("var");
        assert_eq!(reread.destination_path, None);
        assert_eq!(
            db.get_export(&with_path.id)
                .expect("oku")
                .expect("var")
                .destination_path
                .as_deref(),
            Some("/Users/avukat/Masaüstü/kira.pdf")
        );
    }

    /// Liste **en yeni önce** gelir; aynı milisaniyede yazılan iki kayıt
    /// `created_at` ile ayrılmadığı için `rowid` ikinci anahtar (ekranda son
    /// kayıt üstte durmalı, sıra koşular arasında kararlı olmalı).
    #[test]
    fn exports_are_listed_newest_first() {
        let (db, case_id) = db_with_case();
        let first = db
            .record_export(
                &case_id,
                "json",
                None,
                &[],
                "user_selected_path",
                "/tmp/a.json",
                &"1".repeat(64),
            )
            .expect("ilk");
        let second = db
            .record_export(
                &case_id,
                "csv",
                None,
                &[],
                "user_selected_path",
                "/tmp/b.csv",
                &"2".repeat(64),
            )
            .expect("ikinci");
        let listed = db.list_exports(&case_id).expect("liste");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, second.id, "son kayıt üstte");
        assert_eq!(listed[1].id, first.id);
        // Her satır kendi dosyasına kapsanır: başka dosyanın kaydı sızmaz.
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
            .expect("dosya");
        assert!(db.list_exports(&other.id).expect("liste").is_empty());
    }

    /// Aynı zaman damgasına düşen iki kayıt `created_at` ile ayrılmaz; ekranda
    /// sıra **kararlı** olmalı. İkinci anahtar `rowid`dir: sonra yazılan üstte.
    /// `now_iso` mikrosaniye yazdığı için gerçek koşuda eşitlik nadirdir — bu
    /// yüzden senaryo damga **eşitlenerek** zorlanır (yedekten geri yükleme /
    /// içe aktarma da eski damgaları korur).
    #[test]
    fn exports_with_equal_timestamp_keep_insertion_order() {
        let (db, case_id) = db_with_case();
        let first = db
            .record_export(
                &case_id,
                "json",
                None,
                &[],
                "user_selected_path",
                "/tmp/a.json",
                &"3".repeat(64),
            )
            .expect("ilk");
        let second = db
            .record_export(
                &case_id,
                "csv",
                None,
                &[],
                "user_selected_path",
                "/tmp/b.csv",
                &"4".repeat(64),
            )
            .expect("ikinci");
        db.conn()
            .execute(
                "UPDATE exports SET created_at=?1 WHERE case_id=?2",
                rusqlite::params![first.created_at.clone(), case_id],
            )
            .expect("damga eşitle");
        let listed = db.list_exports(&case_id).expect("liste");
        assert_eq!(listed[0].id, second.id, "aynı damgada sonra yazılan üstte");
        assert_eq!(listed[1].id, first.id);
    }

    #[test]
    fn unknown_case_rejected() {
        let (db, _) = db_with_case();
        assert!(db
            .record_export(
                "yok",
                "json",
                None,
                &[],
                "path",
                "/tmp/yok.json",
                &"b".repeat(64)
            )
            .is_err());
    }

    /// Paket üretilemediğinde kayıt `ready` kalamaz: "hazır" der ama ortada
    /// dosya olmaz. Durum `failed`e düşer, ikinci kez düşürülemez.
    #[test]
    fn failed_export_is_marked_and_not_reset() {
        let (db, case_id) = db_with_case();
        let row = db
            .record_export(
                &case_id,
                "package",
                Some("transfer-package@1"),
                &[],
                "user_selected_path",
                "/Users/avukat/Devir/DimDevir-202611061200",
                &"c".repeat(64),
            )
            .expect("kayıt");
        assert_eq!(row.status, "ready");

        db.fail_export(&row.id, "E_PACKAGE_SEAL").expect("işaret");
        let after = db.get_export(&row.id).expect("oku").expect("var");
        assert_eq!(after.status, "failed");
        // Yalnız `ready`den düşer: tamamlanmış kayıt geri alınmaz.
        assert!(db.fail_export(&row.id, "E_PACKAGE_SEAL").is_err());
        assert_eq!(
            db.get_export(&row.id).expect("oku").expect("var").status,
            "failed"
        );
    }
}
