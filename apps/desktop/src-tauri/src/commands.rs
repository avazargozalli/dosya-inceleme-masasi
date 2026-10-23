use std::path::{Path, PathBuf};

use dim_cli_adapters::{discover, DiscoveryState, SPECS};
use dim_ingest::worker::{dev_worker_dir, WorkerClient};
use dim_ingest::{import_path, ImportResult};
use dim_storage::{
    AiCandidateRow, AiRunRow, AnchorLocation, AnchorRow, Case, CasePatch, CitationCheckRow,
    ClaimRow, CoverageReport, Database, DatabaseKey, Document, DocumentVersion, EventRow,
    EventSourceRow, EvidenceLinkRow, ExportRow, NewAnchor, NewCase, NewCitationCheck, NewClaim,
    NewEvent, NewEvidenceLink, NewNote, NewPage, NewPrecedent, NewPrecedentNote, NewRedactionPlan,
    NewReviewItem, NewTask, NewTransferConsent, NoteRow, PageRow, ParseRun, PrecedentNoteRow,
    PrecedentRow, ProviderProfileRow, RedactionPlanRow, RedactionRegionInput, ReviewDecisionRow,
    ReviewItemRow, SearchHit, SegmentBlock, TaskRow, TransferConsentRow,
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

/// Anchor'ı okuyucuda açılacak sayfaya çözer (F07 "kaynağa git").
#[tauri::command]
pub fn anchor_locate(
    state: State<'_, AppState>,
    anchor_id: String,
) -> Result<Option<AnchorLocation>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.locate_anchor(&anchor_id)?)
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

