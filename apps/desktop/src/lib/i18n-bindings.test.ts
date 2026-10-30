/**
 * F04-T08 — sözlük **bağı** kapısı (ekran/lib metni sözlükten mi geliyor?).
 *
 * `localization.test.ts` taşınmış dosyada gömülü Türkçe *harf* arar; burada
 * aranan şey başka: üretilen metnin **birebir sözlükten** gelmesi. Metin lib'e
 * geri gömülürse (kapı yalnız harfe baktığı için `bellek`, `dosya`, `OCR` gibi
 * Türkçe harf içermeyen sözcükler kaçar) bu test kırmızı olur.
 */
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import type { CorpusCoverage, ExportCoverage, JobRetryView, JobRow, PrecedentDocumentText, PrecedentSearchHit, PrecedentSearchPage, PrecedentUsageTerms, PreUpdateBackup, QuoteVerificationRow, SearchPage } from "./api";
import { elapsedText, mibText, profileLimits, secondsText } from "./budget";
import { COVERAGE_EMBED_TARGETS, coverageHeadline, coverageRows } from "./export-coverage";
import { formatTimestamp } from "./format";
import { messageKeys, t } from "./i18n";
import {
  archBadgeText,
  archFormatText,
  archInterpreterText,
  archReasonText,
  archSummaryText,
  archWarnings,
  archWorst,
} from "./arch";
import {
  attackCommand,
  attackFixtureSummary,
  attackProductText,
  attackSkipText,
  attackVerdictText,
  attackWorstVerdict,
} from "./attack";
import {
  packageLimits,
  packageStatusText,
  packageSummary,
  packageWarnings,
  problemText,
  roleLabel,
} from "./package";
import {
  capabilityGateText,
  formatBytes,
  probeScopeLines,
  probeStatusLabel,
  probeSummary,
  residueAge,
  residueMeasureSummary,
  sandboxSweepSummary,
} from "./providers";
import { defaultOcrLanguageLabel } from "./ocr";
import { MIN_PASSWORD_LENGTH, checkPassword, checkPasswordConfirmation } from "./vault-input";
import {
  blockedReasonLabel,
  jobActionLabel,
  jobStateLabel,
  jobTypeLabel,
  queueSummaryLine,
  recoveryLine,
  retryOutcomeLine,
} from "./jobs";
import { parseProgressLine, parseStageLabel } from "./parseProgress";
import {
  documentTextSummary,
  documentTextWarnings,
  transferBlockedReason,
  versionOptionLabel,
} from "./precedent-document";
import {
  corpusGateText,
  corpusResultText,
  coverageText,
  exclusionReasonLabel,
  redistributionLabel,
  termsBadge,
  termsRecordedText,
  usageBasisLabel,
} from "./precedent-terms";
import {
  accessBasisLabel,
  bibliographyMissingText,
  bulkOutcomeLabel,
  bulkReportSummary,
  quoteScanNote,
  quoteStateText,
  versionConflictText,
  versionSourceLabel,
  versionStateText,
} from "./precedents";
import {
  CONFLICT_HINT,
  DRAFT_STATUS_LABEL,
  REVISION_ORIGIN_LABEL,
  revisionOriginLabel,
} from "./notes";
import { REMINDER_POLICY_LABEL, REMINDER_TEXT } from "./reminders";
import {
  ROLLBACK_CONFIRMATION,
  backupStatusText,
  backupVersionLabel,
  backupWarnings,
  rollbackConsequences,
  rollbackDisabledReason,
  rollbackSummary,
} from "./rollback";
import {
  DATE_ROLE_LABELS,
  OCR_STATUS_LABELS,
  SEARCH_MODE_LABELS,
  describeFilters,
  resultSummary,
  searchModeHint,
  textOriginLabel,
  totalSummary,
  verificationSummary,
} from "./search";
import {
  SEARCH_SCOPE_LABELS,
  precedentCitation,
  precedentMatchLabel,
  precedentSearchSummary,
  precedentSnippetForCopy,
  scopeNotice,
} from "./search-scope";

function profile(partial: Partial<JobRow> = {}): JobRow {
  return {
    id: "job-1",
    jobType: "parse_document",
    scope: "document",
    caseId: null,
    state: "queued",
    priority: 100,
    attemptCount: 0,
    maxAttempts: 3,
    progress: 0,
    progressStage: null,
    progressDone: null,
    progressTotal: null,
    nextRunAt: null,
    leaseOwner: null,
    leaseUntil: null,
    cancelRequestedAt: null,
    errorCode: null,
    blockedReason: null,
    createdAt: "2026-10-30T01:02:03Z",
    updatedAt: "2026-10-30T01:02:03Z",
    completedAt: null,
    ...partial,
  };
}

function retryView(partial: Partial<JobRetryView> = {}): JobRetryView {
  return {
    job: profile(),
    started: false,
    skipped: null,
    versionId: "ver-1",
    applied: null,
    ...partial,
  };
}

function coverage(partial: Partial<ExportCoverage> = {}): ExportCoverage {
  return {
    documents: 0,
    versions: 0,
    parseRuns: 0,
    pages: 0,
    textPages: 0,
    ocrPages: 0,
    segments: 0,
    emptyPages: 0,
    unparsedDocuments: 0,
    failedParseRuns: 0,
    complete: false,
    warnings: [],
    statement: "",
    generatedAt: "2026-10-30T00:00:00Z",
    ...partial,
  };
}

