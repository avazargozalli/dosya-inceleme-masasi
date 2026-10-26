use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("veritabanı açılamadı")]
    Open,
    #[error("kasa anahtarı yanlış veya veritabanı bozuk")]
    BadKey,
    #[error("migrasyon {version} checksum'ı değişmiş; dosya sonradan düzenlenmiş")]
    MigrationChecksum { version: u32 },
    #[error("migrasyon {version} uygulanamadı: {source}")]
    Migration {
        version: u32,
        #[source]
        source: rusqlite::Error,
    },
    #[error("şema sürümü beklenenden yeni: {found} > {expected}")]
    SchemaTooNew { found: u32, expected: u32 },
    #[error("güncelleme öncesi yedek alınamadı veya doğrulanamadı: {reason}")]
    MigrationBackup { reason: String },
    #[error("veri v{restored_version} sürümüne geri döndü (güvenlik kopyası: {safety_copy}) ama kasa yeniden açılamadı: {reason}")]
    RollbackReopen {
        restored_version: u32,
        safety_copy: String,
        reason: String,
    },
    #[error("diskte yeterli yer yok: {reason}")]
    DiskFull { reason: String },
    #[error("kasa yanında dolu bir WAL dosyası var: kasa başka bir süreçte açık olabilir. Uygulamayı kapatıp kasayı bir kez açın, sonra geri dönüşü tekrarlayın")]
    RollbackWalBusy,
    #[error("kasa dosyası okunamadı: dosya sistemi hatası (disk dolu ya da birim koptu olabilir): {reason}")]
    VaultIo { reason: String },
    #[error("yedek doğrulanamadı, hiçbir şey geri yüklenmedi: {reason}")]
    BackupInvalid { reason: String },
    #[error("hedef klasör boş değil; geri yükleme mevcut dosyaların üzerine yazmaz")]
    RestoreDestNotEmpty,
    #[error("bütünlük kontrolü başarısız: {0}")]
    Integrity(String),
    #[error("SQL hatası: {0}")]
    Sql(rusqlite::Error),

    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("geçersiz ayar değeri")]
    InvalidSetting,
    #[error("geçersiz etiket: boş, kontrol karakterli veya 40 karakterden uzun")]
    InvalidTag,
    #[error("geçersiz arama filtresi: {0}")]
    SearchFilter(String),
    #[error("geçersiz arama sorgusu: {0}")]
    SearchQuery(String),
    #[error("arama imleci geçersiz ya da bu sorguya ait değil")]
    SearchCursor,
    #[error("{0}")]
    ChecklistRule(String),
    #[error("kayıt bulunamadı")]
    NotFound,
    #[error("kayıt başka bir oturumda değişmiş: beklenen sürüm {expected}, bulunan {actual}")]
    Conflict { expected: i64, actual: i64 },
    #[error("kasa kilitli")]
    VaultLocked,
    #[error("not geçmişi: {0}")]
    NoteHistory(String),
    #[error("alıntı segment metninde bulunamadı")]
    QuoteNotFound,
    #[error("alıntı birden fazla yerde geçiyor; konum belirsiz")]
    QuoteAmbiguous,
    #[error("alıntı hash'i içerikle uyuşmuyor")]
    QuoteHashMismatch,
    #[error("geçerli aktarım onayı yok")]
    AiConsent,
    #[error("sağlayıcı profili bulunamadı")]
    AiProvider,
    #[error("künye listesi: {0}")]
    Manifest(String),
    #[error("kaynak kullanım koşulu: {0}")]
    Terms(String),
    #[error("izinsiz toplu karar korpusu dağıtılmaz: {0}")]
    CorpusNotPermitted(String),
    #[error("emsal tam metni kaynaktan üretilemedi: {0}")]
    PrecedentSource(String),
    #[error("aday şeması geçersiz: {0}")]
    AiSchema(String),
    #[error("adaydaki alıntı kaynak olarak kullanılamaz: {0}")]
    AiQuote(String),
    #[error("alıntı denetimi için metin çok uzun (en çok 50.000 karakter)")]
    QuoteTextTooLong,
    #[error("metinde çok fazla alıntı var (en çok 30)")]
    QuoteTooMany,
    #[error("taranacak metin kapsamı çok büyük (20.000 sayfa); belge seçerek deneyin")]
    QuoteScopeTooLarge,
    #[error("iş kiralaması geçersiz: {0}")]
    JobLease(String),
    #[error("iş durumu bu işleme uygun değil: {0}")]
    JobState(String),
    #[error("iş ilerlemesi geçersiz: {0}")]
    JobProgress(String),
    #[error(
        "migrasyon {version} yabancı anahtar ihlali bıraktı ({violations} satır); geri alındı"
    )]
    MigrationForeignKeys { version: u32, violations: i64 },
}

