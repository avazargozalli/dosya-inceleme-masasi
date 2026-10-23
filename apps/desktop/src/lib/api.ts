import { invoke } from "@tauri-apps/api/core";

export type ApiError = { code: string; message: string; retryable: boolean };

export type CreateVaultResult = { vaultPath: string; recoveryKey: string; vaultId: string };

export type VaultInfo = {
  vaultPath: string;
  vaultId: string;
  version: number;
  createdAtUnix: number;
  locked: boolean;
  schemaVersion: number;
};

export type VaultStatus = {
  unlocked: boolean;
  vaultPath: string | null;
  vaultId: string | null;
  schemaVersion: number | null;
};

export type CaseRow = {
  id: string;
  title: string;
  caseKind: "review" | "reference_archive";
  referenceCode: string | null;
  jurisdictionLabel: string | null;
  practiceArea: string | null;
  status: "active" | "archived" | "deleted";
  sensitivity: "normal" | "high" | "client_confidential";
  defaultAiPolicy: "off" | "local_only" | "cli_allowed" | "api_allowed";
  archivedAt: string | null;
  retentionHold: boolean;
  createdAt: string;
  updatedAt: string;
  rowVersion: number;
};

export type NewCaseInput = {
  title: string;
  caseKind: "review" | "reference_archive";
  referenceCode: string | null;
  jurisdictionLabel: string | null;
  practiceArea: string | null;
  sensitivity: CaseRow["sensitivity"];
  defaultAiPolicy: CaseRow["defaultAiPolicy"];
};

export type DocumentRow = {
  id: string;
  caseId: string;
  displayName: string;
  documentType: string;
  status: string;
  currentVersionId: string | null;
  declaredDocumentDate: string | null;
  createdAt: string;
  updatedAt: string;
  rowVersion: number;
};

export type DocumentVersionRow = {
  id: string;
  documentId: string;
  caseId: string;
  versionNo: number;
  blobId: string;
  originalSha256: string;
  byteSize: number;
  detectedMime: string;
  importedAt: string;
};

export type ImportOutcome =
  | { kind: "imported"; document: DocumentRow; version: DocumentVersionRow }
  | { kind: "duplicate"; documentId: string; versionId: string; displayName: string };

export type CasePatchInput = {
  title?: string;
  referenceCode?: string;
  jurisdictionLabel?: string;
  practiceArea?: string;
  sensitivity?: CaseRow["sensitivity"];
  defaultAiPolicy?: CaseRow["defaultAiPolicy"];
  retentionHold?: boolean;
};

/** IPC hatalarını tek biçime indirger; ham hata metni arayüze sızmaz. */
export function toApiError(error: unknown): ApiError {
  if (typeof error === "object" && error !== null && "code" in error && "message" in error) {
    const e = error as { code: unknown; message: unknown; retryable?: unknown };
    return {
      code: String(e.code),
      message: String(e.message),
      retryable: Boolean(e.retryable),
    };
  }
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

  caseList: (includeArchived = false) => invoke<CaseRow[]>("case_list", { includeArchived }),
  caseCreate: (input: NewCaseInput) => invoke<CaseRow>("case_create", { input }),
  caseGet: (id: string) => invoke<CaseRow | null>("case_get", { id }),
  caseUpdate: (id: string, patch: CasePatchInput, rowVersion: number) =>
    invoke<CaseRow>("case_update", { id, patch, rowVersion }),
  caseArchive: (id: string) => invoke<CaseRow>("case_archive", { id }),
  caseRestore: (id: string) => invoke<CaseRow>("case_restore", { id }),
  caseDelete: (id: string) => invoke<void>("case_delete", { id }),

  documentList: (caseId: string) => invoke<DocumentRow[]>("document_list", { caseId }),
  documentImport: (caseId: string, path: string) =>
    invoke<ImportOutcome>("document_import", { caseId, path }),
};