describe("iş kuyruğu metinleri sözlükten gelir", () => {
  it("durum, tür, engel ve eylem etiketleri birebir sözlük değeridir", () => {
    expect(jobStateLabel("queued")).toBe(t("jobs.state.queued"));
    expect(jobStateLabel("completed")).toBe(t("jobs.state.completed"));
    // Eski ad (0001–0020) da sözlükten; gizlenmez.
    expect(jobStateLabel("succeeded")).toBe(t("jobs.state.succeeded"));
    expect(jobTypeLabel("parse_document")).toBe(t("jobs.type.parse_document"));
    expect(blockedReasonLabel("password")).toBe(t("jobs.blockedReason.password"));
    expect(jobActionLabel("run")).toBe(t("jobs.action.run"));
  });

  it("bilinmeyen durum/engel uydurulmaz, kodu görünür kalır", () => {
    const state = jobStateLabel("weird_state");
    expect(state).toBe(t("jobs.unknownState", { state: "weird_state" }));
    expect(state).toContain("weird_state");
    expect(blockedReasonLabel("weird")).toBe(t("jobs.unknownBlocked", { reason: "weird" }));
  });

  it("parametreli satırlar doldurulur, yer tutucu kalmaz", () => {
    const started = retryOutcomeLine(
      retryView({
        started: true,
        applied: { ocr: true, ocrLang: "tur", textEncoding: "auto", restored: true },
      }),
    );
    expect(started).toContain("tur");
    expect(started).toContain(t("jobs.start.sourceStored"));
    expect(started).not.toContain("{");
    // Kayıt yoksa "kaydedilmiş ayarlar" DENMEZ.
    const fallback = retryOutcomeLine(retryView({ started: true }));
    expect(fallback).toContain(t("jobs.start.sourceDefault"));
    expect(fallback).not.toContain("{");
    expect(recoveryLine({ interrupted: 0, requeued: 2, failed: 0, canceled: 1 })).toBe(
      `${t("jobs.recovery.requeued", { count: 2 })}, ${t("jobs.recovery.canceled", { count: 1 })}.`,
    );
  });

  it("özet satırında durum adı sözlükteki küçük harfli biçimden gelir", () => {
    const line = queueSummaryLine({
      total: 3,
      byState: [
        { state: "running", count: 1 },
        { state: "failed", count: 2 },
      ],
      dueNow: 0,
      oldestQueuedAt: null,
      blockedByDependency: 0,
    });
    expect(line).toContain(t("jobs.summaryPart", { count: 1, state: t("jobs.stateLower.running") }));
    expect(line).toContain(t("jobs.summaryPart", { count: 2, state: t("jobs.stateLower.failed") }));
    expect(line).not.toContain("{");
  });
});

describe("yardımcı lib metinleri sözlükten gelir", () => {
  it("ayrıştırma aşama etiketleri ve sayaç satırı", () => {
    expect(parseStageLabel("start")).toBe(t("parseProgress.stage.start"));
    expect(parseStageLabel("scan")).toBe(t("parseProgress.stage.scan"));
    expect(parseStageLabel("  ")).toBe(t("parseProgress.fallback"));
    // Bilinmeyen aşama kendi protokol adıyla görünür (uydurulmaz).
    expect(parseStageLabel("custom")).toBe("custom");
    const line = parseProgressLine({ stage: "pages", done: 3, total: 10 });
    expect(line).toBe(t("parseProgress.line", { label: t("parseProgress.stage.pages"), done: 3, total: 10, percent: 30 }));
    expect(line).not.toContain("{");
  });

  it("bütçe metinleri sözlükten, ondalık biçim format katmanından", () => {
    expect(mibText(0)).toBe(t("budget.zeroMib"));
    expect(mibText(1024 * 1024 * 1.5)).toBe(t("budget.mib", { value: "1,5" }));
    expect(secondsText(0)).toBe(t("budget.notApplicable"));
    expect(elapsedText(1200)).toBe(t("budget.seconds", { value: "1,2" }));
    const line = profileLimits({
      kind: "ocr",
      label: "OCR",
      cpuSeconds: 120,
      memoryBytes: 1024 * 1024 * 1024,
      outputBytes: 32 * 1024 * 1024,
      openFiles: 256,
      wallClockSeconds: 300,
    });
    expect(line).toContain(t("budget.timeLine", { value: 300 }));
    expect(line).toContain(t("budget.cpuLine", { value: secondsText(120) }));
    expect(line).not.toContain("{");
  });

  it("kapsam satırları ve gömme hedefleri sözlükten", () => {
    const rows = coverageRows(coverage({ documents: 2, pages: 5 }));
    // Boş liste sessizce geçmesin: indeks erişimi `noUncheckedIndexedAccess`
    // altında `undefined` olabilir, bu yüzden uzunluk ayrıca kapıya bağlanır.
    expect(rows.length).toBeGreaterThan(1);
    expect(rows[0]?.label).toBe(t("exportCoverage.row.documents"));
    expect(rows[rows.length - 1]?.label).toBe(t("exportCoverage.row.failedRuns"));
    expect(COVERAGE_EMBED_TARGETS.length).toBeGreaterThan(0);
    expect(COVERAGE_EMBED_TARGETS[0]?.label).toBe(t("exportCoverage.embedTarget.json"));
    const headline = coverageHeadline(coverage({ documents: 2, pages: 5, textPages: 4, ocrPages: 1 }));
    expect(headline).toBe(
      t("exportCoverage.headline", {
        documents: 2,
        pages: 5,
        textPages: 4,
        ocrPages: 1,
        emptyPages: 0,
        unparsed: 0,
      }),
    );
    expect(headline).not.toContain("{");
  });

  it("hatırlatıcı bildirimi ve ilke etiketleri sözlükten", () => {
    expect(REMINDER_TEXT).toBe(t("reminders.notification"));
    expect(REMINDER_POLICY_LABEL.none).toBe(t("reminders.policy.none"));
    expect(REMINDER_POLICY_LABEL.day_before).toBe(t("reminders.policy.day_before"));
  });
});

function searchPage(partial: Partial<SearchPage> = {}): SearchPage {
  return {
    caseId: "case-1",
    query: "ödeme",
    resolvedQuery: "ödeme",
    mode: "flexible",
    scope: "case",
    limit: 25,
    returned: 3,
    hasMore: false,
    nextCursor: null,
    total: null,
    filters: { documentTypes: [], dateRole: null, tags: [], ocrStatus: "any" },
    hits: [],
    ...partial,
  };
}

function precedentPage(partial: Partial<PrecedentSearchPage> = {}): PrecedentSearchPage {
  return {
    scope: "precedent_library",
    mode: "flexible",
    query: "ödeme",
    resolvedQuery: "ödeme",
    hits: [],
    scanned: 4,
    matched: 2,
    withoutText: 0,
    limit: 25,
    truncated: false,
    notes: [],
    ...partial,
  };
}

function precedentHit(partial: Partial<PrecedentSearchHit> = {}): PrecedentSearchHit {
  return {
    precedentId: "prec-1",
    court: "Yargıtay",
    chamber: "1. Hukuk Dairesi",
    docketNo: "2024/123",
    decisionNo: "2025/45",
    decisionDate: "2025-02-03",
    title: null,
    versionId: "1234567890abcdef",
    acquiredAt: "2026-01-02T03:04:05Z",
    sourceKind: "manual",
    snippet: "ödeme yapılmadığı",
    snippetStart: 10,
    matchStart: 10,
    matchLength: 17,
    matchSource: "full_text",
    ...partial,
  };
}

