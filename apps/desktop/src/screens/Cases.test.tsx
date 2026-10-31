// @vitest-environment jsdom
//
// Dosyalar (Cases) ekranı davranış testi. Lib düzeyindeki testler (`cases`
// sözlük bağları, `parseProgress`, `budget`, `ocr`) parçaların **metnini**
// doğrular; burada ekran gerçekten render edilip ürün kararlarının arayüzde
// uygulandığı ölçülür. Ölçülen kararlar:
//
// 1. Liste kapsamı: "Arşiv" kutusu isteği `includeArchived: true` yapar; alt
//    satır sayısı ve kapsamı sunucudan gelen listeyle yazılır.
// 2. Dosya oluşturma: boş başlık **gönderilmez** (E_TITLE yazılır); geçerli
//    başlık kırpılarak gider, form kapanır ve yeni dosya seçilir.
// 3. Arşivle / geri al / sil: düğme dosyanın **durumuna** göre çıkar; işlem
//    sonrası seçim kapanır ve liste tazelenir.
// 4. Belge satırı: sürümü olmayan belgenin Oku/Ayrıştır/OCR düğmeleri kapalıdır
//    (açılamayan belge için düğme sunulmaz).
// 5. Ayrıştırma isteği **tam sözleşmeyle** gider: OCR düğmesi `ocr=true`,
//    kodlama seçimden, parola yalnız doluyken (kırpılmadan) gönderilir.
// 6. Ayrıştırma özeti: uyarı sayısı, kodlama notu **worker uyarısından** ve
//    bütçe satırı çekirdekten gelir; kodlama notu yoksa **uydurulmaz**.
// 7. Toplu ayrıştırma: bekleyen yoksa açıkça yazılır; kısmi başarı gerçek
//    sayılarla (1 / 2, 1 başarısız) ve belge başına sonuçla gösterilir.
// 8. Etiketler: sunucudan belgeye göre gruplanır, Enter ile eklenir, × ile
//    kaldırılır (yerel tahmin yok, sunucunun döndürdüğü liste yazılır).
// 9. Kaynak alıntı: "Okuyucuda aç" gerçek konumu çeker ve okuyucuyu o sürümle
//    açar; kayıt yoksa E_ANCHOR_MISSING yazılır.
// 10. Dosya değişince önceki dosyanın belgeleri, kaynakları, toplu sonucu ve
//     bildirimi **ekranda kalmaz** (INV-05: dosya verisi başka dosyaya örtük
//     eklenmez).
// 11. Başarısız ayrıştırmadan sonra başarılı ayrıştırma eski hata bildirimini
//     bırakmaz (çelişkili ekran: kırmızı hata + yeşil başarı).

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { budgetSummary } from "../lib/budget";
import { t } from "../lib/i18n";
import { DEFAULT_OCR_LANG } from "../lib/ocr";
import type {
  AnchorLocationRow,
  AnchorRow,
  BatchParseResult,
  CaseRow,
  DocumentRow,
  DocumentTagRow,
  ParseResult,
  WorkerBudgetView,
} from "../lib/api";

const mocks = vi.hoisted(() => ({
  caseList: vi.fn(),
  caseCreate: vi.fn(),
  caseArchive: vi.fn(),
  caseRestore: vi.fn(),
  caseDelete: vi.fn(),
  documentList: vi.fn(),
  documentImport: vi.fn(),
  documentParse: vi.fn(),
  documentsParsePending: vi.fn(),
  documentTagsForCase: vi.fn(),
  documentTagAdd: vi.fn(),
  documentTagRemove: vi.fn(),
  anchorList: vi.fn(),
  anchorLocate: vi.fn(),
  documentPages: vi.fn(),
  marksForVersion: vi.fn(),
  noteList: vi.fn(),
  noteDraftGet: vi.fn(),
  openFileDialog: vi.fn(),
  onDragDropEvent: vi.fn(),
  listen: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      caseList: mocks.caseList,
      caseCreate: mocks.caseCreate,
      caseArchive: mocks.caseArchive,
      caseRestore: mocks.caseRestore,
      caseDelete: mocks.caseDelete,
      documentList: mocks.documentList,
      documentImport: mocks.documentImport,
      documentParse: mocks.documentParse,
      documentsParsePending: mocks.documentsParsePending,
      documentTagsForCase: mocks.documentTagsForCase,
      documentTagAdd: mocks.documentTagAdd,
      documentTagRemove: mocks.documentTagRemove,
      anchorList: mocks.anchorList,
      anchorLocate: mocks.anchorLocate,
      documentPages: mocks.documentPages,
      marksForVersion: mocks.marksForVersion,
      noteList: mocks.noteList,
      noteDraftGet: mocks.noteDraftGet,
    },
  };
});

