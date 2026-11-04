// @vitest-environment jsdom
//
// AI (Aday üretimi) ekranı davranış testi. `lib/candidate` ve `lib/transferScope`
// testleri metin parçalarını doğrular; burada 604 satırlık ekran gerçekten render
// edilip F11 kararlarının arayüzde **uygulandığı** ölçülür:
//
// 1. Dosya listesi sunucudan, ilk dosya seçilir, sağlayıcı satırı ve kapsam
//    **çekirdekten** yazılır; bilinmeyen alan uydurulmaz.
// 2. Yalnız `off` sağlayıcı varken hiçbir içerik gönderilemez (`E_AI_OFF`).
// 3. Onay **somut belge kümesine** bağlanır: onay önizlemesi ve onay kaydı aynı
//    kimlik listesiyle gider; amaç seçimi taşınır.
// 4. Onay yokken "Çalıştır" kapalıdır; çalıştırma **onay kimliğiyle** koşar
//    (belge listesi yeniden verilmez) ve özet tazelenir.
// 5. Önizleme: kapsam dışı belgeler sayılır, boş örnek dürüstçe yazılır.
// 6. **Ölçülen kusur:** önizleme isteği düşünce ekran sessiz kalıyordu.
// 7. Onay iptali çekirdeğe gider ve onay satırı kalkar.
// 8. **Ölçülen kusur:** düşen çalıştırma seçimi önceki çalıştırmanın adaylarını
//    ekranda bırakıyordu.
// 9. Doğrulama kapıları: çalıştırma seçilmeden ve JSON bozukken iki düğme de kapalı.
// 10. "Doğrula" yalnız doğrular (kaydetmez), "Doğrula ve kaydet" kaydeder ve
//     adayları tazeler; bildirim **hesaplanan** duruma göre yazılır.
// 11. **Ölçülen kusur:** düşen doğrulama önceki raporu ekranda bırakıyordu.
// 12. Aday türü değişimi raporu ve taslağı tazeler.
// 13. Rapor: alıntı tablosu, model sayfa uyuşmazlığı, sorunlar, notlar ve sınırlar.
// 14. Aday tablosu: şema/doğrulama/alıntı rozetleri, engel gerekçesi düğmeyi
//     kapatır, yeniden doğrulama satırı **yerinde** tazeler.
// 15. İncelemeye gönderim dosya kimliğiyle gider.
// 16. **Ölçülen kusur (INV-05):** dosya değişince önceki dosyanın onayı,
//     önizlemesi, adayları, çalıştırma seçimi ve raporu ekranda kalıyordu.

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AiCandidateRow,
  AiOverview,
  CandidateReport,
  CaseRow,
  ConsentPreview,
  DocumentRow,
  QuoteMatchReport,
  TransferConsentRow,
} from "../lib/api";
import { formatDateTime, formatSize } from "../lib/format";
import { t } from "../lib/i18n";
import { consentScopeLine, skippedSummary } from "../lib/transferScope";
import {
  CANDIDATE_KINDS,
  candidateStateLabel,
  draftJsonError,
  promoteBlockedReason,
  quoteLineStateLabel,
  quotePositionLabel,
  quoteStateLabel,
  quoteSummary,
} from "../lib/candidate";

