use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use base64::Engine as _;
use dim_cli_adapters::{
    discover, find_on_path, run_probe, spec as adapter_spec, DiscoveryState, RunStatus, SPECS,
};
use dim_ingest::inventory::DocumentInventory;
use dim_ingest::packaging::{
    read_manifest, verify_package, ManifestTotals, PackageVerification, TesseractStatus,
    WorkerManifest, MANIFEST_FILE,
};
use dim_ingest::worker::{dev_worker_dir, ParseProgress, WorkerClient};
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
    lock_vault, open_database, vault_id_from_master, AppState, ParseSlotGuard, RunningParse,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_path_rejected() {
        assert_eq!(resolve_vault_path("   ").unwrap_err().code, "E_PATH");
    }

    /// **F04-T07 ilerleme olayı:** yalnız sürüm kimliği, aşama ve sayaç taşır;
    /// belge metni, dosya adı veya yol yayımlanmaz.
    #[test]
    fn parse_progress_event_carries_only_stage_and_counts() {
        let event = ParseProgressEvent::new(
            "version-1",
            ParseProgress {
                stage: "pages".to_string(),
                done: 7,
                total: Some(40),
            },
        );
        let value = serde_json::to_value(&event).expect("olay JSON");
        assert_eq!(value["versionId"], "version-1");
        assert_eq!(value["stage"], "pages");
        assert_eq!(value["done"], 7);
        assert_eq!(value["total"], 40);
        // Sözleşme: dört alan; metin/yol/ad alanı yok.
        assert_eq!(value.as_object().expect("nesne").len(), 4);
        for forbidden in ["text", "fileName", "path", "quote"] {
            assert!(value.get(forbidden).is_none(), "{forbidden} yayımlanmamalı");
        }
        // Toplam bilinmiyorsa `null` (uydurulmuş sayı yok).
        let open = ParseProgressEvent::new(
            "version-1",
            ParseProgress {
                stage: "start".to_string(),
                done: 0,
                total: None,
            },
        );
        let value = serde_json::to_value(&open).expect("olay JSON");
        assert!(value["total"].is_null());
    }

    /// Arama kipi adı sunucuda çözülür: boş → `auto`, bilinmeyen reddedilir.
    /// İki komut (`search_segments_page`, `search_segments_total`) aynı kuralı
    /// kullanır; ayrışırsa sayfa ile sayı farklı kipe ait olurdu.
    #[test]
    fn search_mode_names_are_validated_and_empty_means_auto() {
        assert!(matches!(parse_search_mode(None), Ok(SearchMode::Auto)));
        assert!(matches!(
            parse_search_mode(Some("   ")),
            Ok(SearchMode::Auto)
        ));
        assert!(matches!(
            parse_search_mode(Some("phrase")),
            Ok(SearchMode::Phrase)
        ));
        assert!(matches!(
            parse_search_mode(Some("Akıllı")),
            Err(ApiError { .. })
        ));
        assert_eq!(
            parse_search_mode(Some("fuzzy")).unwrap_err().code,
            "E_SEARCH_QUERY"
        );
    }

    /// Ayrıştırma hatasının tekrar sınıfı: yalnız geçici altyapı hataları
    /// yeniden denenir. Belgeye özgü hatalar tekrar denenirse aynı belge için
    /// üç deneme harcanır ve kullanıcıya yanlış "geçici hata" izlenimi verilir.
    /// **Disk dolu ve kullanıcı iptali** de tekrar denenmez (§17.4).
    #[test]
    fn only_transport_failures_are_retried() {
        for code in [
            "E_WORKER_DISCONNECTED",
            "E_WORKER_TIMEOUT",
            "E_WORKER_FRAME",
            "E_WORKER_RESPONSE",
            "E_IO",
        ] {
            assert!(parse_failure_is_retryable(code), "{code} tekrar denenmeli");
        }
        for code in [
            "E_WORKER_MISSING",
            "E_PARSE",
            "E_PDF_PASSWORD",
            "E_UNSUPPORTED",
            "E_NOT_FOUND",
            "E_DISK_FULL",
            "E_WORKER_CANCELED",
        ] {
            assert!(
                !parse_failure_is_retryable(code),
                "{code} tekrar denenmemeli"
            );
        }
    }

    #[test]
    fn rollback_accepts_only_a_bare_backup_file_name() {
        assert_eq!(
            rollback_backup_name("vault.db.v12-pre-update.bak").expect("ad"),
            "vault.db.v12-pre-update.bak"
        );
        for bad in [
            "",
            "   ",
            " vault.db.v12-pre-update.bak",
            "../vault.db.v12-pre-update.bak",
            "/tmp/vault.db.v12-pre-update.bak",
            "..\\vault.db.v12-pre-update.bak",
            "alt/vault.db.v12-pre-update.bak",
        ] {
            assert_eq!(
                rollback_backup_name(bad).unwrap_err().code,
                "E_ROLLBACK_BACKUP",
                "reddedilmeli: {bad:?}"
            );
        }
    }

    #[test]
    fn rollback_needs_the_exact_confirmation() {
        assert!(rollback_confirmed("GERİ DÖN"));
        assert!(rollback_confirmed("  GERİ DÖN  "), "kırpma hoş görülür");
        for bad in ["", "GERI DON", "geri dön", "GERİ DÖN!", "onayla"] {
            assert!(!rollback_confirmed(bad), "reddedilmeli: {bad:?}");
        }
    }

    #[test]
    fn inventory_gate_accepts_only_docx_and_udf() {
        assert!(inventory_mime_supported(INVENTORY_MIMES[0]));
        assert!(inventory_mime_supported("application/udf"));
        for mime in [
            "application/pdf",
            "image/jpeg",
            "text/plain",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "",
        ] {
            assert!(!inventory_mime_supported(mime), "reddedilmeli: {mime:?}");
        }
    }

    #[test]
    fn package_report_explains_a_missing_package() {
        let view = read_worker_package(Path::new("/olmayan/worker/paketi")).expect("rapor");
        assert!(!view.found);
        assert!(view.manifest.is_none() && view.verification.is_none());
        assert!(
            view.note
                .as_deref()
                .unwrap_or_default()
                .contains("geliştirme"),
            "gerekçe yazılmalı: {:?}",
            view.note
        );
    }

    fn package_fixture() -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("dim-worker-pkg-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(dir.join("_internal")).expect("dizin");
        std::fs::write(dir.join("dim-worker"), b"ikili").expect("ikili");
        std::fs::write(dir.join("_internal").join("base_library.zip"), b"arsiv").expect("arsiv");
        dir
    }

    fn write_manifest(dir: &Path, tamper: bool) {
        let binary = if tamper { "baska" } else { "ikili" };
        let manifest = serde_json::json!({
            "manifestVersion": 1,
            "workerVersion": "0.1.0",
            "mode": "onedir",
            "buildPlatform": "test-arm64",
            "pythonVersion": "3.12.0",
            "pyinstallerVersion": "6.11.1",
            "entryPoint": "dim-worker",
            "totals": {"files": 2, "symlinks": 0, "bytes": 10, "truncated": false, "unsafeLinks": 0},
            "roles": {"bootloader": 1, "stdlib_archive": 1},
            "tesseract": {"bundled": false, "note": "sistemde kurulu olmalı"},
            "files": [
                {"path": "dim-worker", "role": "bootloader", "kind": "file",
                 "bytes": binary.len(), "sha256": hex::encode(Sha256::digest(binary.as_bytes()))},
                {"path": "_internal/base_library.zip", "role": "stdlib_archive", "kind": "file",
                 "bytes": 5, "sha256": hex::encode(Sha256::digest(b"arsiv"))}
            ]
        });
        std::fs::write(
            dir.join(MANIFEST_FILE),
            serde_json::to_string(&manifest).expect("json"),
        )
        .expect("manifest");
    }

    #[test]
    fn package_report_verifies_a_synthetic_package() {
        let dir = package_fixture();
        write_manifest(&dir, false);
        let view = read_worker_package(&dir).expect("rapor");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(view.found, "{:?}", view.note);
        let manifest = view.manifest.expect("manifest");
        assert_eq!(manifest.worker_version, "0.1.0");
        assert_eq!(manifest.totals.files, 2);
        assert_eq!(manifest.roles.get("bootloader"), Some(&1));
        let verification = view.verification.expect("doğrulama");
        assert!(verification.ok, "{verification:?}");
        assert_eq!(verification.verified, 2);
        assert!(verification.entry_point_present);
    }

    #[test]
    fn package_report_reports_tampering_instead_of_passing() {
        let dir = package_fixture();
        write_manifest(&dir, true);
        let view = read_worker_package(&dir).expect("rapor");
        let _ = std::fs::remove_dir_all(&dir);
        let verification = view.verification.expect("doğrulama");
        assert!(!verification.ok, "kurcalama geçmemeli");
        assert!(verification.problems.iter().any(|p| p.path == "dim-worker"
            && p.problem == dim_ingest::packaging::PackageProblemKind::HashChanged));
    }

    #[test]
    fn package_report_flags_a_package_without_manifest() {
        let dir = package_fixture();
        let view = read_worker_package(&dir).expect("rapor");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!view.found);
        assert!(
            view.note
                .as_deref()
                .unwrap_or_default()
                .contains("manifest"),
            "gerekçe yazılmalı: {:?}",
            view.note
        );
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

    #[test]
    fn case_payload_embeds_coverage_block() {
        use dim_storage::{AiPolicy, CaseKind, NewCase, Sensitivity};
        let db = Database::open_in_memory(&DatabaseKey::new([90u8; 32])).expect("db");
        let case = db
            .create_case(&NewCase {
                title: "Kapsam".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: Sensitivity::Normal,
                default_ai_policy: AiPolicy::Off,
            })
            .expect("case");
        let coverage = ExportCoverage::from_report(&db.coverage_report(&case.id).expect("rapor"));
        let payload = case_payload(&db, &case.id, &coverage).expect("yük");
        assert_eq!(
            payload["coverage"]["statement"],
            dim_storage::COVERAGE_STATEMENT
        );
        assert_eq!(payload["coverage"]["documents"], 0);
        assert_eq!(payload["coverage"]["complete"], false);
        assert_eq!(payload["coverage"]["textPages"], 0);
        // Rapor bölümü tablo başlığıyla başlar ve tüm sayımları taşır.
        let section = coverage_section(&coverage);
        assert_eq!(section["heading"], "İncelenen kapsam");
        assert_eq!(section["rows"][0][0], "Ölçü");
        assert_eq!(
            section["rows"].as_array().map(Vec::len),
            Some(coverage.rows().len() + 1)
        );
        assert_eq!(
            section["lines"].as_array().map(Vec::len),
            Some(coverage.warnings.len() + 1)
        );
        assert_eq!(
            section["lines"]
                .as_array()
                .and_then(|lines| lines.last())
                .cloned(),
            Some(serde_json::json!(dim_storage::COVERAGE_STATEMENT))
        );
        // CSV yorumları tablo başlığını kirletmez.
        assert!(coverage
            .csv_comment_block()
            .lines()
            .all(|l| l.starts_with('#')));
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

/// Belgenin sürümleri (en yeniden eskiye). F12-T13'te emsale tam metin
/// aktarılırken kaynak sürüm bu listeden **kullanıcı tarafından** seçilir;
/// "en yeni sürüm" varsayımı yapılmaz.
#[tauri::command]
pub fn document_version_list(
    state: State<'_, AppState>,
    document_id: String,
) -> Result<Vec<DocumentVersion>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_versions(&document_id)?)
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
/// F07-T12: kopmuş/eski bağlantı için güncel sürümde yeniden bağlama önerisi.
/// Eşleşme yoksa `unresolved`, birden fazlaysa `ambiguous` döner; öneri
/// kullanıcı onayı olmadan anchor'ı taşımaz.
#[tauri::command]
pub fn anchor_rebind_suggestion(
    state: State<'_, AppState>,
    anchor_id: String,
) -> Result<RebindSuggestion, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.rebind_suggestion(&anchor_id)?)
}

/// Anchor'ı güncel sürümdeki **doğrulanmış** konuma taşır. Alıntı verilen
/// offset'te birebir durmuyorsa bağ değişmez.
#[tauri::command]
pub fn anchor_rebind(
    state: State<'_, AppState>,
    anchor_id: String,
    segment_id: String,
    start_offset: i64,
) -> Result<AnchorRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.rebind_anchor(&anchor_id, &segment_id, start_offset)?)
}

#[tauri::command]
pub fn anchor_locate(
    state: State<'_, AppState>,
    anchor_id: String,
) -> Result<Option<AnchorLocation>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.locate_anchor(&anchor_id)?)
}

/// F07-T09: metin seçiminden kalıcı işaret. Alıntı segmentte birebir
/// doğrulanmazsa anchor da işaret de oluşmaz.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkSelectionResult {
    pub anchor: AnchorRow,
    pub mark: MarkRow,
}

/// F07-T09: metin seçiminden kaynaklı not. Notun kaynak listesi oluşan
/// anchor'dan gelir; çağıranın verdiği kimlikler yazılmaz.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSelectionResult {
    pub anchor: AnchorRow,
    pub note: NoteRow,
}

#[tauri::command]
pub fn mark_create_from_selection(
    state: State<'_, AppState>,
    input: NewAnchor,
    label: String,
) -> Result<MarkSelectionResult, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let (anchor, mark) = db.mark_from_selection(&input, &label)?;
    Ok(MarkSelectionResult { anchor, mark })
}

#[tauri::command]
pub fn note_create_from_selection(
    state: State<'_, AppState>,
    input: NewAnchor,
    note: NewNote,
) -> Result<NoteSelectionResult, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let (anchor, note) = db.note_from_selection(&input, &note)?;
    Ok(NoteSelectionResult { anchor, note })
}

/// Sayfa bölgesinden işaret (görsel seçim). Dikdörtgenler sayfa uzayındadır.
#[tauri::command]
pub fn mark_create_region(
    state: State<'_, AppState>,
    input: NewRegionMark,
) -> Result<MarkRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.create_region_mark(&input)?)
}

#[tauri::command]
pub fn marks_for_version(
    state: State<'_, AppState>,
    version_id: String,
) -> Result<Vec<MarkRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.marks_for_version(&version_id)?)
}

#[tauri::command]
pub fn mark_delete(state: State<'_, AppState>, mark_id: String) -> Result<(), ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    db.delete_mark(&mark_id)?;
    Ok(())
}

#[tauri::command]
pub fn task_confirm_date(state: State<'_, AppState>, id: String) -> Result<TaskRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.confirm_task_date(&id)?)
}

#[tauri::command]
pub fn task_reminders(
    state: State<'_, AppState>,
    case_id: String,
    today: String,
) -> Result<Vec<ReminderRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.due_task_reminders(&case_id, &today)?)
}

#[tauri::command]
pub fn task_reminders_enqueue(
    state: State<'_, AppState>,
    case_id: String,
    today: String,
) -> Result<ReminderEnqueueResult, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.enqueue_task_reminders(&case_id, &today)?)
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

/// Önizleme PNG üst sınırı: diskte ve bellekte tek kayıt için.
const PREVIEW_MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagePreviewView {
    pub version_id: String,
    pub page_index: u32,
    pub dpi: u32,
    pub png_base64: String,
    pub byte_size: usize,
    /// `true` ise görüntü kasadaki şifreli önbellekten geldi.
    pub cached: bool,
}

/// Sayfa görüntüsü önizlemesi (F07-T11): fiziksel sayfa, PDF.
///
/// Sıra: bellek önbelleği → kasadaki şifreli önbellek → worker'da yeniden
/// üretim. Üretilen görüntü kasa anahtarıyla şifrelenip diske yazılır; düz PNG
/// kasa dışında bırakılmaz. Kasa kilitliyse komut çalışmaz.
#[tauri::command]
pub fn document_page_preview(
    state: State<'_, AppState>,
    version_id: String,
    page_index: u32,
    dpi: Option<u32>,
) -> Result<PagePreviewView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault = guard.unlocked.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault_path = guard.path.as_ref().ok_or_else(ApiError::vault_locked)?;
    let dpi = dpi.unwrap_or(96).clamp(48, 200);
    let key = PreviewKey::new(version_id.clone(), page_index, dpi);
    let store = PreviewStore::new(
        vault_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("preview-cache"),
    );

    let view = |bytes: &[u8], cached: bool| PagePreviewView {
        version_id: version_id.clone(),
        page_index,
        dpi,
        png_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        byte_size: bytes.len(),
        cached,
    };

    let mut memory = state.preview_lock();
    if let Some(bytes) = memory.get(&key) {
        return Ok(view(&bytes, true));
    }
    match store.load(vault, &key) {
        Ok(Some(bytes)) => {
            memory.put(&key, bytes.clone());
            return Ok(view(&bytes, true));
        }
        Ok(None) => {}
        // Bozuk/anahtarı uyuşmayan kayıt sessizce "yok" sayılmaz: atılır ve
        // görüntü yeniden üretilir; önbellek doğruluk kaynağı değildir.
        Err(_) => {
            let _ = store.purge_version(&version_id);
        }
    }
    drop(memory);

    let blob = db
        .blob_for_version(&version_id)?
        .ok_or_else(|| ApiError::new("E_NOT_FOUND", "Belge içeriği bulunamadı."))?;
    if blob.mime != "application/pdf" {
        return Err(ApiError::new(
            "E_PREVIEW_UNSUPPORTED",
            "Sayfa görüntüsü yalnız PDF için üretilir; DOCX/UDF yapısal görünüm kullanır.",
        ));
    }
    let blob_path = vault_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("blobs")
        .join(format!("{}.blb", blob.storage_id));
    let data = vault.read_blob(&blob_path, blob.aad_ref.as_bytes())?;

    let mut worker = spawn_worker()?;
    let rendered = worker.render_page(&data, page_index, dpi);
    worker.shutdown();
    let rendered = rendered?;
    if rendered.png.len() > PREVIEW_MAX_BYTES {
        return Err(ApiError::new(
            "E_PREVIEW_TOO_LARGE",
            "Önizleme 8 MiB sınırını aşıyor; daha düşük çözünürlük deneyin.",
        ));
    }
    // Önbellek yazımı başarısız olsa bile görüntü döner; kalıcı kayıt zorunlu
    // değildir, ama başarı iddiası yalnız gerçekten yazıldıysa "cached" olur.
    let _ = store.store(vault, &key, &rendered.png);
    state.preview_lock().put(&key, rendered.png.clone());
    Ok(view(&rendered.png, false))
}

