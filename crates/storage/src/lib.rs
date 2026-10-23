//! Şifreli veri tabanı: SQLCipher, migrasyon, bütünlük ve yedek.
//!
//! Anahtar kasa ana anahtarından HKDF ile türetilir ve yalnız bu crate'e
//! `DatabaseKey` olarak verilir; `Debug` çıktısında görünmez. Açık metin
//! veritabanı dosyası hiçbir aşamada oluşturulmaz.

mod ai;
mod anchors;
mod cases;
mod claims;
mod documents;
mod error;
mod exports;
mod notes;
mod parsing;
mod precedents;
mod redactions;
mod tasks;
mod timeline;

pub use ai::{
    AiCandidateRow, AiContextItemRow, AiRunRow, CoverageReport, NewAiContextItem, NewAiRun,
    NewTransferConsent, ProviderProfileRow, TransferConsentRow,
};
pub use anchors::{AnchorLocation, AnchorRow, NewAnchor};
pub use cases::{AiPolicy, Case, CaseKind, CasePatch, CaseStatus, NewCase, Sensitivity};
pub use claims::{
    ClaimRelationRow, ClaimRow, EvidenceLinkRow, NewClaim, NewClaimRelation, NewEvidenceLink,
    NewReviewItem, ReviewDecisionRow, ReviewItemRow,
};
pub use documents::{Document, DocumentVersion, MAX_DOCUMENT_BYTES};
pub use error::StorageError;
pub use exports::ExportRow;
pub use notes::{NewNote, NoteRow};
pub use parsing::{
    settings_hash, BlobRef, NewPage, PageRow, ParseRun, SearchHit, SegmentBlock, SegmentGeometry,
};
pub use precedents::{
    CitationCheckRow, NewCitationCheck, NewPrecedent, NewPrecedentNote, PrecedentNoteRow,
    PrecedentRow,
};
pub use redactions::{NewRedactionPlan, RedactionPlanRow, RedactionRegionInput};
pub use tasks::{NewTask, TaskRow};
pub use timeline::{EventPatch, EventRow, EventSourceRow, NewEvent};

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use sha2::{Digest, Sha256};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use zeroize::Zeroizing;

/// Kasa ana anahtarından türetilmiş veritabanı anahtarı.
pub struct DatabaseKey(Zeroizing<[u8; 32]>);

impl std::fmt::Debug for DatabaseKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DatabaseKey(<redacted>)")
    }
}

impl DatabaseKey {
    #[must_use]
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// Kasa ana anahtarından bağımsız DB anahtarı türetir (HKDF-SHA256).
    pub fn derive_from_master(master: &[u8; 32], vault_id: &str) -> Result<Self, StorageError> {
        use hkdf::Hkdf;
        let hk = Hkdf::<Sha256>::new(Some(vault_id.as_bytes()), master);
        let mut out = [0u8; 32];
        hk.expand(b"dimvault/sqlcipher-key/v1", &mut out)
            .map_err(|_| StorageError::BadKey)?;
        Ok(Self(Zeroizing::new(out)))
    }

    fn hex(&self) -> String {
        hex::encode(&self.0[..])
    }
}

struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "0001_init",
        sql: include_str!("migrations/0001_init.sql"),
    },
    Migration {
        version: 2,
        name: "0002_fts",
        sql: include_str!("migrations/0002_fts.sql"),
    },
    Migration {
        version: 3,
        name: "0003_constraints",
        sql: include_str!("migrations/0003_constraints.sql"),
    },
    Migration {
        version: 4,
        name: "0004_page_geometry",
        sql: include_str!("migrations/0004_page_geometry.sql"),
    },
    Migration {
        version: 5,
        name: "0005_claim_relations",
        sql: include_str!("migrations/0005_claim_relations.sql"),
    },
];

pub const CURRENT_SCHEMA_VERSION: u32 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationOutcome {
    pub applied: Vec<u32>,
    pub current: u32,
}

/// Açılmış şifreli veritabanı. Tek yazıcı kuralı çağıran katmandadır; bu tip
/// ikinci bir yazıcı açmaya izin vermez (her `Database` tek `Connection`).
pub struct Database {
    conn: Connection,
    path: Option<PathBuf>,
    schema_version: u32,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("path", &self.path.as_ref().map(|p| p.file_name()))
            .field("schema_version", &self.schema_version)
            .finish()
    }
}