const mocks = vi.hoisted(() => ({
  caseList: vi.fn(),
  aiOverview: vi.fn(),
  documentList: vi.fn(),
  aiConsentPreview: vi.fn(),
  aiConsentCreate: vi.fn(),
  aiConsentRevoke: vi.fn(),
  aiRunExecute: vi.fn(),
  aiCandidatesList: vi.fn(),
  aiCandidateValidate: vi.fn(),
  aiCandidateRecord: vi.fn(),
  aiCandidateRevalidate: vi.fn(),
  aiCandidatePromote: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

const { Ai } = await import("./Ai");

/* ------------------------------------------------------------------ fixtures */

function caseRow(overrides: Partial<CaseRow> & { id: string; title: string }): CaseRow {
  return {
    caseKind: "review",
    referenceCode: null,
    jurisdictionLabel: null,
    practiceArea: null,
    status: "active",
    sensitivity: "normal",
    defaultAiPolicy: "api_allowed",
    archivedAt: null,
    retentionHold: false,
    createdAt: "2026-10-01T08:00:00.000Z",
    updatedAt: "2026-10-01T08:00:00.000Z",
    rowVersion: 1,
    ...overrides,
  };
}

const P1 = caseRow({ id: "p1", title: "Kira uyuşmazlığı" });
const P2 = caseRow({ id: "p2", title: "Ceza dosyası" });

function documentRow(id: string, caseId: string, displayName: string): DocumentRow {
  return {
    id,
    caseId,
    displayName,
    documentType: "sozlesme",
    status: "active",
    currentVersionId: `${id}-v1`,
    declaredDocumentDate: null,
    createdAt: "2026-10-02T09:00:00.000Z",
    updatedAt: "2026-10-02T09:00:00.000Z",
    rowVersion: 1,
  };
}

/** API sağlayıcısı bağlı, ölçülebilir kapsam. */
const OVERVIEW_A: AiOverview = {
  providers: [
    {
      id: "prov-a",
      providerType: "api",
      adapterId: "api:claude",
      executionEnvironment: "local_process",
      policyVersion: "1.4.0",
    },
    {
      id: "prov-off",
      providerType: "off",
      adapterId: "off",
      executionEnvironment: "none",
      policyVersion: "0",
    },
  ],
  runs: [
    {
      id: "r1",
      caseId: "p1",
      taskType: "summarize",
      providerId: "prov-a",
      modelSnapshot: "claude-sonnet-4-5",
      status: "succeeded",
      usageReport: "{}",
      createdAt: "2026-10-20T11:30:00.000Z",
      completedAt: "2026-10-20T11:31:00.000Z",
    },
    {
      id: "r2",
      caseId: "p1",
      taskType: "extract_events",
      providerId: "prov-a",
      modelSnapshot: "claude-sonnet-4-5",
      status: "failed",
      usageReport: "{}",
      createdAt: "2026-10-21T15:05:00.000Z",
      completedAt: null,
    },
  ],
  coverage: {
    documents: 4,
    versions: 6,
    parseRuns: 5,
    pages: 132,
    ocrPages: 9,
    segments: 480,
    emptyPages: 2,
    unparsedDocuments: 1,
    failedParseRuns: 0,
  },
};

/** Yalnız `off`: hiçbir içerik gönderilemez. */
const OVERVIEW_OFF: AiOverview = {
  providers: [
    {
      id: "prov-off",
      providerType: "off",
      adapterId: "off",
      executionEnvironment: "none",
      policyVersion: "0",
    },
  ],
  runs: [],
  coverage: {
    documents: 0,
    versions: 0,
    parseRuns: 0,
    pages: 0,
    ocrPages: 0,
    segments: 0,
    emptyPages: 0,
    unparsedDocuments: 0,
    failedParseRuns: 0,
  },
};

/** İkinci dosya: yerel model bağlı, başka kapsam. */
const OVERVIEW_B: AiOverview = {
  providers: [
    {
      id: "prov-local",
      providerType: "local",
      adapterId: "local:ollama",
      executionEnvironment: "local_daemon",
      policyVersion: "2.0.0",
    },
  ],
  runs: [],
  coverage: {
    documents: 1,
    versions: 1,
    parseRuns: 1,
    pages: 7,
    ocrPages: 0,
    segments: 21,
    emptyPages: 0,
    unparsedDocuments: 0,
    failedParseRuns: 0,
  },
};

const PREVIEW: ConsentPreview = {
  documents: 2,
  segments: 37,
  bytes: 20_480,
  sample: "MADDE 1 — Taraflar arasındaki kira sözleşmesi…",
  skipped: [
    { documentId: "d3", reason: "not_parsed" },
    { documentId: "d4", reason: "empty_text" },
  ],
};

const PREVIEW_EMPTY: ConsentPreview = {
  documents: 1,
  segments: 0,
  bytes: 0,
  sample: "",
  skipped: [],
};

const CONSENT: TransferConsentRow = {
  id: "c-123456789",
  providerSnapshot: "api:claude@1.4.0",
  caseScope: "p1",
  purpose: "ozet",
  selectedDocumentIds: ["d1", "d2"],
  expiresAt: "2026-10-22T10:00:00.000Z",
  consentTextVersion: "2",
  approvedAt: "2026-10-21T10:00:00.000Z",
  revokedAt: null,
};

function quote(overrides: Partial<QuoteMatchReport> = {}): QuoteMatchReport {
  return {
    index: 0,
    documentVersionId: "d1-v1",
    quote: "Taraflar arasındaki kira sözleşmesi 12 ay sürelidir.",
    label: null,
    state: "exact",
    reason: null,
    claimedPageIndex: null,
    pageIndex: 2,
    charStart: 420,
    occurrences: 1,
    candidates: [],
    textOrigin: "text",
    parseRunId: "pr1",
    segmentId: "seg1",
    ...overrides,
  };
}

const REPORT_VALID: CandidateReport = {
  kind: "claim_candidate",
  state: "valid",
  schemaState: "valid",
  quoteState: "verified",
  issues: [],
  quotes: [
    quote(),
    quote({
      index: 1,
      quote: "Ödeme yapılmadığı sabit olup temerrüt oluşmuştur.",
      pageIndex: 3,
      claimedPageIndex: 1,
      occurrences: 2,
      state: "normalized",
      reason: "whitespace",
      textOrigin: "ocr",
    }),
  ],
  notes: ["Alıntı 2, boşluk katlamasıyla eşleşti."],
  checkedAt: "2026-10-21T12:00:00.000Z",
};

const REPORT_INVALID: CandidateReport = {
  kind: "claim_candidate",
  state: "invalid",
  schemaState: "invalid",
  quoteState: "failed",
  issues: [
    { code: "E_CAND_SCHEMA", severity: "error", field: "text", detail: "metin boş olamaz" },
    { code: "E_CAND_QUOTE", severity: "warning", field: "quotes[0]", detail: "alıntı bulunamadı" },
  ],
  quotes: [quote({ state: "not_found", pageIndex: null, charStart: null, occurrences: 0 })],
  notes: [],
  checkedAt: "2026-10-21T12:10:00.000Z",
};

function candidateRow(overrides: Partial<AiCandidateRow> = {}): AiCandidateRow {
  return {
    id: "cand-1",
    aiRunId: "r1",
    candidateType: "claim_candidate",
    structuredPayload: '{"schemaVersion":1,"text":"Kira bedeli ödenmedi"}',
    validationState: "valid",
    validationErrors: "[]",
    schemaState: "valid",
    quoteState: "verified",
    quoteReport: [quote()],
    checkedAt: "2026-10-21T12:00:00.000Z",
    ...overrides,
  };
}

const CAND_VALID = candidateRow();
const CAND_BLOCKED = candidateRow({
  id: "cand-2",
  validationState: "invalid",
  schemaState: "invalid",
  quoteState: "failed",
  structuredPayload: '{"schemaVersion":1,"text":"Eksik iddia"}',
  validationErrors: JSON.stringify([
    { code: "E_CAND_SCHEMA", severity: "error", field: "text", detail: "metin boş olamaz" },
  ]),
  quoteReport: [quote({ state: "not_found", pageIndex: null, charStart: null, occurrences: 0 })],
  checkedAt: null,
});

/* -------------------------------------------------------------------- helpers */

type SetupOptions = {
  cases?: CaseRow[];
  overview?: (caseId: string) => Promise<AiOverview>;
  documents?: (caseId: string) => Promise<DocumentRow[]>;
  preview?: () => Promise<ConsentPreview>;
  consent?: () => Promise<TransferConsentRow>;
  revoke?: () => Promise<void>;
  runExecute?: () => Promise<void>;
  candidates?: (runId: string) => Promise<AiCandidateRow[]>;
  validate?: () => Promise<CandidateReport>;
  record?: () => Promise<unknown>;
  revalidate?: (candidateId: string) => Promise<AiCandidateRow>;
  promote?: () => Promise<unknown>;
};

function setup(options: SetupOptions = {}) {
  mocks.caseList.mockImplementation(options.cases === undefined ? async () => [P1, P2] : async () => options.cases!);
  mocks.aiOverview.mockImplementation(
    options.overview ?? (async (caseId: string) => (caseId === "p2" ? OVERVIEW_B : OVERVIEW_A)),
  );
  mocks.documentList.mockImplementation(
    options.documents ??
      (async (caseId: string) => [
        documentRow("d1", caseId, "Kira sözleşmesi.pdf"),
        documentRow("d2", caseId, "Ödeme makbuzu.pdf"),
      ]),
  );
  mocks.aiConsentPreview.mockImplementation(options.preview ?? (async () => PREVIEW));
  mocks.aiConsentCreate.mockImplementation(options.consent ?? (async () => CONSENT));
  mocks.aiConsentRevoke.mockImplementation(options.revoke ?? (async () => undefined));
  mocks.aiRunExecute.mockImplementation(options.runExecute ?? (async () => undefined));
  mocks.aiCandidatesList.mockImplementation(options.candidates ?? (async () => [CAND_VALID]));
  mocks.aiCandidateValidate.mockImplementation(options.validate ?? (async () => REPORT_VALID));
  mocks.aiCandidateRecord.mockImplementation(options.record ?? (async () => ({ id: "cand-1" })));
  mocks.aiCandidateRevalidate.mockImplementation(
    options.revalidate ?? (async (candidateId: string) => candidateRow({ id: candidateId })),
  );
  mocks.aiCandidatePromote.mockImplementation(options.promote ?? (async () => ({ id: "rv-1" })));
  render(<Ai />);
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** Öğe metni eşleştirici: `<strong>`/`<code>` içeren satırlarda tam metin. */
function byTextContent(value: string) {
  return (_content: string, element: Element | null): boolean => {
    if (element === null) return false;
    return (element.textContent ?? "").replace(/\s+/g, " ").trim() === value;
  };
}

function button(name: string): HTMLButtonElement {
  return screen.getByRole("button", { name }) as HTMLButtonElement;
}

function cardByHeading(title: string): HTMLElement {
  const heading = screen.getByRole("heading", { level: 2, name: title });
  const card = heading.closest(".card");
  if (!(card instanceof HTMLElement)) throw new Error(`${title} kartı yok`);
  return card;
}

function caseSelect(): HTMLSelectElement {
  const select = document.getElementById("ai-case");
  if (!(select instanceof HTMLSelectElement)) throw new Error("dosya seçici yok");
  return select;
}

function kindSelect(): HTMLSelectElement {
  const select = document.getElementById("ai-kind");
  if (!(select instanceof HTMLSelectElement)) throw new Error("aday türü seçici yok");
  return select;
}

function purposeSelect(): HTMLSelectElement {
  const select = document.getElementById("ai-purpose");
  if (!(select instanceof HTMLSelectElement)) throw new Error("amaç seçici yok");
  return select;
}

function draftBox(): HTMLTextAreaElement {
  const box = document.getElementById("ai-draft");
  if (!(box instanceof HTMLTextAreaElement)) throw new Error("taslak alanı yok");
  return box;
}

const CLAIM_TEMPLATE = CANDIDATE_KINDS.find((option) => option.value === "claim_candidate")!.template;
const RUN_LABEL_SELECT = t("ai.selectRun");
const RUN_LABEL_SELECTED = t("ai.selectedRun");

/** Çalıştırma tablosunun veri satırları (başlık hariç). */
function runRows(): HTMLElement[] {
  return within(cardByHeading(t("ai.runsTitle"))).getAllByRole("row").slice(1);
}

function switchCase(caseId: string) {
  fireEvent.change(caseSelect(), { target: { value: caseId } });
}

/** Özet satırı: künyenin hangi dosyaya ait olduğunu **metinden** okur. */
function coverageLine(overview: AiOverview): string {
  return t("ai.coverageLine", {
    documents: overview.coverage.documents,
    pages: overview.coverage.pages,
    ocrPages: overview.coverage.ocrPages,
    emptyPages: overview.coverage.emptyPages,
    unparsed: overview.coverage.unparsedDocuments,
  });
}

const CONSENT_NEW: TransferConsentRow = {
  ...CONSENT,
  id: "c-987654321",
  selectedDocumentIds: ["d9"],
};

beforeEach(() => {
  vi.resetAllMocks();
});

afterEach(() => {
  cleanup();
});

describe("AI ekranı — sağlayıcı, kapsam ve aktarım onayı", () => {
  it("dosya listesini sunucudan alır, ilk dosyayı seçer ve künyeyi çekirdekten yazar", async () => {
    setup();

    await waitFor(() => expect(caseSelect().value).toBe("p1"));
    expect(mocks.caseList).toHaveBeenCalledWith(false);
    expect(mocks.aiOverview).toHaveBeenCalledWith("p1");
    expect(caseSelect().options).toHaveLength(2);

    const providerRow = OVERVIEW_A.providers[0]!;
    expect(
      await screen.findByText(`${providerRow.adapterId} (${providerRow.providerType})`),
    ).toBeTruthy();

    expect(
      screen.getByText(
        t("ai.coverageLine", {
          documents: 4,
          pages: 132,
          ocrPages: 9,
          emptyPages: 2,
          unparsed: 1,
        }),
      ),
    ).toBeTruthy();

    // Amaç varsayılanı ve seçenekleri
    expect(purposeSelect().value).toBe("ozet");
    expect(within(purposeSelect()).getByRole("option", { name: t("ai.purposeClaims") })).toBeTruthy();
  });

  it("yalnız off sağlayıcı varken içerik gönderilemez", async () => {
    setup({ cases: [P1], overview: async () => OVERVIEW_OFF });

    await screen.findByText(t("ai.offTitle"));
    expect(screen.getByText("E_AI_OFF")).toBeTruthy();
    expect(screen.getByText(t("ai.providerNone"))).toBeTruthy();
    expect(button(t("ai.approve")).disabled).toBe(true);
    expect(button(t("ai.run")).disabled).toBe(true);
  });

  it("özet gelmeden kapsam uydurulmaz", async () => {
    const pending = deferred<AiOverview>();
    setup({ cases: [P1], overview: () => pending.promise });

    await waitFor(() => expect(mocks.aiOverview).toHaveBeenCalledWith("p1"));
    expect(screen.getByText(t("common.unknown"))).toBeTruthy();
    // Sağlayıcı bilinmiyorken "AI kapalı" uyarısı da yazılmaz.
    expect(screen.queryByText(t("ai.offTitle"))).toBeNull();
    expect(button(t("ai.approve")).disabled).toBe(true);

    pending.resolve(OVERVIEW_A);
    await screen.findByText("api:claude (api)");
  });

  it("onayı somut belge kümesine bağlar ve amaç seçimini taşır", async () => {
    setup();
    await screen.findByText("api:claude (api)");

    fireEvent.change(purposeSelect(), { target: { value: "iddia_cikarimi" } });
    fireEvent.click(button(t("ai.approve")));

    await waitFor(() => expect(mocks.aiConsentCreate).toHaveBeenCalledTimes(1));
    expect(mocks.documentList).toHaveBeenCalledWith("p1");
    expect(mocks.aiConsentPreview).toHaveBeenCalledWith("p1", ["d1", "d2"]);
    expect(mocks.aiConsentCreate).toHaveBeenCalledWith(
      "p1",
      "api:claude@1.4.0",
      "iddia_cikarimi",
      ["d1", "d2"],
    );

    // Onay satırı: kapsam **ve** onay kimliği birlikte yazılır.
    const revoke = await screen.findByRole("button", { name: t("ai.revoke") });
    const line = revoke.closest("p");
    if (!(line instanceof HTMLParagraphElement)) throw new Error("onay satırı yok");
    expect(line.textContent).toContain(consentScopeLine(2, PREVIEW));
    expect(line.textContent).toContain(
      t("ai.consentTail", { id: CONSENT.id.slice(0, 8), provider: CONSENT.providerSnapshot }),
    );
  });

  it("onay yokken çalıştır kapalıdır; çalıştırma onay kimliğiyle koşar ve özeti tazeler", async () => {
    let overviewCalls = 0;
    setup({
      cases: [P1],
      overview: async () => {
        overviewCalls += 1;
        return OVERVIEW_A;
      },
    });
    await screen.findByText("api:claude (api)");

    expect(button(t("ai.run")).disabled).toBe(true);
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));

    fireEvent.click(button(t("ai.run")));
    await waitFor(() => expect(mocks.aiRunExecute).toHaveBeenCalledTimes(1));
    // Gönderim kapsamı onaydan çözülür: belge listesi burada verilmez.
    expect(mocks.aiRunExecute).toHaveBeenCalledWith("p1", "prov-a", CONSENT.id, "summarize");
    expect(mocks.documentList).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(overviewCalls).toBe(2));
  });

  it("önizleme kapsam dışı belgeleri ve boş örneği dürüstçe yazar", async () => {
    setup();
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.showContent")));
    await screen.findByText(t("ai.previewTitle"));
    expect(mocks.aiConsentPreview).toHaveBeenCalledWith("p1", ["d1", "d2"]);
    expect(
      screen.getByText(
        t("ai.previewSub", {
          documents: 2,
          segments: 37,
          bytes: formatSize(20_480),
        }),
      ),
    ).toBeTruthy();
    expect(
      screen.getByText(byTextContent(`${skippedSummary(PREVIEW.skipped)} ${t("ai.skippedSuffix")}`)),
    ).toBeTruthy();
    expect(screen.getByText(PREVIEW.sample)).toBeTruthy();

    cleanup();
    setup({ preview: async () => PREVIEW_EMPTY });
    await screen.findByText("api:claude (api)");
    fireEvent.click(button(t("ai.showContent")));
    // Örnek yoksa metin uydurulmaz.
    expect(await screen.findByText(t("ai.previewEmpty"))).toBeTruthy();
    expect(screen.queryByText(byTextContent(`${skippedSummary([])} ${t("ai.skippedSuffix")}`))).toBeNull();
  });

  it("KUSUR: önizleme isteği düşerse ekran sessiz kalmaz", async () => {
    let fail = false;
    setup({
      documents: async (caseId: string) => {
        if (fail) throw new Error("kasa kilitli");
        return [documentRow("d1", caseId, "Kira sözleşmesi.pdf")];
      },
      preview: async () => PREVIEW,
    });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.showContent")));
    await screen.findByText(t("ai.previewTitle"));

    fail = true;
    fireEvent.click(button(t("ai.showContent")));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_UI_UNKNOWN");
    // Eski önizleme "güncel" gibi ekranda kalmaz.
    await waitFor(() => expect(screen.queryByText(t("ai.previewTitle"))).toBeNull());
  });

  it("onay iptali çekirdeğe gider ve onay satırını kaldırır", async () => {
    setup();
    await screen.findByText("api:claude (api)");
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));

    fireEvent.click(button(t("ai.revoke")));
    await waitFor(() => expect(mocks.aiConsentRevoke).toHaveBeenCalledWith(CONSENT.id));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(true));
    expect(screen.queryByRole("button", { name: t("ai.revoke") })).toBeNull();
  });
});