describe("arama metinleri sözlükten gelir", () => {
  it("kip ve filtre etiketleri sözlükten", () => {
    expect(SEARCH_MODE_LABELS.auto).toBe(t("searchFilters.mode.auto"));
    expect(SEARCH_MODE_LABELS.exact).toBe(t("searchFilters.mode.exact"));
    expect(SEARCH_MODE_LABELS.flexible).toBe(t("searchFilters.mode.flexible"));
    expect(SEARCH_MODE_LABELS.phrase).toBe(t("searchFilters.mode.phrase"));
    expect(SEARCH_MODE_LABELS.docket).toBe(t("searchFilters.mode.docket"));
    expect(DATE_ROLE_LABELS.deadline_candidate).toBe(t("searchFilters.dateRole.deadline_candidate"));
    expect(DATE_ROLE_LABELS.unknown).toBe(t("searchFilters.dateRole.unknown"));
    expect(OCR_STATUS_LABELS.native).toBe(t("searchFilters.ocrStatus.native"));
    expect(textOriginLabel("mixed")).toBe(t("searchFilters.textOrigin.mixed"));
    // Bilinmeyen kaynak uydurulmaz: sabit "bilinmiyor" metni sözlükten gelir.
    expect(textOriginLabel(null)).toBe(t("searchFilters.textOrigin.unknown"));
    expect(textOriginLabel("something_else")).toBe(t("searchFilters.textOrigin.unknown"));
  });

  it("filtre satırları sözlükten ve parametreler doldurulur", () => {
    const lines = describeFilters({
      documentTypes: ["Vekaletname", "Bilirkişi raporu"],
      dateRole: "notification",
      tags: ["kira", "ödeme"],
      ocrStatus: "ocr",
    });
    expect(lines).toEqual([
      // `canonicalFilters` türleri sıralar: çıktı sıralı gelir (davranış aynı).
      t("searchFilters.describe.documentTypes", { types: "Bilirkişi raporu, Vekaletname" }),
      t("searchFilters.describe.dateRole", { role: t("searchFilters.dateRole.notification") }),
      t("searchFilters.describe.tags", { tags: "kira, ödeme" }),
      t("searchFilters.describe.ocrStatus", { label: t("searchFilters.ocrStatus.ocr") }),
    ]);
    for (const line of lines) expect(line).not.toContain("{");
  });

  it("sonuç/toplam/doğrulama sayaç satırları sözlükten", () => {
    expect(resultSummary(searchPage({ returned: 3 }))).toBe(
      t("searchFilters.result.shown", { returned: 3 }),
    );
    expect(resultSummary(searchPage({ returned: 3, hasMore: true }))).toBe(
      t("searchFilters.result.shownMore", {
        shown: t("searchFilters.result.shown", { returned: 3 }),
      }),
    );
    expect(resultSummary(searchPage({ returned: 3, total: 9 }))).toBe(
      t("searchFilters.result.withTotal", { total: 9, shown: t("searchFilters.result.shown", { returned: 3 }) }),
    );

    const page = searchPage({ resolvedQuery: "ödeme", mode: "exact" });
    expect(totalSummary(page, { status: "counting" })).toBe(t("searchFilters.total.counting"));
    expect(totalSummary(page, { status: "failed", message: "zaman aşımı" })).toBe(
      t("searchFilters.total.failed", { message: "zaman aşımı" }),
    );
    expect(
      totalSummary(page, { status: "done", total: 7, tookMs: 12.6, resolvedQuery: "kira", mode: "exact" }),
    ).toBe(t("searchFilters.total.staleQuery"));
    const done = totalSummary(page, {
      status: "done",
      total: 7,
      tookMs: 12.6,
      resolvedQuery: "ödeme",
      mode: "exact",
    });
    expect(done).toBe(t("searchFilters.total.done", { total: 7, ms: 13 }));
    expect(done).not.toContain("{");
  });

  it("aday penceresi satırı ölçümü sözlükten yazar", () => {
    const base = t("searchFilters.verification.base", { candidates: 40, ms: 12 });
    expect(
      verificationSummary(
        searchPage({ verification: { candidatesExamined: 40, windows: 2, scanMs: 12.4, stoppedEarly: false } }),
      ),
    ).toBe(t("searchFilters.verification.exhausted", { base }));
    expect(
      verificationSummary(
        searchPage({ verification: { candidatesExamined: 40, windows: 2, scanMs: 12.4, stoppedEarly: true } }),
      ),
    ).toBe(t("searchFilters.verification.stoppedEarly", { base }));
    expect(verificationSummary(searchPage())).toBeNull();
  });

  it("kapsam etiketleri, künye ve özet sözlükten", () => {
    expect(SEARCH_SCOPE_LABELS.case_documents).toBe(t("searchScope.label.case_documents"));
    expect(SEARCH_SCOPE_LABELS.precedent_library).toBe(t("searchScope.label.precedent_library"));
    expect(scopeNotice("precedent_library")).toBe(t("searchScope.notice.precedent_library"));
    expect(scopeNotice("case_documents")).toBe(t("searchScope.notice.case_documents"));

    const hit = precedentHit();
    const docket = t("searchScope.citationDocket", { no: "2024/123" });
    const decision = t("searchScope.citationDecision", { no: "2025/45" });
    expect(precedentCitation(hit)).toBe(`Yargıtay 1. Hukuk Dairesi, ${docket}, ${decision}, 2025-02-03`);
    expect(
      precedentCitation(precedentHit({ court: "  ", chamber: null, docketNo: null, decisionNo: null, decisionDate: null })),
    ).toBe(t("searchScope.citationMissing"));
    expect(precedentMatchLabel(hit)).toBe(t("searchScope.matchSource.text"));
    expect(precedentMatchLabel(precedentHit({ matchSource: "metadata" }))).toBe(
      t("searchScope.matchSource.metadata"),
    );
  });

  it("emsal özeti ve kopyalama metni sözlükten kurulur", () => {
    const base = t("searchScope.summary.base", { matched: 2, scanned: 4 });
    expect(precedentSearchSummary(precedentPage())).toBe(base);
    expect(precedentSearchSummary(precedentPage({ withoutText: 3 }))).toBe(
      t("searchScope.summary.withoutText", { base, count: 3 }),
    );
    const hit = precedentHit();
    const snippet = precedentSnippetForCopy(hit);
    expect(snippet).toBe(
      t("searchScope.snippetForCopy", {
        citation: precedentCitation(hit),
        snippet: hit.snippet,
        version: "12345678",
      }),
    );
    // Üç satır korunur: künye + pencere + kaynak; tek satıra inmez.
    expect(snippet.split("\n")).toHaveLength(3);
    expect(snippet).not.toContain("{");
  });

  it("kip ipucu otomatik çözümü sözlükten yazar", () => {
    expect(searchModeHint("ödeme", "auto")).toBe(
      `${t("searchFilters.modeHint.autoPrefix", { label: SEARCH_MODE_LABELS.flexible })} ${t("searchFilters.modeHint.flexible")}`,
    );
    expect(searchModeHint('"kira bedeli"', "auto")).toContain(t("searchFilters.mode.phrase"));
    expect(searchModeHint("2024/123 E.", "auto")).toContain(t("searchFilters.mode.docket"));
    expect(searchModeHint("ödeme", "exact")).toBe(t("searchFilters.modeHint.exact"));
  });
});

