// @vitest-environment jsdom
//
// Okuyucu (DualReader) **seçim akışı** davranış testi.
//
// `DualReader.test.tsx` panel/sayfa/yakınlaştırma davranışını ölçüyor; metinden
// **çekirdeğe giden** üç komut (kaynak = anchor, kalıcı işaret, kaynaklı not)
// bugüne dek hiç ölçülmemişti. Burada sabitlenenler:
// 1. Seçim yokken işlem düğmeleri kapalı; seçim yapılınca aralık **skaler**
//    (Unicode scalar) offsetlerle gösterilir ve düğmeler açılır.
// 2. Kaynak kaydı tam sözleşmeyle gider, başarıda seçim bırakılır, üst bileşene
//    iletilir; düşerse hata yazılır ve **seçim korunur** (yeniden denenebilir).
// 3. İşaret seçimden oluşturulur ve listenin parçası olur; düşen işaret listeye
//    satır eklemez.
// 4. Not taslağı alıntıyla açılır, **hedefini dondurur** (başka metin seçmek ya
//    da sayfa değiştirmek notu başka alıntıya taşımaz), gövde boşken kaydolmaz,
//    düşen kayıt yazılanı korur.
// 5. Her işlem başında önceki hata/son rapor silinir (bayat bildirim yok) ve
//    sürüm değiştiyse geç gelen yanıt yeni sürümün ekranına yazılmaz.

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { t } from "../lib/i18n";
import type { AnchorRow, DocumentPages, DocumentRow, MarkRow, NoteRow, PageRow } from "../lib/api";

const mocks = vi.hoisted(() => ({
  documentPages: vi.fn(),
  marksForVersion: vi.fn(),
  anchorLocate: vi.fn(),
  documentPagePreview: vi.fn(),
  documentInventory: vi.fn(),
  anchorCreate: vi.fn(),
  markCreateFromSelection: vi.fn(),
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
      anchorCreate: mocks.anchorCreate,
      markCreateFromSelection: mocks.markCreateFromSelection,
      noteCreateFromSelection: mocks.noteCreateFromSelection,
      markDelete: mocks.markDelete,
    },
  };
});

const { DualReader } = await import("./DualReader");

const ISO = "2026-10-01T00:00:00Z";

/** Seçilecek alıntı; metin içinde **tek** kez geçer. */
const NEEDLE = "1 Şubat 2026";
const TEXT_A1 = "Kira sözleşmesi 1 Şubat 2026 tarihinde imzalanmıştır.";
const TEXT_A2 = "İkinci sayfa: ödeme planı ekte sunulmuştur.";
const TEXT_B1 = "İhtarname 3 Mart 2026 tarihinde tebliğ edilmiştir.";
/** Skaler offset: metnin başından alıntıya kadar olan **karakter** sayısı. */
const START = Array.from(TEXT_A1.slice(0, TEXT_A1.indexOf(NEEDLE))).length;
const END = START + Array.from(NEEDLE).length;

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

function anchorRow(overrides: Partial<AnchorRow> = {}): AnchorRow {
  return {
    id: "a1",
    caseId: "c1",
    documentVersionId: "v1",
    parseRunId: "run-v1",
    pageId: "v1-p1",
    segmentId: "seg-v1-p1",
    startOffset: START,
    endOffset: END,
    exactQuote: NEEDLE,
    prefix: "",
    suffix: "",
    quoteHash: "hash-1",
    anchorStatus: "verified",
    createdAt: ISO,
    ...overrides,
  };
}

function markRow(overrides: Partial<MarkRow> = {}): MarkRow {
  return {
    id: "m1",
    caseId: "c1",
    documentVersionId: "v1",
    parseRunId: "run-v1",
    anchorId: "a1",
    pageId: "v1-p1",
    markKind: "text",
    startOffset: START,
    endOffset: END,
    quotedText: NEEDLE,
    rects: [],
    label: "",
    createdAt: ISO,
    ...overrides,
  };
}