/// Envanteri çıkarılabilen MIME'lar. PDF/görüntü/düz metin **kapsam dışıdır**:
/// onlarda "yapı/ek/değişiklik" sayımı yoktur, uydurulmaz.
const INVENTORY_MIMES: [&str; 3] = [
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/udf",
    "application/x-udf",
];

fn inventory_mime_supported(mime: &str) -> bool {
    INVENTORY_MIMES.contains(&mime)
}

/// Belge envanteri: yapı, ekler ve değişiklik izi (DOCX/UDF).
///
/// Baytlar kasadan çözülür ve worker'a gönderilir; **kasaya hiçbir şey
/// yazılmaz** (envanter istek üzerine üretilir, ayrıştırma koşusu açmaz).
/// Denetime yalnız sayılar yazılır: ek adı, metin ve dosya yolu girmez.
#[tauri::command]
pub fn document_inventory(
    state: State<'_, AppState>,
    version_id: String,
) -> Result<DocumentInventory, ApiError> {
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
    if !inventory_mime_supported(&blob.mime) {
        return Err(ApiError::new(
            "E_INVENTORY_UNSUPPORTED",
            "Belge envanteri yalnız DOCX/UDF paketleri için çıkarılır; PDF ve görüntülerde yapı sayımı yoktur.",
        ));
    }
    let blob_path = vault_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("blobs")
        .join(format!("{}.blb", blob.storage_id));
    let data = vault.read_blob(&blob_path, blob.aad_ref.as_bytes())?;

    let mut worker = spawn_worker()?;
    let extracted = worker.inventory(&data, &blob.mime);
    worker.shutdown();
    let inventory = extracted?;

    let _ = db.append_audit(
        "document_inventory",
        Some(&version.case_id),
        "extract",
        "local-user",
        &inventory.audit_counts(),
    );
    Ok(inventory)
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

/// Notun güncel hâli. Sürüm çakışmasında arayüz farkı göstermek için buradan
/// okur; çakışma mesajındaki sürüm numarasına güvenip içerik uydurulmaz.
#[tauri::command]
pub fn note_get(state: State<'_, AppState>, id: String) -> Result<Option<NoteRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.get_note(&id)?)
}

#[tauri::command]
pub fn note_update(
    state: State<'_, AppState>,
    id: String,
    expected_row_version: i64,
    input: NoteEdit,
) -> Result<NoteRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.update_note(&id, expected_row_version, &input)?)
}

#[tauri::command]
pub fn note_undo(
    state: State<'_, AppState>,
    id: String,
    expected_row_version: i64,
) -> Result<NoteRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.undo_note(&id, expected_row_version)?)
}

#[tauri::command]
pub fn note_redo(
    state: State<'_, AppState>,
    id: String,
    expected_row_version: i64,
) -> Result<NoteRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.redo_note(&id, expected_row_version)?)
}

#[tauri::command]
pub fn note_revisions(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<NoteRevisionRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_note_revisions(&id)?)
}

#[tauri::command]
pub fn note_draft_save(
    state: State<'_, AppState>,
    case_id: String,
    note_id: Option<String>,
    input: NoteDraftInput,
) -> Result<NoteDraftRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.save_note_draft(&case_id, note_id.as_deref(), &input)?)
}

#[tauri::command]
pub fn note_draft_get(
    state: State<'_, AppState>,
    case_id: String,
    note_id: Option<String>,
) -> Result<Option<NoteDraftRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.get_note_draft(&case_id, note_id.as_deref())?)
}

#[tauri::command]
pub fn note_draft_clear(
    state: State<'_, AppState>,
    case_id: String,
    note_id: Option<String>,
) -> Result<bool, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.clear_note_draft(&case_id, note_id.as_deref())?)
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

/// Kullanıcı düzenlemesi. `expectedRowVersion` arayüzün okuduğu sürümdür;
/// bu arada başka yazma olduysa `E_ROW_VERSION` döner, kayıt ezilmez (F08-T09).
#[tauri::command]
pub fn event_update(
    state: State<'_, AppState>,
    id: String,
    expected_row_version: i64,
    input: EventPatch,
) -> Result<EventRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.update_event(&id, expected_row_version, &input)?)
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

/// `search_segments_page` girdisi. Alanlar tek tek adlandırılmıştır; kip
/// filtreden ayrıdır (F07-T02) ve IPC yükü açık kalır.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPageInput {
    pub case_id: String,
    pub query: String,
    #[serde(default)]
    pub filters: Option<SearchFilters>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub include_total: Option<bool>,
    /// `auto` | `exact` | `flexible` | `phrase` | `docket`.
    #[serde(default)]
    pub mode: Option<String>,
}

/// `mode` alanını çözer: boş/verilmemiş → `auto`; bilinmeyen ad
/// `E_SEARCH_QUERY` ile reddedilir (F07-T02).
fn parse_search_mode(mode: Option<&str>) -> Result<SearchMode, ApiError> {
    match mode.map(str::trim).filter(|raw| !raw.is_empty()) {
        None => Ok(SearchMode::Auto),
        Some(raw) => SearchMode::parse(raw).ok_or_else(|| {
            ApiError::new("E_SEARCH_QUERY", &format!("bilinmeyen arama kipi: {raw}"))
        }),
    }
}

/// Filtreli ve keyset sayfalı arama (F07-T03, F07-T05).
///
/// `filters` verilmezse filtre uygulanmaz. `includeTotal` yalnız ilk sayfada
/// true olmalı; sonraki sayfalarda tam sayım gereksiz maliyettir.
/// `mode` (F07-T02) sunucuda çözülür; bilinmeyen kip `E_SEARCH_QUERY` döner.
#[tauri::command]
pub fn search_segments_page(
    state: State<'_, AppState>,
    input: SearchPageInput,
) -> Result<SearchPage, ApiError> {
    let mode = parse_search_mode(input.mode.as_deref())?;
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.search_page(
        &input.case_id,
        &SearchQuery {
            text: input.query,
            mode,
        },
        &input.filters.unwrap_or_default(),
        input.limit.unwrap_or(50),
        input.cursor.as_deref(),
        input.include_total.unwrap_or(false),
    )?)
}

/// **Ertelenmiş toplam sayım** (F15). Arayüz ilk sayfayı gösterdikten sonra
/// çağırır; böylece sayfa `includeTotal=true` maliyetini beklemez.
///
/// `cursor`/`limit` yok sayılır: sayı sayfaya değil **sorgunun tamamına**
/// aittir. Sayı `search_page(include_total=true)` ile birebir aynı kuraldan
/// gelir (`Database::search_total`).
#[tauri::command]
pub fn search_segments_total(
    state: State<'_, AppState>,
    input: SearchPageInput,
) -> Result<SearchTotal, ApiError> {
    let mode = parse_search_mode(input.mode.as_deref())?;
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.search_total(
        &input.case_id,
        &SearchQuery {
            text: input.query,
            mode,
        },
        &input.filters.unwrap_or_default(),
    )?)
}

#[tauri::command]
pub fn document_tags(
    state: State<'_, AppState>,
    document_id: String,
) -> Result<Vec<String>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_document_tags(&document_id)?)
}

#[tauri::command]
pub fn document_tags_for_case(
    state: State<'_, AppState>,
    case_id: String,
) -> Result<Vec<DocumentTagRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_case_document_tags(&case_id)?)
}

#[tauri::command]
pub fn case_tags(state: State<'_, AppState>, case_id: String) -> Result<Vec<CaseTagRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_case_tags(&case_id)?)
}

/// Etiket ekler ve belgenin güncel etiket listesini döner.
#[tauri::command]
pub fn document_tag_add(
    state: State<'_, AppState>,
    document_id: String,
    tag: String,
) -> Result<Vec<String>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    db.add_document_tag(&document_id, &tag)?;
    Ok(db.list_document_tags(&document_id)?)
}