describe("AI ekranı — çalıştırma seçimi ve aday doğrulama", () => {
  it("çalıştırma seçimi adayları o çalıştırmadan ister ve düğme durumunu değiştirir", async () => {
    setup();
    await screen.findByText("api:claude (api)");
    const runs = cardByHeading(t("ai.runsTitle"));

    const runRows = within(runs).getAllByRole("row").slice(1);
    expect(runRows).toHaveLength(2);
    expect(within(runRows[0]!).getByText(formatDateTime(OVERVIEW_A.runs[0]!.createdAt))).toBeTruthy();

    fireEvent.click(within(runRows[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    await waitFor(() => expect(mocks.aiCandidatesList).toHaveBeenCalledWith("r1"));
    expect(await within(runs).findByRole("button", { name: RUN_LABEL_SELECTED })).toBeTruthy();
    expect(screen.getByText(candidateStateLabel("valid"))).toBeTruthy();
  });

  it("KUSUR: düşen çalıştırma seçimi önceki çalıştırmanın adaylarını bırakmaz", async () => {
    let failFor: string | null = null;
    setup({
      candidates: async (runId: string) => {
        if (runId === failFor) throw new Error("aday listesi okunamadı");
        return [CAND_VALID];
      },
    });
    await screen.findByText("api:claude (api)");
    const runs = cardByHeading(t("ai.runsTitle"));

    fireEvent.click(within(runs).getAllByRole("button", { name: RUN_LABEL_SELECT })[0]!);
    await screen.findByRole("heading", { level: 2, name: t("ai.candidatesTitle") });

    // İkinci satır hâlâ "Seç" sunar; ona geçiş düşecek.
    failFor = "r2";
    fireEvent.click(within(runs).getAllByRole("button", { name: RUN_LABEL_SELECT })[0]!);

    await screen.findByRole("alert");
    await waitFor(() =>
      expect(screen.queryByRole("heading", { level: 2, name: t("ai.candidatesTitle") })).toBeNull(),
    );
  });

  it("doğrulama kapıları: çalıştırma seçilmeden ve JSON bozukken kapalı", async () => {
    setup();
    await screen.findByText("api:claude (api)");

    expect(screen.getByText(t("ai.noRun"))).toBeTruthy();
    expect(button(t("ai.validate")).disabled).toBe(true);
    expect(button(t("ai.validateAndSave")).disabled).toBe(true);

    fireEvent.click(within(cardByHeading(t("ai.runsTitle"))).getAllByRole("button", { name: RUN_LABEL_SELECT })[0]!);
    await waitFor(() => expect(button(t("ai.validate")).disabled).toBe(false));

    fireEvent.change(draftBox(), { target: { value: "{ bozuk" } });
    expect(screen.getByText(draftJsonError("{ bozuk")!)).toBeTruthy();
    expect(button(t("ai.validate")).disabled).toBe(true);
    expect(button(t("ai.validateAndSave")).disabled).toBe(true);
    expect(mocks.aiCandidateValidate).not.toHaveBeenCalled();
  });

  it("«Doğrula» yalnız doğrular, kaydetmez", async () => {
    setup();
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(cardByHeading(t("ai.runsTitle"))).getAllByRole("button", { name: RUN_LABEL_SELECT })[0]!);
    await waitFor(() => expect(button(t("ai.validate")).disabled).toBe(false));

    fireEvent.click(button(t("ai.validate")));

    await waitFor(() => expect(mocks.aiCandidateValidate).toHaveBeenCalledWith("r1", "claim_candidate", CLAIM_TEMPLATE));
    expect(mocks.aiCandidateRecord).not.toHaveBeenCalled();
    const reportCard = cardByHeading(t("ai.validateTitle"));
    expect(await within(reportCard).findByText(quoteSummary(REPORT_VALID))).toBeTruthy();
    expect(within(reportCard).getByText(candidateStateLabel("valid"))).toBeTruthy();
  });

  it("«Doğrula ve kaydet» kaydeder, adayları tazeler ve duruma göre bildirir", async () => {
    setup({ validate: async () => REPORT_INVALID });
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(cardByHeading(t("ai.runsTitle"))).getAllByRole("button", { name: RUN_LABEL_SELECT })[0]!);
    await waitFor(() => expect(button(t("ai.validateAndSave")).disabled).toBe(false));
    const before = mocks.aiCandidatesList.mock.calls.length;

    fireEvent.click(button(t("ai.validateAndSave")));

    await waitFor(() => expect(mocks.aiCandidateRecord).toHaveBeenCalledTimes(1));
    expect(mocks.aiCandidateRecord).toHaveBeenCalledWith("r1", "claim_candidate", CLAIM_TEMPLATE);
    await waitFor(() => expect(mocks.aiCandidatesList.mock.calls.length).toBe(before + 1));
    expect(await screen.findByText(t("ai.noticeInvalid"))).toBeTruthy();
    // Hata bildirimi geçerli sayılmaz: durum rozeti "geçersiz".
    expect(screen.getByText(candidateStateLabel("invalid"))).toBeTruthy();
  });

  it("KUSUR: düşen doğrulama önceki raporu bırakmaz", async () => {
    let fail = false;
    setup({
      validate: async () => {
        if (fail) throw new Error("doğrulama çöktü");
        return REPORT_VALID;
      },
    });
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(cardByHeading(t("ai.runsTitle"))).getAllByRole("button", { name: RUN_LABEL_SELECT })[0]!);
    await waitFor(() => expect(button(t("ai.validate")).disabled).toBe(false));

    fireEvent.click(button(t("ai.validate")));
    await screen.findByText(quoteSummary(REPORT_VALID));

    fail = true;
    fireEvent.click(button(t("ai.validate")));

    await screen.findByRole("alert");
    await waitFor(() => expect(screen.queryByText(quoteSummary(REPORT_VALID))).toBeNull());
  });

  it("aday türü değişimi raporu ve taslağı tazeler", async () => {
    setup();
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(cardByHeading(t("ai.runsTitle"))).getAllByRole("button", { name: RUN_LABEL_SELECT })[0]!);
    await waitFor(() => expect(button(t("ai.validate")).disabled).toBe(false));
    fireEvent.click(button(t("ai.validate")));
    await screen.findByText(quoteSummary(REPORT_VALID));

    const eventOption = CANDIDATE_KINDS.find((option) => option.value === "event_candidate")!;
    fireEvent.change(kindSelect(), { target: { value: "event_candidate" } });

    expect(draftBox().value).toBe(eventOption.template);
    expect(screen.getByText(eventOption.hint)).toBeTruthy();
    expect(screen.queryByText(quoteSummary(REPORT_VALID))).toBeNull();
  });

  it("rapor alıntı tablosunu, model sayfa uyuşmazlığını, sorunları ve notları yazar", async () => {
    setup({ validate: async () => REPORT_INVALID });
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(cardByHeading(t("ai.runsTitle"))).getAllByRole("button", { name: RUN_LABEL_SELECT })[0]!);
    await waitFor(() => expect(button(t("ai.validate")).disabled).toBe(false));
    fireEvent.click(button(t("ai.validate")));

    const invalidQuote = REPORT_INVALID.quotes[0]!;
    expect(await screen.findByText(quoteLineStateLabel(invalidQuote))).toBeTruthy();
    expect(screen.getByText(quotePositionLabel(invalidQuote))).toBeTruthy();
    expect(screen.getByText(t("ai.issuesSummary", { count: 2 }))).toBeTruthy();
    expect(screen.getByText(t("ai.errorBadge", { count: 1 }))).toBeTruthy();
    expect(screen.getByText(t("ai.warningBadge", { count: 1 }))).toBeTruthy();
    expect(screen.getByText(t("ai.limits"))).toBeTruthy();
    // Not_found alıntının konumu yoktur: "konum yok" yazılır, sayfa uydurulmaz.
    expect(screen.getByText(quotePositionLabel(invalidQuote))).toBeTruthy();
  });

  it("rapor notlarını ve eşleşme satırını gösterir; model sayfası uyuşmuyorsa uyarır", async () => {
    setup({ validate: async () => REPORT_VALID });
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(cardByHeading(t("ai.runsTitle"))).getAllByRole("button", { name: RUN_LABEL_SELECT })[0]!);
    await waitFor(() => expect(button(t("ai.validate")).disabled).toBe(false));
    fireEvent.click(button(t("ai.validate")));

    const second = REPORT_VALID.quotes[1]!;
    expect(await screen.findByText(quoteLineStateLabel(second))).toBeTruthy();
    expect(screen.getByText(quotePositionLabel(second))).toBeTruthy();
    expect(screen.getByText(t("ai.claimedPage", { page: 2 }))).toBeTruthy();
    for (const note of REPORT_VALID.notes) {
      expect(screen.getByText(note)).toBeTruthy();
    }
  });
});

