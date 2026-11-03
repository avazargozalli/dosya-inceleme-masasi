// @vitest-environment jsdom
//
// Okuyucu (DualReader) **kaynak yeniden bağlama** akışı davranış testi.
//
// `DualReader.selection.test.tsx` seçimden giden üç komutu, `DualReader.region.test.tsx`
// bölge işaretini ölçüyor. **Kopmuş/eski bağın yeniden bağlanması** (`anchorLocate`,
// `anchorRebindSuggestion`, `anchorRebind`) bugüne dek hiç ölçülmemişti; oysa bu yol
// kasaya **yazar** ve hangi bağa hangi offset'in yazıldığını belirler.
//
// Burada sabitlenenler:
// 1. Bağ künyesi çekirdekten okunur: kopmuş alıntı uyarısı, sürüm rozeti, arama düğmesi.
//    Sağlam ve güncel bağda düğme çizilmez.
// 2. Öneri listesi kendiliğinden uygulanmaz: durum metni + alıntı + aday sayfaları
//    çekirdekten gelir, "Kapat" listeyi kaldırır.
// 3. Seçilen aday tam sözleşmeyle yazılır (`anchorRebind(anchorId, segmentId, startOffset)`),
//    ardından güncel konum okunur, panel yeni sürüme geçer, yeşil bildirim çizilir.
// 4. **Ölçülen kusur (A):** uçuştaki öneri yanıtı hedef bağa bağlı değildi → başka bağa
//    geçildiğinde **önceki bağın adayları** yeni bağın altında çiziliyor ve "Bağla"
//    **yeni bağa eski adayın offset'ini** yazabiliyordu.
// 5. **Ölçülen kusur (B):** uçuştaki yeniden bağlama yanıtı hedef bağa bağlı değildi →
//    yeni bağın künyesi **önceki bağın konumuyla** eziliyor, panel eski sürüme zıplıyordu.
// 6. **Ölçülen kusur (C):** öneri araması başında hata silinmiyordu → düşen denemeden
//    sonra başarılı aramada kırmızı hata ekranda kalıyordu.
// 7. **Ölçülen kusur (D):** yeniden bağlama başında hata silinmiyordu → düşen denemeden
//    sonra başarılı bağlamada kırmızı hata ekranda kalıyordu.
//
// Dürüst sınır: jsdom tarayıcı değil; `api` mock'lu olduğu için `anchor_rebind`in
// birebir doğrulaması (segment offset ↔ alıntı), SQL ve Rust **koşmaz**.

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { t } from "../lib/i18n";
import type {
  AnchorLocationRow,
  DocumentPages,
  DocumentRow,
  PageRow,
  RebindSuggestion,
} from "../lib/api";
import type { ReaderTarget } from "../lib/reader";

const mocks = vi.hoisted(() => ({
  documentPages: vi.fn(),
  marksForVersion: vi.fn(),
  anchorLocate: vi.fn(),
  anchorRebindSuggestion: vi.fn(),
  anchorRebind: vi.fn(),
  documentPagePreview: vi.fn(),
  documentInventory: vi.fn(),
  markCreateRegion: vi.fn(),
  markCreateFromSelection: vi.fn(),
  anchorCreate: vi.fn(),
  noteCreateFromSelection: vi.fn(),
  markDelete: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      documentPages: mocks.documentPages,
      marksForVersion: mocks.marksForVersion,
      anchorLocate: mocks.anchorLocate,
      anchorRebindSuggestion: mocks.anchorRebindSuggestion,
      anchorRebind: mocks.anchorRebind,
      documentPagePreview: mocks.documentPagePreview,
      documentInventory: mocks.documentInventory,
      markCreateRegion: mocks.markCreateRegion,
      markCreateFromSelection: mocks.markCreateFromSelection,
      anchorCreate: mocks.anchorCreate,
      noteCreateFromSelection: mocks.noteCreateFromSelection,
      markDelete: mocks.markDelete,
    },
  };
});

const { DualReader } = await import("./DualReader");

const ISO = "2026-10-01T00:00:00Z";