// Tauri sınırı: jsdom'da pencere/olay/köprü yok. Ekran bu üçünü çağırıyor;
// davranış testi için yalnız "bağlanabildi" varsayımı yeterli.
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: mocks.openFileDialog }));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: mocks.onDragDropEvent }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

const { Cases } = await import("./Cases");

const ISO = "2026-10-01T00:00:00Z";

const CASE: CaseRow = {
  id: "c1",
  title: "Kira uyuşmazlığı",
  caseKind: "review",
  referenceCode: "2026/12",
  jurisdictionLabel: "İstanbul",
  practiceArea: "Kira",
  status: "active",
  sensitivity: "normal",
  defaultAiPolicy: "local_only",
  archivedAt: null,
  retentionHold: false,
  createdAt: ISO,
  updatedAt: ISO,
  rowVersion: 2,
};

const CASE2: CaseRow = { ...CASE, id: "c2", title: "İhtar dosyası", referenceCode: null };
const ARCHIVED: CaseRow = { ...CASE, id: "c3", title: "Kapanmış dosya", status: "archived" };

const DOC: DocumentRow = {
  id: "d1",
  caseId: "c1",
  displayName: "Kira sözleşmesi.pdf",
  documentType: "pdf",
  status: "parsed",
  currentVersionId: "12345678-abcd",
  declaredDocumentDate: null,
  createdAt: ISO,
  updatedAt: ISO,
  rowVersion: 1,
};

/** Sürümü olmayan belge: Oku/Ayrıştır/OCR sunulmamalı. */
const NO_VERSION: DocumentRow = {
  ...DOC,
  id: "d2",
  displayName: "Taranmış dilekçe.pdf",
  currentVersionId: null,
};

const ANCHOR: AnchorRow = {
  id: "a1",
  caseId: "c1",
  documentVersionId: "12345678-abcd",
  parseRunId: "run-1",
  pageId: "p1",
  segmentId: "seg-1",
  startOffset: 120,
  endOffset: 139,
  exactQuote: "kira bedeli ödenmedi",
  prefix: "",
  suffix: "",
  quoteHash: "hash",
  anchorStatus: "current",
  createdAt: ISO,
};

const BUDGET: WorkerBudgetView = {
  kind: "parse",
  label: "Ayrıştırma",
  cpuSeconds: 120,
  memoryBytes: 2048 * 1024 * 1024,
  outputBytes: 64 * 1024 * 1024,
  openFiles: 128,
  wallClockSeconds: 600,
  elapsedMs: 1500,
  observedOutputBytes: 4096,
  peakMemoryBytes: 300 * 1024 * 1024,
  violated: null,
};

function parseResult(overrides: Partial<ParseResult> = {}): ParseResult {
  return {
    parseRun: {
      id: "run-1",
      documentVersionId: "12345678-abcd",
      caseId: "c1",
      engine: "pymupdf",
      engineVersion: "1.24.14",
      status: "parsed",
      pagesExpected: 3,
      pagesProcessed: 3,
      warnings: "",
      createdAt: ISO,
    },
    pages: 3,
    storedSegments: 12,
    warnings: [],
    budget: null,
    ...overrides,
  };
}