describe("AI ekranı — aday tablosu ve incelemeye gönderim", () => {
  it("engel gerekçesi düğmeyi kapatır, yeniden doğrulama satırı yerinde tazeler", async () => {
    setup({ candidates: async () => [CAND_VALID, CAND_BLOCKED] });
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(cardByHeading(t("ai.runsTitle"))).getAllByRole("button", { name: RUN_LABEL_SELECT })[0]!);
    const table = await screen.findByRole("heading", { level: 2, name: t("ai.candidatesTitle") });

    const rows = within(table.closest(".card") as HTMLElement).getAllByRole("row").slice(1);
    expect(rows).toHaveLength(2);

    const blockedRow = rows[1]!;
    const blockedReason = promoteBlockedReason(CAND_BLOCKED)!;
    expect(within(blockedRow).getByText(blockedReason)).toBeTruthy();
    expect((within(blockedRow).getByRole("button", { name: t("ai.promote") }) as HTMLButtonElement).disabled).toBe(true);
    expect(within(blockedRow).getByText(t("ai.neverChecked"))).toBeTruthy();
    expect(within(blockedRow).getByText(candidateStateLabel("invalid"))).toBeTruthy();
    expect(within(blockedRow).getByText(quoteStateLabel("failed"))).toBeTruthy();

    const validRow = rows[0]!;
    expect(within(validRow).getByText(formatDateTime(CAND_VALID.checkedAt))).toBeTruthy();
    expect((within(validRow).getByRole("button", { name: t("ai.promote") }) as HTMLButtonElement).disabled).toBe(false);

    fireEvent.click(within(validRow).getByRole("button", { name: t("ai.revalidate") }));
    await waitFor(() => expect(mocks.aiCandidateRevalidate).toHaveBeenCalledWith("cand-1"));
    // Satır sayısı değişmez: tazeleme **yerinde** yapılır.
    expect(
      within(cardByHeading(t("ai.candidatesTitle"))).getAllByRole("row").slice(1),
    ).toHaveLength(2);
  });

  it("incelemeye gönderim dosya kimliğiyle gider ve bildirim yazar", async () => {
    setup();
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(cardByHeading(t("ai.runsTitle"))).getAllByRole("button", { name: RUN_LABEL_SELECT })[0]!);
    const card = await screen.findByRole("heading", { level: 2, name: t("ai.candidatesTitle") });

    fireEvent.click(within(card.closest(".card") as HTMLElement).getByRole("button", { name: t("ai.promote") }));

    await waitFor(() => expect(mocks.aiCandidatePromote).toHaveBeenCalledWith("cand-1", "p1"));
    expect(await screen.findByText(t("ai.noticePromoted"))).toBeTruthy();
  });
});

