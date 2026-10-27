import { describe, expect, it } from "vitest";

import type { JobQueueSummary, JobRetryView, JobRow } from "./api";
import {
  blockedReasonLabel,
  cancelOutcomeLine,
  canonicalJobState,
  countOf,
  jobActionLabel,
  jobActions,
  jobIsTerminal,
  jobMetaLine,
  jobProgressCounters,
  jobProgressLine,
  jobProgressPercent,
  jobProgressWidth,
  jobStateBadgeClass,
  jobStateLabel,
  jobTypeLabel,
  queueAttention,
  queueHasLiveJobs,
  queueSummaryLine,
  recoveryLine,
  retryOutcomeLine,
  shouldPollQueue,
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
    progressStage: null,
    progressDone: null,
    progressTotal: null,
    nextRunAt: null,
    leaseOwner: null,
    leaseUntil: null,
    cancelRequestedAt: null,
    errorCode: null,
    blockedReason: null,
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

  it("iptal satırı yalnız gerçekleşen sonucu yazar", () => {
    expect(cancelOutcomeLine({ outcome: "canceled", workerStopped: false })).toBe(
      "İş iptal edildi (hiç çalışmadı).",
    );
    expect(cancelOutcomeLine({ outcome: "requested", workerStopped: true })).toContain(
      "süreç durduruldu",
    );
    expect(cancelOutcomeLine({ outcome: "requested", workerStopped: false })).toContain(
      "sonucu bekleniyor",
    );
    // Süreç durdurulduysa "bekleniyor" denmez; durmadıysa "tamam" denmez.
    expect(cancelOutcomeLine({ outcome: "requested", workerStopped: true })).not.toContain(
      "bekleniyor",
    );
    expect(cancelOutcomeLine({ outcome: "requested", workerStopped: false })).not.toContain(
      "durduruldu",
    );
  });

  it("özet satırı yalnız sıfırdan büyük durumları sayar", () => {
    const line = queueSummaryLine(
      summary({
        total: 4,
        byState: [
          { state: "completed", count: 2 },
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

  it("başarısız, engelli ve bağımlılık durumlarını gerekçesiyle bildirir", () => {
    const text = queueAttention(
      summary({
        total: 4,
        byState: [
          { state: "failed", count: 1 },
          { state: "retry_wait", count: 1 },
          { state: "blocked", count: 1 },
        ],
        blockedByDependency: 1,
      }),
    );
    expect(text).toContain("1 iş başarısız");
    expect(text).toContain("1 iş engelli");
    expect(text).toContain("bağımlılığı bittiği için hiç çalıştırılmadı");
    expect(text).toContain("tekrar denemesi bekliyor");
  });

  it("kesilmiş işi kurtarma düğmesine yönlendirir", () => {
    const text = queueAttention(
      summary({ total: 1, byState: [{ state: "interrupted", count: 1 }] }),
    );
    expect(text).toContain("kesintiye uğradı");
  });

  it("§17.2 durum adlarını tanır, eski adları yeni ada indirger", () => {
    expect(jobStateLabel("blocked")).toBe("Engelli");
    expect(jobStateLabel("validating")).toBe("Doğrulanıyor");
    expect(jobStateLabel("canceling")).toBe("İptal ediliyor");
    expect(jobStateLabel("partial")).toBe("Kısmi bitti");
    expect(jobStateLabel("completed")).toBe("Bitti");
    expect(jobStateLabel("canceled")).toBe("İptal edildi");
    expect(jobStateLabel("interrupted")).toBe("Kesildi");
    expect(canonicalJobState("succeeded")).toBe("completed");
    expect(canonicalJobState("cancelled")).toBe("canceled");
    expect(canonicalJobState("leased")).toBe("running");
    // Bilinmeyen ad gizlenmez.
    expect(jobStateLabel("weird")).toBe("bilinmeyen (weird)");
    expect(jobTypeLabel("parse_document")).toBe("Belge ayrıştırma");
    expect(jobTypeLabel("embedding")).toBe("embedding");
  });

  it("rozet sınıfı durumla tutarlı", () => {
    expect(jobStateBadgeClass("completed")).toBe("ok");
    expect(jobStateBadgeClass("succeeded")).toBe("ok");
    expect(jobStateBadgeClass("failed")).toBe("danger");
    expect(jobStateBadgeClass("running")).toBe("warn");
    expect(jobStateBadgeClass("validating")).toBe("warn");
    expect(jobStateBadgeClass("blocked")).toBe("warn");
    expect(jobStateBadgeClass("queued")).toBe("");
  });

  it("eylemler sunucu durumundan türetilir", () => {
    expect(jobActions(job({ state: "queued" }))).toEqual(["cancel"]);
    expect(jobActions(job({ state: "running" }))).toEqual(["cancel"]);
    expect(jobActions(job({ state: "validating" }))).toEqual(["cancel"]);
    expect(jobActions(job({ state: "blocked" }))).toEqual(["unblock", "cancel"]);
    expect(jobActions(job({ state: "failed" }))).toEqual(["retry"]);
    // Terminal durumlarda düğme yok: çalışmayan buton gösterilmez.
    expect(jobActions(job({ state: "completed" }))).toEqual([]);
    expect(jobActions(job({ state: "canceled" }))).toEqual([]);
    expect(jobActions(job({ state: "partial" }))).toEqual([]);
    expect(jobActionLabel("cancel")).toBe("İptal et");
    expect(jobActionLabel("retry")).toBe("Yeniden dene");
    expect(jobActionLabel("unblock")).toBe("Engeli kaldır");
  });

  it("engel nedeni okunur metne çevrilir, bilinmeyen neden gizlenmez", () => {
    expect(blockedReasonLabel(null)).toBeNull();
    expect(blockedReasonLabel("password")).toBe("parola bekliyor");
    expect(blockedReasonLabel("dependency")).toBe("bağımlılık bekliyor");
    expect(blockedReasonLabel("custom")).toBe("engel: custom");
  });

  it("deneme satırı hata kodunu, engel nedenini ve zamanı taşır", () => {
    const line = jobMetaLine(
      job({ attemptCount: 2, maxAttempts: 3, errorCode: "E_WORKER_TIMEOUT" }),
    );
    expect(line).toBe("deneme 2/3 · 2026-10-26 11:05 · E_WORKER_TIMEOUT");
    expect(jobMetaLine(job())).toBe("deneme 0/3 · 2026-10-26 11:05");
    expect(jobMetaLine(job({ state: "blocked", blockedReason: "password" }))).toBe(
      "deneme 0/3 · 2026-10-26 11:05 · parola bekliyor",
    );
  });

  it("ilerleme yüzdesi sınırlar içinde kalır", () => {
    expect(jobProgressPercent(job({ state: "completed", progress: 1 }))).toBe(100);
    expect(jobProgressPercent(job({ state: "succeeded", progress: 1 }))).toBe(100);
    expect(jobProgressPercent(job({ progress: 0.42 }))).toBe(42);
    expect(jobProgressPercent(job({ progress: 1.7 }))).toBe(100);
    expect(jobProgressPercent(job({ progress: -0.2 }))).toBe(0);
    expect(jobProgressPercent(job({ progress: Number.NaN }))).toBe(0);
  });

  it("canlı sayaç satırı aşama + sayı + yüzdeyi ayrıştırma satırıyla aynı dilde yazar", () => {
    const running = job({
      state: "running",
      progressStage: "pages",
      progressDone: 12,
      progressTotal: 40,
      progress: 0.3,
    });
    expect(jobProgressLine(running)).toBe("Sayfalar okunuyor: 12 / 40 · %30");
    expect(jobProgressWidth(running)).toBe(30);
    expect(jobProgressCounters(running)).toEqual({
      stage: "pages",
      done: 12,
      total: 40,
    });

    // Toplam bilinmiyorsa yüzde yok; sayaç varsa yalnız sayı yazılır.
    const noTotal = job({
      state: "running",
      progressStage: "pages",
      progressDone: 5,
      progressTotal: null,
    });
    expect(jobProgressLine(noTotal)).toBe("Sayfalar okunuyor: 5");
    expect(jobProgressWidth(noTotal)).toBeNull();

    // Aşama başlangıcı (0) bilgi taşımıyorsa yalnız etiket.
    expect(
      jobProgressLine(
        job({ state: "running", progressStage: "start", progressDone: 0, progressTotal: null }),
      ),
    ).toBe("Hazırlanıyor");

    // Sayaç toplamı aşarsa kırpılır — "45 / 40" yazılmaz.
    expect(
      jobProgressLine(
        job({ state: "running", progressStage: "pages", progressDone: 45, progressTotal: 40 }),
      ),
    ).toBe("Sayfalar okunuyor: 40 / 40 · %100");
  });

  it("bitmiş işte canlı sayaç gösterilmez (kalıntı yanıltmasın)", () => {
    for (const state of ["completed", "partial", "failed", "canceled", "queued", "blocked"]) {
      const stale = job({
        state,
        progressStage: "pages",
        progressDone: 12,
        progressTotal: 40,
      });
      expect(jobProgressLine(stale), state).toBeNull();
      expect(jobProgressWidth(stale), state).toBeNull();
      expect(jobProgressCounters(stale), state).toBeNull();
    }
    // Aşama hiç yazılmamışsa satır yok (uydurulmuş "0 / 0" yok).
    expect(jobProgressLine(job({ state: "running" }))).toBeNull();
  });

  it("kart yalnız yürüyen iş varken kendini tazeler", () => {
    expect(queueHasLiveJobs([])).toBe(false);
    expect(queueHasLiveJobs([job({ state: "queued" }), job({ state: "completed" })])).toBe(false);
    expect(queueHasLiveJobs([job({ state: "running" })])).toBe(true);
    expect(queueHasLiveJobs([job({ state: "validating" })])).toBe(true);
    expect(queueHasLiveJobs([job({ state: "canceling" })])).toBe(true);
    expect(queueHasLiveJobs([job({ state: "leased" })])).toBe(true);
  });

  it("sayım yardımcısı eksik durumda sıfır döner ve eski adları birleştirir", () => {
    expect(countOf(summary(), "failed")).toBe(0);
    expect(
      countOf(summary({ byState: [{ state: "failed", count: 5 }] }), "failed"),
    ).toBe(5);
    expect(
      countOf(
        summary({
          byState: [
            { state: "completed", count: 2 },
            { state: "succeeded", count: 1 },
          ],
        }),
        "completed",
      ),
    ).toBe(3);
  });

  it("kurtarma satırı sıfırda 'bulunmadı' der, sayı uydurmaz", () => {
    expect(recoveryLine({ interrupted: 0, requeued: 0, failed: 0, canceled: 0 })).toBe(
      "Yarıda kalmış iş yok; yeniden sıraya alınacak bir şey bulunmadı.",
    );
    expect(recoveryLine({ interrupted: 2, requeued: 2, failed: 0, canceled: 1 })).toBe(
      "2 iş yeniden sıraya alındı, 1 iş iptal edildi.",
    );
    expect(recoveryLine({ interrupted: 3, requeued: 1, failed: 2, canceled: 0 })).toContain(
      "2 iş tekrar bütçesi bittiği için kapandı",
    );
  });

  it("yeniden deneme 'sıraya alındı' ile 'başladı'yı ayırır", () => {
    const started: JobRetryView = {
      job: job({ state: "queued" }),
      started: true,
      skipped: null,
      versionId: "v1",
      applied: { ocr: false, ocrLang: "eng", textEncoding: "auto" },
    };
    expect(retryOutcomeLine(started)).toBe(
      "Yeniden deneme başladı (OCR kapalı, kodlama auto); sonucu satırda göreceksiniz.",
    );

    const queuedOnly: JobRetryView = {
      job: job({ state: "queued" }),
      started: false,
      skipped: "type_not_runnable",
      versionId: null,
      applied: null,
    };
    expect(retryOutcomeLine(queuedOnly)).toContain("kendiliğinden başlatan yürütücü yok");
    expect(retryOutcomeLine({ ...queuedOnly, skipped: "no_input" })).toContain(
      "girdisi (belge sürümü) çözülemedi",
    );
    // Uygulanan ayar yazılır: "varsayılan" denip OCR açık gizlenmez.
    expect(
      retryOutcomeLine({
        ...started,
        applied: { ocr: true, ocrLang: "tur", textEncoding: "utf-8" },
      }),
    ).toContain("OCR açık (tur)");
  });

  it("yeniden denemesi başlayan iş terminal olana kadar izlenir", () => {
    expect(jobIsTerminal("completed")).toBe(true);
    expect(jobIsTerminal("succeeded"), "eski ad terminal sayılır").toBe(true);
    expect(jobIsTerminal("partial")).toBe(true);
    expect(jobIsTerminal("queued")).toBe(false);
    const jobs = [job({ id: "j1", state: "queued" })];
    expect(queueHasLiveJobs(jobs), "queued tek başına canlı sayılmaz").toBe(false);
    expect(shouldPollQueue(jobs, "j1"), "izlenen iş tazelenir").toBe(true);
    expect(shouldPollQueue(jobs, null)).toBe(false);
    expect(shouldPollQueue([job({ id: "j1", state: "completed" })], "j1")).toBe(false);
    expect(shouldPollQueue([job({ id: "j2", state: "running" })], null)).toBe(true);
  });
});
