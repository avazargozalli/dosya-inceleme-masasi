// @vitest-environment jsdom
//
// İnceleme (Review) ekranı davranış testi. Lib düzeyindeki testler
// (intext / reviewTarget / i18n-bindings) parçaların **metnini** doğrular;
// burada ekran gerçekten render edilip ürün kararlarının **arayüzde**
// uygulandığı ölçülür. Ölçülen kararlar:
//
// 1. "Kabul et ve kaydet" hedef kayıt türü (iddiada ayrıca iddia türü)
//    seçilmeden **açılmaz** ve kabul `reviewCommit` ile **tek işlemde**
//    yazılır — düz `reviewDecide("accept")` yoluna sessizce düşmez (F08-T08).
// 2. Hedef kaydı üretilmiş onaylı öğe "yerine geç" listesine **girmez**;
//    onaylı kayıt sessizce değiştirilemez (F08-T09).
// 3. Alıntı denetimi **seçilen kapsamla** koşar; metin/kapsam değişince
//    eski sonuç ekranda **kalmaz** (bayat hüküm gösterilmez).
// 4. "Kaynağa bağla" yalnız gerçekten bağlanabilir alıntıda çıkar; sayfa
//    sınırını aşan eşleşmede düğme yerine nedeni yazılır ve bağlama
//    `anchorCreate`e doğru offset'lerle gider.
// 5. Çelişki adayı **iki farklı** kaynak ister; aynı kaynak seçilirse
//    kuyruğa ekleme kapanır (tek kaynaklı "çelişki" üretilemez).
// 6. Geçersiz JSON düzeltmesi gönderilmez (bozuk payload yazılmaz).
// 7. Kuyruk sayımı: bekleyen = `pending` + `needs_more_evidence`.

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../lib/i18n";
import { quoteCheckSummary } from "../lib/intext";
import type {
  AnchorRow,
  CaseRow,
  ClaimRow,
  DocumentRow,
  EvidenceLinkRow,
  InTextQuoteCheck,
  InTextQuoteFinding,
  InTextQuotePosition,
  ReviewItemRow,
} from "../lib/api";

const mocks = vi.hoisted(() => ({
  caseList: vi.fn(),
  claimList: vi.fn(),
  claimCreate: vi.fn(),
  reviewItemList: vi.fn(),
  anchorList: vi.fn(),
  documentList: vi.fn(),
  reviewOutboxPending: vi.fn(),
  contradictionAssessments: vi.fn(),
  inTextQuoteCheck: vi.fn(),
  anchorCreate: vi.fn(),
  reviewCommit: vi.fn(),
  reviewDecide: vi.fn(),
  reviewSupersede: vi.fn(),
  reviewItemCreate: vi.fn(),
  contradictionAssess: vi.fn(),
  evidenceLinkList: vi.fn(),
  evidenceLinkCreate: vi.fn(),
  claimRelationList: vi.fn(),
  claimRelationCreate: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      caseList: mocks.caseList,
      claimList: mocks.claimList,
      claimCreate: mocks.claimCreate,
      reviewItemList: mocks.reviewItemList,
      anchorList: mocks.anchorList,
      documentList: mocks.documentList,
      reviewOutboxPending: mocks.reviewOutboxPending,
      contradictionAssessments: mocks.contradictionAssessments,
      inTextQuoteCheck: mocks.inTextQuoteCheck,
      anchorCreate: mocks.anchorCreate,
      reviewCommit: mocks.reviewCommit,
      reviewDecide: mocks.reviewDecide,
      reviewSupersede: mocks.reviewSupersede,
      reviewItemCreate: mocks.reviewItemCreate,
      contradictionAssess: mocks.contradictionAssess,
      evidenceLinkList: mocks.evidenceLinkList,
      evidenceLinkCreate: mocks.evidenceLinkCreate,
      claimRelationList: mocks.claimRelationList,
      claimRelationCreate: mocks.claimRelationCreate,
    },
  };
});

const { Review } = await import("./Review");

const ISO = "2026-10-01T00:00:00Z";

const CASE: CaseRow = {
  id: "c1",
  title: "Kira uyuşmazlığı",
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
};

const CASE2: CaseRow = {
  ...CASE,
  id: "c2",
  title: "İhtar dosyası",
  referenceCode: "IH-2",
};

const CLAIMS: ClaimRow[] = [
  {
    id: "cl1",
    caseId: "c1",
    claimantActorId: null,
    claimText: "Kira bedeli ödenmedi",
    claimKind: "allegation",
    status: "open",
    sourceAnchorId: null,
    createdAt: ISO,
    updatedAt: ISO,
    rowVersion: 1,
  },
];

function anchor(id: string, quote: string, start: number): AnchorRow {
  return {
    id,
    caseId: "c1",
    documentVersionId: "v1",
    parseRunId: "run-1",
    pageId: "p1",
    segmentId: "seg-1",
    startOffset: start,
    endOffset: start + quote.length,
    exactQuote: quote,
    prefix: "",
    suffix: "",
    quoteHash: `hash-${id}`,
    anchorStatus: "active",
    createdAt: ISO,
  };
}

const ANCHORS: AnchorRow[] = [
  anchor("a1", "Kira bedeli ödenmedi", 10),
  anchor("a2", "Ödeme 3 Ekimde yapıldı", 100),
];

const DOCUMENTS: DocumentRow[] = [
  {
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
  },
];

function item(overrides: Partial<ReviewItemRow> & { id: string; itemKind: string }): ReviewItemRow {
  return {
    caseId: "c1",
    payloadSchemaVersion: 1,
    payload: null,
    sourceAnchorIds: [],
    aiRunId: null,
    state: "pending",
    targetEntityId: null,
    supersededByItemId: null,
    supersededAt: null,
    createdAt: ISO,
    updatedAt: ISO,
    rowVersion: 1,
    ...overrides,
  };
}

