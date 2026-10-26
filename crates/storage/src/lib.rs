//! Şifreli veri tabanı: SQLCipher, migrasyon, bütünlük ve yedek.
//!
//! Anahtar kasa ana anahtarından HKDF ile türetilir ve yalnız bu crate'e
//! `DatabaseKey` olarak verilir; `Debug` çıktısında görünmez. Açık metin
//! veritabanı dosyası hiçbir aşamada oluşturulmaz.

mod ai;
mod ai_candidate;
mod anchors;
pub mod atomic;
pub mod backup;
mod cases;
mod checklists;
mod claims;
mod connectors;
mod documents;
mod error;
mod export_coverage;
mod exports;
mod gc;
mod intext_quotes;
pub mod job_state;
mod marks;
mod notes;
mod parsing;
mod pre_update;
mod precedent_import;
mod precedent_terms;
mod precedents;
pub mod queue;
mod redactions;
mod review_commit;
mod tags;
mod tasks;
mod timeline;

pub use ai::{
    AiCandidateRow, AiContextItemRow, AiRunRow, ConsentPreview, CoverageReport, NewAiContextItem,
    NewAiRun, NewTransferConsent, ProviderProfileRow, TransferConsentRow,
};
pub use ai_candidate::{
    CandidateReport, CandidateSchema, QuoteMatchReport, QuotePosition, QuoteRef, ValidationIssue,
};
pub use anchors::{AnchorLocation, AnchorRow, NewAnchor, RebindCandidate, RebindSuggestion};
pub use cases::{AiPolicy, Case, CaseKind, CasePatch, CaseStatus, NewCase, Sensitivity};
pub use checklists::{
    CaseChecklistItemRow, ChecklistCopyResult, ChecklistTemplateRef, ChecklistTemplateRow,
    LinkedDocumentRow,
};
pub use claims::{
    ClaimRelationRow, ClaimRow, ContradictionAssessmentRow, EvidenceLinkRow, NewClaim,
    NewClaimRelation, NewEvidenceLink, NewReviewItem, ReviewDecisionRow, ReviewItemRow,
};
pub use connectors::{
    connector_catalog, connector_catalog_hash, ConnectorInfo, STATUS_DOCUMENTED_NOT_ENABLED,
};
pub use documents::{Document, DocumentVersion, MAX_DOCUMENT_BYTES};
pub use error::StorageError;
pub use export_coverage::{ExportCoverage, COVERAGE_COUNTING_NOTE, COVERAGE_STATEMENT};
pub use exports::ExportRow;
pub use gc::{GcBlobRow, GcReport};
pub use intext_quotes::{
    extract_quoted_spans, ExtractedQuotes, InTextQuoteCheck, InTextQuoteFinding, InTextQuoteInput,
    InTextQuotePosition, SpannedQuote, MAX_IN_TEXT_CHECK_CHARS, MAX_IN_TEXT_QUOTES,
};
pub use marks::{MarkRow, NewRegionMark, NewTextMark};
pub use notes::{NewNote, NoteDraftInput, NoteDraftRow, NoteEdit, NoteRevisionRow, NoteRow};
pub use parsing::{
    settings_hash, BlobRef, NewPage, PageRow, ParseRun, SearchCursor, SearchFilters, SearchHit,
    SearchMode, SearchPage, SearchQuery, SearchTotal, SearchVerification, SegmentBlock,
    SegmentGeometry, MAX_SEARCH_LIMIT, MAX_SEARCH_QUERY_CHARS, SEARCH_CANDIDATE_WINDOW,
    SEARCH_DATE_ROLES,
};
pub use pre_update::{pre_update_backups, rollback_and_resume, InPlaceRollback, PreUpdateBackup};
pub use precedent_import::{
    parse_manifest, Manifest, ManifestRow, PrecedentCopyInput, PrecedentCopyOutcome,
    MANIFEST_MAX_BYTES,
};
pub use precedent_terms::{
    CorpusCoverage, CorpusExcluded, CorpusExport, CorpusItem, CorpusMode, PrecedentUsageTerms,
    SetUsageTermsInput, REDISTRIBUTION_VALUES, USAGE_BASES,
};
pub use precedents::{
    CitationCheckRow, CitationContext, NewCitationCheck, NewPrecedent, NewPrecedentNote,
    NewPrecedentText, PrecedentBibliography, PrecedentDocumentText, PrecedentNoteRow, PrecedentRow,
    PrecedentSearchHit, PrecedentSearchPage, PrecedentSearchQuery, PrecedentVersionRow,
    QuoteVerification, MAX_PRECEDENT_DOCUMENT_TEXT_BYTES, MAX_PRECEDENT_SEARCH_LIMIT,
    PRECEDENT_SEARCH_SCOPE,
};
pub use queue::{
    JobCancelResult, JobQueueSummary, JobRecovery, JobRow, JobStateCount, DEFAULT_LEASE_SECONDS,
    JOB_PARSE_DOCUMENT,
};
pub use redactions::{NewRedactionPlan, RedactionPlanRow, RedactionRegionInput};
pub use review_commit::{
    OutboxEventRow, ReviewCommitResult, ReviewTarget, EVENT_DECISION_COMMITTED, EVENT_ITEM_DECIDED,
};
pub use tags::{CaseTagRow, DocumentTagRow, MAX_TAG_CHARS};
pub use tasks::{NewTask, ReminderEnqueueResult, ReminderRow, TaskRow, EVENT_TASK_REMINDER};
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
    Migration {
        version: 6,
        name: "0006_review_superseded",
        sql: include_str!("migrations/0006_review_superseded.sql"),
    },
    Migration {
        version: 7,
        name: "0007_contradictions",
        sql: include_str!("migrations/0007_contradictions.sql"),
    },
    Migration {
        version: 8,
        name: "0008_task_date_confirmation",
        sql: include_str!("migrations/0008_task_date_confirmation.sql"),
    },
    Migration {
        version: 9,
        name: "0009_checklists",
        sql: include_str!("migrations/0009_checklists.sql"),
    },
    Migration {
        version: 10,
        name: "0010_document_tags",
        sql: include_str!("migrations/0010_document_tags.sql"),
    },
    Migration {
        version: 11,
        name: "0011_note_history",
        sql: include_str!("migrations/0011_note_history.sql"),
    },
    Migration {
        version: 12,
        name: "0012_source_marks",
        sql: include_str!("migrations/0012_source_marks.sql"),
    },
    Migration {
        version: 13,
        name: "0013_precedent_text",
        sql: include_str!("migrations/0013_precedent_text.sql"),
    },
    Migration {
        version: 15,
        name: "0015_citation_meaning",
        sql: include_str!("migrations/0015_citation_meaning.sql"),
    },
    Migration {
        version: 14,
        name: "0014_precedent_conflict",
        sql: include_str!("migrations/0014_precedent_conflict.sql"),
    },
    Migration {
        version: 16,
        name: "0016_precedent_usage_terms",
        sql: include_str!("migrations/0016_precedent_usage_terms.sql"),
    },
    Migration {
        version: 17,
        name: "0017_ai_candidate_quotes",
        sql: include_str!("migrations/0017_ai_candidate_quotes.sql"),
    },
    Migration {
        version: 18,
        name: "0018_page_lookup_index",
        sql: include_str!("migrations/0018_page_lookup_index.sql"),
    },
    Migration {
        version: 19,
        name: "0019_fts_update_trigger",
        sql: include_str!("migrations/0019_fts_update_trigger.sql"),
    },
    Migration {
        version: 20,
        name: "0020_job_queue",
        sql: include_str!("migrations/0020_job_queue.sql"),
    },
    Migration {
        version: 21,
        name: "0021_job_states",
        sql: include_str!("migrations/0021_job_states.sql"),
    },
];

