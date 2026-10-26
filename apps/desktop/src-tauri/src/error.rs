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

impl From<dim_ingest::worker::WorkerError> for ApiError {
    fn from(value: dim_ingest::worker::WorkerError) -> Self {
        Self {
            retryable: value.retryable(),
            code: value.code().to_string(),
            message: value.to_string(),
        }
    }
}

impl From<dim_providers::ProviderError> for ApiError {
    fn from(value: dim_providers::ProviderError) -> Self {
        Self {
            code: value.code().to_string(),
            message: value.to_string(),
            retryable: value.retryable(),
        }
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
        // Disk dolması kullanıcı yer açtıktan sonra tekrar denenebilir; kasa
        // hatası olarak döner ama kodu ayrıdır (`E_DISK_FULL`).
        let retryable = matches!(
            value,
            dim_storage::StorageError::Io(_)
                | dim_storage::StorageError::DiskFull { .. }
                | dim_storage::StorageError::VaultIo { .. }
                | dim_storage::StorageError::RollbackWalBusy
        );
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
        // Disk dolması ayrı kod alır: arayüz "yer açın" diyebilsin ve testte
        // `E_IO` ile karışmasın.
        if dim_storage::atomic::is_no_space(&value) {
            return Self {
                code: "E_DISK_FULL".to_string(),
                message: "Diskte yeterli alan yok. Yer açıp yeniden deneyin.".to_string(),
                retryable: true,
            };
        }
        let message = match value.kind() {
            std::io::ErrorKind::PermissionDenied => "Bu konuma yazma izni yok.".to_string(),
            std::io::ErrorKind::NotFound => "Klasör bulunamadı.".to_string(),
            _ => "Dosya işlemi tamamlanamadı.".to_string(),
        };
        Self {
            code: "E_IO".to_string(),
            message,
            retryable: true,
        }
    }
}
