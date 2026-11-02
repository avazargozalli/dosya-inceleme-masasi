// @vitest-environment jsdom
//
// Çıktılar ekranı davranış testi. `lib/export-coverage` testleri yalnız metin
// yardımcılarını doğrular; burada 277 satırlık ekran gerçekten render edilip
// F11-T06 (kasadan okunan kapsam, dışa aktarma, devir paketi) kararlarının
// arayüzde **uygulandığı** ölçülür. `api` kısmi mock; `t()` ve
// `lib/export-coverage` gerçektir.
//
// Sabitlenen davranışlar:
//
// 1. Dosya listesi `caseList(false)` ile gelir, ilk dosya seçilir; kapsam o
//    dosya için kasadan istenir ve rozet/başlık/satırlar/uyarılar **sunucudan**
//    yazılır (arayüz kendi sayısını üretmez).
// 2. Kapsam `complete=false` ise “Kapsam kısmi” + uyarı listesi; belgesiz
//    dosyada “Kapsam yok” + dürüst uyarı; sayılar uydurulmaz.
// 3. Dosya yokken dışa aktarma istekleri gitmez, altı düğme de kapalıdır.
// 4. Kaydet diyaloğu dosya **başlığından** ad üretir (`fileStem`), uzantı ve
//    filtre seçilen biçimdir; çekirdeğe `(caseId, hedef yol)` gider.
// 5. Sonuç künyesi yazılan türü, yolu, baytı, sha256'nın ilk 16 karakterini ve
//    **yanıttan gelen** kapsamın gömüldüğü yeri yazar.
// 6. İptal edilen diyalog istek üretmez.
// 7. Düşen dışa aktarma önceki yeşil sonucu bırakmaz (bayat başarı).
// 8. Yazarken düğme “Yazılıyor…” olur, öteki çıktı ve paket düğmeleri kilitlenir.
// 9. Kapsam tazeleme yeni değerleri yazar; **düşen tazeleme** eski kırmızı
//    hatayı bırakmaz ve sonraki başarılı tazelemede silinir.
// 10. Devir paketi klasör ister; parola **10 karakterden kısaysa gönderilmez**
//     (açık metin klasör), değilse olduğu gibi gider.
// 11. Paket künyesi yol/dosya/manifest öneki + şifreli paket yolunu yazar.
// 12. Düşen paket isteği önceki paket satırını bırakmaz.
// 13. **Ölçülen kusur (A):** paket başarısından sonra parola alanı temizlenmiyordu
//     — kullanıcının yazdığı parola ekranda/bellekte kalıyordu.
// 14. **Ölçülen kusur (B, INV-05):** dosya değişince önceki dosyanın kapsamı,
//     çıktı satırı ve paket satırı ekranda kalıyordu; yeni dosyanın isteği
//     askıdayken **başka dosyanın çıktısı** güncel sanılabiliyordu.
// 15. **Ölçülen kusur (C):** düşen kapsam isteği önceki dosyanın kapsamını
//     “okunmuş” gibi bırakıyordu.
// 16. **Ölçülen kusur (D):** kapsam okunurken “Kapsam okunamadı” yazılıyordu
//     (henüz okunmadı ≠ okunamadı).
// 17. Geç gelen **eski** dosya yanıtı yeni dosyanın kapsamını ezmez.
// 18. Dosya listesi düşerse dürüst hata yazılır, sahte liste üretilmez.

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { t } from "../lib/i18n";
import {
  coverageHeadline,
  coverageRows,
  coverageState,
  emptyCoverageWarning,
} from "../lib/export-coverage";
import { fileStem } from "../lib/filename";
import type { CaseRow, ExportCoverage, ExportResult, PackageResult } from "../lib/api";

const mocks = vi.hoisted(() => ({
  caseList: vi.fn(),
  caseCoverage: vi.fn(),
  exportJson: vi.fn(),
  exportCsv: vi.fn(),
  exportDocx: vi.fn(),
  exportPdf: vi.fn(),
  exportPackage: vi.fn(),
}));

const dialog = vi.hoisted(() => ({ save: vi.fn(), open: vi.fn() }));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