#[tauri::command]
pub fn document_tag_remove(
    state: State<'_, AppState>,
    document_id: String,
    tag: String,
) -> Result<Vec<String>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    db.remove_document_tag(&document_id, &tag)?;
    Ok(db.list_document_tags(&document_id)?)
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
    [
        dir.join("worker").join(name),
        dir.join("..").join("Resources").join("worker").join(name),
        // Tauri `resources/worker/**` ifadesini macOS'ta
        // Contents/Resources/resources/worker altına kopyalar; Windows'ta
        // aynı yapı resources\\resources\\worker olur.
        dir.join("..")
            .join("Resources")
            .join("resources")
            .join("worker")
            .join(name),
        dir.join("resources").join("worker").join(name),
        dir.join("resources")
            .join("resources")
            .join("worker")
            .join(name),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

/// Paketlenmiş worker'ın manifest özeti. Dosya listesi arayüze taşınmaz
/// (yüzlerce satır); yalnız sayılar, roller ve sürüm künyesi.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerPackageManifestView {
    pub worker_version: String,
    pub mode: String,
    pub build_platform: String,
    pub python_version: String,
    pub pyinstaller_version: String,
    pub entry_point: String,
    pub totals: ManifestTotals,
    pub roles: std::collections::BTreeMap<String, u64>,
    pub tesseract: TesseractStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerPackageView {
    /// Paket bulundu ve doğrulama koştu mu. `false` bir hata değil: geliştirme
    /// kipinde paket yoktur, uygulama kaynak ağacından çalışır.
    pub found: bool,
    pub package_dir: Option<String>,
    pub note: Option<String>,
    pub manifest: Option<WorkerPackageManifestView>,
    pub verification: Option<PackageVerification>,
    /// Tesseract ikilisi pakete gömülmedi (bilinçli); sistemde PATH'te mi?
    pub tesseract_on_path: bool,
}

/// PATH üzerinde `tesseract` ikilisini arar. Süreç başlatılmaz, dosya
/// çalıştırılmaz — yalnız varlık. Sürüm/dil verisi bu komutun kapsamı dışında.
fn tesseract_on_path() -> bool {
    let name = if cfg!(windows) {
        "tesseract.exe"
    } else {
        "tesseract"
    };
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}

fn manifest_view(manifest: &WorkerManifest) -> WorkerPackageManifestView {
    WorkerPackageManifestView {
        worker_version: manifest.worker_version.clone(),
        mode: manifest.mode.clone(),
        build_platform: manifest.build_platform.clone(),
        python_version: manifest.python_version.clone(),
        pyinstaller_version: manifest.pyinstaller_version.clone(),
        entry_point: manifest.entry_point.clone(),
        totals: manifest.totals.clone(),
        roles: manifest.roles.clone(),
        tesseract: manifest.tesseract.clone(),
    }
}

/// Paket dizinini manifeste karşı doğrular. Dizin/manifest yokluğu **hata
/// değil**, `found=false` + gerekçe; bozuk manifest ise `E_WORKER_PACKAGE`.
fn read_worker_package(dir: &Path) -> Result<WorkerPackageView, ApiError> {
    let entry_point = dir.join("dim-worker");
    if !dir.is_dir() || !entry_point.is_file() {
        return Ok(WorkerPackageView {
            found: false,
            package_dir: None,
            note: Some(
                "Paketlenmiş worker bulunamadı: uygulama geliştirme kipinde kaynak ağacından çalışıyor."
                    .to_string(),
            ),
            manifest: None,
            verification: None,
            tesseract_on_path: tesseract_on_path(),
        });
    }
    if !dir.join(MANIFEST_FILE).is_file() {
        return Ok(WorkerPackageView {
            found: false,
            package_dir: Some(dir.to_string_lossy().to_string()),
            note: Some(
                "Paket var ama manifesti yok: bütünlük doğrulanamaz. `scripts/build-worker.sh` ile yeniden paketleyin."
                    .to_string(),
            ),
            manifest: None,
            verification: None,
            tesseract_on_path: tesseract_on_path(),
        });
    }
    let manifest = read_manifest(dir).map_err(|error| {
        ApiError::new(
            error.code(),
            &format!("Worker manifesti okunamadı: {error}"),
        )
    })?;
    let verification = verify_package(dir, &manifest).map_err(|error| {
        ApiError::new(
            error.code(),
            &format!("Worker paketi doğrulanamadı: {error}"),
        )
    })?;
    Ok(WorkerPackageView {
        found: true,
        package_dir: Some(dir.to_string_lossy().to_string()),
        note: None,
        manifest: Some(manifest_view(&manifest)),
        verification: Some(verification),
        tesseract_on_path: tesseract_on_path(),
    })
}

/// Paketlenmiş worker'ın bütünlüğünü bildirir (yazmaz). Ağır iş (sha256)
/// `spawn_blocking` içinde; kasa kilidi tutulmaz.
#[tauri::command]
pub async fn worker_package_report(
    state: State<'_, AppState>,
) -> Result<WorkerPackageView, ApiError> {
    let dir = bundled_worker_exe().and_then(|exe| exe.parent().map(Path::to_path_buf));
    let joined = tauri::async_runtime::spawn_blocking(move || -> Result<WorkerPackageView, (String, String)> {
        match dir {
            Some(dir) => read_worker_package(&dir).map_err(|err| (err.code.to_string(), err.message.clone())),
            None => Ok(WorkerPackageView {
                found: false,
                package_dir: None,
                note: Some(
                    "Paketlenmiş worker bulunamadı: uygulama geliştirme kipinde kaynak ağacından çalışıyor."
                        .to_string(),
                ),
                manifest: None,
                verification: None,
                tesseract_on_path: tesseract_on_path(),
            }),
        }
    })
    .await
    .map_err(|_| ApiError::new("E_WORKER_PACKAGE", "Paket doğrulaması tamamlanamadı."))?;
    let view = joined.map_err(|(code, message)| ApiError::new(&code, &message))?;

    // Denetime yalnız sayı/durum yazılır: dosya adları ve tam yol yazılmaz.
    {
        let guard = state.lock();
        if let Some(db) = guard.db.as_ref() {
            let metadata = serde_json::json!({
                "found": view.found,
                "manifestFiles": view.manifest.as_ref().map(|m| m.totals.files),
                "manifestBytes": view.manifest.as_ref().map(|m| m.totals.bytes),
                "verified": view.verification.as_ref().map(|v| v.verified),
                "problems": view.verification.as_ref().map(|v| v.problem_count),
                "extra": view.verification.as_ref().map(|v| v.extra_count),
                "ok": view.verification.as_ref().map(|v| v.ok),
                "tesseractOnPath": view.tesseract_on_path,
            });
            let _ = db.append_audit("worker_package", None, "verify", "local-user", &metadata)?;
        }
    }
    Ok(view)
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

/// Arayüze yayılan ayrıştırma ilerlemesi (F04-T07).
///
/// Olay **yalnız** sürüm kimliği, aşama ve sayaç taşır: belge metni, dosya
/// adı, yol veya kasa bilgisi yayımlanmaz (denetim kaydıyla aynı sınır).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseProgressEvent {
    pub version_id: String,
    pub stage: String,
    pub done: u32,
    pub total: Option<u32>,
}

impl ParseProgressEvent {
    fn new(version_id: &str, progress: ParseProgress) -> Self {
        Self {
            version_id: version_id.to_string(),
            stage: progress.stage,
            done: progress.done,
            total: progress.total,
        }
    }
}

/// Belge sürümünü yerel worker'da ayrıştırır. Özgün blob çözülür, metin
/// çıkarılır, sürümlü `parse_run` + sayfa + segment kayıtları yazılır.
///
/// **Arayüz iş parçacığı beklemez (F04-T07):** ağır iş `spawn_blocking`
/// içinde koşar; kasa kilidi orada tutulur. Süren worker yuvaya kaydedilir,
/// `job_cancel` kasa kilidini beklemeden süreci öldürebilir.
///
/// **Canlı aşama/sayım:** worker'ın ilerleme çerçeveleri `parse-progress`
/// olayına çevrilir (`start` → `open` → `pages`). Olay kaybolursa ayrıştırma
/// etkilenmez; sayaç göstergesi bilgilendiricidir, sonuç değişmez.
#[tauri::command]
pub async fn document_parse(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    version_id: String,
    ocr: Option<bool>,
    ocr_lang: Option<String>,
    text_encoding: Option<String>,
    password: Option<String>,
) -> Result<ParseResultView, ApiError> {
    let hub = state.vault_handle();
    let slot = state.parse_handle();
    tauri::async_runtime::spawn_blocking(move || {
        let mut emit = |progress: ParseProgress| {
            let event = ParseProgressEvent::new(&version_id, progress);
            // Pencere kapalıysa olay düşer; ayrıştırma sürer.
            let _ = app.emit("parse-progress", event);
        };
        let guard = lock_vault(&hub);
        parse_version_locked(
            &guard,
            &slot,
            &version_id,
            ocr,
            ocr_lang,
            text_encoding,
            password,
            &mut emit,
        )
    })
    .await
    .map_err(|_| {
        ApiError::new(
            "E_PARSE",
            "Ayrıştırma işi beklenmedik biçimde sona erdi; kasa durumu değişmedi.",
        )
    })?
}

/// `document_parse` gövdesi. Kasa kilidi **çağıranda** tutulur; bu işlev
/// bloklayan iş parçacığında çalışır. `progress` arayüz olayına köprü kurar.
#[allow(clippy::too_many_arguments)]
fn parse_version_locked(
    guard: &crate::state::VaultState,
    slot: &Arc<Mutex<Option<RunningParse>>>,
    version_id: &str,
    ocr: Option<bool>,
    ocr_lang: Option<String>,
    text_encoding: Option<String>,
    password: Option<String>,
    progress: &mut dyn FnMut(ParseProgress),
) -> Result<ParseResultView, ApiError> {
    // İlk olay kasadan okuma başlamadan gider: büyük blob çözülürken arayüz
    // "hazırlanıyor" gösterir, donmuş görünmez.
    progress(ParseProgress {
        stage: "start".to_string(),
        done: 0,
        total: None,
    });
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault = guard.unlocked.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault_path = guard.path.as_ref().ok_or_else(ApiError::vault_locked)?;

    let version = db
        .get_version(version_id)?
        .ok_or_else(|| ApiError::new("E_NOT_FOUND", "Belge sürümü bulunamadı."))?;
    // Kuyruk: içe aktarma `parse:<sürüm>` işini açmıştı. İş burada sahiplenilir;
    // ayrıştırma düşerse iş `failed`/`retry_wait` olur ve "yürüyor" diye asılı
    // kalmaz (F03-T07/T08). İş yoksa (eski kasa) ayrıştırma yine çalışır.
    let job_owner = format!("parse-{}", std::process::id());
    let job_key = format!("parse:{}", version.id);
    let job = db
        .claim_job_by_key(
            &job_key,
            &job_owner,
            dim_storage::DEFAULT_LEASE_SECONDS,
            time::OffsetDateTime::now_utc(),
        )
        .ok()
        .flatten();
    // Ayrıştırma koşusunun durumu (`succeeded`/`partial`) kuyruğa da yansır:
    // uyarılı çıktı `completed` değil `partial` yazılır.
    let mut parse_status: Option<String> = None;
    let outcome = (|| -> Result<ParseResultView, ApiError> {
        let blob = db
            .blob_for_version(version_id)?
            .ok_or_else(|| ApiError::new("E_NOT_FOUND", "Belge içeriği bulunamadı."))?;
        let blob_path = vault_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("blobs")
            .join(format!("{}.blb", blob.storage_id));
        let data = vault.read_blob(&blob_path, blob.aad_ref.as_bytes())?;

        let mut worker = spawn_worker()?;
        // İptal tutamağı yuvaya yazılır: `job_cancel` kasa kilidini
        // beklemeden bu süreci öldürür (F04-T07). Yuva düşünce temizlenir.
        let killer = worker.killer();
        let _slot = ParseSlotGuard::register(
            Arc::clone(slot),
            RunningParse {
                version_id: version_id.to_string(),
                job_id: job.as_ref().map(|job| job.id.clone()),
                killer: killer.clone(),
            },
        );
        let caps = worker
            .capabilities()
            .unwrap_or_else(|_| serde_json::json!({}));
        let ocr_enabled = ocr.unwrap_or(false);
        let lang = ocr_lang.unwrap_or_else(|| "eng".to_string());
        // Kodlama yalnız düz metinde anlamlıdır; PDF/DOCX/UDF'te yok sayılır.
        let is_text = blob.mime.starts_with("text/");
        let encoding = if is_text {
            text_encoding
                .map(|value| value.trim().to_lowercase())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "auto".to_string())
        } else {
            "auto".to_string()
        };
        let extraction = worker.extract_text_with_progress(
            &data,
            &blob.mime,
            ocr_enabled,
            &lang,
            is_text.then_some(encoding.as_str()),
            password.as_deref(),
            progress,
        )?;
        worker.shutdown();
        // İptal, çıkarımdan sonra ama yazmadan önce geldiyse: hiçbir çıktı
        // yazılmaz, koşu `cancelled` kapanır (şartname §17.2 canceled:
        // "tamamlanmamış çıktı uygulanmadı").
        if killer.is_canceled() {
            return Err(canceled_parse_error());
        }

        // Ayar özeti gerçek girdileri taşır: MIME, OCR bayrağı, dil, motor sürümü
        // ve kurulu tessdata parmak izi. Yalnız MIME yazmak yanıltıcıydı.
        let tessdata = caps
            .pointer("/ocr/tessdataHash")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        let encoding_note = if is_text {
            format!("|txt-encoding={encoding}")
        } else {
            String::new()
        };
        let settings = format!(
            "{}|ocr={}|lang={}|engine={}|tessdata={tessdata}{encoding_note}",
            blob.mime, ocr_enabled, lang, extraction.engine_version
        );
        // `start_parse_run` ayar özetini kendisi hash'ler (engine|sürüm|ayarlar);
        // burada ön-hash vermek çift hash üretir ve kaydı okunamaz kılar.
        let run = db.start_parse_run(
            version_id,
            &version.case_id,
            &extraction.engine,
            &extraction.engine_version,
            &settings,
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
                page_label: p.page_label.clone(),
                blocks: p
                    .blocks
                    .iter()
                    .map(|b| SegmentBlock {
                        start: i64::from(b.start),
                        end: i64::from(b.end),
                        bbox: b.bbox,
                        lines: b.lines.clone(),
                        path: b.path.clone(),
                        kind: b.kind.clone(),
                    })
                    .collect(),
            })
            .collect();
        // Koşu açıldıktan sonra iptal geldiyse sayfa **yazılmaz**; koşu
        // `cancelled` kapanır ki yarım çıktı "tamam" görünmesin.
        if killer.is_canceled() {
            let _ = db.finish_parse_run(&run.id, "cancelled", &[]);
            return Err(canceled_parse_error());
        }
        // Yazma aşaması ayrı ve uzun olabilir: okuma bitti, sayfalar kasaya
        // yazılıyor. Sayaç **gerçek** durumu söyler (0 → N); ara değer
        // uydurulmaz, çünkü tek çağrıda yazılıyor.
        progress(ParseProgress {
            stage: "store".to_string(),
            done: 0,
            total: Some(u32::try_from(pages.len()).unwrap_or(u32::MAX)),
        });
        let stored = db.store_pages(&run.id, &version.case_id, &pages)?;
        progress(ParseProgress {
            stage: "store".to_string(),
            done: u32::try_from(pages.len()).unwrap_or(u32::MAX),
            total: Some(u32::try_from(pages.len()).unwrap_or(u32::MAX)),
        });
        let status = if extraction.warnings.is_empty() {
            "succeeded"
        } else {
            "partial"
        };
        db.finish_parse_run(&run.id, status, &extraction.warnings)?;
        parse_status = Some(status.to_string());

        Ok(ParseResultView {
            parse_run: db.get_parse_run(&run.id)?.unwrap_or(run),
            pages: pages.len(),
            stored_segments: stored,
            warnings: extraction.warnings,
        })
    })();

    // Kuyruk sonucu ayrıştırma sonucundan ayrı yazılır: iş kaydı düşse bile
    // kullanıcının belgesi/parse koşusu yerinde kalır (kuyruk ikincil kayıttır).
    // Şartname §17.2 yolu: `running → validating → completed|partial`.
    match (job.as_ref(), &outcome) {
        (Some(job), Ok(_)) => {
            let now = time::OffsetDateTime::now_utc();
            let _ = db.begin_validation(&job.id, &job_owner, now);
            if parse_status.as_deref() == Some("partial") {
                let _ = db.complete_job_partial(&job.id, &job_owner, "E_PARSE_PARTIAL", now);
            } else {
                let _ = db.complete_job(&job.id, &job_owner, now);
            }
        }
        (Some(job), Err(err)) => {
            // Kullanıcı iptali: `running|validating → canceling → canceled`.
            // `fail_job` iptal isteği varsa sonucu `canceled` yazar; iptal
            // otomatik tekrar döngüsüne girmez (§17.4).
            if err.code == "E_WORKER_CANCELED" {
                let now = time::OffsetDateTime::now_utc();
                let _ = db.cancel_job(&job.id, now);
                let _ = db.fail_job(&job.id, &job_owner, "E_WORKER_CANCELED", false, now);
            } else if err.code == "E_PDF_PASSWORD" {
                let _ = db.block_job(
                    &job.id,
                    &job_owner,
                    "password",
                    time::OffsetDateTime::now_utc(),
                );
            } else {
                let _ = db.fail_job(
                    &job.id,
                    &job_owner,
                    &err.code,
                    parse_failure_is_retryable(&err.code),
                    time::OffsetDateTime::now_utc(),
                );
            }
        }
        (None, _) => {}
    }
    outcome
}

/// Ayrıştırma hatası yeniden denenebilir mi? Yalnız **geçici altyapı**
/// hataları (worker koptu, G/Ç) tekrar denenir; belgeye özgü hatalar (parola,
/// bozuk/desteklenmeyen dosya), **disk yetersizliği** ve kullanıcı iptali
/// otomatik tekrar döngüsüne girmez — şartname §17.4.
///
/// `E_DISK_FULL` listeden **çıkarıldı**: yer açılmadan tekrar denemek üç
/// denemeyi harcar ve kullanıcıya yanlış "yeniden deniyorum" izlenimi verir.
fn parse_failure_is_retryable(code: &str) -> bool {
    matches!(
        code,
        "E_WORKER_DISCONNECTED"
            | "E_WORKER_TIMEOUT"
            | "E_WORKER_FRAME"
            | "E_WORKER_RESPONSE"
            | "E_IO"
    )
}

/// Kullanıcı iptali: yarım çıktı yazılmadı, otomatik tekrar yok (§17.4).
fn canceled_parse_error() -> ApiError {
    ApiError::new(
        "E_WORKER_CANCELED",
        "Ayrıştırma iptal edildi; yarım çıktı kasaya yazılmadı. Belge yeniden ayrıştırılabilir.",
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobQueueView {
    pub summary: JobQueueSummary,
    pub jobs: Vec<JobRow>,
}

/// İptal sonucu: kuyruk kararı + **sürecin gerçekten durup durmadığı**.
///
/// `workerStopped=false` "süreç durmadı" demek değildir; yürüyen ayrıştırma
/// yoksa (iş sırada bekliyorsa) durdurulacak süreç de yoktur. Kullanıcıya
/// yalnız gerçekleşen sonuç yazılır (§17.5).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobCancelView {
    pub outcome: String,
    pub job: JobRow,
    pub worker_stopped: bool,
}

#[tauri::command]
pub fn job_queue_status(state: State<'_, AppState>) -> Result<JobQueueView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(JobQueueView {
        summary: db.job_queue_summary()?,
        jobs: db.list_jobs(20)?,
    })
}

#[tauri::command]
pub fn job_queue_recover(state: State<'_, AppState>) -> Result<JobRecovery, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let report = db.recover_stale_jobs(time::OffsetDateTime::now_utc())?;
    let metadata = serde_json::json!({
        "interrupted": report.interrupted,
        "requeued": report.requeued,
        "failed": report.failed,
        "canceled": report.canceled,
    });
    db.append_audit("job_queue", None, "recover", "local-user", &metadata)?;
    Ok(report)
}

/// Kuyruktaki bir işi iptal eder. Sırada bekleyen hemen `canceled` olur;
/// yürüyen iş **önce** gerçekten durdurulur, sonra `canceling` işaretine geçer
/// (şartname §17.5: süreç durmadan "iptal tamam" denmez).
///
/// Süreç durdurma kasa kilidini **beklemez**: ayrıştırma kilidi tutuyorsa
/// `state.parse` yuvasındaki tutamaç kullanılır, kuyruk kaydı sonra yazılır.
#[tauri::command]
pub fn job_cancel(state: State<'_, AppState>, job_id: String) -> Result<JobCancelView, ApiError> {
    // 1) Süreç: yuva kilidi kısa tutulur, kasa kilidi alınmaz.
    let killer = {
        let slot = state.parse_lock();
        match slot.as_ref() {
            Some(run) if run.job_id.as_deref() == Some(job_id.as_str()) => Some(run.killer.clone()),
            _ => None,
        }
    };
    let worker_stopped = killer.map(|killer| killer.cancel()).unwrap_or(false);

    // 2) Kuyruk kaydı (`running → canceling`; hiç çalışmamış iş `canceled`).
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let result = db.cancel_job(&job_id, time::OffsetDateTime::now_utc())?;
    db.append_audit(
        "job_queue",
        None,
        "cancel",
        "local-user",
        &serde_json::json!({
            "outcome": result.outcome,
            "state": result.job.state,
            "workerStopped": worker_stopped,
        }),
    )?;
    Ok(JobCancelView {
        outcome: result.outcome,
        job: result.job,
        worker_stopped,
    })
}

/// Başarısız işi kullanıcı isteğiyle yeniden sıraya alır (`failed → queued`).
#[tauri::command]
pub fn job_retry(state: State<'_, AppState>, job_id: String) -> Result<JobRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let row = db.retry_failed_job(&job_id, time::OffsetDateTime::now_utc())?;
    db.append_audit(
        "job_queue",
        None,
        "retry",
        "local-user",
        &serde_json::json!({ "state": row.state }),
    )?;
    Ok(row)
}

/// Engellenen işin engelini kaldırır (`blocked → queued`).
#[tauri::command]
pub fn job_unblock(state: State<'_, AppState>, job_id: String) -> Result<JobRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let row = db.unblock_job(&job_id, time::OffsetDateTime::now_utc())?;
    db.append_audit(
        "job_queue",
        None,
        "unblock",
        "local-user",
        &serde_json::json!({ "state": row.state }),
    )?;
    Ok(row)
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
///
/// Arşiv **geçici ada** yazılır ve bitince yerleştirilir: yarıda kalan bir
/// `.dimbak` dosyası geçerli arşiv sanılmasın (F15-T06).
fn zip_dir(src: &Path, dest: &Path) -> Result<(), ApiError> {
    use std::io::Write as _;

    dim_storage::atomic::write_atomic_with(dest, |file| {
        let io_err = |err: zip::result::ZipError| std::io::Error::other(err.to_string());
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
                writer.start_file(name, options).map_err(io_err)?;
                writer.write_all(&std::fs::read(&path)?)?;
            }
        }
        writer.finish().map(|_| ()).map_err(io_err)
    })
    .map_err(ApiError::from)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreView {
    pub vault_path: String,
    pub file_count: usize,
    /// Kopyalamadan **önce** doğrulanan dosya sayısı ve bayt (manifest sha256).
    pub verified_files: usize,
    pub verified_bytes: u64,
    pub verify_ms: u64,
    /// Yedekte olup manifestte sayılmayan dosyalar (kasaya kopyalanmadı).
    pub unlisted_blobs: Vec<String>,
}

fn sha256_file(path: &Path) -> Result<String, ApiError> {
    let data = std::fs::read(path)?;
    Ok(hex::encode(Sha256::digest(&data)))
}

