use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use base64::Engine as _;
use dim_cli_adapters::{
    discover, find_on_path, run_probe, spec as adapter_spec, DiscoveryState, RunStatus, SPECS,
};
use dim_ingest::budget::{BudgetReport, WorkerBudget, WorkerKind};
use dim_ingest::inventory::DocumentInventory;
use dim_ingest::packaging::{
    read_manifest, verify_package, ManifestTotals, PackageVerification, TesseractStatus,
    WorkerManifest, MANIFEST_FILE,
};
use dim_ingest::worker::{
    dev_worker_dir, ParseProgress, RedactionOutcome, WorkerClient, WorkerError,
};
use dim_ingest::{import_path, ImportResult};
use dim_preview::{PreviewKey, PreviewStore};
use dim_providers::{AiRequest, ApiProvider, ContextSegment, Provider};
use dim_storage::{
    AiCandidateRow, AiRunRow, AnchorLocation, AnchorRow, CandidateReport, Case,
    CaseChecklistItemRow, CasePatch, CaseTagRow, ChecklistCopyResult, ChecklistTemplateRow,
    CitationCheckRow, CitationContext, ClaimRelationRow, ClaimRow, ConsentPreview,
    ContradictionAssessmentRow, CorpusCoverage, CorpusMode, CoverageReport, Database, DatabaseKey,
    Document, DocumentTagRow, DocumentVersion, EventPatch, EventRow, EventSourceRow,
    EvidenceLinkRow, ExportCoverage, ExportRow, GcReport, InTextQuoteCheck, InTextQuoteInput,
    JobQueueSummary, JobRecovery, JobRow, MarkRow, NewAiRun, NewAnchor, NewCase, NewCitationCheck,
    NewClaim, NewClaimRelation, NewEvent, NewEvidenceLink, NewNote, NewPage, NewPrecedent,
    NewPrecedentNote, NewPrecedentText, NewRedactionPlan, NewRegionMark, NewReviewItem, NewTask,
    NewTransferConsent, NoteDraftInput, NoteDraftRow, NoteEdit, NoteRevisionRow, NoteRow,
    OutboxEventRow, PageRow, ParseRun, PrecedentBibliography, PrecedentDocumentText,
    PrecedentNoteRow, PrecedentRow, PrecedentSearchPage, PrecedentSearchQuery, PrecedentUsageTerms,
    PrecedentVersionRow, ProviderProfileRow, QuoteVerification, RebindSuggestion, RedactionPlanRow,
    RedactionRegionInput, ReminderEnqueueResult, ReminderRow, ReviewCommitResult,
    ReviewDecisionRow, ReviewItemRow, ReviewTarget, SearchFilters, SearchHit, SearchMode,
    SearchPage, SearchQuery, SearchTotal, SegmentBlock, SetUsageTermsInput, TaskRow,
    TransferConsentRow,
};
use dim_vault::{UnlockedVault, VaultFile};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use zeroize::Zeroizing;