/// SQLite hatası disk dolması mı?
///
/// SQLCipher bu hatayı bazen `SQLITE_ERROR` birincil koduyla bildiriyor
/// (`code: Unknown, extended_code: 1`) ve yalnız mesajda "database or disk is
/// full" yazıyor; bu yüzden kod kadar mesaj da denetlenir. Başka hiçbir SQLite
/// mesajı "is full" içermez.
pub(crate) fn sql_is_disk_full(err: &rusqlite::Error) -> bool {
    match err {
        rusqlite::Error::SqliteFailure(inner, message) => {
            inner.code == rusqlite::ErrorCode::DiskFull
                || message
                    .as_deref()
                    .is_some_and(|text| text.to_ascii_lowercase().contains("is full"))
        }
        _ => false,
    }
}

impl From<rusqlite::Error> for StorageError {
    /// Disk dolması ayrı koda (`E_DISK_FULL`) düşer: kullanıcıya "disk dolu"
    /// demek ile "SQL hatası" demek arasındaki fark, yedek alma sırasındaki
    /// müdahaleyi belirler. Diğer SQL hataları eskisi gibi.
    fn from(value: rusqlite::Error) -> Self {
        if sql_is_disk_full(&value) {
            return Self::DiskFull {
                reason: value.to_string(),
            };
        }
        Self::Sql(value)
    }
}

impl StorageError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Open => "E_DB_OPEN",
            Self::BadKey => "E_DB_KEY",
            Self::MigrationChecksum { .. } => "E_DB_MIGRATION_CHECKSUM",
            Self::Migration { .. } => "E_DB_MIGRATION",
            Self::SchemaTooNew { .. } => "E_DB_SCHEMA_NEWER",
            Self::MigrationBackup { .. } => "E_DB_MIGRATION_BACKUP",
            Self::RollbackReopen { .. } => "E_DB_ROLLBACK_REOPEN",
            Self::DiskFull { .. } => "E_DISK_FULL",
            Self::RollbackWalBusy => "E_DB_ROLLBACK_WAL",
            Self::VaultIo { .. } => "E_DB_IO",
            Self::BackupInvalid { .. } => "E_BACKUP_INVALID",
            Self::RestoreDestNotEmpty => "E_RESTORE_DEST_NOT_EMPTY",
            Self::Integrity(_) => "E_DB_INTEGRITY",
            Self::Sql(_) => "E_DB_SQL",
            Self::Io(_) => "E_IO",
            Self::InvalidSetting => "E_DB_SETTING",
            Self::InvalidTag => "E_TAG_INVALID",
            Self::SearchFilter(_) => "E_SEARCH_FILTER",
            Self::SearchQuery(_) => "E_SEARCH_QUERY",
            Self::SearchCursor => "E_SEARCH_CURSOR",
            Self::ChecklistRule(_) => "E_CHECKLIST_RULE",
            Self::NotFound => "E_NOT_FOUND",
            Self::Conflict { .. } => "E_ROW_VERSION",
            Self::VaultLocked => "E_VAULT_LOCKED",
            Self::NoteHistory(_) => "E_NOTE_HISTORY",
            Self::QuoteNotFound => "E_QUOTE_NOT_FOUND",
            Self::QuoteAmbiguous => "E_QUOTE_AMBIGUOUS",
            Self::QuoteHashMismatch => "E_QUOTE_HASH",
            Self::AiConsent => "E_AI_CONSENT",
            Self::AiProvider => "E_AI_PROVIDER",
            Self::Manifest(_) => "E_MANIFEST",
            Self::Terms(_) => "E_TERMS",
            Self::CorpusNotPermitted(_) => "E_CORPUS_NOT_PERMITTED",
            Self::PrecedentSource(_) => "E_PRECEDENT_SOURCE",
            Self::AiSchema(_) => "E_AI_SCHEMA",
            Self::AiQuote(_) => "E_AI_QUOTE",
            Self::QuoteTextTooLong => "E_QUOTE_TEXT",
            Self::QuoteTooMany => "E_QUOTE_TOO_MANY",
            Self::QuoteScopeTooLarge => "E_QUOTE_SCOPE",
            Self::JobLease(_) => "E_JOB_LEASE",
            Self::JobState(_) => "E_JOB_STATE",
            Self::JobProgress(_) => "E_JOB_PROGRESS",
            Self::MigrationForeignKeys { .. } => "E_DB_MIGRATION_FK",
        }
    }
}