/// Kasa dizinini (başlık + SQLCipher DB + şifreli blob'lar) tutarlı bir
/// klasöre yedekler. DB `sqlcipher_export` ile alınır; açık metin yazılmaz.
///
/// Yedek **hazırlık dizininde** üretilir, manifestle doğrulanır ve tek
/// `rename` ile yerleşir. Disk dolması ya da kesinti hâlinde geriye yarım
/// bir "DimYedek-…" klasörü kalmaz; kullanıcı `E_DISK_FULL` görür (F15-T06).
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

    if dest_dir.trim().is_empty() {
        return Err(ApiError::new("E_PATH", "Yedek konumu boş olamaz."));
    }
    let root = PathBuf::from(dest_dir.trim());
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
    let archive = root.join(format!("DimYedek-{stamp}.dimbak"));

    let staging = dim_storage::atomic::temp_sibling(&backup_dir, "dimpart");
    dim_storage::atomic::remove_tree(&staging);
    let built = build_backup_staging(&staging, db, vault_path, &vault_id, vault.master_key());
    let (file_count, total_bytes, manifest_hash) = match built {
        Ok(done) => done,
        Err(err) => {
            dim_storage::atomic::remove_tree(&staging);
            return Err(err);
        }
    };
    if let Err(err) = dim_storage::atomic::commit_dir(&staging, &backup_dir) {
        dim_storage::atomic::remove_tree(&staging);
        return Err(err.into());
    }
    if let Err(err) = zip_dir(&backup_dir, &archive) {
        // Klasör yedeği geçerli ve yerinde; yalnız tek dosyalık arşiv yazılamadı.
        return Err(ApiError::new(
            "E_BACKUP_ZIP",
            &format!(
                "Yedek klasörü hazır ama tek dosya arşivi yazılamadı: {}. Klasörü olduğu gibi kullanabilirsiniz.",
                err.message
            ),
        ));
    }

    let _ = db.append_audit(
        "backup",
        None,
        "create",
        "local-user",
        &serde_json::json!({"files": file_count}),
    );
    Ok(BackupView {
        path: backup_dir.to_string_lossy().to_string(),
        file_count,
        total_bytes,
        manifest_hash,
        archive_path: Some(archive.to_string_lossy().to_string()),
    })
}

/// Hazırlık dizinine yedeği yazar, manifesti üretir ve yedeğin açıldığını
/// doğrular. Dönüş: (dosya sayısı, toplam bayt, manifest sha256).
///
/// Tüm yazımlar atomiktir; hata hâlinde çağıran hazırlık dizinini siler.
fn build_backup_staging(
    staging: &Path,
    db: &Database,
    vault_path: &Path,
    vault_id: &str,
    master_key: &[u8; 32],
) -> Result<(usize, u64, String), ApiError> {
    std::fs::create_dir_all(staging.join(dim_storage::backup::BLOBS_DIR))?;
    let key = DatabaseKey::derive_from_master(master_key, vault_id)?;
    let backup_db = staging.join(dim_storage::backup::DB_FILE);
    db.backup_to(&backup_db, &key)?;
    let mut total = dim_storage::atomic::copy_atomic(
        vault_path,
        &staging.join(dim_storage::backup::VAULT_FILE),
    )?;

    let mut blob_count = 0usize;
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
            let dest = staging
                .join(dim_storage::backup::BLOBS_DIR)
                .join(entry.file_name());
            total += dim_storage::atomic::copy_atomic(&entry.path(), &dest)?;
            blob_count += 1;
        }
    }

    let written = dim_storage::backup::write_backup_manifest(staging)?;
    total += std::fs::metadata(&backup_db)?.len();

    // Yedeğin gerçekten açıldığını doğrula: bozuk yedek "başarılı" sayılmaz.
    {
        let verify = Database::open(&backup_db, &key)?;
        verify.integrity_check()?;
    }

    Ok((blob_count + 3, total, written.manifest_sha256))
}

/// Yedek klasörünü yeni bir kasa dizinine kopyalar. Hedef boş olmalıdır;
/// mevcut kasanın üzerine yazılmaz. Açma işlemi kullanıcı parolasıyla yapılır.
///
/// Kural `dim_storage::backup::restore_backup_dir` içinde: **önce doğrula**
/// (manifest boyut/sha256), sonra hazırlık dizinine kopyala, en son tek
/// `rename`. Komut yalnız kapıları (boş yol) ve görünümü taşır (F15-T06).
#[tauri::command]
pub fn vault_restore(backup_dir: String, dest_dir: String) -> Result<RestoreView, ApiError> {
    if backup_dir.trim().is_empty() || dest_dir.trim().is_empty() {
        return Err(ApiError::new("E_PATH", "Yedek ve hedef konumu zorunlu."));
    }
    let dest = PathBuf::from(dest_dir.trim());
    let outcome =
        dim_storage::backup::restore_backup_dir(&PathBuf::from(backup_dir.trim()), &dest)?;
    Ok(RestoreView {
        vault_path: dest.to_string_lossy().to_string(),
        file_count: outcome.blob_count + 3,
        verified_files: outcome.verified_files,
        verified_bytes: outcome.verified_bytes,
        verify_ms: outcome.verify_ms,
        unlisted_blobs: outcome.unlisted,
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

/// Metin içi alıntı denetimi (F12-T14): metindeki çift tırnaklı pasajları
/// dosyanın güncel belge sürümlerinde doğrular. Okuma dışında yazım yapmaz;
/// metin ve alıntı denetime yazılmaz.
#[tauri::command]
pub fn in_text_quote_check(
    state: State<'_, AppState>,
    case_id: String,
    input: InTextQuoteInput,
) -> Result<InTextQuoteCheck, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.check_in_text_quotes(&case_id, &input)?)
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
pub fn claim_relation_create(
    state: State<'_, AppState>,
    case_id: String,
    claim_id: String,
    input: NewClaimRelation,
) -> Result<ClaimRelationRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.link_claim_relation(&case_id, &claim_id, &input)?)
}

#[tauri::command]
pub fn claim_relation_list(
    state: State<'_, AppState>,
    claim_id: String,
) -> Result<Vec<ClaimRelationRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_claim_relations(&claim_id)?)
}

#[tauri::command]
pub fn review_supersede(
    state: State<'_, AppState>,
    item_id: String,
    superseded_by_item_id: String,
) -> Result<ReviewItemRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.supersede_review_item(&item_id, &superseded_by_item_id)?)
}

#[tauri::command]
pub fn contradiction_assess(
    state: State<'_, AppState>,
    case_id: String,
    item_id: String,
    verdict: String,
    rationale: String,
    reviewer: String,
) -> Result<ContradictionAssessmentRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.assess_contradiction(&case_id, &item_id, &verdict, &rationale, &reviewer)?)
}

#[tauri::command]
pub fn contradiction_assessments(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<Vec<ContradictionAssessmentRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_contradiction_assessments(&item_id)?)
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
pub fn review_commit(
    state: State<'_, AppState>,
    item_id: String,
    decision: String,
    target: ReviewTarget,
    reviewer: String,
    rationale: String,
) -> Result<ReviewCommitResult, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.commit_review_decision(&item_id, &decision, &target, &reviewer, &rationale)?)
}

#[tauri::command]
pub fn review_outbox_pending(
    state: State<'_, AppState>,
    case_id: String,
) -> Result<Vec<OutboxEventRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_outbox_events(Some(&case_id), true)?)
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

/// Katalogdaki lisanslı kontrol listeleri (F08-T11).
#[tauri::command]
pub fn checklist_templates(
    state: State<'_, AppState>,
    case_id: String,
) -> Result<Vec<ChecklistTemplateRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_checklist_templates(&case_id)?)
}

/// Şablonu sürüm ve lisans künyesiyle dosyaya kopyalar (idempotent).
#[tauri::command]
pub fn checklist_copy(
    state: State<'_, AppState>,
    case_id: String,
    template_id: String,
) -> Result<ChecklistCopyResult, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.copy_checklist_to_case(&case_id, &template_id)?)
}

#[tauri::command]
pub fn checklist_items(
    state: State<'_, AppState>,
    case_id: String,
) -> Result<Vec<CaseChecklistItemRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.list_case_checklist(&case_id)?)
}

/// Öğe durumu: `present` belge bağı, `missing`/`not_applicable` gerekçe ister;
/// yazma `expected_row_version` ile korunur.
#[tauri::command]
pub fn checklist_item_state(
    state: State<'_, AppState>,
    item_id: String,
    new_state: String,
    linked_document_ids: Vec<String>,
    note: String,
    expected_row_version: i64,
) -> Result<CaseChecklistItemRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.set_case_checklist_state(
        &item_id,
        &new_state,
        &linked_document_ids,
        &note,
        expected_row_version,
    )?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterArchView {
    pub format: &'static str,
    pub format_label: &'static str,
    pub architectures: Vec<String>,
    pub comparison: &'static str,
    pub comparison_label: &'static str,
    pub contains_app_arch: Option<bool>,
    pub reason: Option<&'static str>,
    pub summary: String,
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
    /// PATH'te bulunan yürütülebilir (varsa); bağ çözümüyle birlikte.
    pub path: Option<String>,
    /// Başlık baytlarından okunan mimari (F10-T01). Kurulu değilse `None`.
    pub architecture: Option<AdapterArchView>,
}

fn adapter_arch_view(report: &dim_cli_adapters::ArchReport) -> AdapterArchView {
    AdapterArchView {
        format: report.format.as_str(),
        format_label: report.format.label(),
        architectures: report.architectures.iter().map(|a| a.as_str()).collect(),
        comparison: report.comparison.as_str(),
        comparison_label: report.comparison.label(),
        contains_app_arch: report.contains_app_arch,
        reason: report.reason,
        summary: report.summary(),
    }
}

/// CLI adaptörlerini yoklar: yalnız `--version` çalıştırılır. Kimlik dosyası
/// okunmaz, oturum açılmaz, istek gönderilmez. Yürütülebilirin mimarisi
/// **başlık baytlarından** okunur (F10-T01); süreç başlatılmaz.
#[tauri::command]
pub fn provider_probe() -> Vec<AdapterView> {
    SPECS
        .iter()
        .map(|spec| {
            let discovery = discover(spec);
            let (state, version, path) = match discovery.state {
                DiscoveryState::Installed { version, path } => {
                    ("installed", Some(version), Some(path))
                }
                DiscoveryState::Unverified { path, .. } => ("unverified", None, Some(path)),
                DiscoveryState::NotInstalled => ("not_installed", None, None),
            };
            let architecture = path
                .as_ref()
                .map(|executable| adapter_arch_view(&dim_cli_adapters::inspect_path(executable)));
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
                path: path.map(|p| p.to_string_lossy().to_string()),
                architecture,
            }
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliProbeInput {
    pub adapter_id: String,
}

/// Kapsam kanıtı: iş dizini izni, `HOME`/`TMPDIR` politikası, stdin boyutu ve
/// alt sürecin kum havuzunda bıraktığı dosyalar. **Dosya içeriği taşınmaz.**
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliProbeSandboxView {
    pub workdir: String,
    pub workdir_mode: Option<String>,
    pub home: Option<String>,
    pub tmp: Option<String>,
    pub home_isolation: &'static str,
    pub stdin_bytes: usize,
    pub stdin_limit_bytes: usize,
    pub files: Vec<CliProbeSandboxFile>,
    pub files_truncated: bool,
    pub total_bytes: u64,
    pub size_limit_bytes: u64,
    pub entry_limit: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliProbeSandboxFile {
    pub name: String,
    pub bytes: u64,
}

/// Arayüzdeki "Bağlantıyı sına" düğmesinin gördüğü sonuç. Hangi argv ile,
/// hangi ortam anahtarlarıyla, nerede ve ne kadar süre çalıştığı açıkça yazılır.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliProbeView {
    pub adapter_id: String,
    pub display_name: String,
    pub executable: String,
    pub args: Vec<String>,
    pub env_keys: Vec<String>,
    pub workdir: String,
    pub timeout_ms: u64,
    pub status: &'static str,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub stdout_head: String,
    pub stderr_head: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub sandbox: CliProbeSandboxView,
}

fn probe_status_label(status: &RunStatus) -> &'static str {
    match status {
        RunStatus::Exited(_) => "exited",
        RunStatus::TimedOut => "timed_out",
        RunStatus::Cancelled => "cancelled",
        RunStatus::Signaled => "signaled",
    }
}

/// **Gerçek** bağlantı sınaması: sürüm yoklaması yürütme çekirdeğinden geçer
/// (izinli argv, allowlist ortam, boş geçici iş dizini, 15 sn sınır, iptal).
/// İstem gönderilmez, kimlik dosyası okunmaz, model çağrılmaz.
#[tauri::command]
pub async fn provider_probe_run(
    state: State<'_, AppState>,
    input: CliProbeInput,
) -> Result<CliProbeView, ApiError> {
    let spec = adapter_spec(&input.adapter_id)
        .ok_or_else(|| ApiError::new("E_CLI_UNKNOWN", "Bilinmeyen adaptör kimliği."))?;
    if spec.batch_forbidden {
        return Err(ApiError::new(
            "E_CLI_BATCH_FORBIDDEN",
            "Bu sağlayıcıda otomatik yürütme kapalı; yalnız etkileşimli kullanım var.",
        ));
    }
    let executable = find_on_path(spec.command).ok_or_else(|| {
        ApiError::new(
            "E_CLI_NOT_INSTALLED",
            "Araç PATH üzerinde bulunamadı; kurulum rehberine bakın.",
        )
    })?;

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state.cli_probe_lock();
        if slot.is_some() {
            return Err(ApiError::new(
                "E_CLI_BUSY",
                "Zaten çalışan bir bağlantı sınaması var.",
            ));
        }
        *slot = Some(Arc::clone(&cancel));
    }

    let run_cancel = Arc::clone(&cancel);
    let joined =
        tauri::async_runtime::spawn_blocking(move || run_probe(spec, executable, &run_cancel))
            .await;

    // Yuva her durumda boşaltılır; iptal edilen ya da düşen koşu yeniyi engellemez.
    {
        let mut slot = state.cli_probe_lock();
        *slot = None;
    }

    let report = match joined {
        Ok(Ok(report)) => report,
        Ok(Err(err)) => return Err(ApiError::new(err.code(), &err.to_string())),
        Err(_) => {
            return Err(ApiError::new(
                "E_CLI_EXEC_SPAWN",
                "Sınama görevi tamamlanamadı.",
            ))
        }
    };

    let exit_code = match report.outcome.status {
        RunStatus::Exited(code) => Some(code),
        _ => None,
    };
    let scope = &report.outcome.scope;
    let sandbox = CliProbeSandboxView {
        workdir: scope.workdir.to_string_lossy().to_string(),
        workdir_mode: scope.workdir_mode.map(|mode| format!("{mode:04o}")),
        home: scope.home.as_ref().map(|p| p.to_string_lossy().to_string()),
        tmp: scope.tmp.as_ref().map(|p| p.to_string_lossy().to_string()),
        home_isolation: scope.home_isolation.as_str(),
        stdin_bytes: scope.stdin_bytes,
        stdin_limit_bytes: scope.stdin_limit,
        files: scope
            .inventory
            .iter()
            .map(|entry| CliProbeSandboxFile {
                name: entry.name.clone(),
                bytes: entry.bytes,
            })
            .collect(),
        files_truncated: scope.inventory_truncated,
        total_bytes: scope.inventory_total_bytes,
        size_limit_bytes: scope.sandbox_size_limit,
        entry_limit: scope.inventory_limit_entries,
    };
    let view = CliProbeView {
        adapter_id: spec.id.to_string(),
        display_name: spec.display_name.to_string(),
        executable: report.executable.to_string_lossy().to_string(),
        args: report.args.clone(),
        env_keys: report.env_keys.clone(),
        workdir: report.workdir.to_string_lossy().to_string(),
        timeout_ms: u64::try_from(report.timeout.as_millis()).unwrap_or(u64::MAX),
        status: probe_status_label(&report.outcome.status),
        exit_code,
        duration_ms: report.outcome.duration_ms,
        stdout_head: report.outcome.stdout_head.clone(),
        stderr_head: report.outcome.stderr_head.clone(),
        stdout_truncated: report.outcome.stdout_truncated,
        stderr_truncated: report.outcome.stderr_truncated,
        sandbox,
    };

    // Kasa açıksa sınama denetime yazılır; **çıktı metni ve dosya adları
    // denetime yazılmaz** (yalnız izin, politika ve sayılar).
    {
        let guard = state.lock();
        if let Some(db) = guard.db.as_ref() {
            let metadata = serde_json::json!({
                "adapterId": view.adapter_id,
                "args": view.args,
                "envKeys": view.env_keys,
                "status": view.status,
                "exitCode": view.exit_code,
                "durationMs": view.duration_ms,
                "stdoutTruncated": view.stdout_truncated,
                "stderrTruncated": view.stderr_truncated,
                "workdirMode": view.sandbox.workdir_mode,
                "homeIsolation": view.sandbox.home_isolation,
                "stdinBytes": view.sandbox.stdin_bytes,
                "stdinLimitBytes": view.sandbox.stdin_limit_bytes,
                "sandboxFileCount": view.sandbox.files.len(),
                "sandboxTotalBytes": view.sandbox.total_bytes,
                "sandboxFilesTruncated": view.sandbox.files_truncated,
            });
            db.append_audit("cli_probe", None, "run", "local-user", &metadata)?;
        }
    }

    Ok(view)
}