function batchResult(overrides: Partial<BatchParseResult> = {}): BatchParseResult {
  return {
    requested: 2,
    succeeded: 1,
    failed: 1,
    pages: 5,
    peakParallel: 2,
    results: [
      {
        versionId: "12345678-abcd",
        documentId: "d1",
        displayName: DOC.displayName,
        ok: true,
        pages: 3,
        code: null,
        message: null,
      },
      {
        versionId: "87654321-abcd",
        documentId: "d2",
        displayName: NO_VERSION.displayName,
        ok: false,
        pages: 0,
        code: "E_PARSE_TIMEOUT",
        message: "worker yanıt vermedi",
      },
    ],
    ...overrides,
  };
}

function anchorLocation(overrides: Partial<AnchorLocationRow> = {}): AnchorLocationRow {
  return {
    anchorId: "a1",
    caseId: "c1",
    documentVersionId: "12345678-abcd",
    documentId: "d1",
    documentName: DOC.displayName,
    parseRunId: "run-1",
    pageId: "p1",
    pageIndex: 0,
    segmentId: "seg-1",
    startOffset: 120,
    endOffset: 139,
    exactQuote: ANCHOR.exactQuote,
    anchorStatus: "current",
    quoteIntact: true,
    quads: [],
    coordinateSpace: null,
    pageRotation: 0,
    isCurrentVersion: true,
    currentVersionId: "12345678-abcd",
    versionNo: 1,
    currentVersionNo: 1,
    isCurrentRun: true,
    ...overrides,
  };
}

function tagRow(documentId: string, tag: string): DocumentTagRow {
  return { documentId, caseId: "c1", tag, createdAt: ISO };
}

type SetupOptions = {
  cases?: CaseRow[];
  /** Çağrı başına farklı liste (ör. arşiv kapsamı değişimi). */
  caseListImpl?: () => Promise<CaseRow[]>;
  documents?: (caseId: string) => Promise<DocumentRow[]>;
  anchors?: (caseId: string) => Promise<AnchorRow[]>;
  tags?: (caseId: string) => Promise<DocumentTagRow[]>;
};

function setup(options: SetupOptions = {}) {
  if (options.caseListImpl) mocks.caseList.mockImplementation(options.caseListImpl);
  else mocks.caseList.mockResolvedValue(options.cases ?? [CASE]);
  mocks.documentList.mockImplementation(options.documents ?? (async () => [DOC]));
  mocks.anchorList.mockImplementation(options.anchors ?? (async () => []));
  mocks.documentTagsForCase.mockImplementation(options.tags ?? (async () => []));
  mocks.noteList.mockResolvedValue([]);
  mocks.noteDraftGet.mockResolvedValue(null);
  mocks.marksForVersion.mockResolvedValue([]);
  mocks.documentPages.mockResolvedValue(null);
  mocks.listen.mockResolvedValue(() => undefined);
  mocks.onDragDropEvent.mockResolvedValue(() => undefined);
  render(<Cases />);
}

/** Listeden dosya seçer ve künye kartının açıldığını doğrular. */
async function selectCase(title: string) {
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(title) }));
  expect(await screen.findByRole("heading", { name: title })).toBeTruthy();
}

/** Başarı bildirimi (yalnız Dosyalar ekranının `notice ok` kutusu). */
function noticeText(): string | null {
  return document.querySelector(".notice.ok")?.textContent ?? null;
}

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) throw new Error(`#${id} yok`);
  return element as T;
}

