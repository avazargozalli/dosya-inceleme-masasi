use std::path::{Path, PathBuf};

use dim_ingest::worker::{dev_worker_dir, WorkerClient};
use dim_ingest::{import_path, ImportResult};
use dim_storage::{
    AnchorRow, Case, CasePatch, Database, DatabaseKey, Document, DocumentVersion, EventRow, NewAnchor,
    NewCase, NewEvent, NewNote, NewPage, NewTask, NoteRow, PageRow, ParseRun, SearchHit, TaskRow,
};
use dim_vault::{UnlockedVault, VaultFile};
use serde::Serialize;
use tauri::State;
use zeroize::Zeroizing;

use crate::error::ApiError;
use crate::state::{open_database, vault_id_from_master, AppState};

use sha2::{Digest, Sha256};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVaultResult {
    pub vault_path: String,
    pub recovery_key: String,
    pub vault_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultInfo {
    pub vault_path: String,
    pub vault_id: String,
    pub version: u8,
    pub created_at_unix: i64,
    pub locked: bool,
    pub schema_version: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub unlocked: bool,
    pub vault_path: Option<String>,
    pub vault_id: Option<String>,
    pub schema_version: Option<u32>,
}

fn resolve_vault_path(input: &str) -> Result<PathBuf, ApiError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ApiError::new("E_PATH", "Kasa konumu boş olamaz."));
    }
    let expanded = if let Some(rest) = trimmed.strip_prefix("~/") {
        home_dir()?.join(rest)
    } else {
        PathBuf::from(trimmed)
    };
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        home_dir()?.join(expanded)
    };
    if absolute.extension().and_then(|e| e.to_str()) == Some("dimvault") {
        Ok(absolute)
    } else {
        Ok(absolute.join("vault.dimvault"))
    }
}

fn home_dir() -> Result<PathBuf, ApiError> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| ApiError::new("E_ENV", "Ev klasörü belirlenemedi."))
}

fn ensure_parent(path: &Path) -> Result<(), ApiError> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[tauri::command]
pub fn vault_create(
    state: State<'_, AppState>,
    path: String,
    password: String,
) -> Result<CreateVaultResult, ApiError> {
    let password = Zeroizing::new(password);
    let vault_path = resolve_vault_path(&path)?;
    ensure_parent(&vault_path)?;

    let (file, recovery, unlocked) = VaultFile::create(&vault_path, &password)?;
    let vault_id = vault_id_from_master(unlocked.master_key());
    let db = open_database(&file.path, &unlocked, &vault_id)?;

    let mut guard = state.lock();
    guard.unlocked = Some(unlocked);
    guard.db = Some(db);
    guard.path = Some(file.path.clone());
    guard.vault_id = Some(vault_id.clone());
    drop(guard);

    Ok(CreateVaultResult {
        vault_path: vault_path.to_string_lossy().to_string(),
        recovery_key: recovery.to_display_string(),
        vault_id,
    })
}

#[tauri::command]
pub fn vault_open(
    state: State<'_, AppState>,
    path: String,
    password: String,
) -> Result<VaultInfo, ApiError> {
    let password = Zeroizing::new(password);
    let vault_path = resolve_vault_path(&path)?;
    let unlocked = VaultFile::open(&vault_path, &password)?;
    remember(state, vault_path, unlocked)
}

#[tauri::command]
pub fn vault_open_with_recovery(
    state: State<'_, AppState>,
    path: String,
    recovery_key: String,
) -> Result<VaultInfo, ApiError> {
    let recovery_key = Zeroizing::new(recovery_key);
    let vault_path = resolve_vault_path(&path)?;
    let unlocked = VaultFile::open_with_recovery(&vault_path, &recovery_key)?;
    remember(state, vault_path, unlocked)
}