pub const CURRENT_SCHEMA_VERSION: u32 = 21;

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
    pre_migration_backup: Option<PathBuf>,
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
        parsing::register_search_functions(&conn)?;
        let mut db = Self {
            conn,
            path: Some(path.to_path_buf()),
            schema_version: 0,
            pre_migration_backup: None,
        };
        db.upgrade_with_backup(key)?;
        Ok(db)
    }

    /// Test ve geçici işler için bellek içi şifreli veritabanı.
    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory().map_err(|_| StorageError::Open)?;
        apply_key_and_pragmas(&conn, key)?;
        parsing::register_search_functions(&conn)?;
        let mut db = Self {
            conn,
            path: None,
            schema_version: 0,
            pre_migration_backup: None,
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

    /// Bu açılışta migrasyondan **önce** alınan doğrulanmış yedeğin yolu.
    /// Güncelleme yoksa `None`.
    #[must_use]
    pub fn pre_migration_backup(&self) -> Option<&Path> {
        self.pre_migration_backup.as_deref()
    }

    /// Dosyada kayıtlı şema sürümü; tablo yoksa (yeni dosya) 0.
    fn recorded_version(conn: &Connection) -> Result<u32, StorageError> {
        let exists: Option<String> = conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Ok(0);
        }
        let found: i64 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?;
        Ok(u32::try_from(found).unwrap_or(u32::MAX))
    }

    /// Yeni şemayı **hiçbir yazma yapmadan** reddeder (F16-T08). Eski uygulama
    /// yeni veritabanını açıp üzerine yazmamalıdır.
    fn reject_newer_schema(&self) -> Result<u32, StorageError> {
        let found = Self::recorded_version(&self.conn)?;
        if found > CURRENT_SCHEMA_VERSION {
            return Err(StorageError::SchemaTooNew {
                found,
                expected: CURRENT_SCHEMA_VERSION,
            });
        }
        Ok(found)
    }

    /// Güncelleme zarfı: bekleyen migrasyon varsa **önce** doğrulanmış yedek
    /// alır, sonra migrasyonu uygular. Yedek alınamaz veya açılıp
    /// doğrulanamazsa güncelleme hiç başlamaz (`E_DB_MIGRATION_BACKUP`).
    fn upgrade_with_backup(&mut self, key: &DatabaseKey) -> Result<MigrationOutcome, StorageError> {
        let before = self.reject_newer_schema()?;
        let pending = MIGRATIONS.iter().any(|m| m.version > before);
        let Some(path) = self.path.clone().filter(|_| before > 0 && pending) else {
            return self.migrate();
        };

        let backup = pre_migration_backup_path(&path, before);
        if backup.exists() {
            // Önceki denemede alınmış yedek korunur; üzerine yazılmaz.
            Self::verify_backup(&backup, key, before)?;
        } else {
            self.backup_to(&backup, key)
                .map_err(|err| StorageError::MigrationBackup {
                    reason: err.to_string(),
                })?;
            Self::verify_backup(&backup, key, before)?;
        }
        self.pre_migration_backup = Some(backup);

        let outcome = self.migrate()?;
        let _ = self.append_audit(
            "vault_upgrade",
            None,
            "migrate",
            "system",
            &serde_json::json!({"from": before, "to": outcome.current, "backup": true}),
        );
        Ok(outcome)
    }

    /// Kapalı bir kasa dosyasını açar, bütünlüğünü doğrular ve şema sürümünü
    /// döndürür. Migrasyon uygulamaz; yedek doğrulaması için kullanılır.
    fn verify_database_file(path: &Path, key: &DatabaseKey) -> Result<u32, StorageError> {
        Self::verify_with_flags(path, key, false)
    }

    /// Aynı doğrulama, dosya **salt okunur** açılır: yedek listesi her bakışta
    /// yedeğin başlığını değiştirmemeli (WAL bayrağı yazılmaz) ve salt okunur
    /// ortamda duran yedek de doğrulanabilmeli.
    fn verify_database_file_readonly(path: &Path, key: &DatabaseKey) -> Result<u32, StorageError> {
        Self::verify_with_flags(path, key, true)
    }

    fn verify_with_flags(
        path: &Path,
        key: &DatabaseKey,
        readonly: bool,
    ) -> Result<u32, StorageError> {
        let flags = if readonly {
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX
        };
        let conn = Connection::open_with_flags(path, flags).map_err(|err| {
            StorageError::MigrationBackup {
                reason: format!("dosya açılamadı: {err}"),
            }
        })?;
        apply_key_and_pragmas(&conn, key).map_err(|err| StorageError::MigrationBackup {
            reason: format!("anahtar/bütünlük: {err}"),
        })?;
        let version = Self::recorded_version(&conn)?;
        let check: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if check != "ok" {
            return Err(StorageError::Integrity(check));
        }
        Ok(version)
    }

    fn verify_backup(backup: &Path, key: &DatabaseKey, expected: u32) -> Result<(), StorageError> {
        let found = Self::verify_database_file(backup, key)?;
        if found != expected {
            return Err(StorageError::MigrationBackup {
                reason: format!("yedek şema sürümü {found}, beklenen {expected}"),
            });
        }
        Ok(())
    }

    /// Bekleyen migrasyonları sırayla uygular. Uygulanmış bir migrasyonun
    /// checksum'ı değişmişse durur: dosya sonradan düzenlenmiş demektir.
    ///
    /// Yabancı anahtar denetimi migrasyon boyunca **kapatılır**: tablo yeniden
    /// kuran bir migrasyonda `DROP TABLE <üst>` (ör. `jobs`) çocuk satırları
    /// `ON DELETE CASCADE` ile siler — `foreign_keys` açıkken bu ölçüldü ve
    /// `job_attempts` boşaldı. `PRAGMA foreign_keys` işlem içinde değişmediği
    /// için kapatma işlemden önce yapılır; commit'ten önce her migrasyonun
    /// ardından `PRAGMA foreign_key_check` koşar ve ihlal varsa migrasyon
    /// geri alınır. Dönüşte denetim yeniden açılır (hata yolunda da).
    pub fn migrate(&mut self) -> Result<MigrationOutcome, StorageError> {
        self.conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
        let outcome = self.migrate_without_foreign_keys();
        let restored = self.conn.execute_batch("PRAGMA foreign_keys = ON;");
        let outcome = outcome?;
        restored?;
        Ok(outcome)
    }

    fn migrate_without_foreign_keys(&mut self) -> Result<MigrationOutcome, StorageError> {
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
                    let violations = foreign_key_violations(&tx)?;
                    if violations > 0 {
                        return Err(StorageError::MigrationForeignKeys {
                            version: migration.version,
                            violations,
                        });
                    }
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
    ///
    /// Hedef ada **doğrudan** yazılmaz: disk dolduğunda yarım bir DB kalır ve
    /// sonraki deneme "hedef zaten var" ile kapanırdı. Geçici ada yazılır,
    /// diske indirilir, sonra yerleştirilir (F15-T06).
    pub fn backup_to(&self, dest: &Path, key: &DatabaseKey) -> Result<(), StorageError> {
        if dest.exists() {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "yedek hedefi zaten var",
            )));
        }
        if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(atomic::map_io)?;
        }
        let temp = atomic::temp_sibling(dest, "dimdb");
        let path_literal = temp.to_string_lossy().replace('\'', "''");
        let exported = (|| -> Result<(), StorageError> {
            self.conn.execute_batch(&format!(
                "ATTACH DATABASE '{path_literal}' AS dim_backup KEY \"x'{}'\";",
                key.hex()
            ))?;
            // `DETACH` her durumda denenir; başarısız export bağlantıyı
            // iliştirilmiş bırakırsa sonraki yedekler de düşerdi.
            let export = self
                .conn
                .execute_batch("SELECT sqlcipher_export('dim_backup');");
            let detach = self.conn.execute_batch("DETACH DATABASE dim_backup;");
            export?;
            detach?;
            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(&temp)
                .map_err(atomic::map_io)?;
            file.sync_all().map_err(atomic::map_io)?;
            Ok(())
        })();
        if let Err(err) = exported {
            atomic::remove_file_best_effort(&temp);
            return Err(err);
        }
        if let Err(err) = std::fs::rename(&temp, dest) {
            atomic::remove_file_best_effort(&temp);
            return Err(atomic::map_io(err));
        }
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