const ITEMS: ReviewItemRow[] = [
  item({ id: "i-pend", itemKind: "note_candidate", payload: { text: "Kira ödenmedi notu" }, sourceAnchorIds: ["a1"] }),
  item({
    id: "i-pend2",
    itemKind: "claim_candidate",
    payload: { text: "Ödeme yapıldı" },
    sourceAnchorIds: ["a1"],
    state: "needs_more_evidence",
  }),
  item({
    id: "i-contra",
    itemKind: "contradiction_candidate",
    payload: { text: "Tarih çelişkisi" },
    sourceAnchorIds: ["a1", "a2"],
  }),
  item({
    id: "i-acc",
    itemKind: "claim_candidate",
    payload: { text: "Ödenmedi" },
    state: "accepted",
    targetEntityId: "9f8e7d6c-1111-2222",
  }),
  item({ id: "i-acc2", itemKind: "note_candidate", payload: { text: "Not" }, state: "accepted" }),
];

function position(overrides: Partial<InTextQuotePosition> = {}): InTextQuotePosition {
  return {
    documentId: "d1",
    documentName: "Kira sözleşmesi",
    documentVersionId: "v1",
    parseRunId: "run-1",
    pageId: "p1",
    segmentId: "seg-1",
    pageIndex: 0,
    endPageIndex: 0,
    spansPages: false,
    versionNo: 1,
    isCurrentVersion: true,
    charStart: 10,
    charEnd: 30,
    matchKind: "exact",
    textOrigin: "native",
    sourceText: "kira bedeli ödenmedi",
    ...overrides,
  };
}

function check(overrides: Partial<InTextQuoteCheck> = {}): InTextQuoteCheck {
  return {
    caseId: "c1",
    scope: "case",
    scopeDocumentId: null,
    scopeDocumentName: null,
    textChars: 24,
    spansFound: 1,
    skippedShort: 0,
    skippedLong: 0,
    unclosed: 0,
    quotes: [],
    exact: 1,
    normalized: 0,
    ambiguous: 0,
    notFound: 0,
    unverifiable: 0,
    documentsTotal: 1,
    documentsWithText: 1,
    documentsWithoutText: 0,
    pagesScanned: 3,
    versionsScanned: 1,
    historicalVersionsScanned: 0,
    foundInOldVersion: 0,
    dehyphenated: 0,
    ligature: 0,
    notes: [],
    checkedAt: ISO,
    ...overrides,
  };
}

function finding(overrides: Partial<InTextQuoteFinding> & { index: number; quote: string }): InTextQuoteFinding {
  return {
    state: "exact",
    reason: null,
    occurrences: 1,
    distinctDocuments: 1,
    position: position(),
    candidates: [],
    ...overrides,
  };
}

/** İkinci iddia (aynı dosyada) ve ikinci dosyanın künyeleri. */
const CLAIM_B: ClaimRow = {
  ...CLAIMS[0]!,
  id: "cl2",
  claimText: "Ödeme dekontu sunuldu",
  claimKind: "defense",
};

const CLAIM_C2: ClaimRow = {
  ...CLAIMS[0]!,
  id: "cl9",
  caseId: "c2",
  claimText: "İhtar bedeli ödenmedi",
};

const ITEM_C2: ReviewItemRow = item({
  id: "i-c2",
  itemKind: "note_candidate",
  payload: { text: "İhtar notu" },
  caseId: "c2",
});

const ANCHOR_C2: AnchorRow = {
  ...anchor("b1", "İhtar 3 Ekimde gönderildi", 0),
  caseId: "c2",
};

function link(id: string, claimId: string, anchorId: string, relation: string): EvidenceLinkRow {
  return {
    id,
    caseId: "c1",
    claimId,
    sourceAnchorId: anchorId,
    relation,
    rationale: "",
    reviewState: "pending",
    createdAt: ISO,
  };
}

/** "Bağlam · Kira bedeli ödenmedi" satırı. */
const LINK_A = link("ln-a", "cl1", "a1", "context");
/** "Sunuldu · Ödeme 3 Ekimde yapıldı" satırı. */
const LINK_B = link("ln-b", "cl2", "a2", "submitted_for");

/** İkinci dosyanın iddiası listede ve seçicide geçer; düğme rolüyle aranır. */
const C2_CLAIM_BUTTON = /İhtar bedeli ödenmedi/;

/** Askıda tutulabilen söz (uçuştaki istekleri ölçmek için). */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

type MaybeAsync<T> = T[] | Promise<T[]> | (() => Promise<T[]>);

type CaseFixtures = {
  claims?: MaybeAsync<ClaimRow>;
  items?: MaybeAsync<ReviewItemRow>;
  anchors?: MaybeAsync<AnchorRow>;
  documents?: MaybeAsync<DocumentRow>;
};

async function resolveAs<T>(value: MaybeAsync<T> | undefined, fallback: T[]): Promise<T[]> {
  if (value === undefined) return fallback;
  if (typeof value === "function") return value();
  return value;
}

