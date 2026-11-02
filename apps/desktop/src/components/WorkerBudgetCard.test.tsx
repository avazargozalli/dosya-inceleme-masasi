// @vitest-environment jsdom
//
// Kaynak bütçesi kartı davranış testi (§17.3).
//
// `lib/budget.ts` testleri yalnız biçimlendirmeyi doğrular; burada 109 satırlık
// kart gerçekten çizilip **çekirdekten gelen** sınırların ekrana nasıl
// taşındığı ölçülür. Kart kendi sınırını yazmaz/hesaplamaz: sunucu ne dediyse
// o yazılır, sıfır sınır "0 MiB"/"uygulanmaz" olur (uydurma sınır yok).
//
// Sabitlenen davranışlar:
//
// 1. Kasa kilitliyken profiller **okunmaz**; kart yalnız kilit metnini yazar,
//    "Yenile" kapalıdır.
// 2. Tablo künyesi ve altı sütun sunucudan gelir: profil adı, CPU, bellek,
//    çıktı, açık dosya, duvar saati. Biçim `lib/budget` üzerinden tek katman.
// 3. Her profil için tek satırlık sınır özeti (`profileLimits`) yazılır;
//    ayırıcı " · " ve etiketler sözlükten.
// 4. Sıfır sınır uydurulmaz: CPU 0 → "uygulanmaz", bellek 0 → "0 MiB".
// 5. Profil listesi boşsa "Profil bulunamadı."; okuma sürerken "Profiller
//    okunuyor…" yazar (ikisi karıştırılmaz).
// 6. Okuma düşerse sahte profil üretilmez; hata kodu ve ileti yazılır, tablo
//    çizilmez (önbellekten "geçerli" görünen bayat profil yok).
// 7. "Yenile" çekirdeği yeniden okur.

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { mibText, profileLimits, secondsText } from "../lib/budget";
import { t } from "../lib/i18n";
import type { WorkerBudgetProfile } from "../lib/api";

const mocks = vi.hoisted(() => ({ workerBudgets: vi.fn() }));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

const { WorkerBudgetCard } = await import("./WorkerBudgetCard");

/* ------------------------------------------------------------------ fixtures */

const PDF: WorkerBudgetProfile = {
  kind: "pdf",
  label: "PDF worker",
  cpuSeconds: 120,
  memoryBytes: 256 * 1024 * 1024,
  outputBytes: 64 * 1024 * 1024,
  openFiles: 64,
  wallClockSeconds: 300,
};