/// Kasanın ilk okuması düştüğünde hatayı sınıflandırır.
///
/// `E_DB_KEY` "anahtar yanlış olabilir" der; ama disk dolu ya da birim
/// koptuğunda da ilk okuma düşer ve kullanıcıya "kasanız bozuk" demek
/// yanıltıcı olur (tatbikatta ölçüldü: tam dolu APFS'te SQLite
/// `SystemIoFailure`/`SHMCLOSE` — "disk I/O error" — veriyor). Bu yüzden G/Ç
/// sınıfı ayrı koda (`E_DB_IO`) düşer; gerçek anahtar/bozulma hatası
/// `E_DB_KEY` olarak kalır.
fn classify_first_read_failure(err: &rusqlite::Error) -> StorageError {
    if crate::error::sql_is_disk_full(err) {
        return StorageError::DiskFull {
            reason: err.to_string(),
        };
    }
    if let rusqlite::Error::SqliteFailure(inner, _) = err {
        if matches!(
            inner.code,
            rusqlite::ErrorCode::SystemIoFailure
                | rusqlite::ErrorCode::CannotOpen
                | rusqlite::ErrorCode::DatabaseBusy
        ) {
            return StorageError::VaultIo {
                reason: err.to_string(),
            };
        }
    }
    StorageError::BadKey
}

