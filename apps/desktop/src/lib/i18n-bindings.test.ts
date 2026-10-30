/**
 * F04-T08 — sözlük **bağı** kapısı (ekran/lib metni sözlükten mi geliyor?).
 *
 * `localization.test.ts` taşınmış dosyada gömülü Türkçe *harf* arar; burada
 * aranan şey başka: üretilen metnin **birebir sözlükten** gelmesi. Metin lib'e
 * geri gömülürse (kapı yalnız harfe baktığı için `bellek`, `dosya`, `OCR` gibi
 * Türkçe harf içermeyen sözcükler kaçar) bu test kırmızı olur.
 */
import { describe, expect, it } from "vitest";

import type { ExportCoverage, JobRetryView, JobRow } from "./api";
import { elapsedText, mibText, profileLimits, secondsText } from "./budget";
import { COVERAGE_EMBED_TARGETS, coverageHeadline, coverageRows } from "./export-coverage";
import { t } from "./i18n";
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
import { REMINDER_POLICY_LABEL, REMINDER_TEXT } from "./reminders";

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