fn remember(
    state: State<'_, AppState>,
    path: PathBuf,
    unlocked: UnlockedVault,
) -> Result<VaultInfo, ApiError> {
    let vault_id = vault_id_from_master(unlocked.master_key());
    let db = open_database(&path, &unlocked, &vault_id)?;
    let schema_version = db.schema_version();
    let info = VaultInfo {
        vault_path: path.to_string_lossy().to_string(),
        vault_id: vault_id.clone(),
        version: unlocked.header().version,
        created_at_unix: unlocked.header().created_at_unix,
        locked: false,
        schema_version,
    };
    let mut guard = state.lock();
    guard.unlocked = Some(unlocked);
    guard.db = Some(db);
    guard.path = Some(path);
    guard.vault_id = Some(vault_id);
    Ok(info)
}

#[tauri::command]
pub fn vault_lock(state: State<'_, AppState>) -> Result<(), ApiError> {
    let mut guard = state.lock();
    guard.unlocked = None;
    guard.db = None;
    Ok(())
}

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> VaultStatus {
    let guard = state.lock();
    VaultStatus {
        unlocked: guard.db.is_some(),
        vault_path: guard.path.as_ref().map(|p| p.to_string_lossy().to_string()),
        vault_id: guard.vault_id.clone(),
        schema_version: guard.db.as_ref().map(dim_storage::Database::schema_version),
    }
}

#[tauri::command]
pub fn case_create(state: State<'_, AppState>, input: NewCase) -> Result<Case, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.create_case(&input)?)
}

#[tauri::command]
pub fn case_list(
    state: State<'_, AppState>,
    include_archived: Option<bool>,
) -> Result<Vec<Case>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_cases(include_archived.unwrap_or(false))?)
}

#[tauri::command]
pub fn case_get(state: State<'_, AppState>, id: String) -> Result<Option<Case>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.get_case(&id)?)
}

#[tauri::command]
pub fn case_update(
    state: State<'_, AppState>,
    id: String,
    patch: CasePatch,
    row_version: i64,
) -> Result<Case, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.update_case(&id, &patch, row_version)?)
}

#[tauri::command]
pub fn case_archive(state: State<'_, AppState>, id: String) -> Result<Case, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.archive_case(&id)?)
}

#[tauri::command]
pub fn case_restore(state: State<'_, AppState>, id: String) -> Result<Case, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.restore_case(&id)?)
}

#[tauri::command]
pub fn case_delete(state: State<'_, AppState>, id: String) -> Result<(), ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.soft_delete_case(&id)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub app_version: &'static str,
    pub protocol_version: u32,
    pub platform: &'static str,
}

/// Teşhis: sürüm ve platform dışında bilgi dönmez (REQ-033).
#[tauri::command]
pub fn system_info() -> SystemInfo {
    SystemInfo {
        app_version: env!("CARGO_PKG_VERSION"),
        protocol_version: 1,
        platform: std::env::consts::OS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_path_rejected() {
        assert_eq!(resolve_vault_path("   ").unwrap_err().code, "E_PATH");
    }

    #[test]
    fn directory_gets_vault_file_name() {
        let p = resolve_vault_path("DimTest").expect("yol");
        assert!(p.ends_with("DimTest/vault.dimvault") || p.ends_with("DimTest\\vault.dimvault"));
    }

    #[test]
    fn explicit_extension_is_kept() {
        let p = resolve_vault_path("~/DimTest/ozel.dimvault").expect("yol");
        assert!(p.ends_with("ozel.dimvault"));
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)] // IPC yanıtı; kutu serde çıktısını değiştirmez ama gereksiz karmaşıklık
pub enum ImportView {
    Imported {
        document: Document,
        version: DocumentVersion,
    },
    Duplicate {
        document_id: String,
        version_id: String,
        display_name: String,
    },
}

#[tauri::command]
pub fn document_import(
    state: State<'_, AppState>,
    case_id: String,
    path: String,
) -> Result<ImportView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault = guard.unlocked.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault_path = guard.path.as_ref().ok_or_else(ApiError::vault_locked)?;
    let blob_dir = vault_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("blobs");

    let result = import_path(db, vault, &blob_dir, &case_id, Path::new(&path), false)?;
    Ok(match result {
        ImportResult::Imported { document, version } => ImportView::Imported { document, version },
        ImportResult::Duplicate {
            document_id,
            version_id,
            display_name,
        } => ImportView::Duplicate {
            document_id,
            version_id,
            display_name,
        },
    })
}