/// `PRAGMA foreign_key_check` ihlal sayısı. Migrasyon commit'inden önce
/// çağrılır: tablo yeniden kuran bir migrasyon yetim satır bırakırsa
/// migrasyon geri alınır (sessizce bozuk kasa yazılmaz).
fn foreign_key_violations(tx: &rusqlite::Transaction<'_>) -> Result<i64, StorageError> {
    let mut stmt = tx.prepare("PRAGMA foreign_key_check")?;
    let mut rows = stmt.query([])?;
    let mut count = 0i64;
    while rows.next()?.is_some() {
        count += 1;
    }
    Ok(count)
}

fn apply_key_and_pragmas(conn: &Connection, key: &DatabaseKey) -> Result<(), StorageError> {
    // `PRAGMA key` parametre kabul etmez; anahtar hex olarak verilir (alfabe sabit).
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key.hex()))
        .map_err(|_| StorageError::Open)?;
    // Anahtar doğrulaması **ilk** okumada yapılır. Pragma'lar önce çalışırsa
    // yanlış anahtar `E_DB_SQL` olarak sızabilir; kullanıcıya "anahtar yanlış"
    // demek için bu sıra korunmalıdır. **Ama** disk doluysa ilk okuma da
    // düşer: o zaman "anahtar yanlış" demek yanıltıcı olur (tatbikatta ölçüldü,
    // kasa `BadKey` görünüyordu). Disk dolması kendi koduna ayrılır.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(|err| classify_first_read_failure(&err))?;

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

