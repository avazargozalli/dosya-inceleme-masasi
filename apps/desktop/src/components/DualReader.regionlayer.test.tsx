// @vitest-environment jsdom
//
// Okuyucu (DualReader) **bölge katmanı** ve **işaret silme** davranış testi.
//
// `DualReader.region.test.tsx` sürükleyip bölge **yazmayı** ölçüyor; yazılmış
// bölgelerin sayfa görüntüsü üzerinde **çizilmesi** (`previewRegions`), sayfa
// haritasına (`PageMap`) verilmesi ve işaretin **silinmesi** bugüne dek hiç
// ölçülmemişti. Burada sabitlenenler:
// 1. Kayıtlı bölge kutuları görünen uzaya çevrilip yüzde olarak çizilir
//    (belge döndürmesi 90° iken de); başka sayfanın bölgesi ve metin işareti
//    çizilmez.
// 2. Sayfa haritasına **yalnız o sayfanın** bölge dikdörtgenleri gider.
// 3. Sayfa değişince / önizleme kapanınca kutular kalkar, açılınca geri gelir.
// 4. **Ölçülen kusur (A):** düşen silme, önceki "İşaret silindi" bildirimini
//    ekranda bırakıyordu (yeşil + kırmızı aynı anda).
// 5. **Ölçülen kusur (B):** uçuştaki silme yanıtı sürüm kapısı taşımıyordu →
//    başka sürüme geçildikten sonra dönen yanıt yeni sürüme bildirim yazıyordu.
// 6. **Ölçülen kusur (C):** düşen silme hatası da sürüm kapısından geçmiyordu →
//    yeni sürüme `E_MARK` yazılıyordu.
// 7. **Ölçülen kusur (D):** silme düğmesinin uçuş kapısı yoktu → iki tık iki
//    `markDelete` gönderiyor, ikincisi düşünce başarılı silmenin üstüne kırmızı
//    hata yazılıyordu.
//
// jsdom sınırı: CSS/yerleşim, gerçek Tauri penceresi ve görsel QA yok; kutular
// yüzde **stilinden** okunur, tarayıcının çizimi ölçülmez.

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { t } from "../lib/i18n";
import type {
  DocumentPages,
  DocumentRow,
  MarkRow,
  PageBlock,
  PagePreviewView,
  PageRow,
} from "../lib/api";