impl Database {
    /// Dosya tabanlı kasa veritabanını açar ve migrasyonları uygular.
    pub fn open(path: &Path, key: &DatabaseKey) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| StorageError::Open)?;
        apply_key_and_pragmas(&conn, key)?;
        let mut db = Self {
            conn,
            path: Some(path.to_path_buf()),
            schema_version: 0,
        };
        db.migrate()?;
        Ok(db)
    }

    /// Test ve geçici işler için bellek içi şifreli veritabanı.
    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory().map_err(|_| StorageError::Open)?;
        apply_key_and_pragmas(&conn, key)?;
        let mut db = Self {
            conn,
            path: None,
            schema_version: 0,
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Bekleyen migrasyonları sırayla uygular. Uygulanmış bir migrasyonun
    /// checksum'ı değişmişse durur: dosya sonradan düzenlenmiş demektir.
    pub fn migrate(&mut self) -> Result<MigrationOutcome, StorageError> {
        let tx = self.conn.transaction()?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
               version INTEGER PRIMARY KEY,
               name TEXT NOT NULL,
               checksum TEXT NOT NULL,
               applied_at TEXT NOT NULL
             );",
        )?;

        let mut applied = Vec::new();
        for migration in MIGRATIONS {
            let checksum = hex::encode(Sha256::digest(migration.sql.as_bytes()));
            let existing: Option<(String, String)> = tx
                .query_row(
                    "SELECT name, checksum FROM schema_migrations WHERE version = ?1",
                    [migration.version],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;

            match existing {
                Some((_, stored)) if stored != checksum => {
                    return Err(StorageError::MigrationChecksum {
                        version: migration.version,
                    });
                }
                Some(_) => continue,
                None => {
                    tx.execute_batch(migration.sql)
                        .map_err(|source| StorageError::Migration {
                            version: migration.version,
                            source,
                        })?;
                    tx.execute(
                        "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![
                            migration.version,
                            migration.name,
                            checksum,
                            now_iso()
                        ],
                    )?;
                    applied.push(migration.version);
                }
            }
        }
        tx.commit()?;

        let found: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )?;
        let found = u32::try_from(found).unwrap_or(u32::MAX);
        if found > CURRENT_SCHEMA_VERSION {
            return Err(StorageError::SchemaTooNew {
                found,
                expected: CURRENT_SCHEMA_VERSION,
            });
        }
        self.schema_version = found;
        Ok(MigrationOutcome {
            applied,
            current: found,
        })
    }

    /// SQLCipher sürümünü döndürür; şifreleme gerçekten etkin mi diye kontrol için.
    pub fn cipher_version(&self) -> Result<String, StorageError> {
        let v: String = self
            .conn
            .query_row("PRAGMA cipher_version", [], |r| r.get(0))?;
        Ok(v)
    }

    pub fn integrity_check(&self) -> Result<(), StorageError> {
        let result: String = self
            .conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if result == "ok" {
            Ok(())
        } else {
            Err(StorageError::Integrity(result))
        }
    }

    /// Tutarlı, şifreli yedek üretir. SQLCipher'ın `backup` API'si şifreli
    /// veritabanlarını desteklemediği için `sqlcipher_export` kullanılır; hedef
    /// dosya aynı anahtarla şifreli olarak oluşur ve düz metin yazılmaz.
    pub fn backup_to(&self, dest: &Path, key: &DatabaseKey) -> Result<(), StorageError> {
        if dest.exists() {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "yedek hedefi zaten var",
            )));
        }
        if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let path_literal = dest.to_string_lossy().replace('\'', "''");
        self.conn.execute_batch(&format!(
            "ATTACH DATABASE '{path_literal}' AS dim_backup KEY \"x'{}'\";",
            key.hex()
        ))?;
        let export = self
            .conn
            .execute_batch("SELECT sqlcipher_export('dim_backup');");
        let detach = self.conn.execute_batch("DETACH DATABASE dim_backup;");
        export?;
        detach?;
        Ok(())
    }

    /// Denetim zincirine kayıt ekler: her kayıt öncekinin hash'ini taşır.
    pub fn append_audit(
        &self,
        category: &str,
        entity_id: Option<&str>,
        operation: &str,
        actor_label: &str,
        metadata: &serde_json::Value,
    ) -> Result<String, StorageError> {
        let prior: Option<String> = self
            .conn
            .query_row(
                "SELECT event_hash FROM audit_events ORDER BY sequence DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let id = uuid::Uuid::now_v7().to_string();
        let timestamp = now_iso();
        let canonical = serde_json::json!({
            "id": id.clone(),
            "category": category,
            "entityId": entity_id,
            "operation": operation,
            "actor": actor_label,
            "metadata": metadata,
            "priorHash": prior,
            "timestamp": timestamp.clone(),
        });
        let mut hasher = Sha256::new();
        hasher.update(canonical.to_string().as_bytes());
        let event_hash = hex::encode(hasher.finalize());
        self.conn.execute(
            "INSERT INTO audit_events (id, category, entity_id, operation, actor_label, metadata_allowlist, prior_hash, event_hash, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                id,
                category,
                entity_id,
                operation,
                actor_label,
                metadata.to_string(),
                prior,
                event_hash,
                timestamp
            ],
        )?;
        Ok(event_hash)
    }

    /// Denetim zincirini baştan sona doğrular (hash kopukluğu = kurcalama).
    pub fn verify_audit_chain(&self) -> Result<usize, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category, entity_id, operation, actor_label, metadata_allowlist, prior_hash, event_hash, timestamp
             FROM audit_events ORDER BY sequence ASC",
        )?;
        let rows = stmt.query_map([], |row| {
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
        let mut count = 0usize;
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
                return Err(StorageError::Integrity(format!(
                    "denetim zinciri koptu: {id}"
                )));
            }
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
                return Err(StorageError::Integrity(format!(
                    "denetim kaydı bozulmuş: {id}"
                )));
            }
            prior = Some(event_hash);
            count += 1;
        }
        Ok(count)
    }
}