/// Paketlenmiş worker ikilisini arar. Sıra: `DIM_WORKER_EXE`, uygulama yanındaki
/// `worker/`, macOS `.app/Contents/Resources/worker/`, Windows `resources/worker/`.
fn bundled_worker_exe() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DIM_WORKER_EXE") {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(windows) {
        "dim-worker.exe"
    } else {
        "dim-worker"
    };
    for candidate in [
        dir.join("worker").join(name),
        dir.join("..").join("Resources").join("worker").join(name),
        dir.join("resources").join("worker").join(name),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn spawn_worker() -> Result<WorkerClient, ApiError> {
    if let Some(exe) = bundled_worker_exe() {
        return WorkerClient::spawn_bundled(&exe).map_err(Into::into);
    }
    let dir = dev_worker_dir().ok_or_else(|| {
        ApiError::new(
            "E_WORKER_MISSING",
            "Belge worker'ı bulunamadı: paket içinde worker yok, geliştirme dizini de yok.",
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
    password: Option<String>,
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
    let caps = worker
        .capabilities()
        .unwrap_or_else(|_| serde_json::json!({}));
    let ocr_enabled = ocr.unwrap_or(false);
    let lang = ocr_lang.unwrap_or_else(|| "eng".to_string());
    let extraction =
        worker.extract_text(&data, &blob.mime, ocr_enabled, &lang, password.as_deref())?;
    worker.shutdown();

    // Ayar özeti gerçek girdileri taşır: MIME, OCR bayrağı, dil, motor sürümü
    // ve kurulu tessdata parmak izi. Yalnız MIME yazmak yanıltıcıydı.
    let tessdata = caps
        .pointer("/ocr/tessdataHash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let settings = format!(
        "{}|ocr={}|lang={}|engine={}|tessdata={tessdata}",
        blob.mime, ocr_enabled, lang, extraction.engine_version
    );
    let settings_hash = hex::encode(Sha256::digest(settings.as_bytes()));
    let run = db.start_parse_run(
        &version_id,
        &version.case_id,
        &extraction.engine,
        &extraction.engine_version,
        &settings_hash,
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
            rotation: i64::from(p.rotation % 360),
            coordinate_space: p.coordinate_space.clone(),
            blocks: p
                .blocks
                .iter()
                .map(|b| SegmentBlock {
                    start: i64::from(b.start),
                    end: i64::from(b.end),
                    bbox: b.bbox,
                    lines: b.lines.clone(),
                })
                .collect(),
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
    /// Tek dosyalık `.dimbak` arşivi (içerik zaten şifreli).
    pub archive_path: Option<String>,
}

/// Klasörü tek dosyalık arşive paketler. Arşiv şifrelemez; içindeki DB ve
/// blob'lar zaten kasa anahtarıyla şifreli olduğu için açık metin sızmaz.
fn zip_dir(src: &Path, dest: &Path) -> Result<(), ApiError> {
    use std::io::Write as _;

    let file = std::fs::File::create(dest)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path
                .strip_prefix(src)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            writer
                .start_file(name, options)
                .map_err(|e| ApiError::new("E_BACKUP_ZIP", &e.to_string()))?;
            writer.write_all(&std::fs::read(&path)?)?;
        }
    }
    writer
        .finish()
        .map_err(|e| ApiError::new("E_BACKUP_ZIP", &e.to_string()))?;
    Ok(())
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
        return Err(ApiError::new(
            "E_BACKUP_EXISTS",
            "Aynı adlı yedek klasörü zaten var.",
        ));
    }
    std::fs::create_dir_all(backup_dir.join("blobs"))?;

    let key = DatabaseKey::derive_from_master(vault.master_key(), &vault_id)?;
    let backup_db = backup_dir.join("vault.db");
    db.backup_to(&backup_db, &key)?;
    std::fs::copy(vault_path, backup_dir.join("vault.dimvault"))?;

    let mut files: Vec<serde_json::Value> = Vec::new();
    let mut total: u64 = 0;
    let blob_src = vault_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("blobs");
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
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|_| ApiError::new("E_BACKUP", "Manifest üretilemedi."))?;
    std::fs::write(backup_dir.join("manifest.json"), &manifest_bytes)?;

    // Yedeğin açılabildiğini doğrula: bozuk yedek "başarılı" sayılmaz.
    {
        let verify = Database::open(&backup_db, &key)?;
        verify.integrity_check()?;
    }

    let manifest_hash = hex::encode(Sha256::digest(&manifest_bytes));
    let file_count = files.len() + 3;
    let _ = db.append_audit(
        "backup",
        None,
        "create",
        "local-user",
        &serde_json::json!({"files": file_count}),
    );
    let archive = root.join(format!("DimYedek-{stamp}.dimbak"));
    zip_dir(&backup_dir, &archive)?;
    Ok(BackupView {
        path: backup_dir.to_string_lossy().to_string(),
        file_count,
        total_bytes: total
            + std::fs::metadata(&backup_db)?.len()
            + std::fs::metadata(backup_dir.join("vault.dimvault"))?.len(),
        manifest_hash,
        archive_path: Some(archive.to_string_lossy().to_string()),
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
            return Err(ApiError::new(
                "E_RESTORE_INVALID",
                "Seçilen klasör geçerli bir yedek değil.",
            ));
        }
    }
    if dest.join("vault.dimvault").exists() {
        return Err(ApiError::new(
            "E_RESTORE_DEST_NOT_EMPTY",
            "Hedef klasörde zaten bir kasa var.",
        ));
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
    Ok(RestoreView {
        vault_path: dest.to_string_lossy().to_string(),
        file_count: count + 3,
    })
}

#[tauri::command]
pub fn claim_create(
    state: State<'_, AppState>,
    case_id: String,
    input: NewClaim,
) -> Result<ClaimRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.create_claim(&case_id, &input)?)
}

#[tauri::command]
pub fn claim_list(state: State<'_, AppState>, case_id: String) -> Result<Vec<ClaimRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_claims(&case_id)?)
}

#[tauri::command]
pub fn evidence_link_create(
    state: State<'_, AppState>,
    case_id: String,
    input: NewEvidenceLink,
) -> Result<EvidenceLinkRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.link_evidence(&case_id, &input)?)
}

#[tauri::command]
pub fn evidence_link_list(
    state: State<'_, AppState>,
    claim_id: String,
) -> Result<Vec<EvidenceLinkRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_evidence_links(&claim_id)?)
}

#[tauri::command]
pub fn review_item_create(
    state: State<'_, AppState>,
    case_id: String,
    input: NewReviewItem,
) -> Result<ReviewItemRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.create_review_item(&case_id, &input)?)
}

#[tauri::command]
pub fn review_item_list(
    state: State<'_, AppState>,
    case_id: String,
    review_state: Option<String>,
) -> Result<Vec<ReviewItemRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_review_items(&case_id, review_state.as_deref())?)
}

