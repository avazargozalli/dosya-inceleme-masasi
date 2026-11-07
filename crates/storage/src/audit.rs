//! Denetim zinciri (`audit_events`): **okuma** ve **bütünlük doğrulaması**.
//!
//! `append_audit` her kaydı öncekinin `event_hash`ine bağlar (F03). Bu modül
//! zinciri okur (`list_audit_events`) ve doğrular (`audit_chain_status`).
//! Doğrulama ürün yolunda da çağrılabilir: böylece "zincir kurcalanmış mı"
//! sorusu yalnız testte değil, çalışan uygulamada da yanıtlanır.
//!
//! Dürüstlük kuralları:
//! - `metadata_allowlist` çözülemezse satır **düşürülmez**, `metadata: null`
//!   döner (kayıt kaybolmaz; bozukluk görünür).
//! - Doğrulama **durum** döndürür (`intact`, kırılan kaydın kimliği ve nedeni);
//!   arayüz nedeni kendi dilinde yazar, çekirdek cümle uydurmaz.
//! - `checked` yalnız **doğrulanmış** kayıt sayısıdır: kopukluk varsa kopukluktan
//!   önceki kayıtlar sayılır (kırılan kayıt "doğrulandı" sayılmaz).

use rusqlite::Row;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{Database, StorageError};

/// Listeleme üst sınırı: denetim defteri büyür, tek istekte tümünü döndürmek
/// arayüzü kilitler. Sınır çekirdekte uygulanır (arayüz kırpamaz).
pub const MAX_AUDIT_EVENTS: usize = 200;

/// Varsayılan liste boyu (çağıran sınır vermezse).
pub const DEFAULT_AUDIT_EVENTS: usize = 50;

/// Tek denetim kaydı. `sequence` ekleme sırasıdır (AUTOINCREMENT); sıra
/// `timestamp`e göre değil **yazılma sırasına** göredir (aynı saniyede iki
/// kayıt olabilir).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventRow {
    pub sequence: i64,
    pub id: String,
    pub category: String,
    pub entity_id: Option<String>,
    pub operation: String,
    pub actor_label: String,
    /// `metadata_allowlist` çözümü; bozuksa `null` (satır yine de döner).
    pub metadata: Option<Value>,
    pub prior_hash: Option<String>,
    pub event_hash: String,
    pub timestamp: String,
}

/// Zincir doğrulamasının sonucu. `intact == false` iken `broken_event_id`
/// kurcalanmış/bozulmuş kaydı, `broken_reason` ise hangi bağın koptuğunu
/// söyler: `prior_hash` (önceki kayda bağ kopuk) ya da `event_hash`
/// (kaydın kendi özeti tutmuyor).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditChainStatus {
    pub checked: usize,
    pub intact: bool,
    pub broken_event_id: Option<String>,
    pub broken_reason: Option<String>,
}

/// Zincir yürüyüşünün ham sonucu (iç kullanım).
pub(crate) struct ChainWalk {
    pub(crate) checked: usize,
    pub(crate) broken: Option<ChainBreak>,
}

pub(crate) struct ChainBreak {
    pub(crate) event_id: String,
    /// `prior_hash` | `event_hash` — kararlı kod, cümle değil.
    pub(crate) reason: &'static str,
}

/// Zinciri baştan sona yürür ve ilk kopuklukta **durur**.
///
/// `verify_audit_chain` (hata döndüren eski sözleşme) ve `audit_chain_status`
/// (durum döndüren yeni sözleşme) **aynı** yürüyüşü paylaşır: hash kuralı tek
/// yerde durur, iki kopya zamanla ayrışamaz.
pub(crate) fn walk_chain(conn: &rusqlite::Connection) -> Result<ChainWalk, StorageError> {
    let mut stmt = conn.prepare(
        "SELECT id, category, entity_id, operation, actor_label, metadata_allowlist, prior_hash, event_hash, timestamp
         FROM audit_events ORDER BY sequence ASC",
    )?;
    let rows = stmt.query_map([], |row: &Row<'_>| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
        ))
    })?;

    let mut prior: Option<String> = None;
    let mut checked = 0usize;
    for row in rows {
        let (
            id,
            category,
            entity_id,
            operation,
            actor_label,
            metadata,
            prior_hash,
            event_hash,
            timestamp,
        ) = row?;
        if prior_hash != prior {
            return Ok(ChainWalk {
                checked,
                broken: Some(ChainBreak {
                    event_id: id,
                    reason: "prior_hash",
                }),
            });
        }
        // Kanonik JSON **append_audit ile birebir** aynı olmalı; alan adları
        // değişirse eski kayıtların özeti tutmaz.
        let canonical = serde_json::json!({
            "id": id.clone(),
            "category": category,
            "entityId": entity_id,
            "operation": operation,
            "actor": actor_label,
            "metadata": serde_json::from_str::<serde_json::Value>(&metadata).unwrap_or(serde_json::Value::Null),
            "priorHash": prior_hash,
            "timestamp": timestamp,
        });
        let mut hasher = Sha256::new();
        hasher.update(canonical.to_string().as_bytes());
        if hex::encode(hasher.finalize()) != event_hash {
            return Ok(ChainWalk {
                checked,
                broken: Some(ChainBreak {
                    event_id: id,
                    reason: "event_hash",
                }),
            });
        }
        prior = Some(event_hash);
        checked += 1;
    }
    Ok(ChainWalk {
        checked,
        broken: None,
    })
}