#[tauri::command]
pub fn document_list(
    state: State<'_, AppState>,
    case_id: String,
) -> Result<Vec<Document>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_documents(&case_id)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPagesView {
    pub parse_run: ParseRun,
    pub pages: Vec<PageRow>,
}

/// Bir sürümün en güncel ayrıştırma koşusunu ve sayfalarını döndürür.
#[tauri::command]
pub fn anchor_create(state: State<'_, AppState>, input: NewAnchor) -> Result<AnchorRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.create_anchor(&input)?)
}

#[tauri::command]
pub fn anchor_list(
    state: State<'_, AppState>,
    case_id: String,
) -> Result<Vec<AnchorRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_anchors(&case_id)?)
}

#[tauri::command]
pub fn document_pages(
    state: State<'_, AppState>,
    version_id: String,
) -> Result<Option<DocumentPagesView>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let runs = db.list_parse_runs(&version_id)?;
    let Some(run) = runs.into_iter().next() else {
        return Ok(None);
    };
    let pages = db.pages_for_run(&run.id)?;
    Ok(Some(DocumentPagesView {
        parse_run: run,
        pages,
    }))
}

#[tauri::command]
pub fn note_create(
    state: State<'_, AppState>,
    case_id: String,
    input: NewNote,
) -> Result<NoteRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.create_note(&case_id, &input)?)
}

#[tauri::command]
pub fn note_list(state: State<'_, AppState>, case_id: String) -> Result<Vec<NoteRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_notes(&case_id)?)
}

#[tauri::command]
pub fn note_delete(state: State<'_, AppState>, id: String) -> Result<(), ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.delete_note(&id)?)
}

#[tauri::command]
pub fn task_create(
    state: State<'_, AppState>,
    case_id: String,
    input: NewTask,
) -> Result<TaskRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.create_task(&case_id, &input)?)
}

#[tauri::command]
pub fn task_list(
    state: State<'_, AppState>,
    case_id: String,
    include_done: Option<bool>,
) -> Result<Vec<TaskRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_tasks(&case_id, include_done.unwrap_or(false))?)
}

#[tauri::command]
pub fn task_complete(state: State<'_, AppState>, id: String) -> Result<TaskRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.complete_task(&id)?)
}

#[tauri::command]
pub fn task_delete(state: State<'_, AppState>, id: String) -> Result<(), ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.soft_delete_task(&id)?)
}

#[tauri::command]
pub fn event_create(
    state: State<'_, AppState>,
    case_id: String,
    input: NewEvent,
) -> Result<EventRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.create_event(&case_id, &input)?)
}

#[tauri::command]
pub fn event_list(state: State<'_, AppState>, case_id: String) -> Result<Vec<EventRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_events(&case_id)?)
}

#[tauri::command]
pub fn event_confirm(state: State<'_, AppState>, id: String) -> Result<EventRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.confirm_event(&id)?)
}

#[tauri::command]
pub fn event_delete(state: State<'_, AppState>, id: String) -> Result<(), ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.soft_delete_event(&id)?)
}