const TEXT_A1 = "Kira sözleşmesi 1 Şubat 2026 tarihinde imzalanmıştır.";
const TEXT_A2 = "İkinci sayfa: ödeme planı ekte sunulmuştur.";
const TEXT_B1 = "İhtarname 3 Mart 2026 tarihinde tebliğ edilmiştir.";

function page(overrides: Partial<PageRow> & { id: string; pageIndex: number; text: string }): PageRow {
  return {
    segmentId: `seg-${overrides.id}`,
    textOrigin: "native",
    qualityFlags: "",
    width: 600,
    height: 800,
    rotation: 0,
    coordinateSpace: "unrotated_cropbox_points",
    pageLabel: null,
    blocks: [],
    ...overrides,
  };
}

function pagesFor(versionId: string, texts: string[]): DocumentPages {
  return {
    parseRun: {
      id: `run-${versionId}`,
      documentVersionId: versionId,
      status: "succeeded",
      engine: "pymupdf",
      engineVersion: "1.24.14",
      pagesProcessed: texts.length,
      warnings: "",
      createdAt: ISO,
    },
    pages: texts.map((text, index) => page({ id: `${versionId}-p${index + 1}`, pageIndex: index, text })),
  };
}

const DOC_A: DocumentRow = {
  id: "d1",
  caseId: "c1",
  displayName: "Kira sözleşmesi",
  documentType: "pdf",
  status: "parsed",
  currentVersionId: "v1",
  declaredDocumentDate: null,
  createdAt: ISO,
  updatedAt: ISO,
  rowVersion: 1,
};

const DOC_B: DocumentRow = { ...DOC_A, id: "d2", displayName: "İhtarname", currentVersionId: "v2" };

const PAIN_A = pagesFor("v1", [TEXT_A1, TEXT_A2]);
const PAIN_B = pagesFor("v2", [TEXT_B1]);

function anchorTarget(anchorId: string, versionId = "v1", documentName = DOC_A.displayName): ReaderTarget {
  return {
    versionId,
    documentName,
    pageId: `${versionId}-p1`,
    segmentId: `seg-${versionId}-p1`,
    pageIndex: 0,
    startOffset: 4,
    endOffset: 16,
    quote: "sözleşmesi",
    query: null,
    anchorId,
    quoteIntact: false,
    quads: [],
  };
}

const TARGET_A1 = anchorTarget("a1");
const TARGET_A2 = anchorTarget("a2");
const TARGET_B = {
  ...anchorTarget("", "v2", DOC_B.displayName),
  pageId: "v2-p1",
  segmentId: "seg-v2-p1",
  startOffset: null,
  endOffset: null,
  quote: null,
  anchorId: null,
  quoteIntact: null,
};

function location(overrides: Partial<AnchorLocationRow> & { anchorId: string }): AnchorLocationRow {
  return {
    caseId: "c1",
    documentVersionId: "v1",
    documentId: "d1",
    documentName: DOC_A.displayName,
    parseRunId: "run-v1",
    pageId: "v1-p1",
    pageIndex: 0,
    segmentId: "seg-v1-p1",
    startOffset: 4,
    endOffset: 16,
    exactQuote: "sözleşmesi",
    anchorStatus: "active",
    quoteIntact: true,
    quads: [],
    coordinateSpace: "unrotated_cropbox_points",
    pageRotation: 0,
    isCurrentVersion: true,
    currentVersionId: "v1",
    versionNo: 1,
    currentVersionNo: 1,
    isCurrentRun: true,
    ...overrides,
  };
}

/** Eski sürüme bağlı, alıntısı segmentte bulunamayan bağ (a1). */
const BROKEN_A1 = location({
  anchorId: "a1",
  quoteIntact: false,
  isCurrentVersion: false,
  currentVersionId: "v2",
  versionNo: 1,
  currentVersionNo: 2,
});

/** Aynı belgenin güncel sürümüne bağlı, sağlam bağ (a1). */
const SOUND_A1 = location({ anchorId: "a1" });