describe("dosyalar ekranı davranışı", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("arşiv kutusu isteği değiştirir; alt satır sayıyı ve kapsamı sunucudan yazar", async () => {
    let calls = 0;
    setup({ caseListImpl: async () => (calls++ === 0 ? [CASE] : []) });

    expect(await screen.findByText(CASE.title)).toBeTruthy();
    expect(mocks.caseList).toHaveBeenCalledWith(false);
    expect(
      screen.getByText(t("cases.listSubtitle", { count: 1, scope: t("cases.scopeActive") })),
    ).toBeTruthy();

    fireEvent.click(screen.getByRole("checkbox"));

    await waitFor(() => expect(mocks.caseList).toHaveBeenCalledWith(true));
    expect(await screen.findByText(t("cases.emptyArchived"))).toBeTruthy();
    expect(
      screen.getByText(t("cases.listSubtitle", { count: 0, scope: t("cases.scopeArchived") })),
    ).toBeTruthy();
    // Kapsam değişince aktif boş durum metni kalmaz.
    expect(screen.queryByText(t("cases.emptyActive"))).toBeNull();
  });

  it("boş başlıkla dosya oluşturulmaz; geçerli başlık kırpılarak gider ve yeni dosya seçilir", async () => {
    const created: CaseRow = { ...CASE, id: "c9", title: "Kira alacağı", referenceCode: null };
    mocks.caseCreate.mockResolvedValue(created);
    setup();

    fireEvent.click(await screen.findByRole("button", { name: t("cases.newCase") }));
    fireEvent.click(screen.getByRole("button", { name: t("cases.create") }));

    // Boş başlık sunucuya gitmez; kullanıcıya gerçek hata kodu yazılır.
    expect(await screen.findByText("E_TITLE")).toBeTruthy();
    expect(screen.getByText(t("cases.titleRequired"), { exact: false })).toBeTruthy();
    expect(mocks.caseCreate).not.toHaveBeenCalled();

    fireEvent.change(byId<HTMLInputElement>("case-title"), {
      target: { value: "  Kira alacağı  " },
    });
    fireEvent.change(byId<HTMLInputElement>("case-ref"), { target: { value: "  " } });
    fireEvent.click(screen.getByRole("button", { name: t("cases.create") }));

    await waitFor(() => expect(mocks.caseCreate).toHaveBeenCalledTimes(1));
    expect(mocks.caseCreate).toHaveBeenCalledWith({
      title: "Kira alacağı",
      caseKind: "review",
      referenceCode: null,
      jurisdictionLabel: null,
      practiceArea: null,
      sensitivity: "normal",
      defaultAiPolicy: "off",
    });
    // Form kapandı ve yeni dosya künyesi açıldı.
    expect(await screen.findByRole("heading", { name: "Kira alacağı" })).toBeTruthy();
    expect(screen.getByRole("button", { name: t("cases.newCase") })).toBeTruthy();
  });

  it("arşivle ve sil düğmeleri dosya durumuna göre çıkar; işlem sonrası seçim kapanır", async () => {
    mocks.caseArchive.mockResolvedValue({ ...CASE, status: "archived" });
    mocks.caseDelete.mockResolvedValue(undefined);
    setup({ cases: [CASE] });

    await selectCase(CASE.title);
    expect(screen.queryByRole("button", { name: t("cases.restore") })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: t("cases.archive") }));
    await waitFor(() => expect(mocks.caseArchive).toHaveBeenCalledWith("c1"));
    // Seçim kapanır: künye kartı ve belge listesi ekranda kalmaz.
    await waitFor(() => expect(screen.queryByRole("heading", { name: CASE.title })).toBeNull());
    expect(mocks.caseList).toHaveBeenCalledTimes(2);

    await selectCase(CASE.title);
    fireEvent.click(screen.getByRole("button", { name: t("cases.delete") }));
    await waitFor(() => expect(mocks.caseDelete).toHaveBeenCalledWith("c1"));
  });

  it("arşivli dosyada geri al düğmesi çıkar, arşivle çıkmaz", async () => {
    mocks.caseRestore.mockResolvedValue({ ...ARCHIVED, status: "active" });
    setup({ cases: [ARCHIVED], documents: async () => [] });

    await selectCase(ARCHIVED.title);
    expect(screen.queryByRole("button", { name: t("cases.archive") })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: t("cases.restore") }));
    await waitFor(() => expect(mocks.caseRestore).toHaveBeenCalledWith("c3"));
  });

  it("sürümü olmayan belgenin Oku/Ayrıştır/OCR düğmeleri kapalıdır", async () => {
    setup({ documents: async () => [DOC, NO_VERSION] });

    await selectCase(CASE.title);
    await waitFor(() => expect(screen.getByText(DOC.displayName)).toBeTruthy());
    expect(screen.getByText("12345678")).toBeTruthy();
    expect(screen.getByText(t("cases.noVersion"))).toBeTruthy();

    const read = screen.getAllByRole("button", { name: t("cases.read") }) as HTMLButtonElement[];
    const parse = screen.getAllByRole("button", { name: t("cases.parse") }) as HTMLButtonElement[];
    const ocr = screen.getAllByRole("button", { name: t("cases.ocr") }) as HTMLButtonElement[];
    expect(read).toHaveLength(2);
    expect([read[0]!.disabled, read[1]!.disabled]).toEqual([false, true]);
    expect([parse[0]!.disabled, parse[1]!.disabled]).toEqual([false, true]);
    expect([ocr[0]!.disabled, ocr[1]!.disabled]).toEqual([false, true]);
  });

  it("ayrıştırma isteği tam sözleşmeyle gider: OCR bayrağı, kodlama ve yalnız dolu parola", async () => {
    mocks.documentParse.mockResolvedValue(parseResult());
    setup();

    await selectCase(CASE.title);
    await waitFor(() => expect(screen.getByText(DOC.displayName)).toBeTruthy());

    fireEvent.click(screen.getByRole("button", { name: t("cases.parse") }));
    await waitFor(() => expect(mocks.documentParse).toHaveBeenCalledTimes(1));
    expect(mocks.documentParse).toHaveBeenLastCalledWith(
      "12345678-abcd",
      false,
      DEFAULT_OCR_LANG,
      "auto",
      undefined,
    );
    await waitFor(() => expect(noticeText()).not.toBeNull());

    fireEvent.change(byId<HTMLSelectElement>("txt-encoding"), {
      target: { value: "windows-1254" },
    });
    fireEvent.change(byId<HTMLInputElement>("pdf-password"), {
      target: { value: " giZli " },
    });
    fireEvent.click(screen.getByRole("button", { name: t("cases.ocr") }));

    await waitFor(() => expect(mocks.documentParse).toHaveBeenCalledTimes(2));
    // Parola kırpılmaz (boşluk parolanın parçası olabilir); OCR bayrağı gerçekten true.
    expect(mocks.documentParse).toHaveBeenLastCalledWith(
      "12345678-abcd",
      true,
      DEFAULT_OCR_LANG,
      "windows-1254",
      " giZli ",
    );
  });

  it("ayrıştırma özeti uyarı sayısını, worker'ın kodlama notunu ve bütçe satırını yazar", async () => {
    mocks.documentParse.mockResolvedValue(
      parseResult({
        warnings: ["OCR: 3 sayfa işlendi", "kodlama: windows-1254 kullanıldı"],
        budget: BUDGET,
      }),
    );
    setup();

    await selectCase(CASE.title);
    fireEvent.click(await screen.findByRole("button", { name: t("cases.parse") }));

    const expected = [
      t("cases.parseSummary", { status: "parsed", pages: 3, segments: 12 }),
      t("cases.parseWarnings", { count: 2 }),
      "kodlama: windows-1254 kullanıldı",
      budgetSummary(BUDGET),
    ].join(" · ");
    await waitFor(() => expect(noticeText()).toBe(expected));
  });

  it("kodlama uyarısı yoksa kodlama notu uydurulmaz, bütçe yoksa bütçe satırı yazılmaz", async () => {
    mocks.documentParse.mockResolvedValue(
      parseResult({ warnings: ["boş sayfa atlandı"], budget: null }),
    );
    setup();

    await selectCase(CASE.title);
    fireEvent.click(await screen.findByRole("button", { name: t("cases.parse") }));

    const expected = [
      t("cases.parseSummary", { status: "parsed", pages: 3, segments: 12 }),
      t("cases.parseWarnings", { count: 1 }),
    ].join(" · ");
    await waitFor(() => expect(noticeText()).toBe(expected));
  });

  it("toplu ayrıştırma: bekleyen yokken açıkça yazar, kısmi başarıyı gerçek sayılarla gösterir", async () => {
    mocks.documentsParsePending.mockResolvedValueOnce(
      batchResult({ requested: 0, succeeded: 0, failed: 0, pages: 0, peakParallel: 0, results: [] }),
    );
    mocks.documentsParsePending.mockResolvedValueOnce(batchResult());
    setup();

    await selectCase(CASE.title);
    await waitFor(() => expect(screen.getByText(DOC.displayName)).toBeTruthy());

    fireEvent.click(screen.getByRole("button", { name: t("cases.parsePending") }));
    await waitFor(() => expect(mocks.documentsParsePending).toHaveBeenCalledWith("c1", 4));
    expect(
      await screen.findByText(t("cases.batchResult", { succeeded: 0, requested: 0 })),
    ).toBeTruthy();
    expect(screen.getByText(t("cases.batchNonePending"))).toBeTruthy();

    mocks.documentsParsePending.mockResolvedValueOnce(batchResult());
    fireEvent.click(screen.getByRole("button", { name: t("cases.parsePending") }));

    expect(
      await screen.findByText(t("cases.batchResult", { succeeded: 1, requested: 2 })),
    ).toBeTruthy();
    expect(screen.getByText(t("cases.batchFailed", { count: 1 }), { exact: false })).toBeTruthy();
    expect(screen.getByText(t("cases.batchPages", { count: 3 }))).toBeTruthy();
    // Başarısız belge kendi kodunu ve nedenini taşır.
    expect(screen.getByText("E_PARSE_TIMEOUT")).toBeTruthy();
    expect(screen.getByText(/worker yanıt vermedi/)).toBeTruthy();
  });

  it("belge etiketleri sunucudan gruplanır; Enter ekler, × kaldırır", async () => {
    mocks.documentTagAdd.mockResolvedValue(["kira", "2026", "yeni"]);
    mocks.documentTagRemove.mockResolvedValue(["2026", "yeni"]);
    setup({ tags: async () => [tagRow("d1", "kira"), tagRow("d1", "2026")] });

    await selectCase(CASE.title);
    expect(await screen.findByText("#kira")).toBeTruthy();
    expect(screen.getByText("#2026")).toBeTruthy();

    const input = screen.getByLabelText(t("cases.tagAria", { name: DOC.displayName }));
    fireEvent.change(input, { target: { value: "  yeni  " } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(mocks.documentTagAdd).toHaveBeenCalledWith("d1", "yeni"));
    expect(await screen.findByText("#yeni")).toBeTruthy();
    // Sunucunun döndürdüğü liste yazıldıktan sonra giriş temizlenir.
    await waitFor(() => expect((input as HTMLInputElement).value).toBe(""));

    fireEvent.click(screen.getByTitle(t("cases.removeTagTitle", { tag: "kira" })));
    await waitFor(() => expect(mocks.documentTagRemove).toHaveBeenCalledWith("d1", "kira"));
    await waitFor(() => expect(screen.queryByText("#kira")).toBeNull());
  });

  it("kaynak alıntı okuyucuyu gerçek konumla açar; kayıt yoksa E_ANCHOR_MISSING yazar", async () => {
    mocks.documentTagsForCase.mockResolvedValue([]);
    mocks.anchorLocate.mockResolvedValue(anchorLocation());
    setup({ anchors: async () => [ANCHOR] });

    await selectCase(CASE.title);
    expect((await screen.findAllByText(/kira bedeli ödenmedi/)).length).toBeGreaterThan(0);
    expect(screen.getByText(/karakter 120–139/)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: t("cases.openInReader") }));
    await waitFor(() => expect(mocks.anchorLocate).toHaveBeenCalledWith("a1"));
    // Okuyucu anchor'ın sürümüyle açılır: sol panel görünür.
    expect(await screen.findByLabelText(t("readerUi.leftDocument"))).toBeTruthy();
    expect(mocks.documentPages).toHaveBeenCalledWith("12345678-abcd");

    // Konum bulunamazsa okuyucu açılmaz; gerçek hata kodu yazılır.
    cleanup();
    mocks.anchorLocate.mockResolvedValue(null);
    setup({ anchors: async () => [ANCHOR] });
    await selectCase(CASE.title);
    fireEvent.click(await screen.findByRole("button", { name: t("cases.openInReader") }));
    expect(await screen.findByText("E_ANCHOR_MISSING")).toBeTruthy();
    expect(screen.getByText(t("cases.anchorMissing"), { exact: false })).toBeTruthy();
    expect(screen.queryByLabelText(t("readerUi.leftDocument"))).toBeNull();
  });

  it("dosya değişince önceki dosyanın belgeleri, kaynakları, toplu sonucu ve bildirimi kalmaz", async () => {
    mocks.documentParse.mockResolvedValue(parseResult());
    mocks.documentsParsePending.mockResolvedValue(batchResult());
    // İkinci dosyanın verisi **askıda**: eski verinin temizlendiği pencere ölçülür,
    // sonra gerçekten yeni dosyanın verisi geldiğinde kartın dolduğu doğrulanır.
    let resolveDocs: (rows: DocumentRow[]) => void = () => undefined;
    const pendingDocs = new Promise<DocumentRow[]>((resolve) => {
      resolveDocs = resolve;
    });
    setup({
      cases: [CASE, CASE2],
      documents: (caseId) => (caseId === "c1" ? Promise.resolve([DOC]) : pendingDocs),
      anchors: (caseId) =>
        caseId === "c1" ? Promise.resolve([ANCHOR]) : new Promise<AnchorRow[]>(() => undefined),
    });

    await selectCase(CASE.title);
    expect((await screen.findAllByText(DOC.displayName)).length).toBeGreaterThan(0);
    expect((await screen.findAllByText(/kira bedeli ödenmedi/)).length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("button", { name: t("cases.parse") }));
    await waitFor(() => expect(noticeText()).not.toBeNull());
    fireEvent.click(screen.getByRole("button", { name: t("cases.parsePending") }));
    expect(
      await screen.findByText(t("cases.batchResult", { succeeded: 1, requested: 2 })),
    ).toBeTruthy();

    // Yeni dosyanın belgeleri/kaynakları henüz gelmedi (istek askıda):
    // bu pencerede önceki dosyanın verisi gösterilmemeli.
    fireEvent.click(await screen.findByRole("button", { name: new RegExp(CASE2.title) }));
    expect(await screen.findByRole("heading", { name: CASE2.title })).toBeTruthy();

    expect(screen.queryAllByText(DOC.displayName)).toHaveLength(0);
    expect(screen.queryAllByText(/kira bedeli ödenmedi/)).toHaveLength(0);
    expect(screen.queryByText(t("cases.batchResult", { succeeded: 1, requested: 2 }))).toBeNull();
    expect(noticeText()).toBeNull();
    // Bekleme açıkça yazılır: boş kart sessizce "veri yok" demez.
    expect(screen.getByText(t("cases.documentsEmpty"))).toBeTruthy();
    expect(screen.getByText(t("cases.anchorsEmpty"))).toBeTruthy();

    // Yeni dosyanın verisi gelince kart gerçekten dolar (temizlik yüklemeyi kesmez).
    resolveDocs([
      { ...DOC, id: "d9", caseId: "c2", displayName: "İhtar mektubu.pdf", currentVersionId: "c2-v1" },
    ]);
    expect(await screen.findByText("İhtar mektubu.pdf")).toBeTruthy();
    expect(screen.queryByText(t("cases.documentsEmpty"))).toBeNull();
  });

  it("başarısız ayrıştırmadan sonra başarılı ayrıştırma eski hata bildirimini bırakmaz", async () => {
    mocks.documentParse.mockRejectedValueOnce({
      code: "E_PARSE",
      message: "ayrıştırma hatası",
      retryable: true,
    });
    mocks.documentParse.mockResolvedValue(parseResult());
    setup();

    await selectCase(CASE.title);
    fireEvent.click(await screen.findByRole("button", { name: t("cases.parse") }));
    expect(await screen.findByText("E_PARSE")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: t("cases.parse") }));
    await waitFor(() => expect(noticeText()).not.toBeNull());
    // Aynı ekranda hem kırmızı hata hem yeşil başarı gösterilmez.
    expect(screen.queryByText("E_PARSE")).toBeNull();
  });
});
