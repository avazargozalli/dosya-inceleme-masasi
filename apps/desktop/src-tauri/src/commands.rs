use std::path::{Path, PathBuf};

use dim_storage::{Case, CasePatch, NewCase};
use dim_vault::{UnlockedVault, VaultFile};
use serde::Serialize;
use tauri::State;
use zeroize::Zeroizing;

use crate::error::ApiError;
use crate::state::{open_database, vault_id_from_master, AppState};

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
pub fn vault_create(state: State<'_, AppState>, path: String, password: String) -> Result<CreateVaultResult, ApiError> {
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
pub fn vault_open(state: State<'_, AppState>, path: String, password: String) -> Result<VaultInfo, ApiError> {
    let password = Zeroizing::new(password);
    let vault_path = resolve_vault_path(&path)?;
    let unlocked = VaultFile::open(&vault_path, &password)?;
    remember(state, vault_path, unlocked)
}

#[tauri::command]
pub fn vault_open_with_recovery(state: State<'_, AppState>, path: String, recovery_key: String) -> Result<VaultInfo, ApiError> {
    let recovery_key = Zeroizing::new(recovery_key);
    let vault_path = resolve_vault_path(&path)?;
    let unlocked = VaultFile::open_with_recovery(&vault_path, &recovery_key)?;
    remember(state, vault_path, unlocked)
}

fn remember(state: State<'_, AppState>, path: PathBuf, unlocked: UnlockedVault) -> Result<VaultInfo, ApiError> {
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
pub fn case_list(state: State<'_, AppState>, include_archived: Option<bool>) -> Result<Vec<Case>, ApiError> {
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
pub fn case_update(state: State<'_, AppState>, id: String, patch: CasePatch, row_version: i64) -> Result<Case, ApiError> {
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
    SystemInfo { app_version: env!("CARGO_PKG_VERSION"), protocol_version: 1, platform: std::env::consts::OS }
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
