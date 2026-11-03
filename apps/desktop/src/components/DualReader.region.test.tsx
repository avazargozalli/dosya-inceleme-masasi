// @vitest-environment jsdom
//
// Okuyucu (DualReader) **bölge işareti** akışı davranış testi.
//
// `DualReader.selection.test.tsx` metinden seçimle giden üç komutu ölçüyor;
// sayfa görüntüsü üzerinde **sürüklenen bölge** (`markCreateRegion`) bugüne dek
// hiç ölçülmemişti. Burada sabitlenenler:
// 1. Bölge seçimi yokken işaret düğmesi kapalı; sürükleme onu açar, "temizle"
//    seçimi kaldırır; 2 puntodan küçük sürükleme bölge sayılmaz.
// 2. Sürükleme görünen uzaydan **sayfa uzayına** çevrilip tam sözleşmeyle
//    gönderilir; döndürülmüş görünümde de koordinat döndürme öncesidir.
// 3. **Ölçülen kusur (A):** sayfa değişince seçili bölge taşınıyordu → yeni
//    sayfaya **önceki sayfanın koordinatlarıyla** işaret yazılabiliyordu.
// 4. **Ölçülen kusur (B):** sürüm değişince seçili bölge taşınıyordu → başka
//    belgenin sayfasına eski koordinatlar yazılabiliyordu.
// 5. **Ölçülen kusur (C):** uçuştaki `markCreateRegion` yanıtı sürüm kapısı
//    taşımıyordu → eski sürümün işareti yeni sürümün listesine düşüyordu.
// 6. **Ölçülen kusur (D):** işlem başında hata silinmiyordu → düşen denemeden
//    sonra başarılı işarette kırmızı hata ekranda kalıyordu.
// 7. **Ölçülen kusur (E):** uçuştaki yanıt, o sırada çizilen **yeni** bölgeyi
//    de siliyordu (kullanıcının seçimi sessizce kayboluyordu).
//
// jsdom sınırı: `PointerEvent` ve pointer capture yok; olaylar `clientX/clientY`
// taşıyan `MouseEvent` ile, yakalama metotları no-op olarak kurulur. Gerçek
// işaretçi, CSS ve görsel QA bu testin kapsamı dışındadır.

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { t } from "../lib/i18n";
import type { DocumentPages, DocumentRow, MarkRow, PagePreviewView, PageRow } from "../lib/api";

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