/// Süren sınamayı iptal eder: alt süreç öldürülür, kısmi çıktı raporlanır.
/// `true` = işaretlenecek bir koşu vardı.
#[tauri::command]
pub fn provider_probe_cancel(state: State<'_, AppState>) -> bool {
    let slot = state.cli_probe_lock();
    match slot.as_ref() {
        Some(flag) => {
            flag.store(true, Ordering::SeqCst);
            true
        }
        None => false,
    }
}

// --- F10-T01: mimari uyum denetimi ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliArchInterpreterView {
    pub command: String,
    pub path: Option<String>,
    pub resolved: Option<String>,
    pub format: &'static str,
    pub architectures: Vec<String>,
    pub comparison: &'static str,
    pub reason: Option<&'static str>,
}

/// Adaptör başına mimari denetimi. Yalnız başlık baytları okunur; dosya
/// **çalıştırılmaz**, süreç başlatılmaz.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliArchAdapterView {
    pub adapter_id: &'static str,
    pub display_name: &'static str,
    pub command: &'static str,
    pub installed: bool,
    pub path: Option<String>,
    pub resolved: Option<String>,
    pub symlink: bool,
    pub format: &'static str,
    pub format_label: &'static str,
    pub architectures: Vec<String>,
    pub comparison: &'static str,
    pub comparison_label: &'static str,
    pub contains_app_arch: Option<bool>,
    pub summary: String,
    pub interpreter: Option<CliArchInterpreterView>,
    pub reason: Option<&'static str>,
    pub header_bytes: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliArchReportView {
    pub app_arch: String,
    pub probed_at_unix: u64,
    pub adapters: Vec<CliArchAdapterView>,
    pub same: usize,
    pub different: usize,
    pub fat: usize,
    pub unknown: usize,
    pub notes: Vec<&'static str>,
}

/// **Gerçek** mimari denetimi (F10-T01): her adaptörün PATH'te bulunan
/// yürütülebiliri başlık baytlarından okunur (Mach-O thin/fat, ELF, PE,
/// `#!` betiği → shebang yorumlayıcısı) ve uygulamanın süreç mimarisiyle
/// karşılaştırılır. Sonuç `same` / `different` / `fat` / `unknown`.
/// Süreç başlatılmaz, dosya çalıştırılmaz, ağ kullanılmaz.
#[tauri::command]
pub fn cli_arch_report(state: State<'_, AppState>) -> Result<CliArchReportView, ApiError> {
    let mut adapters: Vec<CliArchAdapterView> = Vec::new();
    for adapter in SPECS {
        let executable = find_on_path(adapter.command);
        let Some(path) = executable else {
            adapters.push(CliArchAdapterView {
                adapter_id: adapter.id,
                display_name: adapter.display_name,
                command: adapter.command,
                installed: false,
                path: None,
                resolved: None,
                symlink: false,
                format: "unknown",
                format_label: "bilinmiyor",
                architectures: Vec::new(),
                comparison: "unknown",
                comparison_label: "mimari doğrulanamadı",
                contains_app_arch: None,
                summary: "PATH üzerinde bulunamadı".to_string(),
                interpreter: None,
                reason: Some("not_found"),
                header_bytes: 0,
            });
            continue;
        };
        let report = dim_cli_adapters::inspect_path(&path);
        adapters.push(CliArchAdapterView {
            adapter_id: adapter.id,
            display_name: adapter.display_name,
            command: adapter.command,
            installed: true,
            path: Some(path.to_string_lossy().to_string()),
            resolved: report
                .resolved
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            symlink: report.symlink,
            format: report.format.as_str(),
            format_label: report.format.label(),
            architectures: report.architectures.iter().map(|a| a.as_str()).collect(),
            comparison: report.comparison.as_str(),
            comparison_label: report.comparison.label(),
            contains_app_arch: report.contains_app_arch,
            summary: report.summary(),
            interpreter: report
                .interpreter
                .as_ref()
                .map(|interp| CliArchInterpreterView {
                    command: interp.command.clone(),
                    path: interp
                        .path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string()),
                    resolved: interp
                        .resolved
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string()),
                    format: interp.format.as_str(),
                    architectures: interp.architectures.iter().map(|a| a.as_str()).collect(),
                    comparison: interp.comparison.as_str(),
                    reason: interp.reason,
                }),
            reason: report.reason,
            header_bytes: report.header_bytes,
        });
    }

    let count = |wanted: &str| {
        adapters
            .iter()
            .filter(|row| row.comparison == wanted)
            .count()
    };

    // Denetime yalnız kimlik, biçim ve karar yazılır; yol ve shebang yazılmaz.
    {
        let guard = state.lock();
        if let Some(db) = guard.db.as_ref() {
            let metadata = serde_json::json!({
                "appArch": dim_cli_adapters::app_arch_label(),
                "adapters": adapters
                    .iter()
                    .map(|a| serde_json::json!({
                        "adapterId": a.adapter_id,
                        "installed": a.installed,
                        "format": a.format,
                        "comparison": a.comparison,
                        "reason": a.reason,
                    }))
                    .collect::<Vec<_>>(),
            });
            db.append_audit("cli_arch", None, "probe", "local-user", &metadata)?;
        }
    }

    let probed_at_unix = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(CliArchReportView {
        app_arch: dim_cli_adapters::app_arch_label(),
        probed_at_unix,
        same: count("same"),
        different: count("different"),
        fat: count("fat"),
        unknown: count("unknown"),
        adapters,
        notes: vec![
            "Yalnız başlık baytları okunur (en çok 4 KiB; PE başlığı ilerideyse o bölüm). Dosya çalıştırılmaz, süreç başlatılmaz.",
            "Karşılaştırma uygulamanın süreç mimarisiyle yapılır (std::env::consts::ARCH). Rosetta altında çalışan x86_64 süreç kendini x86_64 bildirir; donanım mimarisi ayrıca okunmaz.",
            "Betiğin kendi mimarisi yoktur: sonuç shebang yorumlayıcısından gelir. Yorumlayıcı bulunamazsa mimari 'bilinmiyor' kalır.",
            "Evrensel (fat) ikili ayrı durumdur; uygulama mimarisini içerip içermediği ayrıca yazılır.",
            "Mimari uyumu tek başına güvenlik kanıtı değildir; sürüm, yetenek kapısı (T11) ve kapsam (T05) ayrı denetlenir.",
        ],
    })
}

// --- F10-T11: yetenek matrisi ve bağlantı durumu ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliCapabilityRowView {
    pub capability: &'static str,
    pub label: &'static str,
    pub required: bool,
    pub note: &'static str,
    pub state: &'static str,
    pub state_label: &'static str,
    /// Bildirilen bayraklar (`declared`) ya da eksik olanlar (`missing`).
    pub flags: Vec<String>,
    /// Durumun ayrıntısı: bayrak listesi, eksik bayraklar ya da neden kodu.
    pub detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliCapabilityAdapterView {
    pub adapter_id: &'static str,
    pub display_name: &'static str,
    pub command: &'static str,
    pub version: Option<String>,
    pub help_args: Vec<String>,
    pub help_read: bool,
    pub help_error: Option<String>,
    pub link: &'static str,
    pub link_label: &'static str,
    pub gate_open: bool,
    pub gate_reasons: Vec<String>,
    pub rows: Vec<CliCapabilityRowView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliCapabilityMatrixView {
    pub adapters: Vec<CliCapabilityAdapterView>,
    pub probed_at_unix: u64,
    pub help_timeout_ms: u64,
    pub notes: Vec<&'static str>,
}

fn capability_rows(report: &dim_cli_adapters::CapabilityReport) -> Vec<CliCapabilityRowView> {
    report
        .rows
        .iter()
        .map(|row| {
            let (flags, detail) = match &row.state {
                dim_cli_adapters::CapabilityState::Declared { flags } => (
                    flags.iter().map(|f| (*f).to_string()).collect::<Vec<_>>(),
                    flags.join(" "),
                ),
                dim_cli_adapters::CapabilityState::Missing { absent } => (
                    absent.iter().map(|f| (*f).to_string()).collect::<Vec<_>>(),
                    format!("eksik: {}", absent.join(" ")),
                ),
                dim_cli_adapters::CapabilityState::NoPath { reason } => {
                    (Vec::new(), (*reason).to_string())
                }
                dim_cli_adapters::CapabilityState::Unread { reason } => {
                    (Vec::new(), reason.clone())
                }
            };
            CliCapabilityRowView {
                capability: row.capability.id(),
                label: row.capability.label(),
                required: row.required,
                note: row.note,
                state: row.state.id(),
                state_label: row.state.label(),
                flags,
                detail,
            }
        })
        .collect()
}

/// Dört adaptörün **yetenek matrisini** yoklar: kurulu sürüm + o sürümün kendi
/// `--help` metni. Gerekli bir yetenek yoksa bağlantı **uyumsuz**, doğrulanamıyorsa
/// **sınırlı** olur ve yetenek kapısı kapanır. İstem gönderilmez; yardım metni
/// denetime yazılmaz, yalnız durumlar ve bayrak adları yazılır.
#[tauri::command]
pub async fn cli_capability_matrix(
    state: State<'_, AppState>,
) -> Result<CliCapabilityMatrixView, ApiError> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state.cli_probe_lock();
        if slot.is_some() {
            return Err(ApiError::new(
                "E_CLI_BUSY",
                "Zaten çalışan bir CLI işlemi var.",
            ));
        }
        *slot = Some(Arc::clone(&cancel));
    }
    let run_cancel = Arc::clone(&cancel);
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let mut out = Vec::with_capacity(SPECS.len());
        for adapter in SPECS {
            let discovery = discover(adapter);
            let version = match &discovery.state {
                DiscoveryState::Installed { version, .. } => Some(version.clone()),
                _ => None,
            };
            // Yardım yalnız kurulu ve otomatik yürütmeye açık adaptörde yoklanır
            // (`help_probe=false` → etkileşimli-only, süreç başlatılmaz).
            let (help, help_error) =
                match (&discovery.state, dim_cli_adapters::capabilities(adapter.id)) {
                    (DiscoveryState::Installed { .. }, Some(caps)) if caps.help_probe => {
                        match find_on_path(adapter.command) {
                            Some(executable) => {
                                match dim_cli_adapters::run_help_probe(
                                    adapter,
                                    &executable,
                                    &run_cancel,
                                ) {
                                    Ok(probe) => match dim_cli_adapters::help_text_of(&probe) {
                                        Ok(text) => (Some(text.to_string()), None),
                                        Err(reason) => (None, Some(reason.to_string())),
                                    },
                                    Err(err) => (None, Some(err.code().to_string())),
                                }
                            }
                            None => (None, Some("not_installed".to_string())),
                        }
                    }
                    _ => (None, None),
                };
            let Some(report) = dim_cli_adapters::evaluate(
                adapter.id,
                version.as_deref(),
                help.as_deref(),
                help_error.as_deref(),
            ) else {
                continue;
            };
            out.push(CliCapabilityAdapterView {
                adapter_id: report.adapter_id,
                display_name: adapter.display_name,
                command: adapter.command,
                version: report.installed_version.clone(),
                help_args: report.help_args.clone(),
                help_read: report.help_read,
                help_error: report.help_error.clone(),
                link: report.link.id(),
                link_label: report.link.label(),
                gate_open: report.gate_open,
                gate_reasons: report.gate_reasons.clone(),
                rows: capability_rows(&report),
            });
        }
        out
    })
    .await;
    {
        let mut slot = state.cli_probe_lock();
        *slot = None;
    }
    let adapters = match joined {
        Ok(adapters) => adapters,
        Err(_) => {
            return Err(ApiError::new(
                "E_CLI_EXEC_SPAWN",
                "Yetenek yoklaması tamamlanamadı.",
            ))
        }
    };

    // Denetime yalnız durum ve bayrak adları yazılır; yardım metni yazılmaz.
    {
        let guard = state.lock();
        if let Some(db) = guard.db.as_ref() {
            let metadata = serde_json::json!({
                "adapters": adapters
                    .iter()
                    .map(|a| serde_json::json!({
                        "adapterId": a.adapter_id,
                        "version": a.version,
                        "link": a.link,
                        "gateOpen": a.gate_open,
                        "gateReasons": a.gate_reasons,
                        "helpRead": a.help_read,
                        "helpError": a.help_error,
                    }))
                    .collect::<Vec<_>>(),
            });
            db.append_audit("cli_capability", None, "probe", "local-user", &metadata)?;
        }
    }

    let probed_at_unix = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(CliCapabilityMatrixView {
        adapters,
        probed_at_unix,
        help_timeout_ms: u64::try_from(dim_cli_adapters::HELP_TIMEOUT.as_millis())
            .unwrap_or(u64::MAX),
        notes: vec![
            "Yetenek kanıtı kurulu sürümün kendi --help metnidir; davranış (araç çağrısı, oturum yazımı) ölçülmedi.",
            "Gerekli bir yetenek yardım metninde yoksa bağlantı uyumsuz, doğrulanamıyorsa sınırlı olur ve yetenek kapısı kapanır.",
            "Kesilmiş, boş ya da sıfır olmayan çıkışlı yardım çıktısı kanıt sayılmaz.",
        ],
    })
}

