//! İçe aktarma hattı: dosyayı oku → türünü **içerikten** doğrula → SHA-256 →
//! şifreli blob yaz → belge/sürüm kaydı → ayrıştırma işi kuyruğa.
//!
//! Özgün baytlar hiçbir aşamada değiştirilmez (INV-01). Aynı içerik aynı dosya
//! kapsamında bulunursa yeni mantıksal belge oluşturmak kullanıcı kararıdır.

pub mod budget;
pub mod inventory;
pub mod packaging;
pub mod worker;

use std::path::Path;

use dim_storage::{Database, Document, DocumentVersion, MAX_DOCUMENT_BYTES};
use dim_vault::{UnlockedVault, VaultError};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Pdf,
    Docx,
    Udf,
    Text,
    Png,
    Jpeg,
    Tiff,
}

impl FileKind {
    #[must_use]
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Udf => "application/udf",
            Self::Text => "text/plain",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Tiff => "image/tiff",
        }
    }

    #[must_use]
    pub const fn document_type(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Udf => "udf",
            Self::Text => "text",
            Self::Png | Self::Jpeg | Self::Tiff => "image",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("dosya okunamadı: {0}")]
    Io(#[from] std::io::Error),
    #[error("belge 250 MiB sınırını aşıyor")]
    TooLarge,
    #[error("desteklenmeyen dosya türü")]
    Unsupported,
    #[error("depolama hatası: {0}")]
    Storage(#[from] dim_storage::StorageError),
    #[error("kasa hatası: {0}")]
    Vault(#[from] VaultError),
}

impl IngestError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "E_IO",
            Self::TooLarge => "E_IMPORT_TOO_LARGE",
            Self::Unsupported => "E_IMPORT_UNSUPPORTED",
            Self::Storage(e) => e.code(),
            Self::Vault(e) => e.code(),
        }
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // kısa ömürlü sonuç; kutu eklemek çağrı yerlerini bozar
pub enum ImportResult {
    Imported {
        document: Document,
        version: DocumentVersion,
    },
    /// Aynı baytlar bu dosyada zaten var; yeni kayıt oluşturulmadı.
    Duplicate {
        document_id: String,
        version_id: String,
        display_name: String,
    },
}

/// İçerikten tür algılama. Uzantı tek başına yeterli değildir; magic bytes
/// uyuşmazsa dosya reddedilir (kötü niyetli yeniden adlandırma).
#[must_use]
pub fn detect(data: &[u8], file_name: &str) -> Option<FileKind> {
    let ext = Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);

    if data.starts_with(b"%PDF-") {
        return Some(FileKind::Pdf);
    }
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(FileKind::Png);
    }
    if data.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(FileKind::Jpeg);
    }
    if data.starts_with(b"II*\x00") || data.starts_with(b"MM\x00*") {
        return Some(FileKind::Tiff);
    }
    if data.starts_with(b"PK\x03\x04") {
        match ext.as_deref() {
            Some("docx") => return Some(FileKind::Docx),
            // UDF, ZIP içinde `content.xml` taşır; uzantı tek başına yetmez.
            Some("udf") if data.windows(11).any(|window| window == b"content.xml") => {
                return Some(FileKind::Udf)
            }
            _ => {}
        }
    }
    if matches!(ext.as_deref(), Some("txt" | "md" | "csv")) && std::str::from_utf8(data).is_ok() {
        return Some(FileKind::Text);
    }
    None
}

