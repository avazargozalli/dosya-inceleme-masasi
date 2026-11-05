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
  TransferConsentListRow,
  TransferConsentRow,
  TransferReceiptRow,
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
  aiConsentList: vi.fn(),
  aiReceiptList: vi.fn(),
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
      // CONSENT ile **aynı** varış yeri: onay bu profille yürütülebilir.
      providerSnapshot: "api:claude@1.4.0",
    },
    {
      id: "prov-off",
      providerType: "off",
      adapterId: "off",
      executionEnvironment: "none",
      policyVersion: "0",
      providerSnapshot: "off@0",
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
      providerSnapshot: "off@0",
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
      providerSnapshot: "local:ollama@2.0.0",
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
  consents?: (caseId: string) => Promise<TransferConsentListRow[]>;
  receipts?: (caseId: string) => Promise<TransferReceiptRow[]>;
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
  // Onay **istenen** profile bağlanır (çekirdek sözleşmesi): sabit bir özet
  // döndürmek, p2'nin yerel sağlayıcısıyla alınmış onayı p1'in API varış
  // yerine bağlar ve ekranı yanlış yere "sağlayıcı değişti" dedirtirdi.
  const snapshotFor = (providerId: string): string =>
    providerId === "prov-local" ? "local:ollama@2.0.0" : CONSENT.providerSnapshot;
  mocks.aiConsentCreate.mockImplementation(
    options.consent ??
      (async (_caseId: string, providerId: string) => ({
        ...CONSENT,
        providerSnapshot: snapshotFor(providerId),
      })),
  );
  mocks.aiConsentRevoke.mockImplementation(options.revoke ?? (async () => undefined));
  mocks.aiConsentList.mockImplementation(options.consents ?? (async () => []));
  mocks.aiReceiptList.mockImplementation(options.receipts ?? (async () => []));
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
    // Künye okuması bir **effect**te başlar: seçicinin değeri render olduğu an
    // hazır olabilir, istek henüz gitmemiş olabilir (CI'da yüklü koşucuda
    // yarışıyordu). Çağrı da beklenir — çağrı hiç yapılmazsa test yine düşer.
    await waitFor(() => expect(mocks.aiOverview).toHaveBeenCalledWith("p1"));
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
      "prov-a",
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
        "prov-a",
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

// Bu blok **yazma uçuşu** kapılarını ölçer: önceki blok yanıtın hangi
// dosya/çalıştırmaya yazıldığını kapatmıştı; burada kilit devri, tek-uçuş
// (aynı düğme ikinci kez yazmaz), "gönderilen değer" ve okuma sırası var.
describe("AI ekranı — yazma uçuşu, kilit devri ve geç yanıt kapıları", () => {
  it("onay uçuşta iken dosya değişirse yeni dosyanın düğmeleri eski yanıta kilitli kalmaz", async () => {
    const gate = deferred<TransferConsentRow>();
    setup({ consent: () => gate.promise });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(mocks.aiConsentCreate).toHaveBeenCalledTimes(1));
    expect(button(t("ai.approve")).disabled).toBe(true);

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    // p1'in onayı hâlâ askıda: p2'nin düğmesi onu beklemez (yanıt hiç
    // gelmezse süresiz kilitli kalırdı).
    expect(button(t("ai.approve")).disabled).toBe(false);

    await act(async () => {
      gate.resolve(CONSENT);
      await Promise.resolve();
    });
    // Devredilen yanıt p2'ye onay yazmaz.
    expect(screen.queryByRole("button", { name: t("ai.revoke") })).toBeNull();
  });

  it("devredilen onayın kapanışı yeni dosyadaki onayın kilidini erken açmaz", async () => {
    const first = deferred<TransferConsentRow>();
    const second = deferred<TransferConsentRow>();
    let call = 0;
    setup({
      consent: () => {
        call += 1;
        return call === 1 ? first.promise : second.promise;
      },
    });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(mocks.aiConsentCreate).toHaveBeenCalledTimes(1));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(mocks.aiConsentCreate).toHaveBeenCalledTimes(2));
    expect(button(t("ai.approve")).disabled).toBe(true);

    // p1'in **geç** kapanışı p2'de süren onayın kilidini açmaz.
    await act(async () => {
      first.resolve(CONSENT);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(button(t("ai.approve")).disabled).toBe(true);
    expect(screen.queryByRole("button", { name: t("ai.revoke") })).toBeNull();

    await act(async () => {
      second.resolve({ ...CONSENT, id: "c-p2-onay", caseScope: "p2" });
      await Promise.resolve();
    });
    expect(button(t("ai.approve")).disabled).toBe(false);
    expect(await screen.findByRole("button", { name: t("ai.revoke") })).toBeTruthy();
  });

  it("«İncelemeye gönder» uçuşta ikinci tıklama ikinci yazma göndermez", async () => {
    const gate = deferred<unknown>();
    setup();
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    const heading = await screen.findByRole("heading", { level: 2, name: t("ai.candidatesTitle") });
    const card = heading.closest(".card") as HTMLElement;

    mocks.aiCandidatePromote.mockImplementation(() => gate.promise);
    const promoteButton = within(card).getByRole("button", { name: t("ai.promote") }) as HTMLButtonElement;
    fireEvent.click(promoteButton);
    await waitFor(() => expect(mocks.aiCandidatePromote).toHaveBeenCalledTimes(1));
    expect(promoteButton.disabled).toBe(true);

    fireEvent.click(promoteButton);
    expect(mocks.aiCandidatePromote).toHaveBeenCalledTimes(1);

    await act(async () => {
      gate.resolve({ id: "rv-1" });
      await Promise.resolve();
    });
    expect(await screen.findByText(t("ai.noticePromoted"))).toBeTruthy();
    expect((within(card).getByRole("button", { name: t("ai.promote") }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("onay iptali uçuşta ikinci tıklama ikinci iptal göndermez", async () => {
    const gate = deferred<void>();
    setup();
    await screen.findByText("api:claude (api)");
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));

    mocks.aiConsentRevoke.mockImplementation(() => gate.promise);
    fireEvent.click(button(t("ai.revoke")));
    await waitFor(() => expect(mocks.aiConsentRevoke).toHaveBeenCalledTimes(1));
    expect(button(t("ai.revoke")).disabled).toBe(true);

    fireEvent.click(button(t("ai.revoke")));
    expect(mocks.aiConsentRevoke).toHaveBeenCalledTimes(1);

    await act(async () => {
      gate.resolve();
      await Promise.resolve();
    });
    // Başarılı iptal satırı kaldırır; ikinci isteğin `E_NOT_FOUND`u yok.
    expect(screen.queryByRole("button", { name: t("ai.revoke") })).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("kayıt uçuşta çalıştırma değişirse kayıt bildirimi ve liste yeni çalıştırmaya yazılmaz", async () => {
    const gate = deferred<unknown>();
    setup({
      // İki çalıştırmanın adayları **ayrı**: geç tazeleme yanlış listeyi yazarsa görünür.
      candidates: async (runId: string) => (runId === "r2" ? [CAND_BLOCKED] : [CAND_VALID]),
    });
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    await waitFor(() => expect(button(t("ai.validateAndSave")).disabled).toBe(false));

    mocks.aiCandidateRecord.mockImplementation(() => gate.promise);
    fireEvent.click(button(t("ai.validateAndSave")));
    await waitFor(() => expect(mocks.aiCandidateRecord).toHaveBeenCalledTimes(1));

    // Kayıt askıda iken ikinci çalıştırmaya geçilir (bildirim oraya yazılmamalı).
    fireEvent.click(within(runRows()[1]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    await waitFor(() => expect(mocks.aiCandidatesList).toHaveBeenLastCalledWith("r2"));
    expect(await screen.findByText(CAND_BLOCKED.structuredPayload)).toBeTruthy();

    await act(async () => {
      gate.resolve({ id: "cand-1" });
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    // Kullanıcının istediği kayıt çekirdeğe gitti; ne bildirim ne liste yeni bağlama yazılır.
    expect(mocks.aiCandidateRecord).toHaveBeenCalledTimes(1);
    expect(screen.queryByText(t("ai.noticeValid"))).toBeNull();
    expect(screen.queryByText(t("ai.noticeInvalid"))).toBeNull();
    expect(screen.queryByText(t("ai.noticePartial"))).toBeNull();
    expect(screen.queryByText(CAND_VALID.structuredPayload)).toBeNull();
    expect(screen.getByText(CAND_BLOCKED.structuredPayload)).toBeTruthy();
  });

  it("kayıt sonrası liste tazelenirken çalıştırma değişirse bildirim yazılmaz", async () => {
    const refresh = deferred<AiCandidateRow[]>();
    let r1Calls = 0;
    setup({
      candidates: (runId: string) => {
        if (runId === "r2") return Promise.resolve([CAND_BLOCKED]);
        r1Calls += 1;
        // İlk okuma (seçim) döner; **tazeleme** askıda kalır.
        return r1Calls === 1 ? Promise.resolve([CAND_VALID]) : refresh.promise;
      },
    });
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    await waitFor(() => expect(button(t("ai.validateAndSave")).disabled).toBe(false));

    fireEvent.click(button(t("ai.validateAndSave")));
    await waitFor(() => expect(r1Calls).toBe(2));

    fireEvent.click(within(runRows()[1]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    expect(await screen.findByText(CAND_BLOCKED.structuredPayload)).toBeTruthy();

    await act(async () => {
      refresh.resolve([CAND_VALID]);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(screen.queryByText(t("ai.noticeValid"))).toBeNull();
    expect(screen.getByText(CAND_BLOCKED.structuredPayload)).toBeTruthy();
  });

  it("gönderim uçuşta çalıştırma değişirse gönderim bildirimi yeni çalıştırmaya yazılmaz", async () => {
    const gate = deferred<unknown>();
    setup();
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    const heading = await screen.findByRole("heading", { level: 2, name: t("ai.candidatesTitle") });

    mocks.aiCandidatePromote.mockImplementation(() => gate.promise);
    fireEvent.click(
      within(heading.closest(".card") as HTMLElement).getByRole("button", { name: t("ai.promote") }),
    );
    await waitFor(() => expect(mocks.aiCandidatePromote).toHaveBeenCalledTimes(1));

    fireEvent.click(within(runRows()[1]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    await waitFor(() => expect(mocks.aiCandidatesList).toHaveBeenLastCalledWith("r2"));

    await act(async () => {
      gate.resolve({ id: "rv-1" });
      await Promise.resolve();
    });

    expect(screen.queryByText(t("ai.noticePromoted"))).toBeNull();
  });

  it("iki «İçeriği göster» isteğinden eskisinin hatası yeni önizlemeyi silmez", async () => {
    const first = deferred<ConsentPreview>();
    const second = deferred<ConsentPreview>();
    let call = 0;
    setup({
      preview: () => {
        call += 1;
        return call === 1 ? first.promise : second.promise;
      },
    });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.showContent")));
    await waitFor(() => expect(mocks.aiConsentPreview).toHaveBeenCalledTimes(1));
    fireEvent.click(button(t("ai.showContent")));
    await waitFor(() => expect(mocks.aiConsentPreview).toHaveBeenCalledTimes(2));

    // **Son** istek önce döner.
    await act(async () => {
      second.resolve(PREVIEW);
      await Promise.resolve();
    });
    expect(await screen.findByText(t("ai.previewTitle"))).toBeTruthy();

    // Eskisinin geç hatası yeni önizlemeyi ne siler ne kırmızı yazar.
    await act(async () => {
      first.reject(new Error("önizleme çekilemedi"));
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText(t("ai.previewTitle"))).toBeTruthy();
  });

  it("düşen önizlemeden sonra başarılı okuma eski kırmızıyı siler", async () => {
    let fail = true;
    setup({
      preview: async () => {
        if (fail) throw new Error("önizleme çekilemedi");
        return PREVIEW;
      },
    });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.showContent")));
    expect((await screen.findByRole("alert")).textContent).toContain("E_UI_UNKNOWN");

    fail = false;
    fireEvent.click(button(t("ai.showContent")));
    expect(await screen.findByText(t("ai.previewTitle"))).toBeTruthy();
    // Son okuma başarılı: önceki isteğin kırmızısı ekranda kalmaz.
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("eski isteğin geç başarısı yeni isteğin hatasını silmez", async () => {
    const first = deferred<ConsentPreview>();
    const second = deferred<ConsentPreview>();
    let call = 0;
    setup({
      preview: () => {
        call += 1;
        return call === 1 ? first.promise : second.promise;
      },
    });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.showContent")));
    await waitFor(() => expect(mocks.aiConsentPreview).toHaveBeenCalledTimes(1));
    fireEvent.click(button(t("ai.showContent")));
    await waitFor(() => expect(mocks.aiConsentPreview).toHaveBeenCalledTimes(2));

    // **Son** istek düşer: kırmızı yazılır, önizleme yok.
    await act(async () => {
      second.reject(new Error("önizleme çekilemedi"));
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.getByRole("alert")).toBeTruthy();
    expect(screen.queryByText(t("ai.previewTitle"))).toBeNull();

    // Eskisinin **geç başarısı** yeni isteğin hatasını silmez.
    await act(async () => {
      first.resolve(PREVIEW);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.queryByText(t("ai.previewTitle"))).toBeNull();
    expect(screen.getByRole("alert")).toBeTruthy();
  });

  it("doğrulama uçuşta aday türü değişirse rapor yeni türün altına yazılmaz", async () => {
    const gate = deferred<CandidateReport>();
    setup({ validate: () => gate.promise });
    await screen.findByText("api:claude (api)");
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    await waitFor(() => expect(button(t("ai.validate")).disabled).toBe(false));

    fireEvent.click(button(t("ai.validate")));
    await waitFor(() => expect(mocks.aiCandidateValidate).toHaveBeenCalledTimes(1));

    const eventOption = CANDIDATE_KINDS.find((option) => option.value === "event_candidate")!;
    fireEvent.change(kindSelect(), { target: { value: "event_candidate" } });
    expect(draftBox().value).toBe(eventOption.template);

    await act(async () => {
      gate.resolve(REPORT_VALID);
      await Promise.resolve();
      await Promise.resolve();
    });

    // Rapor **gönderilen** türe aitti: yeni türün altında "güncel" gibi durmaz.
    expect(screen.queryByText(quoteSummary(REPORT_VALID))).toBeNull();
    const reportCard = cardByHeading(t("ai.validateTitle"));
    expect(within(reportCard).queryByText(candidateStateLabel("valid"))).toBeNull();
  });

  it("dosya değişimi gönderim ve iptal kilidini devreder", async () => {
    // p2'de de bir çalıştırma ve aday olsun: devredilen kilit orada görünür.
    const overviewP2: AiOverview = {
      ...OVERVIEW_B,
      runs: [
        {
          id: "r9",
          caseId: "p2",
          taskType: "summarize",
          providerId: "prov-local",
          modelSnapshot: "llama3-8b",
          status: "succeeded",
          usageReport: "{}",
          createdAt: "2026-10-22T09:00:00.000Z",
          completedAt: "2026-10-22T09:01:00.000Z",
        },
      ],
    };
    const promoteGate = deferred<unknown>();
    const revokeGate = deferred<void>();
    setup({ overview: async (caseId: string) => (caseId === "p2" ? overviewP2 : OVERVIEW_A) });
    await screen.findByText("api:claude (api)");

    // p1: onay + aday; gönderim ve iptal **askıda** bırakılır.
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    const p1Heading = await screen.findByRole("heading", { level: 2, name: t("ai.candidatesTitle") });
    mocks.aiCandidatePromote.mockImplementation(() => promoteGate.promise);
    mocks.aiConsentRevoke.mockImplementation(() => revokeGate.promise);
    fireEvent.click(
      within(p1Heading.closest(".card") as HTMLElement).getByRole("button", { name: t("ai.promote") }),
    );
    await waitFor(() => expect(mocks.aiCandidatePromote).toHaveBeenCalledTimes(1));
    fireEvent.click(button(t("ai.revoke")));
    await waitFor(() => expect(mocks.aiConsentRevoke).toHaveBeenCalledTimes(1));

    switchCase("p2");
    await screen.findByText("local:ollama (local)");

    // p2: yeni çalıştırmayı seç ve onay al.
    fireEvent.click(within(runRows()[0]!).getByRole("button", { name: RUN_LABEL_SELECT }));
    await waitFor(() => expect(mocks.aiCandidatesList).toHaveBeenLastCalledWith("r9"));
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));

    // p1'in iki isteği hâlâ askıda: p2'nin düğmeleri kilitli değil.
    expect(button(t("ai.revoke")).disabled).toBe(false);
    const p2Heading = cardByHeading(t("ai.candidatesTitle"));
    expect(
      (within(p2Heading).getByRole("button", { name: t("ai.promote") }) as HTMLButtonElement).disabled,
    ).toBe(false);
  });
});

describe("AI ekranı — onay amacı ve iptal uçuşu", () => {
  it("onay iptali uçuştayken çalıştır kapalıdır: gönderim iptalden önce başlamaz", async () => {
    const revokeGate = deferred<void>();
    setup({ revoke: () => revokeGate.promise });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));

    fireEvent.click(button(t("ai.revoke")));
    await waitFor(() => expect(mocks.aiConsentRevoke).toHaveBeenCalledTimes(1));

    // F09-T09: iptal **uygulanmadan** yeni gönderim başlamaz.
    expect(button(t("ai.run")).disabled).toBe(true);
    fireEvent.click(button(t("ai.run")));
    expect(mocks.aiRunExecute).toHaveBeenCalledTimes(0);

    revokeGate.resolve();
    await waitFor(() => expect(screen.queryByRole("button", { name: t("ai.revoke") })).toBeNull());
    expect(mocks.aiRunExecute).toHaveBeenCalledTimes(0);
  });

  it("iptal uçuşu, o sırada alınan yeni onayın çalıştırmasını kapatmaz", async () => {
    const revokeGate = deferred<void>();
    setup({ revoke: () => revokeGate.promise });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));
    fireEvent.click(button(t("ai.revoke")));
    await waitFor(() => expect(mocks.aiConsentRevoke).toHaveBeenCalledTimes(1));

    // Eski onay iptal edilirken yeni bir onay alınır.
    mocks.aiConsentCreate.mockResolvedValueOnce(CONSENT_NEW);
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(mocks.aiConsentCreate).toHaveBeenCalledTimes(2));

    // Yeni onay eski iptalin hedefi değil: çalıştırma açık kalır.
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));
    fireEvent.click(button(t("ai.run")));
    await waitFor(() => expect(mocks.aiRunExecute).toHaveBeenCalledTimes(1));
    expect(mocks.aiRunExecute).toHaveBeenCalledWith("p1", "prov-a", CONSENT_NEW.id, "summarize");

    revokeGate.resolve();
    // Eski onayın geç iptali yeni onayı silmez.
    await waitFor(() => expect(button(t("ai.revoke")).disabled).toBe(false));
  });

  it("çalıştırma onayın amacıyla koşar: iddia çıkarımı özet diye gönderilmez", async () => {
    setup({ consent: async () => ({ ...CONSENT, purpose: "iddia_cikarimi" }) });
    await screen.findByText("api:claude (api)");

    fireEvent.change(purposeSelect(), { target: { value: "iddia_cikarimi" } });
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() =>
      expect(mocks.aiConsentCreate).toHaveBeenCalledWith(
        "p1",
        "prov-a",
        "iddia_cikarimi",
        ["d1", "d2"],
      ),
    );
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));

    fireEvent.click(button(t("ai.run")));
    await waitFor(() => expect(mocks.aiRunExecute).toHaveBeenCalledTimes(1));
    expect(mocks.aiRunExecute).toHaveBeenCalledWith("p1", "prov-a", CONSENT.id, "extract_claims");
  });

  it("olay çıkarımı onayı olay göreviyle koşar", async () => {
    setup({ consent: async () => ({ ...CONSENT, purpose: "olay_cikarimi" }) });
    await screen.findByText("api:claude (api)");

    fireEvent.change(purposeSelect(), { target: { value: "olay_cikarimi" } });
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));

    fireEvent.click(button(t("ai.run")));
    await waitFor(() => expect(mocks.aiRunExecute).toHaveBeenCalledTimes(1));
    expect(mocks.aiRunExecute).toHaveBeenCalledWith("p1", "prov-a", CONSENT.id, "extract_events");
  });

  it("onaydan sonra amaç değişirse onay düşer ve yeni gönderim yeniden onay ister", async () => {
    setup();
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));

    // Onay "Özet" amacına bağlıydı; seçim değişince o onay artık geçerli değil.
    fireEvent.change(purposeSelect(), { target: { value: "iddia_cikarimi" } });
    await waitFor(() => expect(screen.queryByRole("button", { name: t("ai.revoke") })).toBeNull());
    expect(button(t("ai.run")).disabled).toBe(true);
    expect(mocks.aiRunExecute).toHaveBeenCalledTimes(0);

    // Yeniden onay: gönderim yeni amacın göreviyle koşar.
    mocks.aiConsentCreate.mockResolvedValueOnce({ ...CONSENT, purpose: "iddia_cikarimi" });
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));
    fireEvent.click(button(t("ai.run")));
    await waitFor(() => expect(mocks.aiRunExecute).toHaveBeenCalledTimes(1));
    expect(mocks.aiRunExecute).toHaveBeenCalledWith("p1", "prov-a", CONSENT.id, "extract_claims");
  });

  it("onay satırı onaylanan amacı yazar", async () => {
    setup({ consent: async () => ({ ...CONSENT, purpose: "olay_cikarimi" }) });
    await screen.findByText("api:claude (api)");

    fireEvent.click(button(t("ai.approve")));
    const revoke = await screen.findByRole("button", { name: t("ai.revoke") });
    const line = revoke.closest("p");
    if (!(line instanceof HTMLParagraphElement)) throw new Error("onay satırı yok");
    expect(line.textContent).toContain(
      t("ai.consentPurpose", { purpose: t("ai.purposeEvents") }),
    );
  });
});

describe("AI ekranı — aktarım onayı envanteri", () => {
  function consentListRow(
    overrides: Partial<TransferConsentListRow> & { id: string },
  ): TransferConsentListRow {
    return {
      providerSnapshot: "claude-code@1",
      caseScope: "p1",
      purpose: "ozet",
      selectedDocumentIds: ["d1"],
      expiresAt: null,
      consentTextVersion: "1",
      approvedAt: "2026-11-01T10:00:00.000Z",
      revokedAt: null,
      active: true,
      providerCurrent: true,
      ...overrides,
    };
  }

  it("envanteri çekirdekten okur; amaç, sağlayıcı ve durumu yazar", async () => {
    const active = consentListRow({ id: "c1" });
    const revoked = consentListRow({
      id: "c2",
      purpose: "iddia_cikarimi",
      revokedAt: "2026-11-02T10:00:00.000Z",
      active: false,
    });
    const expired = consentListRow({
      id: "c3",
      purpose: "olay_cikarimi",
      expiresAt: "2000-01-01T00:00:00.000Z",
      active: false,
    });
    setup({ consents: async () => [active, revoked, expired] });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.consentsTitle"));
    await within(card).findByText(t("ai.consentActive"));
    expect(mocks.aiConsentList).toHaveBeenCalledWith("p1");

    const rows = within(card).getAllByRole("row").slice(1);
    expect(rows).toHaveLength(3);
    expect(rows[0]!.textContent).toContain(t("ai.purposeSummary"));
    expect(rows[0]!.textContent).toContain("claude-code@1");
    expect(rows[0]!.textContent).toContain(t("ai.consentNoExpiry"));
    expect(rows[1]!.textContent).toContain(t("ai.consentRevoked"));
    expect(rows[2]!.textContent).toContain(t("ai.consentExpired"));
    // İptal yalnız **etkin** satırda sunulur; kapanmış yetki yeniden iptal edilmez.
    expect(within(rows[0]!).getByRole("button", { name: t("ai.revoke") })).toBeTruthy();
    expect(within(rows[1]!).queryByRole("button", { name: t("ai.revoke") })).toBeNull();
    expect(within(rows[2]!).queryByRole("button", { name: t("ai.revoke") })).toBeNull();
  });

  it("envanterden iptal çekirdeğe gider ve liste çekirdekten tazelenir", async () => {
    let rows = [consentListRow({ id: "c1" })];
    setup({ consents: async () => rows });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.consentsTitle"));
    const revoke = await within(card).findByRole("button", { name: t("ai.revoke") });

    rows = [
      consentListRow({
        id: "c1",
        revokedAt: "2026-11-05T10:00:00.000Z",
        active: false,
      }),
    ];
    fireEvent.click(revoke);

    await waitFor(() => expect(mocks.aiConsentRevoke).toHaveBeenCalledWith("c1"));
    await within(card).findByText(t("ai.consentRevoked"));
    expect(within(card).queryByRole("button", { name: t("ai.revoke") })).toBeNull();
    expect(mocks.aiConsentList.mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it("iptal uçuşunda satır düğmesi ikinci isteği göndermez", async () => {
    const pending = deferred<void>();
    setup({ consents: async () => [consentListRow({ id: "c1" })], revoke: () => pending.promise });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.consentsTitle"));
    const revoke = (await within(card).findByRole("button", {
      name: t("ai.revoke"),
    })) as HTMLButtonElement;
    fireEvent.click(revoke);
    await waitFor(() => expect(revoke.disabled).toBe(true));
    fireEvent.click(revoke);
    expect(mocks.aiConsentRevoke).toHaveBeenCalledTimes(1);
    await act(async () => pending.resolve(undefined));
  });

  it("iptal düşerse hata görünür, satır etkin kalır ve liste tazelenmez", async () => {
    setup({
      consents: async () => [consentListRow({ id: "c1" })],
      revoke: async () => {
        throw { code: "E_DB", message: "yazılamadı" };
      },
    });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.consentsTitle"));
    const revoke = await within(card).findByRole("button", { name: t("ai.revoke") });
    const listCalls = mocks.aiConsentList.mock.calls.length;
    fireEvent.click(revoke);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DB");
    expect(within(card).getByText(t("ai.consentActive"))).toBeTruthy();
    expect(mocks.aiConsentList.mock.calls.length).toBe(listCalls);
  });

  it("dosya değişiminde geç gelen envanter yeni dosyanın listesine yazılmaz", async () => {
    const pending = deferred<TransferConsentListRow[]>();
    setup({
      consents: (caseId: string) =>
        caseId === "p1"
          ? pending.promise
          : Promise.resolve([
              consentListRow({ id: "c2", caseScope: "p2", purpose: "iddia_cikarimi" }),
            ]),
    });

    await screen.findByText("api:claude (api)");
    switchCase("p2");
    const card = cardByHeading(t("ai.consentsTitle"));
    await within(card).findByText(t("ai.purposeClaims"));

    await act(async () => pending.resolve([consentListRow({ id: "c1" })]));
    expect(within(card).queryByText(t("ai.purposeSummary"))).toBeNull();
    expect(within(card).getByText(t("ai.purposeClaims"))).toBeTruthy();
  });

  it("onay verilince envanter tazelenir ve yeni satır görünür", async () => {
    let rows: TransferConsentListRow[] = [];
    setup({ consents: async () => rows });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.consentsTitle"));
    await within(card).findByText(t("ai.consentsEmpty"));
    const listCalls = mocks.aiConsentList.mock.calls.length;

    rows = [consentListRow({ id: CONSENT.id, selectedDocumentIds: ["d1", "d2"] })];
    fireEvent.click(button(t("ai.approve")));

    await waitFor(() => expect(mocks.aiConsentCreate).toHaveBeenCalledTimes(1));
    await within(card).findByText(t("ai.consentActive"));
    expect(mocks.aiConsentList.mock.calls.length).toBeGreaterThan(listCalls);
  });

  it("envanter okuma hatası görünür ve boş liste dürüstçe yazılır", async () => {
    setup({
      consents: async () => {
        throw { code: "E_DB", message: "okunamadı" };
      },
    });

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DB");
    const card = cardByHeading(t("ai.consentsTitle"));
    expect(within(card).getByText(t("ai.consentsEmpty"))).toBeTruthy();
  });

  it("iptal uçuşunda dosya değişirse eski dosyanın envanteri tazelenmez", async () => {
    const pending = deferred<void>();
    setup({
      consents: async (caseId: string) =>
        caseId === "p1"
          ? [consentListRow({ id: "c1" })]
          : [consentListRow({ id: "c2", caseScope: "p2", purpose: "iddia_cikarimi" })],
      revoke: () => pending.promise,
    });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.consentsTitle"));
    const revoke = (await within(card).findByRole("button", {
      name: t("ai.revoke"),
    })) as HTMLButtonElement;
    fireEvent.click(revoke);
    await waitFor(() => expect(revoke.disabled).toBe(true));

    switchCase("p2");
    await within(card).findByText(t("ai.purposeClaims"));
    const p1Calls = mocks.aiConsentList.mock.calls.filter(([id]) => id === "p1").length;

    await act(async () => pending.resolve(undefined));
    expect(mocks.aiConsentList.mock.calls.filter(([id]) => id === "p1").length).toBe(p1Calls);
    expect(within(card).getByText(t("ai.purposeClaims"))).toBeTruthy();
    expect(within(card).queryByText(t("ai.purposeSummary"))).toBeNull();
  });

  it("onay sonrası tazeleme, askıdaki eski okumanın geç yanıtını ezer", async () => {
    const initial = deferred<TransferConsentListRow[]>();
    let calls = 0;
    setup({
      consents: () => {
        calls += 1;
        return calls === 1
          ? initial.promise
          : Promise.resolve([consentListRow({ id: "c-new" })]);
      },
    });

    await screen.findByText("api:claude (api)");
    fireEvent.click(button(t("ai.approve")));
    const card = cardByHeading(t("ai.consentsTitle"));
    await within(card).findByText(t("ai.consentActive"));

    // İlk okuma (boş/eski liste) **sonra** döner; yeni dosyanın listesini ezmemeli.
    await act(async () =>
      initial.resolve([consentListRow({ id: "c-old", purpose: "iddia_cikarimi" })]),
    );
    expect(within(card).queryByText(t("ai.purposeClaims"))).toBeNull();
    expect(within(card).getByText(t("ai.purposeSummary"))).toBeTruthy();
  });

  it("Yenile uçuşta iken ikinci okuma başlatmaz", async () => {
    const pending = deferred<TransferConsentListRow[]>();
    let calls = 0;
    setup({
      consents: () => {
        calls += 1;
        return calls === 1
          ? Promise.resolve([consentListRow({ id: "c1" })])
          : pending.promise;
      },
    });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.consentsTitle"));
    await within(card).findByText(t("ai.consentActive"));
    const refresh = within(card).getByRole("button", {
      name: t("ai.consentsRefresh"),
    }) as HTMLButtonElement;

    fireEvent.click(refresh);
    await waitFor(() => expect(refresh.disabled).toBe(true));
    fireEvent.click(refresh);
    expect(calls).toBe(2);
    await act(async () => pending.resolve([]));
  });
});

/**
 * Sağlayıcının **varış yeri** (uç nokta/model) değiştiğinde onay kaydı
 * yürürlükte kalır ama hiçbir profille yürütülemez. Ekran bunu satırda ve
 * "Çalıştır" kapısında **aynı** olgudan yazar; kullanıcı çekirdekten gelen
 * genel bir `E_AI_CONSENT`i yorumlamak zorunda kalmaz.
 */
describe("AI ekranı — sağlayıcı değişimi ve onayın yürütülebilirliği", () => {
  function row(
    overrides: Partial<TransferConsentListRow> & { id: string },
  ): TransferConsentListRow {
    return {
      providerSnapshot: "api:claude@1.4.0",
      caseScope: "p1",
      purpose: "ozet",
      selectedDocumentIds: ["d1"],
      expiresAt: null,
      consentTextVersion: "1",
      approvedAt: "2026-11-01T10:00:00.000Z",
      revokedAt: null,
      active: true,
      providerCurrent: true,
      ...overrides,
    };
  }

  /** Künye: API profilinin varış yeri özeti değiştirilmiş. */
  function overviewWith(snapshot: string): AiOverview {
    return {
      ...OVERVIEW_A,
      providers: [
        { ...OVERVIEW_A.providers[0]!, providerSnapshot: snapshot },
        OVERVIEW_A.providers[1]!,
      ],
    };
  }

  it("envanter sağlayıcı değişimini satırda yazar; kayıt yürürlükte kalır", async () => {
    const current = row({ id: "c1" });
    const stale = row({ id: "c2", purpose: "iddia_cikarimi", providerCurrent: false });
    setup({ consents: async () => [current, stale] });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.consentsTitle"));
    const rows = await within(card).findAllByRole("row");
    expect(rows[1]!.textContent).toContain(t("ai.consentActive"));
    expect(rows[2]!.textContent).toContain(t("ai.consentProviderChanged"));
    expect(rows[2]!.textContent).not.toContain(t("ai.consentActive"));
    // Yürütülemez onay **iptal edilebilir** bir kayıttır: yetki duruyor.
    expect(within(rows[2]!).getByRole("button", { name: t("ai.revoke") })).toBeTruthy();
    const badge = within(rows[2]!).getByText(t("ai.consentProviderChanged"));
    expect(badge.className).toContain("warn");
  });

  it("onayın varış yeri hiçbir profille eşleşmiyorsa çalıştır kapalıdır ve nedeni yazılır", async () => {
    setup({ overview: async () => overviewWith("api:claude@2.0.0") });
    await screen.findByText("api:claude (api)");

    // Onay **eski** varış yerine bağlı döner: profil 2.0.0'a taşınmış.
    fireEvent.click(button(t("ai.approve")));
    await screen.findByText(t("ai.consentStale", { provider: CONSENT.providerSnapshot }));
    expect(button(t("ai.run")).disabled).toBe(true);

    fireEvent.click(button(t("ai.run")));
    expect(mocks.aiRunExecute).not.toHaveBeenCalled();
  });

  it("onay başka bir profile bağlıysa çalıştırma o profille koşar", async () => {
    // İlk off-dışı profil `prov-x`; onay ise `prov-a` varış yerine bağlı.
    // Eski davranış ilk profili seçip çekirdekten `E_AI_CONSENT` alıyordu.
    const other = {
      id: "prov-x",
      providerType: "api",
      adapterId: "api:other",
      executionEnvironment: "local_process",
      policyVersion: "9.0.0",
      providerSnapshot: "api:other@9.0.0",
    };
    setup({
      overview: async () => ({ ...OVERVIEW_A, providers: [other, ...OVERVIEW_A.providers] }),
    });
    await screen.findByText("api:other (api)");

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));
    fireEvent.click(button(t("ai.run")));
    await waitFor(() => expect(mocks.aiRunExecute).toHaveBeenCalledTimes(1));
    expect(mocks.aiRunExecute).toHaveBeenCalledWith("p1", "prov-a", CONSENT.id, "summarize");
  });

  it("E_AI_PROVIDER_CHANGED ekranı tazeler: varış yeri yazılır, çalıştır kapanır", async () => {
    let calls = 0;
    setup({
      overview: async () => {
        calls += 1;
        return calls === 1 ? OVERVIEW_A : overviewWith("api:claude@2.0.0");
      },
      runExecute: async () => {
        throw { code: "E_AI_PROVIDER_CHANGED", message: "sağlayıcının varış yeri değişti" };
      },
    });
    await screen.findByText("api:claude (api)");
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));

    fireEvent.click(button(t("ai.run")));
    await waitFor(() =>
      expect(screen.getByRole("alert").textContent).toContain("E_AI_PROVIDER_CHANGED"),
    );
    // Künye tazelendi: yeni varış yeri ekranda, onay artık yürütülemez.
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(true));
    expect(calls).toBe(2);
    expect(screen.getByText("api:claude@2.0.0")).toBeTruthy();
    expect(
      screen.getByText(t("ai.consentStale", { provider: CONSENT.providerSnapshot })),
    ).toBeTruthy();
  });
});