// Tauri olmadan ekranı ölçmek için dosya/klasör seçicileri sahtelenir.
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: dialog.save, open: dialog.open }));

const { Exports } = await import("./Exports");

/* ------------------------------------------------------------------ fixtures */

const ISO = "2026-11-02T09:00:00.000Z";

function caseRow(overrides: Partial<CaseRow> & { id: string; title: string }): CaseRow {
  return {
    caseKind: "review",
    referenceCode: null,
    jurisdictionLabel: null,
    practiceArea: null,
    status: "active",
    sensitivity: "normal",
    defaultAiPolicy: "local_only",
    archivedAt: null,
    retentionHold: false,
    createdAt: ISO,
    updatedAt: ISO,
    rowVersion: 1,
    ...overrides,
  };
}

const P1 = caseRow({ id: "p1", title: "Kira uyuşmazlığı" });
const P2 = caseRow({ id: "p2", title: "Ceza dosyası" });

function coverage(overrides: Partial<ExportCoverage> = {}): ExportCoverage {
  return {
    documents: 3,
    versions: 4,
    parseRuns: 4,
    pages: 120,
    textPages: 110,
    ocrPages: 8,
    segments: 640,
    emptyPages: 2,
    unparsedDocuments: 0,
    failedParseRuns: 0,
    complete: true,
    warnings: [],
    statement: "Kapsam kasadan okundu; sayılar çekirdekten gelir.",
    generatedAt: ISO,
    ...overrides,
  };
}

const FULL = coverage();
const PARTIAL = coverage({
  complete: false,
  ocrPages: 12,
  emptyPages: 3,
  unparsedDocuments: 2,
  failedParseRuns: 1,
  warnings: ["2 belge ayrıştırılmadı", "3 boş sayfa"],
  statement: "Kapsam kısmi; incelenmedi ≠ temiz.",
});
const EMPTY = coverage({
  documents: 0,
  versions: 0,
  parseRuns: 0,
  pages: 0,
  textPages: 0,
  ocrPages: 0,
  segments: 0,
  emptyPages: 0,
  complete: false,
  warnings: [],
  statement: "Dosyada belge yok.",
});

function exportResult(overrides: Partial<ExportResult> = {}): ExportResult {
  return {
    path: "/Users/avukat/Cikti/kira.json",
    bytes: 20480,
    sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    record: {
      id: "ex-1",
      exportType: "json",
      status: "ok",
      outputHash: "0123456789abcdef",
      createdAt: ISO,
    },
    coverage: FULL,
    ...overrides,
  };
}

function packageResult(overrides: Partial<PackageResult> = {}): PackageResult {
  return {
    path: "/Users/avukat/Devir/kira-paket",
    files: 5,
    sha256: "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    sealedPath: null,
    record: { id: "pk-1", exportType: "package", status: "ok", createdAt: ISO },
    coverage: FULL,
    ...overrides,
  };
}

type Deferred<T> = { promise: Promise<T>; resolve: (v: T) => void; reject: (e: unknown) => void };

function deferred<T>(): Deferred<T> {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function apiError(code: string, message: string) {
  return { code, message };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.caseList.mockResolvedValue([P1, P2]);
  mocks.caseCoverage.mockResolvedValue(FULL);
  mocks.exportJson.mockResolvedValue(exportResult());
  mocks.exportCsv.mockResolvedValue(
    exportResult({
      path: "/Users/avukat/Cikti/kira.csv",
      record: { id: "ex-2", exportType: "csv", status: "ok", outputHash: null, createdAt: ISO },
    }),
  );
  mocks.exportDocx.mockResolvedValue(exportResult());
  mocks.exportPdf.mockResolvedValue(exportResult());
  dialog.save.mockResolvedValue("/Users/avukat/Cikti/kira.json");
  dialog.open.mockResolvedValue("/Users/avukat/Devir");
});


afterEach(() => {
  cleanup();
});

async function renderScreen() {
  render(<Exports />);
  await screen.findByText(coverageHeadline(FULL));
}