fn apply_key_and_pragmas(conn: &Connection, key: &DatabaseKey) -> Result<(), StorageError> {
    // `PRAGMA key` parametre kabul etmez; anahtar hex olarak verilir (alfabe sabit).
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key.hex()))
        .map_err(|_| StorageError::Open)?;
    // Anahtar doğrulaması **ilk** okumada yapılır. Pragma'lar önce çalışırsa
    // yanlış anahtar `E_DB_SQL` olarak sızabilir; kullanıcıya "anahtar yanlış"
    // demek için bu sıra korunmalıdır.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(|_| StorageError::BadKey)?;

    // `cipher_memory_security` tüm SQLCipher build'lerinde yok; eksikse
    // bağlantıyı düşürmemeli, yalnız bu sertleştirme atlanır.
    let _ = conn.execute_batch("PRAGMA cipher_memory_security = ON;");
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA secure_delete = ON;
         PRAGMA temp_store = MEMORY;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;",
    )?;
    let _ = conn.execute_batch("PRAGMA journal_mode = WAL;");
    Ok(())
}

#[must_use]
pub fn now_iso() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> DatabaseKey {
        DatabaseKey::new([7u8; 32])
    }

    #[test]
    fn fresh_database_migrates_to_current() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
        assert_eq!(
            MIGRATIONS.len(),
            usize::try_from(CURRENT_SCHEMA_VERSION).unwrap_or(0)
        );
    }

    #[test]
    fn migration_is_idempotent() {
        let mut db = Database::open_in_memory(&key()).expect("açılır");
        let outcome = db.migrate().expect("ikinci migrasyon");
        assert!(outcome.applied.is_empty());
        assert_eq!(outcome.current, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn sqlcipher_is_active() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        let version = db.cipher_version().expect("sürüm");
        assert!(
            version.contains("SQLCipher") || version.starts_with('4'),
            "cipher_version={version}"
        );
    }

    #[test]
    fn wrong_key_cannot_read_database() {
        let dir = tempfile::tempdir().expect("dizin");
        let path = dir.path().join("vault.db");
        {
            let db = Database::open(&path, &key()).expect("oluştur");
            db.conn().execute(
                "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
                 VALUES ('c1','Test','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                [],
            ).expect("ekle");
        }
        let err = Database::open(&path, &DatabaseKey::new([9u8; 32])).expect_err("yanlış anahtar");
        assert_eq!(err.code(), "E_DB_KEY");
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        let result = db.conn().execute(
            "INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','olmayan-dosya','Belge','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        );
        assert!(result.is_err());
    }

    #[test]
    fn fts_indexes_turkish_text_and_updates() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        db.conn().execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Kira','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,created_at)
             VALUES ('b1','aa11',1,10,'ref','2026-01-01T00:00:00Z');
             INSERT INTO documents (id,case_id,display_name,created_at,updated_at)
             VALUES ('d1','c1','Kira sozlesmesi','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO document_versions (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at)
             VALUES ('v1','d1','c1',1,'b1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',10,'application/pdf','2026-01-01T00:00:00Z');
             INSERT INTO parse_runs (id,document_version_id,case_id,engine,engine_version,settings_hash,status,created_at)
             VALUES ('p1','v1','c1','pymupdf','1.24','hash','succeeded','2026-01-01T00:00:00Z');
             INSERT INTO text_segments (id,parse_run_id,case_id,raw_text,normalized_text,created_at)
             VALUES ('s1','p1','c1','İtirazın reddine karar verildi','itirazin reddine karar verildi','2026-01-01T00:00:00Z');",
        ).expect("ekle");

        let hits: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM text_segments_fts WHERE text_segments_fts MATCH 'itiraz*'",
                [],
                |r| r.get(0),
            )
            .expect("arama");
        assert_eq!(hits, 1);

        db.conn()
            .execute("DELETE FROM text_segments WHERE id='s1'", [])
            .expect("sil");
        let after: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM text_segments_fts WHERE text_segments_fts MATCH 'itiraz*'",
                [],
                |r| r.get(0),
            )
            .expect("arama");
        assert_eq!(after, 0);
    }

    #[test]
    fn audit_chain_detects_tamper() {
        let db = Database::open_in_memory(&key()).expect("açılır");
        db.append_audit(
            "case",
            Some("c1"),
            "create",
            "local-user",
            &serde_json::json!({"title":"Kira"}),
        )
        .expect("kayıt");
        db.append_audit(
            "document",
            Some("d1"),
            "import",
            "local-user",
            &serde_json::json!({"bytes":10}),
        )
        .expect("kayıt");
        assert_eq!(db.verify_audit_chain().expect("zincir"), 2);

        db.conn()
            .execute(
                "UPDATE audit_events SET operation='silindi' WHERE sequence=2",
                [],
            )
            .expect("kurcala");
        assert!(db.verify_audit_chain().is_err());
    }

    #[test]
    fn backup_is_readable_only_with_key() {
        let dir = tempfile::tempdir().expect("dizin");
        let path = dir.path().join("vault.db");
        let backup = dir.path().join("yedek.db");
        let db = Database::open(&path, &key()).expect("açılır");
        db.conn().execute(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Yedek','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            [],
        ).expect("ekle");
        db.backup_to(&backup, &key()).expect("yedek");

        let restored = Database::open(&backup, &key()).expect("geri aç");
        let count: i64 = restored
            .conn()
            .query_row("SELECT count(*) FROM cases", [], |r| r.get(0))
            .expect("say");
        assert_eq!(count, 1);

        assert_eq!(
            Database::open(&backup, &DatabaseKey::new([1u8; 32]))
                .expect_err("yanlış")
                .code(),
            "E_DB_KEY"
        );
    }

    #[test]
    fn database_key_derivation_is_stable_and_domain_separated() {
        let a = DatabaseKey::derive_from_master(&[1u8; 32], "vault-a").expect("türet");
        let b = DatabaseKey::derive_from_master(&[1u8; 32], "vault-a").expect("türet");
        let c = DatabaseKey::derive_from_master(&[1u8; 32], "vault-b").expect("türet");
        assert_eq!(a.hex(), b.hex());
        assert_ne!(a.hex(), c.hex());
    }
}
