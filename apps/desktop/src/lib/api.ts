import { invoke } from "@tauri-apps/api/core";

import type { ReviewTargetInput } from "./reviewTarget";

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

export type PageBlock = {
  start: number;
  end: number;
  bbox?: [number, number, number, number] | null;
  lines?: [number, number, number, number][];
  /** Yapısal motorlarda paragraf/bölüm yolu (`Kira › Ödeme · p/4`). */
  path?: string | null;
  /** `heading`, `paragraph`, `table`, `header`, `footer`, `annotation`. */
  kind?: string | null;
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
  /** Gösterim döndürmesi; koordinatlar döndürme öncesi uzaydadır. */
  rotation: number;
  /** `unrotated_cropbox_points`, `image_pixels`, `structure` veya null. */
  coordinateSpace: string | null;
  /** PDF `/PageLabels` basılı etiketi (`iv`, `A-12`); tanımsızsa null. */
  pageLabel: string | null;
  /** Blok aralıkları ve kutuları; kutu yoksa boş dizi. */
  blocks: PageBlock[];
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

export type CoverageReport = {
  documents: number;
  versions: number;
  parseRuns: number;
  pages: number;
  ocrPages: number;
  segments: number;
  emptyPages: number;
  unparsedDocuments: number;
  failedParseRuns: number;
};

export type ProviderProfileRow = {
  id: string;
  providerType: string;
  adapterId: string;
  executionEnvironment: string;
  policyVersion: string;
};

export type AiRunRow = {
  id: string;
  caseId: string;
  taskType: string;
  providerId: string | null;
  modelSnapshot: string;
  status: string;
  usageReport: string;
  createdAt: string;
  completedAt: string | null;
};

export type TransferConsentRow = {
  id: string;
  providerSnapshot: string;
  caseScope: string;
  purpose: string;
  selectedDocumentIds: string[];
  expiresAt: string | null;
  consentTextVersion: string;
  approvedAt: string;
  revokedAt: string | null;
};

export type AiOverview = { providers: ProviderProfileRow[]; runs: AiRunRow[]; coverage: CoverageReport };

export type AiCandidateRow = {
  id: string;
  aiRunId: string;
  candidateType: string;
  structuredPayload: string;
  validationState: string;
  validationErrors: string;
};

export type PackageResult = {
  path: string;
  files: number;
  sha256: string;
  sealedPath: string | null;
  record: { id: string; exportType: string; status: string; createdAt: string };
};

export type ExportResult = {
  path: string;
  bytes: number;
  sha256: string;
  record: { id: string; exportType: string; status: string; outputHash: string | null; createdAt: string };
};

export type RedactionResult = {
  document: DocumentRow;
  version: DocumentVersionRow;
  plan: { id: string; state: string; outputProfile: string };
  hits: number;
  remaining: string[];
  ocrVerified: boolean;
  engines: string[];
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

/** Kararın tam metin sürümü (F12-T05). Metnin kendisi arayüze gelmez. */
export type PrecedentVersionRow = {
  id: string;
  precedentId: string;
  documentVersionId: string | null;
  acquiredAt: string;
  sourceKind: "paste" | "document" | "file";
  bodyHash: string;
  verificationState: "unverified" | "metadata_verified" | "text_verified" | "failed";
  charCount: number;
  hasText: boolean;
};

/** Metin içi alıntı doğrulaması. `not_checked` "uydurulmuş" demek değildir. */
export type QuoteVerificationRow = {
  precedentId: string;
  versionId: string | null;
  state: "match" | "mismatch" | "not_checked";
  method: "exact" | "whitespace" | "turkish_fold" | null;
  matchedStart: number | null;
  matchedLength: number | null;
  occurrences: number;
  reason: "no_full_text" | "quote_too_short" | "not_found" | null;
  quote: string;
  checkedAt: string;
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

export type ClaimRelationRow = {
  id: string;
  caseId: string;
  claimId: string;
  relatedClaimId: string;
  relation: string;
  rationale: string;
  createdAt: string;
};

export type ContradictionAssessmentRow = {
  id: string;
  caseId: string;
  reviewItemId: string;
  verdict: string;
  rationale: string;
  reviewerLabel: string;
  assessedAt: string;
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
  state: "pending" | "accepted" | "rejected" | "edited" | "needs_more_evidence" | "superseded";
  targetEntityId: string | null;
  supersededByItemId: string | null;
  supersededAt: string | null;
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

/** İnceleme kararının ürettiği hedef kayıt (F08-T08). */
export type ReviewCommitResult = {
  item: ReviewItemRow;
  decisionId: string;
  targetKind: "claim" | "note" | "task" | "event";
  targetId: string;
  outboxEventId: string;
};

export type OutboxEventRow = {
  id: string;
  aggregateId: string;
  caseId: string | null;
  eventType: string;
  payloadVersion: number;
  payload: unknown;
  createdAt: string;
  deliveredAt: string | null;
  attemptCount: number;
};

export type GcBlobRow = { id: string; storageId: string; encryptedSize: number };

export type VaultGcResult = {
  dryRun: boolean;
  candidates: GcBlobRow[];
  deleted: GcBlobRow[];
  freedBytes: number;
  missingFiles: string[];
};

export type BackupResult = {
  path: string;
  fileCount: number;
  totalBytes: number;
  manifestHash: string;
  archivePath: string | null;
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

/** F07-T09: kalıcı işaret. `text` işareti doğrulanmış anchor'a, `region`
 *  işareti sayfanın kendi koordinat uzayındaki dikdörtgenlere bağlıdır. */
export type MarkRow = {
  id: string;
  caseId: string;
  documentVersionId: string;
  parseRunId: string | null;
  anchorId: string | null;
  pageId: string | null;
  markKind: "text" | "region";
  startOffset: number | null;
  endOffset: number | null;
  quotedText: string;
  rects: [number, number, number, number][];
  label: string;
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
  revisionNo: number;
  canUndo: boolean;
  canRedo: boolean;
};

export type PagePreviewView = {
  versionId: string;
  pageIndex: number;
  dpi: number;
  pngBase64: string;
  byteSize: number;
  cached: boolean;
};

export type NoteRevisionRow = {
  noteId: string;
  revisionNo: number;
  title: string;
  body: string;
  sourceAnchorIds: string[];
  origin: string;
  createdAt: string;
};

export type NoteDraftRow = {
  caseId: string;
  noteId: string | null;
  title: string;
  body: string;
  sourceAnchorIds: string[];
  baseRowVersion: number | null;
  updatedAt: string;
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
  /** Tarih kullanıcı tarafından açıkça onaylandıysa dolu (F08-T10). */
  dateConfirmedAt: string | null;
  createdAt: string;
  updatedAt: string;
  rowVersion: number;
};

export type ReminderRow = {
  taskId: string;
  dueDate: string;
  policy: string;
  localDay: string;
};

export type ReminderEnqueueResult = {
  queued: number;
  reminders: ReminderRow[];
};

export type NewTaskInput = {
  title: string;
  userDueDate: string | null;
  dateTimezone: string | null;
  assigneeLabel: string | null;
  reminderPolicy: string;
};

/** Lisanslı kontrol listesi şablonu (F08-T11); künye dosyaya birlikte kopyalanır. */
export type ChecklistTemplateRow = {
  id: string;
  title: string;
  practiceArea: string | null;
  version: number;
  author: string;
  reviewDate: string | null;
  licenseId: string | null;
  licenseUrl: string | null;
  sourceUrl: string | null;
  contentHash: string;
  isCommunity: boolean;
  itemCount: number;
  copiedItemCount: number;
};

export type ChecklistTemplateRef = {
  id: string;
  title: string;
  version: number;
  licenseId: string | null;
  licenseUrl: string | null;
  author: string;
  reviewDate: string | null;
  contentHash: string;
};

export type LinkedDocumentRow = {
  id: string;
  displayName: string | null;
};

export type ChecklistItemState = "open" | "present" | "missing" | "not_applicable";

export type CaseChecklistItemRow = {
  id: string;
  caseId: string;
  templateVersionId: string | null;
  itemText: string;
  state: string;
  linkedDocumentIds: string[];
  linkedDocuments: LinkedDocumentRow[];
  note: string;
  ordinal: number;
  category: string;
  createdAt: string;
  updatedAt: string;
  rowVersion: number;
  template: ChecklistTemplateRef | null;
};

export type ChecklistCopyResult = {
  created: boolean;
  itemCount: number;
  templateId: string;
  templateTitle: string;
  templateVersion: number;
  licenseId: string | null;
  contentHash: string;
};

export type EventSourceRow = {
  id: string;
  eventId: string;
  sourceAnchorId: string;
  relation: string;
  userConfirmed: boolean;
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

export type EventPatchInput = {
  eventKind: string;
  dateValue: string | null;
  datePrecision: string;
  dateRole: string;
  dateRaw: string | null;
  title: string;
  description: string;
};

export type SearchHit = {
  segmentId: string;
  pageId: string | null;
  /** Sonucun ait olduğu belge sürümü; okuyucu bunu açar (F07 kabul testi). */
  documentVersionId: string | null;
  documentId: string | null;
  documentName: string | null;
  /** Kapsam görünürlüğü (F07-T05): belge türü, durumu, metnin kaynağı. */
  documentType: string | null;
  documentStatus: string | null;
  textOrigin: string | null;
  caseId: string | null;
  tags: string[];
  rawText: string;
  pageIndex: number | null;
  rank: number;
};

/** Arama filtreleri (F07-T03). Boş alan = filtre yok. */
export type SearchFilters = {
  documentTypes: string[];
  /** Belgeye kaynaklı olayın tarih rolü (tebliğ, sözleşme…). */
  dateRole: string | null;
  /** Belgelerin taşıması gereken **tüm** etiketler. */
  tags: string[];
  /** "any" | "ocr" | "native" */
  ocrStatus: string | null;
};

/** Arama kipi (F07-T02). `auto` kullanıcının yazımından kipi çıkarır. */
export type SearchMode = "auto" | "exact" | "flexible" | "phrase" | "docket";

export type SearchPage = {
  caseId: string;
  query: string;
  /** Tırnaklar atılmış, kipe göre çözümlenmiş metin (alıntı işaretlemesi). */
  resolvedQuery: string;
  /** Sunucunun çözümlediği kesin kip; `auto` dönmez. */
  mode: Exclude<SearchMode, "auto">;
  /** `case` = yalnız bu dosya. */
  scope: string;
  limit: number;
  returned: number;
  hasMore: boolean;
  nextCursor: string | null;
  /** Yalnız ilk sayfada istenir. */
  total: number | null;
  filters: SearchFilters;
  hits: SearchHit[];
};

export type CaseTagRow = {
  tag: string;
  documentCount: number;
};

export type DocumentTagRow = {
  documentId: string;
  caseId: string;
  tag: string;
  createdAt: string;
};

export type AnchorLocationRow = {
  anchorId: string;
  caseId: string;
  documentVersionId: string;
  documentId: string | null;
  documentName: string | null;
  parseRunId: string;
  pageId: string | null;
  pageIndex: number | null;
  segmentId: string;
  startOffset: number;
  endOffset: number;
  exactQuote: string;
  anchorStatus: string;
  quoteIntact: boolean;
  /** Alıntının blok kutuları (segmentin koordinat uzayında). */
  quads: [number, number, number, number][];
  coordinateSpace: string | null;
  pageRotation: number;
  /** Anchor belgenin güncel sürümüne mi bağlı (F07-T12). */
  isCurrentVersion: boolean;
  currentVersionId: string | null;
  versionNo: number | null;
  currentVersionNo: number | null;
  /** Anchor sürümün en son ayrıştırmasına mı bağlı (F06-T10). */
  isCurrentRun: boolean;
};

export type RebindCandidate = {
  segmentId: string;
  pageId: string | null;
  pageIndex: number | null;
  startOffset: number;
  endOffset: number;
};

/** Kopmuş/eski bağlantı için yeniden bağlama önerisi; öneri kendiliğinden
 *  uygulanmaz, kullanıcı onayı gerekir. */
export type RebindSuggestion = {
  anchorId: string;
  quote: string;
  currentVersionId: string | null;
  versionNo: number | null;
  currentVersionNo: number | null;
  /** Anchor sürümün en son ayrıştırmasına mı bağlı (F06-T10). */
  isCurrentRun: boolean;
  status: "current" | "exact" | "ambiguous" | "unresolved";
  candidates: RebindCandidate[];
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
  vaultGc: (dryRun: boolean) => invoke<VaultGcResult>("vault_gc", { dryRun }),
  vaultChangePassword: (oldPassword: string, newPassword: string) =>
    invoke<void>("vault_change_password", { oldPassword, newPassword }),
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
  documentParse: (
    versionId: string,
    ocr = false,
    ocrLang = "eng",
    textEncoding = "auto",
    password?: string,
  ) =>
    invoke<ParseResult>("document_parse", { versionId, ocr, ocrLang, textEncoding, password }),
  searchSegments: (caseId: string, query: string, limit = 50) =>
    invoke<SearchHit[]>("search_segments", { caseId, query, limit }),
  /** Filtreli + keyset sayfalı arama (F07-T03/T05); kip F07-T02. */
  searchPage: (
    caseId: string,
    query: string,
    filters: SearchFilters,
    options: {
      limit?: number;
      cursor?: string | null;
      includeTotal?: boolean;
      mode?: SearchMode;
    } = {},
  ) =>
    invoke<SearchPage>("search_segments_page", {
      input: {
        caseId,
        query,
        filters,
        limit: options.limit ?? 25,
        cursor: options.cursor ?? null,
        includeTotal: options.includeTotal ?? false,
        mode: options.mode ?? "auto",
      },
    }),

  documentTags: (documentId: string) => invoke<string[]>("document_tags", { documentId }),
  documentTagsForCase: (caseId: string) =>
    invoke<DocumentTagRow[]>("document_tags_for_case", { caseId }),
  caseTags: (caseId: string) => invoke<CaseTagRow[]>("case_tags", { caseId }),
  documentTagAdd: (documentId: string, tag: string) =>
    invoke<string[]>("document_tag_add", { documentId, tag }),
  documentTagRemove: (documentId: string, tag: string) =>
    invoke<string[]>("document_tag_remove", { documentId, tag }),

  eventList: (caseId: string) => invoke<EventRow[]>("event_list", { caseId }),
  eventCreate: (caseId: string, input: NewEventInput) =>
    invoke<EventRow>("event_create", { caseId, input }),
  eventConfirm: (id: string) => invoke<EventRow>("event_confirm", { id }),
  eventUpdate: (id: string, expectedRowVersion: number, input: EventPatchInput) =>
    invoke<EventRow>("event_update", { id, expectedRowVersion, input }),
  eventDelete: (id: string) => invoke<void>("event_delete", { id }),
  eventSources: (eventId: string) => invoke<EventSourceRow[]>("event_sources", { eventId }),
  eventLinkSource: (eventId: string, anchorId: string, relation: string) =>
    invoke<EventSourceRow>("event_link_source", { eventId, anchorId, relation }),
  eventUnlinkSource: (id: string) => invoke<void>("event_unlink_source", { id }),

  taskList: (caseId: string, includeDone = false) =>
    invoke<TaskRow[]>("task_list", { caseId, includeDone }),
  taskCreate: (caseId: string, input: NewTaskInput) =>
    invoke<TaskRow>("task_create", { caseId, input }),
  taskComplete: (id: string) => invoke<TaskRow>("task_complete", { id }),
  taskDelete: (id: string) => invoke<void>("task_delete", { id }),
  taskConfirmDate: (id: string) => invoke<TaskRow>("task_confirm_date", { id }),
  taskReminders: (caseId: string, today: string) =>
    invoke<ReminderRow[]>("task_reminders", { caseId, today }),
  taskRemindersEnqueue: (caseId: string, today: string) =>
    invoke<ReminderEnqueueResult>("task_reminders_enqueue", { caseId, today }),

  anchorList: (caseId: string) => invoke<AnchorRow[]>("anchor_list", { caseId }),
  anchorLocate: (anchorId: string) =>
    invoke<AnchorLocationRow | null>("anchor_locate", { anchorId }),
  providerProbe: () => invoke<AdapterView[]>("provider_probe"),

  documentRedact: (versionId: string, terms: string[]) =>
    invoke<RedactionResult>("document_redact", { versionId, terms }),
  documentRedactRegions: (versionId: string, regions: Array<{ pageIndex: number; rect: [number, number, number, number] }>) =>
    invoke<RedactionResult>("document_redact_regions", { versionId, regions }),
  exportJson: (caseId: string, destPath: string) =>
    invoke<ExportResult>("export_json", { caseId, destPath }),
  exportCsv: (caseId: string, destPath: string) =>
    invoke<ExportResult>("export_csv", { caseId, destPath }),
  exportDocx: (caseId: string, destPath: string) =>
    invoke<ExportResult>("export_docx", { caseId, destPath }),
  exportPdf: (caseId: string, destPath: string) =>
    invoke<ExportResult>("export_pdf", { caseId, destPath }),
  exportPackage: (caseId: string, destDir: string, password?: string) =>
    invoke<PackageResult>("export_package", { caseId, destDir, password }),

  aiOverview: (caseId: string) => invoke<AiOverview>("ai_overview", { caseId }),
  aiConsentCreate: (caseId: string, providerSnapshot: string, purpose: string, documentIds: string[]) =>
    invoke<TransferConsentRow>("ai_consent_create", { caseId, providerSnapshot, purpose, documentIds }),
  aiConsentRevoke: (consentId: string) => invoke<void>("ai_consent_revoke", { consentId }),
  aiRunStart: (caseId: string, providerId: string, consentId: string, taskType: string) =>
    invoke<AiRunRow>("ai_run_start", { caseId, providerId, consentId, taskType }),
  aiCandidatesList: (runId: string) => invoke<AiCandidateRow[]>("ai_candidates_list", { runId }),
  aiCandidatePromote: (candidateId: string, caseId: string) =>
    invoke<string>("ai_candidate_promote", { candidateId, caseId }),

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
  precedentTextAdd: (
    precedentId: string,
    input: {
      sourceKind: "paste" | "document" | "file";
      text: string;
      documentVersionId: string | null;
    },
  ) => invoke<PrecedentVersionRow>("precedent_text_add", { precedentId, input }),
  precedentVersionList: (precedentId: string) =>
    invoke<PrecedentVersionRow[]>("precedent_version_list", { precedentId }),
  precedentQuoteVerify: (precedentId: string, quote: string) =>
    invoke<QuoteVerificationRow>("precedent_quote_verify", { precedentId, quote }),
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

  claimRelationList: (claimId: string) =>
    invoke<ClaimRelationRow[]>("claim_relation_list", { claimId }),
  claimRelationCreate: (
    caseId: string,
    claimId: string,
    input: { relatedClaimId: string; relation: string; rationale: string },
  ) => invoke<ClaimRelationRow>("claim_relation_create", { caseId, claimId, input }),

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
  reviewSupersede: (itemId: string, supersededByItemId: string) =>
    invoke<ReviewItemRow>("review_supersede", { itemId, supersededByItemId }),
  reviewCommit: (
    itemId: string,
    decision: "accept" | "edit",
    target: ReviewTargetInput,
    rationale: string,
  ) =>
    invoke<ReviewCommitResult>("review_commit", {
      itemId,
      decision,
      target,
      reviewer: "local-user",
      rationale,
    }),
  reviewOutboxPending: (caseId: string) =>
    invoke<OutboxEventRow[]>("review_outbox_pending", { caseId }),
  contradictionAssess: (
    caseId: string,
    itemId: string,
    verdict: string,
    rationale: string,
    reviewer: string,
  ) =>
    invoke<ContradictionAssessmentRow>("contradiction_assess", {
      caseId,
      itemId,
      verdict,
      rationale,
      reviewer,
    }),
  contradictionAssessments: (itemId: string) =>
    invoke<ContradictionAssessmentRow[]>("contradiction_assessments", { itemId }),
  reviewHistory: (itemId: string) => invoke<ReviewDecisionRow[]>("review_history", { itemId }),

  checklistTemplates: (caseId: string) =>
    invoke<ChecklistTemplateRow[]>("checklist_templates", { caseId }),
  checklistCopy: (caseId: string, templateId: string) =>
    invoke<ChecklistCopyResult>("checklist_copy", { caseId, templateId }),
  checklistItems: (caseId: string) =>
    invoke<CaseChecklistItemRow[]>("checklist_items", { caseId }),
  checklistItemState: (
    itemId: string,
    newState: ChecklistItemState,
    linkedDocumentIds: string[],
    note: string,
    expectedRowVersion: number,
  ) =>
    invoke<CaseChecklistItemRow>("checklist_item_state", {
      itemId,
      newState,
      linkedDocumentIds,
      note,
      expectedRowVersion,
    }),
  anchorRebindSuggestion: (anchorId: string) =>
    invoke<RebindSuggestion>("anchor_rebind_suggestion", { anchorId }),
  anchorRebind: (anchorId: string, segmentId: string, startOffset: number) =>
    invoke<AnchorRow>("anchor_rebind", { anchorId, segmentId, startOffset }),
  anchorCreate: (input: {
    caseId: string;
    documentVersionId: string;
    parseRunId: string;
    segmentId: string;
    startOffset: number;
    endOffset: number;
    exactQuote: string;
  }) => invoke<AnchorRow>("anchor_create", { input }),

  markCreateFromSelection: (
    input: {
      caseId: string;
      documentVersionId: string;
      parseRunId: string;
      segmentId: string;
      startOffset: number;
      endOffset: number;
      exactQuote: string;
    },
    label: string,
  ) => invoke<{ anchor: AnchorRow; mark: MarkRow }>("mark_create_from_selection", { input, label }),
  noteCreateFromSelection: (
    input: {
      caseId: string;
      documentVersionId: string;
      parseRunId: string;
      segmentId: string;
      startOffset: number;
      endOffset: number;
      exactQuote: string;
    },
    note: { title: string; body: string },
  ) => invoke<{ anchor: AnchorRow; note: NoteRow }>("note_create_from_selection", { input, note }),
  markCreateRegion: (input: {
    caseId: string;
    documentVersionId: string;
    pageId: string;
    rects: [number, number, number, number][];
    label: string;
  }) => invoke<MarkRow>("mark_create_region", { input }),
  marksForVersion: (versionId: string) => invoke<MarkRow[]>("marks_for_version", { versionId }),
  markDelete: (markId: string) => invoke<void>("mark_delete", { markId }),
  documentPages: (versionId: string) =>
    invoke<DocumentPages | null>("document_pages", { versionId }),
  documentPagePreview: (versionId: string, pageIndex: number, dpi = 96) =>
    invoke<PagePreviewView>("document_page_preview", { versionId, pageIndex, dpi }),
  noteList: (caseId: string) => invoke<NoteRow[]>("note_list", { caseId }),
  noteCreate: (caseId: string, input: { title: string; body: string; sourceAnchorIds: string[] }) =>
    invoke<NoteRow>("note_create", { caseId, input }),
  noteDelete: (id: string) => invoke<void>("note_delete", { id }),
  noteGet: (id: string) => invoke<NoteRow | null>("note_get", { id }),
  noteUpdate: (
    id: string,
    expectedRowVersion: number,
    input: { title: string; body: string; sourceAnchorIds: string[] },
  ) => invoke<NoteRow>("note_update", { id, expectedRowVersion, input }),
  noteUndo: (id: string, expectedRowVersion: number) =>
    invoke<NoteRow>("note_undo", { id, expectedRowVersion }),
  noteRedo: (id: string, expectedRowVersion: number) =>
    invoke<NoteRow>("note_redo", { id, expectedRowVersion }),
  noteRevisions: (id: string) => invoke<NoteRevisionRow[]>("note_revisions", { id }),
  noteDraftSave: (
    caseId: string,
    noteId: string | null,
    input: { title: string; body: string; sourceAnchorIds: string[]; baseRowVersion: number | null },
  ) => invoke<NoteDraftRow>("note_draft_save", { caseId, noteId, input }),
  noteDraftGet: (caseId: string, noteId: string | null) =>
    invoke<NoteDraftRow | null>("note_draft_get", { caseId, noteId }),
  noteDraftClear: (caseId: string, noteId: string | null) =>
    invoke<boolean>("note_draft_clear", { caseId, noteId }),
};
