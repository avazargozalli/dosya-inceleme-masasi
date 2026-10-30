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

import type { ExportCoverage, JobRetryView, JobRow, PrecedentSearchHit, PrecedentSearchPage, PreUpdateBackup, SearchPage } from "./api";
import { elapsedText, mibText, profileLimits, secondsText } from "./budget";
import { COVERAGE_EMBED_TARGETS, coverageHeadline, coverageRows } from "./export-coverage";
import { messageKeys, t } from "./i18n";
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