/// Güncelleme öncesi yedeğin yolu: `<kasa dosyası>.v<eski sürüm>-pre-update.bak`.
fn pre_migration_backup_path(path: &Path, from_version: u32) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "vault.db".to_string());
    path.with_file_name(format!("{name}.v{from_version}-pre-update.bak"))
}

/// Geri dönüş tatbikatının sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackOutcome {
    /// Yedeğin taşıdığı şema sürümü.
    pub restored_version: u32,
    /// Geri dönüşten önceki dosyanın saklandığı güvenlik kopyası.
    pub safety_copy: PathBuf,
    /// Bu denemeden önce silinen sahipsiz `.rollback-*` geçici dosyası sayısı
    /// (önceki çökmenin artıkları).
    pub swept_temps: usize,
}

/// Kapalı bir kasa dosyasını, doğrulanmış güncelleme yedeğine geri döndürür.
///
/// Sıra: yedeği aç ve bütünlüğünü doğrula → yedeği geçici ada kopyala ve
/// doğrula → mevcut dosyayı **sabit bağlantıyla** (desteklenmiyorsa kopyayla)
/// güvenlik kopyası olarak sakla → yedeği `rename` ile yerine koy → yerleşen
/// dosyayı yeniden doğrula. Son doğrulama başarısız olursa güvenlik kopyası
/// atomik olarak geri konur; yani geri dönüş de kendi içinde atomiktir.
///
/// **Kasa dosyası hiçbir an ortadan kalkmaz:** eski sıra dosyayı önce
/// taşıyordu, süreç o arada ölürse kasa "yok" görünüyor ve `Database::open`
/// (`SQLITE_OPEN_CREATE`) boş bir kasa yaratıyordu — kullanıcı için "bütün
/// dosyalar kayboldu" demek. Sabit bağlantı bu pencereyi kapatır ve ek yer
/// kaplamaz.
pub fn rollback_vault(
    path: &Path,
    backup: &Path,
    key: &DatabaseKey,
) -> Result<RollbackOutcome, StorageError> {
    if !path.is_file() {
        return Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "kasa dosyası bulunamadı",
        )));
    }
    if !backup.is_file() {
        return Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "güncelleme yedeği bulunamadı",
        )));
    }
    // Yedek **kendi bıraktığı** yan dosyalar temizlenerek doğrulanır: aksi
    // hâlde her deneme kullanıcının klasöründe `-wal`/`-shm` artığı bırakır
    // (`verify_without_residue` dolu `-wal`'ı silmez, sorun olarak bildirir).
    let (verified, problem) = crate::pre_update::verify_without_residue(backup, key);
    if let Some(problem) = problem {
        return Err(StorageError::MigrationBackup { reason: problem });
    }
    let backup_version = verified.ok_or_else(|| StorageError::MigrationBackup {
        reason: "yedek sürümü okunamadı".to_string(),
    })?;
    if backup_version > CURRENT_SCHEMA_VERSION {
        return Err(StorageError::SchemaTooNew {
            found: backup_version,
            expected: CURRENT_SCHEMA_VERSION,
        });
    }

    let stamp = OffsetDateTime::now_utc().unix_timestamp();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "vault.db".to_string());

    // Yarıda kalan önceki denemenin geçici dosyaları burada temizlenir: her
    // çökme kasa klasöründe bir `.rollback-*` artığı bırakırdı. Kasa kilidi
    // tek yazıcı olduğundan bu artıklar sahipsizdir (canlı bir geri dönüşün
    // geçici dosyası bu noktada olamaz).
    let swept_temps = atomic::sweep_tagged_temps(path, "rollback", std::time::Duration::ZERO);

    // Dolu `-wal`/`-shm`: kasa başka bir süreçte açık ya da düzgün kapanmamış
    // demektir. Yan dosyalar yerinden oynatılırsa içlerindeki işlemler sessizce
    // kaybolur; bu yüzden geri dönüş **başlamaz** (kasa bir kez açılıp
    // kapanınca WAL emilir ve dosya kaybolur).
    if sidecar_has_data(path, &name) {
        return Err(StorageError::RollbackWalBusy);
    }

    // Güvenlik kopyası adı saniye çözünürlüklü damgadan gelir; aynı saniyedeki
    // ikinci deneme önceki güvenlik kopyasının üzerine yazmasın diye sıra eklenir.
    let mut safety_copy = path.with_file_name(format!("{name}.rollback-{stamp}.safety"));
    let mut attempt = 0u32;
    while safety_copy.exists() {
        attempt += 1;
        safety_copy = path.with_file_name(format!("{name}.rollback-{stamp}-{attempt}.safety"));
    }

    // Yan dosyalar taşınmazsa eski `-wal` yeni dosyaya uygulanır ve kasa bozulur.
    let sidecars: Vec<(PathBuf, PathBuf)> = ["-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            let from = path.with_file_name(format!("{name}{suffix}"));
            let to = safety_copy.with_file_name(format!(
                "{}{suffix}",
                safety_copy
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            ));
            (from, to)
        })
        .collect();

    // Kopya **önce** hazırlanır ve doğrulanır: disk dolarsa ya da yedek
    // bozuksa mevcut kasa hiç yerinden oynamaz. (Eski sıra kasayı taşıyıp
    // sonra kopyalıyordu; ENOSPC'de kasa bir an "yok" oluyordu.)
    let staged = atomic::temp_sibling(path, "rollback");
    let staged_version = std::fs::copy(backup, &staged).map_err(atomic::map_io);
    let staged_version = match staged_version {
        Ok(_) => Database::verify_database_file_readonly(&staged, key),
        Err(err) => Err(err),
    };
    // Geçici doğrulama dosyasının **kendi** yan dosyaları silinir: SQLite
    // doğrulama sırasında `-shm` (32 KiB) bırakır ve bu artık her geri
    // dönüşte kasa klasöründe kalıyordu. Dosya bizim geçici kopyamız olduğu
    // için ikisi de güvenle silinir.
    remove_temp_sidecars(&staged);
    let staged_version = match staged_version {
        Ok(version) => version,
        Err(err) => {
            atomic::remove_file_best_effort(&staged);
            return Err(err);
        }
    };
    if staged_version != backup_version {
        atomic::remove_file_best_effort(&staged);
        return Err(StorageError::MigrationBackup {
            reason: format!(
                "geri yüklenen kasa sürümü {staged_version}, yedek sürümü {backup_version}"
            ),
        });
    }

    // Güvenlik kopyası **taşınmaz, bağlanır**: `path` hiçbir an yok olmaz.
    // Sabit bağlantı kurulamayan dosya sisteminde tam kopya alınır ve
    // doğrulanır (yer maliyeti kasanın boyutu kadar).
    let linked = atomic::hard_link_or_copy(path, &safety_copy)?;
    if !linked {
        let verified =
            Database::verify_database_file_readonly(&safety_copy, key).inspect_err(|_| {
                atomic::remove_file_best_effort(&safety_copy);
            });
        match verified {
            Ok(_) => remove_temp_sidecars(&safety_copy),
            Err(err) => return Err(err),
        }
    }

    // Tatbikat kancası: süreç burada öldürülürse kasa dosyası **yerinde**
    // kalmalı (bkz. `tests/rollback_drill.rs`). Ürün derlemesinde boş.
    atomic::drill_pause("before_install");

    for (from, to) in &sidecars {
        if from.exists() {
            if let Err(err) = std::fs::rename(from, to) {
                atomic::remove_file_best_effort(&safety_copy);
                return Err(atomic::map_io(err));
            }
        }
    }

    let restored = std::fs::rename(&staged, path)
        .map_err(atomic::map_io)
        .and_then(|()| {
            Database::verify_database_file(path, key).map_err(|err| StorageError::MigrationBackup {
                reason: format!("{}: {err}", err.code()),
            })
        });

    match restored {
        Ok(version) if version == backup_version => Ok(RollbackOutcome {
            restored_version: version,
            safety_copy,
            swept_temps,
        }),
        other => {
            // Eski dosya `safety_copy` üzerinde duruyor; `path` **atomik
            // olarak** onunla değiştirilir. Önce silmek kısa da olsa bir
            // "kasa yok" penceresi açardı.
            let _ = std::fs::rename(&safety_copy, path);
            for (from, to) in sidecars.iter().rev() {
                if to.exists() {
                    let _ = std::fs::rename(to, from);
                }
            }
            Err(match other {
                Ok(version) => StorageError::MigrationBackup {
                    reason: format!(
                        "geri yüklenen kasa sürümü {version}, yedek sürümü {backup_version}"
                    ),
                },
                Err(err) => err,
            })
        }
    }
}