/** Yeşil sonuç bildirimi (metni `<strong>` ile bölünür; düğüm olarak aranır). */
async function findOkNotice(): Promise<HTMLElement> {
  return waitFor(() => {
    const el = document.querySelector(".notice.ok");
    if (!el) throw new Error("yeşil bildirim yok");
    return el as HTMLElement;
  });
}

/* -------------------------------------------------------------------- tests */

describe("Çıktılar ekranı", () => {
  it("dosya listesini kasadan ister, ilk dosyayı seçer ve kapsamı o dosya için okur", async () => {
    await renderScreen();

    expect(mocks.caseList).toHaveBeenCalledWith(false);
    expect(mocks.caseCoverage).toHaveBeenCalledTimes(1);
    expect(mocks.caseCoverage).toHaveBeenCalledWith("p1");

    const select = screen.getByLabelText(t("common.case")) as HTMLSelectElement;
    expect(select.value).toBe("p1");
    expect(screen.getByRole("option", { name: "Kira uyuşmazlığı" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Ceza dosyası" })).toBeTruthy();

    // Kapsam künyesi sunucudan: rozet + tek satır özet + on satır + cümle.
    expect(screen.getByText(coverageState(FULL).label)).toBeTruthy();
    expect(screen.getByText(coverageState(FULL).detail)).toBeTruthy();
    expect(screen.getByText(coverageHeadline(FULL))).toBeTruthy();
    for (const row of coverageRows(FULL)) {
      expect(screen.getByText(row.label)).toBeTruthy();
      expect(screen.getAllByText(String(row.value)).length).toBeGreaterThan(0);
    }
    expect(screen.getByText(FULL.statement)).toBeTruthy();
    expect(screen.queryByText(t("exports.coverageWarnings"))).toBeNull();
  });

  it("kısmi kapsamda uyarıları ve “incelenmedi ≠ temiz” cümlesini yazar", async () => {
    mocks.caseCoverage.mockResolvedValue(PARTIAL);
    render(<Exports />);

    await screen.findByText(coverageHeadline(PARTIAL));
    expect(screen.getByText(coverageState(PARTIAL).label)).toBeTruthy();
    expect(screen.getByText(coverageState(PARTIAL).detail)).toBeTruthy();
    expect(screen.getByText(t("exports.coverageWarnings"))).toBeTruthy();
    for (const warning of PARTIAL.warnings) {
      expect(screen.getByText(warning)).toBeTruthy();
    }
    expect(screen.getByText(PARTIAL.statement)).toBeTruthy();
    expect(emptyCoverageWarning(PARTIAL)).toBeNull();
  });

  it("belgesiz dosyada “Kapsam yok” ve dürüst uyarı yazar", async () => {
    mocks.caseCoverage.mockResolvedValue(EMPTY);
    render(<Exports />);

    await screen.findByText(coverageHeadline(EMPTY));
    expect(screen.getByText(coverageState(EMPTY).label)).toBeTruthy();
    expect(screen.getByText(t("exportCoverage.emptyWarning"))).toBeTruthy();
    // Sıfır sayıları uydurulmaz: hepsi sunucudan gelir.
    expect(screen.getByText(EMPTY.statement)).toBeTruthy();
  });

  it("dosya yokken kapsam isteği gitmez ve altı düğme de kapalıdır", async () => {
    mocks.caseList.mockResolvedValue([]);
    render(<Exports />);

    await screen.findByRole("option", { name: t("common.noCase") });
    expect(mocks.caseCoverage).not.toHaveBeenCalled();

    const buttons = screen.getAllByRole("button");
    const enabled = buttons.filter((b) => !(b as HTMLButtonElement).disabled);
    expect(enabled).toHaveLength(0);
    expect(buttons).toHaveLength(6); // 4 çıktı + kapsam tazele + paket
  });

  it("JSON çıktısında kaydet diyaloğu dosya başlığından ad üretir ve sözleşmeyle yazar", async () => {
    await renderScreen();

    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[0]!);

    await waitFor(() => expect(mocks.exportJson).toHaveBeenCalledTimes(1));
    expect(dialog.save).toHaveBeenCalledWith({
      title: t("exports.saveDialog"),
      defaultPath: `${fileStem(P1.title)}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    expect(mocks.exportJson).toHaveBeenCalledWith("p1", "/Users/avukat/Cikti/kira.json");
    expect(mocks.exportCsv).not.toHaveBeenCalled();

    const notice = await findOkNotice();
    expect(notice.textContent).toContain("JSON");
    expect(notice.textContent).toContain("/Users/avukat/Cikti/kira.json");
    expect(notice.textContent).toContain(`20480 ${t("units.byte")}`);
    expect(notice.textContent).toContain("0123456789abcdef…");
    expect(notice.textContent).toContain(t("exportCoverage.embedWhere.json"));

    // Gösterilen kapsam yanıttan gelir.
    expect(screen.getByText(coverageHeadline(FULL))).toBeTruthy();
  });

  it("iptal edilen kaydet diyaloğu istek üretmez", async () => {
    await renderScreen();
    dialog.save.mockResolvedValue(null);

    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[1]!);
    await waitFor(() => expect(dialog.save).toHaveBeenCalledTimes(1));

    expect(mocks.exportCsv).not.toHaveBeenCalled();
    expect(screen.queryByText(t("exports.writing"))).toBeNull();
  });

  it("düşen dışa aktarma önceki yeşil sonucu bırakmaz", async () => {
    await renderScreen();
    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[0]!);
    await findOkNotice();

    mocks.exportCsv.mockRejectedValue(apiError("E_DISK_FULL", "Disk dolu."));
    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[1]!);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DISK_FULL");
    expect(document.querySelector(".notice.ok")).toBeNull();
  });

  it("yazarken düğme “Yazılıyor…” olur ve öteki çıktılar kilitlenir", async () => {
    await renderScreen();
    const pending = deferred<ExportResult>();
    mocks.exportJson.mockReturnValue(pending.promise);

    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[0]!);

    const writing = await screen.findByRole("button", { name: t("exports.writing") });
    expect((writing as HTMLButtonElement).disabled).toBe(true);
    for (const other of screen.getAllByRole("button", { name: t("common.save") })) {
      expect((other as HTMLButtonElement).disabled).toBe(true);
    }
    expect((screen.getByRole("button", { name: t("exports.packageCreate") }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: t("exports.refresh") }) as HTMLButtonElement).disabled).toBe(true);

    pending.resolve(exportResult());
    await findOkNotice();
    await waitFor(() =>
      expect((screen.getAllByRole("button", { name: t("common.save") })[0] as HTMLButtonElement).disabled).toBe(false),
    );
  });

  it("kapsam tazeleme yeni değerleri yazar ve düşen tazelemenin hatasını sonraki başarıda siler", async () => {
    await renderScreen();

    mocks.caseCoverage.mockResolvedValueOnce(PARTIAL);
    fireEvent.click(screen.getByRole("button", { name: t("exports.refresh") }));
    await screen.findByText(coverageHeadline(PARTIAL));

    mocks.caseCoverage.mockRejectedValueOnce(apiError("E_DB", "Kasa okunamadı."));
    fireEvent.click(screen.getByRole("button", { name: t("exports.refresh") }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DB");

    // Başarılı tazeleme eski kırmızıyı bırakmaz.
    mocks.caseCoverage.mockResolvedValueOnce(FULL);
    fireEvent.click(screen.getByRole("button", { name: t("exports.refresh") }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(screen.getByText(coverageHeadline(FULL))).toBeTruthy();
  });

  it("devir paketi parolayı 10 karakter eşiğinde gönderir", async () => {
    await renderScreen();

    fireEvent.change(screen.getByLabelText(t("exports.packagePassword")), { target: { value: "kisa" } });
    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));

    await waitFor(() => expect(mocks.exportPackage).toHaveBeenCalledTimes(1));
    expect(dialog.open).toHaveBeenCalledWith({
      directory: true,
      multiple: false,
      title: t("exports.dirDialog"),
    });
    expect(mocks.exportPackage).toHaveBeenCalledWith("p1", "/Users/avukat/Devir", undefined);

    mocks.exportPackage.mockResolvedValue(packageResult());
    fireEvent.change(screen.getByLabelText(t("exports.packagePassword")), {
      target: { value: "cok-guclu-parola" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));
    await waitFor(() => expect(mocks.exportPackage).toHaveBeenCalledTimes(2));
    expect(mocks.exportPackage).toHaveBeenLastCalledWith("p1", "/Users/avukat/Devir", "cok-guclu-parola");
  });

  it("paket künyesini ve şifreli paket yolunu yazar", async () => {
    await renderScreen();
    const created = packageResult({ sealedPath: "/Users/avukat/Devir/kira-paket.dimpak" });
    mocks.exportPackage.mockResolvedValue(created);

    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));

    const ready = await findOkNotice();
    expect(ready.textContent).toContain(t("exports.packageReady", { path: created.path }));
    expect(ready.textContent).toContain(
      t("exports.packageFiles", { files: created.files, hash: created.sha256.slice(0, 16) }),
    );
    expect(ready.textContent).toContain(created.sealedPath!);

  });

  it("iptal edilen klasör seçimi paket isteği üretmez", async () => {
    await renderScreen();
    dialog.open.mockResolvedValue(null);

    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));
    await waitFor(() => expect(dialog.open).toHaveBeenCalledTimes(1));
    expect(mocks.exportPackage).not.toHaveBeenCalled();
  });

  it("düşen paket isteği önceki paket satırını bırakmaz", async () => {
    await renderScreen();
    mocks.exportPackage.mockResolvedValueOnce(packageResult());
    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));
    await findOkNotice();

    mocks.exportPackage.mockRejectedValueOnce(apiError("E_IO", "Yazılamadı."));
    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_IO");
    expect(document.querySelector(".notice.ok")).toBeNull();
  });

  it("ÖLÇÜLEN KUSUR (A): paket başarısından sonra parola alanı temizlenir", async () => {
    await renderScreen();
    mocks.exportPackage.mockResolvedValue(packageResult());
    const input = screen.getByLabelText(t("exports.packagePassword")) as HTMLInputElement;

    fireEvent.change(input, { target: { value: "cok-guclu-parola" } });
    expect(input.value).toBe("cok-guclu-parola");
    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));

    await findOkNotice();
    await waitFor(() => expect((screen.getByLabelText(t("exports.packagePassword")) as HTMLInputElement).value).toBe(""));

  });

  it("ÖLÇÜLEN KUSUR (B): dosya değişince önceki kapsam, çıktı ve paket satırları kalkar", async () => {
    await renderScreen();
    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[0]!);
    const written = await screen.findByText(/Cikti\/kira\.json/);
    mocks.exportPackage.mockResolvedValue(packageResult());
    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));
    await waitFor(() => expect(document.querySelectorAll(".notice.ok")).toHaveLength(2));

    // Yeni dosyanın kapsamı askıda: eski veri ekranda kalmamalı.
    const pending = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValue(pending.promise);
    fireEvent.change(screen.getByLabelText(t("common.case")), { target: { value: "p2" } });

    await waitFor(() => expect(screen.queryByText(/Cikti\/kira\.json/)).toBeNull());
    expect(written.isConnected).toBe(false);
    expect(document.querySelector(".notice.ok")).toBeNull();
    expect(screen.queryByText(coverageHeadline(FULL))).toBeNull();
    expect(screen.queryByText(coverageState(FULL).label)).toBeNull();
    expect(screen.getByText(t("exports.coverageLoading"))).toBeTruthy();
  });

  it("dosya değişince önceki dosyanın hatası da kalkar", async () => {
    await renderScreen();
    mocks.exportJson.mockRejectedValueOnce(apiError("E_DISK_FULL", "Disk dolu."));
    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[0]!);
    expect((await screen.findByRole("alert")).textContent).toContain("E_DISK_FULL");

    mocks.caseCoverage.mockResolvedValueOnce(PARTIAL);
    fireEvent.change(screen.getByLabelText(t("common.case")), { target: { value: "p2" } });

    await screen.findByText(coverageHeadline(PARTIAL));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("uçuştaki dışa aktarma dosya değişince yeni dosyanın altına yazılmaz", async () => {
    await renderScreen();
    const pending = deferred<ExportResult>();
    mocks.exportJson.mockReturnValue(pending.promise);
    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[0]!);
    await waitFor(() => expect(mocks.exportJson).toHaveBeenCalledTimes(1));

    mocks.caseCoverage.mockResolvedValueOnce(PARTIAL);
    fireEvent.change(screen.getByLabelText(t("common.case")), { target: { value: "p2" } });
    await screen.findByText(coverageHeadline(PARTIAL));

    pending.resolve(exportResult());
    // İstek p1 için koştu; p2 ekranında "yazıldı" satırı görünmemeli.
    await waitFor(() => expect(document.querySelector(".notice.ok")).toBeNull());
    expect(screen.queryByText(/Cikti\/kira\.json/)).toBeNull();
  });

  it("uçuştaki kapsam tazeleme dosya değişince yeni kapsamı ezmez", async () => {
    await renderScreen();
    const stale = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValueOnce(stale.promise);
    fireEvent.click(screen.getByRole("button", { name: t("exports.refresh") }));
    await waitFor(() => expect(mocks.caseCoverage).toHaveBeenCalledTimes(2));

    mocks.caseCoverage.mockResolvedValueOnce(PARTIAL);
    fireEvent.change(screen.getByLabelText(t("common.case")), { target: { value: "p2" } });
    await screen.findByText(coverageHeadline(PARTIAL));

    stale.resolve(FULL);
    await waitFor(() => expect(screen.queryByText(coverageHeadline(FULL))).toBeNull());
    expect(screen.getByText(coverageHeadline(PARTIAL))).toBeTruthy();
  });


  it("ÖLÇÜLEN KUSUR (C): düşen kapsam isteği önceki dosyanın kapsamını bırakmaz", async () => {
    await renderScreen();
    mocks.caseCoverage.mockRejectedValue(apiError("E_DB", "Kapsam okunamadı."));

    fireEvent.change(screen.getByLabelText(t("common.case")), { target: { value: "p2" } });

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DB");
    expect(screen.queryByText(coverageHeadline(FULL))).toBeNull();
    expect(screen.getByText(t("exports.coverageUnreadable"))).toBeTruthy();
  });

  it("ÖLÇÜLEN KUSUR (D): kapsam okunurken “okunamadı” yazılmaz", async () => {
    const pending = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValue(pending.promise);

    render(<Exports />);
    await waitFor(() => expect(mocks.caseCoverage).toHaveBeenCalledWith("p1"));
    expect(screen.queryByText(t("exports.coverageUnreadable"))).toBeNull();

    pending.resolve(FULL);
    await screen.findByText(coverageHeadline(FULL));
  });

  it("geç gelen eski kapsam yanıtı yeni dosyanın kapsamını ezmez", async () => {
    const first = deferred<ExportCoverage>();
    const second = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    render(<Exports />);
    await waitFor(() => expect(mocks.caseCoverage).toHaveBeenCalledWith("p1"));
    fireEvent.change(screen.getByLabelText(t("common.case")), { target: { value: "p2" } });
    await waitFor(() => expect(mocks.caseCoverage).toHaveBeenCalledWith("p2"));

    second.resolve(PARTIAL);
    await screen.findByText(coverageHeadline(PARTIAL));
    first.resolve(FULL);
    await waitFor(() => expect(screen.queryByText(coverageHeadline(FULL))).toBeNull());
    expect(screen.getByText(coverageHeadline(PARTIAL))).toBeTruthy();
  });

  it("dosya listesi düşerse dürüst hata yazar, sahte liste üretmez", async () => {
    mocks.caseList.mockRejectedValue(apiError("E_DB", "Kasa açılamadı."));
    render(<Exports />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DB");
    expect(screen.getByRole("option", { name: t("common.noCase") })).toBeTruthy();
    expect(mocks.caseCoverage).not.toHaveBeenCalled();
  });
});