#[tauri::command]
pub fn review_decide(
    state: State<'_, AppState>,
    item_id: String,
    decision: String,
    edited_payload: Option<serde_json::Value>,
    reviewer: String,
    rationale: String,
) -> Result<ReviewItemRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.decide_review_item(
        &item_id,
        &decision,
        edited_payload.as_ref(),
        &reviewer,
        &rationale,
    )?)
}

#[tauri::command]
pub fn review_history(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<Vec<ReviewDecisionRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.review_history(&item_id)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterView {
    pub id: &'static str,
    pub display_name: &'static str,
    pub command: &'static str,
    pub state: &'static str,
    pub version: Option<String>,
    pub auth_mode: &'static str,
    pub batch_forbidden: bool,
    pub allowed_args: &'static [&'static str],
}

/// CLI adaptörlerini yoklar: yalnız `--version` çalıştırılır. Kimlik dosyası
/// okunmaz, oturum açılmaz, istek gönderilmez.
#[tauri::command]
pub fn provider_probe() -> Vec<AdapterView> {
    SPECS
        .iter()
        .map(|spec| {
            let discovery = discover(spec);
            let (state, version) = match discovery.state {
                DiscoveryState::Installed { version, .. } => ("installed", Some(version)),
                DiscoveryState::Unverified { .. } => ("unverified", None),
                DiscoveryState::NotInstalled => ("not_installed", None),
            };
            AdapterView {
                id: spec.id,
                display_name: spec.display_name,
                command: spec.command,
                state,
                version,
                auth_mode: match spec.auth_mode {
                    dim_cli_adapters::AuthMode::Subscription => "subscription",
                    dim_cli_adapters::AuthMode::ApiKey => "api_key",
                    dim_cli_adapters::AuthMode::InteractiveOnly => "interactive_only",
                },
                batch_forbidden: spec.batch_forbidden,
                allowed_args: spec.allowed_args,
            }
        })
        .collect()
}

#[tauri::command]
pub fn precedent_create(
    state: State<'_, AppState>,
    input: NewPrecedent,
) -> Result<PrecedentRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.create_precedent(&input)?)
}

#[tauri::command]
pub fn precedent_list(state: State<'_, AppState>) -> Result<Vec<PrecedentRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_precedents()?)
}

#[tauri::command]
pub fn precedent_note_create(
    state: State<'_, AppState>,
    precedent_id: String,
    input: NewPrecedentNote,
) -> Result<PrecedentNoteRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.create_precedent_note(&precedent_id, &input)?)
}

#[tauri::command]
pub fn precedent_note_list(
    state: State<'_, AppState>,
    precedent_id: String,
) -> Result<Vec<PrecedentNoteRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_precedent_notes(&precedent_id)?)
}

#[tauri::command]
pub fn citation_check_create(
    state: State<'_, AppState>,
    input: NewCitationCheck,
) -> Result<CitationCheckRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.record_citation_check(&input)?)
}

#[tauri::command]
pub fn citation_check_list(
    state: State<'_, AppState>,
    version_id: String,
) -> Result<Vec<CitationCheckRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_citation_checks(&version_id)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactionView {
    pub document: Document,
    pub version: DocumentVersion,
    pub plan: RedactionPlanRow,
    pub hits: usize,
    pub remaining: Vec<String>,
    pub ocr_verified: bool,
    pub engines: Vec<String>,
}