#[tauri::command]
pub fn search_segments(
    state: State<'_, AppState>,
    case_id: String,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<SearchHit>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.search_segments(&case_id, &query, limit.unwrap_or(50).clamp(1, 200))?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseResultView {
    pub parse_run: ParseRun,
    pub pages: usize,
    pub stored_segments: usize,
    pub warnings: Vec<String>,
}

fn spawn_worker() -> Result<WorkerClient, ApiError> {
    let dir = dev_worker_dir().ok_or_else(|| {
        ApiError::new(
            "E_WORKER_MISSING",
            "Belge worker'ı bulunamadı (workers/document).",
        )
    })?;
    let python = std::env::var("DIM_WORKER_PYTHON").unwrap_or_else(|_| "python3".to_string());
    WorkerClient::spawn_dev(Path::new(&python), &dir).map_err(Into::into)
}

/// Belge sürümünü yerel worker'da ayrıştırır. Özgün blob çözülür, metin
/// çıkarılır, sürümlü `parse_run` + sayfa + segment kayıtları yazılır.
#[tauri::command]
pub fn document_parse(
    state: State<'_, AppState>,
    version_id: String,
    ocr: Option<bool>,
    ocr_lang: Option<String>,
) -> Result<ParseResultView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault = guard.unlocked.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault_path = guard.path.as_ref().ok_or_else(ApiError::vault_locked)?;

    let version = db
        .get_version(&version_id)?
        .ok_or_else(|| ApiError::new("E_NOT_FOUND", "Belge sürümü bulunamadı."))?;
    let blob = db
        .blob_for_version(&version_id)?
        .ok_or_else(|| ApiError::new("E_NOT_FOUND", "Belge içeriği bulunamadı."))?;
    let blob_path = vault_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("blobs")
        .join(format!("{}.blb", blob.storage_id));
    let data = vault.read_blob(&blob_path, blob.aad_ref.as_bytes())?;

    let mut worker = spawn_worker()?;
    let extraction = worker.extract_text(
        &data,
        &blob.mime,
        ocr.unwrap_or(false),
        &ocr_lang.unwrap_or_else(|| "eng".to_string()),
    )?;
    worker.shutdown();

    let run = db.start_parse_run(
        &version_id,
        &version.case_id,
        &extraction.engine,
        &extraction.engine_version,
        &blob.mime,
    )?;
    let pages: Vec<NewPage> = extraction
        .pages
        .iter()
        .map(|p| NewPage {
            index: i64::from(p.index),
            text: p.text.clone(),
            width: Some(p.width),
            height: Some(p.height),
            origin: if p.text.trim().is_empty() {
                "none".to_string()
            } else {
                "native".to_string()
            },
            warnings: p.warnings.clone(),
        })
        .collect();
    let stored = db.store_pages(&run.id, &version.case_id, &pages)?;
    let status = if extraction.warnings.is_empty() {
        "succeeded"
    } else {
        "partial"
    };
    db.finish_parse_run(&run.id, status, &extraction.warnings)?;

    Ok(ParseResultView {
        parse_run: db.get_parse_run(&run.id)?.unwrap_or(run),
        pages: pages.len(),
        stored_segments: stored,
        warnings: extraction.warnings,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupView {
    pub path: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub manifest_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreView {
    pub vault_path: String,
    pub file_count: usize,
}

fn sha256_file(path: &Path) -> Result<String, ApiError> {
    let data = std::fs::read(path)?;
    Ok(hex::encode(Sha256::digest(&data)))
}

/// Kasa dizinini (başlık + SQLCipher DB + şifreli blob'lar) tutarlı bir
/// klasöre yedekler. DB `sqlcipher_export` ile alınır; açık metin yazılmaz.
#[tauri::command]
pub fn vault_backup(state: State<'_, AppState>, dest_dir: String) -> Result<BackupView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault = guard.unlocked.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault_path = guard.path.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault_id = guard
        .vault_id
        .clone()
        .ok_or_else(|| ApiError::new("E_VAULT_LOCKED", "Kasa kimliği yok."))?;

    let root = PathBuf::from(dest_dir.trim());
    if dest_dir.trim().is_empty() {
        return Err(ApiError::new("E_PATH", "Yedek konumu boş olamaz."));
    }
    let stamp = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "2026-01-01T00:00:00Z".to_string())
        .replace([':', '-'], "")
        .replace('T', "-")
        .chars()
        .take(15)
        .collect::<String>();
    let backup_dir = root.join(format!("DimYedek-{stamp}"));
    if backup_dir.exists() {
        return Err(ApiError::new("E_BACKUP_EXISTS", "Aynı adlı yedek klasörü zaten var."));
    }
    std::fs::create_dir_all(backup_dir.join("blobs"))?;

    let key = DatabaseKey::derive_from_master(vault.master_key(), &vault_id)?;
    let backup_db = backup_dir.join("vault.db");
    db.backup_to(&backup_db, &key)?;
    std::fs::copy(vault_path, backup_dir.join("vault.dimvault"))?;

    let mut files: Vec<serde_json::Value> = Vec::new();
    let mut total: u64 = 0;
    let blob_src = vault_path.parent().unwrap_or_else(|| Path::new(".")).join("blobs");
    if blob_src.is_dir() {
        for entry in std::fs::read_dir(&blob_src)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let dest = backup_dir.join("blobs").join(&name);
            std::fs::copy(entry.path(), &dest)?;
            let size = std::fs::metadata(&dest)?.len();
            total += size;
            files.push(serde_json::json!({
                "name": name,
                "bytes": size,
                "sha256": sha256_file(&dest)?,
            }));
        }
    }
    let vault_hash = sha256_file(&backup_dir.join("vault.dimvault"))?;
    let db_hash = sha256_file(&backup_db)?;
    let manifest = serde_json::json!({
        "format": "dim-backup",
        "version": 1,
        "createdAt": time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
        "vaultSha256": vault_hash,
        "dbSha256": db_hash,
        "blobs": files,
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|_| ApiError::new("E_BACKUP", "Manifest üretilemedi."))?;
    std::fs::write(backup_dir.join("manifest.json"), &manifest_bytes)?;

    // Yedeğin açılabildiğini doğrula: bozuk yedek "başarılı" sayılmaz.
    {
        let verify = Database::open(&backup_db, &key)?;
        verify.integrity_check()?;
    }

    let manifest_hash = hex::encode(Sha256::digest(&manifest_bytes));
    let file_count = files.len() + 3;
    let _ = db.append_audit("backup", None, "create", "local-user", &serde_json::json!({"files": file_count}));
    Ok(BackupView {
        path: backup_dir.to_string_lossy().to_string(),
        file_count,
        total_bytes: total + std::fs::metadata(&backup_db)?.len() + std::fs::metadata(backup_dir.join("vault.dimvault"))?.len(),
        manifest_hash,
    })
}

/// Yedek klasörünü yeni bir kasa dizinine kopyalar. Hedef boş olmalıdır;
/// mevcut kasanın üzerine yazılmaz. Açma işlemi kullanıcı parolasıyla yapılır.
#[tauri::command]
pub fn vault_restore(backup_dir: String, dest_dir: String) -> Result<RestoreView, ApiError> {
    let src = PathBuf::from(backup_dir.trim());
    let dest = PathBuf::from(dest_dir.trim());
    if backup_dir.trim().is_empty() || dest_dir.trim().is_empty() {
        return Err(ApiError::new("E_PATH", "Yedek ve hedef konumu zorunlu."));
    }
    for required in ["vault.dimvault", "vault.db", "manifest.json"] {
        if !src.join(required).is_file() {
            return Err(ApiError::new("E_RESTORE_INVALID", "Seçilen klasör geçerli bir yedek değil."));
        }
    }
    if dest.join("vault.dimvault").exists() {
        return Err(ApiError::new("E_RESTORE_DEST_NOT_EMPTY", "Hedef klasörde zaten bir kasa var."));
    }
    std::fs::create_dir_all(dest.join("blobs"))?;
    std::fs::copy(src.join("vault.dimvault"), dest.join("vault.dimvault"))?;
    std::fs::copy(src.join("vault.db"), dest.join("vault.db"))?;
    std::fs::copy(src.join("manifest.json"), dest.join("manifest.json"))?;
    let mut count = 0usize;
    let blobs = src.join("blobs");
    if blobs.is_dir() {
        for entry in std::fs::read_dir(&blobs)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                std::fs::copy(entry.path(), dest.join("blobs").join(entry.file_name()))?;
                count += 1;
            }
        }
    }
    Ok(RestoreView { vault_path: dest.to_string_lossy().to_string(), file_count: count + 3 })
}