const mocks = vi.hoisted(() => ({
  documentPages: vi.fn(),
  marksForVersion: vi.fn(),
  anchorLocate: vi.fn(),
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

const BLOCKS: PageBlock[] = [{ start: 0, end: 12, bbox: [40, 50, 320, 200] }];

function page(
  overrides: Partial<PageRow> & { id: string; pageIndex: number; text: string },
): PageRow {
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

function pagesFor(versionId: string, texts: string[], extra: Partial<PageRow> = {}): DocumentPages {
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
    pages: texts.map((text, index) =>
      page({ id: `${versionId}-p${index + 1}`, pageIndex: index, text, ...extra }),
    ),
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

function target(versionId: string, documentName: string) {
  return {
    versionId,
    documentName,
    pageId: null,
    segmentId: null,
    pageIndex: null,
    startOffset: null,
    endOffset: null,
    quote: null,
    query: null,
    anchorId: null,
    quoteIntact: null,
    quads: [] as [number, number, number, number][],
  };
}

const TARGET_A = target("v1", DOC_A.displayName);
const TARGET_B = target("v2", DOC_B.displayName);

function regionMark(overrides: Partial<MarkRow> = {}): MarkRow {
  return {
    id: "mr1",
    caseId: "c1",
    documentVersionId: "v1",
    parseRunId: "run-v1",
    anchorId: null,
    pageId: "v1-p1",
    markKind: "region",
    startOffset: null,
    endOffset: null,
    quotedText: "",
    rects: [[60, 80, 300, 400]],
    label: "",
    createdAt: ISO,
    ...overrides,
  };
}

function textMark(overrides: Partial<MarkRow> = {}): MarkRow {
  return regionMark({
    id: "mt1",
    markKind: "text",
    startOffset: 0,
    endOffset: 18,
    quotedText: TEXT_A1.slice(0, 18),
    rects: [],
    ...overrides,
  });
}

function preview(pageIndex: number, versionId = "v1"): PagePreviewView {
  return {
    versionId,
    pageIndex,
    dpi: 96,
    pngBase64:
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
    byteSize: 1234,
    cached: true,
  };
}

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

type MountOptions = {
  pages?: DocumentPages;
  marks?: (versionId: string) => MarkRow[];
};

function mount(options: MountOptions = {}) {
  mocks.documentPages.mockImplementation(async (versionId: string) => {
    if (options.pages && versionId === "v1") return options.pages;
    return versionId === "v2" ? PAIN_B : PAIN_A;
  });
  mocks.marksForVersion.mockImplementation(async (versionId: string) =>
    options.marks ? options.marks(versionId) : [],
  );
  mocks.anchorLocate.mockResolvedValue(null);
  mocks.documentPagePreview.mockImplementation(async (versionId: string, pageIndex: number) =>
    preview(pageIndex, versionId),
  );
  mocks.markDelete.mockResolvedValue(undefined);
  render(
    <DualReader
      caseId="c1"
      documents={[DOC_B, DOC_A]}
      left={TARGET_A}
      right={TARGET_B}
      onClose={() => {}}
    />,
  );
}

function pane(label: string): HTMLElement {
  return screen.getByRole("region", { name: label });
}

async function leftPaneWithText(): Promise<HTMLElement> {
  await screen.findByText(TEXT_A1);
  return pane(t("readerUi.leftDocument"));
}

/** Önizlemeyi açar ve sürükleme kapsayıcısını döndürür. */
async function openPreview(scope: HTMLElement, pageNumber = 1): Promise<HTMLElement> {
  if (!within(scope).queryByRole("button", { name: t("readerUi.previewOn") })) {
    fireEvent.click(within(scope).getByRole("button", { name: t("readerUi.previewOff") }));
  }
  await within(scope).findByAltText(
    t("readerUi.previewImageAlt", { label: t("readerUi.leftDocument"), page: pageNumber }),
  );
  return within(scope).getByRole("group", {
    name: t("readerUi.previewRegionAria", { label: t("readerUi.leftDocument") }),
  });
}

/** Önizleme kutusu üzerine çizilmiş kalıcı bölge dikdörtgenleri. */
function regionBoxes(group: HTMLElement): HTMLElement[] {
  return Array.from(group.querySelectorAll<HTMLElement>('div[aria-hidden="true"]'));
}

function boxStyle(box: HTMLElement): string {
  return [box.style.left, box.style.top, box.style.width, box.style.height].join("/");
}

/** Sayfa haritasındaki bölge (kesikli) ve blok dikdörtgenleri. */
function mapRects(scope: HTMLElement): { regions: Element[]; blocks: Element[] } {
  const svgs = Array.from(
    within(scope).queryAllByRole("img").filter((node) => node.tagName.toLowerCase() === "svg"),
  );
  const svg = svgs[svgs.length - 1]!;
  return {
    regions: Array.from(svg.querySelectorAll('rect[stroke-dasharray="3 2"]')),
    blocks: Array.from(svg.querySelectorAll('rect[stroke="var(--line-strong)"]')),
  };
}

function deleteButtons(scope: HTMLElement): HTMLButtonElement[] {
  return within(scope)
    .queryAllByRole("button", { name: t("readerUi.deleteMarkAria") })
    .map((node) => node as HTMLButtonElement);
}

beforeAll(() => {
  // jsdom pointer capture sağlamaz; bölge akışı için no-op yeterlidir.
  Element.prototype.setPointerCapture = vi.fn();
  Element.prototype.releasePointerCapture = vi.fn();
  Element.prototype.hasPointerCapture = vi.fn(() => false);
});

afterAll(() => {
  Reflect.deleteProperty(Element.prototype, "setPointerCapture");
  Reflect.deleteProperty(Element.prototype, "releasePointerCapture");
  Reflect.deleteProperty(Element.prototype, "hasPointerCapture");
});

describe("okuyucu bölge katmanı", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("kayıtlı bölge kutuları görünen uzaya çevrilip yüzdeyle çizilir", async () => {
    mount({ marks: () => [regionMark()] });
    const left = await leftPaneWithText();
    const group = await openPreview(left);

    const boxes = regionBoxes(group);
    expect(boxes).toHaveLength(1);
    // Sayfa 600×800 punto; kutu [60,80,300,400] → %10/%10/%40/%40.
    expect(boxStyle(boxes[0]!)).toBe("10%/10%/40%/40%");
  });

  it("belge döndürmesi 90° olan sayfada kutu döndürme öncesi uzaydan çevrilir", async () => {
    mount({
      pages: pagesFor("v1", [TEXT_A1, TEXT_A2], { rotation: 90 }),
      marks: () => [regionMark({ rects: [[60, 80, 300, 480]] })],
    });
    const left = await leftPaneWithText();
    const group = await openPreview(left);

    const boxes = regionBoxes(group);
    expect(boxes).toHaveLength(1);
    // 600×800, 90°: [60,80,300,480] → [320,60,720,300]; görünen kutu 800×600.
    expect(boxStyle(boxes[0]!)).toBe("40%/10%/50%/40%");
  });

  it("başka sayfanın bölgesi ve metin işareti kutu olarak çizilmez", async () => {
    mount({
      marks: () => [
        regionMark({ id: "mr2", pageId: "v1-p2", rects: [[10, 20, 100, 200]] }),
        textMark(),
      ],
    });
    // Metin işareti sayfa metnini parçalara böldüğü için ham cümle tek düğüm
    // değildir; paneli işaret listesinden bekleriz.
    const left = pane(t("readerUi.leftDocument"));
    await within(left).findByText(t("readerUi.marksSummary", { count: 2 }));
    const group = await openPreview(left);

    expect(regionBoxes(group)).toHaveLength(0);
    // İşaret listede duruyor (sayfa 2'de): çizilmemesi onu gizlemez.
    expect(
      within(left).getByRole("button", { name: t("readerUi.regionMarkLabel", { count: 1 }) }),
    ).toBeTruthy();
  });

  it("sayfa haritasına yalnız o sayfanın bölge dikdörtgenleri gider", async () => {
    mount({
      pages: pagesFor("v1", [TEXT_A1, TEXT_A2], { blocks: BLOCKS }),
      marks: () => [
        regionMark({ id: "mr1", pageId: "v1-p1" }),
        regionMark({ id: "mr2", pageId: "v1-p2", rects: [[10, 20, 100, 200]] }),
      ],
    });
    const left = await leftPaneWithText();

    const first = mapRects(left);
    expect(first.blocks).toHaveLength(1);
    expect(first.regions).toHaveLength(1);

    fireEvent.click(within(left).getByRole("button", { name: "Fiziksel sayfa 2" }));
    expect(await within(left).findByText(TEXT_A2)).toBeTruthy();
    const second = mapRects(left);
    expect(second.blocks).toHaveLength(1);
    expect(second.regions).toHaveLength(1);
    // Sayfa 2'nin dikdörtgeni gerçekten öteki işaretten gelir.
    expect(second.regions[0]!.getAttribute("x")).toBe("10");
    expect(second.regions[0]!.getAttribute("y")).toBe("20");
  });

  it("sayfa değişince önceki sayfanın kutusu yeni sayfada çizilmez", async () => {
    mount({ marks: () => [regionMark({ pageId: "v1-p1" })] });
    const left = await leftPaneWithText();
    const group = await openPreview(left);
    expect(regionBoxes(group)).toHaveLength(1);

    fireEvent.click(within(left).getByRole("button", { name: "Fiziksel sayfa 2" }));
    expect(await within(left).findByText(TEXT_A2)).toBeTruthy();
    const second = await openPreview(left, 2);
    expect(regionBoxes(second)).toHaveLength(0);
  });

  it("önizleme kapalıyken kutu yoktur, açılınca geri gelir", async () => {
    mount({ marks: () => [regionMark()] });
    const left = await leftPaneWithText();
    await openPreview(left);
    expect(within(left).queryByRole("group", {
      name: t("readerUi.previewRegionAria", { label: t("readerUi.leftDocument") }),
    })).toBeTruthy();

    fireEvent.click(within(left).getByRole("button", { name: t("readerUi.previewOn") }));
    expect(
      within(left).queryByRole("group", {
        name: t("readerUi.previewRegionAria", { label: t("readerUi.leftDocument") }),
      }),
    ).toBeNull();

    const again = await openPreview(left);
    expect(regionBoxes(again)).toHaveLength(1);
  });

  it("silinen bölge kutuyu ve satırı kaldırır", async () => {
    mount({ marks: () => [regionMark()] });
    const left = await leftPaneWithText();
    const group = await openPreview(left);
    expect(regionBoxes(group)).toHaveLength(1);

    fireEvent.click(deleteButtons(left)[0]!);

    await waitFor(() => expect(mocks.markDelete).toHaveBeenCalledWith("mr1"));
    await waitFor(() => expect(regionBoxes(group)).toHaveLength(0));
    expect(within(left).queryByText(t("readerUi.marksSummary", { count: 1 }))).toBeNull();
    expect(await within(left).findByText(t("readerUi.markDeleted"))).toBeTruthy();
  });

  it("düşen silme önceki “silindi” bildirimini bırakmaz", async () => {
    mount({
      marks: () => [
        regionMark({ id: "mr1" }),
        regionMark({ id: "mr2", rects: [[20, 30, 120, 260]] }),
      ],
    });
    const left = await leftPaneWithText();
    await openPreview(left);

    fireEvent.click(deleteButtons(left)[0]!);
    expect(await within(left).findByText(t("readerUi.markDeleted"))).toBeTruthy();

    mocks.markDelete.mockRejectedValueOnce({
      code: "E_MARK",
      message: "İşaret silinemedi.",
      retryable: true,
    });
    fireEvent.click(deleteButtons(left)[0]!);
    const alert = await within(left).findByRole("alert");
    expect(alert.textContent).toContain("E_MARK");
    // Kırmızı hatanın yanında önceki başarının yeşili durmamalı.
    expect(within(left).queryByText(t("readerUi.markDeleted"))).toBeNull();
  });

  it("düşen silmeden sonra başarılı silme kırmızı hatayı siler", async () => {
    mount({ marks: () => [regionMark({ id: "mr1" }), regionMark({ id: "mr2" })] });
    const left = await leftPaneWithText();
    await openPreview(left);

    mocks.markDelete.mockRejectedValueOnce({
      code: "E_MARK",
      message: "İşaret silinemedi.",
      retryable: true,
    });
    fireEvent.click(deleteButtons(left)[0]!);
    expect((await within(left).findByRole("alert")).textContent).toContain("E_MARK");

    fireEvent.click(deleteButtons(left)[0]!);
    await waitFor(() => expect(within(left).queryByRole("alert")).toBeNull());
    expect(await within(left).findByText(t("readerUi.markDeleted"))).toBeTruthy();
  });

  it("uçuştaki silme ikinci istek üretmez", async () => {
    mount({ marks: () => [regionMark()] });
    const left = await leftPaneWithText();
    await openPreview(left);
    const gate = deferred<void>();
    mocks.markDelete.mockReturnValue(gate.promise);

    fireEvent.click(deleteButtons(left)[0]!);
    expect(mocks.markDelete).toHaveBeenCalledTimes(1);
    // Uçuşta düğme kapalı: ikinci tık sessizce yutulur, ikinci yazma olmaz.
    expect(deleteButtons(left)[0]!.disabled).toBe(true);
    fireEvent.click(deleteButtons(left)[0]!);
    expect(mocks.markDelete).toHaveBeenCalledTimes(1);

    await act(async () => {
      gate.resolve();
      await gate.promise;
    });
    expect(await within(left).findByText(t("readerUi.markDeleted"))).toBeTruthy();
  });

  it("sürüm değiştikten sonra dönen silme yanıtı yeni sürüme yazılmaz", async () => {
    mount({ marks: (versionId) => (versionId === "v1" ? [regionMark()] : []) });
    const left = await leftPaneWithText();
    await openPreview(left);
    const gate = deferred<void>();
    mocks.markDelete.mockReturnValue(gate.promise);

    fireEvent.click(deleteButtons(left)[0]!);
    fireEvent.change(within(left).getByRole("combobox"), { target: { value: "v2" } });
    expect(await within(left).findByText(TEXT_B1)).toBeTruthy();

    await act(async () => {
      gate.resolve();
      await gate.promise;
    });

    expect(within(left).queryByText(t("readerUi.markDeleted"))).toBeNull();
    expect(within(left).queryByRole("alert")).toBeNull();
  });

  it("sürüm değiştikten sonra düşen silme yeni sürüme hata yazmaz", async () => {
    mount({ marks: (versionId) => (versionId === "v1" ? [regionMark()] : []) });
    const left = await leftPaneWithText();
    await openPreview(left);
    const gate = deferred<void>();
    mocks.markDelete.mockReturnValue(gate.promise);

    fireEvent.click(deleteButtons(left)[0]!);
    fireEvent.change(within(left).getByRole("combobox"), { target: { value: "v2" } });
    expect(await within(left).findByText(TEXT_B1)).toBeTruthy();

    await act(async () => {
      gate.reject({ code: "E_MARK", message: "İşaret silinemedi.", retryable: true });
      try {
        await gate.promise;
      } catch {
        // Hata bileşen tarafından yakalanır; testte yalnız akış beklenir.
      }
    });

    expect(within(left).queryByRole("alert")).toBeNull();
    expect(within(left).queryByText(t("readerUi.markDeleted"))).toBeNull();
  });
});