// --- F10-T12: başlangıç saldırıları (hook, MCP, proje talimatı, kapsam dışı canary) ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliAttackVectorView {
    pub vector: &'static str,
    pub label: &'static str,
    pub note: &'static str,
    pub guard: &'static str,
    pub guard_label: &'static str,
    pub guard_flags: Vec<&'static str>,
    pub hit: bool,
    pub source: &'static str,
    pub source_label: &'static str,
    pub verdict: &'static str,
    pub verdict_label: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliAttackProductView {
    pub vector: &'static str,
    pub label: &'static str,
    pub hit: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliAttackFixtureView {
    pub detector_proven: bool,
    pub detector_control_clean: bool,
    pub polluted_hits: usize,
    pub clean_hits: usize,
    pub vector_total: usize,
    pub gate_rejected: bool,
    pub gate_code: Option<&'static str>,
    pub product_config: Vec<CliAttackProductView>,
    pub canary_read_in_product_config: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliAttackAdapterView {
    pub adapter_id: String,
    pub display_name: &'static str,
    pub version: Option<String>,
    pub argv: Vec<String>,
    pub status: &'static str,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub hits: usize,
    pub help_read: bool,
    pub help_error: Option<String>,
    pub link: &'static str,
    pub link_label: &'static str,
    pub gate_reasons: Vec<String>,
    pub vectors: Vec<CliAttackVectorView>,
    /// Koşu yapılmadıysa nedeni (`not_installed`, `interactive_only`,
    /// `executable_missing` ya da hata kodu).
    pub skipped: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliAttackView {
    pub fixture: CliAttackFixtureView,
    pub adapters: Vec<CliAttackAdapterView>,
    pub probed_at_unix: u64,
    pub notes: Vec<&'static str>,
}

fn run_status_id(status: &RunStatus) -> &'static str {
    match status {
        RunStatus::Exited(0) => "exited_ok",
        RunStatus::Exited(_) => "exited_error",
        RunStatus::TimedOut => "timed_out",
        RunStatus::Cancelled => "cancelled",
        RunStatus::Signaled => "signaled",
    }
}

fn attack_vector_views(outcomes: &[dim_cli_adapters::AttackOutcome]) -> Vec<CliAttackVectorView> {
    outcomes
        .iter()
        .map(|outcome| CliAttackVectorView {
            vector: outcome.vector.id(),
            label: outcome.vector.label(),
            note: outcome.vector.note(),
            guard: outcome.guard.id(),
            guard_label: outcome.guard.label(),
            guard_flags: outcome.guard.flags().to_vec(),
            hit: outcome.observation.hit,
            source: outcome.observation.source.id(),
            source_label: outcome.observation.source.label(),
            verdict: outcome.verdict.id(),
            verdict_label: outcome.verdict.label(),
        })
        .collect()
}

/// **Başlangıç saldırılarını** ölçer (F10-T12): fikstür dedektör kanıtı + kirli
/// proje dizininin iş dizini olarak reddi + kurulu CLI'larda sertleştirme
/// bayraklarıyla gerçek koşu. İstem gönderilmez; kimlik dosyası okunmaz; fikstür
/// belirteci ve yardım metni denetime **yazılmaz**.
#[tauri::command]
pub async fn cli_attack_probe(state: State<'_, AppState>) -> Result<CliAttackView, ApiError> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state.cli_probe_lock();
        if slot.is_some() {
            return Err(ApiError::new(
                "E_CLI_BUSY",
                "Zaten çalışan bir CLI işlemi var.",
            ));
        }
        *slot = Some(Arc::clone(&cancel));
    }
    let run_cancel = Arc::clone(&cancel);
    let joined = tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
        let fixture =
            dim_cli_adapters::AttackFixture::create().map_err(|err| err.code().to_string())?;
        let result = fixture.run().map_err(|err| err.code().to_string())?;
        let mut adapters = Vec::with_capacity(SPECS.len());
        for adapter in SPECS {
            let discovery = discover(adapter);
            let version = match &discovery.state {
                DiscoveryState::Installed { version, .. } => Some(version.clone()),
                _ => None,
            };
            let mut row = CliAttackAdapterView {
                adapter_id: adapter.id.to_string(),
                display_name: adapter.display_name,
                version: version.clone(),
                argv: Vec::new(),
                status: "skipped",
                exit_code: None,
                duration_ms: 0,
                hits: 0,
                help_read: false,
                help_error: None,
                link: "not_checked",
                link_label: "koşulmadı",
                gate_reasons: Vec::new(),
                vectors: Vec::new(),
                skipped: None,
            };
            let caps = dim_cli_adapters::capabilities(adapter.id);
            if version.is_none() {
                row.skipped = Some("not_installed");
                adapters.push(row);
                continue;
            }
            if caps.is_none_or(|caps| caps.help_args.is_empty()) {
                row.skipped = Some("interactive_only");
                adapters.push(row);
                continue;
            }
            let Some(executable) = find_on_path(adapter.command) else {
                row.skipped = Some("executable_missing");
                adapters.push(row);
                continue;
            };
            match dim_cli_adapters::run_real_attack(&fixture, adapter, &executable, &run_cancel) {
                Ok(run) => {
                    let probe = dim_cli_adapters::HelpProbe {
                        args: run.argv.clone(),
                        stdout: run.stdout_head.clone(),
                        stderr: run.stderr_head.clone(),
                        status: run.status.clone(),
                        duration_ms: run.duration_ms,
                        truncated: run.truncated,
                    };
                    let help = dim_cli_adapters::help_text_of(&probe).ok();
                    let report =
                        dim_cli_adapters::evaluate(adapter.id, version.as_deref(), help, None);
                    row.argv = run.argv.clone();
                    row.status = run_status_id(&run.status);
                    row.exit_code = run.exit_code;
                    row.duration_ms = run.duration_ms;
                    row.hits = run.observations.iter().filter(|o| o.hit).count();
                    row.help_read = run.help_read;
                    row.help_error = run.help_error.map(str::to_string);
                    if let Some(report) = &report {
                        row.link = report.link.id();
                        row.link_label = report.link.label();
                        row.gate_reasons = report.gate_reasons.clone();
                        row.vectors = attack_vector_views(&dim_cli_adapters::outcomes(
                            report,
                            &run.observations,
                            result.detector.proven,
                        ));
                    }
                    if !matches!(run.status, RunStatus::Exited(0)) {
                        // Sıfır dışı çıkışta yardım metni kanıt değildir; vektör
                        // kararları yine de "ölçülemedi"ye düşer.
                        row.skipped = Some("run_failed");
                        row.vectors.clear();
                    }
                    adapters.push(row);
                }
                Err(err) => {
                    row.skipped = Some(err.code());
                    adapters.push(row);
                }
            }
        }
        Ok((result, adapters))
    })
    .await;
    {
        let mut slot = state.cli_probe_lock();
        *slot = None;
    }
    let (result, adapters) = match joined {
        Ok(Ok(value)) => value,
        Ok(Err(code)) => return Err(ApiError::new(&code, "Saldırı fikstürü hazırlanamadı.")),
        Err(_) => {
            return Err(ApiError::new(
                "E_CLI_EXEC_SPAWN",
                "Saldırı testi tamamlanamadı.",
            ))
        }
    };

    // Denetime yalnız sayı/durum yazılır: fikstür belirteci, dosya adı ve yardım
    // metni yazılmaz.
    {
        let guard = state.lock();
        if let Some(db) = guard.db.as_ref() {
            let metadata = serde_json::json!({
                "detectorProven": result.detector.proven,
                "detectorControlClean": result.detector.control_clean,
                "pollutedHits": result.detector.polluted_hits,
                "cleanHits": result.detector.clean_hits,
                "gateRejected": result.gate.rejected,
                "gateCode": result.gate.code,
                "canaryReadInProductConfig": result.canary_read_in_product_config(),
                "adapters": adapters
                    .iter()
                    .map(|a| serde_json::json!({
                        "adapterId": a.adapter_id,
                        "version": a.version,
                        "status": a.status,
                        "exitCode": a.exit_code,
                        "hits": a.hits,
                        "skipped": a.skipped,
                        "verdicts": a
                            .vectors
                            .iter()
                            .map(|v| serde_json::json!({
                                "vector": v.vector,
                                "verdict": v.verdict,
                                "hit": v.hit,
                                "source": v.source,
                            }))
                            .collect::<Vec<_>>(),
                    }))
                    .collect::<Vec<_>>(),
            });
            db.append_audit("cli_attack", None, "probe", "local-user", &metadata)?;
        }
    }

    let fixture = CliAttackFixtureView {
        detector_proven: result.detector.proven,
        detector_control_clean: result.detector.control_clean,
        polluted_hits: result.detector.polluted_hits,
        clean_hits: result.detector.clean_hits,
        vector_total: dim_cli_adapters::AttackVector::ALL.len(),
        gate_rejected: result.gate.rejected,
        gate_code: result.gate.code,
        product_config: result
            .product_config
            .iter()
            .map(|observation| CliAttackProductView {
                vector: observation.vector.id(),
                label: observation.vector.label(),
                hit: observation.hit,
            })
            .collect(),
        canary_read_in_product_config: result.canary_read_in_product_config(),
    };

    let probed_at_unix = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(CliAttackView {
        fixture,
        adapters,
        probed_at_unix,
        notes: vec![
            "Vektörler geçici bir fikstürde kurulur; gerçek CLI'ya istem gönderilmez, kimlik dosyası okunmaz.",
            "Dedektör kanıtı: kirli fikstürde naif yükleyici taklidi dört vektörün hepsinde işaret bıraktı, temiz fikstürde hiçbirinde. Vekil gerçek CLI kodu değildir.",
            "Sessizlik davranış kanıtı değildir: hook/MCP/proje talimatı ancak bir oturum açıldığında yüklenir; bu çağrıda oturum açılmadı.",
            "Kirli proje dizini iş dizini olarak reddedilir; ürün CLI'ya proje dizini vermez, iş dizini her zaman boş kum havuzudur.",
            "Kapsam dışı canary: mutlak yolla okuma işletim sistemi düzeyinde engellenmiyor; koruma istem/araç yolunun yokluğuna dayanır.",
            "Kimi etkileşimli-only olduğu için otomatik saldırı koşusu yapılmaz.",
        ],
    })
}

// --- F10-T13: CLI kalıntı envanteri, ölçümü ve yalnız uygulamaya ait temizlik ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliResiduePathView {
    pub relative: String,
    pub kind: &'static str,
    pub note: &'static str,
    pub exists: bool,
    pub files: usize,
    pub bytes: u64,
    pub truncated: bool,
    pub newest_age_secs: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliResidueScanView {
    pub adapter_id: String,
    pub root: String,
    pub paths: Vec<CliResiduePathView>,
    pub total_files: usize,
    pub total_bytes: u64,
    pub truncated: bool,
    pub file_limit: usize,
    /// Oturum kalıcılığını kapatma bayrağı (yalnız `--help` ile doğrulandıysa).
    pub persistence_flag: Option<&'static str>,
    pub persistence_applies_to: &'static str,
    pub persistence_verified_with: Option<&'static str>,
}

fn residue_scan_view(root: &Path, adapter_id: &str) -> CliResidueScanView {
    let scan = dim_cli_adapters::scan_residue(root, adapter_id, SystemTime::now());
    residue_scan_view_of(scan)
}

fn residue_scan_view_of(scan: dim_cli_adapters::ResidueScan) -> CliResidueScanView {
    let control = dim_cli_adapters::persistence_control(&scan.adapter);
    CliResidueScanView {
        adapter_id: scan.adapter,
        root: scan.root.to_string_lossy().to_string(),
        paths: scan
            .entries
            .into_iter()
            .map(|entry| CliResiduePathView {
                relative: entry.relative,
                kind: entry.kind,
                note: entry.note,
                exists: entry.exists,
                files: entry.files,
                bytes: entry.bytes,
                truncated: entry.truncated,
                newest_age_secs: entry.newest_age_secs,
            })
            .collect(),
        total_files: scan.total_files,
        total_bytes: scan.total_bytes,
        truncated: scan.truncated,
        file_limit: dim_cli_adapters::MAX_RESIDUE_FILES,
        persistence_flag: control.flag,
        persistence_applies_to: control.applies_to,
        persistence_verified_with: control.verified_with,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliResidueFileView {
    pub relative: String,
    pub bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliResidueSkipView {
    pub path: String,
    pub reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliResidueMeasureView {
    pub adapter_id: String,
    pub root: String,
    pub measured_at_unix: u64,
    pub used_user_home: bool,
    pub status: &'static str,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub args: Vec<String>,
    pub before: CliResidueScanView,
    pub after: CliResidueScanView,
    pub created: Vec<CliResidueFileView>,
    pub modified: Vec<CliResidueFileView>,
    pub removed: Vec<String>,
    pub created_dirs: Vec<String>,
    pub bytes_added: u64,
    pub cleanup_hint: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliResidueCleanupView {
    pub removed_files: usize,
    pub removed_bytes: u64,
    pub removed_dirs: usize,
    pub skipped: Vec<CliResidueSkipView>,
    pub errors: Vec<CliResidueSkipView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliSandboxSweepView {
    pub considered: usize,
    pub removed: Vec<String>,
    pub skipped: Vec<CliResidueSkipView>,
    pub min_age_secs: u64,
}

fn user_home() -> Result<PathBuf, ApiError> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| ApiError::new("E_CLI_RESIDUE_IO", "Ev dizini belirlenemedi."))
}

/// Bildirilen kalıntı yollarını **üst veriyle** tarar: dosya içeriği açılmaz,
/// sembolik bağ izlenmez, kullanıcı ev dizini genel olarak taranmaz.
#[tauri::command]
pub fn cli_residue_scan(input: CliProbeInput) -> Result<CliResidueScanView, ApiError> {
    if adapter_spec(&input.adapter_id).is_none() {
        return Err(ApiError::new(
            "E_CLI_UNKNOWN",
            "Bilinmeyen adaptör kimliği.",
        ));
    }
    let home = user_home()?;
    Ok(residue_scan_view(&home, &input.adapter_id))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliResidueMeasureInput {
    pub adapter_id: String,
    /// Gerçek ev dizininde çalıştırma **açık onay** ister.
    pub confirm_user_home: bool,
}

/// Kalıntı ölçümü: sürüm yoklaması **gerçek ev dizininde** çalışır, önce/sonra
/// farkı yalnız bildirilen yollarda hesaplanır. İstem gönderilmez.
#[tauri::command]
pub async fn cli_residue_measure(
    state: State<'_, AppState>,
    input: CliResidueMeasureInput,
) -> Result<CliResidueMeasureView, ApiError> {
    if !input.confirm_user_home {
        return Err(ApiError::new(
            "E_CLI_RESIDUE_CONFIRM",
            "Kalıntı ölçümü gerçek ev dizininde çalışır; açık onay gerekir.",
        ));
    }
    let spec = adapter_spec(&input.adapter_id)
        .ok_or_else(|| ApiError::new("E_CLI_UNKNOWN", "Bilinmeyen adaptör kimliği."))?;
    if spec.batch_forbidden {
        return Err(ApiError::new(
            "E_CLI_BATCH_FORBIDDEN",
            "Bu sağlayıcıda otomatik yürütme kapalı; ölçüm yapılmaz.",
        ));
    }
    let executable = find_on_path(spec.command).ok_or_else(|| {
        ApiError::new(
            "E_CLI_NOT_INSTALLED",
            "Araç PATH üzerinde bulunamadı; kurulum rehberine bakın.",
        )
    })?;
    let home = user_home()?;
    let measured_at = SystemTime::now();
    let measured_at_unix = measured_at
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state.cli_probe_lock();
        if slot.is_some() {
            return Err(ApiError::new(
                "E_CLI_BUSY",
                "Zaten çalışan bir CLI işlemi var.",
            ));
        }
        *slot = Some(Arc::clone(&cancel));
    }
    let run_cancel = Arc::clone(&cancel);
    let home_for_run = home.clone();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        dim_cli_adapters::measure_residue(spec, &executable, &home_for_run, &run_cancel)
    })
    .await;
    {
        let mut slot = state.cli_probe_lock();
        *slot = None;
    }
    let measurement = match joined {
        Ok(Ok(measurement)) => measurement,
        Ok(Err(err)) => return Err(ApiError::new(err.code(), &err.to_string())),
        Err(_) => {
            return Err(ApiError::new(
                "E_CLI_EXEC_SPAWN",
                "Ölçüm görevi tamamlanamadı.",
            ))
        }
    };

    let view = CliResidueMeasureView {
        adapter_id: measurement.adapter.clone(),
        root: measurement.root.to_string_lossy().to_string(),
        measured_at_unix,
        used_user_home: measurement.used_user_home,
        status: probe_status_label(&measurement.status),
        exit_code: measurement.exit_code,
        duration_ms: measurement.duration_ms,
        args: measurement.args.clone(),
        before: residue_scan_view_of(measurement.scan_before),
        after: residue_scan_view_of(measurement.scan_after),
        created: measurement
            .delta
            .created
            .iter()
            .map(|stamp| CliResidueFileView {
                relative: stamp.relative.clone(),
                bytes: stamp.bytes,
            })
            .collect(),
        modified: measurement
            .delta
            .modified
            .iter()
            .map(|stamp| CliResidueFileView {
                relative: stamp.relative.clone(),
                bytes: stamp.bytes,
            })
            .collect(),
        removed: measurement.delta.removed.clone(),
        created_dirs: measurement.delta.created_dirs.clone(),
        bytes_added: measurement.delta.bytes_added,
        cleanup_hint:
            "Yalnız bu ölçümde oluşan dosyalar silinebilir; önceden var olan dosyalara dokunulmaz.",
    };

    // Denetime yalnız sayılar ve politika yazılır; **dosya adı ve içerik yok**.
    {
        let guard = state.lock();
        if let Some(db) = guard.db.as_ref() {
            let metadata = serde_json::json!({
                "adapterId": view.adapter_id,
                "status": view.status,
                "exitCode": view.exit_code,
                "durationMs": view.duration_ms,
                "homeIsolation": "user_home",
                "createdFiles": view.created.len(),
                "modifiedFiles": view.modified.len(),
                "createdDirs": view.created_dirs.len(),
                "bytesAdded": view.bytes_added,
                "scanTruncated": view.after.truncated,
            });
            db.append_audit("cli_residue", None, "measure", "local-user", &metadata)?;
        }
    }

    Ok(view)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliResidueCleanupInput {
    pub adapter_id: String,
    /// Ölçümün başladığı an (saniye). Bundan eski dosyalar silinmez.
    pub measured_at_unix: u64,
    pub paths: Vec<String>,
}

/// **Yalnız** ölçümde oluştuğu bildirilen dosyaları siler. Dosya bildirilen
/// kapsamda değilse, sembolik bağsa ya da ölçümden önce yazıldıysa atlanır.
#[tauri::command]
pub fn cli_residue_cleanup(
    state: State<'_, AppState>,
    input: CliResidueCleanupInput,
) -> Result<CliResidueCleanupView, ApiError> {
    if adapter_spec(&input.adapter_id).is_none() {
        return Err(ApiError::new(
            "E_CLI_UNKNOWN",
            "Bilinmeyen adaptör kimliği.",
        ));
    }
    let home = user_home()?;
    let since = std::time::UNIX_EPOCH + std::time::Duration::from_secs(input.measured_at_unix);
    let report =
        dim_cli_adapters::cleanup_created_since(&home, &input.adapter_id, since, &input.paths);
    let view = CliResidueCleanupView {
        removed_files: report.removed_files,
        removed_bytes: report.removed_bytes,
        removed_dirs: report.removed_dirs,
        skipped: report
            .skipped
            .into_iter()
            .map(|(path, reason)| CliResidueSkipView { path, reason })
            .collect(),
        errors: report
            .errors
            .into_iter()
            .map(|(path, reason)| CliResidueSkipView { path, reason })
            .collect(),
    };
    {
        let guard = state.lock();
        if let Some(db) = guard.db.as_ref() {
            let metadata = serde_json::json!({
                "adapterId": input.adapter_id,
                "removedFiles": view.removed_files,
                "removedBytes": view.removed_bytes,
                "removedDirs": view.removed_dirs,
                "skipped": view.skipped.len(),
                "errors": view.errors.len(),
            });
            db.append_audit("cli_residue", None, "cleanup", "local-user", &metadata)?;
        }
    }
    Ok(view)
}

/// Çökme sonrası kalan **uygulamaya ait** kum havuzlarını süpürür: yalnız
/// `dim-cli-sandbox-` öneki, `0700`, sahibi biz ve yaş eşiğini geçmiş dizinler.
#[tauri::command]
pub fn cli_stale_sandbox_sweep() -> CliSandboxSweepView {
    let report = dim_cli_adapters::sweep_stale_sandboxes(
        &std::env::temp_dir(),
        dim_cli_adapters::STALE_SANDBOX_AGE,
        SystemTime::now(),
    );
    CliSandboxSweepView {
        considered: report.considered,
        removed: report.removed,
        skipped: report
            .skipped
            .into_iter()
            .map(|(path, reason)| CliResidueSkipView { path, reason })
            .collect(),
        min_age_secs: dim_cli_adapters::STALE_SANDBOX_AGE.as_secs(),
    }
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

/// Karar tam metnini kasaya kaydeder (F12-T05). Metin şifreli veritabanında
/// kalır; arayüze yalnız hash ve karakter sayısı döner.
#[tauri::command]
pub fn precedent_text_add(
    state: State<'_, AppState>,
    precedent_id: String,
    input: NewPrecedentText,
) -> Result<PrecedentVersionRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.add_precedent_text(&precedent_id, &input)?)
}

/// Kasadaki bir belge sürümünün çıkarılmış metninden emsal tam metni üretir
/// (F12-T13). Ayrıştırma yoksa / metin boşsa metin **yazılmaz**
/// (`E_PRECEDENT_SOURCE`); aynı SHA-256 zaten kayıtlıysa yeni sürüm yazılmaz.
#[tauri::command]
pub fn precedent_text_from_document(
    state: State<'_, AppState>,
    precedent_id: String,
    document_version_id: String,
) -> Result<PrecedentDocumentText, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.precedent_text_from_document(&precedent_id, &document_version_id)?)
}