describe("AI ekranı — dosya değişimi", () => {
  it("KUSUR (INV-05): dosya değişince önceki dosyanın onayı, önizlemesi, adayları ve raporu kalmaz", async () => {
    const pending = deferred<AiOverview>();
    let overviewFor = (_caseId: string): Promise<AiOverview> => Promise.resolve(OVERVIEW_A);
    setup({
      overview: (caseId: string) => overviewFor(caseId),
    });
    await screen.findByText("api:claude (api)");

    // p1 üzerinde tam akış: onay + önizleme + çalıştırma seçimi + rapor
    fireEvent.click(button(t("ai.showContent")));
    await screen.findByText(t("ai.previewTitle"));
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));
    fireEvent.click(within(cardByHeading(t("ai.runsTitle"))).getAllByRole("button", { name: RUN_LABEL_SELECT })[0]!);
    await screen.findByRole("heading", { level: 2, name: t("ai.candidatesTitle") });
    fireEvent.click(button(t("ai.validate")));
    await screen.findByText(quoteSummary(REPORT_VALID));

    // p2'ye geç: özet isteği askıda kalsın (yeni veri gelene kadar pencere)
    overviewFor = (caseId: string) => (caseId === "p2" ? pending.promise : Promise.resolve(OVERVIEW_A));
    fireEvent.change(caseSelect(), { target: { value: "p2" } });
    await waitFor(() => expect(mocks.aiOverview).toHaveBeenCalledWith("p2"));

    // Önceki dosyanın türetilmiş durumu ekranda kalmamalı.
    await waitFor(() => expect(screen.queryByText(t("ai.previewTitle"))).toBeNull());
    expect(screen.queryByRole("button", { name: t("ai.revoke") })).toBeNull();
    expect(screen.queryByRole("heading", { level: 2, name: t("ai.candidatesTitle") })).toBeNull();
    expect(screen.queryByText(quoteSummary(REPORT_VALID))).toBeNull();
    expect(screen.queryByText(t("ai.noticeValid"))).toBeNull();
    expect(screen.getByText(t("ai.runsEmpty"))).toBeTruthy();
    expect(screen.queryByRole("button", { name: RUN_LABEL_SELECTED })).toBeNull();
    // Onay p1'e aitti: p2 için çalıştırma **kapalı**, eski onayla koşmaz.
    expect(button(t("ai.run")).disabled).toBe(true);
    expect(button(t("ai.approve")).disabled).toBe(true);
    fireEvent.click(button(t("ai.run")));
    expect(mocks.aiRunExecute).not.toHaveBeenCalled();

    pending.resolve(OVERVIEW_B);
    await screen.findByText("local:ollama (local)");
    expect(button(t("ai.run")).disabled).toBe(true);
  });
});

