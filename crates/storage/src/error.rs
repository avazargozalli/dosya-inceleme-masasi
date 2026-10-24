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
    #[error("bütünlük kontrolü başarısız: {0}")]
    Integrity(String),
    #[error("SQL hatası: {0}")]
    Sql(#[from] rusqlite::Error),
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
        }
    }
}
