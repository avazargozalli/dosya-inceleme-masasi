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

export type PageRow = {
  id: string;
  segmentId: string | null;
  pageIndex: number;
  text: string;
  textOrigin: string;
  qualityFlags: string;
  width: number | null;
  height: number | null;
};

export type DocumentPages = {
  parseRun: {
    id: string;
    documentVersionId: string;
    status: string;
    engine: string;
    engineVersion: string;
    pagesProcessed: number;
    warnings: string;
    createdAt: string;
  };
  pages: PageRow[];
};

export type PrecedentRow = {
  id: string;
  court: string;
  chamber: string | null;
  docketNo: string | null;
  decisionNo: string | null;
  applicationNo: string | null;
  decisionDate: string | null;
  title: string | null;
  sourceUrl: string | null;
  userTags: string[];
  legalIssue: string | null;
  completeness: "complete" | "incomplete";
  createdAt: string;
};

export type PrecedentNoteRow = {
  id: string;
  precedentId: string;
  caseIdOptional: string | null;
  usefulPassageAnchor: string | null;
  similarityNote: string;
  distinguishingNote: string;
  userConclusion: string;
  createdAt: string;
};

export type CitationCheckRow = {
  id: string;
  inputDocumentVersionId: string;
  extractedCitation: string;
  candidatePrecedentId: string | null;
  identityMatch: "match" | "mismatch" | "unknown";
  quoteMatch: "match" | "mismatch" | "not_checked" | "unknown";
  meaningReview: string;
  reasonCodes: string[];
  createdAt: string;
};

export type AdapterView = {
  id: string;
  displayName: string;
  command: string;
  state: "installed" | "not_installed" | "unverified";
  version: string | null;
  authMode: "subscription" | "api_key" | "interactive_only";
  batchForbidden: boolean;
  allowedArgs: string[];
};

export type ClaimRow = {
  id: string;
  caseId: string;
  claimantActorId: string | null;
  claimText: string;
  claimKind: string;
  status: string;
  sourceAnchorId: string | null;
  createdAt: string;
  updatedAt: string;
  rowVersion: number;
};

export type EvidenceLinkRow = {
  id: string;
  caseId: string;
  claimId: string;
  sourceAnchorId: string;
  relation: string;
  rationale: string;
  reviewState: string;
  createdAt: string;
};

export type ReviewItemRow = {
  id: string;
  caseId: string;
  itemKind: string;
  payloadSchemaVersion: number;
  payload: unknown;
  sourceAnchorIds: string[];
  aiRunId: string | null;
  state: "pending" | "accepted" | "rejected" | "edited" | "needs_more_evidence";
  targetEntityId: string | null;
  createdAt: string;
  updatedAt: string;
  rowVersion: number;
};

export type ReviewDecisionRow = {
  id: string;
  reviewItemId: string;
  decision: string;
  editedPayload: unknown;
  reviewerLabel: string;
  rationale: string;
  decidedAt: string;
};

export type BackupResult = {
  path: string;
  fileCount: number;
  totalBytes: number;
  manifestHash: string;
};

export type RestoreResult = { vaultPath: string; fileCount: number };

export type AnchorRow = {
  id: string;
  caseId: string;
  documentVersionId: string;
  parseRunId: string;
  pageId: string | null;
  segmentId: string;
  startOffset: number;
  endOffset: number;
  exactQuote: string;
  prefix: string;
  suffix: string;
  quoteHash: string;
  anchorStatus: string;
  createdAt: string;
};

export type NoteRow = {
  id: string;
  caseId: string;
  title: string;
  body: string;
  authorKind: string;
  visibilityLabel: string;
  sourceAnchorIds: string[];
  approvedAt: string | null;
  createdAt: string;
  updatedAt: string;
  rowVersion: number;
};

export type TaskRow = {
  id: string;
  caseId: string;
  title: string;
  status: string;
  userDueDate: string | null;
  dateTimezone: string | null;
  dateOrigin: string;
  assigneeLabel: string | null;
  reminderPolicy: string;
  createdAt: string;
  updatedAt: string;
  rowVersion: number;
};

export type NewTaskInput = {
  title: string;
  userDueDate: string | null;
  dateTimezone: string | null;
  assigneeLabel: string | null;
  reminderPolicy: string;
};

export type EventRow = {
  id: string;
  caseId: string;
  eventKind: string;
  dateValue: string | null;
  datePrecision: string;
  dateRole: string;
  dateRaw: string | null;
  sortableDate: string | null;
  title: string;
  description: string;
  assertionKind: string;
  status: string;
  createdAt: string;
  updatedAt: string;
  rowVersion: number;
};

export type NewEventInput = {
  eventKind: string;
  dateValue: string | null;
  datePrecision: string;
  dateRole: string;
  dateRaw: string | null;
  title: string;
  description: string;
  assertionKind: string;
};

export type SearchHit = {
  segmentId: string;
  pageId: string | null;
  rawText: string;
  pageIndex: number | null;
  rank: number;
};