#[tauri::command]
pub fn precedent_version_list(
    state: State<'_, AppState>,
    precedent_id: String,
) -> Result<Vec<PrecedentVersionRow>, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.precedent_versions(&precedent_id)?)
}

/// Metin içi alıntıyı emsalin en yeni tam metninde doğrular. Sonuç
/// `match`/`mismatch`/`not_checked`; hiçbiri "uydurulmuş" hükmü değildir.
#[tauri::command]
pub fn precedent_quote_verify(
    state: State<'_, AppState>,
    precedent_id: String,
    quote: String,
) -> Result<QuoteVerification, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.verify_precedent_quote(&precedent_id, &quote)?)
}

/// Bibliyografik künye (F12-T12): künye, kaynak bağlantısı ve erişim tarihi.
/// Eksik alanlar `missing` listesinde döner; hiçbir alan tahmin edilmez.
#[tauri::command]
pub fn precedent_bibliography(
    state: State<'_, AppState>,
    precedent_id: String,
) -> Result<PrecedentBibliography, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.precedent_bibliography(&precedent_id)?)
}

/// Emsal kütüphanesi araması (F12-T10). Kapsam yalnız kütüphanedir; aktif
/// dosyanın belge segmentleri bu komutta hiç okunmaz.
#[tauri::command]
pub fn precedent_search(
    state: State<'_, AppState>,
    input: PrecedentSearchQuery,
) -> Result<PrecedentSearchPage, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.search_precedents(&input)?)
}

/// Kaynak kullanım koşulu (F12-T11). Kayıt yoksa `unknown` döner; bu "izin
/// var" demek değildir.
#[tauri::command]
pub fn precedent_terms_get(
    state: State<'_, AppState>,
    precedent_id: String,
) -> Result<PrecedentUsageTerms, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.precedent_usage_terms(&precedent_id)?)
}

/// Kullanım koşulunu kaydeder. `allowed` için yazılı dayanak zorunlu; kapı
/// Rust tarafında (`E_TERMS`), arayüzde gizlenen bir kontrol değil.
#[tauri::command]
pub fn precedent_terms_set(
    state: State<'_, AppState>,
    precedent_id: String,
    input: SetUsageTermsInput,
) -> Result<PrecedentUsageTerms, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.set_precedent_usage_terms(&precedent_id, &input)?)
}

/// Dağıtım kapısının gördüğü tablo: toplam, metinli, izinli, izinsiz,
/// bilinmiyor. Arayüz bu sayıları gizlemeden gösterir.
#[tauri::command]
pub fn precedent_corpus_coverage(state: State<'_, AppState>) -> Result<CorpusCoverage, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.precedent_corpus_coverage()?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorpusExportView {
    pub path: String,
    pub bytes: usize,
    pub sha256: String,
    pub exported: usize,
    pub excluded: usize,
    pub content_hash: String,
}

/// Toplu karar korpusu dışa aktarımı (F12-T11). İzin kaydı olmayan metin
/// dosyaya **yazılmaz**; `all` kipinde tek izinsiz kayıt varsa istek reddedilir
/// ve ret denemesi denetime düşer. Ağ isteği yok; dosya kullanıcının seçtiği
/// konuma yazılır.
#[tauri::command]
pub fn precedent_corpus_export(
    state: State<'_, AppState>,
    mode: String,
    dest_path: String,
) -> Result<CorpusExportView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let mode = CorpusMode::parse(&mode)?;
    let export = match db.build_precedent_corpus(mode) {
        Ok(export) => export,
        Err(error) => {
            if matches!(error, dim_storage::StorageError::CorpusNotPermitted(_)) {
                let _ = db.append_audit(
                    "precedent_corpus",
                    None,
                    "export_denied",
                    "local-user",
                    &serde_json::json!({
                        "mode": mode.as_str(),
                        "reason": error.to_string(),
                    }),
                );
            }
            return Err(error.into());
        }
    };
    if export.items.is_empty() {
        return Err(ApiError::new(
            "E_CORPUS_EMPTY",
            "Dışa aktarılacak izinli tam metin yok; dosya yazılmadı.",
        ));
    }
    let payload = serde_json::json!({
        "format": "dim-precedent-corpus",
        "version": 1,
        "createdAt": dim_storage::now_iso(),
        "mode": mode.as_str(),
        "contentHash": export.content_hash,
        "coverage": export.coverage,
        "notes": export.notes,
        "items": export.items,
    });
    let bytes = serde_json::to_vec_pretty(&payload)
        .map_err(|_| ApiError::new("E_EXPORT", "JSON üretilemedi."))?;
    write_export(&dest_path, &bytes)?;
    let sha = hex::encode(Sha256::digest(&bytes));
    db.append_audit(
        "precedent_corpus",
        None,
        "export",
        "local-user",
        &serde_json::json!({
            "mode": mode.as_str(),
            "exported": export.exported,
            "excluded": export.excluded,
            "sha256": sha,
        }),
    )?;
    Ok(CorpusExportView {
        path: dest_path,
        bytes: bytes.len(),
        sha256: sha,
        exported: export.exported,
        excluded: export.excluded,
        content_hash: export.content_hash,
    })
}

/// Tek turda içe alınabilecek dosya sayısı. Daha büyük seçim sessizce
/// kırpılmaz; kullanıcı seçimini böler.
const MAX_BULK_FILES: usize = 200;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkImportDefaults {
    #[serde(default)]
    pub court: String,
    #[serde(default)]
    pub chamber: String,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub legal_issue: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkImportRequest {
    /// Kullanıcının dosya seçiciden seçtiği **mutlak** yollar.
    pub files: Vec<String>,
    /// İsteğe bağlı künye listesi (CSV) yolu.
    #[serde(default)]
    pub manifest_path: Option<String>,
    #[serde(default)]
    pub defaults: BulkImportDefaults,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkImportItemView {
    pub file_name: String,
    /// `created` | `duplicate` | `conflict` | `error`
    pub outcome: String,
    pub precedent_id: Option<String>,
    pub version_id: Option<String>,
    pub body_hash: Option<String>,
    pub char_count: i64,
    /// Aynı künye farklı metin: iki nüsha karşılaştırılmalı.
    pub needs_review: bool,
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkImportReport {
    pub manifest_hash: Option<String>,
    pub manifest_warnings: Vec<String>,
    pub items: Vec<BulkImportItemView>,
    pub created: usize,
    pub duplicates: usize,
    pub conflicts: usize,
    pub failed: usize,
}

fn bulk_error(file_name: &str, code: &str, message: &str) -> BulkImportItemView {
    BulkImportItemView {
        file_name: file_name.to_string(),
        outcome: "error".into(),
        precedent_id: None,
        version_id: None,
        body_hash: None,
        char_count: 0,
        needs_review: false,
        code: Some(code.to_string()),
        message: Some(message.to_string()),
    }
}

/// **Toplu içe alma** (F12-T06): seçilen dosyaların metni belge worker'ında
/// çıkarılır, künye listesi/varsayılanlarla eşlenir ve kasadaki emsal
/// kütüphanesine yazılır. Ağ isteği yoktur; künye **tahmin edilmez**.
/// Bir dosyanın hatası diğerlerini durdurmaz, sonuçta tek tek raporlanır.
#[tauri::command]
pub fn precedent_bulk_import(
    state: State<'_, AppState>,
    input: BulkImportRequest,
) -> Result<BulkImportReport, ApiError> {
    if input.files.is_empty() {
        return Err(ApiError::new(
            "E_BULK_EMPTY",
            "İçe alınacak dosya seçilmedi.",
        ));
    }
    if input.files.len() > MAX_BULK_FILES {
        return Err(ApiError::new(
            "E_BULK_TOO_MANY",
            "Tek turda en çok 200 dosya içe alınır; seçimi bölün.",
        ));
    }

    // Künye listesi kasadan bağımsız okunur; hatalı liste hiçbir dosyaya yazmaz.
    let mut manifest_hash = None;
    let mut manifest_warnings = Vec::new();
    let mut manifest: Option<dim_storage::Manifest> = None;
    if let Some(path) = input
        .manifest_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let path = Path::new(path);
        let meta = std::fs::metadata(path)
            .map_err(|_| ApiError::new("E_MANIFEST", "Künye listesi okunamadı; yol geçerli mi?"))?;
        if !meta.is_file() {
            return Err(ApiError::new(
                "E_MANIFEST",
                "Künye listesi bir dosya olmalı.",
            ));
        }
        if meta.len() > dim_storage::MANIFEST_MAX_BYTES as u64 {
            return Err(ApiError::new(
                "E_MANIFEST",
                "Künye listesi 4 MiB sınırını aşıyor.",
            ));
        }
        let text = std::fs::read_to_string(path)
            .map_err(|_| ApiError::new("E_MANIFEST", "Künye listesi UTF-8 okunamadı."))?;
        let parsed = dim_storage::parse_manifest(&text)
            .map_err(|err| ApiError::new(err.code(), &err.to_string()))?;
        manifest_hash = Some(parsed.hash.clone());
        manifest_warnings = parsed.warnings.clone();
        manifest = Some(parsed);
    }

    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let mut worker = spawn_worker()?;

    let mut items = Vec::with_capacity(input.files.len());
    for raw in &input.files {
        let path = Path::new(raw);
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("belge")
            .to_string();
        let outcome = import_one_precedent(
            &file_name,
            path,
            &input.defaults,
            manifest.as_ref(),
            &mut worker,
            db,
        );
        items.push(outcome);
    }
    worker.shutdown();

    let mut report = BulkImportReport {
        manifest_hash,
        manifest_warnings,
        items,
        created: 0,
        duplicates: 0,
        conflicts: 0,
        failed: 0,
    };
    for item in &report.items {
        match item.outcome.as_str() {
            "created" => report.created += 1,
            "duplicate" => report.duplicates += 1,
            "conflict" => report.conflicts += 1,
            _ => report.failed += 1,
        }
    }
    Ok(report)
}

fn import_one_precedent(
    file_name: &str,
    path: &Path,
    defaults: &BulkImportDefaults,
    manifest: Option<&dim_storage::Manifest>,
    worker: &mut WorkerClient,
    db: &Database,
) -> BulkImportItemView {
    if !path.is_absolute() {
        return bulk_error(
            file_name,
            "E_BULK_PATH",
            "Dosya yolu mutlak olmalı; göreli yol kabul edilmez.",
        );
    }
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(_) => {
            return bulk_error(
                file_name,
                "E_IO",
                "Dosya okunamadı (taşınmış veya izin yok).",
            )
        }
    };
    if !meta.is_file() {
        return bulk_error(
            file_name,
            "E_IMPORT_UNSUPPORTED",
            "Yalnız dosyalar içe alınır.",
        );
    }
    if meta.len() > dim_storage::MAX_DOCUMENT_BYTES {
        return bulk_error(
            file_name,
            "E_IMPORT_TOO_LARGE",
            "Dosya 250 MiB sınırını aşıyor.",
        );
    }
    let data = match std::fs::read(path) {
        Ok(data) => data,
        Err(_) => return bulk_error(file_name, "E_IO", "Dosya içeriği okunamadı."),
    };
    let Some(kind) = dim_ingest::detect(&data, file_name) else {
        return bulk_error(
            file_name,
            "E_IMPORT_UNSUPPORTED",
            "Desteklenmeyen dosya türü; PDF, DOCX, UDF veya düz metin kullanın.",
        );
    };
    if matches!(
        kind,
        dim_ingest::FileKind::Png | dim_ingest::FileKind::Jpeg | dim_ingest::FileKind::Tiff
    ) {
        return bulk_error(
            file_name,
            "E_BULK_IMAGE",
            "Görüntü dosyasından karar metni çıkarılmaz; OCR'li PDF kullanın.",
        );
    }
    let is_text = kind == dim_ingest::FileKind::Text;
    let extraction = match worker.extract_text(
        &data,
        kind.mime(),
        false,
        "tur",
        is_text.then_some("auto"),
        None,
    ) {
        Ok(extraction) => extraction,
        Err(err) => {
            return bulk_error(file_name, err.code(), "Metin çıkarılamadı.");
        }
    };
    let text = extraction
        .pages
        .iter()
        .map(|page| page.text.trim())
        .filter(|page| !page.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if text.trim().is_empty() {
        return bulk_error(
            file_name,
            "E_BULK_EMPTY_TEXT",
            "Dosyadan metin çıkmadı (taranmış olabilir); alıntı doğrulanamayacağı için yazılmadı.",
        );
    }

    let row = manifest.and_then(|value| value.row_for(file_name));
    let court = row
        .and_then(|r| r.court.clone())
        .or_else(|| Some(defaults.court.trim().to_string()).filter(|v| !v.is_empty()));
    let Some(court) = court else {
        return bulk_error(
            file_name,
            "E_BULK_COURT",
            "Mahkeme bilinmiyor: varsayılan mahkeme girin veya künye listesinde belirtin.",
        );
    };
    let title = row.and_then(|r| r.title.clone()).or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
    });
    let tags = match row.map(|r| r.user_tags.clone()) {
        Some(tags) if !tags.is_empty() => tags,
        _ => defaults.tags.clone(),
    };
    let citation = NewPrecedent {
        court,
        chamber: row
            .and_then(|r| r.chamber.clone())
            .or_else(|| Some(defaults.chamber.trim().to_string()).filter(|v| !v.is_empty())),
        docket_no: row.and_then(|r| r.docket_no.clone()),
        decision_no: row.and_then(|r| r.decision_no.clone()),
        application_no: None,
        decision_date: row.and_then(|r| r.decision_date.clone()),
        title,
        source_url: row
            .and_then(|r| r.source_url.clone())
            .or_else(|| Some(defaults.source_url.trim().to_string()).filter(|v| !v.is_empty())),
        user_tags: tags,
        legal_issue: row
            .and_then(|r| r.legal_issue.clone())
            .or_else(|| Some(defaults.legal_issue.trim().to_string()).filter(|v| !v.is_empty())),
    };
    let input = dim_storage::PrecedentCopyInput {
        citation,
        text,
        source_kind: Some("file".into()),
        document_version_id: None,
    };
    match db.import_precedent_copy(&input) {
        Ok(result) => BulkImportItemView {
            file_name: file_name.to_string(),
            outcome: result.outcome,
            precedent_id: Some(result.precedent_id),
            version_id: result.version_id,
            body_hash: Some(result.body_hash),
            char_count: result.char_count,
            needs_review: result.needs_review,
            code: None,
            message: None,
        },
        Err(err) => bulk_error(file_name, err.code(), &err.to_string()),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorCatalogView {
    pub catalog_hash: String,
    pub connectors: Vec<dim_storage::ConnectorInfo>,
    pub note: String,
}

/// Dış bağlayıcı **sicili** (F12-T06). Kaynakların koşulları belgelenmiştir;
/// hiçbiri etkin değildir ve ürün bu kaynaklara ağ isteği göndermez.
#[tauri::command]
pub fn precedent_connector_list() -> Result<ConnectorCatalogView, ApiError> {
    let catalog_hash = dim_storage::connector_catalog_hash()
        .map_err(|err| ApiError::new(err.code(), &err.to_string()))?;
    Ok(ConnectorCatalogView {
        catalog_hash,
        connectors: dim_storage::connector_catalog(),
        note: "Sicil yalnız belgelenmiş kaynakları listeler. Hiçbiri etkin değil: ürün bu\
               adreslere istek göndermez; içe alma kullanıcının kendi indirdiği dosyayla yapılır."
            .into(),
    })
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
    /// Bağımsız doğrulama sayıları (F01-T05): arayüz "doğrulandı" iddiasını
    /// bunlarla birlikte gösterir.
    pub streams_scanned: u32,
    pub streams_binary: u32,
    pub streams_unreadable: u32,
    pub annotations_removed: u32,
    pub offpage_spans_removed: u32,
    pub scripts_removed: u32,
    pub warnings: Vec<String>,
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
        streams_scanned: outcome.streams_scanned,
        streams_binary: outcome.streams_binary,
        streams_unreadable: outcome.streams_unreadable,
        annotations_removed: outcome.annotations_removed,
        offpage_spans_removed: outcome.offpage_spans_removed,
        scripts_removed: outcome.scripts_removed,
        warnings: outcome.warnings,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportView {
    pub path: String,
    pub bytes: usize,
    pub sha256: String,
    pub record: ExportRow,
    /// Çıktının içine gömülen kapsam bloğu (F11-T06). Arayüz "ne yazıldı"
    /// iddiasını buradan gösterir; ölçüm kasadan gelir.
    pub coverage: ExportCoverage,
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

fn case_payload(
    db: &Database,
    case_id: &str,
    coverage: &ExportCoverage,
) -> Result<serde_json::Value, ApiError> {
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
        // Kapsam her çıktıda taşınır: "incelenmedi ≠ temiz".
        "coverage": coverage,
        "documents": documents,
        "events": events,
        "claims": claims,
        "evidenceLinks": evidence,
        "notes": notes,
        "tasks": tasks,
        "anchors": anchors,
    }))
}

/// Dosyanın o anki kapsam raporu (dışa aktarmadan önce arayüzde göstermek için).
#[tauri::command]
pub fn case_coverage(
    state: State<'_, AppState>,
    case_id: String,
) -> Result<ExportCoverage, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(ExportCoverage::from_report(&db.coverage_report(&case_id)?))
}

