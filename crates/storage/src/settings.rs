//! Kasa ayarları — şema `settings` (`scope`, `key`, `typed_value`, `version`).
//!
//! Neden bu modül: şartname (tablo sözleşmesi) "kasa ayarı ile cihaz/UI ayarı
//! **ayrı** depolanır" der ve tablo şemanın ilk sürümünden beri var; ama hiçbir
//! SQL onu okumuyor/yazmıyordu (`git grep`: tek çağrı yok). Sonuç: kasa
//! politikası sayılan otomatik yedek hedefi yalnız WebView `localStorage`'ında
//! duruyordu — kasa başka bir OS profilinde açıldığında ayar görünmüyor,
//! uygulama verisi silinince yedek sessizce kapanıyordu.
//!
//! Kurallar:
//! - `scope` **kapalı küme** ([`SETTING_SCOPES`], tablo CHECK'iyle aynı).
//! - `key` kırpılır; 1..=[`MAX_SETTING_KEY`] ve yalnız `[a-z0-9._-]`. Boş ya da
//!   biçimsiz anahtar yazılmaz (yazım hatası sessizce ikinci bir ayar açmaz).
//! - Değer **JSON** olarak saklanır; `null` ve [`MAX_SETTING_VALUE_BYTES`] üstü
//!   reddedilir. Okurken çözülemeyen değer **hata**dır: "ayar yok" denmez.
//! - Aynı anahtar yeniden yazılırsa `version` artar, `updated_at` tazelenir.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

/// Tablodaki CHECK kısıtıyla **aynı** kapalı küme. Bilinmeyen kapsam yazılmaz.
pub const SETTING_SCOPES: [&str; 3] = ["vault", "device", "ui"];

/// Anahtar üst sınırı (kırpılmış uzunluk).
pub const MAX_SETTING_KEY: usize = 64;

/// Tek ayar değeri için üst sınır: ayarlar küçüktür; kasaya büyük blob yazılmaz.
pub const MAX_SETTING_VALUE_BYTES: usize = 8 * 1024;

/// Liste üst sınırı (sessizce sınırsız liste dönmez).
pub const MAX_SETTING_LIST: usize = 200;

/// `settings` tablosunun okunmuş satırı. Alan adları arayüzde camelCase'tir.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingRow {
    pub scope: String,
    pub key: String,
    /// Çözülmüş JSON değeri (saklanan `typed_value` metni değil).
    pub value: serde_json::Value,
    /// Kaç kez yazıldığı; ilk yazım 1'dir.
    pub version: i64,
    pub updated_at: String,
}

fn normalize_scope(scope: &str) -> Result<&str, StorageError> {
    let trimmed = scope.trim();
    if SETTING_SCOPES.contains(&trimmed) {
        Ok(trimmed)
    } else {
        Err(StorageError::SettingInvalid {
            reason: format!("bilinmeyen kapsam: {scope}"),
        })
    }
}

fn normalize_key(key: &str) -> Result<&str, StorageError> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(StorageError::SettingInvalid {
            reason: "ayar anahtarı boş olamaz".to_string(),
        });
    }
    if trimmed.len() > MAX_SETTING_KEY {
        return Err(StorageError::SettingInvalid {
            reason: format!("ayar anahtarı {MAX_SETTING_KEY} karakterden uzun"),
        });
    }
    let valid = trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'));
    if !valid {
        return Err(StorageError::SettingInvalid {
            reason: format!("ayar anahtarı yalnız [a-z0-9._-] içerebilir: {trimmed}"),
        });
    }
    Ok(trimmed)
}

/// Değeri saklanacak JSON metnine çevirir. `null` ve aşırı büyük değer yazılmaz.
fn encode_value(value: &serde_json::Value) -> Result<String, StorageError> {
    if value.is_null() {
        return Err(StorageError::SettingInvalid {
            reason: "ayar değeri boş (null) olamaz".to_string(),
        });
    }
    let encoded = serde_json::to_string(value).map_err(|error| StorageError::SettingInvalid {
        reason: format!("ayar değeri JSON'a çevrilemedi: {error}"),
    })?;
    if encoded.len() > MAX_SETTING_VALUE_BYTES {
        return Err(StorageError::SettingInvalid {
            reason: format!("ayar değeri {MAX_SETTING_VALUE_BYTES} bayttan büyük"),
        });
    }
    Ok(encoded)
}