function setup(
  overrides: { quoteCheck?: InTextQuoteCheck; byCase?: Record<string, CaseFixtures> } = {},
) {
  const byCase = overrides.byCase ?? {};
  mocks.caseList.mockResolvedValue([CASE, CASE2]);
  mocks.claimList.mockImplementation(async (id: string) => resolveAs(byCase[id]?.claims, CLAIMS));
  mocks.reviewItemList.mockImplementation(async (id: string) => resolveAs(byCase[id]?.items, ITEMS));
  mocks.anchorList.mockImplementation(async (id: string) => resolveAs(byCase[id]?.anchors, ANCHORS));
  mocks.documentList.mockImplementation(async (id: string) =>
    resolveAs(byCase[id]?.documents, DOCUMENTS),
  );
  mocks.reviewOutboxPending.mockResolvedValue([{ id: "o1" }]);
  mocks.contradictionAssessments.mockResolvedValue([]);
  mocks.evidenceLinkList.mockResolvedValue([]);
  mocks.claimRelationList.mockResolvedValue([]);
  mocks.inTextQuoteCheck.mockResolvedValue(overrides.quoteCheck ?? check());
  mocks.anchorCreate.mockResolvedValue(anchor("an-12345678", "kira bedeli ödenmedi", 10));
  mocks.claimCreate.mockResolvedValue(CLAIMS[0]);
  mocks.evidenceLinkCreate.mockResolvedValue(LINK_A);
  mocks.claimRelationCreate.mockResolvedValue({ id: "rel-new" });
  mocks.reviewCommit.mockResolvedValue({});
  mocks.reviewDecide.mockResolvedValue({});
  mocks.reviewItemCreate.mockResolvedValue({});
  mocks.reviewSupersede.mockResolvedValue({});
  render(<Review />);
}

/** Aday kartını payload metninden bulur (kart içindeki `<pre>` JSON'u). */
function itemCard(payloadText: string): HTMLElement {
  const pre = screen.getByText(new RegExp(payloadText), { selector: "pre" });
  const card = pre.closest(".card");
  if (!(card instanceof HTMLElement)) throw new Error("aday kartı bulunamadı");
  return card;
}

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) throw new Error(`#${id} yok`);
  return element as T;
}

