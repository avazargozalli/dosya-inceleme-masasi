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
    #[error("bütünlük kontrolü başarısız: {0}")]
    Integrity(String),
    #[error("SQL hatası: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("geçersiz ayar değeri")]
    InvalidSetting,
}

impl StorageError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Open => "E_DB_OPEN",
            Self::BadKey => "E_DB_KEY",
            Self::MigrationChecksum { .. } => "E_DB_MIGRATION_CHECKSUM",
            Self::Migration { .. } => "E_DB_MIGRATION",
            Self::SchemaTooNew { .. } => "E_DB_SCHEMA_NEWER",
            Self::Integrity(_) => "E_DB_INTEGRITY",
            Self::Sql(_) => "E_DB_SQL",
            Self::Io(_) => "E_IO",
            Self::InvalidSetting => "E_DB_SETTING",
        }
    }
}