describe("AI ekranı — aktarım makbuzu envanteri", () => {
  function receiptRow(
    overrides: Partial<TransferReceiptRow> & { id: string },
  ): TransferReceiptRow {
    return {
      aiRunId: "r1",
      caseId: "p1",
      taskType: "summarize",
      destinationLabel: "api.ornek.com · claude-sonnet",
      outboundHash: "a".repeat(64),
      itemCount: 3,
      byteCount: 4096,
      status: "dispatched",
      dispatchedAt: "2026-11-05T09:00:00.000Z",
      ...overrides,
    };
  }

  it("makbuzları çekirdekten okur: hedef, öğe, bayt, özet ve durum yazılır", async () => {
    // Sıra **çekirdekten** gelir (en yeni önce); ekran yeniden sıralamaz.
    const failed = receiptRow({
      id: "m1",
      status: "failed",
      outboundHash: "b".repeat(64),
      itemCount: 1,
      byteCount: 12,
    });
    const open = receiptRow({ id: "m2", status: "attempted", outboundHash: "c".repeat(64) });
    const sent = receiptRow({ id: "m3" });
    setup({ receipts: async () => [failed, open, sent] });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.receiptsTitle"));
    await within(card).findByText(t("ai.receiptFailed"));
    expect(mocks.aiReceiptList).toHaveBeenCalledWith("p1");

    const rows = within(card).getAllByRole("row").slice(1);
    expect(rows).toHaveLength(3);
    expect(rows[0]!.textContent).toContain(t("ai.receiptFailed"));
    expect(rows[0]!.textContent).toContain("api.ornek.com · claude-sonnet");
    expect(rows[0]!.textContent).toContain("b".repeat(64));
    expect(rows[1]!.textContent).toContain(t("ai.receiptAttempted"));
    expect(rows[2]!.textContent).toContain(t("ai.receiptDispatched"));
    expect(rows[2]!.textContent).toContain("4096");
    expect(rows[2]!.textContent).toContain(formatDateTime(sent.dispatchedAt));

    // Renk de sözleşmenin parçası: `attempted` **başarı** değildir.
    expect(within(rows[1]!).getByText(t("ai.receiptAttempted")).className).toContain("warn");
    expect(within(rows[2]!).getByText(t("ai.receiptDispatched")).className).toContain("ok");
    expect(within(rows[0]!).getByText(t("ai.receiptFailed")).className).toContain("danger");
  });

  it("makbuz yokken uydurmaz: boş satır yazar, tablo çizmez", async () => {
    setup();

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.receiptsTitle"));
    await within(card).findByText(t("ai.receiptsEmpty"));
    expect(within(card).queryByRole("table")).toBeNull();
  });

  it("dosya değişiminde önceki dosyanın makbuzları ekranda kalmaz", async () => {
    setup({
      receipts: async (caseId) =>
        caseId === "p2"
          ? [
              receiptRow({
                id: "m2",
                caseId: "p2",
                destinationLabel: "local:ollama · llama",
                taskType: "extract_events",
              }),
            ]
          : [receiptRow({ id: "m1" })],
    });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.receiptsTitle"));
    await within(card).findByText("api.ornek.com · claude-sonnet");

    switchCase("p2");
    await within(card).findByText("local:ollama · llama");
    expect(within(card).queryByText("api.ornek.com · claude-sonnet")).toBeNull();
    expect(mocks.aiReceiptList).toHaveBeenCalledWith("p2");
  });

  it("geç gelen makbuz yanıtı yeni dosyanın listesine yazılmaz", async () => {
    const pending = deferred<TransferReceiptRow[]>();
    setup({
      receipts: (caseId) =>
        caseId === "p1"
          ? pending.promise
          : Promise.resolve([
              receiptRow({
                id: "m2",
                caseId: "p2",
                destinationLabel: "local:ollama · llama",
              }),
            ]),
    });

    await screen.findByText("api:claude (api)");
    switchCase("p2");
    const card = cardByHeading(t("ai.receiptsTitle"));
    await within(card).findByText("local:ollama · llama");

    // p1'in okuması **sonra** döner: yeni dosyanın listesini ezmez (INV-05).
    await act(async () => {
      pending.resolve([receiptRow({ id: "m1", destinationLabel: "geç gelen hedef" })]);
      await Promise.resolve();
    });
    expect(within(card).queryByText("geç gelen hedef")).toBeNull();
    expect(within(card).getByText("local:ollama · llama")).toBeTruthy();
  });

  it("başarılı gönderimden sonra makbuzlar tazelenir ve yeni satır görünür", async () => {
    let rows: TransferReceiptRow[] = [];
    setup({ receipts: async () => rows });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.receiptsTitle"));
    await within(card).findByText(t("ai.receiptsEmpty"));

    rows = [receiptRow({ id: "m1" })];
    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));
    fireEvent.click(button(t("ai.run")));

    await within(card).findByText(t("ai.receiptDispatched"));
    expect(mocks.aiReceiptList.mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it("düşen gönderimde de makbuz tazelenir: «gönderilemedi» satırı görünür", async () => {
    // Eski davranış: makbuz yalnız başarıda yazılıyordu → düşen denemenin
    // sicilde hiç izi yoktu, ekranda da hiçbir şey görünmüyordu.
    let rows: TransferReceiptRow[] = [];
    setup({
      receipts: async () => rows,
      runExecute: async () => {
        rows = [receiptRow({ id: "m9", status: "failed", outboundHash: "d".repeat(64) })];
        throw { code: "E_AI_IO", message: "ağ hatası" };
      },
    });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.receiptsTitle"));
    await within(card).findByText(t("ai.receiptsEmpty"));

    fireEvent.click(button(t("ai.approve")));
    await waitFor(() => expect(button(t("ai.run")).disabled).toBe(false));
    fireEvent.click(button(t("ai.run")));

    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("E_AI_IO"));
    await within(card).findByText(t("ai.receiptFailed"));
    expect(within(card).getByText("d".repeat(64))).toBeTruthy();
  });

  it("makbuz okuması düşerse sessiz kalmaz", async () => {
    setup({
      receipts: async () => {
        throw { code: "E_DB_SQL", message: "makbuz okunamadı" };
      },
    });

    await waitFor(() =>
      expect(screen.getByRole("alert").textContent).toContain("E_DB_SQL"),
    );
    const card = cardByHeading(t("ai.receiptsTitle"));
    expect(within(card).queryByRole("table")).toBeNull();
  });

  it("okuma uçuşta iken «Yenile» kapalıdır", async () => {
    const pending = deferred<TransferReceiptRow[]>();
    let calls = 0;
    setup({
      receipts: () => {
        calls += 1;
        return calls === 1 ? Promise.resolve([receiptRow({ id: "m1" })]) : pending.promise;
      },
    });

    await screen.findByText("api:claude (api)");
    const card = cardByHeading(t("ai.receiptsTitle"));
    await within(card).findByText(t("ai.receiptDispatched"));

    const refresh = within(card).getByRole("button", {
      name: t("ai.receiptsRefresh"),
    }) as HTMLButtonElement;
    fireEvent.click(refresh);
    await waitFor(() => expect(calls).toBe(2));
    await waitFor(() => expect(refresh.disabled).toBe(true));
    expect(within(card).getByText(t("ai.receiptsLoading"))).toBeTruthy();

    await act(async () => {
      pending.resolve([receiptRow({ id: "m1" })]);
      await Promise.resolve();
    });
    await waitFor(() => expect(refresh.disabled).toBe(false));
  });
});