/** AI çağrısı yerel süreç değildir: CPU sınırı yok (0 = uygulanmaz). */
const AI: WorkerBudgetProfile = {
  kind: "ai",
  label: "AI köprüsü",
  cpuSeconds: 0,
  memoryBytes: 0,
  outputBytes: 8 * 1024 * 1024,
  openFiles: 16,
  wallClockSeconds: 60,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

function rowOf(text: string): HTMLElement {
  const row = screen.getByText(text).closest("tr");
  if (!row) throw new Error(`"${text}" satırı bulunamadı`);
  return row as HTMLElement;
}

beforeEach(() => {
  mocks.workerBudgets.mockResolvedValue([PDF]);
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

/* -------------------------------------------------------------------- tests */

describe("kaynak bütçesi kartı", () => {
  it("kasa kilitliyken profiller okunmaz, 'Yenile' kapalıdır", async () => {
    render(<WorkerBudgetCard unlocked={false} />);

    expect(mocks.workerBudgets).not.toHaveBeenCalled();
    expect(screen.getByText(t("workerBudget.locked"))).toBeTruthy();
    const refresh = screen.getByRole("button", { name: t("workerBudget.refresh") }) as HTMLButtonElement;
    expect(refresh.disabled).toBe(true);
    expect(screen.queryByRole("table")).toBeNull();
  });

  it("sınırları çekirdekten okur ve altı sütunda yazar", async () => {
    render(<WorkerBudgetCard unlocked />);

    expect(await screen.findByText("PDF worker")).toBeTruthy();
    expect(mocks.workerBudgets).toHaveBeenCalledTimes(1);

    // Başlıklar sözlükten: yeni boyut eklenirse burada görünür.
    for (const key of [
      "workerBudget.colProfile",
      "workerBudget.colCpu",
      "workerBudget.colMemory",
      "workerBudget.colOutput",
      "workerBudget.colFiles",
      "workerBudget.colTime",
    ] as const) {
      expect(screen.getByRole("columnheader", { name: t(key) })).toBeTruthy();
    }

    const row = rowOf("PDF worker");
    const cells = [...row.querySelectorAll("td")].map((cell) => cell.textContent);
    expect(cells).toEqual(["PDF worker", "120 sn", "256 MiB", "64 MiB", "64", "300 sn"]);
  });

  it("her profil için tek satırlık sınır özeti yazar", async () => {
    render(<WorkerBudgetCard unlocked />);

    expect(await screen.findByText(profileLimits(PDF))).toBeTruthy();
    // Biçim tek katmandan: yardımcıların çıktısı birebir taşınır.
    expect(profileLimits(PDF)).toContain(secondsText(120));
    expect(profileLimits(PDF)).toContain(mibText(256 * 1024 * 1024));
  });

  it("sıfır sınırı 'uygulanmaz' ve '0 MiB' yazar, sınır uydurmaz", async () => {
    mocks.workerBudgets.mockResolvedValue([PDF, AI]);
    render(<WorkerBudgetCard unlocked />);

    expect(await screen.findByText("AI köprüsü")).toBeTruthy();
    const aiRow = rowOf("AI köprüsü");
    const cells = [...aiRow.querySelectorAll("td")].map((cell) => cell.textContent);
    expect(cells).toEqual(["AI köprüsü", "uygulanmaz", "0 MiB", "8 MiB", "16", "60 sn"]);

    const pdfRow = rowOf("PDF worker");
    expect(pdfRow.textContent).not.toContain(t("budget.notApplicable"));
  });

  it("okuma sürerken 'okunuyor', liste boşsa 'bulunamadı' yazar", async () => {
    const pending = deferred<WorkerBudgetProfile[]>();
    mocks.workerBudgets.mockReturnValue(pending.promise);
    render(<WorkerBudgetCard unlocked />);

    expect(screen.getByText(t("workerBudget.busy"))).toBeTruthy();
    expect(screen.queryByText(t("workerBudget.notFound"))).toBeNull();

    pending.resolve([]);
    expect(await screen.findByText(t("workerBudget.notFound"))).toBeTruthy();
    expect(screen.queryByText(t("workerBudget.busy"))).toBeNull();
  });

  it("okuma düşerse sahte profil üretmez, kodu ve iletiyi yazar", async () => {
    mocks.workerBudgets.mockRejectedValue({ code: "E_DB", message: "kasa okunamadı", retryable: true });
    render(<WorkerBudgetCard unlocked />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DB");
    expect(alert.textContent).toContain("kasa okunamadı");
    expect(screen.queryByRole("table")).toBeNull();
    expect(screen.queryByText(t("workerBudget.notFound"))).toBeNull();
  });

  it("'Yenile' çekirdeği yeniden okur", async () => {
    render(<WorkerBudgetCard unlocked />);
    expect(await screen.findByText("PDF worker")).toBeTruthy();
    expect(mocks.workerBudgets).toHaveBeenCalledTimes(1);

    mocks.workerBudgets.mockResolvedValue([AI]);
    fireEvent.click(screen.getByRole("button", { name: t("workerBudget.refresh") }));

    expect(await screen.findByText("AI köprüsü")).toBeTruthy();
    expect(mocks.workerBudgets).toHaveBeenCalledTimes(2);
    expect(screen.queryByText("PDF worker")).toBeNull();
  });
});