/// Kasanın yanındaki `-wal`/`-shm` dosyalarından **dolu** olan var mı?
///
/// Dolu yan dosya, kasa kapalı değilken (başka süreç/açık bağlantı) ya da
/// düzgün kapanmadan bırakılmış demektir; geri dönüş bunları yerinden
/// oynatırsa içlerindeki işlemler sessizce kaybolur.
fn sidecar_has_data(path: &Path, name: &str) -> bool {
    ["-wal", "-shm"].into_iter().any(|suffix| {
        std::fs::metadata(path.with_file_name(format!("{name}{suffix}")))
            .is_ok_and(|meta| meta.len() > 0)
    })
}

/// Geçici doğrulama dosyasının **kendi** yan dosyalarını (`-wal`, `-shm`)
/// siler.
///
/// Yalnız bizim ürettiğimiz geçici kopyalar için çağrılır: SQLite doğrulama
/// sırasında `-shm` (32 KiB) bırakır, `-wal` boş kalır. Boş olanları silmek
/// yetmiyordu; 32 KiB'lik `-shm` her geri dönüşte kasa klasöründe kalıyordu
/// (tatbikatta ölçüldü). Kullanıcı dosyalarında kullanılmaz.
fn remove_temp_sidecars(path: &Path) {
    let base = path.to_string_lossy().to_string();
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{base}{suffix}"));
        if std::fs::metadata(&sidecar).is_ok() {
            let _ = std::fs::remove_file(&sidecar);
        }
    }
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
    fn migration_0006_preserves_review_decisions() {
        let conn = rusqlite::Connection::open_in_memory().expect("bağlantı");
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .expect("pragma");
        for migration in MIGRATIONS.iter().filter(|m| m.version <= 5) {
            conn.execute_batch(migration.sql).expect("v5 şeması");
        }
        conn.execute_batch(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c1','Dosya','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO review_items (id,case_id,item_kind,payload,source_anchor_ids,state,created_at,updated_at)
             VALUES ('i1','c1','note_candidate','{}','[]','accepted','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');
             INSERT INTO review_decisions (id,review_item_id,decision,reviewer_label,rationale,decided_at)
             VALUES ('d1','i1','accept','avukat','kabul','2026-01-01T00:00:00Z');",
        )
        .expect("v5 verisi");

        let m6 = MIGRATIONS
            .iter()
            .find(|m| m.version == 6)
            .expect("0006 var");
        conn.execute_batch(m6.sql).expect("0006 uygulanır");

        let items: i64 = conn
            .query_row("SELECT count(*) FROM review_items", [], |r| r.get(0))
            .expect("öğe");
        let decisions: i64 = conn
            .query_row("SELECT count(*) FROM review_decisions", [], |r| r.get(0))
            .expect("karar");
        assert_eq!(items, 1, "öğe korunmalı");
        assert_eq!(decisions, 1, "karar cascade ile silinmemeli");

        conn.execute(
            "UPDATE review_items SET superseded_by_item_id=NULL, superseded_at='2026-01-02T00:00:00Z' WHERE id='i1'",
            [],
        )
        .expect("superseded işareti yazılmalı");
        let mut stmt = conn
            .prepare("PRAGMA foreign_key_check")
            .expect("fk sorgusu");
        let violations = stmt.query_map([], |_| Ok(())).expect("fk tarama").count();
        assert_eq!(violations, 0);
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

#[cfg(test)]
mod upgrade_tests;