describe("Ayarlar ekranı ve geri dönüş yardımcıları sözlükten gelir", () => {
  // Ekranın kendi metni `localization.test.ts`te (Türkçe harf kapısı) ve
  // `tsc`te (anahtar tipi) tutuluyor; burada **üretilen** metnin birebir
  // sözlükten geldiği sınanır. Metin lib'e geri gömülürse kırmızı olur.
  function preUpdate(overrides: Partial<PreUpdateBackup> = {}): PreUpdateBackup {
    return {
      fileName: "vault.db.v12-pre-update.bak",
      claimedVersion: 12,
      verifiedVersion: 12,
      sizeBytes: 4096,
      modifiedAt: "2026-10-24T09:00:00Z",
      problem: null,
      usable: true,
      nameMismatch: false,
      ...overrides,
    };
  }

  it("rozet ve durum satırı sözlükten kurulur", () => {
    expect(backupVersionLabel(preUpdate({ verifiedVersion: null }))).toBe(
      t("rollback.versionUnverified", { version: 12 }),
    );
    expect(backupStatusText(preUpdate())).toBe(t("rollback.verifiedSchema", { version: 12 }));
    expect(backupStatusText(preUpdate({ claimedVersion: 3, verifiedVersion: 17, nameMismatch: true }))).toBe(
      t("rollback.verifiedMismatch", { verified: 17, claimed: 3 }),
    );
    expect(backupStatusText(preUpdate({ usable: false, problem: "E_DB_MIGRATION_BACKUP" }))).toBe(
      t("rollback.unusableProblem", { problem: "E_DB_MIGRATION_BACKUP" }),
    );
    expect(backupStatusText(preUpdate({ usable: false, verifiedVersion: null }))).toBe(
      t("rollback.unusable"),
    );
    expect(backupWarnings(preUpdate({ claimedVersion: 3, verifiedVersion: 17, nameMismatch: true }))[0]).toBe(
      t("rollback.nameMismatch", { claimed: 3, verified: 17 }),
    );
  });

  it("geri dönüş sınırları ve gerekçeler sözlükten, parametreler dolar", () => {
    const lines = rollbackConsequences();
    expect(lines).toHaveLength(6);
    expect(lines[0]).toBe(t("rollback.consequence1"));
    expect(lines[5]).toBe(t("rollback.consequence6"));
    expect(lines.join(" ")).not.toContain("{");

    expect(rollbackDisabledReason([], "", "")).toBe(t("rollback.noBackups"));
    expect(rollbackDisabledReason([preUpdate()], "", "")).toBe(t("rollback.selectFirst"));
    expect(rollbackDisabledReason([preUpdate({ usable: false })], preUpdate().fileName, "")).toBe(
      t("rollback.notVerifiedSelected"),
    );
    const confirmLine = rollbackDisabledReason([preUpdate()], preUpdate().fileName, "");
    expect(confirmLine).toBe(t("rollback.confirmRequired", { token: ROLLBACK_CONFIRMATION }));
    expect(confirmLine).not.toContain("{");
    expect(rollbackDisabledReason([preUpdate()], preUpdate().fileName, ROLLBACK_CONFIRMATION)).toBeNull();

    expect(
      rollbackSummary({
        backupFile: "vault.db.v12-pre-update.bak",
        restoredVersion: 12,
        resumedVersion: 17,
        safetyCopy: "/tmp/vault.db.rollback-1.safety",
      }),
    ).toBe(t("rollback.summary", { restored: 12, resumed: 17 }));
  });

  it("onay belirteci çekirdekteki sabitle birebir aynı (sözlük kayması yakalanır)", () => {
    // Sözlük tek kaynak olduğu için belirteç artık buradan gelir; Rust tarafı
    // `commands.rs`te sabittir. İkisi ayrışırsa geri dönüş **hiç** çalışmazdı.
    const rust = readFileSync(
      fileURLToPath(new URL("../../src-tauri/src/commands.rs", import.meta.url)),
      "utf8",
    );
    expect(rust).toContain(`pub const ROLLBACK_CONFIRMATION: &str = "${ROLLBACK_CONFIRMATION}";`);
  });

  it("Ayarlar metinleri parametreleri doldurur, yarım yer tutucu bırakmaz", () => {
    expect(t("settings.versionValue", { version: "0.1.0" })).toBe("0.1.0 (pre-alpha)");
    expect(t("settings.autoDaysValue", { days: 7 })).toBe("7 gün");
    expect(t("settings.gcCandidates", { count: 3, size: "1,5 KiB" })).toBe("3 aday · 1,5 KiB");
    const summary = t("settings.backupSummary", { count: 12, size: "2,0 MiB", hash: "abcdef0123456789" });
    expect(summary).toContain("abcdef0123456789");
    expect(summary).not.toContain("{");
    const restored = t("settings.restoreVerifiedTail", { size: "2,0 MiB", ms: 8 });
    expect(restored).toContain("sha256");
    expect(restored).not.toContain("{");
  });
});

