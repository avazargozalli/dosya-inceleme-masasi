use serde::Serialize;

use dim_vault::VaultError;

/// Arayüze dönen tek hata biçimi. Ham `Debug` çıktısı sızdırılmaz;
/// `code` kararlıdır, `message` kullanıcıya gösterilebilir.
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl ApiError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            retryable: false,
        }
    }

    pub fn vault_locked() -> Self {
        Self::new("E_VAULT_LOCKED", "Kasa kilitli. Önce kasayı açın.")
    }
}

impl From<dim_ingest::IngestError> for ApiError {
    fn from(value: dim_ingest::IngestError) -> Self {
        Self {
            code: value.code().to_string(),
            message: value.to_string(),
            retryable: matches!(value, dim_ingest::IngestError::Io(_)),
        }
    }
}

impl From<dim_storage::StorageError> for ApiError {
    fn from(value: dim_storage::StorageError) -> Self {
        let retryable = matches!(value, dim_storage::StorageError::Io(_));
        Self {
            code: value.code().to_string(),
            message: value.to_string(),
            retryable,
        }
    }
}

impl From<VaultError> for ApiError {
    fn from(value: VaultError) -> Self {
        let retryable = matches!(value, VaultError::Io(_));
        Self {
            code: value.code().to_string(),
            message: value.to_string(),
            retryable,
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(value: std::io::Error) -> Self {
        let message = match value.kind() {
            std::io::ErrorKind::PermissionDenied => "Bu konuma yazma izni yok.".to_string(),
            std::io::ErrorKind::NotFound => "Klasör bulunamadı.".to_string(),
            std::io::ErrorKind::StorageFull => "Diskte yeterli alan yok.".to_string(),
            _ => "Dosya işlemi tamamlanamadı.".to_string(),
        };
        Self {
            code: "E_IO".to_string(),
            message,
            retryable: true,
        }
    }
}