/// Saklanan metni çözer. Bozuk metin **hata**dır: sessizce "ayar yok" denmez
/// (aksi hâlde bozuk bir kasa ayarı ayarın hiç olmadığı gibi okunurdu).
fn decode_row(row: &Row<'_>) -> rusqlite::Result<SettingRow> {
    let scope: String = row.get(0)?;
    let key: String = row.get(1)?;
    let raw: String = row.get(2)?;
    let version: i64 = row.get(3)?;
    let updated_at: String = row.get(4)?;
    let value = serde_json::from_str(&raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(SettingRow {
        scope,
        key,
        value,
        version,
        updated_at,
    })
}

impl Database {
    /// Tek ayarı okur. Kayıt yoksa `None`; **bozuk değer** `SettingInvalid`dır.
    pub fn get_setting(&self, scope: &str, key: &str) -> Result<Option<SettingRow>, StorageError> {
        let scope = normalize_scope(scope)?;
        let key = normalize_key(key)?;
        let row = self
            .conn()
            .query_row(
                "SELECT scope, key, typed_value, version, updated_at
                 FROM settings WHERE scope = ?1 AND key = ?2",
                rusqlite::params![scope, key],
                decode_row,
            )
            .optional();
        match row {
            Ok(Some(row)) => Ok(Some(row)),
            Ok(None) => Ok(None),
            Err(rusqlite::Error::FromSqlConversionFailure(_, _, _)) => {
                Err(StorageError::SettingInvalid {
                    reason: format!("{scope}.{key} değeri JSON değil"),
                })
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Ayarı yazar ve **geri okur**. Aynı anahtar ikinci kez yazılırsa sürüm artar.
    pub fn set_setting(
        &self,
        scope: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<SettingRow, StorageError> {
        let scope = normalize_scope(scope)?;
        let key = normalize_key(key)?;
        let encoded = encode_value(value)?;
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO settings (scope, key, typed_value, version, updated_at)
             VALUES (?1, ?2, ?3, 1, ?4)
             ON CONFLICT(scope, key) DO UPDATE SET
                 typed_value = excluded.typed_value,
                 version = settings.version + 1,
                 updated_at = excluded.updated_at",
            rusqlite::params![scope, key, encoded, now],
        )?;
        self.get_setting(scope, key)?.ok_or(StorageError::NotFound)
    }

    /// Ayarı siler; **satır var mıydı** bilgisini döndürür (sessiz yutma yok).
    pub fn delete_setting(&self, scope: &str, key: &str) -> Result<bool, StorageError> {
        let scope = normalize_scope(scope)?;
        let key = normalize_key(key)?;
        let removed = self.conn().execute(
            "DELETE FROM settings WHERE scope = ?1 AND key = ?2",
            rusqlite::params![scope, key],
        )?;
        Ok(removed > 0)
    }

    /// Bir kapsamdaki ayarları **anahtar sırasıyla** listeler.
    pub fn list_settings(&self, scope: &str) -> Result<Vec<SettingRow>, StorageError> {
        let scope = normalize_scope(scope)?;
        let mut stmt = self.conn().prepare(
            "SELECT scope, key, typed_value, version, updated_at
             FROM settings WHERE scope = ?1 ORDER BY key ASC LIMIT ?2",
        )?;
        let limit = i64::try_from(MAX_SETTING_LIST).unwrap_or(200);
        let mut out = Vec::new();
        for row in stmt.query_map(rusqlite::params![scope, limit], decode_row)? {
            // Bozuk satır **atlanmaz**: liste "eksik ama doğru" görünmesin.
            let row = row.map_err(|error| match error {
                rusqlite::Error::FromSqlConversionFailure(_, _, _) => {
                    StorageError::SettingInvalid {
                        reason: format!("{scope} kapsamında okunamayan ayar değeri"),
                    }
                }
                other => other.into(),
            })?;
            out.push(row);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseKey;

    fn db() -> Database {
        Database::open_in_memory(&DatabaseKey::new([57u8; 32])).expect("db")
    }

    #[test]
    fn setting_round_trips_and_bumps_version() {
        let db = db();
        let first = db
            .set_setting(
                "vault",
                "backup.schedule",
                &serde_json::json!({ "destination": "/Volumes/Yedek", "days": 7 }),
            )
            .expect("yazım");
        assert_eq!(first.scope, "vault");
        assert_eq!(first.key, "backup.schedule");
        assert_eq!(first.version, 1);
        assert_eq!(first.value["destination"], "/Volumes/Yedek");
        assert_eq!(first.value["days"], 7);

        let second = db
            .set_setting(
                "vault",
                "backup.schedule",
                &serde_json::json!({ "destination": "/Volumes/Yeni", "days": 3 }),
            )
            .expect("ikinci yazım");
        assert_eq!(
            second.version, 2,
            "aynı anahtar ikinci kez yazılınca sürüm artar"
        );
        assert_eq!(second.value["destination"], "/Volumes/Yeni");

        let read = db
            .get_setting("vault", "backup.schedule")
            .expect("okuma")
            .expect("kayıt var");
        assert_eq!(read, second, "yazılan değer geri okunur");
    }

    #[test]
    fn scope_and_key_are_validated() {
        let db = db();
        // Kapsam kırpılır: `" device "` geçerlidir, `"kasa"` değildir.
        db.set_setting(" device ", "tema", &serde_json::json!("koyu"))
            .expect("kırpılmış kapsam");
        let err = db
            .set_setting("kasa", "tema", &serde_json::json!("koyu"))
            .expect_err("bilinmeyen kapsam");
        assert!(matches!(err, StorageError::SettingInvalid { .. }));

        for key in [
            "",
            "   ",
            "Backup.Schedule",
            "backup schedule",
            "backup/../x",
        ] {
            let err = db
                .set_setting("vault", key, &serde_json::json!(1))
                .expect_err("bozuk anahtar");
            assert!(
                matches!(err, StorageError::SettingInvalid { .. }),
                "{key} reddedilmeli"
            );
        }
        let long = "a".repeat(MAX_SETTING_KEY + 1);
        let err = db
            .set_setting("vault", &long, &serde_json::json!(1))
            .expect_err("uzun anahtar");
        assert!(matches!(err, StorageError::SettingInvalid { .. }));

        // Okuma yolu da aynı kapıdan geçer: bozuk anahtar okuma sanılmaz.
        assert!(matches!(
            db.get_setting("vault", "Yok").expect_err("okuma reddi"),
            StorageError::SettingInvalid { .. }
        ));
    }

    #[test]
    fn value_must_be_non_null_json_and_small() {
        let db = db();
        let err = db
            .set_setting("vault", "bos", &serde_json::Value::Null)
            .expect_err("null değer yazılmaz");
        assert!(matches!(err, StorageError::SettingInvalid { .. }));

        let big = "x".repeat(MAX_SETTING_VALUE_BYTES);
        let err = db
            .set_setting("vault", "buyuk", &serde_json::json!({ "blob": big }))
            .expect_err("büyük değer yazılmaz");
        assert!(matches!(err, StorageError::SettingInvalid { .. }));
        assert!(db.list_settings("vault").expect("liste").is_empty());

        // Skaler değerler de geçerlidir: ayar tipi çağıranın sözleşmesidir.
        db.set_setting("vault", "sayi", &serde_json::json!(3))
            .expect("skaler");
        assert_eq!(db.get_setting("vault", "sayi").unwrap().unwrap().value, 3);
    }

    #[test]
    fn delete_reports_whether_a_row_existed() {
        let db = db();
        db.set_setting(
            "vault",
            "backup.schedule",
            &serde_json::json!({ "days": 7 }),
        )
        .expect("yazım");
        assert!(db.delete_setting("vault", "backup.schedule").expect("sil"));
        assert!(!db.delete_setting("vault", "backup.schedule").expect("yok"));
        assert!(db
            .get_setting("vault", "backup.schedule")
            .unwrap()
            .is_none());
    }

    #[test]
    fn list_is_scoped_and_ordered_by_key() {
        let db = db();
        db.set_setting("vault", "z.son", &serde_json::json!(1))
            .expect("z");
        db.set_setting("vault", "a.ilk", &serde_json::json!(2))
            .expect("a");
        db.set_setting("device", "m.cihaz", &serde_json::json!(3))
            .expect("cihaz");
        let vault = db.list_settings("vault").expect("liste");
        let keys: Vec<&str> = vault.iter().map(|row| row.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["a.ilk", "z.son"],
            "anahtar sırası; cihaz kapsamı karışmaz"
        );
        assert_eq!(db.list_settings("ui").expect("ui").len(), 0);
    }

    #[test]
    fn corrupt_stored_value_is_reported_not_hidden() {
        let db = db();
        // Bozuk değer doğrudan yazılır (uygulama yolu bunu üretemez).
        db.conn()
            .execute(
                "INSERT INTO settings (scope, key, typed_value, version, updated_at)
                 VALUES ('vault', 'bozuk', '{nope', 1, '2026-11-07T00:00:00Z')",
                [],
            )
            .expect("ham yazım");
        let err = db
            .get_setting("vault", "bozuk")
            .expect_err("bozuk değer hata olmalı");
        assert!(matches!(err, StorageError::SettingInvalid { .. }));
        assert!(
            db.list_settings("vault").is_err(),
            "liste bozuk satırı sessizce atlamaz"
        );
    }
}