impl Database {
    /// En yeni kayıt önce (`sequence DESC`). `limit` **çekirdekte** kırpılır
    /// (`1..=MAX_AUDIT_EVENTS`); `category` verilirse yalnız o kategori.
    ///
    /// Sıra `timestamp`e göre değildir: aynı damgada yazılan kayıtlar bile
    /// yazılma sırasını korur (denetim defteri için doğru olan budur).
    pub fn list_audit_events(
        &self,
        limit: usize,
        category: Option<&str>,
    ) -> Result<Vec<AuditEventRow>, StorageError> {
        let limit = limit.clamp(1, MAX_AUDIT_EVENTS) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT sequence, id, category, entity_id, operation, actor_label, metadata_allowlist, prior_hash, event_hash, timestamp
             FROM audit_events
             WHERE (?1 IS NULL OR category = ?1)
             ORDER BY sequence DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![category, limit], |row: &Row<'_>| {
            let metadata: String = row.get(6)?;
            Ok(AuditEventRow {
                sequence: row.get(0)?,
                id: row.get(1)?,
                category: row.get(2)?,
                entity_id: row.get(3)?,
                operation: row.get(4)?,
                actor_label: row.get(5)?,
                metadata: serde_json::from_str::<Value>(&metadata).ok(),
                prior_hash: row.get(7)?,
                event_hash: row.get(8)?,
                timestamp: row.get(9)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Zincir bütünlüğü: kaç kayıt doğrulandı, kopukluk var mı, nerede.
    pub fn audit_chain_status(&self) -> Result<AuditChainStatus, StorageError> {
        let walk = walk_chain(&self.conn)?;
        Ok(AuditChainStatus {
            checked: walk.checked,
            intact: walk.broken.is_none(),
            broken_event_id: walk.broken.as_ref().map(|b| b.event_id.clone()),
            broken_reason: walk.broken.as_ref().map(|b| b.reason.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseKey;

    fn key() -> DatabaseKey {
        DatabaseKey::new([7u8; 32])
    }

    fn append(db: &Database, category: &str, operation: &str) -> String {
        db.append_audit(
            category,
            Some("e1"),
            operation,
            "local-user",
            &serde_json::json!({"n": 1}),
        )
        .expect("kayıt")
    }

    #[test]
    fn audit_events_are_listed_newest_first_and_limited() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        append(&db, "case", "create");
        append(&db, "document", "import");
        append(&db, "note", "update");

        let all = db.list_audit_events(10, None).expect("liste");
        assert_eq!(all.len(), 3);
        assert_eq!(
            all.iter().map(|r| r.operation.as_str()).collect::<Vec<_>>(),
            vec!["update", "import", "create"],
            "en yeni önce gelmeli"
        );
        assert!(all[0].sequence > all[1].sequence, "sequence azalmalı");

        let two = db.list_audit_events(2, None).expect("liste");
        assert_eq!(two.len(), 2);
        assert_eq!(two[0].operation, "update");
        assert_eq!(two[1].operation, "import");
        // Sıfır ve taşan sınır çekirdekte kırpılır: sessizce "hepsi" dönmez.
        assert_eq!(db.list_audit_events(0, None).expect("liste").len(), 1);
        assert_eq!(
            db.list_audit_events(usize::MAX, None).expect("liste").len(),
            3
        );
    }

    #[test]
    fn audit_events_can_be_filtered_by_category() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        append(&db, "case", "create");
        append(&db, "document", "import");
        append(&db, "case", "update");

        let cases = db.list_audit_events(10, Some("case")).expect("liste");
        assert_eq!(cases.len(), 2);
        assert!(cases.iter().all(|r| r.category == "case"));
        assert_eq!(cases[0].operation, "update");
        assert!(
            db.list_audit_events(10, Some("yok"))
                .expect("liste")
                .is_empty(),
            "olmayan kategori boş döner"
        );
    }

    #[test]
    fn broken_metadata_does_not_drop_the_row() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        db.conn()
            .execute(
                "INSERT INTO audit_events (id, category, entity_id, operation, actor_label, metadata_allowlist, prior_hash, event_hash, timestamp)
                 VALUES ('a1', 'case', NULL, 'create', 'local-user', '{bozuk', NULL, 'deadbeef', '2026-01-01T00:00:00.000Z')",
                [],
            )
            .expect("ekle");

        let rows = db.list_audit_events(10, None).expect("liste");
        assert_eq!(rows.len(), 1, "bozuk metadata satırı düşürmemeli");
        assert!(rows[0].metadata.is_none(), "çözülemeyen metadata null");
        assert_eq!(rows[0].event_hash, "deadbeef");
    }

    #[test]
    fn chain_status_is_intact_and_counts_verified_events() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        append(&db, "case", "create");
        append(&db, "document", "import");
        append(&db, "note", "update");

        let status = db.audit_chain_status().expect("durum");
        assert!(status.intact);
        assert_eq!(status.checked, 3);
        assert!(status.broken_event_id.is_none());
        assert!(status.broken_reason.is_none());
        // Eski sözleşme aynı sayıyı verir (tek yürüyüş paylaşılır).
        assert_eq!(db.verify_audit_chain().expect("zincir"), 3);
    }

    #[test]
    fn tampered_event_is_reported_as_status_instead_of_panic() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        append(&db, "case", "create");
        append(&db, "document", "import");
        append(&db, "note", "update");
        let ids: Vec<String> = db
            .list_audit_events(10, None)
            .expect("liste")
            .into_iter()
            .map(|r| r.id)
            .collect();
        let tampered = ids[1].clone(); // son kayıt değil: önceki bağ da kırılır

        db.conn()
            .execute(
                "UPDATE audit_events SET operation='silindi' WHERE id=?1",
                rusqlite::params![tampered],
            )
            .expect("kurcala");

        let status = db.audit_chain_status().expect("durum");
        assert!(!status.intact, "kurcalama görülmeli");
        // Kurcalanan kayıt **ikinci**; ondan önce yalnız birinci doğrulandı.
        assert_eq!(status.checked, 1, "kopukluktan önceki kayıtlar sayılır");
        assert_eq!(status.broken_reason.as_deref(), Some("event_hash"));
        assert_eq!(status.broken_event_id.as_deref(), Some(tampered.as_str()));
        // Eski sözleşme hata döndürmeye devam eder.
        assert!(db.verify_audit_chain().is_err());
    }

    #[test]
    fn removed_middle_event_breaks_the_prior_hash_link() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        append(&db, "case", "create");
        append(&db, "document", "import");
        append(&db, "note", "update");
        let ids: Vec<String> = db
            .list_audit_events(10, None)
            .expect("liste")
            .into_iter()
            .map(|r| r.id)
            .collect();
        // Ortadaki kaydı sil: üçüncü kaydın `prior_hash`i artık birinciye bağlı
        // değil → bağ kopukluğu (özet bozulması değil).
        db.conn()
            .execute(
                "DELETE FROM audit_events WHERE id=?1",
                rusqlite::params![ids[1]],
            )
            .expect("sil");

        let status = db.audit_chain_status().expect("durum");
        assert!(!status.intact);
        assert_eq!(status.broken_reason.as_deref(), Some("prior_hash"));
        assert_eq!(status.broken_event_id.as_deref(), Some(ids[0].as_str()));
        assert_eq!(status.checked, 1);
    }

    #[test]
    fn empty_chain_is_intact_with_zero_events() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        let status = db.audit_chain_status().expect("durum");
        assert!(status.intact);
        assert_eq!(status.checked, 0);
        assert!(db.list_audit_events(10, None).expect("liste").is_empty());
    }
}