/// Terimleri belgeden gerçekten çıkarır, çıktıyı yeni bir sürüm olarak kaydeder.
/// Worker doğrulaması geçmezse hiçbir çıktı kaydedilmez.
#[tauri::command]
pub fn document_redact(
    state: State<'_, AppState>,
    version_id: String,
    terms: Vec<String>,
) -> Result<RedactionView, ApiError> {
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
    let outcome = worker.redact(&data, &terms)?;
    worker.shutdown();

    if !outcome.remaining.is_empty() {
        return Err(ApiError::new(
            "E_REDACT_VERIFY",
            "Maskeli çıktıda hâlâ aranan metin var; dosya kaydedilmedi.",
        ));
    }

    let out_sha = hex::encode(Sha256::digest(&outcome.pdf));
    let aad = format!("{}:redacted:{}", version.case_id, out_sha);
    let out_path = vault.write_blob(
        &vault_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("blobs"),
        &outcome.pdf,
        aad.as_bytes(),
    )?;
    let out_size = i64::try_from(std::fs::metadata(&out_path)?.len()).unwrap_or(i64::MAX);
    let storage_id = out_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("blob")
        .to_string();
    let out_blob_id = db.create_blob_record(&storage_id, out_size, &aad)?;

    let source_doc = db
        .get_document(&version.document_id)?
        .ok_or_else(|| ApiError::new("E_NOT_FOUND", "Belge bulunamadı."))?;
    let new_doc = db.create_document(
        &version.case_id,
        &format!("{} (maskeli)", source_doc.display_name),
        "pdf",
        None,
        "unknown",
    )?;
    let new_version = db.add_version_with_parent(
        &new_doc.id,
        &version.case_id,
        &out_blob_id,
        &out_sha,
        i64::try_from(outcome.pdf.len()).unwrap_or(i64::MAX),
        "application/pdf",
        Some(&version_id),
    )?;

    let plan = db.create_redaction_plan(
        &version.case_id,
        &NewRedactionPlan {
            document_version_id: version_id.clone(),
            parse_run_id: None,
            render_hash: version.original_sha256.clone(),
            output_profile: "share".to_string(),
        },
    )?;
    let regions: Vec<RedactionRegionInput> = outcome
        .hits
        .iter()
        .map(|hit| RedactionRegionInput {
            page_index: i64::from(hit.page),
            rect: hit.rect,
            term_hash: hex::encode(Sha256::digest(hit.term.as_bytes())),
        })
        .collect();
    db.add_redaction_regions(&plan.id, &regions)?;

    Ok(RedactionView {
        document: new_doc,
        version: new_version,
        plan,
        hits: outcome.hits.len(),
        remaining: outcome.remaining,
        ocr_verified: outcome.ocr_verified,
        engines: outcome.engines,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportView {
    pub path: String,
    pub bytes: usize,
    pub sha256: String,
    pub record: ExportRow,
}

fn write_export(path: &str, bytes: &[u8]) -> Result<(), ApiError> {
    let target = PathBuf::from(path.trim());
    if path.trim().is_empty() {
        return Err(ApiError::new("E_PATH", "Çıktı konumu boş olamaz."));
    }
    if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&target, bytes)?;
    Ok(())
}

fn case_payload(db: &Database, case_id: &str) -> Result<serde_json::Value, ApiError> {
    let case = db
        .get_case(case_id)?
        .ok_or_else(|| ApiError::new("E_NOT_FOUND", "Dosya bulunamadı."))?;
    let documents = db.list_documents(case_id)?;
    let events = db.list_events(case_id)?;
    let claims = db.list_claims(case_id)?;
    let notes = db.list_notes(case_id)?;
    let tasks = db.list_tasks(case_id, true)?;
    let anchors = db.list_anchors(case_id)?;
    let mut evidence = Vec::new();
    for claim in &claims {
        for link in db.list_evidence_links(&claim.id)? {
            evidence.push(serde_json::json!({
                "claimId": link.claim_id,
                "anchorId": link.source_anchor_id,
                "relation": link.relation,
                "rationale": link.rationale,
            }));
        }
    }
    Ok(serde_json::json!({
        "format": "dim-case-export",
        "version": 1,
        "exportedAt": dim_storage::now_iso(),
        "case": case,
        "documents": documents,
        "events": events,
        "claims": claims,
        "evidenceLinks": evidence,
        "notes": notes,
        "tasks": tasks,
        "anchors": anchors,
    }))
}

#[tauri::command]
pub fn export_json(
    state: State<'_, AppState>,
    case_id: String,
    dest_path: String,
) -> Result<ExportView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let payload = case_payload(db, &case_id)?;
    let bytes = serde_json::to_vec_pretty(&payload)
        .map_err(|_| ApiError::new("E_EXPORT", "JSON üretilemedi."))?;
    write_export(&dest_path, &bytes)?;
    let sha = hex::encode(Sha256::digest(&bytes));
    let record = db.record_export(&case_id, "json", &[], "user_selected_path", &sha)?;
    Ok(ExportView {
        path: dest_path,
        bytes: bytes.len(),
        sha256: sha,
        record,
    })
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[tauri::command]
pub fn export_csv(
    state: State<'_, AppState>,
    case_id: String,
    dest_path: String,
) -> Result<ExportView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let mut out = String::from("kayit,tarih,tur,baslik,durum\n");
    for event in db.list_events(&case_id)? {
        out.push_str(&format!(
            "olay,{},{},{},{}\n",
            csv_escape(event.sortable_date.as_deref().unwrap_or("")),
            csv_escape(&event.date_role),
            csv_escape(&event.title),
            csv_escape(&event.status)
        ));
    }
    for task in db.list_tasks(&case_id, true)? {
        out.push_str(&format!(
            "gorev,{},{},{},{}\n",
            csv_escape(task.user_due_date.as_deref().unwrap_or("")),
            csv_escape("gorev"),
            csv_escape(&task.title),
            csv_escape(&task.status)
        ));
    }
    let bytes = out.into_bytes();
    write_export(&dest_path, &bytes)?;
    let sha = hex::encode(Sha256::digest(&bytes));
    let record = db.record_export(&case_id, "csv", &[], "user_selected_path", &sha)?;
    Ok(ExportView {
        path: dest_path,
        bytes: bytes.len(),
        sha256: sha,
        record,
    })
}

#[tauri::command]
pub fn export_docx(
    state: State<'_, AppState>,
    case_id: String,
    dest_path: String,
) -> Result<ExportView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let case = db
        .get_case(&case_id)?
        .ok_or_else(|| ApiError::new("E_NOT_FOUND", "Dosya bulunamadı."))?;
    let events = db.list_events(&case_id)?;
    let claims = db.list_claims(&case_id)?;
    let documents = db.list_documents(&case_id)?;
    let notes = db.list_notes(&case_id)?;
    let tasks = db.list_tasks(&case_id, true)?;

    let event_rows: Vec<Vec<String>> = std::iter::once(vec![
        "Tarih".to_string(),
        "Tür".to_string(),
        "Başlık".to_string(),
        "Kesinlik".to_string(),
        "Durum".to_string(),
    ])
    .chain(events.iter().map(|e| {
        vec![
            e.sortable_date
                .clone()
                .unwrap_or_else(|| "bilinmiyor".into()),
            e.date_role.clone(),
            e.title.clone(),
            e.date_precision.clone(),
            e.status.clone(),
        ]
    }))
    .collect();
    let claim_rows: Vec<Vec<String>> =
        std::iter::once(vec!["İddia".to_string(), "Tür".to_string()])
            .chain(
                claims
                    .iter()
                    .map(|c| vec![c.claim_text.clone(), c.claim_kind.clone()]),
            )
            .collect();
    let document_rows: Vec<Vec<String>> = std::iter::once(vec![
        "Belge".to_string(),
        "Tür".to_string(),
        "Durum".to_string(),
    ])
    .chain(documents.iter().map(|d| {
        vec![
            d.display_name.clone(),
            d.document_type.clone(),
            d.status.clone(),
        ]
    }))
    .collect();
    let task_rows: Vec<Vec<String>> = std::iter::once(vec![
        "Görev".to_string(),
        "Tarih".to_string(),
        "Durum".to_string(),
    ])
    .chain(tasks.iter().map(|t| {
        vec![
            t.title.clone(),
            t.user_due_date.clone().unwrap_or_default(),
            t.status.clone(),
        ]
    }))
    .collect();

    let payload = serde_json::json!({
        "title": format!("{} — dosya raporu", case.title),
        "meta": {
            "Dosya": case.title,
            "Esas/referans": case.reference_code.clone().unwrap_or_default(),
            "Tür": case.case_kind,
            "Hassasiyet": case.sensitivity,
            "Oluşturma": case.created_at,
        },
        "sections": [
            {"heading": "Kronoloji", "rows": event_rows},
            {"heading": "İddialar", "rows": claim_rows},
            {"heading": "Belge dizini", "rows": document_rows},
            {"heading": "Görevler", "rows": task_rows},
            {"heading": "Notlar", "lines": notes.iter().map(|n| format!("{}: {}", if n.title.is_empty() { "Not" } else { &n.title }, n.body)).collect::<Vec<_>>()},
        ],
    });

    let mut worker = spawn_worker()?;
    let bytes = worker.render_report(&payload)?;
    worker.shutdown();
    write_export(&dest_path, &bytes)?;
    let sha = hex::encode(Sha256::digest(&bytes));
    let record = db.record_export(&case_id, "docx", &[], "user_selected_path", &sha)?;
    Ok(ExportView {
        path: dest_path,
        bytes: bytes.len(),
        sha256: sha,
        record,
    })
}