export type ParseResult = {
  parseRun: {
    id: string;
    documentVersionId: string;
    caseId: string;
    engine: string;
    engineVersion: string;
    status: string;
    pagesExpected: number | null;
    pagesProcessed: number;
    warnings: string;
    createdAt: string;
  };
  pages: number;
  storedSegments: number;
  warnings: string[];
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
  vaultBackup: (destDir: string) => invoke<BackupResult>("vault_backup", { destDir }),
  vaultRestore: (backupDir: string, destDir: string) =>
    invoke<RestoreResult>("vault_restore", { backupDir, destDir }),
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
  documentParse: (versionId: string, ocr = false, ocrLang = "eng") =>
    invoke<ParseResult>("document_parse", { versionId, ocr, ocrLang }),
  searchSegments: (caseId: string, query: string, limit = 50) =>
    invoke<SearchHit[]>("search_segments", { caseId, query, limit }),

  eventList: (caseId: string) => invoke<EventRow[]>("event_list", { caseId }),
  eventCreate: (caseId: string, input: NewEventInput) =>
    invoke<EventRow>("event_create", { caseId, input }),
  eventConfirm: (id: string) => invoke<EventRow>("event_confirm", { id }),
  eventDelete: (id: string) => invoke<void>("event_delete", { id }),

  taskList: (caseId: string, includeDone = false) =>
    invoke<TaskRow[]>("task_list", { caseId, includeDone }),
  taskCreate: (caseId: string, input: NewTaskInput) =>
    invoke<TaskRow>("task_create", { caseId, input }),
  taskComplete: (id: string) => invoke<TaskRow>("task_complete", { id }),
  taskDelete: (id: string) => invoke<void>("task_delete", { id }),

  anchorList: (caseId: string) => invoke<AnchorRow[]>("anchor_list", { caseId }),
  providerProbe: () => invoke<AdapterView[]>("provider_probe"),

  precedentList: () => invoke<PrecedentRow[]>("precedent_list"),
  precedentCreate: (input: {
    court: string;
    chamber: string | null;
    docketNo: string | null;
    decisionNo: string | null;
    applicationNo: string | null;
    decisionDate: string | null;
    title: string | null;
    sourceUrl: string | null;
    userTags: string[];
    legalIssue: string | null;
  }) => invoke<PrecedentRow>("precedent_create", { input }),
  precedentNoteList: (precedentId: string) =>
    invoke<PrecedentNoteRow[]>("precedent_note_list", { precedentId }),
  precedentNoteCreate: (
    precedentId: string,
    input: {
      caseIdOptional: string | null;
      usefulPassageAnchor: string | null;
      similarityNote: string;
      distinguishingNote: string;
      userConclusion: string;
    },
  ) => invoke<PrecedentNoteRow>("precedent_note_create", { precedentId, input }),
  citationCheckList: (versionId: string) =>
    invoke<CitationCheckRow[]>("citation_check_list", { versionId }),
  citationCheckCreate: (input: {
    inputDocumentVersionId: string;
    extractedCitation: string;
    candidatePrecedentId: string | null;
    identityMatch: "match" | "mismatch" | "unknown";
    quoteMatch: "match" | "mismatch" | "not_checked" | "unknown";
    meaningReview: string;
    reasonCodes: string[];
  }) => invoke<CitationCheckRow>("citation_check_create", { input }),

  claimList: (caseId: string) => invoke<ClaimRow[]>("claim_list", { caseId }),
  claimCreate: (
    caseId: string,
    input: { claimantActorId: string | null; claimText: string; claimKind: string; sourceAnchorId: string | null },
  ) => invoke<ClaimRow>("claim_create", { caseId, input }),
  evidenceLinkList: (claimId: string) => invoke<EvidenceLinkRow[]>("evidence_link_list", { claimId }),
  evidenceLinkCreate: (
    caseId: string,
    input: { claimId: string; sourceAnchorId: string; relation: string; rationale: string },
  ) => invoke<EvidenceLinkRow>("evidence_link_create", { caseId, input }),

  reviewItemList: (caseId: string, reviewState?: string) =>
    invoke<ReviewItemRow[]>("review_item_list", { caseId, reviewState: reviewState ?? null }),
  reviewItemCreate: (
    caseId: string,
    input: { itemKind: string; payload: unknown; sourceAnchorIds: string[]; aiRunId: string | null },
  ) => invoke<ReviewItemRow>("review_item_create", { caseId, input }),
  reviewDecide: (
    itemId: string,
    decision: "accept" | "reject" | "edit" | "request_evidence",
    rationale: string,
    editedPayload?: unknown,
  ) =>
    invoke<ReviewItemRow>("review_decide", {
      itemId,
      decision,
      editedPayload: editedPayload ?? null,
      reviewer: "local-user",
      rationale,
    }),
  reviewHistory: (itemId: string) => invoke<ReviewDecisionRow[]>("review_history", { itemId }),
  anchorCreate: (input: {
    caseId: string;
    documentVersionId: string;
    parseRunId: string;
    segmentId: string;
    startOffset: number;
    endOffset: number;
    exactQuote: string;
  }) => invoke<AnchorRow>("anchor_create", { input }),

  documentPages: (versionId: string) =>
    invoke<DocumentPages | null>("document_pages", { versionId }),
  noteList: (caseId: string) => invoke<NoteRow[]>("note_list", { caseId }),
  noteCreate: (caseId: string, input: { title: string; body: string; sourceAnchorIds: string[] }) =>
    invoke<NoteRow>("note_create", { caseId, input }),
  noteDelete: (id: string) => invoke<void>("note_delete", { id }),
};