// AI ekranının yazma/okuma yolları **bağlam** taşımıyordu: yanıtlar geldiğinde
// hangi dosya/çalıştırma için istendiğine bakılmıyordu. Aşağıdaki testler her
// yolu askıda tutup dosyayı/çalıştırmayı değiştirir ve yanıtı **sonra** çözer:
// eski yanıt yeni bağlama yazılmamalı, ama kullanıcının istediği çekirdek yazısı
// (aday kaydı) yine de gitmelidir.
describe("AI ekranı — uçuştaki yanıtlar (dosya ve çalıştırma bağlamı)", () => {
  it("onay uçuşta dosya değişirse eski dosyanın onayı yeni dosyaya yazılmaz", async () => {
    const gate = deferred<TransferConsentRow>();
    setup({ consent: () => gate.promise });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() =>
      expect(mocks.aiConsentCreate).toHaveBeenCalledWith(
        "p1",
        "api:claude@1.4.0",
        "ozet",
        ["d1", "d2"],
      ),
    );

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.resolve(CONSENT);
      await Promise.resolve();
    });

    // Onay p1'in belgelerine aitti: p2'de onay satırı ve çalıştırma açılmaz.
    expect(button(t("ai.approve")).disabled).toBe(false);
    expect(screen.queryByRole("button", { name: t("ai.revoke") })).toBeNull();
    expect(button(t("ai.run")).disabled).toBe(true);
    expect(screen.queryByText(t("ai.previewTitle"))).toBeNull();
  });

  it("onay önizlemesi uçuşta dosya değişirse eski dosyanın önizlemesi yeni dosyada çizilmez", async () => {
    const gate = deferred<ConsentPreview>();
    setup({ preview: () => gate.promise });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(mocks.aiConsentPreview).toHaveBeenCalledTimes(1));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.resolve(PREVIEW);
      await Promise.resolve();
    });

    expect(screen.queryByText(t("ai.previewTitle"))).toBeNull();
    // Onay p1'in belgeleri için oluşturulmaz: dosya değişti.
    expect(mocks.aiConsentCreate).not.toHaveBeenCalled();
  });

  it("çalıştırma uçuşta dosya değişirse eski dosyanın özeti yeni dosyaya yazılmaz", async () => {
    const gate = deferred<void>();
    setup({
      runExecute: () => gate.promise,
      overview: async (caseId: string) => (caseId === "p2" ? OVERVIEW_B : OVERVIEW_A),
    });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));
    fireEvent.click(button(t("ai.run")));
    await waitFor(() =>
      expect(mocks.aiRunExecute).toHaveBeenCalledWith("p1", "prov-a", CONSENT.id, "summarize"),
    );

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.resolve();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    // Tazeleme p1'in özetiydi: p2'nin künyesi ve sağlayıcısı korunur.
    expect(screen.getByText(coverageLine(OVERVIEW_B))).toBeTruthy();
    expect(screen.queryByText(coverageLine(OVERVIEW_A))).toBeNull();
    expect(screen.queryByText("api:claude (api)")).toBeNull();
  });

  it("çalıştırma seçimi hızlı değişince geç gelen aday listesi yeni seçimi ezmez", async () => {
    const gate = deferred<AiCandidateRow[]>();
    setup({
      candidates: (runId) =>
        runId === "r1" ? gate.promise : Promise.resolve([CAND_BLOCKED]),
    });
    await screen.findByText("api:claude (api)");

    const rows = runRows();
    fireEvent.click(within(rows[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    fireEvent.click(within(rows[1]!).getByRole("button", { name: RUN_LABEL_SELECT }));

    expect(await screen.findByText(CAND_BLOCKED.structuredPayload)).toBeTruthy();
    expect(mocks.aiCandidatesList.mock.calls.map(([runId]) => runId)).toEqual(["r1", "r2"]);

    await act(async () => {
      gate.resolve([CAND_VALID]);
      await Promise.resolve();
    });

    // r1'in geç yanıtı r2'nin listesini ezmez.
    expect(screen.getByText(CAND_BLOCKED.structuredPayload)).toBeTruthy();
    expect(screen.queryByText(CAND_VALID.structuredPayload)).toBeNull();
  });

  it("doğrulama uçuşta dosya değişirse rapor yeni dosyaya yazılmaz", async () => {
    const gate = deferred<CandidateReport>();
    setup({ validate: () => gate.promise });
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    await waitFor(() => expect(button(t("ai.validate")).disabled).toBe(false));

    fireEvent.click(button(t("ai.validate")));
    await waitFor(() => expect(mocks.aiCandidateValidate).toHaveBeenCalledTimes(1));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.resolve(REPORT_VALID);
      await Promise.resolve();
    });

    expect(screen.queryByText(quoteSummary(REPORT_VALID))).toBeNull();
    expect(screen.getByText(t("ai.noRun"))).toBeTruthy();
  });

  it("«Doğrula ve kaydet» uçuşta dosya değişirse yazma gider, liste ve bildirim yeni dosyaya yazılmaz", async () => {
    const gate = deferred<CandidateReport>();
    setup({ validate: () => gate.promise });
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    await waitFor(() => expect(button(t("ai.validateAndSave")).disabled).toBe(false));

    fireEvent.click(button(t("ai.validateAndSave")));
    await waitFor(() => expect(mocks.aiCandidateValidate).toHaveBeenCalledTimes(1));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.resolve(REPORT_INVALID);
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    // Kullanıcının istediği kayıt çekirdeğe **gitti**; arayüz yalnız yazmaz.
    expect(mocks.aiCandidateRecord).toHaveBeenCalledTimes(1);
    expect(mocks.aiCandidateRecord).toHaveBeenCalledWith("r1", "claim_candidate", CLAIM_TEMPLATE);
    expect(
      screen.queryByRole("heading", { level: 2, name: t("ai.candidatesTitle") }),
    ).toBeNull();
    expect(screen.queryByText(t("ai.noticeInvalid"))).toBeNull();
    expect(screen.queryByText(t("ai.noticeValid"))).toBeNull();
  });

  it("geç dönen onay iptali yeni onayı silmez", async () => {
    const gate = deferred<void>();
    let consentCalls = 0;
    setup({
      consent: async () => {
        consentCalls += 1;
        return consentCalls === 1 ? CONSENT : CONSENT_NEW;
      },
    });
    await screen.findByText("api:claude (api)");
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));

    mocks.aiConsentRevoke.mockImplementation(() => gate.promise);
    fireEvent.click(button(t("ai.revoke")));
    await waitFor(() => expect(mocks.aiConsentRevoke).toHaveBeenCalledWith(CONSENT.id));

    // İptal uçuşta iken yeni onay alınır.
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(mocks.aiConsentCreate).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));

    await act(async () => {
      gate.resolve();
      await Promise.resolve();
    });

    // Geç yanıt **eski** onayı iptal etti; yeni onay satırı durur.
    expect(screen.queryByRole("button", { name: t("ai.revoke") })).not.toBeNull();
    expect(button(t("ai.run")).disabled).toBe(false);
  });

  it("incelemeye gönderim uçuşta dosya değişirse bildirim yeni dosyaya yazılmaz", async () => {
    const gate = deferred<unknown>();
    setup();
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    const card = await screen.findByRole("heading", { level: 2, name: t("ai.candidatesTitle") });

    mocks.aiCandidatePromote.mockImplementation(() => gate.promise);
    fireEvent.click(
      within(card.closest(".card") as HTMLElement).getByRole("button", { name: t("ai.promote") }),
    );
    await waitFor(() => expect(mocks.aiCandidatePromote).toHaveBeenCalledWith("cand-1", "p1"));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.resolve({ id: "rv-1" });
      await Promise.resolve();
    });

    expect(screen.queryByText(t("ai.noticePromoted"))).toBeNull();
  });

  it("incelemeye gönderim uçuşta düşerse hata yeni dosyaya yazılmaz", async () => {
    const gate = deferred<unknown>();
    setup();
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    const card = await screen.findByRole("heading", { level: 2, name: t("ai.candidatesTitle") });

    mocks.aiCandidatePromote.mockImplementation(() => gate.promise);
    fireEvent.click(
      within(card.closest(".card") as HTMLElement).getByRole("button", { name: t("ai.promote") }),
    );
    await waitFor(() => expect(mocks.aiCandidatePromote).toHaveBeenCalledTimes(1));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.reject(new Error("çekirdek reddetti"));
      await Promise.resolve();
    });

    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("«İçeriği göster» uçuşta dosya değişirse eski dosyanın önizlemesi yeni dosyada çizilmez", async () => {
    const gate = deferred<ConsentPreview>();
    setup({ preview: () => gate.promise });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.showContent")));
    await waitFor(() => expect(mocks.aiConsentPreview).toHaveBeenCalledTimes(1));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.resolve(PREVIEW);
      await Promise.resolve();
    });

    expect(screen.queryByText(t("ai.previewTitle"))).toBeNull();
  });

  it("yeniden doğrulama uçuşta düşerse hata yeni dosyaya yazılmaz", async () => {
    const gate = deferred<AiCandidateRow>();
    setup();
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    const card = await screen.findByRole("heading", { level: 2, name: t("ai.candidatesTitle") });

    mocks.aiCandidateRevalidate.mockImplementation(() => gate.promise);
    fireEvent.click(
      within(card.closest(".card") as HTMLElement).getByRole("button", { name: t("ai.revalidate") }),
    );
    await waitFor(() => expect(mocks.aiCandidateRevalidate).toHaveBeenCalledWith("cand-1"));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.reject(new Error("yeniden doğrulama çöktü"));
      await Promise.resolve();
    });

    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("hızlı dosya değişiminde geç gelen özet yeni dosyanın künyesini ezmez", async () => {
    const gate = deferred<AiOverview>();
    setup({
      overview: (caseId: string) =>
        caseId === "p2" ? gate.promise : Promise.resolve(OVERVIEW_A),
    });
    await screen.findByText("api:claude (api)");

    switchCase("p2");
    await waitFor(() => expect(mocks.aiOverview).toHaveBeenCalledWith("p2"));
    switchCase("p1");
    await waitFor(() => expect(screen.getByText(coverageLine(OVERVIEW_A))).toBeTruthy());

    await act(async () => {
      gate.resolve(OVERVIEW_B);
      await Promise.resolve();
    });

    expect(screen.getByText(coverageLine(OVERVIEW_A))).toBeTruthy();
    expect(screen.queryByText("local:ollama (local)")).toBeNull();
  });

  /* ------------------- hata yolları: geç gelen hata yeni bağlama yazılmaz */

  it("onay uçuşta düşerse hata yeni dosyaya yazılmaz", async () => {
    const gate = deferred<TransferConsentRow>();
    setup({ consent: () => gate.promise });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(mocks.aiConsentCreate).toHaveBeenCalledTimes(1));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.reject(new Error("onay yazılamadı"));
      await Promise.resolve();
    });

    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("çalıştırma uçuşta düşerse hata yeni dosyaya yazılmaz", async () => {
    const gate = deferred<void>();
    setup({ runExecute: () => gate.promise });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));
    fireEvent.click(button(t("ai.run")));
    await waitFor(() => expect(mocks.aiRunExecute).toHaveBeenCalledTimes(1));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.reject(new Error("çalıştırma reddedildi"));
      await Promise.resolve();
    });

    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("hızlı dosya değişiminde düşen özet yeni dosyaya hata yazmaz", async () => {
    const gate = deferred<AiOverview>();
    setup({
      overview: (caseId: string) =>
        caseId === "p2" ? gate.promise : Promise.resolve(OVERVIEW_A),
    });
    await screen.findByText("api:claude (api)");

    switchCase("p2");
    await waitFor(() => expect(mocks.aiOverview).toHaveBeenCalledWith("p2"));
    switchCase("p1");
    await waitFor(() => expect(screen.getByText(coverageLine(OVERVIEW_A))).toBeTruthy());

    await act(async () => {
      gate.reject(new Error("özet okunamadı"));
      await Promise.resolve();
    });

    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("aday listesi uçuşta düşerse hata yeni seçime yazılmaz", async () => {
    const gate = deferred<AiCandidateRow[]>();
    setup({
      candidates: (runId) =>
        runId === "r1" ? gate.promise : Promise.resolve([CAND_BLOCKED]),
    });
    await screen.findByText("api:claude (api)");

    const rows = runRows();
    fireEvent.click(within(rows[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    fireEvent.click(within(rows[1]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    expect(await screen.findByText(CAND_BLOCKED.structuredPayload)).toBeTruthy();

    await act(async () => {
      gate.reject(new Error("aday listesi okunamadı"));
      await Promise.resolve();
    });

    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText(CAND_BLOCKED.structuredPayload)).toBeTruthy();
  });

  it("doğrulama uçuşta düşerse hata yeni dosyaya yazılmaz", async () => {
    const gate = deferred<CandidateReport>();
    setup({ validate: () => gate.promise });
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    await waitFor(() => expect(button(t("ai.validate")).disabled).toBe(false));

    fireEvent.click(button(t("ai.validate")));
    await waitFor(() => expect(mocks.aiCandidateValidate).toHaveBeenCalledTimes(1));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.reject(new Error("doğrulama çöktü"));
      await Promise.resolve();
    });

    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("onay iptali uçuşta düşerse hata yeni dosyaya yazılmaz", async () => {
    const gate = deferred<void>();
    setup();
    await screen.findByText("api:claude (api)");
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));

    mocks.aiConsentRevoke.mockImplementation(() => gate.promise);
    fireEvent.click(button(t("ai.revoke")));
    await waitFor(() => expect(mocks.aiConsentRevoke).toHaveBeenCalledTimes(1));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    await act(async () => {
      gate.reject(new Error("iptal edilemedi"));
      await Promise.resolve();
    });

    expect(screen.queryByRole("alert")).toBeNull();
  });
});