use crate::error::ApiError;
use crate::state::{
    lock_vault, open_database, vault_id_from_master, AppState, RunningWorker, VaultState,
    WorkerSlotGuard,
};

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
    /// Bu açılışta güncelleme öncesi alınan yedek (varsa).
    pub pre_update_backup: Option<String>,
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
    // F03-T08: açılışta yarım kalmış işler sessizce "yürüyor" görünmesin.
    // Kirası dolmuş işler yeniden sıraya alınır; iptal istenmiş olanlar kapanır.
    // Sayı denetime yazılır, iş kimliği/dosya adı yazılmaz.
    let recovery = db
        .recover_stale_jobs(time::OffsetDateTime::now_utc())
        .unwrap_or(dim_storage::JobRecovery {
            interrupted: 0,
            requeued: 0,
            failed: 0,
            canceled: 0,
            interrupted_job_ids: Vec::new(),
        });
    if recovery.interrupted > 0 || recovery.canceled > 0 {
        let metadata = serde_json::json!({
            "interrupted": recovery.interrupted,
            "requeued": recovery.requeued,
            "failed": recovery.failed,
            "canceled": recovery.canceled,
        });
        let _ = db.append_audit("job_queue", None, "recover", "local-user", &metadata);
    }
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
    // §17.5: kasa kilidi güvenlik önceliğidir — süren ayrıştırmalar **önce**
    // durdurulur; aksi hâlde kilitli kasaya yazmaya çalışan bir süreç kalırdı.
    // (§17.3 ile ayrıştırma kilidi bırakarak çalıştığı için bu gerekli oldu.)
    let _stopped = state.cancel_all_workers();
    let mut guard = state.lock();
    guard.unlocked = None;
    guard.db = None;
    // K0: kilitte çözülmüş önizlemeler bellekten düşer. Diskteki kayıtlar
    // kasa anahtarıyla şifreli kalır, düz metin bırakılmaz.
    state.preview_lock().clear();
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
        pre_update_backup: guard.db.as_ref().and_then(|db| {
            db.pre_migration_backup()
                .map(|path| path.to_string_lossy().to_string())
        }),
    }
}

