import { describe, expect, it } from "vitest";

import type { JobQueueSummary, JobRow } from "./api";
import {
  countOf,
  jobMetaLine,
  jobProgressPercent,
  jobStateBadgeClass,
  jobStateLabel,
  jobTypeLabel,
  queueAttention,
  queueSummaryLine,
  recoveryLine,
} from "./jobs";

function summary(partial: Partial<JobQueueSummary> = {}): JobQueueSummary {
  return {
    total: 0,
    byState: [],
    dueNow: 0,
    oldestQueuedAt: null,
    blockedByDependency: 0,
    ...partial,
  };
}

function job(partial: Partial<JobRow> = {}): JobRow {
  return {
    id: "j1",
    jobType: "parse_document",
    scope: "case",
    caseId: "c1",
    state: "queued",
    priority: 0,
    attemptCount: 0,
    maxAttempts: 3,
    progress: 0,
    nextRunAt: null,
    leaseOwner: null,
    leaseUntil: null,
    cancelRequestedAt: null,
    errorCode: null,
    createdAt: "2026-10-26T11:05:33Z",
    updatedAt: "2026-10-26T11:05:33Z",
    completedAt: null,
    ...partial,
  };
}

describe("iş kuyruğu yardımcıları", () => {
  it("boş kuyruğu sıfır uydurmadan yazar", () => {
    expect(queueSummaryLine(summary())).toBe("Kuyruk boş — bekleyen iş yok.");
    expect(queueAttention(summary())).toBeNull();
  });

  it("özet satırı yalnız sıfırdan büyük durumları sayar", () => {
    const line = queueSummaryLine(
      summary({
        total: 4,
        byState: [
          { state: "succeeded", count: 2 },
          { state: "queued", count: 1 },
          { state: "failed", count: 1 },
        ],
        dueNow: 1,
      }),
    );
    expect(line).toContain("4 iş");
    expect(line).toContain("1 bekliyor");
    expect(line).toContain("2 bitti");
    expect(line).toContain("1 başarısız");
  });

  it("başarısız ve bağımlılık durumlarını gerekçesiyle bildirir", () => {
    const text = queueAttention(
      summary({
        total: 3,
        byState: [
          { state: "failed", count: 1 },
          { state: "retry_wait", count: 1 },
        ],
        blockedByDependency: 1,
      }),
    );
    expect(text).toContain("1 iş başarısız");
    expect(text).toContain("bağımlılığı bittiği için çalıştırılmadı");
    expect(text).toContain("tekrar denemesi bekliyor");
  });

  it("bilinmeyen durum adını gizlemez", () => {
    expect(jobStateLabel("blocked")).toBe("bilinmeyen (blocked)");
    expect(jobStateLabel("running")).toBe("Yürüyor");
    expect(jobTypeLabel("parse_document")).toBe("Belge ayrıştırma");
    expect(jobTypeLabel("embedding")).toBe("embedding");
  });

  it("rozet sınıfı durumla tutarlı", () => {
    expect(jobStateBadgeClass("succeeded")).toBe("ok");
    expect(jobStateBadgeClass("failed")).toBe("danger");
    expect(jobStateBadgeClass("running")).toBe("warn");
    expect(jobStateBadgeClass("queued")).toBe("");
  });

  it("deneme satırı hata kodunu ve zamanı taşır", () => {
    const line = jobMetaLine(
      job({ attemptCount: 2, maxAttempts: 3, errorCode: "E_WORKER_TIMEOUT" }),
    );
    expect(line).toBe("deneme 2/3 · 2026-10-26 11:05 · E_WORKER_TIMEOUT");
    expect(jobMetaLine(job())).toBe("deneme 0/3 · 2026-10-26 11:05");
  });

  it("ilerleme yüzdesi sınırlar içinde kalır", () => {
    expect(jobProgressPercent(job({ state: "succeeded", progress: 1 }))).toBe(100);
    expect(jobProgressPercent(job({ progress: 0.42 }))).toBe(42);
    expect(jobProgressPercent(job({ progress: 1.7 }))).toBe(100);
    expect(jobProgressPercent(job({ progress: -0.2 }))).toBe(0);
    expect(jobProgressPercent(job({ progress: Number.NaN }))).toBe(0);
  });

  it("sayım yardımcısı eksik durumda sıfır döner", () => {
    expect(countOf(summary(), "failed")).toBe(0);
    expect(
      countOf(summary({ byState: [{ state: "failed", count: 5 }] }), "failed"),
    ).toBe(5);
  });

  it("kurtarma satırı sıfırda 'bulunmadı' der, sayı uydurmaz", () => {
    expect(recoveryLine(0, 0)).toBe(
      "Yarıda kalmış iş yok; yeniden sıraya alınacak bir şey bulunmadı.",
    );
    expect(recoveryLine(2, 1)).toBe("2 iş yeniden sıraya alındı, 1 iş iptal edildi.");
  });
});