#[tauri::command]
pub fn export_pdf(
    state: State<'_, AppState>,
    case_id: String,
    dest_path: String,
) -> Result<ExportView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let case = db
        .get_case(&case_id)?
        .ok_or_else(|| ApiError::new("E_NOT_FOUND", "Dosya bulunamadı."))?;
    let events = db.list_events(&case_id)?;
    let claims = db.list_claims(&case_id)?;
    let documents = db.list_documents(&case_id)?;
    let notes = db.list_notes(&case_id)?;
    let tasks = db.list_tasks(&case_id, true)?;

    let payload = serde_json::json!({
        "title": format!("{} — dosya raporu", case.title),
        "meta": {
            "Dosya": case.title,
            "Esas/referans": case.reference_code.clone().unwrap_or_default(),
            "Tür": case.case_kind,
            "Hassasiyet": case.sensitivity,
            "Oluşturma": case.created_at,
        },
        "sections": [
            {"heading": "Kronoloji", "rows": std::iter::once(vec!["Tarih".to_string(),"Tür".to_string(),"Başlık".to_string(),"Kesinlik".to_string(),"Durum".to_string()]).chain(events.iter().map(|e| vec![e.sortable_date.clone().unwrap_or_else(|| "bilinmiyor".into()), e.date_role.clone(), e.title.clone(), e.date_precision.clone(), e.status.clone()])).collect::<Vec<_>>()},
            {"heading": "İddialar", "rows": std::iter::once(vec!["İddia".to_string(),"Tür".to_string()]).chain(claims.iter().map(|c| vec![c.claim_text.clone(), c.claim_kind.clone()])).collect::<Vec<_>>()},
            {"heading": "Belge dizini", "rows": std::iter::once(vec!["Belge".to_string(),"Tür".to_string(),"Durum".to_string()]).chain(documents.iter().map(|d| vec![d.display_name.clone(), d.document_type.clone(), d.status.clone()])).collect::<Vec<_>>()},
            {"heading": "Görevler", "rows": std::iter::once(vec!["Görev".to_string(),"Tarih".to_string(),"Durum".to_string()]).chain(tasks.iter().map(|t| vec![t.title.clone(), t.user_due_date.clone().unwrap_or_default(), t.status.clone()])).collect::<Vec<_>>()},
            {"heading": "Notlar", "lines": notes.iter().map(|n| format!("{}: {}", if n.title.is_empty() { "Not" } else { &n.title }, n.body)).collect::<Vec<_>>()},
        ],
    });

    let mut worker = spawn_worker()?;
    let bytes = worker.render_report_pdf(&payload)?;
    worker.shutdown();
    write_export(&dest_path, &bytes)?;
    let sha = hex::encode(Sha256::digest(&bytes));
    let record = db.record_export(&case_id, "pdf", &[], "user_selected_path", &sha)?;
    Ok(ExportView {
        path: dest_path,
        bytes: bytes.len(),
        sha256: sha,
        record,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageView {
    pub path: String,
    pub files: usize,
    pub sha256: String,
    pub record: ExportRow,
}

/// Devir paketi: dosya metadatası + rapor iskeleti + manifest. Belge baytları
/// pakete otomatik konmaz; kullanıcı maskeli kopyaları ayrıca ekler.
#[tauri::command]
pub fn export_package(
    state: State<'_, AppState>,
    case_id: String,
    dest_dir: String,
) -> Result<PackageView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    if dest_dir.trim().is_empty() {
        return Err(ApiError::new("E_PATH", "Paket konumu boş olamaz."));
    }
    let payload = case_payload(db, &case_id)?;
    let stamp = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
        .replace([':', '-'], "")
        .chars()
        .take(13)
        .collect::<String>();
    let dir = PathBuf::from(dest_dir.trim()).join(format!("DimDevir-{stamp}"));
    if dir.exists() {
        return Err(ApiError::new(
            "E_PACKAGE_EXISTS",
            "Aynı adlı paket klasörü var.",
        ));
    }
    std::fs::create_dir_all(&dir)?;

    let case_bytes = serde_json::to_vec_pretty(&payload)
        .map_err(|_| ApiError::new("E_EXPORT", "JSON üretilemedi."))?;
    std::fs::write(dir.join("case.json"), &case_bytes)?;
    let readme = "Dosya İnceleme Masası — devir paketi\n\nBu klasör dosya metadatasını (case.json) ve manifest'i taşır.\nBelge baytları kasa dışına otomatik çıkarılmaz; paylaşılacak belgeler için\nMaskeleme ekranından maskeli kopya üretip bu klasöre ekleyin.\n\nPaket açık metindir; şifreli taşıma F16 kapsamındadır.\n";
    std::fs::write(dir.join("README.txt"), readme)?;

    let files = [
        ("case.json", sha256_file(&dir.join("case.json"))?),
        ("README.txt", sha256_file(&dir.join("README.txt"))?),
    ];
    let manifest = serde_json::json!({
        "format": "dim-handoff",
        "version": 1,
        "createdAt": dim_storage::now_iso(),
        "files": files.iter().map(|(name, hash)| serde_json::json!({"name": name, "sha256": hash})).collect::<Vec<_>>(),
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|_| ApiError::new("E_EXPORT", "Manifest üretilemedi."))?;
    std::fs::write(dir.join("manifest.json"), &manifest_bytes)?;
    let sha = hex::encode(Sha256::digest(&manifest_bytes));
    let record = db.record_export(&case_id, "package", &[], "user_selected_path", &sha)?;
    Ok(PackageView {
        path: dir.to_string_lossy().to_string(),
        files: 3,
        sha256: sha,
        record,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiOverview {
    pub providers: Vec<ProviderProfileRow>,
    pub runs: Vec<AiRunRow>,
    pub coverage: CoverageReport,
}

#[tauri::command]
pub fn ai_overview(state: State<'_, AppState>, case_id: String) -> Result<AiOverview, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    db.ensure_off_provider()?;
    Ok(AiOverview {
        providers: db.list_provider_profiles()?,
        runs: db.list_ai_runs(&case_id)?,
        coverage: db.coverage_report(&case_id)?,
    })
}

#[tauri::command]
pub fn ai_consent_create(
    state: State<'_, AppState>,
    case_id: String,
    provider_snapshot: String,
    purpose: String,
    document_ids: Vec<String>,
) -> Result<TransferConsentRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.create_transfer_consent(&NewTransferConsent {
        provider_snapshot,
        case_scope: case_id,
        purpose,
        selected_document_ids: document_ids,
        expires_at: None,
        consent_text_version: "1".into(),
    })?)
}

#[tauri::command]
pub fn ai_consent_revoke(state: State<'_, AppState>, consent_id: String) -> Result<(), ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    db.revoke_transfer_consent(&consent_id)?;
    Ok(())
}

#[tauri::command]
pub fn ai_run_start(
    state: State<'_, AppState>,
    case_id: String,
    provider_id: String,
    consent_id: String,
    task_type: String,
) -> Result<AiRunRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let profile = db
        .list_provider_profiles()?
        .into_iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| ApiError::new("E_AI_PROVIDER", "Sağlayıcı profili bulunamadı."))?;
    if profile.provider_type == "off" {
        return Err(ApiError::new(
            "E_AI_OFF",
            "AI kapalı. Sağlayıcı bağlanmadan hiçbir içerik cihazdan çıkmaz.",
        ));
    }
    Ok(db.start_ai_run(
        &case_id,
        &dim_storage::NewAiRun {
            task_type,
            provider_id,
            model_snapshot: "unknown".into(),
            input_manifest_hash: "pending".into(),
            prompt_version: "1".into(),
            output_schema_version: "1".into(),
            consent_id,
        },
    )?)
}

#[tauri::command]
pub fn ai_candidates_list(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<Vec<AiCandidateRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_ai_candidates(&run_id)?)
}

#[tauri::command]
pub fn ai_candidate_promote(
    state: State<'_, AppState>,
    candidate_id: String,
    case_id: String,
) -> Result<String, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.promote_candidate(&candidate_id, &case_id)?)
}

#[tauri::command]
pub fn event_sources(
    state: State<'_, AppState>,
    event_id: String,
) -> Result<Vec<EventSourceRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_event_sources(&event_id)?)
}

#[tauri::command]
pub fn event_link_source(
    state: State<'_, AppState>,
    event_id: String,
    anchor_id: String,
    relation: String,
) -> Result<EventSourceRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.link_event_source(&event_id, &anchor_id, &relation)?)
}

#[tauri::command]
pub fn event_unlink_source(state: State<'_, AppState>, id: String) -> Result<(), ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    db.unlink_event_source(&id)?;
    Ok(())
}