function noteRow(overrides: Partial<NoteRow> = {}): NoteRow {
  return {
    id: "n1",
    caseId: "c1",
    title: "Kira notu",
    body: "Gövde",
    authorKind: "user",
    visibilityLabel: "",
    sourceAnchorIds: ["a1"],
    approvedAt: null,
    createdAt: ISO,
    updatedAt: ISO,
    rowVersion: 1,
    revisionNo: 1,
    canUndo: false,
    canRedo: false,
    ...overrides,
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

function mount(
  options: {
    pages?: DocumentPages;
    onAnchor?: (anchor: AnchorRow) => void;
    onNote?: (note: NoteRow) => void;
  } = {},
) {
  mocks.documentPages.mockImplementation(async (versionId: string) => {
    if (options.pages && versionId === "v1") return options.pages;
    return versionId === "v2" ? PAIN_B : PAIN_A;
  });
  mocks.marksForVersion.mockResolvedValue([]);
  mocks.anchorLocate.mockResolvedValue(null);
  // `exactOptionalPropertyTypes`: tanımsız geri çağırma prop olarak geçmez.
  const callbacks = {
    ...(options.onAnchor ? { onAnchor: options.onAnchor } : {}),
    ...(options.onNote ? { onNote: options.onNote } : {}),
  };
  render(
    <DualReader
      caseId="c1"
      documents={[DOC_B, DOC_A]}
      left={TARGET_A}
      right={TARGET_B}
      onClose={() => {}}
      {...callbacks}
    />,
  );
}

function pane(label: string): HTMLElement {
  return screen.getByRole("region", { name: label });
}

/** Panelin metin kutusu (`aria-label` sayfa etiketinden gelir). */
function textBox(scope: HTMLElement): HTMLElement {
  return within(scope).getByLabelText(t("readerUi.textAria", { physical: t("readerUi.physicalPage", { page: 1 }) }));
}

/** DOM seçimini verilen alıntıya kurar ve `mouseup`ı tetikler. */
function selectText(scope: HTMLElement, needle: string) {
  const box = textBox(scope);
  const full = box.textContent ?? "";
  const utf16Start = full.indexOf(needle);
  if (utf16Start < 0) throw new Error(`seçilecek metin bulunamadı: ${needle}`);
  const walker = document.createTreeWalker(box, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  for (let node = walker.nextNode(); node; node = walker.nextNode()) nodes.push(node as Text);
  const locate = (index: number) => {
    let left = index;
    for (const node of nodes) {
      if (left <= node.data.length) return { node, offset: left };
      left -= node.data.length;
    }
    const last = nodes[nodes.length - 1]!;
    return { node: last, offset: last.data.length };
  };
  const from = locate(utf16Start);
  const to = locate(utf16Start + needle.length);
  const range = document.createRange();
  range.setStart(from.node, from.offset);
  range.setEnd(to.node, to.offset);
  const selection = window.getSelection()!;
  selection.removeAllRanges();
  selection.addRange(range);
  fireEvent.mouseUp(box);
}

function bindButton(scope: HTMLElement): HTMLButtonElement {
  return within(scope).getByRole("button", { name: t("readerUi.bindSelection") }) as HTMLButtonElement;
}

function markButton(scope: HTMLElement): HTMLButtonElement {
  return within(scope).getByRole("button", { name: t("readerUi.mark") }) as HTMLButtonElement;
}

function noteButton(scope: HTMLElement): HTMLButtonElement {
  return within(scope).getByRole("button", { name: t("readerUi.noteFromSelection") }) as HTMLButtonElement;
}

function noteBody(scope: HTMLElement): HTMLTextAreaElement {
  return within(scope).getByLabelText(t("readerUi.noteLabel")) as HTMLTextAreaElement;
}

function noteTitle(scope: HTMLElement): HTMLInputElement {
  return within(scope).getByLabelText(t("readerUi.titleLabel")) as HTMLInputElement;
}

function saveNoteButton(scope: HTMLElement): HTMLButtonElement {
  return within(scope).getByRole("button", { name: t("readerUi.saveNote") }) as HTMLButtonElement;
}

/** Satırı **tam metniyle** arar: seçim/kaynak satırları birden çok metin
 *  düğümünden oluşur (`getByText` tam eşleşmede bulamaz). En derin eşleşme
 *  döner (ata düğümler de aynı metni taşıyabilir). */
function textLine(scope: HTMLElement, expected: string): HTMLElement | null {
  const matches = [...scope.querySelectorAll<HTMLElement>("div, p, span, summary, li")].filter(
    (node) => (node.textContent ?? "").trim() === expected,
  );
  return matches.at(-1) ?? null;
}

async function leftPaneWithText(): Promise<HTMLElement> {
  await screen.findByText(TEXT_A1);
  return pane(t("readerUi.leftDocument"));
}

describe("okuyucu seçim akışı", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    window.getSelection()?.removeAllRanges();
  });

  it("seçim yokken işlem düğmeleri kapalıdır; seçim skaler aralıkla gösterilir", async () => {
    mount();
    const left = await leftPaneWithText();

    expect(textBox(left).textContent).toBe(TEXT_A1);
    expect(bindButton(left).disabled).toBe(true);
    expect(markButton(left).disabled).toBe(true);
    expect(noteButton(left).disabled).toBe(true);

    selectText(left, NEEDLE);

    expect(
      textLine(
        left,
        `${t("readerUi.selectionLine", { quote: NEEDLE })} · ${t("readerUi.selectionRange", { start: START, end: END })}`,
      ),
    ).toBeTruthy();
    expect(bindButton(left).disabled).toBe(false);
    expect(markButton(left).disabled).toBe(false);
    expect(noteButton(left).disabled).toBe(false);
  });

  it("seçimdeki offsetler skaler sayılır (emoji tek karakter)", async () => {
    mount({ pages: pagesFor("v1", ["😀 Kira bedeli 12.500 TL", TEXT_A2]) });
    await screen.findByText("😀 Kira bedeli 12.500 TL");
    const left = pane(t("readerUi.leftDocument"));

    selectText(left, "Kira bedeli");
    // UTF-16 uzunluğu 3 olurdu (emoji vekil çift); gösterilen aralık skaler 2–13.
    expect(
      textLine(
        left,
        `${t("readerUi.selectionLine", { quote: "Kira bedeli" })} · ${t("readerUi.selectionRange", { start: 2, end: 13 })}`,
      ),
    ).toBeTruthy();

    mocks.anchorCreate.mockResolvedValue(anchorRow({ id: "a9", startOffset: 2, endOffset: 13, exactQuote: "Kira bedeli" }));
    fireEvent.click(bindButton(left));

    await waitFor(() => expect(mocks.anchorCreate).toHaveBeenCalledTimes(1));
    expect(mocks.anchorCreate.mock.calls[0]![0]).toMatchObject({ startOffset: 2, endOffset: 13 });
  });

  it("kaynak kaydı tam sözleşmeyle gider, seçim bırakılır ve üst bileşene iletilir", async () => {
    const onAnchor = vi.fn();
    mount({ onAnchor });
    const left = await leftPaneWithText();
    const anchor = anchorRow();
    mocks.anchorCreate.mockResolvedValue(anchor);

    selectText(left, NEEDLE);
    fireEvent.click(bindButton(left));

    await waitFor(() => expect(mocks.anchorCreate).toHaveBeenCalledTimes(1));
    expect(mocks.anchorCreate.mock.calls[0]![0]).toEqual({
      caseId: "c1",
      documentVersionId: "v1",
      parseRunId: "run-v1",
      segmentId: "seg-v1-p1",
      startOffset: START,
      endOffset: END,
      exactQuote: NEEDLE,
    });
    expect(
      await within(left).findByText(t("readerUi.sourceSaved", { quote: NEEDLE })),
    ).toBeTruthy();
    expect(onAnchor).toHaveBeenCalledWith(anchor);
    expect(within(left).queryByText(t("readerUi.selectionLine", { quote: NEEDLE }))).toBeNull();
    expect(bindButton(left).disabled).toBe(true);
  });

  it("düşen kaynak kaydı hatayı yazar ve seçimi korur", async () => {
    mount();
    const left = await leftPaneWithText();
    mocks.anchorCreate.mockRejectedValue({ code: "E_QUOTE", message: "Alıntı segmentte doğrulanamadı.", retryable: false });

    selectText(left, NEEDLE);
    fireEvent.click(bindButton(left));

    const alert = await within(left).findByRole("alert");
    expect(alert.textContent).toContain("E_QUOTE");
    expect(
      textLine(
        left,
        `${t("readerUi.selectionLine", { quote: NEEDLE })} · ${t("readerUi.selectionRange", { start: START, end: END })}`,
      ),
    ).toBeTruthy();
    expect(bindButton(left).disabled).toBe(false);
  });

  it("kaynak kaydı sürerken ikinci tık ikinci istek üretmez", async () => {
    mount();
    const left = await leftPaneWithText();
    const gate = deferred<AnchorRow>();
    mocks.anchorCreate.mockReturnValue(gate.promise);

    selectText(left, NEEDLE);
    fireEvent.click(bindButton(left));
    fireEvent.click(bindButton(left));

    expect(mocks.anchorCreate).toHaveBeenCalledTimes(1);
    gate.resolve(anchorRow());
    expect(await within(left).findByText(t("readerUi.sourceSaved", { quote: NEEDLE }))).toBeTruthy();
    expect(bindButton(left).disabled).toBe(true);
  });

  it("seçimden işaret oluşturulur ve belgenin işaret listesine girer", async () => {
    mount();
    const left = await leftPaneWithText();
    mocks.markCreateFromSelection.mockResolvedValue({ anchor: anchorRow(), mark: markRow() });

    selectText(left, NEEDLE);
    fireEvent.click(markButton(left));

    await waitFor(() => expect(mocks.markCreateFromSelection).toHaveBeenCalledTimes(1));
    expect(mocks.markCreateFromSelection.mock.calls[0]![0]).toMatchObject({
      caseId: "c1",
      documentVersionId: "v1",
      segmentId: "seg-v1-p1",
      startOffset: START,
      endOffset: END,
      exactQuote: NEEDLE,
    });
    expect(mocks.markCreateFromSelection.mock.calls[0]![1]).toBe("");
    expect(await within(left).findByText(t("readerUi.markSaved"))).toBeTruthy();
    expect(within(left).getByText(t("readerUi.marksSummary", { count: 1 }))).toBeTruthy();
    expect(within(left).queryByText(t("readerUi.selectionLine", { quote: NEEDLE }))).toBeNull();
  });

  it("düşen işaret listeye satır eklemez", async () => {
    mount();
    const left = await leftPaneWithText();
    mocks.markCreateFromSelection.mockRejectedValue({ code: "E_MARK", message: "İşaret yazılamadı.", retryable: false });

    selectText(left, NEEDLE);
    fireEvent.click(markButton(left));

    const alert = await within(left).findByRole("alert");
    expect(alert.textContent).toContain("E_MARK");
    expect(within(left).queryByText(t("readerUi.marksSummary", { count: 1 }))).toBeNull();
    expect(within(left).queryByText(t("readerUi.markSaved"))).toBeNull();
    expect(markButton(left).disabled).toBe(false);
  });

  it("seçimden not taslağı alıntıyla ve kaynak satırıyla açılır", async () => {
    mount();
    const left = await leftPaneWithText();

    selectText(left, NEEDLE);
    fireEvent.click(noteButton(left));

    expect(noteBody(left).value).toBe(NEEDLE);
    expect(noteTitle(left).value).toBe(
      t("readerUi.noteTitle", { page: t("readerUi.physicalPage", { page: 1 }) }),
    );
    expect(
      textLine(
        left,
        t("readerUi.noteSource", {
          physical: t("readerUi.physicalPage", { page: 1 }),
          start: START,
          end: END,
        }),
      ),
    ).toBeTruthy();
    expect(saveNoteButton(left).disabled).toBe(false);
  });

  it("not kaydı çapayı ve yazılan gövdeyi tek istekte gönderir", async () => {
    const onNote = vi.fn();
    mount({ onNote });
    const left = await leftPaneWithText();
    const saved = noteRow({ title: "Kira notu", body: "Kira bedeli aylık ödenir." });
    mocks.noteCreateFromSelection.mockResolvedValue({ anchor: anchorRow(), note: saved });

    selectText(left, NEEDLE);
    fireEvent.click(noteButton(left));
    fireEvent.change(noteTitle(left), { target: { value: "Kira notu" } });
    fireEvent.change(noteBody(left), { target: { value: "Kira bedeli aylık ödenir." } });
    fireEvent.click(saveNoteButton(left));

    await waitFor(() => expect(mocks.noteCreateFromSelection).toHaveBeenCalledTimes(1));
    expect(mocks.noteCreateFromSelection.mock.calls[0]![0]).toEqual({
      caseId: "c1",
      documentVersionId: "v1",
      parseRunId: "run-v1",
      segmentId: "seg-v1-p1",
      startOffset: START,
      endOffset: END,
      exactQuote: NEEDLE,
    });
    expect(mocks.noteCreateFromSelection.mock.calls[0]![1]).toEqual({
      title: "Kira notu",
      body: "Kira bedeli aylık ödenir.",
    });
    expect(
      await within(left).findByText(t("readerUi.noteCreated", { title: "Kira notu" })),
    ).toBeTruthy();
    expect(onNote).toHaveBeenCalledWith(saved);
    expect(within(left).queryByLabelText(t("readerUi.noteLabel"))).toBeNull();
    expect(within(left).queryByText(t("readerUi.selectionLine", { quote: NEEDLE }))).toBeNull();
  });

  it("düşen not kaydı yazılanı korur ve yeniden denenebilir", async () => {
    mount();
    const left = await leftPaneWithText();
    mocks.noteCreateFromSelection.mockRejectedValue({ code: "E_NOTE", message: "Not yazılamadı.", retryable: true });

    selectText(left, NEEDLE);
    fireEvent.click(noteButton(left));
    fireEvent.change(noteBody(left), { target: { value: "Uzun uzun yazılmış not." } });
    fireEvent.click(saveNoteButton(left));

    const alert = await within(left).findByRole("alert");
    expect(alert.textContent).toContain("E_NOTE");
    expect(noteBody(left).value).toBe("Uzun uzun yazılmış not.");
    expect(saveNoteButton(left).disabled).toBe(false);
  });

  it("boş gövde çekirdeğe gönderilmez", async () => {
    mount();
    const left = await leftPaneWithText();

    selectText(left, NEEDLE);
    fireEvent.click(noteButton(left));
    fireEvent.change(noteBody(left), { target: { value: "   " } });

    expect(saveNoteButton(left).disabled).toBe(true);
    fireEvent.click(saveNoteButton(left));
    expect(mocks.noteCreateFromSelection).not.toHaveBeenCalled();
  });

  it("not taslağı açıkken başka metin seçmek notu ilk alıntıya bağlı tutar", async () => {
    mount();
    const left = await leftPaneWithText();
    mocks.noteCreateFromSelection.mockResolvedValue({ anchor: anchorRow(), note: noteRow() });

    selectText(left, NEEDLE);
    fireEvent.click(noteButton(left));
    // Kullanıcı taslağı yazarken belgede başka bir metni seçiyor.
    selectText(left, "tarihinde imzalanmıştır");
    fireEvent.click(saveNoteButton(left));

    await waitFor(() => expect(mocks.noteCreateFromSelection).toHaveBeenCalledTimes(1));
    // Gövde ilk alıntıdan geldiği için çapa da **ilk** alıntı olmalı; yeni seçim
    // sessizce notun hedefini değiştirmemeli.
    expect(mocks.noteCreateFromSelection.mock.calls[0]![0]).toMatchObject({
      startOffset: START,
      endOffset: END,
      exactQuote: NEEDLE,
    });
    expect(mocks.noteCreateFromSelection.mock.calls[0]![1]).toMatchObject({ body: NEEDLE });
  });

  it("not taslağı sayfa değişince hedefini korur ve sessizce kaybolmaz", async () => {
    mount();
    const left = await leftPaneWithText();
    mocks.noteCreateFromSelection.mockResolvedValue({ anchor: anchorRow(), note: noteRow() });

    selectText(left, NEEDLE);
    fireEvent.click(noteButton(left));
    fireEvent.click(within(left).getByRole("button", { name: "Fiziksel sayfa 2" }));
    expect(await within(left).findByText(TEXT_A2)).toBeTruthy();

    // Sayfa değişimi seçimi bırakır; taslak kendi alıntısını taşıdığı için
    // kaydetme sessizce hiçbir şey yapmamalı değil, ilk sayfaya yazmalı.
    expect(
      textLine(
        left,
        t("readerUi.noteSource", {
          physical: t("readerUi.physicalPage", { page: 1 }),
          start: START,
          end: END,
        }),
      ),
    ).toBeTruthy();
    fireEvent.click(saveNoteButton(left));

    await waitFor(() => expect(mocks.noteCreateFromSelection).toHaveBeenCalledTimes(1));
    expect(mocks.noteCreateFromSelection.mock.calls[0]![0]).toMatchObject({
      documentVersionId: "v1",
      segmentId: "seg-v1-p1",
      startOffset: START,
      endOffset: END,
    });
  });

  it("ikinci kaynak denemesi düşerse önceki yeşil kaynak satırı kalkar", async () => {
    mount();
    const left = await leftPaneWithText();
    mocks.anchorCreate.mockResolvedValueOnce(anchorRow());

    selectText(left, NEEDLE);
    fireEvent.click(bindButton(left));
    expect(
      await within(left).findByText(t("readerUi.sourceSaved", { quote: NEEDLE })),
    ).toBeTruthy();

    mocks.anchorCreate.mockRejectedValueOnce({ code: "E_QUOTE", message: "Alıntı doğrulanamadı.", retryable: false });
    selectText(left, NEEDLE);
    fireEvent.click(bindButton(left));

    expect((await within(left).findByRole("alert")).textContent).toContain("E_QUOTE");
    expect(within(left).queryByText(t("readerUi.sourceSaved", { quote: NEEDLE }))).toBeNull();
  });

  it("önceki hatayı başarılı işlem siler (bayat bildirim kalmaz)", async () => {
    mount();
    const left = await leftPaneWithText();
    mocks.anchorCreate.mockRejectedValue({ code: "E_QUOTE", message: "Alıntı doğrulanamadı.", retryable: false });
    mocks.markCreateFromSelection.mockResolvedValue({ anchor: anchorRow(), mark: markRow() });

    selectText(left, NEEDLE);
    fireEvent.click(bindButton(left));
    expect((await within(left).findByRole("alert")).textContent).toContain("E_QUOTE");

    fireEvent.click(markButton(left));
    await waitFor(() => expect(within(left).queryByRole("alert")).toBeNull());
    expect(within(left).getByText(t("readerUi.markSaved"))).toBeTruthy();
  });

  it("sürüm değişince geç gelen işaret yanıtı yeni sürüme yazılmaz", async () => {
    mount();
    const left = await leftPaneWithText();
    const gate = deferred<{ anchor: AnchorRow; mark: MarkRow }>();
    mocks.markCreateFromSelection.mockReturnValue(gate.promise);

    selectText(left, NEEDLE);
    fireEvent.click(markButton(left));
    expect(mocks.markCreateFromSelection).toHaveBeenCalledTimes(1);

    // İstek askıdayken panel başka belgeye geçiyor.
    fireEvent.change(within(left).getByRole("combobox"), { target: { value: "v2" } });
    expect(await within(left).findByText(TEXT_B1)).toBeTruthy();

    gate.resolve({ anchor: anchorRow(), mark: markRow() });
    await waitFor(() => expect(within(left).queryByRole("alert")).toBeNull());
    // Eski sürümün işareti ve başarı bildirimi yeni sürümün ekranına düşmemeli.
    expect(within(left).queryByText(t("readerUi.marksSummary", { count: 1 }))).toBeNull();
    expect(within(left).queryByText(t("readerUi.markSaved"))).toBeNull();
  });
});