/** Yeniden bağlamadan sonraki konum: alıntı yeni sürümün ilk sayfasında. */
const REPAIRED_A1 = location({
  anchorId: "a1",
  documentVersionId: "v2",
  parseRunId: "run-v2",
  pageId: "v2-p1",
  segmentId: "seg-v2-p1",
  startOffset: 10,
  endOffset: 22,
  exactQuote: "İhtarname 3 Mart",
  isCurrentVersion: true,
  currentVersionId: "v2",
  versionNo: 2,
  currentVersionNo: 2,
});

/** İkinci bağ (a2): başka belgede, kopmuş ama güncel sürüme bağlı. */
const BROKEN_A2 = location({
  anchorId: "a2",
  documentVersionId: "v1",
  pageId: "v1-p2",
  pageIndex: 1,
  segmentId: "seg-v1-p2",
  startOffset: 0,
  endOffset: 12,
  exactQuote: "İkinci sayfa",
  quoteIntact: false,
  isCurrentVersion: true,
  currentVersionId: "v1",
  versionNo: 1,
  currentVersionNo: 1,
});

const SUGGESTION_A1: RebindSuggestion = {
  anchorId: "a1",
  quote: "sözleşmesi",
  currentVersionId: "v2",
  versionNo: 1,
  currentVersionNo: 2,
  isCurrentRun: true,
  status: "exact",
  candidates: [
    { segmentId: "seg-v2-p1", pageId: "v2-p1", pageIndex: 0, startOffset: 10, endOffset: 22 },
  ],
};

/** Çözümü testin elinde olan vaat: uçuştaki isteği askıda tutar. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function mount(left: ReaderTarget | null = TARGET_A1, onAnchorRebound = vi.fn()) {
  const view = render(
    <DualReader
      caseId="c1"
      documents={[DOC_B, DOC_A]}
      left={left}
      right={TARGET_B}
      onClose={() => {}}
      onAnchorRebound={onAnchorRebound}
    />,
  );
  const rerenderLeft = (next: ReaderTarget | null) =>
    view.rerender(
      <DualReader
        caseId="c1"
        documents={[DOC_B, DOC_A]}
        left={next}
        right={TARGET_B}
        onClose={() => {}}
        onAnchorRebound={onAnchorRebound}
      />,
    );
  return { view, rerenderLeft, onAnchorRebound };
}

function paneLeft(): HTMLElement {
  return screen.getByRole("region", { name: t("readerUi.leftDocument") });
}

function picker(scope: HTMLElement): HTMLSelectElement {
  return within(scope).getByRole("combobox", {
    name: t("readerUi.documentPickerAria", { label: t("readerUi.leftDocument") }),
  }) as HTMLSelectElement;
}

function rebindButton(scope: HTMLElement): HTMLButtonElement {
  return within(scope).getByRole("button", { name: t("readerUi.rebindSearch") }) as HTMLButtonElement;
}

const EXACT_STATUS = t("readerLib.exact", { scope: t("readerLib.scopeVersion") });

/** Aday düğmesi öneri kartına daraltılır: sayfa sekmeleri de aynı adı taşır. */
function candidateButton(card: HTMLElement, page: number): HTMLButtonElement {
  return within(card).getByRole("button", {
    name: t("readerUi.physicalPage", { page }),
  }) as HTMLButtonElement;
}

/** Öneri kartı: durum satırını taşıyan kart. Çizilmediyse `null`. */
function suggestionCard(scope: HTMLElement, status = EXACT_STATUS): HTMLElement | null {
  const line = Array.from(scope.querySelectorAll("p.hint")).find((node) =>
    node.textContent?.includes(status),
  );
  return (line?.closest("div.card") as HTMLElement | null) ?? null;
}

async function findSuggestionCard(scope: HTMLElement, status = EXACT_STATUS): Promise<HTMLElement> {
  return waitFor(() => {
    const card = suggestionCard(scope, status);
    if (!card) throw new Error("öneri kartı çizilmedi");
    return card;
  });
}