function preview(pageIndex: number, versionId = "v1"): PagePreviewView {
  return {
    versionId,
    pageIndex,
    dpi: 96,
    pngBase64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
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

function mount(options: { pages?: DocumentPages } = {}) {
  mocks.documentPages.mockImplementation(async (versionId: string) => {
    if (options.pages && versionId === "v1") return options.pages;
    return versionId === "v2" ? PAIN_B : PAIN_A;
  });
  mocks.marksForVersion.mockResolvedValue([]);
  mocks.anchorLocate.mockResolvedValue(null);
  mocks.documentPagePreview.mockImplementation(async (versionId: string, pageIndex: number) =>
    preview(pageIndex, versionId),
  );
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

/** jsdom görünüm kutusunu 0 ölçer; sürükleme oranları için gerçek kutu kurulur. */
function sizeBox(box: HTMLElement, width: number, height: number) {
  Object.defineProperty(box, "getBoundingClientRect", {
    configurable: true,
    value: () => ({
      x: 0,
      y: 0,
      left: 0,
      top: 0,
      right: width,
      bottom: height,
      width,
      height,
      toJSON: () => ({}),
    }),
  });
}

/** jsdom `PointerEvent` üretmez; `clientX/clientY` taşıyan olay yeterlidir. */
function drag(box: HTMLElement, from: [number, number], to: [number, number]) {
  fireEvent(box, new MouseEvent("pointerdown", { bubbles: true, clientX: from[0], clientY: from[1] }));
  fireEvent(box, new MouseEvent("pointermove", { bubbles: true, clientX: to[0], clientY: to[1] }));
  fireEvent(box, new MouseEvent("pointerup", { bubbles: true, clientX: to[0], clientY: to[1] }));
}

function markRegionButton(scope: HTMLElement): HTMLButtonElement {
  return within(scope).getByRole("button", { name: t("readerUi.markRegion") }) as HTMLButtonElement;
}

function clearRegionButton(scope: HTMLElement): HTMLButtonElement {
  return within(scope).getByRole("button", { name: t("readerUi.clearRegion") }) as HTMLButtonElement;
}

beforeAll(() => {
  // jsdom pointer capture sağlamaz; sürükleme akışı için no-op yeterlidir.
  Element.prototype.setPointerCapture = vi.fn();
  Element.prototype.releasePointerCapture = vi.fn();
  Element.prototype.hasPointerCapture = vi.fn(() => false);
});

afterAll(() => {
  Reflect.deleteProperty(Element.prototype, "setPointerCapture");
  Reflect.deleteProperty(Element.prototype, "releasePointerCapture");
  Reflect.deleteProperty(Element.prototype, "hasPointerCapture");
});

describe("okuyucu bölge işareti akışı", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("bölge seçimi yokken işaret kapalıdır; sürükleme açar, temizleme kaldırır", async () => {
    mount();
    const left = await leftPaneWithText();
    const box = await openPreview(left);
    sizeBox(box, 400, 600);

    expect(markRegionButton(left).disabled).toBe(true);
    expect(clearRegionButton(left).disabled).toBe(true);

    drag(box, [40, 60], [200, 300]);
    expect(markRegionButton(left).disabled).toBe(false);
    expect(clearRegionButton(left).disabled).toBe(false);

    fireEvent.click(clearRegionButton(left));
    expect(markRegionButton(left).disabled).toBe(true);
    expect(clearRegionButton(left).disabled).toBe(true);
  });

  it("2 puntodan küçük sürükleme bölge sayılmaz", async () => {
    mount();
    const left = await leftPaneWithText();
    const box = await openPreview(left);
    sizeBox(box, 400, 600);

    // 400 px genişlikte 2 px = %0,5; eşik %1.
    drag(box, [100, 100], [102, 130]);
    expect(markRegionButton(left).disabled).toBe(true);
    fireEvent.click(markRegionButton(left));
    expect(mocks.markCreateRegion).not.toHaveBeenCalled();
  });

  it("sürüklenen bölge sayfa koordinatına çevrilip tam sözleşmeyle gönderilir", async () => {
    mount();
    const left = await leftPaneWithText();
    const box = await openPreview(left);
    sizeBox(box, 400, 600);
    mocks.markCreateRegion.mockResolvedValue(regionMark());

    drag(box, [40, 60], [200, 300]);
    fireEvent.click(markRegionButton(left));

    await waitFor(() => expect(mocks.markCreateRegion).toHaveBeenCalledTimes(1));
    // Oranlar görünen 400×600 kutudan; sayfa 600×800 punto. Döndürme yok.
    expect(mocks.markCreateRegion.mock.calls[0]![0]).toEqual({
      caseId: "c1",
      documentVersionId: "v1",
      pageId: "v1-p1",
      rects: [[60, 80, 300, 400]],
      label: "",
    });
    expect(await within(left).findByText(t("readerUi.regionMarkSaved"))).toBeTruthy();
    expect(within(left).getByText(t("readerUi.marksSummary", { count: 1 }))).toBeTruthy();
    expect(
      within(left).getByRole("button", { name: t("readerUi.regionMarkLabel", { count: 1 }) }),
    ).toBeTruthy();
    // Başarılı kayıttan sonra seçim bırakılır.
    expect(markRegionButton(left).disabled).toBe(true);
  });

  it("döndürülmüş görünümde bölge döndürme öncesi koordinata yazılır", async () => {
    mount();
    const left = await leftPaneWithText();
    const box = await openPreview(left);
    // Görünüm 90° döndürülünce sayfa 800×600 olarak çizilir.
    sizeBox(box, 800, 600);
    fireEvent.click(
      within(left).getByRole("button", {
        name: t("readerUi.rotateRightAria", { label: t("readerUi.leftDocument") }),
      }),
    );
    mocks.markCreateRegion.mockResolvedValue(regionMark({ rects: [[60, 400, 300, 720]] }));

    drag(box, [80, 60], [400, 300]);
    fireEvent.click(markRegionButton(left));

    await waitFor(() => expect(mocks.markCreateRegion).toHaveBeenCalledTimes(1));
    expect(mocks.markCreateRegion.mock.calls[0]![0]).toMatchObject({
      pageId: "v1-p1",
      rects: [[60, 400, 300, 720]],
    });
  });

  it("sayfa değişince seçili bölge yeni sayfaya taşınmaz", async () => {
    mount();
    const left = await leftPaneWithText();
    const box = await openPreview(left);
    sizeBox(box, 400, 600);

    drag(box, [40, 60], [200, 300]);
    expect(markRegionButton(left).disabled).toBe(false);

    // Kullanıcı sayfa 2'ye geçiyor; seçim sayfa 1'e ait.
    fireEvent.click(within(left).getByRole("button", { name: "Fiziksel sayfa 2" }));
    expect(await within(left).findByText(TEXT_A2)).toBeTruthy();
    await within(left).findByAltText(
      t("readerUi.previewImageAlt", { label: t("readerUi.leftDocument"), page: 2 }),
    );

    expect(markRegionButton(left).disabled).toBe(true);
    fireEvent.click(markRegionButton(left));
    expect(mocks.markCreateRegion).not.toHaveBeenCalled();
  });

  it("sürüm değişince seçili bölge yeni belgeye taşınmaz", async () => {
    mount();
    const left = await leftPaneWithText();
    const box = await openPreview(left);
    sizeBox(box, 400, 600);

    drag(box, [40, 60], [200, 300]);
    fireEvent.change(within(left).getByRole("combobox"), { target: { value: "v2" } });
    expect(await within(left).findByText(TEXT_B1)).toBeTruthy();
    await within(left).findByAltText(
      t("readerUi.previewImageAlt", { label: t("readerUi.leftDocument"), page: 1 }),
    );

    expect(markRegionButton(left).disabled).toBe(true);
    fireEvent.click(markRegionButton(left));
    expect(mocks.markCreateRegion).not.toHaveBeenCalled();
  });

  it("uçuştaki bölge yanıtı sürüm değişince yeni sürüme yazılmaz", async () => {
    mount();
    const left = await leftPaneWithText();
    const box = await openPreview(left);
    sizeBox(box, 400, 600);
    const gate = deferred<MarkRow>();
    mocks.markCreateRegion.mockReturnValue(gate.promise);

    drag(box, [40, 60], [200, 300]);
    fireEvent.click(markRegionButton(left));
    expect(mocks.markCreateRegion).toHaveBeenCalledTimes(1);

    fireEvent.change(within(left).getByRole("combobox"), { target: { value: "v2" } });
    expect(await within(left).findByText(TEXT_B1)).toBeTruthy();

    await act(async () => {
      gate.resolve(regionMark());
      await gate.promise;
    });

    // Eski sürümün işareti ve başarı bildirimi yeni sürümün ekranına düşmemeli.
    expect(within(left).queryByText(t("readerUi.marksSummary", { count: 1 }))).toBeNull();
    expect(within(left).queryByText(t("readerUi.regionMarkSaved"))).toBeNull();
  });

  it("sürüm değiştikten sonra düşen istek yeni sürüme hata yazmaz", async () => {
    mount();
    const left = await leftPaneWithText();
    const box = await openPreview(left);
    sizeBox(box, 400, 600);
    const gate = deferred<MarkRow>();
    mocks.markCreateRegion.mockReturnValue(gate.promise);

    drag(box, [40, 60], [200, 300]);
    fireEvent.click(markRegionButton(left));

    fireEvent.change(within(left).getByRole("combobox"), { target: { value: "v2" } });
    expect(await within(left).findByText(TEXT_B1)).toBeTruthy();

    await act(async () => {
      gate.reject({ code: "E_MARK", message: "İşaret yazılamadı.", retryable: true });
      try {
        await gate.promise;
      } catch {
        // Hata bileşen tarafından yakalanır; testte yalnız akış beklenir.
      }
    });

    expect(within(left).queryByRole("alert")).toBeNull();
    expect(within(left).queryByText(t("readerUi.regionMarkSaved"))).toBeNull();
  });

  it("düşen bölge işareti hatayı yazar, sonraki başarı onu siler", async () => {
    mount();
    const left = await leftPaneWithText();
    const box = await openPreview(left);
    sizeBox(box, 400, 600);
    mocks.markCreateRegion.mockRejectedValueOnce({
      code: "E_MARK",
      message: "İşaret yazılamadı.",
      retryable: true,
    });

    drag(box, [40, 60], [200, 300]);
    fireEvent.click(markRegionButton(left));
    const alert = await within(left).findByRole("alert");
    expect(alert.textContent).toContain("E_MARK");
    // Hata seçimi düşürmez: yeniden denenebilir.
    expect(markRegionButton(left).disabled).toBe(false);

    mocks.markCreateRegion.mockResolvedValueOnce(regionMark());
    fireEvent.click(markRegionButton(left));

    expect(await within(left).findByText(t("readerUi.regionMarkSaved"))).toBeTruthy();
    expect(within(left).queryByRole("alert")).toBeNull();
  });

  it("uçuştaki yanıt, o sırada çizilen yeni bölgeyi silmez", async () => {
    mount();
    const left = await leftPaneWithText();
    const box = await openPreview(left);
    sizeBox(box, 400, 600);
    const gate = deferred<MarkRow>();
    mocks.markCreateRegion.mockReturnValueOnce(gate.promise);

    drag(box, [40, 60], [200, 300]);
    fireEvent.click(markRegionButton(left));

    // İstek askıdayken kullanıcı yeni bir bölge çiziyor.
    drag(box, [100, 120], [260, 360]);

    await act(async () => {
      gate.resolve(regionMark());
      await gate.promise;
    });

    // Yeni seçim korunmalı: düğme açık kalır ve **yeni** koordinatlar gider.
    expect(markRegionButton(left).disabled).toBe(false);
    mocks.markCreateRegion.mockResolvedValue(regionMark({ rects: [[150, 160, 390, 480]] }));
    fireEvent.click(markRegionButton(left));
    await waitFor(() => expect(mocks.markCreateRegion).toHaveBeenCalledTimes(2));
    expect(mocks.markCreateRegion.mock.calls[1]![0]).toMatchObject({
      rects: [[150, 160, 390, 480]],
    });
  });

  it("sayfa ölçüsü olmayan belgede bölge yazılmaz", async () => {
    mount({ pages: { ...PAIN_A, pages: [page({ id: "v1-p1", pageIndex: 0, text: TEXT_A1, width: null, height: null })] } });
    const left = await leftPaneWithText();
    const box = await openPreview(left);
    sizeBox(box, 400, 600);

    drag(box, [40, 60], [200, 300]);
    fireEvent.click(markRegionButton(left));
    expect(mocks.markCreateRegion).not.toHaveBeenCalled();
  });
});
