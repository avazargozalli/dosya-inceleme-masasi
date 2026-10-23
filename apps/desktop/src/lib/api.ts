import { invoke } from "@tauri-apps/api/core";

export type ApiError = { code: string; message: string; retryable: boolean };

export type CreateVaultResult = {
  vaultPath: string;
  recoveryKey: string;
  vaultId: string;
};

export type VaultInfo = {
  vaultPath: string;
  vaultId: string;
  version: number;
  createdAt: string;
  locked: boolean;
};

export type VaultStatus = { unlocked: boolean; vaultPath: string | null; vaultId: string | null };

export function isApiError(value: unknown): value is ApiError {
  return typeof value === "object" && value !== null && "code" in value && "message" in value;
}

/** IPC hatalarını tek biçime indirger; ham hata metni arayüze sızmaz. */
export function toApiError(error: unknown): ApiError {
  if (isApiError(error)) return error;
  return { code: "E_UI_UNKNOWN", message: "Beklenmeyen bir hata oluştu.", retryable: false };
}

export const api = {
  createVault: (path: string, password: string) =>
    invoke<CreateVaultResult>("vault_create", { path, password }),
  openVault: (path: string, password: string) => invoke<VaultInfo>("vault_open", { path, password }),
  openWithRecovery: (path: string, recoveryKey: string) =>
    invoke<VaultInfo>("vault_open_with_recovery", { path, recoveryKey }),
  lockVault: () => invoke<void>("vault_lock"),
  status: () => invoke<VaultStatus>("vault_status"),
};