/// Geri dönüş onayı. Kullanıcı bu metni birebir yazmadan düğme çalışmaz;
/// tek tıkla veri geri alma kazasına izin verilmez.
pub const ROLLBACK_CONFIRMATION: &str = "GERİ DÖN";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreUpdateBackupView {
    /// Seçim anahtarı; istemci **yol** göndermez, bu adı gönderir.
    pub file_name: String,
    pub claimed_version: u32,
    pub verified_version: Option<u32>,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub problem: Option<String>,
    pub usable: bool,
    /// Dosya adı ile dosyanın bildirdiği sürüm ayrışıyor (yeniden adlandırılmış olabilir).
    pub name_mismatch: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackView {
    pub backup_file: String,
    /// Yedeğin taşıdığı, geri konan şema sürümü.
    pub restored_version: u32,
    /// Yeniden açılıştan sonra kasanın bulunduğu (güncel) şema sürümü.
    pub resumed_version: u32,
    /// Geri dönüş öncesi dosyanın saklandığı güvenlik kopyası.
    pub safety_copy: String,
}

/// Kasa açıkken kullanılabilir geri dönüş bağlamı: veritabanı yolu ve anahtar.
fn rollback_context(guard: &crate::state::VaultState) -> Result<(PathBuf, DatabaseKey), ApiError> {
    let vault = guard.unlocked.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault_id = guard
        .vault_id
        .as_deref()
        .ok_or_else(ApiError::vault_locked)?;
    let vault_path = guard.path.as_ref().ok_or_else(ApiError::vault_locked)?;
    let key = DatabaseKey::derive_from_master(vault.master_key(), vault_id)?;
    let db_path = vault_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("vault.db");
    Ok((db_path, key))
}

/// Kasanın yanındaki güncelleme öncesi yedekleri listeler ve doğrular.
/// Doğrulama yedeği değiştirmez; liste boşsa boş döner (uydurma kayıt yok).
#[tauri::command]
pub fn vault_pre_update_backups(
    state: State<'_, AppState>,
) -> Result<Vec<PreUpdateBackupView>, ApiError> {
    let guard = state.lock();
    let (db_path, key) = rollback_context(&guard)?;
    let list = dim_storage::pre_update_backups(&db_path, &key)?;
    Ok(list
        .into_iter()
        .map(|backup| {
            // Türetilmiş alanlar taşımadan **önce** hesaplanır.
            let usable = backup.usable();
            let name_mismatch = backup.name_mismatch();
            PreUpdateBackupView {
                file_name: backup.file_name,
                claimed_version: backup.claimed_version,
                verified_version: backup.verified_version,
                size_bytes: backup.size_bytes,
                modified_at: backup.modified_at,
                problem: backup.problem,
                usable,
                name_mismatch,
            }
        })
        .collect())
}

/// Yedek seçimi yalnız **dosya adıyla** yapılır; istemciden yol kabul edilmez.
/// Ad, listede bulunanla birebir eşleşmek zorunda.
fn rollback_backup_name(input: &str) -> Result<&str, ApiError> {
    let name = input.trim();
    if name.is_empty()
        || name != input
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
    {
        return Err(ApiError::new(
            "E_ROLLBACK_BACKUP",
            "Yedek adı geçersiz. Yedek listeden seçilir.",
        ));
    }
    Ok(name)
}

/// Onay metni birebir eşleşmeli (büyük/küçük harf ve Türkçe harfler dahil).
fn rollback_confirmed(input: &str) -> bool {
    input.trim() == ROLLBACK_CONFIRMATION
}

/// Seçilen güncelleme öncesi yedeğe **yerinde** döner: kasayı kilitler, yedeği
/// geri koyar, kasayı yeniden açar ve güncel şemaya ilerletir.
///
/// İkili (binary) sürüm düşürülmez: veri güncelleme öncesi hâline döner, şema
/// bu uygulamanın sürümüne yeniden yükseltilir. Geri dönüş öncesi dosya
/// güvenlik kopyası olarak saklanır.
#[tauri::command]
pub fn vault_rollback_pre_update(
    state: State<'_, AppState>,
    backup_file: String,
    confirmation: String,
) -> Result<RollbackView, ApiError> {
    if !rollback_confirmed(&confirmation) {
        return Err(ApiError::new(
            "E_ROLLBACK_CONFIRM",
            "Geri dönüş için onay metni birebir yazılmalı: GERİ DÖN",
        ));
    }
    let name = rollback_backup_name(&backup_file)?;

    let mut guard = state.lock();
    let (db_path, key) = rollback_context(&guard)?;
    let backup = dim_storage::pre_update_backups(&db_path, &key)?
        .into_iter()
        .find(|entry| entry.file_name == name)
        .ok_or_else(|| {
            ApiError::new(
                "E_ROLLBACK_BACKUP",
                "Bu ada sahip güncelleme öncesi yedek bulunamadı.",
            )
        })?;
    if !backup.usable() {
        return Err(ApiError::new(
            "E_ROLLBACK_BACKUP",
            "Seçilen yedek doğrulanamadı; geri dönüş başlatılmadı.",
        ));
    }

    // İstek, geri dönüşten **önceki** kasaya yazılır; tamamlanma kaydı geri
    // konan kasaya yazılır. İkisi birlikte tek bir olayı anlatır.
    if let Some(db) = guard.db.as_ref() {
        let _ = db.append_audit(
            "vault_rollback",
            None,
            "requested",
            "local-user",
            &serde_json::json!({
                "fromSchema": db.schema_version(),
                "backupVersion": backup.verified_version,
            }),
        );
    }

    // Dosya yerine konmadan önce bütün bağlantılar kapanmalı (Windows kilidi).
    guard.db = None;
    state.preview_lock().clear();

    match dim_storage::rollback_and_resume(&db_path, &backup.path, &key) {
        Ok(resumed) => {
            let resumed_version = resumed.database.schema_version();
            guard.db = Some(resumed.database);
            Ok(RollbackView {
                backup_file: backup.file_name,
                restored_version: resumed.restored_version,
                resumed_version,
                safety_copy: resumed.safety_copy.to_string_lossy().to_string(),
            })
        }
        Err(err) => {
            // Geri dönüş başlamadıysa kasa eski hâlinde yeniden açılır; kullanıcı
            // kilitli kalmaz. `E_DB_ROLLBACK_REOPEN` ise dosya zaten geri konmuş
            // durumdadır ve hata mesajı hangi sürümün geri konduğunu taşır.
            if let Ok(database) = Database::open(&db_path, &key) {
                guard.db = Some(database);
            }
            Err(ApiError::from(err))
        }
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

// Testler ayrı dosyada: `commands.rs` yalnız komut yüzeyini taşır.
#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
