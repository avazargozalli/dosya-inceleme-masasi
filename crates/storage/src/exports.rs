//! Dışa aktarma kayıtları. Bir dosyanın verisini kasa dışına çıkaran her eylem
//! ayrı kayda alınır; kaynak kimlikleri ve çıktı hash'i korunur.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

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
    pub created_at: String,
    pub completed_at: Option<String>,
}

impl Database {
    pub fn record_export(
        &self,
        case_id: &str,
        export_type: &str,
        included_ids: &[String],
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
        self.conn().execute(
            "INSERT INTO exports (id,case_id,type,included_ids,status,destination_kind,destination_path,output_hash,created_at,completed_at)
             VALUES (?1,?2,?3,?4,'ready',?5,?6,?7,?8,?8)",
            rusqlite::params![
                id,
                case_id,
                export_type,
                serde_json::to_string(included_ids).unwrap_or_else(|_| "[]".into()),
                destination_kind,
                destination_path,
                output_hash,
                now
            ],
        )?;
        self.append_audit(
            "export",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({
                "type": export_type,
                "destination": destination_kind,
                "path": destination_path,
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
            "SELECT id,case_id,type,status,destination_kind,destination_path,output_hash,created_at,completed_at
             FROM exports WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_export).optional()?)
    }

    /// Dosyanın dışa aktarma kayıtları, **en yeni önce**. Aynı milisaniyede
    /// yazılan iki kayıt `created_at` ile ayırt edilemez; `rowid` ikinci anahtar
    /// olarak kullanılır (ekranda sıra kararlı olsun, son kayıt üstte dursun).
    pub fn list_exports(&self, case_id: &str) -> Result<Vec<ExportRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,type,status,destination_kind,destination_path,output_hash,created_at,completed_at
             FROM exports WHERE case_id=?1 ORDER BY created_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map([case_id], map_export)?;
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
        created_at: row.get(7)?,
        completed_at: row.get(8)?,
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

    #[test]
    fn export_is_recorded_with_hash() {
        let (db, case_id) = db_with_case();
        let row = db
            .record_export(
                &case_id,
                "json",
                &["d1".into()],
                "user_selected_path",
                "/Users/avukat/Cikti/kira.json",
                &"a".repeat(64),
            )
            .expect("kayıt");
        assert_eq!(row.status, "ready");
        assert_eq!(row.output_hash.as_deref(), Some("a".repeat(64).as_str()));
        assert_eq!(db.list_exports(&case_id).expect("liste").len(), 1);
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
            .record_export("yok", "json", &[], "path", "/tmp/yok.json", &"b".repeat(64))
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