#[tauri::command]
pub fn export_json(
    state: State<'_, AppState>,
    case_id: String,
    dest_path: String,
) -> Result<ExportView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let coverage = ExportCoverage::from_report(&db.coverage_report(&case_id)?);
    let payload = case_payload(db, &case_id, &coverage)?;
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
        coverage,
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
    // Kapsam bloğu tablo satırlarına karışmasın diye `#` yorum satırı olarak
    // başa yazılır; sayılar kasadan gelir.
    let coverage = ExportCoverage::from_report(&db.coverage_report(&case_id)?);
    let mut out = coverage.csv_comment_block();
    out.push_str("kayit,tarih,tur,baslik,durum\n");
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
        coverage,
    })
}

/// DOCX/PDF raporuna eklenen kapsam bölümü: sayım tablosu + uyarı/sınır
/// satırları. Bölüm **ilk** sırada yer alır ki okuyucu içeriğe geçmeden
/// kapsamın sınırını görsün.
fn coverage_section(coverage: &ExportCoverage) -> serde_json::Value {
    let mut rows: Vec<Vec<String>> = vec![vec!["Ölçü".to_string(), "Değer".to_string()]];
    for (label, value) in coverage.rows() {
        rows.push(vec![label.to_string(), value.to_string()]);
    }
    serde_json::json!({
        "heading": "İncelenen kapsam",
        "rows": rows,
        "lines": coverage.lines(),
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
    let coverage = ExportCoverage::from_report(&db.coverage_report(&case_id)?);

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
            "Kapsam": coverage.headline(),
        },
        "sections": [
            coverage_section(&coverage),
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
        coverage,
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
    let coverage = ExportCoverage::from_report(&db.coverage_report(&case_id)?);

    let payload = serde_json::json!({
        "title": format!("{} — dosya raporu", case.title),
        "meta": {
            "Dosya": case.title,
            "Esas/referans": case.reference_code.clone().unwrap_or_default(),
            "Tür": case.case_kind,
            "Hassasiyet": case.sensitivity,
            "Oluşturma": case.created_at,
            "Kapsam": coverage.headline(),
        },
        "sections": [
            coverage_section(&coverage),
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
        coverage,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageView {
    pub path: String,
    pub files: usize,
    pub sha256: String,
    pub record: ExportRow,
    /// Parola verildiyse üretilen şifreli `.dimpak` dosyası.
    pub sealed_path: Option<String>,
    /// Pakete gömülen kapsam bloğu (F11-T06).
    pub coverage: ExportCoverage,
}

/// Devir paketi: dosya metadatası + rapor iskeleti + manifest. Belge baytları
/// pakete otomatik konmaz; kullanıcı maskeli kopyaları ayrıca ekler.
#[tauri::command]
pub fn export_package(
    state: State<'_, AppState>,
    case_id: String,
    dest_dir: String,
    password: Option<String>,
) -> Result<PackageView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    if dest_dir.trim().is_empty() {
        return Err(ApiError::new("E_PATH", "Paket konumu boş olamaz."));
    }
    let coverage = ExportCoverage::from_report(&db.coverage_report(&case_id)?);
    let payload = case_payload(db, &case_id, &coverage)?;
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
    // Kapsam manifesti: paketin hangi belge/sayfa kapsamıyla üretildiğini yazar
    // (okunabilir markdown). "İncelenmedi ≠ temiz" cümlesi paketin içinde kalır.
    let coverage_md = coverage.markdown(payload["case"]["title"].as_str().unwrap_or("Dosya"));
    std::fs::write(dir.join("KAPSAM.md"), coverage_md.as_bytes())?;
    let readme = "Dosya İnceleme Masası — devir paketi\n\nBu klasör dosya metadatasını (case.json), inceleme kapsamını (KAPSAM.md)\nve manifest'i taşır. Belge baytları kasa dışına otomatik çıkarılmaz;\npaylaşılacak belgeler için Maskeleme ekranından maskeli kopya üretip bu\nklasöre ekleyin.\n\nKAPSAM.md zorunlu okumadır: hangi belge ve sayfaların gerçekten\nayrıştırıldığını, kaç sayfanın OCR ile okunduğunu ve kaç belgenin hiç\nincelenmediğini yazar. Kapsam dışı içerik \"temiz\" sayılmaz.\n\nPaket açık metindir; şifreli taşıma F16 kapsamındadır.\n";
    std::fs::write(dir.join("README.txt"), readme)?;

    let files = [
        ("case.json", sha256_file(&dir.join("case.json"))?),
        ("KAPSAM.md", sha256_file(&dir.join("KAPSAM.md"))?),
        ("README.txt", sha256_file(&dir.join("README.txt"))?),
    ];
    let manifest = serde_json::json!({
        "format": "dim-handoff",
        "version": 1,
        "createdAt": dim_storage::now_iso(),
        "coverage": coverage,
        "files": files.iter().map(|(name, hash)| serde_json::json!({"name": name, "sha256": hash})).collect::<Vec<_>>(),
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|_| ApiError::new("E_EXPORT", "Manifest üretilemedi."))?;
    std::fs::write(dir.join("manifest.json"), &manifest_bytes)?;
    let sha = hex::encode(Sha256::digest(&manifest_bytes));
    let record = db.record_export(&case_id, "package", &[], "user_selected_path", &sha)?;

    // Parola verildiyse klasör tek şifreli dosyaya paketlenir; alıcıya kasa
    // anahtarı verilmez, yalnız paket parolası söylenir.
    let mut sealed_path = None;
    if let Some(secret) = password.filter(|p| p.chars().count() >= 10) {
        let archive = dir.with_extension("zip");
        zip_dir(&dir, &archive)?;
        let zip_bytes = std::fs::read(&archive)?;
        let sealed = dim_vault::package::seal(&zip_bytes, &secret)?;
        let target = dir.with_extension("dimpak");
        std::fs::write(&target, &sealed)?;
        let _ = std::fs::remove_file(&archive);
        let _ = std::fs::remove_dir_all(&dir);
        sealed_path = Some(target.to_string_lossy().to_string());
    }

    Ok(PackageView {
        path: dir.to_string_lossy().to_string(),
        files: files.len(),
        sha256: sha,
        record,
        sealed_path,
        coverage,
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

/// Adayı **yazmadan** doğrular (şema + alıntı eşleştirmesi); denetim kaydı
/// oluşturmaz. Kalıcı kayıt `ai_candidate_record` ile yapılır.
#[tauri::command]
pub fn ai_candidate_validate(
    state: State<'_, AppState>,
    run_id: String,
    candidate_type: String,
    payload: String,
) -> Result<CandidateReport, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.validate_ai_candidate(&run_id, &candidate_type, &payload)?)
}

/// Adayı doğrular ve **hesaplanan** durumla kaydeder. Çağıran `valid`
/// yazdıramaz.
#[tauri::command]
pub fn ai_candidate_record(
    state: State<'_, AppState>,
    run_id: String,
    candidate_type: String,
    payload: String,
) -> Result<AiCandidateRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.record_ai_candidate(&run_id, &candidate_type, &payload)?)
}

/// Belge yeniden ayrıştırıldıysa adayı güncel metne göre yeniden doğrular.
#[tauri::command]
pub fn ai_candidate_revalidate(
    state: State<'_, AppState>,
    candidate_id: String,
) -> Result<AiCandidateRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.revalidate_ai_candidate(&candidate_id)?)
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultGcView {
    pub dry_run: bool,
    pub candidates: Vec<dim_storage::GcBlobRow>,
    pub deleted: Vec<dim_storage::GcBlobRow>,
    pub freed_bytes: i64,
    pub missing_files: Vec<String>,
}

/// Referanssız blob kayıtlarını (isteğe bağlı olarak dosyalarıyla) temizler.
/// Önce kuru çalıştırma yapılması önerilir; arayüz iki adımlı akış sunar.
#[tauri::command]
pub fn vault_gc(state: State<'_, AppState>, dry_run: bool) -> Result<VaultGcView, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let vault_path = guard.path.as_ref().ok_or_else(ApiError::vault_locked)?;

    let report: GcReport = db.garbage_collect_blobs(dry_run)?;
    let mut missing_files = Vec::new();
    if !dry_run {
        let blob_dir = vault_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("blobs");
        for blob in &report.deleted {
            let path = blob_dir.join(format!("{}.blb", blob.storage_id));
            if path.exists() {
                if let Err(error) = std::fs::remove_file(&path) {
                    missing_files.push(format!("{}: {error}", blob.storage_id));
                }
            } else {
                missing_files.push(format!("{}: dosya yok", blob.storage_id));
            }
        }
    }

    Ok(VaultGcView {
        dry_run: report.dry_run,
        candidates: report.candidates,
        deleted: report.deleted,
        freed_bytes: report.freed_bytes,
        missing_files,
    })
}

/// Parolayı değiştirir. Yalnız parola yuvası yeniden sarılır; ana anahtar ve
/// veritabanı anahtarı değişmez, açık oturum ve kurtarma anahtarı geçerli kalır.
#[tauri::command]
pub fn vault_change_password(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> Result<(), ApiError> {
    let guard = state.lock();
    let path = guard.path.as_ref().ok_or_else(ApiError::vault_locked)?;
    VaultFile::change_password(path, &old_password, &new_password)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactRegionInput {
    pub page_index: u32,
    pub rect: [f64; 4],
}

/// Kullanıcının sayfa üzerinde seçtiği bölgeleri gerçekten kaldırır ve yeni
/// sürüm olarak kaydeder. Doğrulama geçmezse hiçbir çıktı yazılmaz.
#[tauri::command]
pub fn document_redact_regions(
    state: State<'_, AppState>,
    version_id: String,
    regions: Vec<RedactRegionInput>,
) -> Result<RedactionView, ApiError> {
    if regions.is_empty() {
        return Err(ApiError::new("E_REDACT_REGIONS", "En az bir bölge seçin."));
    }
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

    let pairs: Vec<(u32, [f64; 4])> = regions.iter().map(|r| (r.page_index, r.rect)).collect();
    let mut worker = spawn_worker()?;
    let outcome = worker.redact_regions(&data, &pairs)?;
    worker.shutdown();

    if !outcome.remaining.is_empty() {
        return Err(ApiError::new(
            "E_REDACT_VERIFY",
            "Seçilen bölgede metin kaldı; dosya kaydedilmedi.",
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
        &format!("{} (bölge maskeli)", source_doc.display_name),
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
        &dim_storage::NewRedactionPlan {
            document_version_id: version_id.clone(),
            parse_run_id: None,
            render_hash: version.original_sha256.clone(),
            output_profile: "share".to_string(),
        },
    )?;
    let inputs: Vec<dim_storage::RedactionRegionInput> = outcome
        .hits
        .iter()
        .map(|hit| dim_storage::RedactionRegionInput {
            page_index: i64::from(hit.page),
            rect: hit.rect,
            term_hash: hex::encode(Sha256::digest(
                format!("region:{}:{:?}", hit.page, hit.rect).as_bytes(),
            )),
        })
        .collect();
    db.add_redaction_regions(&plan.id, &inputs)?;

    Ok(RedactionView {
        document: new_doc,
        version: new_version,
        plan,
        hits: outcome.hits.len(),
        remaining: outcome.remaining,
        ocr_verified: outcome.ocr_verified,
        engines: outcome.engines,
        streams_scanned: outcome.streams_scanned,
        streams_binary: outcome.streams_binary,
        streams_unreadable: outcome.streams_unreadable,
        annotations_removed: outcome.annotations_removed,
        offpage_spans_removed: outcome.offpage_spans_removed,
        scripts_removed: outcome.scripts_removed,
        warnings: outcome.warnings,
    })
}

#[tauri::command]
pub fn citation_meaning_review(
    state: State<'_, AppState>,
    check_id: String,
    review: String,
    note: String,
) -> Result<CitationCheckRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.review_citation_meaning(&check_id, &review, &note)?)
}

#[tauri::command]
pub fn citation_context(
    state: State<'_, AppState>,
    check_id: String,
    radius: Option<u32>,
) -> Result<CitationContext, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let radius = radius.unwrap_or(120).clamp(10, 500);
    Ok(db.citation_context(&check_id, radius as usize)?)
}

#[tauri::command]
pub fn provider_api_profile_save(
    state: State<'_, AppState>,
    endpoint: String,
    model: String,
) -> Result<ProviderProfileRow, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.register_api_profile(&endpoint, &model)?)
}

#[tauri::command]
pub fn ai_consent_preview(
    state: State<'_, AppState>,
    case_id: String,
    document_ids: Vec<String>,
) -> Result<ConsentPreview, ApiError> {
    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    Ok(db.consent_preview(&case_id, &document_ids)?)
}

/// Onaylı aktarımı **gerçekten** yürütür: context segmentleri toplar, API
/// sağlayıcısına gönderir, dönen adayı doğrulayıp kaydeder. Onay yoksa veya
/// profil tanımlı değilse çağrı hiç kurulmaz.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn ai_run_execute(
    state: State<'_, AppState>,
    case_id: String,
    provider_id: String,
    consent_id: String,
    task_type: String,
    document_ids: Vec<String>,
) -> Result<AiCandidateRow, ApiError> {
    use sha2::{Digest, Sha256};

    let guard = state.lock();
    let db = guard.db.as_ref().ok_or_else(ApiError::vault_locked)?;
    let (endpoint, model) = db
        .api_profile_endpoint()?
        .ok_or_else(|| ApiError::new("E_AI_OFF", "API profili tanımlı değil."))?;
    let segments = db.context_segments(&case_id, &document_ids)?;
    if segments.is_empty() {
        return Err(ApiError::new(
            "E_AI_UNSUPPORTED",
            "Gönderilecek ayrıştırılmış metin yok.",
        ));
    }

    let mut hasher = Sha256::new();
    for (id, text) in &segments {
        hasher.update(id.as_bytes());
        hasher.update(text.as_bytes());
    }
    let manifest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    let run = db.start_ai_run(
        &case_id,
        &NewAiRun {
            task_type: task_type.clone(),
            provider_id,
            model_snapshot: model.clone(),
            input_manifest_hash: manifest,
            prompt_version: "1".to_string(),
            output_schema_version: "candidate.v1".to_string(),
            consent_id,
        },
    )?;

    let request = AiRequest {
        case_id,
        task_type: task_type.clone(),
        prompt_version: "1".to_string(),
        output_schema_version: "candidate.v1".to_string(),
        segments: segments
            .iter()
            .map(|(id, text)| ContextSegment {
                segment_id: id.clone(),
                text: text.clone(),
                source_ref_json: None,
            })
            .collect(),
    };
    let bytes = request.outbound_bytes();
    let mut provider = ApiProvider::new(&endpoint, &model)?;
    let candidate = match provider.propose(&request) {
        Ok(candidate) => candidate,
        Err(error) => {
            let report = serde_json::json!({"error": error.code()}).to_string();
            db.finish_ai_run(&run.id, "failed", &report)?;
            return Err(error.into());
        }
    };
    let payload = serde_json::to_string(&candidate.payload).unwrap_or_else(|_| "{}".to_string());
    let row = db.record_ai_candidate(&run.id, &task_type, &payload)?;
    let report = serde_json::json!({"segments": segments.len(), "bytes": bytes}).to_string();
    db.finish_ai_run(&run.id, "succeeded", &report)?;
    Ok(row)
}
