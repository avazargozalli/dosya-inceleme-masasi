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
        // v64: bozuk metadata artık **doğamaz** (kapı bunu reddeder). Burada
        // sınanan şey **okuyucu**: v64 öncesi satırı taklit etmek için doğum
        // kapısı bilinçli düşürülür.
        db.conn()
            .execute_batch("DROP TRIGGER audit_events_evidence_birth;")
            .expect("kapı bilinçli düşürüldü");
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
        drop_ledger_gates(&db);

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
        drop_ledger_gates(&db);
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

    /// v64 şema kapısını **bilinçli** düşürür: bu testlerde sınanan şey zincir
    /// **doğrulayıcısıdır** (kurcalamayı görüyor mu), şema kapısı değil.
    /// Kapının kendisi `audit_ledger_is_chained_at_birth_and_never_rewritten_or_erased`
    /// testinde ölçülür.
    fn drop_ledger_gates(db: &Database) {
        db.conn()
            .execute_batch(
                "DROP TRIGGER audit_events_evidence_frozen;
                 DROP TRIGGER audit_events_evidence_no_delete;",
            )
            .expect("kapı bilinçli düşürüldü");
    }

    #[test]
    fn empty_chain_is_intact_with_zero_events() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        let status = db.audit_chain_status().expect("durum");
        assert!(status.intact);
        assert_eq!(status.checked, 0);
        assert!(db.list_audit_events(10, None).expect("liste").is_empty());
    }

    /// Ham `audit_events` yazımı (ürün yolu `append_audit`; saldırı yolu bu).
    struct LedgerInsert<'a> {
        id: &'a str,
        category: &'a str,
        entity: Option<&'a str>,
        operation: &'a str,
        actor: &'a str,
        metadata: &'a str,
        prior: Option<&'a str>,
        event_hash: &'a str,
        stamp: &'a str,
    }

    impl<'a> LedgerInsert<'a> {
        fn new(id: &'a str, prior: &'a str, stamp: &'a str) -> Self {
            LedgerInsert {
                id,
                category: "case",
                entity: None,
                operation: "create",
                actor: "local-user",
                metadata: "{}",
                prior: Some(prior),
                event_hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                stamp,
            }
        }

        fn with(mut self, change: impl FnOnce(&mut Self)) -> Self {
            change(&mut self);
            self
        }

        fn run(&self, db: &Database) -> Result<usize, rusqlite::Error> {
            db.conn().execute(
                "INSERT INTO audit_events (id,category,entity_id,operation,actor_label,
                 metadata_allowlist,prior_hash,event_hash,timestamp)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                rusqlite::params![
                    self.id,
                    self.category,
                    self.entity,
                    self.operation,
                    self.actor,
                    self.metadata,
                    self.prior,
                    self.event_hash,
                    self.stamp
                ],
            )
        }
    }

    fn assert_gate(result: Result<usize, rusqlite::Error>, label: &str) {
        match result {
            Ok(_) => panic!("{label}: kapı yazmayı kabul etti"),
            Err(err) => assert!(
                err.to_string().contains("DIM_INTEGRITY:audit_events"),
                "{label}: beklenen kapı reddi değil: {err}"
            ),
        }
    }

    #[test]
    fn audit_ledger_is_chained_at_birth_and_never_rewritten_or_erased() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        append(&db, "case", "create");
        append(&db, "document", "import");
        let head = append(&db, "note", "update");
        let newest = db.list_audit_events(1, None).expect("liste")[0].id.clone();
        assert_eq!(
            db.verify_audit_chain().expect("zincir"),
            3,
            "meşru yol kapıdan geçer"
        );

        let ts = "2026-11-22T10:00:00.000000Z";
        let bad = "c".repeat(64);
        let hash = "d".repeat(64);
        let long_id = "e".repeat(65);

        // --- Doğum: biçim ve sınırlar (28 saldırı) ------------------------
        assert_gate(
            LedgerInsert::new("ev-01", &head, ts)
                .with(|r| r.id = "")
                .run(&db),
            "boş kimlik",
        );
        assert_gate(
            LedgerInsert::new("ev-02", &head, ts)
                .with(|r| r.id = " ev-02 ")
                .run(&db),
            "kırpılmamış kimlik",
        );
        assert_gate(
            LedgerInsert::new(long_id.as_str(), &head, ts).run(&db),
            "65 haneli kimlik",
        );
        assert_gate(
            LedgerInsert::new("ev-04", &head, ts)
                .with(|r| r.category = "")
                .run(&db),
            "boş kategori",
        );
        assert_gate(
            LedgerInsert::new("ev-05", &head, ts)
                .with(|r| r.category = "   ")
                .run(&db),
            "yalnız boşluk kategori",
        );
        assert_gate(
            LedgerInsert::new("ev-06", &head, ts)
                .with(|r| r.category = " case")
                .run(&db),
            "kırpılmamış kategori",
        );
        assert_gate(
            LedgerInsert::new("ev-07", &head, ts)
                .with(|r| r.operation = "")
                .run(&db),
            "boş işlem",
        );
        assert_gate(
            LedgerInsert::new("ev-08", &head, ts)
                .with(|r| r.operation = "create ")
                .run(&db),
            "kırpılmamış işlem",
        );
        assert_gate(
            LedgerInsert::new("ev-09", &head, ts)
                .with(|r| r.actor = "")
                .run(&db),
            "boş aktör",
        );
        assert_gate(
            LedgerInsert::new("ev-10", &head, ts)
                .with(|r| r.actor = " local-user")
                .run(&db),
            "kırpılmamış aktör",
        );
        assert_gate(
            LedgerInsert::new("ev-11", &head, ts)
                .with(|r| r.entity = Some(""))
                .run(&db),
            "boş metin entity",
        );
        assert_gate(
            LedgerInsert::new("ev-12", &head, ts)
                .with(|r| r.entity = Some(" case-1"))
                .run(&db),
            "kırpılmamış entity",
        );
        assert_gate(
            LedgerInsert::new("ev-13", &head, ts)
                .with(|r| r.metadata = "{")
                .run(&db),
            "bozuk JSON metadata",
        );
        assert_gate(
            LedgerInsert::new("ev-14", &head, ts)
                .with(|r| r.metadata = "[]")
                .run(&db),
            "metadata dizi",
        );
        assert_gate(
            LedgerInsert::new("ev-15", &head, ts)
                .with(|r| r.metadata = "null")
                .run(&db),
            "metadata null",
        );
        assert_gate(
            LedgerInsert::new("ev-16", &head, ts)
                .with(|r| r.metadata = "\"x\"")
                .run(&db),
            "metadata metin",
        );
        assert_gate(
            LedgerInsert::new("ev-17", &head, ts)
                .with(|r| {
                    r.event_hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                })
                .run(&db),
            "özet 63 hane",
        );
        assert_gate(
            LedgerInsert::new("ev-18", &head, ts)
                .with(|r| {
                    r.event_hash =
                        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
                })
                .run(&db),
            "özet büyük harf",
        );
        assert_gate(
            LedgerInsert::new("ev-19", &head, ts)
                .with(|r| {
                    r.event_hash =
                        "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ";
                })
                .run(&db),
            "özet hex değil",
        );
        assert_gate(
            LedgerInsert::new("ev-20", &head, ts)
                .with(|r| r.stamp = "dün")
                .run(&db),
            "damga 'dün'",
        );
        assert_gate(
            LedgerInsert::new("ev-21", &head, ts)
                .with(|r| r.stamp = "2026-11-22")
                .run(&db),
            "damga yalnız gün",
        );
        assert_gate(
            LedgerInsert::new("ev-22", &head, ts)
                .with(|r| r.stamp = "2026-02-30T10:00:00.000000Z")
                .run(&db),
            "damga olmayan gün",
        );
        assert_gate(
            LedgerInsert::new("ev-23", &head, ts)
                .with(|r| r.stamp = "2026-11-22T99:00:00.000000Z")
                .run(&db),
            "damga olmayan saat",
        );
        assert_gate(
            LedgerInsert::new("ev-24", &head, ts)
                .with(|r| r.stamp = "2026-11-22T10:00Z")
                .run(&db),
            "damga saniyesiz",
        );
        assert_gate(
            LedgerInsert::new("ev-24b", &head, ts)
                .with(|r| r.stamp = "2026-11-22 10:00:00.000000Z")
                .run(&db),
            "damga ayırıcısı boşluk",
        );
        assert_gate(
            LedgerInsert::new("ev-24c", &head, ts)
                .with(|r| r.stamp = "2026-11-22T10:00:60.000000Z")
                .run(&db),
            "damga artık saniye",
        );
        // --- Doğum: zincir bağı ------------------------------------------
        assert_gate(
            LedgerInsert::new("ev-25", &bad, ts).run(&db),
            "zincir kopuk prior",
        );
        assert_gate(
            LedgerInsert::new("ev-26", &head, ts)
                .with(|r| r.prior = None)
                .run(&db),
            "prior NULL (zincirde satır var)",
        );
        assert_gate(
            LedgerInsert::new("ev-27", &head, ts)
                .with(|r| {
                    r.prior =
                        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                })
                .run(&db),
            "prior 63 hane",
        );
        assert_gate(
            LedgerInsert::new("ev-28", &head, ts)
                .with(|r| {
                    r.prior =
                        Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
                })
                .run(&db),
            "prior büyük harf",
        );
        // Sıra: açıkça verilen düşük `sequence` satırı defterin önüne düşürür.
        let low_sequence = db.conn().execute(
            "INSERT INTO audit_events (sequence,id,category,entity_id,operation,
             actor_label,metadata_allowlist,prior_hash,event_hash,timestamp)
             VALUES (0,'ev-29','case',NULL,'create','local-user','{}',?1,?2,?3)",
            rusqlite::params![head, hash, ts],
        );
        assert_gate(low_sequence, "açık düşük sequence");
        // Boş defterde ilk satırın `prior_hash`i NULL olmalı.
        let fresh = Database::open_in_memory(&key()).expect("açılır");
        assert_gate(
            LedgerInsert::new("ev-30", "a", ts)
                .with(|r| r.prior = Some(hash.as_str()))
                .run(&fresh),
            "ilk satırda prior dolu",
        );
        assert_eq!(
            fresh.list_audit_events(10, None).expect("liste").len(),
            0,
            "reddedilen satır defterde kalmamalı"
        );

        // --- Doğmuş satır değişmez ---------------------------------------
        for (label, sql) in [
            ("işlem çevrildi", "operation='silindi'"),
            ("kategori çevrildi", "category='silinmis'"),
            ("aktör çevrildi", "actor_label='baska'"),
            ("metadata çevrildi", "metadata_allowlist='{\"x\":1}'"),
            (
                "özet çevrildi",
                "event_hash='eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee'",
            ),
            ("damga çevrildi", "timestamp='2030-01-01T00:00:00.000000Z'"),
            ("kimlik çevrildi", "id='ev-yeni'"),
            ("entity çevrildi", "entity_id='case-2'"),
        ] {
            let result = db.conn().execute(
                &format!("UPDATE audit_events SET {sql} WHERE id=?1"),
                [&newest],
            );
            assert_gate(result, label);
        }
        // No-op yazma takılmaz.
        db.conn()
            .execute(
                "UPDATE audit_events SET operation='update' WHERE id=?1",
                [&newest],
            )
            .expect("no-op güncelleme kabul edilmeli");
        // Defter silinemez.
        let before = db.list_audit_events(100, None).expect("liste").len();
        assert_gate(
            db.conn()
                .execute("DELETE FROM audit_events WHERE id=?1", [&newest]),
            "kayıt silindi",
        );
        assert_eq!(
            db.list_audit_events(100, None).expect("liste").len(),
            before,
            "silme denemesi defteri küçültmemeli"
        );
        // Reddedilen her saldırıdan sonra zincir hâlâ sağlam.
        assert_eq!(db.verify_audit_chain().expect("zincir"), 3);
    }
}