describe("inceleme ekranı davranışı", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("kuyruk sayımı ek kanıt bekleyenleri bekleyen sayar, hedefe yazılmış kayıt yerine geçmez", async () => {
    setup();

    // Bekleyen = i-pend + i-pend2 (needs_more_evidence) + i-contra = 3; karar = 2.
    expect(
      await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 })),
    ).toBeTruthy();

    // Hedef kaydı üretilmiş onaylı öğe "yerine geç" listesinde yok.
    const card = itemCard("Kira ödenmedi notu");
    const supersede = within(card).getByLabelText(t("review.supersedeLabel")) as HTMLSelectElement;
    const values = Array.from(supersede.options).map((option) => option.value);
    expect(values).toContain("i-acc2");
    expect(values).not.toContain("i-acc");

    // Hedef kimliği karar listesinde rozet olarak görünür (ilk 8 hane).
    expect(screen.getByText(t("review.targetBadge", { id: "9f8e7d6c" }))).toBeTruthy();
  });

  it("kabul, hedef kayıt seçilmeden açılmaz ve seçilince tek işlemde yazılır", async () => {
    setup();
    await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 }));

    const card = itemCard("Kira ödenmedi notu");
    await waitFor(() => expect(within(card).getByRole("button", { name: t("review.acceptCommit") })).toBeTruthy());
    const accept = within(card).getByRole("button", { name: t("review.acceptCommit") }) as HTMLButtonElement;
    expect(accept.disabled).toBe(true);

    // Kayıt türü seçilir: iddia türü de istenmeden buton açılmaz.
    const kind = within(card).getByLabelText(t("review.targetKindLabel")) as HTMLSelectElement;
    fireEvent.change(kind, { target: { value: "claim" } });
    expect(accept.disabled).toBe(true);

    const claimKind = within(card).getByLabelText(t("review.targetClaimKindLabel")) as HTMLSelectElement;
    fireEvent.change(claimKind, { target: { value: "allegation" } });
    expect(accept.disabled).toBe(false);

    fireEvent.click(accept);
    await waitFor(() => expect(mocks.reviewCommit).toHaveBeenCalledTimes(1));
    expect(mocks.reviewCommit).toHaveBeenCalledWith(
      "i-pend",
      "accept",
      { kind: "claim", claimText: "Kira ödenmedi notu", claimKind: "allegation" },
      t("review.auditUserApproval"),
    );
    // Kabul düz karar yoluna düşmedi: hedefsiz onay yazılmadı.
    expect(mocks.reviewDecide).not.toHaveBeenCalled();
  });

  it("alıntı denetimi seçilen kapsamla koşar ve metin değişince eski sonuç silinir", async () => {
    setup();
    await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 }));

    const quote = byId<HTMLTextAreaElement>("rv-qtext");
    fireEvent.change(quote, { target: { value: "kira bedeli ödenmedi" } });
    fireEvent.click(screen.getByRole("button", { name: t("review.quoteRun") }));

    await waitFor(() => expect(mocks.inTextQuoteCheck).toHaveBeenCalledTimes(1));
    expect(mocks.inTextQuoteCheck).toHaveBeenLastCalledWith("c1", {
      text: "kira bedeli ödenmedi",
      documentId: null,
    });

    const summary = quoteCheckSummary(check());
    await screen.findByText(summary);

    // Kapsam belgeye daraltılır: sonuç bayatlar, ekranda kalmaz.
    fireEvent.change(byId<HTMLSelectElement>("rv-qscope"), { target: { value: "d1" } });
    expect(screen.queryByText(summary)).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: t("review.quoteRun") }));
    await waitFor(() => expect(mocks.inTextQuoteCheck).toHaveBeenCalledTimes(2));
    expect(mocks.inTextQuoteCheck).toHaveBeenLastCalledWith("c1", {
      text: "kira bedeli ödenmedi",
      documentId: "d1",
    });
  });

  it("yalnız bağlanabilir alıntı düğme alır; sayfa sınırını aşan alıntı bağlanmaz", async () => {
    setup({
      quoteCheck: check({
        quotes: [
          finding({ index: 0, quote: "kira bedeli ödenmedi" }),
          finding({
            index: 1,
            quote: "kira bedeli",
            position: position({ spansPages: true, endPageIndex: 1, charEnd: 11 }),
          }),
        ],
      }),
    });
    await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 }));

    fireEvent.change(byId<HTMLTextAreaElement>("rv-qtext"), { target: { value: "kira bedeli ödenmedi" } });
    fireEvent.click(screen.getByRole("button", { name: t("review.quoteRun") }));

    const bindButtons = await screen.findAllByRole("button", { name: t("review.bindQuoteAction") });
    expect(bindButtons.length).toBe(1);
    // Sayfa sınırını aşan eşleşme için düğme yerine nedeni yazılır.
    expect(screen.getByText(t("intext.bindSpansPages"))).toBeTruthy();

    fireEvent.click(bindButtons[0]!);
    await waitFor(() => expect(mocks.anchorCreate).toHaveBeenCalledTimes(1));
    expect(mocks.anchorCreate).toHaveBeenCalledWith({
      caseId: "c1",
      documentVersionId: "v1",
      parseRunId: "run-1",
      segmentId: "seg-1",
      startOffset: 10,
      endOffset: 30,
      exactQuote: "kira bedeli ödenmedi",
    });
    expect(await screen.findByText(t("review.boundBadge", { id: "an-12345" }))).toBeTruthy();
  });

  it("çelişki adayı iki farklı kaynak ister; aynı kaynak seçilirse ekleme kapanır", async () => {
    setup();
    await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 }));

    fireEvent.change(byId<HTMLSelectElement>("rv-dkind"), {
      target: { value: "contradiction_candidate" },
    });
    const add = screen.getByRole("button", { name: t("review.queueAdd") }) as HTMLButtonElement;
    expect(add.disabled).toBe(true);

    fireEvent.change(byId<HTMLSelectElement>("rv-danchor"), { target: { value: "a1" } });
    fireEvent.change(byId<HTMLInputElement>("rv-dtext"), { target: { value: "Tarih çelişkisi" } });
    expect(add.disabled).toBe(true);

    fireEvent.change(byId<HTMLSelectElement>("rv-danchor2"), { target: { value: "a2" } });
    expect(add.disabled).toBe(false);

    // Aynı kaynak iki kez seçilemez: ikinci kaynak birinciye eşitlenince kapanır.
    fireEvent.change(byId<HTMLSelectElement>("rv-danchor"), { target: { value: "a2" } });
    expect(add.disabled).toBe(true);
    expect(mocks.reviewItemCreate).not.toHaveBeenCalled();

    fireEvent.change(byId<HTMLSelectElement>("rv-danchor"), { target: { value: "a1" } });
    fireEvent.click(add);
    await waitFor(() => expect(mocks.reviewItemCreate).toHaveBeenCalledTimes(1));
    expect(mocks.reviewItemCreate).toHaveBeenCalledWith("c1", {
      itemKind: "contradiction_candidate",
      payload: { text: "Tarih çelişkisi" },
      sourceAnchorIds: ["a1", "a2"],
      aiRunId: null,
    });
  });

  it("dosya değişince önceki dosyanın alıntı hükmü ve belge kapsamı temizlenir", async () => {
    setup();
    await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 }));

    fireEvent.change(byId<HTMLTextAreaElement>("rv-qtext"), {
      target: { value: "kira bedeli ödenmedi" },
    });
    fireEvent.change(byId<HTMLSelectElement>("rv-qscope"), { target: { value: "d1" } });
    fireEvent.click(screen.getByRole("button", { name: t("review.quoteRun") }));

    const summary = quoteCheckSummary(check());
    await screen.findByText(summary);
    await waitFor(() => expect(mocks.inTextQuoteCheck).toHaveBeenCalledTimes(1));
    expect(mocks.inTextQuoteCheck).toHaveBeenLastCalledWith("c1", {
      text: "kira bedeli ödenmedi",
      documentId: "d1",
    });

    // Dosya değişir: önceki dosyanın hükmü ve belge kapsamı ekranda kalmaz.
    fireEvent.change(byId<HTMLSelectElement>("rv-case"), { target: { value: "c2" } });
    await waitFor(() => expect(screen.queryByText(summary)).toBeNull());
    expect(byId<HTMLSelectElement>("rv-qscope").value).toBe("");

    // Yeni dosyada denetim eski dosyanın belgesine daraltılmış gitmez.
    fireEvent.click(screen.getByRole("button", { name: t("review.quoteRun") }));
    await waitFor(() => expect(mocks.inTextQuoteCheck).toHaveBeenCalledTimes(2));
    expect(mocks.inTextQuoteCheck).toHaveBeenLastCalledWith("c2", {
      text: "kira bedeli ödenmedi",
      documentId: null,
    });
  });

  it("geçersiz JSON düzeltmesi gönderilmez", async () => {
    setup();
    await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 }));

    const card = itemCard("Kira ödenmedi notu");
    fireEvent.click(within(card).getByRole("button", { name: t("review.editAction") }));

    const editor = within(card).getByLabelText(t("review.editJsonLabel")) as HTMLTextAreaElement;
    expect(editor.value).toContain("Kira ödenmedi notu");
    fireEvent.change(editor, { target: { value: "{bozuk" } });
    fireEvent.click(within(card).getByRole("button", { name: t("common.save") }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain(t("review.errEditJson"));
    expect(mocks.reviewDecide).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Yazma yolları ve kapsam kapıları.
//
// Yukarıdaki blok ekranın **kararlarını** ölçüyor (hedefsiz kabul yok, bayat
// alıntı hükmü yok...). Bu blok aynı ekranın **uçuştaki** davranışını ölçer:
// hangi dosyaya/iddiaya hangi yanıtın yazıldığı (INV-05), bayat hata ve
// kullanıcı girdisinin sessizce silinmesi.
// ---------------------------------------------------------------------------
describe("inceleme yazma yolları ve kapsam kapıları", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  function addButton(): HTMLButtonElement {
    return screen.getByRole("button", { name: t("review.queueAdd") }) as HTMLButtonElement;
  }

  function bindButton(): HTMLButtonElement {
    return screen.getByRole("button", { name: t("review.bindAction") }) as HTMLButtonElement;
  }

  it("geç gelen eski dosya yanıtı yeni dosyanın listesini ezmez", async () => {
    const slowClaims = deferred<ClaimRow[]>();
    setup({
      byCase: {
        c1: { claims: slowClaims.promise },
        c2: { claims: [CLAIM_C2], items: [ITEM_C2], anchors: [ANCHOR_C2], documents: [] },
      },
    });

    await waitFor(() => expect(mocks.claimList).toHaveBeenCalledWith("c1"));
    fireEvent.change(byId<HTMLSelectElement>("rv-case"), { target: { value: "c2" } });
    await screen.findByRole("button", { name: C2_CLAIM_BUTTON });
    expect(screen.getByText(t("review.claimsCount", { count: 1 }))).toBeTruthy();

    // Eski dosyanın yanıtı **sonra** gelir: yeni dosyanın listesini ezmemeli.
    await act(async () => {
      slowClaims.resolve([CLAIMS[0]!, CLAIM_B]);
    });

    expect(screen.getByText(t("review.claimsCount", { count: 1 }))).toBeTruthy();
    expect(screen.queryByText(CLAIM_B.claimText)).toBeNull();
  });

  it("düşen yükleme önceki dosyanın iddia ve adaylarını ekranda bırakmaz", async () => {
    setup({
      byCase: {
        c1: { claims: CLAIMS, items: ITEMS, anchors: ANCHORS, documents: DOCUMENTS },
        c2: {
          claims: () => Promise.reject(new Error("db down")),
          items: [],
          anchors: [],
          documents: [],
        },
      },
    });

    await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 }));
    fireEvent.change(byId<HTMLSelectElement>("rv-case"), { target: { value: "c2" } });
    await screen.findByRole("alert");

    // Yeni dosyanın verisi okunamadı: eski dosyanın listesi "yeni dosyanın
    // verisi" gibi ekranda kalamaz.
    expect(screen.getByText(t("review.claimsEmpty"))).toBeTruthy();
    expect(screen.getByText(t("review.queueEmpty"))).toBeTruthy();
    expect(screen.queryByText(t("review.claimsCount", { count: 1 }))).toBeNull();
  });

  it("iddia ekleme uçuşta dosya değişirse eski dosyanın listesi yeni dosyaya yazılmaz", async () => {
    const slowCreate = deferred<ClaimRow>();
    setup({
      byCase: {
        c1: { claims: [CLAIMS[0]!, CLAIM_B], items: [], anchors: ANCHORS, documents: DOCUMENTS },
        c2: { claims: [CLAIM_C2], items: [], anchors: [ANCHOR_C2], documents: [] },
      },
    });

    await screen.findByText(t("review.claimsCount", { count: 2 }));
    mocks.claimCreate.mockReturnValue(slowCreate.promise);
    fireEvent.change(byId<HTMLInputElement>("rv-claim"), { target: { value: "Yeni iddia" } });
    fireEvent.change(byId<HTMLSelectElement>("rv-kind"), { target: { value: "allegation" } });
    fireEvent.click(screen.getByRole("button", { name: t("review.addAction") }));
    await waitFor(() => expect(mocks.claimCreate).toHaveBeenCalledTimes(1));

    fireEvent.change(byId<HTMLSelectElement>("rv-case"), { target: { value: "c2" } });
    await screen.findByRole("button", { name: C2_CLAIM_BUTTON });

    // İşlem tamamlanınca eski dosya yeniden yüklenir; yeni dosyanın görünümü
    // bundan etkilenmemeli.
    await act(async () => {
      slowCreate.resolve(CLAIMS[0]!);
    });

    expect(screen.getByText(t("review.claimsCount", { count: 1 }))).toBeTruthy();
    expect(screen.queryByText(CLAIM_B.claimText)).toBeNull();
  });

  it("alıntı denetimi uçuşta dosya değişirse eski dosyanın hükmü yeni dosyada çizilmez", async () => {
    const slowCheck = deferred<InTextQuoteCheck>();
    setup({
      byCase: {
        c1: { claims: [CLAIMS[0]!], items: [], anchors: ANCHORS, documents: DOCUMENTS },
        c2: { claims: [CLAIM_C2], items: [], anchors: [ANCHOR_C2], documents: [] },
      },
    });

    await screen.findByText(t("review.claimsCount", { count: 1 }));
    mocks.inTextQuoteCheck.mockReturnValue(slowCheck.promise);
    fireEvent.change(byId<HTMLTextAreaElement>("rv-qtext"), {
      target: { value: "kira bedeli ödenmedi" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("review.quoteRun") }));
    await waitFor(() => expect(mocks.inTextQuoteCheck).toHaveBeenCalledTimes(1));

    fireEvent.change(byId<HTMLSelectElement>("rv-case"), { target: { value: "c2" } });
    await screen.findByRole("button", { name: C2_CLAIM_BUTTON });

    const summary = quoteCheckSummary(check());
    await act(async () => {
      slowCheck.resolve(check());
    });

    expect(screen.queryByText(summary)).toBeNull();
  });

  it("uçuşta bağlanan çapa yeni dosyanın kaynak listesine eklenmez", async () => {
    const slowBind = deferred<AnchorRow>();
    setup({
      quoteCheck: check({ quotes: [finding({ index: 0, quote: "kira bedeli ödenmedi" })] }),
      byCase: {
        c1: { claims: [CLAIMS[0]!], items: [], anchors: ANCHORS, documents: DOCUMENTS },
        c2: { claims: [CLAIM_C2], items: [], anchors: [ANCHOR_C2], documents: [] },
      },
    });

    await screen.findByText(t("review.claimsCount", { count: 1 }));
    mocks.anchorCreate.mockReturnValue(slowBind.promise);
    fireEvent.change(byId<HTMLTextAreaElement>("rv-qtext"), {
      target: { value: "kira bedeli ödenmedi" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("review.quoteRun") }));
    fireEvent.click(await screen.findByRole("button", { name: t("review.bindQuoteAction") }));
    await waitFor(() => expect(mocks.anchorCreate).toHaveBeenCalledTimes(1));

    fireEvent.change(byId<HTMLSelectElement>("rv-case"), { target: { value: "c2" } });
    await screen.findByRole("button", { name: C2_CLAIM_BUTTON });
    await act(async () => {
      slowBind.resolve(anchor("an-99999999", "İhtar alıntısı", 0));
    });

    const values = Array.from(byId<HTMLSelectElement>("rv-danchor").options).map((o) => o.value);
    expect(values).toEqual(["", "b1"]);
  });

  it("düşen alıntı bağlamadan sonra başarılı bağlama kırmızı hatayı siler", async () => {
    setup({ quoteCheck: check({ quotes: [finding({ index: 0, quote: "kira bedeli ödenmedi" })] }) });
    await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 }));

    mocks.anchorCreate.mockRejectedValueOnce(new Error("db down"));
    fireEvent.change(byId<HTMLTextAreaElement>("rv-qtext"), {
      target: { value: "kira bedeli ödenmedi" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("review.quoteRun") }));
    fireEvent.click(await screen.findByRole("button", { name: t("review.bindQuoteAction") }));
    await screen.findByRole("alert");

    fireEvent.click(screen.getByRole("button", { name: t("review.bindQuoteAction") }));

    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("iddia değişince önceki iddianın delil bağları hemen kalkar", async () => {
    const slowB = deferred<EvidenceLinkRow[]>();
    setup({
      byCase: {
        c1: { claims: [CLAIMS[0]!, CLAIM_B], items: [], anchors: ANCHORS, documents: DOCUMENTS },
      },
    });
    mocks.evidenceLinkList.mockImplementation(async (claimId: string) =>
      claimId === "cl1" ? [LINK_A] : slowB.promise,
    );

    fireEvent.click(await screen.findByRole("button", { name: /Kira bedeli ödenmedi/ }));
    await screen.findByText(/^Bağlam ·/);

    fireEvent.click(screen.getByRole("button", { name: /Ödeme dekontu sunuldu/ }));

    // Yeni iddianın bağları yüklenirken önceki iddianın bağı görünmemeli.
    expect(screen.queryByText(/^Bağlam ·/)).toBeNull();
  });

  it("geç gelen eski iddia yanıtı yeni iddianın panelini ezmez", async () => {
    const slowA = deferred<EvidenceLinkRow[]>();
    setup({
      byCase: {
        c1: { claims: [CLAIMS[0]!, CLAIM_B], items: [], anchors: ANCHORS, documents: DOCUMENTS },
      },
    });
    mocks.evidenceLinkList.mockImplementation(async (claimId: string) =>
      claimId === "cl1" ? slowA.promise : [LINK_B],
    );

    fireEvent.click(await screen.findByRole("button", { name: /Kira bedeli ödenmedi/ }));
    fireEvent.click(screen.getByRole("button", { name: /Ödeme dekontu sunuldu/ }));
    await screen.findByText(/^Sunuldu ·/);

    await act(async () => {
      slowA.resolve([LINK_A]);
    });

    expect(screen.queryByText(/^Bağlam ·/)).toBeNull();
    expect(screen.getByText(/^Sunuldu ·/)).toBeTruthy();
  });

  it("iddia bağı uçuşta iddia değişirse eski iddianın bağı yeni panelde görünmez", async () => {
    const slowCreate = deferred<EvidenceLinkRow>();
    setup({
      byCase: {
        c1: { claims: [CLAIMS[0]!, CLAIM_B], items: [], anchors: ANCHORS, documents: DOCUMENTS },
      },
    });
    let listCalls = 0;
    mocks.evidenceLinkList.mockImplementation(async (claimId: string) => {
      if (claimId !== "cl1") return [];
      listCalls += 1;
      return listCalls === 1 ? [] : [LINK_A];
    });
    mocks.evidenceLinkCreate.mockReturnValue(slowCreate.promise);

    fireEvent.click(await screen.findByRole("button", { name: /Kira bedeli ödenmedi/ }));
    await waitFor(() => expect(mocks.evidenceLinkList).toHaveBeenCalledWith("cl1"));
    fireEvent.change(byId<HTMLSelectElement>("rv-anchor"), { target: { value: "a1" } });
    fireEvent.change(byId<HTMLSelectElement>("rv-rel"), { target: { value: "context" } });
    fireEvent.click(bindButton());
    await waitFor(() => expect(mocks.evidenceLinkCreate).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole("button", { name: /Ödeme dekontu sunuldu/ }));
    await screen.findByText(t("review.evidenceEmpty"));

    await act(async () => {
      slowCreate.resolve(LINK_A);
    });

    expect(screen.queryByText(/^Bağlam ·/)).toBeNull();
    expect(screen.getByText(t("review.evidenceEmpty"))).toBeTruthy();
  });

  it("dosya değişince seçili kaynak taşınmaz: yeni dosyaya eski çapanın kimliği gönderilmez", async () => {
    setup({
      byCase: {
        c1: { claims: CLAIMS, items: [], anchors: ANCHORS, documents: DOCUMENTS },
        c2: { claims: [CLAIM_C2], items: [], anchors: [ANCHOR_C2], documents: [] },
      },
    });

    await screen.findByText(t("review.claimsCount", { count: 1 }));
    fireEvent.change(byId<HTMLSelectElement>("rv-danchor"), { target: { value: "a1" } });
    fireEvent.change(byId<HTMLInputElement>("rv-dtext"), { target: { value: "Taslak aday" } });
    expect(addButton().disabled).toBe(false);

    fireEvent.change(byId<HTMLSelectElement>("rv-case"), { target: { value: "c2" } });
    await screen.findByRole("button", { name: C2_CLAIM_BUTTON });

    // Kaynak seçimi önceki dosyanın çapasına bağlıydı: yeni dosyada seçim yok.
    expect(addButton().disabled).toBe(true);
  });

  it("dosya değişince ikinci kaynak seçimi taşınmaz", async () => {
    setup({
      byCase: {
        c1: { claims: CLAIMS, items: [], anchors: ANCHORS, documents: DOCUMENTS },
        c2: { claims: [CLAIM_C2], items: [], anchors: [ANCHOR_C2], documents: [] },
      },
    });

    await screen.findByText(t("review.claimsCount", { count: 1 }));
    fireEvent.change(byId<HTMLSelectElement>("rv-dkind"), {
      target: { value: "contradiction_candidate" },
    });
    fireEvent.change(byId<HTMLSelectElement>("rv-danchor"), { target: { value: "a1" } });
    fireEvent.change(byId<HTMLSelectElement>("rv-danchor2"), { target: { value: "a2" } });
    fireEvent.change(byId<HTMLInputElement>("rv-dtext"), { target: { value: "Çelişki adayı" } });
    expect(addButton().disabled).toBe(false);

    fireEvent.change(byId<HTMLSelectElement>("rv-case"), { target: { value: "c2" } });
    await screen.findByRole("button", { name: C2_CLAIM_BUTTON });
    fireEvent.change(byId<HTMLSelectElement>("rv-danchor"), { target: { value: "b1" } });

    // İkinci kaynak boş: bayat "a2" ile çelişki adayı yazılabilir olmamalı.
    expect(addButton().disabled).toBe(true);
  });

  it("dosya değişince kaynak alıntı seçimi taşınmaz", async () => {
    setup({
      byCase: {
        c1: { claims: [CLAIMS[0]!], items: [], anchors: ANCHORS, documents: DOCUMENTS },
        c2: { claims: [CLAIM_C2], items: [], anchors: [ANCHOR_C2], documents: [] },
      },
    });

    await screen.findByText(t("review.claimsCount", { count: 1 }));
    fireEvent.click(await screen.findByRole("button", { name: /Kira bedeli ödenmedi/ }));
    fireEvent.change(byId<HTMLSelectElement>("rv-anchor"), { target: { value: "a1" } });
    fireEvent.change(byId<HTMLSelectElement>("rv-rel"), { target: { value: "context" } });
    expect(bindButton().disabled).toBe(false);

    fireEvent.change(byId<HTMLSelectElement>("rv-case"), { target: { value: "c2" } });
    fireEvent.click(await screen.findByRole("button", { name: /İhtar bedeli ödenmedi/ }));

    // Yeni dosyanın iddiasına eski dosyanın çapası bağlanabilir olmamalı.
    expect(bindButton().disabled).toBe(true);
  });

  it("düşen iddia eklemeden sonra başarılı ekleme kırmızı hatayı siler", async () => {
    setup();
    await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 }));

    mocks.claimCreate.mockRejectedValueOnce(new Error("db down"));
    fireEvent.change(byId<HTMLInputElement>("rv-claim"), { target: { value: "Yeni iddia" } });
    fireEvent.change(byId<HTMLSelectElement>("rv-kind"), { target: { value: "allegation" } });
    fireEvent.click(screen.getByRole("button", { name: t("review.addAction") }));
    await screen.findByRole("alert");

    fireEvent.change(byId<HTMLInputElement>("rv-claim"), { target: { value: "İkinci iddia" } });
    fireEvent.change(byId<HTMLSelectElement>("rv-kind"), { target: { value: "allegation" } });
    fireEvent.click(screen.getByRole("button", { name: t("review.addAction") }));

    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("düşen delil bağından sonra başarılı bağlama kırmızı hatayı siler", async () => {
    setup({
      byCase: {
        c1: { claims: [CLAIMS[0]!], items: [], anchors: ANCHORS, documents: DOCUMENTS },
      },
    });
    mocks.evidenceLinkCreate.mockRejectedValueOnce(new Error("db down"));

    fireEvent.click(await screen.findByRole("button", { name: /Kira bedeli ödenmedi/ }));
    fireEvent.change(byId<HTMLSelectElement>("rv-anchor"), { target: { value: "a1" } });
    fireEvent.change(byId<HTMLSelectElement>("rv-rel"), { target: { value: "context" } });
    fireEvent.click(bindButton());
    await screen.findByRole("alert");

    fireEvent.click(bindButton());

    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("düşen alıntı denetiminden sonra başarılı denetim kırmızı hatayı siler", async () => {
    setup();
    await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 }));

    mocks.inTextQuoteCheck.mockRejectedValueOnce(new Error("db down"));
    fireEvent.change(byId<HTMLTextAreaElement>("rv-qtext"), {
      target: { value: "kira bedeli ödenmedi" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("review.quoteRun") }));
    await screen.findByRole("alert");

    fireEvent.click(screen.getByRole("button", { name: t("review.quoteRun") }));

    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("düşen çelişki değerlendirmesinden sonra başarılı değerlendirme kırmızı hatayı siler", async () => {
    setup();
    await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 }));

    mocks.contradictionAssess.mockRejectedValueOnce(new Error("db down"));
    fireEvent.change(byId<HTMLSelectElement>("verdict-i-contra"), { target: { value: "conflict" } });
    fireEvent.click(screen.getByRole("button", { name: t("review.assessAction") }));
    await screen.findByRole("alert");

    fireEvent.change(byId<HTMLSelectElement>("verdict-i-contra"), { target: { value: "conflict" } });
    fireEvent.click(screen.getByRole("button", { name: t("review.assessAction") }));

    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("karar uçuşta başka öğenin düzenleyicisi açılırsa kapanmaz", async () => {
    const slowDecide = deferred<ReviewItemRow>();
    setup();
    await screen.findByText(t("review.queueSummary", { pending: 3, decided: 2, outbox: 1 }));
    mocks.reviewDecide.mockReturnValue(slowDecide.promise);

    fireEvent.click(
      within(itemCard("Kira ödenmedi notu")).getByRole("button", { name: t("review.editAction") }),
    );
    fireEvent.click(
      within(itemCard("Kira ödenmedi notu")).getByRole("button", { name: t("common.save") }),
    );
    await waitFor(() => expect(mocks.reviewDecide).toHaveBeenCalledTimes(1));

    // Bu sırada başka öğenin düzenleyicisi açılır ve metin yazılır.
    fireEvent.click(
      within(itemCard("Ödeme yapıldı")).getByRole("button", { name: t("review.editAction") }),
    );
    fireEvent.change(within(itemCard("Ödeme yapıldı")).getByLabelText(t("review.editJsonLabel")), {
      target: { value: '{ "text": "elle düzeltildi" }' },
    });

    await act(async () => {
      slowDecide.resolve(ITEMS[0]!);
    });

    // Eski kararın yanıtı, yeni açılan düzenleyiciyi kapatmamalı ve metnini
    // silmemeli.
    const kept = within(itemCard("Ödeme yapıldı")).getByLabelText(
      t("review.editJsonLabel"),
    ) as HTMLTextAreaElement;
    expect(kept.value).toBe('{ "text": "elle düzeltildi" }');
  });

  it("aday ekleme uçuşta dosya değişirse yeni dosyanın taslağı silinmez", async () => {
    const slowCreate = deferred<ReviewItemRow>();
    setup({
      byCase: {
        c1: { claims: CLAIMS, items: [], anchors: ANCHORS, documents: DOCUMENTS },
        c2: { claims: [CLAIM_C2], items: [], anchors: [ANCHOR_C2], documents: [] },
      },
    });

    await screen.findByText(t("review.claimsCount", { count: 1 }));
    mocks.reviewItemCreate.mockReturnValue(slowCreate.promise);
    fireEvent.change(byId<HTMLSelectElement>("rv-danchor"), { target: { value: "a1" } });
    fireEvent.change(byId<HTMLInputElement>("rv-dtext"), { target: { value: "İlk dosyanın adayı" } });
    fireEvent.click(addButton());
    await waitFor(() => expect(mocks.reviewItemCreate).toHaveBeenCalledTimes(1));

    fireEvent.change(byId<HTMLSelectElement>("rv-case"), { target: { value: "c2" } });
    await screen.findByRole("button", { name: C2_CLAIM_BUTTON });

    await act(async () => {
      slowCreate.resolve(ITEMS[0]!);
    });

    // Yeni dosyanın taslak kutusu, önceki dosyanın isteği tamamlandı diye
    // boşaltılmaz: yazılan metin kullanıcınındır.
    expect(byId<HTMLInputElement>("rv-dtext").value).toBe("İlk dosyanın adayı");
  });

  it("aday ekleme uçuşta taslak değiştirilirse yazılan metin silinmez", async () => {
    const slowCreate = deferred<ReviewItemRow>();
    setup({ byCase: { c1: { claims: CLAIMS, items: [], anchors: ANCHORS, documents: DOCUMENTS } } });

    await screen.findByText(t("review.claimsCount", { count: 1 }));
    mocks.reviewItemCreate.mockReturnValue(slowCreate.promise);
    fireEvent.change(byId<HTMLSelectElement>("rv-danchor"), { target: { value: "a1" } });
    fireEvent.change(byId<HTMLInputElement>("rv-dtext"), { target: { value: "İlk aday" } });
    fireEvent.click(addButton());
    await waitFor(() => expect(mocks.reviewItemCreate).toHaveBeenCalledTimes(1));

    fireEvent.change(byId<HTMLInputElement>("rv-dtext"), { target: { value: "Düzeltilmiş aday" } });

    await act(async () => {
      slowCreate.resolve(ITEMS[0]!);
    });

    expect(byId<HTMLInputElement>("rv-dtext").value).toBe("Düzeltilmiş aday");
  });
});