#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Dosyayı içe aktarır. `force_new=false` iken aynı SHA-256 varsa yeni kayıt
/// açılmaz; kullanıcıya mevcut belge gösterilir.
pub fn import_path(
    db: &Database,
    vault: &UnlockedVault,
    blob_dir: &Path,
    case_id: &str,
    path: &Path,
    force_new: bool,
) -> Result<ImportResult, IngestError> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_DOCUMENT_BYTES {
        return Err(IngestError::TooLarge);
    }
    if !meta.is_file() {
        return Err(IngestError::Unsupported);
    }

    let data = std::fs::read(path)?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("belge")
        .to_string();
    let kind = detect(&data, &name).ok_or(IngestError::Unsupported)?;
    let sha = sha256_hex(&data);

    if !force_new {
        if let Some(existing) = db.find_version_by_sha(case_id, &sha)? {
            let document = db
                .get_document(&existing.document_id)?
                .ok_or(dim_storage::StorageError::NotFound)?;
            return Ok(ImportResult::Duplicate {
                document_id: existing.document_id,
                version_id: existing.id,
                display_name: document.display_name,
            });
        }
    }

    // AAD dosya kapsamına ve içeriğe bağlanır: blob başka dosyaya taşınamaz.
    let aad = format!("{case_id}:{sha}");
    let blob_path = vault.write_blob(blob_dir, &data, aad.as_bytes())?;
    let encrypted_size = i64::try_from(std::fs::metadata(&blob_path)?.len()).unwrap_or(i64::MAX);
    let storage_id = blob_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("blob")
        .to_string();

    let blob_id = db.create_blob_record(&storage_id, encrypted_size, &aad)?;
    let document = db.create_document(case_id, &name, kind.document_type(), None, "unknown")?;
    let version = db.add_version(
        &document.id,
        case_id,
        &blob_id,
        &sha,
        i64::try_from(data.len()).unwrap_or(i64::MAX),
        kind.mime(),
    )?;
    db.enqueue_job(
        "parse_document",
        Some(case_id),
        &sha,
        &format!("parse:{}", version.id),
    )?;

    Ok(ImportResult::Imported { document, version })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dim_storage::{CaseKind, DatabaseKey, NewCase};

    fn setup(dir: &Path) -> (Database, UnlockedVault, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([11u8; 32])).expect("db");
        let case = db
            .create_case(&NewCase {
                title: "Kira".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: dim_storage::Sensitivity::Normal,
                default_ai_policy: dim_storage::AiPolicy::Off,
            })
            .expect("case");
        let (_file, _recovery, vault) =
            dim_vault::VaultFile::create(&dir.join("v.dimvault"), "parola-uzun-123").expect("kasa");
        (db, vault, case.id)
    }

    #[test]
    fn detects_by_content_not_extension() {
        assert_eq!(detect(b"%PDF-1.7\n...", "x.pdf"), Some(FileKind::Pdf));
        assert_eq!(detect(b"%PDF-1.7", "x.exe"), Some(FileKind::Pdf));
        assert_eq!(detect(b"merhaba", "not.txt"), Some(FileKind::Text));
        assert_eq!(detect(b"{}\x00", "x.pdf"), None);
    }

    #[test]
    fn detects_udf_container_by_content_manifest() {
        let mut udf = Vec::new();
        udf.extend_from_slice(b"PK\x03\x04");
        udf.extend_from_slice(&[0u8; 26]);
        udf.extend_from_slice(b"content.xml");
        assert_eq!(detect(&udf, "dilekce.udf"), Some(FileKind::Udf));
        assert_eq!(FileKind::Udf.mime(), "application/udf");
        assert_eq!(FileKind::Udf.document_type(), "udf");
        // `content.xml` yoksa uzantıya rağmen UDF sayılmaz.
        assert_eq!(detect(b"PK\x03\x04rastgele", "dilekce.udf"), None);
    }

    #[test]
    fn import_writes_encrypted_blob_and_version() {
        let dir = tempfile::tempdir().expect("dizin");
        let (db, vault, case_id) = setup(dir.path());
        let file = dir.path().join("sozlesme.pdf");
        std::fs::write(&file, b"%PDF-1.4\nsentetik").expect("yaz");

        let result = import_path(
            &db,
            &vault,
            &dir.path().join("blobs"),
            &case_id,
            &file,
            false,
        )
        .expect("içe al");
        let (doc, version) = match result {
            ImportResult::Imported { document, version } => (document, version),
            other => panic!("beklenmeyen: {other:?}"),
        };
        assert_eq!(version.version_no, 1);
        assert_eq!(doc.display_name, "sozlesme.pdf");
        assert_eq!(db.list_documents(&case_id).expect("liste").len(), 1);

        // Blob diskte şifreli: düz metin başlığı görünmez.
        let blobs: Vec<_> = std::fs::read_dir(dir.path().join("blobs"))
            .expect("dizin")
            .filter_map(Result::ok)
            .collect();
        assert_eq!(blobs.len(), 1);
        let bytes = std::fs::read(blobs[0].path()).expect("oku");
        assert!(!bytes.windows(5).any(|w| w == b"%PDF-"));

        let opened = vault
            .read_blob(
                &blobs[0].path(),
                format!("{case_id}:{}", version.original_sha256).as_bytes(),
            )
            .expect("çöz");
        assert!(opened.starts_with(b"%PDF-"));
    }

    #[test]
    fn duplicate_content_is_reported_without_new_document() {
        let dir = tempfile::tempdir().expect("dizin");
        let (db, vault, case_id) = setup(dir.path());
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"ayni icerik").expect("yaz");

        import_path(
            &db,
            &vault,
            &dir.path().join("blobs"),
            &case_id,
            &file,
            false,
        )
        .expect("ilk");
        let second = import_path(
            &db,
            &vault,
            &dir.path().join("blobs"),
            &case_id,
            &file,
            false,
        )
        .expect("ikinci");
        assert!(matches!(second, ImportResult::Duplicate { .. }));
        assert_eq!(db.list_documents(&case_id).expect("liste").len(), 1);

        let forced = import_path(
            &db,
            &vault,
            &dir.path().join("blobs"),
            &case_id,
            &file,
            true,
        )
        .expect("zorla");
        assert!(matches!(forced, ImportResult::Imported { .. }));
        assert_eq!(db.list_documents(&case_id).expect("liste").len(), 2);
    }

    #[test]
    fn unsupported_file_is_rejected() {
        let dir = tempfile::tempdir().expect("dizin");
        let (db, vault, case_id) = setup(dir.path());
        let file = dir.path().join("bilinmeyen.bin");
        std::fs::write(&file, [0u8, 1, 2, 3]).expect("yaz");
        let err = import_path(
            &db,
            &vault,
            &dir.path().join("blobs"),
            &case_id,
            &file,
            false,
        )
        .unwrap_err();
        assert_eq!(err.code(), "E_IMPORT_UNSUPPORTED");
    }
}
