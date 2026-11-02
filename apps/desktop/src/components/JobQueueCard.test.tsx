// @vitest-environment jsdom
//
// İş kuyruğu kartı davranış testi (F03-T07/T08).
//
// `lib/jobs.ts` testleri yalnız **saf** yardımcıları doğruluyor: etiket, özet
// satırı, eylem seçimi, dürüst sonuç cümleleri. Bu dosya 283 satırlık kartın
// kendisini gerçekten çizip **bağlantıyı** ölçer: hangi düğme hangi çekirdek
// komutunu çağırıyor, kart ne zaman kendini tazeliyor, bir istek düştüğünde
// kullanıcı ne görüyor. `api` kısmi mock; `t()`, `lib/jobs` ve
// `lib/parseProgress` gerçektir.
//
// Sabitlenen davranışlar:
//
// 1. Kasa kilitliyken kuyruk **okunmaz**; kart yalnız kilit metnini yazar,
//    düğmeler kapalıdır (veri sızdırmaz).
// 2. Kuyruk okunamazsa sahte özet/tablo üretilmez; hata kodu ve ileti yazılır.
// 3. Özet satırı, dikkat uyarısı, satır künyeleri, durum rozetleri, ilerleme
//    satırı/çubuğu ve eylem düğmeleri **sunucudan** gelir; sayı uydurulmaz.
// 4. Boş kuyruk dürüst boş durumdur; kurtarma düğmesi yine durur.
// 5. "İptal et" gerçek `job_cancel`u çağırır; sonuç cümlesi çekirdeğin
//    `outcome`/`workerStopped` alanlarından yazılır (süreç durmadıysa
//    "iptal tamam" denmez) ve liste tazelenir.
// 6. Yeniden denemesi **başlayan** iş terminal olana kadar izlenir: satır
//    `queued` görünürken donmuş kalmaz, kart kendini tazeler.
// 7. "Başlat" yalnız `queued` **parse_document** işinde çıkar ve `job_run`u
//    çağırır; yürütücüsü olmayan türde düğme uydurulmaz.
// 8. "Engeli kaldır" `job_unblock`u çağırır.
// 9. İşlem sürerken satırdaki bütün eylem düğmeleri ve "Yenile" kilitlidir
//    (çift istek yok).
// 10. Canlı iş yokken kart kendini tazelemez; canlı iş varken sessiz tazeleme
//     düğmeleri **kilitlemez** (1,2 sn'de bir tıklama engellenmez).
// 11. **Ölçülen kusur 1 — hata veriyi siliyordu (A + C):** `error` dalı bütün
//     kartı değiştiriyordu. Tek bir arka plan okuma hatası çalışan işin
//     ilerlemesini yok ediyor (A); işlem **başarılı olduktan sonra** düşen
//     tazeleme ise gerçekten yapılmış iptali/başlatmayı görünmez kılıyordu
//     (C). İki ayrı kullanıcı yolunun tek kök nedeni: hata, verinin yerine
//     geçiyordu. Artık hata bir uyarı bandıdır; son okunan kuyruk ve işlemin
//     sonucu ekranda kalır.
// 12. **Ölçülen kusur 2 — yanıt sırası yok (B, B2…B5):** beş yazma yolu
//     (sessiz tazeleme, elle tazeleme, eylem sonrası tazeleme ve bunların
//     çakışmaları) sıraya bağlı değildi; yavaş kalan eski yanıt sonra gelince
//     **daha yeni** durumu eziyordu: ilerleme geriye sıçrıyor, biten iş
//     yeniden "Yürüyor", iptal istenmiş iş yeniden çalışıyor gibi görünüyordu.
//     Her yol tek bir sıra numarasıyla kapıya bağlandı; beş test beş çakışmayı
//     ayrı ayrı ölçüyor.
// 13. **Ölçülen kusur 3 — kasa kilitlenince durum taşınıyordu (D):** önceki
//     oturumun başarı bildirimi ve izlenen iş kimliği duruyordu; yeniden
//     açılan kasada eski bir sonuç "az önce oldu" gibi okunuyordu.
// 14. Toplam bilinmiyorsa ilerleme **çubuğu** çizilmez (oran uydurulmaz),
//     yalnız sayılan birim yazılır.

import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { t } from "../lib/i18n";
import type { JobCancelResult, JobQueueSummary, JobQueueView, JobRow } from "../lib/api";

