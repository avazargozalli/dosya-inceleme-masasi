// @vitest-environment jsdom
//
// Okuyucu (DualReader) davranış testi. Lib düzeyindeki eşitlik testleri
// (reader.test.ts / inventory.test.ts) metnin **nereden** geldiğini doğrular;
// burada bileşen gerçekten render edilip **panellerin birbirinden bağımsız**
// olduğu, sayfa/sayfa haritası/yakınlaştırmanın gerçekten çalıştığı ölçülür.
//
// Ölçülen davranışlar (hepsi ürün kararı):
// 1. İki panel kendi belgesini açar; bir panelin sayfası diğerini değiştirmez.
// 2. Yakınlaştırma panel başınadır (tek görünüm nesnesi paneli başkasına taşımaz).
// 3. Sayfa haritası yalnız **gerçek** blok kutuları ve sayfa ölçüsü varsa çizilir;
//    kutu/ölçü yoksa konum uydurulmaz (PageMap null döner).
// 4. Boş panel, diğer panelde açık sürümü seçmez (yan yana iki farklı belge).

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { t } from "../lib/i18n";
import type { DocumentPages, DocumentRow, PageRow } from "../lib/api";

const mocks = vi.hoisted(() => ({
  documentPages: vi.fn(),
  marksForVersion: vi.fn(),
  anchorLocate: vi.fn(),
  documentPagePreview: vi.fn(),
  documentInventory: vi.fn(),
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
    },
  };
});

const { DualReader } = await import("./DualReader");

const ISO = "2026-10-01T00:00:00Z";

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

const DOC_B: DocumentRow = {
  ...DOC_A,
  id: "d2",
  displayName: "İhtarname",
  currentVersionId: "v2",
};

const TARGET_A = {
  versionId: "v1",
  documentName: DOC_A.displayName,
  pageId: null,
  segmentId: null,
  pageIndex: null,
  startOffset: null,
  endOffset: null,
  quote: null,
  query: null,
  anchorId: null,
  quoteIntact: null,
  quads: [],
};

const TARGET_B = { ...TARGET_A, versionId: "v2", documentName: DOC_B.displayName };

const PAIN_A = pagesFor("v1", ["Sol belge birinci sayfa", "Sol belge ikinci sayfa"]);
const PAIN_B = pagesFor("v2", ["Sag belge birinci sayfa", "Sag belge ikinci sayfa"]);

function setup(overrides: { left?: typeof TARGET_A | null; right?: typeof TARGET_B | null } = {}) {
  mocks.documentPages.mockImplementation(async (versionId: string) =>
    versionId === "v2" ? PAIN_B : PAIN_A,
  );
  mocks.marksForVersion.mockResolvedValue([]);
  render(
    <DualReader
      caseId="c1"
      documents={[DOC_B, DOC_A]}
      left={overrides.left === undefined ? TARGET_A : overrides.left}
      right={overrides.right === undefined ? TARGET_B : overrides.right}
      onClose={() => {}}
    />,
  );
}

function pane(label: string): HTMLElement {
  return screen.getByRole("region", { name: label });
}

/** Panel içindeki yakınlaştırma rozetini okur (`%…` biçimli). */
function zoomBadge(scope: HTMLElement): string {
  const badge = within(scope)
    .getAllByText(/^\d+%$/)
    .at(0);
  return badge?.textContent ?? "";
}