describe("Notlar bileşeni ve yardımcıları sözlükten gelir", () => {
  it("taslak durumu ve revizyon kaynağı etiketleri sözlükten kurulur", () => {
    // `DRAFT_STATUS_LABEL` / `REVISION_ORIGIN_LABEL` modül sabitleri de sözlükten
    // türetilir; metin lib'e geri gömülürse burada kırmızı olur.
    expect(DRAFT_STATUS_LABEL.idle).toBe(t("notes.draftIdle"));
    expect(DRAFT_STATUS_LABEL.saving).toBe(t("notes.draftSaving"));
    expect(DRAFT_STATUS_LABEL.saved).toBe(t("notes.draftSaved"));
    expect(DRAFT_STATUS_LABEL.failed).toBe(t("notes.draftFailed"));
    expect(REVISION_ORIGIN_LABEL.create).toBe(t("notes.originCreate"));
    expect(REVISION_ORIGIN_LABEL.edit).toBe(t("notes.originEdit"));
    expect(REVISION_ORIGIN_LABEL.undo).toBe(t("notes.originUndo"));
    expect(REVISION_ORIGIN_LABEL.redo).toBe(t("notes.originRedo"));
    expect(revisionOriginLabel("bilinmeyen")).toBe(t("notes.originUnknown"));
    expect(CONFLICT_HINT).toBe(t("notes.conflictHint"));
  });

  it("çakışma ve sürüm satırları parametreleri doldurur", () => {
    const stats = t("notes.conflictStats", { mine: 3, theirs: 2 });
    expect(stats).toContain("3");
    expect(stats).toContain("2");
    expect(stats).not.toContain("{");
    const meta = t("notes.editMeta", { version: 4, revision: 2, time: "24.10.2026 09:00" });
    expect(meta).toContain("24.10.2026 09:00");
    expect(meta).not.toContain("{");
    const current = t("notes.currentVersion", { version: 4, time: "24.10.2026 09:00" });
    expect(current).toContain("4");
    expect(current).not.toContain("{");
    const draftDiffers = t("notes.draftDiffers", { time: "24.10.2026 09:00" });
    expect(draftDiffers).toContain("24.10.2026 09:00");
    expect(draftDiffers).not.toContain("{");
    const cardSub = t("notes.cardSub", { count: 7 });
    expect(cardSub.startsWith("7 ")).toBe(true);
    expect(cardSub).not.toContain("{");
    expect(t("notes.versionLabel", { version: 12 })).toBe("sürüm 12");
  });

  it("kasa girişi gerekçeleri sözlükten gelir (Unlock ekranıyla tek kaynak)", () => {
    const short = checkPassword("kisa");
    expect(short).toEqual({ ok: false, reason: t("vaultInput.tooShort", { min: MIN_PASSWORD_LENGTH }) });
    expect(checkPassword("avukat123456")).toEqual({ ok: false, reason: t("vaultInput.common") });
    expect(checkPassword("aaaaaaaaaaaaaa")).toEqual({ ok: false, reason: t("vaultInput.repeated") });
    expect(checkPassword("1234567890123")).toEqual({ ok: false, reason: t("vaultInput.digitsOnly") });
    expect(checkPasswordConfirmation("a", "b")).toEqual({ ok: false, reason: t("vaultInput.mismatch") });
    expect(checkPassword("Kira-2026-dosya")).toEqual({ ok: true });
  });

  // Dosyalar ekranı (F04-T08): satırların hepsi parametreli; eksik parametre
  // `t()` tarafından hata olarak atılır, ekranda yarım cümle kalmaz.
  it("Dosyalar ekranı satırları sözlükten ve parametreleri doldurulmuş gelir", () => {
    const sub = t("cases.listSubtitle", { count: 7, scope: t("cases.scopeActive") });
    expect(sub.startsWith("7 ")).toBe(true);
    expect(sub).toContain("·");
    expect(sub).toContain(t("cases.scopeActive"));
    expect(sub).not.toContain("{");

    const scopeAll = t("cases.listSubtitle", { count: 2, scope: t("cases.scopeArchived") });
    expect(scopeAll).toContain(t("cases.scopeArchived"));
    expect(scopeAll).not.toBe(sub);

    const summary = t("cases.parseSummary", { status: "completed", pages: 3, segments: 12 });
    expect(summary).toContain("3");
    expect(summary).toContain("12");
    expect(summary).not.toContain("{");

    const detail = t("cases.batchDetail", { pages: 4, peak: 2 });
    expect(detail).toContain("4");
    expect(detail).toContain("(en fazla 2)");
    expect(detail).not.toContain("{");

    const result = t("cases.batchResult", { succeeded: 2, requested: 3 });
    expect(result).toContain("2");
    expect(result).toContain("3");
    expect(result).not.toContain("{");

    const imported = t("cases.imported", { name: "kira.pdf", version: 3 });
    expect(imported).toBe("“kira.pdf” içe alındı (sürüm 3).");

    const tagTitle = t("cases.removeTagTitle", { tag: "kira" });
    expect(tagTitle).toBe("“kira” etiketini kaldır");
    expect(t("cases.tagAria", { name: "kira.pdf" })).toBe("kira.pdf için etiket");
  });

  it("Dosyalar ekranı OCR ipucu dil adını sözlükten alır", () => {
    const title = t("cases.ocrTitle", { lang: defaultOcrLanguageLabel() });
    expect(title).toBe("Taranmış sayfalar için OCR (Türkçe + İngilizce)");
    expect(title).not.toContain("{");
  });

  it("Dosyalar ekranının sözlük anahtarları eksiksizdir", () => {
    // Ekranda geçen her `cases.*` anahtarı sözlükte var olmalı; silinen bir
    // anahtar ekranı çalışma zamanında patlatırdı.
    const source = readFileSync(
      fileURLToPath(new URL("../screens/Cases.tsx", import.meta.url)),
      "utf8",
    );
    const used = [...source.matchAll(/t\("(cases\.[a-zA-Z0-9_.]+)"/g)].map((m) => m[1]!);
    expect(used.length).toBeGreaterThan(40);
    // Parametre isteyen anahtarlar parametresiz çağrıldığında bilinçli olarak
    // hata atar; burada aranan şey **varlık**, o yüzden sözlük anahtar listesi
    // kullanılır.
    const known = new Set(messageKeys());
    const missing = used.filter((key) => !known.has(key as never));
    expect(missing).toEqual([]);
  });
});

describe("Emsal kümesi (ekran + üç yardımcı lib) sözlükten gelir", () => {
  function termsRow(overrides: Partial<PrecedentUsageTerms>): PrecedentUsageTerms {
    return {
      precedentId: "p1",
      usageBasis: "unknown",
      redistribution: "unknown",
      termsUrl: null,
      termsNote: "",
      termsRecordedAt: null,
      ...overrides,
    };
  }

  function coverage(overrides: Partial<CorpusCoverage>): CorpusCoverage {
    return {
      total: 0,
      withText: 0,
      allowed: 0,
      notAllowed: 0,
      unknown: 0,
      blockedTotal: 0,
      blocked: [],
      ...overrides,
    };
  }

  function verification(overrides: Partial<QuoteVerificationRow>): QuoteVerificationRow {
    return {
      precedentId: "p1",
      versionId: "pv1",
      state: "not_checked",
      method: null,
      matchedStart: null,
      matchedLength: null,
      occurrences: 0,
      reason: null,
      versionsScanned: 0,
      historicalVersionsScanned: 0,
      foundInOldVersion: false,
      historicalTruncated: false,
      quote: "itirazın reddine",
      checkedAt: "2026-10-24T00:00:00Z",
      ...overrides,
    };
  }

  function documentText(overrides: Partial<PrecedentDocumentText>): PrecedentDocumentText {
    return {
      version: {
        id: "pv1",
        precedentId: "p1",
        documentVersionId: "dv1",
        acquiredAt: "2026-10-24T09:15:00Z",
        sourceKind: "document",
        bodyHash: "abcd1234ef567890".padEnd(64, "0"),
        verificationState: "text_verified",
        charCount: 8412,
        hasText: true,
        identityConflict: false,
        conflictWithVersionId: null,
      },
      parseRunId: "run-1",
      engine: "pymupdf",
      parseStatus: "succeeded",
      pages: 12,
      ocrPages: 0,
      emptyPages: 0,
      charCount: 8412,
      written: true,
      notes: [],
      ...overrides,
    };
  }

  it("kullanım koşulu metinleri sözlükten kurulur", () => {
    expect(usageBasisLabel("licence")).toBe(t("terms.basisLicence"));
    expect(usageBasisLabel("internal_only")).toBe(t("terms.basisInternalOnly"));
    expect(redistributionLabel("allowed")).toBe(t("terms.redistributionAllowed"));
    expect(termsBadge({ redistribution: "not_allowed" }).text).toBe(t("terms.badgeNotAllowed"));
    // Kayıt yokluğu izin sayılmaz; metin sözlükten gelir, uydurulmaz.
    expect(termsRecordedText(termsRow({}))).toBe(t("terms.recordedNone"));
    expect(termsRecordedText(termsRow({ termsRecordedAt: "2026-10-24T15:03:21+03:00" }))).toBe(
      t("terms.recordedAt", { at: "2026-10-24 15:03" }),
    );
  });

  it("kapsam ve kapı cümleleri sözlük parçalarından birleşir", () => {
    const covered = coverage({
      total: 5,
      withText: 3,
      allowed: 1,
      notAllowed: 1,
      unknown: 1,
      blockedTotal: 2,
    });
    expect(coverageText(covered)).toBe(
      [
        t("terms.coverageTotal", { total: 5 }),
        t("terms.coverageWithText", { withText: 3 }),
        t("terms.coverageAllowed", { allowed: 1 }),
        t("terms.coverageNotAllowed", { notAllowed: 1 }),
        t("terms.coverageUnknown", { unknown: 1 }),
      ].join(" · "),
    );
    expect(corpusGateText(coverage({ withText: 0 }))).toBe(t("terms.gateNoText"));
    expect(
      corpusGateText(coverage({ total: 3, withText: 3, allowed: 1, unknown: 2, blockedTotal: 2 })),
    ).toBe(t("terms.gateBlocked", { blocked: 2 }));
    expect(exclusionReasonLabel("redistribution_not_allowed")).toBe(
      t("terms.exclusionNotAllowed"),
    );
    expect(exclusionReasonLabel("redistribution_unknown")).toBe(t("terms.exclusionUnknown"));
  });

  it("aktarım sonucu sayı ve hash ile sözlükten yazılır", () => {
    expect(
      corpusResultText({
        path: "/tmp/k.json",
        bytes: 1,
        sha256: "abcdef0123456789",
        exported: 2,
        excluded: 1,
        contentHash: "ffff",
      }),
    ).toBe(
      [
        t("terms.resultExported", { exported: 2 }),
        t("terms.resultExcluded", { excluded: 1 }),
        t("terms.resultHash", { hash: "abcdef012345" }),
      ].join(" · "),
    );
  });

  it("alıntı durumu cümlesi sözlük parçalarından kurulur", () => {
    expect(quoteStateText(verification({ state: "match", method: "exact" }))).toBe(
      t("precedentLib.quoteExact", {
        where: t("precedentLib.quoteWhereCurrent"),
        at: "",
        repeated: "",
        old: "",
      }),
    );

    // Eski sürümde bulunan alıntı: konum, tekrar ve uyarı eki birlikte kurulur.
    expect(
      quoteStateText(
        verification({
          state: "match",
          method: "exact",
          matchedStart: 4,
          occurrences: 2,
          foundInOldVersion: true,
        }),
      ),
    ).toBe(
      t("precedentLib.quoteExact", {
        where: t("precedentLib.quoteWhereOld"),
        at: ` ${t("precedentLib.quoteAt", { position: "5" })}`,
        repeated: ` · ${t("precedentLib.quoteRepeated", { count: 2 })}`,
        old: ` ${t("precedentLib.quoteOldSuffix")}`,
      }),
    );

    expect(quoteStateText(verification({ state: "mismatch" }))).toBe(
      t("precedentLib.quoteMismatch", {
        scope: t("precedentLib.quoteMismatchCurrent"),
        cut: "",
      }),
    );
    expect(
      quoteStateText(verification({ state: "not_checked", reason: "quote_too_short" })),
    ).toBe(t("precedentLib.quoteTooShort"));
    expect(quoteStateText(verification({ state: "not_checked", reason: "no_full_text" }))).toBe(
      t("precedentLib.quoteNoText"),
    );
  });

  it("tarama, kaynak ve toplu içe alma etiketleri sözlükten", () => {
    expect(
      quoteScanNote(
        verification({ state: "mismatch", versionsScanned: 3, historicalVersionsScanned: 2 }),
      ),
    ).toBe(
      [
        t("precedentLib.scanVersions", { count: 3 }),
        t("precedentLib.scanHistorical", { count: 2 }),
      ].join(" · "),
    );
    expect(versionSourceLabel({ sourceKind: "document" })).toBe(
      t("precedentLib.sourceDocument"),
    );
    expect(
      bulkOutcomeLabel({ outcome: "duplicate", code: null, message: null, charCount: 1 }),
    ).toBe(t("precedentLib.bulkDuplicate"));
    expect(
      bulkReportSummary({ created: 3, duplicates: 1, conflicts: 1, failed: 0 }),
    ).toBe(
      [
        t("precedentLib.bulkReportCreated", { count: 3 }),
        t("precedentLib.bulkReportDuplicates", { count: 1 }),
        t("precedentLib.bulkReportConflicts", { count: 1 }),
      ].join(" · "),
    );
    expect(versionConflictText({ identityConflict: true })).toBe(t("precedentLib.conflictText"));
    expect(bibliographyMissingText([])).toBe(t("precedentLib.bibliographyComplete"));
    expect(bibliographyMissingText(["karar tarihi"])).toBe(
      t("precedentLib.bibliographyMissing", { fields: "karar tarihi" }),
    );
    expect(accessBasisLabel("version")).toBe(t("precedentLib.accessBasisVersion"));
    expect(versionStateText({ hasText: false, charCount: 0 })).toBe(
      t("precedentLib.versionNoText"),
    );
  });

  it("kasadan aktarma metinleri sözlükten", () => {
    expect(versionOptionLabel({ versionNo: 7, importedAt: "bozuk" })).toBe(
      t("precedentDoc.versionOption", { version: "v7", at: "—" }),
    );
    expect(
      transferBlockedReason({
        hasPrecedent: false,
        documentCount: 1,
        versionCount: 1,
        selectedVersionId: "x",
      }),
    ).toBe(t("precedentDoc.blockedNoPrecedent"));
    expect(
      transferBlockedReason({
        hasPrecedent: true,
        documentCount: 0,
        versionCount: 1,
        selectedVersionId: "x",
      }),
    ).toBe(t("precedentDoc.blockedNoDocument"));
    expect(
      transferBlockedReason({
        hasPrecedent: true,
        documentCount: 1,
        versionCount: 0,
        selectedVersionId: "x",
      }),
    ).toBe(t("precedentDoc.blockedNoVersion"));
    expect(
      transferBlockedReason({
        hasPrecedent: true,
        documentCount: 1,
        versionCount: 1,
        selectedVersionId: "   ",
      }),
    ).toBe(t("precedentDoc.blockedNoSelection"));
    expect(documentTextSummary(documentText({}))).toBe(
      t("precedentDoc.summary", {
        at: formatTimestamp("2026-10-24T09:15:00Z"),
        pages: 12,
        characters: 8412,
        hash: "abcd1234ef56",
      }),
    );
    expect(documentTextWarnings(documentText({ parseStatus: "partial" }))).toEqual([
      t("precedentDoc.warnPartial"),
    ]);
    expect(documentTextWarnings(documentText({ ocrPages: 3 }))).toEqual([
      t("precedentDoc.warnOcr", { pages: 3 }),
    ]);
    expect(documentTextWarnings(documentText({ emptyPages: 2 }))).toEqual([
      t("precedentDoc.warnEmpty", { pages: 2 }),
    ]);
    expect(documentTextWarnings(documentText({ written: false }))).toEqual([
      t("precedentDoc.warnDuplicate"),
    ]);
  });

  it("Emsal ekranının sözlük anahtarları eksiksizdir", () => {
    // Ekranda `t("…")` ile çağrılan her anahtar sözlükte var olmalı; silinen bir
    // anahtar ekranı çalışma zamanında patlatırdı.
    const source = readFileSync(
      fileURLToPath(new URL("../screens/Precedents.tsx", import.meta.url)),
      "utf8",
    );
    const used = [
      ...source.matchAll(/t\("((?:precedents|precedentLib|terms|precedentDoc)\.[a-zA-Z0-9_.]+)"/g),
    ].map((match) => match[1]!);
    expect(used.length).toBeGreaterThan(100);
    const known = new Set(messageKeys());
    const missing = used.filter((key) => !known.has(key as never));
    expect(missing).toEqual([]);
  });
});

describe("Sağlayıcılar kümesi (F04-T08)", () => {
  it("mimari denetimi metinleri sözlükten gelir", () => {
    expect(archBadgeText({ comparison: "same", containsAppArch: true })).toBe(t("arch.badgeSame"));
    expect(archBadgeText({ comparison: "fat", containsAppArch: false })).toBe(
      t("arch.badgeFatWithoutApp"),
    );
    expect(archBadgeText({ comparison: "bilinmeyen", containsAppArch: null })).toBe(
      t("arch.badgeUnknown"),
    );
    expect(archWorst({ same: 0, different: 1, fat: 0, unknown: 0 }).label).toBe(
      t("arch.worstDifferent"),
    );
    expect(archWorst({ same: 0, different: 0, fat: 0, unknown: 0 }).label).toBe(t("arch.worstNone"));

    const summary = archSummaryText({
      appArch: "aarch64",
      same: 2,
      different: 1,
      fat: 0,
      unknown: 0,
      adapters: [{}, {}] as never,
    });
    expect(summary).toBe(
      t("arch.summary", {
        appArch: "aarch64",
        adapters: 2,
        same: 2,
        different: 1,
        fat: 0,
        unknown: 0,
      }),
    );
    expect(summary).not.toContain("{");

    expect(
      archWarnings({
        adapters: [
          { displayName: "claude", comparison: "different", containsAppArch: null },
        ] as never,
        different: 1,
        unknown: 0,
        fat: 0,
      }),
    ).toEqual([t("arch.warnDifferent", { count: 1, names: "claude" })]);

    expect(archReasonText("magic_unrecognized")).toBe(t("arch.reasonMagicUnrecognized"));
    expect(archReasonText("yeni_kod")).toBe("yeni_kod");
    expect(archFormatText({ formatLabel: "Mach-O", symlink: true })).toBe(
      `Mach-O · ${t("arch.symlink")}`,
    );
    expect(
      archInterpreterText({
        interpreter: {
          command: "/usr/bin/env",
          path: null,
          resolved: "/usr/bin/python3",
          architectures: ["arm64"],
        },
      } as never),
    ).toBe(
      `${t("arch.interpreterLine", {
        command: "/usr/bin/env",
        target: "/usr/bin/python3",
      })} · arm64`,
    );
  });

  it("paket doğrulama metinleri sözlükten gelir", () => {
    expect(packageStatusText(null)).toBe(t("package.statusUnverified"));
    expect(
      packageSummary({
        totals: { files: 118, symlinks: 18, bytes: 4096 },
        roles: {},
      } as never),
    ).toBe(
      t("package.summaryLine", {
        head: t("package.summaryFilesLinks", { files: 118, links: 18 }),
        size: formatBytes(4096),
      }),
    );
    expect(roleLabel("bootloader")).toBe(t("package.role.bootloader"));
    expect(roleLabel("bilinmeyen_rol")).toBe("bilinmeyen_rol");
    expect(problemText({ problem: "hashChanged", detail: null } as never)).toBe(
      t("package.problem.hashChanged"),
    );
    expect(problemText({ problem: "hashChanged", detail: "12 B" } as never)).toBe(
      t("package.problemWithDetail", {
        label: t("package.problem.hashChanged"),
        detail: "12 B",
      }),
    );
    expect(
      packageWarnings({
        found: true,
        note: null,
        manifest: null,
        packageDir: null,
        tesseractOnPath: true,
        verification: {
          ok: false,
          truncated: false,
          verified: 3,
          problemCount: 2,
          extraCount: 0,
          entryPointPresent: true,
          problems: [
            { path: "dim-worker", problem: "hashChanged" },
            { path: "libx.so", problem: "missing" },
          ],
        },
      } as never),
    ).toEqual([t("package.warnProblems", { count: 2 })]);
    expect(packageLimits()).toBe(t("package.limits"));
  });

  it("saldırı testi metinleri sözlükten gelir", () => {
    expect(
      attackVerdictText({
        verdict: "triggered",
        verdictLabel: "ham",
        sourceLabel: "işaretçi dosyası",
        hit: true,
      }),
    ).toBe(t("attack.verdictTriggered", { source: "işaretçi dosyası" }));
    expect(
      attackVerdictText({
        verdict: "silent_guarded",
        verdictLabel: "ham",
        sourceLabel: "x",
        hit: false,
      }),
    ).toBe(t("attack.verdictSilentGuarded"));
    expect(
      attackFixtureSummary({
        detectorProven: true,
        pollutedHits: 3,
        cleanHits: 0,
        vectorTotal: 4,
        gateRejected: true,
        gateCode: "E_SCOPE",
        detectorControlClean: true,
        productConfig: [],
        canaryReadInProductConfig: false,
      } as never),
    ).toBe(
      t("attack.fixtureSummary", {
        proven: t("attack.provenYes"),
        polluted: 3,
        total: 4,
        clean: 0,
        gate: t("attack.gateRejectedCode", { code: "E_SCOPE" }),
      }),
    );
    expect(attackSkipText("not_installed")).toBe(t("attack.skipNotInstalled"));
    expect(attackSkipText("beklenmeyen")).toBe(t("attack.skipOther", { code: "beklenmeyen" }));
    expect(attackSkipText(null)).toBeNull();
    expect(attackProductText({ label: "Hook", hit: true, vector: "hook" } as never)).toBe(
      t("attack.productLine", { label: "Hook", state: t("attack.productHit") }),
    );
    expect(attackCommand({ version: "2.1.303", argv: ["--x", ""] } as never)).toBe(
      '2.1.303 · --x ""',
    );
    expect(attackWorstVerdict([]).label).toBe(t("attack.worstNone"));
  });

  it("yoklama, kalıntı ve yetenek metinleri sözlükten gelir", () => {
    expect(probeStatusLabel({ status: "exited", exitCode: 0 })).toEqual({
      label: t("probe.statusDone"),
      tone: "ok",
    });
    expect(probeStatusLabel({ status: "exited", exitCode: 3 }).label).toBe(
      t("probe.statusExit", { code: 3 }),
    );
    expect(probeSummary({ exitCode: null, durationMs: 120, timeoutMs: 15000 })).toBe(
      [t("probe.summaryDuration", { ms: 120 }), t("probe.summaryTimeout", { seconds: 15 })].join(
        " · ",
      ),
    );

    const sandbox = {
      workdirMode: "700",
      homeIsolation: "sandbox",
      stdinBytes: 0,
      stdinLimitBytes: 1_048_576,
      files: [],
      totalBytes: 0,
      sizeLimitBytes: 0,
      entryLimit: 100,
      filesTruncated: false,
    };
    expect(probeScopeLines({ sandbox } as never).map((line) => line.label)).toEqual([
      t("probe.scopePermLabel"),
      t("probe.scopeHomeLabel"),
      t("probe.scopeStdinLabel"),
      t("probe.scopeFilesLabel"),
    ]);
    expect(probeScopeLines({ sandbox } as never)[3]?.value).toBe(t("probe.filesNone"));

    expect(residueAge(30)).toBe(t("probe.residueAgeNow"));
    expect(residueAge(7200)).toBe(t("probe.residueAgeHours", { count: 2 }));
    expect(residueAge(null)).toBe(t("common.unknown"));

    expect(
      residueMeasureSummary({
        durationMs: 1200,
        created: [{ relative: "a" }],
        modified: [],
        bytesAdded: 2048,
      } as never),
    ).toBe(
      [
        t("probe.summaryDuration", { ms: 1200 }),
        t("probe.summaryCreated", { count: 1 }),
        t("probe.summaryModified", { count: 0 }),
        t("probe.summaryAdded", { size: formatBytes(2048) }),
      ].join(" · "),
    );
    expect(
      sandboxSweepSummary({ considered: 4, removed: [], skipped: [], minAgeSecs: 3600 } as never),
    ).toBe(t("probe.sweep", { considered: 4, removed: 0, skipped: 0, minutes: 60 }));
    expect(capabilityGateText({ gateOpen: true } as never)).toBe(t("probe.gateOpen"));
    expect(capabilityGateText({ gateOpen: false, link: "not_installed" } as never)).toBe(
      t("probe.gateNotInstalled"),
    );
  });

  it("parametreli sağlayıcı satırları yarım yer tutucu bırakmaz", () => {
    const lines = [
      t("providers.matrixProbed", { at: "10:42", seconds: 10 }),
      t("providers.archProbed", { at: "10:42", summary: "özet" }),
      t("providers.roleRow", { label: "Modül", count: 7 }),
      t("providers.cleanupRun", { count: 3 }),
      t("providers.createdFiles", { count: 2 }),
      t("providers.headerBytes", { bytes: 32 }),
      t("providers.sandboxFiles", { count: 1 }),
      t("package.warnListCut", { listed: 5, total: 9 }),
      t("arch.warnUnknown", { names: "x, y" }),
      t("probe.persistenceLine", { flag: "--no-session", scope: "claude", version: "2.1" }),
    ];
    for (const line of lines) {
      expect(line).not.toContain("{");
      expect(line.trim().length).toBeGreaterThan(0);
      expect(line).not.toContain("  ");
    }
  });

  it("Sağlayıcılar ekranı ve dört lib'in sözlük anahtarları eksiksizdir", () => {
    const targets: Array<[string, RegExp]> = [
      ["../screens/Providers.tsx", /t\("((?:providers|probe)\.[a-zA-Z0-9_.]+)"/g],
      ["../lib/providers.ts", /t\("((?:probe|common)\.[a-zA-Z0-9_.]+)"/g],
      ["../lib/arch.ts", /t\("((?:arch|common)\.[a-zA-Z0-9_.]+)"/g],
      ["../lib/package.ts", /t\("((?:package|common)\.[a-zA-Z0-9_.]+)"/g],
      ["../lib/attack.ts", /t\("((?:attack|common)\.[a-zA-Z0-9_.]+)"/g],
    ];
    const known = new Set(messageKeys());
    for (const [relative, pattern] of targets) {
      const source = readFileSync(fileURLToPath(new URL(relative, import.meta.url)), "utf8");
      const used = [...source.matchAll(pattern)].map((match) => match[1]!);
      expect(used.length).toBeGreaterThan(10);
      const missing = used.filter((key) => !known.has(key as never));
      expect({ relative, missing }).toEqual({ relative, missing: [] });
    }
  });
});