const mocks = vi.hoisted(() => ({
  jobQueueStatus: vi.fn(),
  jobQueueRecover: vi.fn(),
  jobCancel: vi.fn(),
  jobRetry: vi.fn(),
  jobRun: vi.fn(),
  jobUnblock: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

const { JobQueueCard } = await import("./JobQueueCard");

/* ------------------------------------------------------------------ fixtures */

const ISO = "2026-11-02T09:00:00.000Z";

/** Kartın kendi tazeleme aralığı (`LIVE_REFRESH_MS`); burada birebir yazılır. */
const LIVE = 1200;

function job(over: Partial<JobRow> = {}): JobRow {
  return {
    id: "j1",
    jobType: "parse_document",
    scope: "case:c1",
    caseId: "c1",
    state: "queued",
    priority: 0,
    attemptCount: 1,
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
    createdAt: ISO,
    updatedAt: ISO,
    completedAt: null,
    ...over,
  };
}

/** Sunucunun sayımını iş listesinden üretir (kart sayıları kendisi hesaplamaz). */
function view(jobs: JobRow[], extra: Partial<JobQueueSummary> = {}): JobQueueView {
  const counts = new Map<string, number>();
  for (const row of jobs) counts.set(row.state, (counts.get(row.state) ?? 0) + 1);
  return {
    summary: {
      total: jobs.length,
      byState: [...counts.entries()].map(([state, count]) => ({ state, count })),
      dueNow: 0,
      oldestQueuedAt: null,
      blockedByDependency: 0,
      ...extra,
    },
    jobs,
  };
}

/* ------------------------------------------------------------------- harness */

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** Mikro görevleri boşalt (sahte zamanlayıcıda `findBy`/`waitFor` kullanılmaz). */
async function flush(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

/** Sahte saati ilerlet: kartın kurduğu aralık tetiklenir, yanıtlar boşaltılır. */
async function tick(ms = LIVE): Promise<void> {
  await act(async () => {
    vi.advanceTimersByTime(ms);
  });
  await flush();
}

async function mount(unlocked = true) {
  const utils = render(<JobQueueCard unlocked={unlocked} />);
  await flush();
  return utils;
}

function rowOf(text: string): HTMLElement {
  const cell = screen.getByText(text);
  const row = cell.closest("tr");
  if (!row) throw new Error(`"${text}" satırı bulunamadı`);
  return row as HTMLElement;
}

/** Satırın n. hücresi (0 tür, 1 durum, 2 ilerleme, 3 deneme, 4 eylem). */
function cellOf(row: HTMLElement, index: number): HTMLElement {
  const cells = row.querySelectorAll("td");
  if (!cells[index]) throw new Error(`${index}. hücre yok`);
  return cells[index] as HTMLElement;
}

function messageLine(line: string): HTMLElement {
  return screen.getByText(line);
}

beforeEach(() => {
  vi.useFakeTimers();
  mocks.jobQueueStatus.mockResolvedValue(view([]));
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.useRealTimers();
});

/* -------------------------------------------------------------------- tests */

describe("iş kuyruğu kartı", () => {
  it("kasa kilitliyken kuyruk okunmaz, düğmeler kapalıdır", async () => {
    await mount(false);

    expect(mocks.jobQueueStatus).not.toHaveBeenCalled();
    expect(screen.getByText(t("jobQueue.locked"))).toBeTruthy();
    const refresh = screen.getByRole("button", { name: t("jobQueue.refresh") }) as HTMLButtonElement;
    expect(refresh.disabled).toBe(true);
    expect(screen.queryByRole("table")).toBeNull();
  });

  it("kuyruk okunamazsa sahte özet üretmez, kodu ve iletiyi yazar", async () => {
    mocks.jobQueueStatus.mockRejectedValue({ code: "E_DB", message: "kasa okunamadı", retryable: true });
    await mount();

    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("E_DB");
    expect(alert.textContent).toContain("kasa okunamadı");
    expect(screen.queryByText(t("jobs.summaryEmpty"))).toBeNull();
    expect(screen.queryByRole("table")).toBeNull();
  });

  it("özeti, dikkat uyarısını ve satır künyelerini sunucudan yazar", async () => {
    const running = job({
      id: "j-run",
      state: "running",
      attemptCount: 1,
      progressStage: "pages",
      progressDone: 3,
      progressTotal: 10,
      progress: 0.3,
    });
    const failed = job({
      id: "j-fail",
      jobType: "export",
      state: "failed",
      attemptCount: 3,
      maxAttempts: 3,
      errorCode: "E_IO",
    });
    const blocked = job({ id: "j-block", jobType: "import", state: "blocked", blockedReason: "password" });
    const done = job({ id: "j-done", jobType: "backup", state: "completed" });
    const queuedOcr = job({ id: "j-ocr", jobType: "ocr_page", state: "queued" });

    mocks.jobQueueStatus.mockResolvedValue(view([running, failed, blocked, done, queuedOcr]));
    await mount();

    expect(
      screen.getByText("5 iş · 1 yürüyor · 1 bekliyor · 1 engelli · 1 bitti · 1 başarısız"),
    ).toBeTruthy();
    expect(
      screen.getByText(
        `${t("jobs.attentionFailed", { count: 1 })} ${t("jobs.attentionBlocked", { count: 1 })}`,
      ),
    ).toBeTruthy();

    const runRow = rowOf(t("jobs.type.parse_document"));
    expect(within(runRow).getByText(t("jobs.state.running"))).toBeTruthy();
    expect(within(runRow).getByText("Sayfalar okunuyor: 3 / 10 · %30")).toBeTruthy();
    const bar = within(runRow).getByRole("progressbar");
    expect(bar.getAttribute("aria-valuenow")).toBe("30");
    expect(bar.getAttribute("aria-label")).toBe(
      t("jobQueue.progressAria", { line: "Sayfalar okunuyor: 3 / 10 · %30" }),
    );
    expect(within(runRow).getByRole("button", { name: t("jobs.action.cancel") })).toBeTruthy();

    const failRow = rowOf(t("jobs.type.export"));
    expect(within(failRow).getByText(t("jobs.state.failed"))).toBeTruthy();
    expect(within(failRow).getByText(/deneme 3\/3/).textContent).toContain("E_IO");
    expect(within(failRow).getByText(/2026-11-02 09:00/)).toBeTruthy();
    expect(within(failRow).getByRole("button", { name: t("jobs.action.retry") })).toBeTruthy();
    // Yürümeyen satırda canlı sayaç yoktur: kalıntı sayaç yanıltmaz.
    expect(cellOf(failRow, 2).textContent).toBe("—");
    expect(within(failRow).queryByRole("progressbar")).toBeNull();

    const blockRow = rowOf(t("jobs.type.import"));
    expect(within(blockRow).getByText(t("jobs.state.blocked"))).toBeTruthy();
    expect(within(blockRow).getByText(/parola bekliyor/)).toBeTruthy();
    expect(within(blockRow).getByRole("button", { name: t("jobs.action.unblock") })).toBeTruthy();
    expect(within(blockRow).getByRole("button", { name: t("jobs.action.cancel") })).toBeTruthy();

    const doneRow = rowOf(t("jobs.type.backup"));
    expect(within(doneRow).getByText(t("jobs.state.completed"))).toBeTruthy();
    // Terminal işte eylem sütunu boş: düğme uydurulmaz.
    expect(cellOf(doneRow, 4).textContent).toBe("—");
    expect(within(doneRow).queryByRole("button")).toBeNull();

    const ocrRow = rowOf(t("jobs.type.ocr_page"));
    expect(within(ocrRow).getByRole("button", { name: t("jobs.action.cancel") })).toBeTruthy();
    expect(within(ocrRow).queryByRole("button", { name: t("jobs.action.run") })).toBeNull();
  });

  it("boş kuyruğu dürüst yazar, kurtarma düğmesini yine gösterir", async () => {
    await mount();

    expect(screen.getByText(t("jobs.summaryEmpty"))).toBeTruthy();
    expect(screen.getByText(t("jobQueue.empty"))).toBeTruthy();
    expect(screen.queryByRole("table")).toBeNull();
    expect(screen.getByRole("button", { name: t("jobQueue.recover") })).toBeTruthy();
  });

  it("iptal gerçek komutu çağırır, çekirdeğin sonucunu yazar ve listeyi tazeler", async () => {
    const running = job({ id: "j1", state: "running", progressStage: "pages", progressDone: 2, progressTotal: 4 });
    const cancelResult: JobCancelResult = { outcome: "requested", job: running, workerStopped: true };
    mocks.jobQueueStatus
      .mockResolvedValueOnce(view([running]))
      .mockResolvedValueOnce(view([job({ id: "j1", state: "canceling" })]));
    mocks.jobCancel.mockResolvedValue(cancelResult);

    await mount();
    fireEvent.click(screen.getByRole("button", { name: t("jobs.action.cancel") }));
    await flush();

    expect(mocks.jobCancel).toHaveBeenCalledWith("j1");
    // Süreç durduruldu → "iptal tamam" değil, durduruldu cümlesi yazılır.
    expect(messageLine(t("jobs.cancelStopped"))).toBeTruthy();
    expect(screen.queryByText(t("jobs.cancelNeverRan"))).toBeNull();
    expect(mocks.jobQueueStatus).toHaveBeenCalledTimes(2);
    expect(
      within(rowOf(t("jobs.type.parse_document"))).getByText(t("jobs.state.canceling")),
    ).toBeTruthy();
  });

  it("yeniden denemesi başlayan işi terminal olana kadar izler", async () => {
    const failed = job({
      id: "j7",
      state: "failed",
      attemptCount: 3,
      maxAttempts: 3,
      errorCode: "E_PARSE",
    });
    mocks.jobQueueStatus
      .mockResolvedValueOnce(view([failed]))
      // Eylem sonrası tazeleme: iş henüz `queued` görünür (yürütücü sahiplenmedi).
      .mockResolvedValueOnce(view([job({ id: "j7", state: "queued", attemptCount: 3, maxAttempts: 3 })]))
      // İlk sessiz tazeleme: iş gerçekten başladı.
      .mockResolvedValueOnce(view([job({ id: "j7", state: "running", progressStage: "start" })]));
    mocks.jobRetry.mockResolvedValue({
      job: failed,
      started: true,
      skipped: null,
      versionId: "v1",
      applied: { ocr: false, ocrLang: "tur", textEncoding: "utf-8", restored: false },
    });

    await mount();
    fireEvent.click(screen.getByRole("button", { name: t("jobs.action.retry") }));
    await flush();

    expect(mocks.jobRetry).toHaveBeenCalledWith("j7");
    expect(
      messageLine(
        t("jobs.start.line", {
          verb: t("jobs.start.verbRetry"),
          source: t("jobs.start.sourceDefault"),
          settings: t("jobs.start.settings", {
            ocr: t("jobs.start.ocrOff"),
            encoding: "utf-8",
          }),
        }),
      ),
    ).toBeTruthy();

    // İzleme olmasa satır `queued` görünüp donardı.
    await tick();
    expect(mocks.jobQueueStatus).toHaveBeenCalledTimes(3);
    expect(
      within(rowOf(t("jobs.type.parse_document"))).getByText(t("jobs.state.running")),
    ).toBeTruthy();
  });

  it("yalnız sıradaki parse_document işinde 'Başlat' çıkar ve job_run çağırır", async () => {
    const queuedParse = job({ id: "j3", jobType: "parse_document", state: "queued" });
    const queuedExport = job({ id: "j4", jobType: "export", state: "queued" });
    mocks.jobQueueStatus
      .mockResolvedValueOnce(view([queuedParse, queuedExport]))
      .mockResolvedValueOnce(
        view([job({ id: "j3", state: "running", progressStage: "start" }), queuedExport]),
      );
    mocks.jobRun.mockResolvedValue({
      job: queuedParse,
      started: true,
      skipped: null,
      versionId: "v9",
      applied: null,
    });

    await mount();

    // Yürütücüsü olmayan türde düğme yok: habersiz iş başlatılmaz.
    expect(
      within(rowOf(t("jobs.type.export"))).queryByRole("button", { name: t("jobs.action.run") }),
    ).toBeNull();

    fireEvent.click(
      within(rowOf(t("jobs.type.parse_document"))).getByRole("button", { name: t("jobs.action.run") }),
    );
    await flush();

    expect(mocks.jobRun).toHaveBeenCalledWith("j3");
    expect(mocks.jobRetry).not.toHaveBeenCalled();
    expect(
      messageLine(
        t("jobs.start.line", {
          verb: t("jobs.start.verbRun"),
          source: t("jobs.start.sourceDefault"),
          settings: t("jobs.start.settingsDefault"),
        }),
      ),
    ).toBeTruthy();
  });

  it("engeli kaldırma çekirdeğe gider ve sonucu yazar", async () => {
    const blocked = job({ id: "j5", jobType: "import", state: "blocked", blockedReason: "dependency" });
    mocks.jobQueueStatus
      .mockResolvedValueOnce(view([blocked]))
      .mockResolvedValueOnce(view([job({ id: "j5", jobType: "import", state: "queued" })]));
    mocks.jobUnblock.mockResolvedValue(blocked);

    await mount();
    fireEvent.click(screen.getByRole("button", { name: t("jobs.action.unblock") }));
    await flush();

    expect(mocks.jobUnblock).toHaveBeenCalledWith("j5");
    expect(messageLine(t("jobQueue.unblocked"))).toBeTruthy();
  });

  it("işlem sürerken bütün eylem düğmeleri ve 'Yenile' kilitlidir", async () => {
    const running = job({ id: "j1", state: "running" });
    mocks.jobQueueStatus.mockResolvedValue(view([running]));
    const pending = deferred<JobCancelResult>();
    mocks.jobCancel.mockReturnValue(pending.promise);

    await mount();
    const cancelButton = screen.getByRole("button", {
      name: t("jobs.action.cancel"),
    }) as HTMLButtonElement;
    fireEvent.click(cancelButton);

    expect(cancelButton.disabled).toBe(true);
    expect(
      (screen.getByRole("button", { name: t("jobQueue.refresh") }) as HTMLButtonElement).disabled,
    ).toBe(true);

    pending.resolve({ outcome: "requested", job: running, workerStopped: false });
    await flush();
    expect(
      (screen.getByRole("button", { name: t("jobQueue.refresh") }) as HTMLButtonElement).disabled,
    ).toBe(false);
  });

  it("canlı iş yokken kart kendini tazelemez", async () => {
    mocks.jobQueueStatus.mockResolvedValue(view([job({ id: "j1", state: "completed" })]));
    await mount();

    expect(mocks.jobQueueStatus).toHaveBeenCalledTimes(1);
    await tick(LIVE * 3);
    expect(mocks.jobQueueStatus).toHaveBeenCalledTimes(1);
  });

  it("canlı iş varken sessiz tazeleme düğmeleri kilitlemez, sayacı günceller", async () => {
    const running = job({ id: "j1", state: "running", progressStage: "pages", progressDone: 2, progressTotal: 10 });
    const pending = deferred<JobQueueView>();
    mocks.jobQueueStatus.mockResolvedValueOnce(view([running])).mockReturnValueOnce(pending.promise);

    await mount();
    await tick();

    // Sessiz tazeleme askıdayken kullanıcı işlem yapabilmeli.
    const cancelButton = screen.getByRole("button", {
      name: t("jobs.action.cancel"),
    }) as HTMLButtonElement;
    expect(cancelButton.disabled).toBe(false);

    pending.resolve(
      view([job({ id: "j1", state: "running", progressStage: "pages", progressDone: 6, progressTotal: 10 })]),
    );
    await flush();
    expect(screen.getByText("Sayfalar okunuyor: 6 / 10 · %60")).toBeTruthy();
  });

  it("ÖLÇÜLEN KUSUR A: arka plan tazelemesi düşünce kuyruk ekrandan silinmez", async () => {
    const running = job({ id: "j1", state: "running", progressStage: "pages", progressDone: 2, progressTotal: 10 });
    mocks.jobQueueStatus
      .mockResolvedValueOnce(view([running]))
      .mockRejectedValueOnce({ code: "E_DB", message: "geçici okuma hatası", retryable: true });

    await mount();
    await tick();

    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("E_DB");
    // Tek bir okuma hatası, çalışan işin görünen ilerlemesini yok etmez.
    expect(screen.getByRole("table")).toBeTruthy();
    expect(screen.getByText("Sayfalar okunuyor: 2 / 10 · %20")).toBeTruthy();
  });

  it("ÖLÇÜLEN KUSUR B: geç gelen eski yanıt yeni durumu ezmez", async () => {
    const running = job({ id: "j1", state: "running", progressStage: "pages", progressDone: 2, progressTotal: 10 });
    const slow = deferred<JobQueueView>();
    mocks.jobQueueStatus
      .mockResolvedValueOnce(view([running]))
      .mockReturnValueOnce(slow.promise)
      .mockResolvedValueOnce(
        view([job({ id: "j1", state: "running", progressStage: "pages", progressDone: 7, progressTotal: 10 })]),
      );

    await mount();
    await tick(); // 1. sessiz tazeleme başladı (yavaş, askıda)
    await tick(); // 2. sessiz tazeleme bitti (7/10)
    expect(screen.getByText("Sayfalar okunuyor: 7 / 10 · %70")).toBeTruthy();

    // Eski yanıt sonra geliyor: daha yeni durumu ezmemeli.
    slow.resolve(view([job({ id: "j1", state: "running", progressStage: "pages", progressDone: 3, progressTotal: 10 })]));
    await flush();

    expect(screen.queryByText("Sayfalar okunuyor: 3 / 10 · %30")).toBeNull();
    expect(screen.getByText("Sayfalar okunuyor: 7 / 10 · %70")).toBeTruthy();
  });

  it("ÖLÇÜLEN KUSUR B2: elle 'Yenile' askıdaki sessiz tazelemenin eski yanıtına ezilmez", async () => {
    const running = job({ id: "j1", state: "running", progressStage: "pages", progressDone: 2, progressTotal: 10 });
    const slowPoll = deferred<JobQueueView>();
    mocks.jobQueueStatus
      .mockResolvedValueOnce(view([running]))
      .mockReturnValueOnce(slowPoll.promise)
      .mockResolvedValueOnce(view([job({ id: "j1", state: "completed" })]));

    await mount();
    await tick(); // sessiz tazeleme askıda
    fireEvent.click(screen.getByRole("button", { name: t("jobQueue.refresh") }));
    await flush();
    expect(screen.getByText(t("jobs.state.completed"))).toBeTruthy();

    slowPoll.resolve(view([running]));
    await flush();
    // Biten iş, eski yanıt yüzünden yeniden "Yürüyor" görünmemeli.
    expect(screen.queryByText(t("jobs.state.running"))).toBeNull();
    expect(screen.getByText(t("jobs.state.completed"))).toBeTruthy();
  });

  it("ÖLÇÜLEN KUSUR B3: işlem sonrası tazeleme, askıdaki eski yoklamaya ezilmez", async () => {
    const running = job({ id: "j1", state: "running" });
    const slowPoll = deferred<JobQueueView>();
    mocks.jobQueueStatus
      .mockResolvedValueOnce(view([running]))
      .mockReturnValueOnce(slowPoll.promise)
      .mockResolvedValueOnce(view([job({ id: "j1", state: "canceling" })]));
    mocks.jobCancel.mockResolvedValue({ outcome: "requested", job: running, workerStopped: false });

    await mount();
    await tick(); // sessiz tazeleme askıda (eski anlık görüntü)
    fireEvent.click(screen.getByRole("button", { name: t("jobs.action.cancel") }));
    await flush();
    expect(screen.getByText(t("jobs.state.canceling"))).toBeTruthy();

    slowPoll.resolve(view([running]));
    await flush();
    // İptal istemi geri alınmış gibi görünmemeli.
    expect(screen.queryByText(t("jobs.state.running"))).toBeNull();
    expect(screen.getByText(t("jobs.state.canceling"))).toBeTruthy();
  });

  it("ÖLÇÜLEN KUSUR B4: geç kalan elle tazeleme daha yeni sayacı ezmez", async () => {
    const running = job({ id: "j1", state: "running", progressStage: "pages", progressDone: 2, progressTotal: 10 });
    const slowRefresh = deferred<JobQueueView>();
    mocks.jobQueueStatus
      .mockResolvedValueOnce(view([running]))
      .mockReturnValueOnce(slowRefresh.promise)
      .mockResolvedValueOnce(
        view([job({ id: "j1", state: "running", progressStage: "pages", progressDone: 7, progressTotal: 10 })]),
      );

    await mount();
    fireEvent.click(screen.getByRole("button", { name: t("jobQueue.refresh") })); // askıda kalan elle tazeleme
    await flush();
    await tick(); // sessiz tazeleme bitti (7/10)
    expect(screen.getByText("Sayfalar okunuyor: 7 / 10 · %70")).toBeTruthy();

    slowRefresh.resolve(view([running])); // eski anlık görüntü sonra geliyor
    await flush();
    expect(screen.queryByText("Sayfalar okunuyor: 2 / 10 · %20")).toBeNull();
    expect(screen.getByText("Sayfalar okunuyor: 7 / 10 · %70")).toBeTruthy();
  });

  it("ÖLÇÜLEN KUSUR B5: geç kalan eylem sonrası tazeleme yeni durumu ezmez", async () => {
    const running = job({ id: "j1", state: "running" });
    const slowRefetch = deferred<JobQueueView>();
    mocks.jobQueueStatus
      .mockResolvedValueOnce(view([running]))
      .mockReturnValueOnce(slowRefetch.promise)
      .mockResolvedValueOnce(view([job({ id: "j1", state: "canceling" })]));
    mocks.jobCancel.mockResolvedValue({ outcome: "requested", job: running, workerStopped: false });

    await mount();
    fireEvent.click(screen.getByRole("button", { name: t("jobs.action.cancel") })); // tazeleme askıda
    await flush();
    await tick(); // sessiz tazeleme: iptal istendi
    expect(screen.getByText(t("jobs.state.canceling"))).toBeTruthy();

    slowRefetch.resolve(view([running]));
    await flush();
    expect(screen.queryByText(t("jobs.state.running"))).toBeNull();
    expect(screen.getByText(t("jobs.state.canceling"))).toBeTruthy();
  });

  it("ÖLÇÜLEN KUSUR C: işlem başarılı ama liste tazelemesi düşerse sonuç görünür", async () => {
    const running = job({ id: "j1", state: "running" });
    mocks.jobQueueStatus
      .mockResolvedValueOnce(view([running]))
      .mockRejectedValueOnce({ code: "E_DB", message: "tazeleme okunamadı", retryable: true });
    mocks.jobCancel.mockResolvedValue({ outcome: "requested", job: running, workerStopped: false });

    await mount();
    fireEvent.click(screen.getByRole("button", { name: t("jobs.action.cancel") }));
    await flush();

    // İptal istemi gerçekten yazıldı: sonucu kırmızı hatanın yanında görünmeli.
    expect(messageLine(t("jobs.cancelRequested"))).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("E_DB");
  });

  it("ÖLÇÜLEN KUSUR D: kasa kilitlenince önceki oturumun bildirimi taşınmaz", async () => {
    const running = job({ id: "j1", state: "running" });
    mocks.jobQueueStatus.mockResolvedValue(view([running]));
    mocks.jobCancel.mockResolvedValue({ outcome: "canceled", job: running, workerStopped: false });

    const { rerender } = await mount();
    fireEvent.click(screen.getByRole("button", { name: t("jobs.action.cancel") }));
    await flush();
    expect(messageLine(t("jobs.cancelNeverRan"))).toBeTruthy();

    rerender(<JobQueueCard unlocked={false} />);
    await flush();
    expect(screen.getByText(t("jobQueue.locked"))).toBeTruthy();

    rerender(<JobQueueCard unlocked />);
    await flush();
    expect(screen.queryByText(t("jobs.cancelNeverRan"))).toBeNull();
  });

  it("toplam bilinmiyorsa ilerleme çubuğu çizilmez, sayılan birim yazılır", async () => {
    const running = job({
      id: "j1",
      state: "running",
      progressStage: "pages",
      progressDone: 4,
      progressTotal: null,
      progress: 0.5,
    });
    mocks.jobQueueStatus.mockResolvedValue(view([running]));

    await mount();
    const row = rowOf(t("jobs.type.parse_document"));
    expect(within(row).getByText("Sayfalar okunuyor: 4")).toBeTruthy();
    expect(within(row).queryByRole("progressbar")).toBeNull();
  });

  it("kurtarma gerçek komutu çağırır ve raporu tek satır yazar", async () => {
    mocks.jobQueueRecover.mockResolvedValue({
      interrupted: 2,
      requeued: 1,
      failed: 1,
      canceled: 0,
      interruptedJobIds: ["j1", "j2"],
    });

    await mount();
    fireEvent.click(screen.getByRole("button", { name: t("jobQueue.recover") }));
    await flush();

    expect(mocks.jobQueueRecover).toHaveBeenCalledTimes(1);
    expect(
      screen.getByText(
        `${t("jobs.recovery.requeued", { count: 1 })}, ${t("jobs.recovery.failed", { count: 1 })}.`,
      ),
    ).toBeTruthy();
  });

  it("'Yenile' çekirdekten yeniden okur", async () => {
    await mount();
    expect(mocks.jobQueueStatus).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: t("jobQueue.refresh") }));
    await flush();
    expect(mocks.jobQueueStatus).toHaveBeenCalledTimes(2);
  });
});