describe("okuyucu panel davranışı", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("iki panel kendi belgesini açar ve kendi sayfa metnini gösterir", async () => {
    setup();

    expect(await screen.findByText(/Sol belge birinci sayfa/)).toBeTruthy();
    expect(await screen.findByText(/Sag belge birinci sayfa/)).toBeTruthy();

    const left = pane(t("readerUi.leftDocument"));
    const right = pane(t("readerUi.rightDocument"));
    expect((within(left).getByRole("combobox") as HTMLSelectElement).value).toBe("v1");
    expect((within(right).getByRole("combobox") as HTMLSelectElement).value).toBe("v2");

    // Her panel yalnız kendi sürümünün sayfalarını istedi (çapraz istek yok).
    expect(mocks.documentPages.mock.calls.map((call) => call[0]).sort()).toEqual(["v1", "v2"]);
  });

  it("bir panelde sayfa değişince diğer panel yerinde kalır", async () => {
    setup();
    await screen.findByText(/Sol belge birinci sayfa/);
    await screen.findByText(/Sag belge birinci sayfa/);

    const left = pane(t("readerUi.leftDocument"));
    fireEvent.click(within(left).getByRole("button", { name: "Fiziksel sayfa 2" }));

    expect(await within(left).findByText(/Sol belge ikinci sayfa/)).toBeTruthy();
    expect(within(left).queryByText(/Sol belge birinci sayfa/)).toBeNull();

    const right = pane(t("readerUi.rightDocument"));
    expect(within(right).getByText(/Sag belge birinci sayfa/)).toBeTruthy();
    expect(within(right).queryByText(/Sag belge ikinci sayfa/)).toBeNull();
  });

  it("yakınlaştırma panel başına uygulanır", async () => {
    setup();
    await screen.findByText(/Sol belge birinci sayfa/);
    await screen.findByText(/Sag belge birinci sayfa/);

    const left = pane(t("readerUi.leftDocument"));
    const right = pane(t("readerUi.rightDocument"));
    expect(zoomBadge(left)).toBe("100%");
    expect(zoomBadge(right)).toBe("100%");

    fireEvent.click(
      within(left).getByRole("button", { name: t("readerUi.zoomInAria", { label: t("readerUi.leftDocument") }) }),
    );
    // İkinci tık **mevcut** yakınlaştırmadan ilerlemeli; sabit bir kademeye
    // dönmek (1 → 1,25) sessiz kusurdur ve burada yakalanır.
    fireEvent.click(
      within(left).getByRole("button", { name: t("readerUi.zoomInAria", { label: t("readerUi.leftDocument") }) }),
    );

    expect(zoomBadge(left)).toBe("150%");
    expect(zoomBadge(right)).toBe("100%");

    fireEvent.click(
      within(left).getByRole("button", { name: t("readerUi.zoomOutAria", { label: t("readerUi.leftDocument") }) }),
    );
    expect(zoomBadge(left)).toBe("125%");
    expect(zoomBadge(right)).toBe("100%");
  });

  it("sayfa haritası yalnız gerçek blok kutusu varken çizilir", async () => {
    mocks.documentPages.mockImplementation(async (versionId: string) =>
      versionId === "v1"
        ? {
            ...PAIN_A,
            pages: [
              page({
                id: "v1-p1",
                pageIndex: 0,
                text: "Kutulu sayfa",
                blocks: [{ start: 0, end: 5, bbox: [10, 10, 200, 60] }],
              }),
            ],
          }
        : PAIN_B,
    );
    mocks.marksForVersion.mockResolvedValue([]);
    render(
      <DualReader caseId="c1" documents={[DOC_A, DOC_B]} left={TARGET_A} right={TARGET_B} onClose={() => {}} />,
    );

    await screen.findByText("Kutulu sayfa");
    const left = pane(t("readerUi.leftDocument"));
    const map = await within(left).findByRole("img", {
      name: t("pageMap.aria", { physical: "Fiziksel sayfa 1", count: 1 }),
    });
    expect(map.tagName.toLowerCase()).toBe("svg");
    // Sağ panelin sayfasında blok yok → harita çizilmedi (konum uydurulmaz).
    expect(within(pane(t("readerUi.rightDocument"))).queryByRole("img")).toBeNull();
  });

  it("sayfa ölçüsü yoksa blok kutusu olsa da harita çizilmez", async () => {
    mocks.documentPages.mockImplementation(async (versionId: string) =>
      versionId === "v1"
        ? {
            ...PAIN_A,
            pages: [
              page({
                id: "v1-p1",
                pageIndex: 0,
                text: "Olcusuz sayfa",
                width: null,
                height: null,
                blocks: [{ start: 0, end: 5, bbox: [10, 10, 200, 60] }],
              }),
            ],
          }
        : PAIN_B,
    );
    mocks.marksForVersion.mockResolvedValue([]);
    render(
      <DualReader caseId="c1" documents={[DOC_A, DOC_B]} left={TARGET_A} right={TARGET_B} onClose={() => {}} />,
    );

    await screen.findByText("Olcusuz sayfa");
    expect(within(pane(t("readerUi.leftDocument"))).queryByRole("img")).toBeNull();
  });

  it("boş panel diğer panelde açık sürümü seçmez", async () => {
    setup({ left: null, right: TARGET_B });

    await waitFor(() => {
      const left = pane(t("readerUi.leftDocument"));
      expect((within(left).getByRole("combobox") as HTMLSelectElement).value).toBe("v1");
    });

    const left = pane(t("readerUi.leftDocument"));
    expect(await within(left).findByText(/Sol belge birinci sayfa/)).toBeTruthy();
    expect(within(left).queryByText(/Sag belge birinci sayfa/)).toBeNull();
  });
});