// jsdom `scrollIntoView` sağlamaz; hedef aralık çizilince okuyucu çağırıyor.
beforeAll(() => {
  Element.prototype.scrollIntoView = vi.fn();
});

afterAll(() => {
  Reflect.deleteProperty(Element.prototype, "scrollIntoView");
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("okuyucu kaynak yeniden bağlama akışı", () => {
  it("kopmuş bağın künyesi çekirdekten okunur: uyarı, sürüm rozeti, arama düğmesi", async () => {
    mocks.documentPages.mockResolvedValue(PAIN_A);
    mocks.marksForVersion.mockResolvedValue([]);
    mocks.anchorLocate.mockResolvedValue(BROKEN_A1);

    mount(TARGET_A1);
    const left = await waitFor(() => paneLeft());

    await within(left).findByText(t("readerUi.anchorBroken"));
    expect(mocks.anchorLocate).toHaveBeenCalledWith("a1");
    within(left).getByText(t("readerLib.versionBadge", { from: "v1", to: "v2" }));
    expect(rebindButton(left).disabled).toBe(false);
  });

  it("sağlam ve güncel bağda uyarı ve yeniden bağlama düğmesi çizilmez", async () => {
    mocks.documentPages.mockResolvedValue(PAIN_A);
    mocks.marksForVersion.mockResolvedValue([]);
    mocks.anchorLocate.mockResolvedValue(SOUND_A1);

    mount(TARGET_A1);
    const left = await waitFor(() => paneLeft());
    await waitFor(() => expect(mocks.anchorLocate).toHaveBeenCalledWith("a1"));
    await act(async () => {});

    expect(within(left).queryByText(t("readerUi.anchorBroken"))).toBeNull();
    expect(within(left).queryByRole("button", { name: t("readerUi.rebindSearch") })).toBeNull();
  });

  it("öneri kendiliğinden uygulanmaz: durum, alıntı ve aday sayfası çekirdekten gelir", async () => {
    mocks.documentPages.mockResolvedValue(PAIN_A);
    mocks.marksForVersion.mockResolvedValue([]);
    mocks.anchorLocate.mockResolvedValue(BROKEN_A1);
    mocks.anchorRebindSuggestion.mockResolvedValue(SUGGESTION_A1);

    mount(TARGET_A1);
    const left = await waitFor(() => paneLeft());
    await within(left).findByText(t("readerUi.anchorBroken"));

    fireEvent.click(rebindButton(left));
    expect(mocks.anchorRebindSuggestion).toHaveBeenCalledWith("a1");
    expect(mocks.anchorRebind).not.toHaveBeenCalled();

    const card = await findSuggestionCard(paneLeft());
    expect(card.textContent).toContain(SUGGESTION_A1.quote);
    expect(candidateButton(card, 1).disabled).toBe(false);

    fireEvent.click(within(card).getByRole("button", { name: t("readerUi.close") }));
    await waitFor(() => expect(suggestionCard(paneLeft())).toBeNull());
  });

  it("seçilen aday tam sözleşmeyle yazılır; güncel konum okunur ve panel yeni sürüme geçer", async () => {
    mocks.documentPages.mockImplementation(async (versionId: string) =>
      versionId === "v2" ? PAIN_B : PAIN_A,
    );
    mocks.marksForVersion.mockResolvedValue([]);
    mocks.anchorLocate.mockResolvedValueOnce(BROKEN_A1).mockResolvedValue(REPAIRED_A1);
    mocks.anchorRebindSuggestion.mockResolvedValue(SUGGESTION_A1);
    mocks.anchorRebind.mockResolvedValue(undefined);

    const { onAnchorRebound } = mount(TARGET_A1);
    const left = await waitFor(() => paneLeft());
    await within(left).findByText(t("readerUi.anchorBroken"));

    fireEvent.click(rebindButton(left));
    fireEvent.click(candidateButton(await findSuggestionCard(paneLeft()), 1));

    await waitFor(() =>
      expect(mocks.anchorRebind).toHaveBeenCalledWith("a1", "seg-v2-p1", 10),
    );
    await waitFor(() => expect(mocks.anchorLocate).toHaveBeenCalledTimes(2));
    expect(mocks.anchorLocate).toHaveBeenLastCalledWith("a1");

    const after = paneLeft();
    expect(await within(after).findByText(t("readerUi.rebindNotice"))).toBeTruthy();
    expect(picker(after).value).toBe("v2");
    expect(onAnchorRebound).toHaveBeenCalledWith(REPAIRED_A1);
    await waitFor(() => expect(suggestionCard(paneLeft())).toBeNull());
    // Kopma uyarısı kalkar: çekirdek artık "alıntı sağlam" diyor.
    expect(within(paneLeft()).queryByText(t("readerUi.anchorBroken"))).toBeNull();
  });

  it("ÖLÇÜLEN KUSUR A: uçuştaki öneri başka bağın altında çizilmez", async () => {
    const pending = deferred<RebindSuggestion>();
    mocks.documentPages.mockResolvedValue(PAIN_A);
    mocks.marksForVersion.mockResolvedValue([]);
    mocks.anchorLocate.mockImplementation(async (anchorId: string) =>
      anchorId === "a2" ? BROKEN_A2 : BROKEN_A1,
    );
    mocks.anchorRebindSuggestion.mockReturnValue(pending.promise);

    const { rerenderLeft } = mount(TARGET_A1);
    const left = await waitFor(() => paneLeft());
    await within(left).findByText(t("readerUi.anchorBroken"));

    fireEvent.click(rebindButton(left));
    expect(mocks.anchorRebindSuggestion).toHaveBeenCalledWith("a1");

    // Kullanıcı öneri uçuştayken başka bağa geçer.
    rerenderLeft(TARGET_A2);
    await waitFor(() => expect(mocks.anchorLocate).toHaveBeenCalledWith("a2"));
    await act(async () => {
      pending.resolve(SUGGESTION_A1);
      await pending.promise;
    });

    const after = paneLeft();
    expect(suggestionCard(after)).toBeNull();
    expect(within(after).queryByText(t("readerUi.rebindNotice"))).toBeNull();
  });

  it("ÖLÇÜLEN KUSUR B: uçuştaki yeniden bağlama yeni bağın künyesini ezmez", async () => {
    const relocate = deferred<AnchorLocationRow | null>();
    let a1Calls = 0;
    mocks.documentPages.mockResolvedValue(PAIN_A);
    mocks.marksForVersion.mockResolvedValue([]);
    mocks.anchorLocate.mockImplementation(async (anchorId: string) => {
      if (anchorId === "a2") return BROKEN_A2;
      a1Calls += 1;
      return a1Calls === 1 ? BROKEN_A1 : relocate.promise;
    });
    mocks.anchorRebindSuggestion.mockResolvedValue(SUGGESTION_A1);
    mocks.anchorRebind.mockResolvedValue(undefined);

    const { rerenderLeft } = mount(TARGET_A1);
    const left = await waitFor(() => paneLeft());
    await within(left).findByText(t("readerUi.anchorBroken"));

    fireEvent.click(rebindButton(left));
    fireEvent.click(candidateButton(await findSuggestionCard(paneLeft()), 1));
    await waitFor(() => expect(mocks.anchorRebind).toHaveBeenCalledTimes(1));

    // Yazma tamamlandı, güncel konum isteği askıda; kullanıcı başka bağa geçer.
    rerenderLeft(TARGET_A2);
    await waitFor(() => expect(mocks.anchorLocate).toHaveBeenCalledWith("a2"));

    await act(async () => {
      relocate.resolve(REPAIRED_A1);
      await relocate.promise;
    });

    const after = paneLeft();
    // Yeni bağın paneli kendi sürümünde kalır; eski bağın konumu yazılmaz.
    expect(picker(after).value).toBe("v1");
    expect(within(after).queryByText(t("readerUi.rebindNotice"))).toBeNull();
    expect(within(after).queryByText(t("readerLib.versionBadge", { from: "v1", to: "v2" }))).toBeNull();
  });

  it("ÖLÇÜLEN KUSUR C: düşen öneri aramasından sonra başarılı arama kırmızıyı siler", async () => {
    mocks.documentPages.mockResolvedValue(PAIN_A);
    mocks.marksForVersion.mockResolvedValue([]);
    mocks.anchorLocate.mockResolvedValue(BROKEN_A1);
    mocks.anchorRebindSuggestion
      .mockRejectedValueOnce({ code: "E_DB", message: "Öneri okunamadı." })
      .mockResolvedValue(SUGGESTION_A1);

    mount(TARGET_A1);
    const left = await waitFor(() => paneLeft());
    await within(left).findByText(t("readerUi.anchorBroken"));

    fireEvent.click(rebindButton(left));
    await within(left).findByText("E_DB");

    fireEvent.click(rebindButton(left));
    await findSuggestionCard(paneLeft());
    expect(within(paneLeft()).queryByRole("alert")).toBeNull();
  });

  it("ÖLÇÜLEN KUSUR D: düşen bağlamadan sonra başarılı bağlama kırmızıyı siler", async () => {
    mocks.documentPages.mockResolvedValue(PAIN_A);
    mocks.marksForVersion.mockResolvedValue([]);
    mocks.anchorLocate.mockResolvedValue(BROKEN_A1);
    mocks.anchorRebindSuggestion.mockResolvedValue(SUGGESTION_A1);
    mocks.anchorRebind
      .mockRejectedValueOnce({ code: "E_ANCHOR", message: "Bağ yazılamadı." })
      .mockResolvedValue(undefined);

    mount(TARGET_A1);
    const left = await waitFor(() => paneLeft());
    await within(left).findByText(t("readerUi.anchorBroken"));

    fireEvent.click(rebindButton(left));
    fireEvent.click(candidateButton(await findSuggestionCard(paneLeft()), 1));
    await within(left).findByText("E_ANCHOR");

    // Öneri listesi düşen denemeden sonra kalır; aynı aday yeniden denenir.
    fireEvent.click(candidateButton(await findSuggestionCard(paneLeft()), 1));
    await waitFor(() => expect(mocks.anchorRebind).toHaveBeenCalledTimes(2));
    await within(paneLeft()).findByText(t("readerUi.rebindNotice"));
    expect(within(paneLeft()).queryByRole("alert")).toBeNull();
  });

  it("çift tık tek yazma üretir", async () => {
    mocks.documentPages.mockResolvedValue(PAIN_A);
    mocks.marksForVersion.mockResolvedValue([]);
    mocks.anchorLocate.mockResolvedValue(BROKEN_A1);
    mocks.anchorRebindSuggestion.mockResolvedValue(SUGGESTION_A1);
    let release!: () => void;
    mocks.anchorRebind.mockImplementation(
      () => new Promise<void>((resolve) => {
        release = () => resolve();
      }),
    );

    mount(TARGET_A1);
    const left = await waitFor(() => paneLeft());
    await within(left).findByText(t("readerUi.anchorBroken"));

    fireEvent.click(rebindButton(left));
    const candidate = candidateButton(await findSuggestionCard(paneLeft()), 1);
    fireEvent.click(candidate);
    fireEvent.click(candidate);

    await act(async () => {
      release();
    });
    await waitFor(() => expect(mocks.anchorRebind).toHaveBeenCalledTimes(1));
  });

  it("bağ bulunamazsa künye uydurulmaz ve sessiz kalınır", async () => {
    mocks.documentPages.mockResolvedValue(PAIN_A);
    mocks.marksForVersion.mockResolvedValue([]);
    mocks.anchorLocate.mockResolvedValue(null);

    mount(TARGET_A1);
    const left = await waitFor(() => paneLeft());
    await waitFor(() => expect(mocks.anchorLocate).toHaveBeenCalledWith("a1"));
    await act(async () => {});

    expect(within(left).queryByText(t("readerUi.anchorBroken"))).toBeNull();
    expect(within(left).queryByRole("button", { name: t("readerUi.rebindSearch") })).toBeNull();
    expect(within(left).queryByRole("alert")).toBeNull();
  });
});
