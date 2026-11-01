// @vitest-environment jsdom
//
// Emsal (Precedents) ekranı davranış testi. Lib testleri (`precedents`,
// `precedent-terms`, `precedent-document`) metin parçalarını doğrular; burada
// 1453 satırlık ekran gerçekten render edilip ürün kararlarının arayüzde
// uygulandığı ölçülür. Ölçülen kararlar:
//
// 1. Kütüphane: satır sayısı sunucudan; her satırda yeniden dağıtım izni rozeti
//    (izinli / izinsiz / izin bilinmiyor) ve künye tamlığı; eksik başlık/tarih
//    uydurulmaz ("başlıksız", "tarih yok").
// 2. Karar ekleme: boş mahkeme **gönderilmez** (E_COURT); künye kırpılır, boş
//    isteğe bağlı alan `null` olur, etiketler virgülden ayrılıp boşlar atılır.
// 3. Kullanım koşulu: kayıt sunucudan forma yazılır; kaydetme tam sözleşmeyle
//    gider (bağlantı kırpılır, boşsa `null`), liste ve dağıtım kapsamı tazelenir.
//    "İzinli" işaretlemek yazılı dayanak ister (E_TERMS uyarısı).
// 4. Korpus kapısı: kapsam sunucudan; izinli < metinli ise kapı kapalı ve
//    dışlanma nedenleri yazılır; "tümü" kipi kapalıyken not düşülür; hedef
//    seçilmeden çekirdeğe gidilmez.
// 5. Bibliyografya: eksik alanlar, erişim tarihi ve dayanağı, tam metin durumu;
//    kopyalama panoya **gerçek metni** yazar, hata sessiz yutulmaz.
// 6. Tam metin: yapıştırılan metin **olduğu gibi** gider; sürümler/künye ve
//    dağıtım kapsamı tazelenir; metin değişince eski alıntı hükmü silinir.
// 7. Alıntı doğrulama: metin yoksa düğme kapalı; sonuç lib'in dürüst cümlesiyle
//    ve tarama notuyla yazılır; hüküm atıf kontrolü alanına taşınır.
// 8. Kasadan aktarım: belge/sürüm yoksa neden yazılır ve düğme kapalı; aktarım
//    seçilen sürümle gider; kısmi/OCR/metinsiz/aynı-metin uyarıları gizlenmez.
// 9. Toplu içe alma: seçilen dosyalar ve varsayılanlar tam sözleşmeyle gider;
//    rapor sayıları, liste hash'i ve çakışma notu sunucudan gelir.
// 10. Atıf kontrolü: künye ve alıntı boyutları ayrı yazılır; kontrol tam
//     sözleşmeyle kaydedilir (kontrol edilmedi → `text_not_checked`).
// 11. Anlam incelemesi: bağlam gerçek metinden gelir ve alıntı işaretlenir;
//     hüküm gerekçesiyle kaydedilir.
// 12. **Ölçülen kusur:** emsal değişince önceki emsalin notları, sürümleri,
//     künyesi ve koşul formu ekranda kalmamalı (INV-05). Kalmadığı ölçülür,
//     çünkü "Koşulu kaydet" o pencere içinde **önceki emsalin izin kaydını yeni
//     emsale yazıyordu**.
// 13. **Ölçülen kusur:** dosya değişince önceki dosyanın belge sürümü seçimi ve
//     atıf kontrolleri temizlenmeli; aksi hâlde kontrol **başka dosyanın
//     belgesine** yazılıyordu.
// 14. **Ölçülen kusur:** başarısız alıntı doğrulaması eski sonucu/hata
//     bildirimini bırakmaz (çelişkili ekran).

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { formatSize } from "../lib/format";
import { t } from "../lib/i18n";
import {
  accessBasisLabel,
  bibliographyMissingText,
  bulkOutcomeLabel,
  bulkReportSummary,
  quoteScanNote,
  quoteStateText,
  quoteToCitationMatch,
  versionSourceLabel,
  versionStateText,
} from "../lib/precedents";
import {
  corpusGateText,
  corpusResultText,
  coverageText,
  exclusionReasonLabel,
  redistributionLabel,
  termsRecordedText,
  usageBasisLabel,
} from "../lib/precedent-terms";
import {
  documentTextSummary,
  documentTextWarnings,
  transferBlockedReason,
  versionOptionLabel,
} from "../lib/precedent-document";
import type {
  BulkImportReport,
  CaseRow,
  CitationCheckRow,
  CitationContext,
  ConnectorCatalog,
  CorpusCoverage,
  DocumentRow,
  DocumentVersionRow,
  PrecedentBibliography,
  PrecedentDocumentText,
  PrecedentNoteRow,
  PrecedentRow,
  PrecedentUsageTerms,
  PrecedentVersionRow,
  QuoteVerificationRow,
} from "../lib/api";

const mocks = vi.hoisted(() => ({
  precedentList: vi.fn(),
  precedentCreate: vi.fn(),
  precedentNoteList: vi.fn(),
  precedentNoteCreate: vi.fn(),
  precedentTextAdd: vi.fn(),
  precedentVersionList: vi.fn(),
  precedentTextFromDocument: vi.fn(),
  precedentQuoteVerify: vi.fn(),
  precedentBibliography: vi.fn(),
  precedentTermsGet: vi.fn(),
  precedentTermsSet: vi.fn(),
  precedentCorpusCoverage: vi.fn(),
  precedentCorpusExport: vi.fn(),
  precedentBulkImport: vi.fn(),
  precedentConnectorList: vi.fn(),
  citationCheckList: vi.fn(),
  citationMeaningReview: vi.fn(),
  citationContext: vi.fn(),
  citationCheckCreate: vi.fn(),
  caseList: vi.fn(),
  documentList: vi.fn(),
  documentVersionList: vi.fn(),
  openDialog: vi.fn(),
  saveDialog: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      precedentList: mocks.precedentList,
      precedentCreate: mocks.precedentCreate,
      precedentNoteList: mocks.precedentNoteList,
      precedentNoteCreate: mocks.precedentNoteCreate,
      precedentTextAdd: mocks.precedentTextAdd,
      precedentVersionList: mocks.precedentVersionList,
      precedentTextFromDocument: mocks.precedentTextFromDocument,
      precedentQuoteVerify: mocks.precedentQuoteVerify,
      precedentBibliography: mocks.precedentBibliography,
      precedentTermsGet: mocks.precedentTermsGet,
      precedentTermsSet: mocks.precedentTermsSet,
      precedentCorpusCoverage: mocks.precedentCorpusCoverage,
      precedentCorpusExport: mocks.precedentCorpusExport,
      precedentBulkImport: mocks.precedentBulkImport,
      precedentConnectorList: mocks.precedentConnectorList,
      citationCheckList: mocks.citationCheckList,
      citationMeaningReview: mocks.citationMeaningReview,
      citationContext: mocks.citationContext,
      citationCheckCreate: mocks.citationCheckCreate,
      caseList: mocks.caseList,
      documentList: mocks.documentList,
      documentVersionList: mocks.documentVersionList,
    },
  };
});

// Tauri sınırı: jsdom'da dosya seçme/kaydetme penceresi yok.
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: mocks.openDialog,
  save: mocks.saveDialog,
}));

const { Precedents } = await import("./Precedents");

const ISO = "2026-05-01T10:00:00Z";

const PREC: PrecedentRow = {
  id: "p1",
  court: "Yargıtay",
  chamber: "1. Hukuk Dairesi",
  docketNo: "2024/123",
  decisionNo: "2025/456",
  applicationNo: null,
  decisionDate: "2025-03-12",
  title: "Kira bedelinin uyarlanması",
  sourceUrl: "https://karar.test/1",
  userTags: ["kira"],
  legalIssue: "Uyarlama",
  completeness: "complete",
  createdAt: ISO,
  usageBasis: "licence",
  redistribution: "allowed",
  termsRecordedAt: ISO,
};

/** İzinsiz kayıt: listede kırmızı rozet, dağıtıma giremez. */
const PREC2: PrecedentRow = {
  ...PREC,
  id: "p2",
  court: "Danıştay",
  chamber: null,
  docketNo: null,
  decisionNo: null,
  decisionDate: null,
  title: "İhtarın geçersizliği",
  sourceUrl: null,
  userTags: [],
  completeness: "incomplete",
  usageBasis: "unknown",
  redistribution: "not_allowed",
  termsRecordedAt: null,
};

/** İzni bilinmeyen, başlıksız kayıt: hiçbir alan uydurulmaz. */
const PREC3: PrecedentRow = {
  ...PREC,
  id: "p3",
  chamber: null,
  docketNo: null,
  decisionNo: null,
  decisionDate: null,
  title: null,
  sourceUrl: null,
  userTags: [],
  completeness: "incomplete",
  usageBasis: "unknown",
  redistribution: "unknown",
  termsRecordedAt: null,
};

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

const DOC: DocumentRow = {
  id: "d1",
  caseId: "c1",
  displayName: "Kira sözleşmesi.pdf",
  documentType: "pdf",
  status: "parsed",
  currentVersionId: "v1",
  declaredDocumentDate: null,
  createdAt: ISO,
  updatedAt: ISO,
  rowVersion: 1,
};

const DOC2: DocumentRow = {
  ...DOC,
  id: "d2",
  caseId: "c2",
  displayName: "İhtar mektubu.pdf",
  currentVersionId: "v2",
};

const VERSION: DocumentVersionRow = {
  id: "v1",
  documentId: "d1",
  caseId: "c1",
  versionNo: 1,
  blobId: "b1",
  originalSha256: "a".repeat(64),
  byteSize: 12345,
  detectedMime: "application/pdf",
  importedAt: ISO,
};

const NOTE: PrecedentNoteRow = {
  id: "n1",
  precedentId: "p1",
  caseIdOptional: null,
  usefulPassageAnchor: null,
  similarityNote: "A notu: benzer gerekçe",
  distinguishingNote: "Ayrışan: bedel farkı",
  userConclusion: "Uygulanabilir",
  createdAt: ISO,
};

const PV_TEXT: PrecedentVersionRow = {
  id: "pv1",
  precedentId: "p1",
  documentVersionId: null,
  acquiredAt: ISO,
  sourceKind: "paste",
  bodyHash: "abcdef0123456789",
  verificationState: "text_verified",
  charCount: 4321,
  hasText: true,
  identityConflict: false,
  conflictWithVersionId: null,
};

const PV_CONFLICT: PrecedentVersionRow = {
  ...PV_TEXT,
  id: "pv2",
  bodyHash: "fedcba9876543210",
  sourceKind: "document",
  identityConflict: true,
  conflictWithVersionId: "pv1",
  hasText: false,
  charCount: 0,
};

const TERMS_A: PrecedentUsageTerms = {
  precedentId: "p1",
  usageBasis: "licence",
  redistribution: "allowed",
  termsUrl: "https://a.test/kosul",
  termsNote: "A dayanağı",
  termsRecordedAt: ISO,
};

const BIB: PrecedentBibliography = {
  precedentId: "p1",
  text: "Yargıtay 1. HD, E. 2024/123, K. 2025/456, 12.03.2025",
  missing: ["docketNo"],
  warnings: ["künye kaynaktan doğrulandı"],
  sourceUrl: "https://karar.test/1",
  accessedAt: ISO,
  accessBasis: "record",
  hasFullText: false,
};

function coverage(overrides: Partial<CorpusCoverage> = {}): CorpusCoverage {
  return {
    total: 5,
    withText: 3,
    allowed: 1,
    notAllowed: 2,
    unknown: 2,
    blockedTotal: 2,
    blocked: [
      {
        precedentId: "p2",
        court: "Danıştay",
        docketNo: null,
        reason: "redistribution_not_allowed",
      },
      {
        precedentId: "p3",
        court: "Yargıtay",
        docketNo: "2024/9",
        reason: "redistribution_unknown",
      },
    ],
    ...overrides,
  };
}

const CATALOG: ConnectorCatalog = {
  catalogHash: "0123456789abcdef",
  connectors: [
    {
      id: "k1",
      name: "Ulusal Yargı Ağı",
      operator: "Adalet Bakanlığı",
      accessMethod: "Kurumsal erişim",
      termsUrl: "https://ornek.test/kosullar",
      scope: "Karar metinleri",
      rateLimit: "belirsiz",
      reuseNote: "Yeniden dağıtım yazılı izin ister",
      status: "documented_not_enabled",
      networkAllowed: false,
      blockedReason: "ağ erişimi kapalı",
    },
  ],
  note: "Sicil yalnız belge amaçlıdır.",
};

const CHECK: CitationCheckRow = {
  id: "ch1",
  inputDocumentVersionId: "v1",
  extractedCitation: "Yargıtay 1. HD 2024/123",
  candidatePrecedentId: null,
  identityMatch: "mismatch",
  quoteMatch: "not_checked",
  meaningReview: "not_reviewed",
  meaningNote: "",
  meaningReviewedAt: null,
  reasonCodes: ["text_not_checked"],
  createdAt: ISO,
};

const CONTEXT: CitationContext = {
  found: true,
  before: "… kararın önceki kısmı ",
  quote: "kira bedeli ödenmedi",
  after: " sonraki kısmı …",
  charPosition: 120,
  state: "exact",
  reason: null,
};

const VERIFY_MATCH: QuoteVerificationRow = {
  precedentId: "p1",
  versionId: "pv1",
  state: "match",
  method: "exact",
  matchedStart: 119,
  matchedLength: 18,
  occurrences: 1,
  reason: null,
  versionsScanned: 1,
  historicalVersionsScanned: 0,
  foundInOldVersion: false,
  historicalTruncated: false,
  quote: "kira bedeli ödenmedi",
  checkedAt: ISO,
};

const VERIFY_OLD: QuoteVerificationRow = {
  ...VERIFY_MATCH,
  matchedStart: null,
  matchedLength: null,
  occurrences: 0,
  method: "whitespace",
  versionsScanned: 3,
  historicalVersionsScanned: 2,
  foundInOldVersion: true,
};

const EXPORT_RESULT = {
  path: "/tmp/korpus.json",
  bytes: 2048,
  sha256: "0123456789abcdef",
  exported: 1,
  excluded: 2,
  contentHash: "c".repeat(64),
};

function transferred(overrides: Partial<PrecedentDocumentText> = {}): PrecedentDocumentText {
  return {
    version: PV_TEXT,
    parseRunId: "run-abcdef12",
    engine: "pymupdf",
    parseStatus: "partial",
    pages: 4,
    ocrPages: 2,
    emptyPages: 1,
    charCount: 4321,
    written: true,
    notes: ["kaynak: kasadaki belge"],
    ...overrides,
  };
}

const REPORT: BulkImportReport = {
  manifestHash: "0123456789abcdef",
  manifestWarnings: ["2 satır okundu"],
  items: [
    {
      fileName: "a.pdf",
      outcome: "created",
      precedentId: "p9",
      versionId: "pv9",
      bodyHash: "h1",
      charCount: 4200,
      needsReview: false,
      code: null,
      message: null,
    },
    {
      fileName: "b.pdf",
      outcome: "conflict",
      precedentId: null,
      versionId: null,
      bodyHash: null,
      charCount: 0,
      needsReview: true,
      code: null,
      message: null,
    },
    {
      fileName: "c.pdf",
      outcome: "error",
      precedentId: null,
      versionId: null,
      bodyHash: null,
      charCount: 0,
      needsReview: false,
      code: "E_IMPORT",
      message: "metin yok",
    },
  ],
  created: 1,
  duplicates: 0,
  conflicts: 1,
  failed: 1,
};

type SetupOptions = {
  precedents?: PrecedentRow[];
  cases?: CaseRow[];
  documents?: (caseId: string) => Promise<DocumentRow[]>;
  versions?: (documentId: string) => Promise<DocumentVersionRow[]>;
  notes?: (precedentId: string) => Promise<PrecedentNoteRow[]>;
  precedentVersions?: (precedentId: string) => Promise<PrecedentVersionRow[]>;
  bibliography?: (precedentId: string) => Promise<PrecedentBibliography>;
  terms?: (precedentId: string) => Promise<PrecedentUsageTerms>;
  coverage?: CorpusCoverage;
  checks?: (versionId: string) => Promise<CitationCheckRow[]>;
  openDialogResult?: string | string[] | null;
  saveDialogResult?: string | null;
};

function setup(options: SetupOptions = {}) {
  mocks.precedentList.mockResolvedValue(options.precedents ?? [PREC]);
  mocks.precedentNoteList.mockImplementation(options.notes ?? (async () => []));
  mocks.precedentVersionList.mockImplementation(options.precedentVersions ?? (async () => []));
  mocks.precedentBibliography.mockImplementation(options.bibliography ?? (async () => BIB));
  mocks.precedentTermsGet.mockImplementation(
    options.terms ??
      (async () => ({
        precedentId: "p1",
        usageBasis: "unknown" as const,
        redistribution: "unknown" as const,
        termsUrl: null,
        termsNote: "",
        termsRecordedAt: null,
      })),
  );
  mocks.precedentCorpusCoverage.mockResolvedValue(options.coverage ?? coverage());
  mocks.precedentConnectorList.mockResolvedValue(CATALOG);
  mocks.caseList.mockResolvedValue(options.cases ?? [CASE]);
  mocks.documentList.mockImplementation(options.documents ?? (async () => [DOC]));
  mocks.documentVersionList.mockImplementation(options.versions ?? (async () => [VERSION]));
  mocks.citationCheckList.mockImplementation(options.checks ?? (async () => []));
  mocks.citationContext.mockResolvedValue(CONTEXT);
  mocks.openDialog.mockResolvedValue(options.openDialogResult ?? null);
  mocks.saveDialog.mockResolvedValue(options.saveDialogResult ?? null);
  render(<Precedents />);
}

/** Kütüphaneden emsal seçer ve künye kartlarının açıldığını doğrular. */
async function selectPrecedent(title: string) {
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(title) }));
  expect(await screen.findByText(t("precedents.termsTitle"))).toBeTruthy();
}

/** Ekrandaki tüm `role=status` kutularının metni (doğrulama, korpus, toplu). */
function statusTexts(): string[] {
  return Array.from(document.querySelectorAll("[role='status']")).map(
    (el) => el.textContent ?? "",
  );
}

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) throw new Error(`#${id} yok`);
  return element as T;
}

describe("emsal ekranı davranışı", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("kütüphane sayısını ve izin rozetlerini sunucudan yazar; boş kütüphane açıkça söylenir", async () => {
    setup({ precedents: [PREC, PREC2, PREC3] });

    expect(await screen.findByText(t("precedents.libraryCount", { count: 3 }))).toBeTruthy();
    expect(screen.getByText(t("terms.badgeAllowed"))).toBeTruthy();
    expect(screen.getByText(t("terms.badgeNotAllowed"))).toBeTruthy();
    expect(screen.getByText(t("terms.badgeUnknown"))).toBeTruthy();
    // Künye tamlığı sunucudan gelir; eksik künye "complete" görünmez.
    expect(screen.getAllByText("complete")).toHaveLength(1);
    expect(screen.getAllByText("incomplete")).toHaveLength(2);

    const row = screen.getByRole("button", { name: /Kira bedelinin uyarlanması/ });
    expect(row.textContent).toContain(t("precedents.decisionShort", { no: "2025/456" }));
    // Başlığı/tarihi olmayan satırda uydurma yok.
    expect(
      screen.getByText(new RegExp(`${t("precedents.untitled")} · ${t("precedents.noDate")}`)),
    ).toBeTruthy();

    cleanup();
    setup({ precedents: [] });
    expect(await screen.findByText(t("precedents.libraryCount", { count: 0 }))).toBeTruthy();
    expect(screen.getByText(t("precedents.emptyLibrary"))).toBeTruthy();
  });

  it("boş mahkeme gönderilmez; künye kırpılır, boş alanlar null, etiketler ayrılır", async () => {
    mocks.precedentCreate.mockResolvedValue({ ...PREC, id: "p9" });
    setup({ precedents: [PREC] });
    await screen.findByRole("button", { name: new RegExp(PREC.title!) });

    // Varsayılan mahkeme dolu gelir; boşaltıp gönderilmediğini ölçüyoruz.
    fireEvent.change(byId<HTMLInputElement>("p-court"), { target: { value: "   " } });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.add") }));
    expect(await screen.findByText("E_COURT")).toBeTruthy();
    expect(screen.getByText(t("precedents.errorCourt"), { exact: false })).toBeTruthy();
    expect(mocks.precedentCreate).not.toHaveBeenCalled();

    fireEvent.change(byId<HTMLInputElement>("p-court"), { target: { value: "  Danıştay  " } });
    fireEvent.change(byId<HTMLInputElement>("p-chamber"), { target: { value: " 10. Daire " } });
    fireEvent.change(byId<HTMLInputElement>("p-docket"), { target: { value: "  " } });
    fireEvent.change(byId<HTMLInputElement>("p-decision"), { target: { value: " 2025/9 " } });
    fireEvent.change(byId<HTMLInputElement>("p-date"), { target: { value: "2025-02-03" } });
    fireEvent.change(byId<HTMLInputElement>("p-title"), { target: { value: "  Yeni karar  " } });
    fireEvent.change(byId<HTMLInputElement>("p-url"), { target: { value: "  " } });
    fireEvent.change(byId<HTMLInputElement>("p-tags"), {
      target: { value: " kira, ,temerrüt " },
    });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.add") }));

    await waitFor(() => expect(mocks.precedentCreate).toHaveBeenCalledTimes(1));
    expect(mocks.precedentCreate).toHaveBeenCalledWith({
      court: "Danıştay",
      chamber: "10. Daire",
      docketNo: null,
      decisionNo: "2025/9",
      applicationNo: null,
      decisionDate: "2025-02-03",
      title: "Yeni karar",
      sourceUrl: null,
      userTags: ["kira", "temerrüt"],
      legalIssue: null,
    });
    // Liste tazelendi ve form varsayılana döndü; eski hata bildirimi kalmaz.
    await waitFor(() => expect(mocks.precedentList).toHaveBeenCalledTimes(2));
    expect(byId<HTMLInputElement>("p-court").value).toBe(t("precedents.defaultCourt"));
    expect(byId<HTMLInputElement>("p-title").value).toBe("");
    await waitFor(() => expect(screen.queryByText("E_COURT")).toBeNull());
  });

  it("koşul kaydı forma sunucudan yazılır; kaydetme kırpılmış tam sözleşmeyle gider ve izin dayanak ister", async () => {
    mocks.precedentTermsSet.mockImplementation(
      async (
        precedentId: string,
        input: { usageBasis: string; redistribution: string; termsUrl: string | null; termsNote: string },
      ) => ({
        precedentId,
        ...input,
        termsRecordedAt: "2026-06-01T09:00:00Z",
      }),
    );
    setup({ terms: async () => TERMS_A });

    await selectPrecedent(PREC.title!);
    await waitFor(() => expect(byId<HTMLSelectElement>("t-basis").value).toBe("licence"));
    expect(byId<HTMLSelectElement>("t-redistribution").value).toBe("allowed");
    expect(byId<HTMLInputElement>("t-url").value).toBe("https://a.test/kosul");
    expect(byId<HTMLTextAreaElement>("t-note").value).toBe("A dayanağı");
    expect(screen.getByText(new RegExp(termsRecordedText(TERMS_A)))).toBeTruthy();
    // "İzinli" + yazılı dayanak var → E_TERMS uyarısı yok.
    expect(screen.queryByText("E_TERMS")).toBeNull();

    fireEvent.change(byId<HTMLInputElement>("t-url"), {
      target: { value: "  https://ornek.test/kosul  " },
    });
    fireEvent.change(byId<HTMLTextAreaElement>("t-note"), {
      target: { value: "  yazılı izin  " },
    });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.termsSave") }));

    await waitFor(() => expect(mocks.precedentTermsSet).toHaveBeenCalledTimes(1));
    expect(mocks.precedentTermsSet).toHaveBeenCalledWith("p1", {
      usageBasis: "licence",
      redistribution: "allowed",
      termsUrl: "https://ornek.test/kosul",
      termsNote: "yazılı izin",
    });
    // Liste ve dağıtım kapsamı tazelenir (rozet sayıları bayat kalmasın).
    await waitFor(() => expect(mocks.precedentList).toHaveBeenCalledTimes(2));
    expect(mocks.precedentCorpusCoverage).toHaveBeenCalledTimes(2);
    expect(byId<HTMLInputElement>("t-url").value).toBe("https://ornek.test/kosul");

    // Dayanak boşaltılınca "izinli" işaretlemenin E_TERMS ile reddedileceği yazılır.
    fireEvent.change(byId<HTMLInputElement>("t-url"), { target: { value: "  " } });
    fireEvent.change(byId<HTMLTextAreaElement>("t-note"), { target: { value: "   " } });
    expect(await screen.findByText("E_TERMS")).toBeTruthy();
    expect(
      screen.getByText(
        new RegExp(`${usageBasisLabel("licence")} · ${redistributionLabel("allowed")}`),
      ),
    ).toBeTruthy();
  });

  it("korpus kapısı kapsam sunucudan kapalıyken dışlanma nedenlerini yazar; hedef seçilmeden çekirdeğe gitmez", async () => {
    const cov = coverage();
    setup({ coverage: cov });

    expect(await screen.findByText(coverageText(cov))).toBeTruthy();
    expect(screen.getByText(corpusGateText(cov))).toBeTruthy();
    expect(screen.getByText(t("precedents.corpusGateClosed"))).toBeTruthy();
    expect(screen.getByText(exclusionReasonLabel("redistribution_not_allowed"))).toBeTruthy();
    expect(screen.getByText(exclusionReasonLabel("redistribution_unknown"))).toBeTruthy();

    fireEvent.change(byId<HTMLSelectElement>("corpus-mode"), { target: { value: "all" } });
    expect(
      await screen.findByText(t("precedents.corpusClosedNote", { blocked: cov.blockedTotal })),
    ).toBeTruthy();

    // Kullanıcı kaydetme penceresini iptal ederse çekirdeğe istek gitmez.
    fireEvent.click(screen.getByRole("button", { name: t("precedents.corpusExport") }));
    await waitFor(() => expect(mocks.saveDialog).toHaveBeenCalledTimes(1));
    expect(mocks.precedentCorpusExport).not.toHaveBeenCalled();
  });

  it("korpus dışa aktarımı seçilen kiple çalışır; izinli metin yoksa düğme kapalıdır", async () => {
    mocks.precedentCorpusExport.mockResolvedValue(EXPORT_RESULT);
    setup({ coverage: coverage(), saveDialogResult: "/tmp/korpus.json" });

    await screen.findByText(coverageText(coverage()));
    fireEvent.change(byId<HTMLSelectElement>("corpus-mode"), { target: { value: "all" } });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.corpusExport") }));

    await waitFor(() =>
      expect(mocks.precedentCorpusExport).toHaveBeenCalledWith("all", "/tmp/korpus.json"),
    );
    expect(await screen.findByText(corpusResultText(EXPORT_RESULT))).toBeTruthy();
    expect(
      screen.getByText(`${EXPORT_RESULT.path} · ${formatSize(EXPORT_RESULT.bytes)}`),
    ).toBeTruthy();
    // Kapsam yeniden okunur.
    expect(mocks.precedentCorpusCoverage).toHaveBeenCalledTimes(2);

    // Hiç izinli metin yoksa "tümü" kipi de kapalıdır: düğme sunucuya gitmez.
    cleanup();
    const emptyCoverage = coverage({ allowed: 0 });
    setup({ coverage: emptyCoverage });
    await screen.findByText(coverageText(emptyCoverage));
    const button = screen.getByRole("button", {
      name: t("precedents.corpusExport"),
    }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("bibliyografya eksik alanları, erişim dayanağını ve tam metin durumunu yazar; kopyalama panoya gider", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    setup();

    await selectPrecedent(PREC.title!);
    expect(await screen.findByText(BIB.text)).toBeTruthy();
    expect(screen.getByText(bibliographyMissingText(BIB.missing))).toBeTruthy();
    expect(screen.getByText(t("precedents.bibAccessed", { at: BIB.accessedAt }))).toBeTruthy();
    expect(screen.getByText(t("precedents.bibNoText"))).toBeTruthy();
    expect(screen.getByText(accessBasisLabel(BIB.accessBasis))).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: t("precedents.bibCopy") }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith(BIB.text));
    expect(await screen.findByRole("button", { name: t("precedents.bibCopied") })).toBeTruthy();

    // Pano reddederse sessiz kalınmaz: gerçek hata kodu yazılır.
    cleanup();
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: vi.fn().mockRejectedValue(new Error("izin yok")) },
      configurable: true,
    });
    setup();
    await selectPrecedent(PREC.title!);
    fireEvent.click(await screen.findByRole("button", { name: t("precedents.bibCopy") }));
    expect(await screen.findByText("E_CLIPBOARD")).toBeTruthy();
    expect(screen.getByText(t("precedents.errorClipboard"), { exact: false })).toBeTruthy();
  });

  it("tam metin olduğu gibi kaydedilir; sürümler, künye ve kapsam tazelenir, eski alıntı hükmü silinir", async () => {
    mocks.precedentQuoteVerify.mockResolvedValue(VERIFY_MATCH);
    mocks.precedentTextAdd.mockResolvedValue({ ...PV_TEXT, id: "pv2" });
    setup({ precedentVersions: async () => [PV_TEXT] });

    await selectPrecedent(PREC.title!);
    await waitFor(() =>
      expect(screen.getByText(t("precedents.textSub", { versions: 1, withText: 1 }))).toBeTruthy(),
    );

    fireEvent.change(byId<HTMLInputElement>("p-quote"), {
      target: { value: "kira bedeli ödenmedi" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.quoteVerify") }));
    await waitFor(() =>
      expect(statusTexts().some((text) => text.includes(quoteStateText(VERIFY_MATCH)))).toBe(true),
    );

    fireEvent.change(byId<HTMLTextAreaElement>("p-fulltext"), {
      target: { value: "  Yeni tam metin  " },
    });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.fullTextSave") }));

    await waitFor(() => expect(mocks.precedentTextAdd).toHaveBeenCalledTimes(1));
    // Metin kırpılmaz: yapıştırılan hâli kaydedilir.
    expect(mocks.precedentTextAdd).toHaveBeenCalledWith("p1", {
      sourceKind: "paste",
      text: "  Yeni tam metin  ",
      documentVersionId: null,
    });
    expect(byId<HTMLTextAreaElement>("p-fulltext").value).toBe("");
    // Metin değişti: önceki alıntı doğrulaması artık bu sürüme ait değil.
    await waitFor(() =>
      expect(statusTexts().some((text) => text.includes(quoteStateText(VERIFY_MATCH)))).toBe(false),
    );
    expect(mocks.precedentVersionList).toHaveBeenCalledTimes(2);
    expect(mocks.precedentBibliography).toHaveBeenCalledTimes(2);
    expect(mocks.precedentCorpusCoverage).toHaveBeenCalledTimes(2);
  });

  it("alıntı doğrulama metin yokken kapalıdır; sonucu dürüst cümleyle yazar ve atıf alanına taşır", async () => {
    setup({ precedentVersions: async () => [PV_CONFLICT] });
    await selectPrecedent(PREC.title!);
    await waitFor(() =>
      expect(screen.getAllByText(versionStateText(PV_CONFLICT)).length).toBeGreaterThan(0),
    );
    expect(
      (screen.getByRole("button", { name: t("precedents.quoteVerify") }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);

    cleanup();
    mocks.precedentQuoteVerify.mockResolvedValue(VERIFY_OLD);
    setup({ precedentVersions: async () => [PV_TEXT] });
    await selectPrecedent(PREC.title!);
    await waitFor(() =>
      expect(screen.getByText(versionSourceLabel(PV_TEXT), { exact: false })).toBeTruthy(),
    );

    const verify = screen.getByRole("button", {
      name: t("precedents.quoteVerify"),
    }) as HTMLButtonElement;
    // Girdi boşken de kapalı: boş alıntı sunucuya gitmez.
    expect(verify.disabled).toBe(true);

    fireEvent.change(byId<HTMLInputElement>("p-quote"), {
      target: { value: "kira bedeli ödenmedi" },
    });
    fireEvent.click(verify);
    await waitFor(() =>
      expect(mocks.precedentQuoteVerify).toHaveBeenCalledWith("p1", "kira bedeli ödenmedi"),
    );
    await waitFor(() =>
      expect(statusTexts().some((text) => text.includes(quoteStateText(VERIFY_OLD)))).toBe(true),
    );
    // Tarama kapsamı (eski sürümler dahil) gizlenmez.
    expect(statusTexts().some((text) => text.includes(quoteScanNote(VERIFY_OLD)!))).toBe(true);
    // "Bulunamadı" hükmü tek başına "uydurulmuş" değildir; ekran da bunu yazar.
    expect(screen.getByText(t("precedents.quoteHint"))).toBeTruthy();

    fireEvent.click(
      screen.getByRole("button", {
        name: t("precedents.quoteToCitation", { state: quoteToCitationMatch(VERIFY_OLD) }),
      }),
    );
    expect(byId<HTMLSelectElement>("c-quote").value).toBe("match");
  });

  it("kasadan aktarım: engel nedeni yazılır, seçilen sürümle gider, kısmi/OCR uyarıları gizlenmez", async () => {
    // (a) Kasadaki dosyada belge yok → düğme kapalı, nedeni yazılı.
    setup({ documents: async () => [] });
    await selectPrecedent(PREC.title!);
    const blocked = transferBlockedReason({
      hasPrecedent: true,
      documentCount: 0,
      versionCount: 0,
      selectedVersionId: "",
    });
    expect(blocked).not.toBeNull();
    expect(screen.getByText(blocked!)).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: t("precedents.transferRun") }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);

    // (b) Belge + sürüm var → seçilen sürümle aktarılır.
    cleanup();
    mocks.precedentTextFromDocument.mockResolvedValue(transferred());
    let resolveSourceDocs: (rows: DocumentRow[]) => void = () => undefined;
    const pendingSourceDocs = new Promise<DocumentRow[]>((resolve) => {
      resolveSourceDocs = resolve;
    });
    setup({
      precedentVersions: async () => [PV_TEXT],
      cases: [CASE, CASE2],
      documents: (caseId) => (caseId === "c1" ? Promise.resolve([DOC]) : pendingSourceDocs),
    });
    await selectPrecedent(PREC.title!);
    await waitFor(() => expect(mocks.documentVersionList).toHaveBeenCalledWith("d1"));
    const run = screen.getByRole("button", {
      name: t("precedents.transferRun"),
    }) as HTMLButtonElement;
    expect(run.disabled).toBe(false);
    const option = versionOptionLabel(VERSION);
    expect(await screen.findByText(option)).toBeTruthy();

    fireEvent.click(run);
    await waitFor(() => expect(mocks.precedentTextFromDocument).toHaveBeenCalledWith("p1", "v1"));
    const result = transferred();
    expect(await screen.findByText(documentTextSummary(result))).toBeTruthy();
    for (const warning of documentTextWarnings(result)) {
      expect(screen.getByText(warning)).toBeTruthy();
    }
    expect(screen.getByText(t("precedents.transferWritten"))).toBeTruthy();
    expect(screen.getByText(/run-abcd/)).toBeTruthy();

    // Kaynak dosya değişince belge/sürüm seçimi eskisinden devralınmaz:
    // aktarım yanlış dosyanın sürümüyle başlamaz (düğme kapalı).
    fireEvent.change(byId<HTMLSelectElement>("src-case"), { target: { value: "c2" } });
    await waitFor(() => expect(byId<HTMLSelectElement>("src-doc").value).toBe(""));
    expect(
      (screen.getByRole("button", { name: t("precedents.transferRun") }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    resolveSourceDocs([DOC2]);
    await waitFor(() => expect(byId<HTMLSelectElement>("src-doc").value).toBe("d2"));
  });

  it("toplu içe alma seçilen dosyaları ve varsayılanları gönderir; raporu sayılarıyla yazar", async () => {
    mocks.precedentBulkImport.mockResolvedValue(REPORT);
    setup({ openDialogResult: ["/tmp/a.pdf", "/tmp/b.pdf"] });
    await screen.findByText(t("precedents.libraryCount", { count: 1 }));

    fireEvent.click(screen.getByRole("button", { name: t("precedents.bulkPick") }));
    expect(
      await screen.findByRole("button", { name: t("precedents.bulkPicked", { count: 2 }) }),
    ).toBeTruthy();

    fireEvent.change(byId<HTMLInputElement>("b-chamber"), { target: { value: " 9. Daire " } });
    fireEvent.change(byId<HTMLInputElement>("b-tags"), { target: { value: " kira, " } });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.bulkRun") }));

    await waitFor(() => expect(mocks.precedentBulkImport).toHaveBeenCalledTimes(1));
    expect(mocks.precedentBulkImport).toHaveBeenCalledWith({
      files: ["/tmp/a.pdf", "/tmp/b.pdf"],
      manifestPath: null,
      defaults: {
        court: t("precedents.defaultCourt"),
        chamber: "9. Daire",
        sourceUrl: "",
        tags: ["kira"],
        legalIssue: "",
      },
    });

    const summary = bulkReportSummary(REPORT);
    expect(
      await screen.findByText(summary, { exact: false }),
    ).toBeTruthy();
    expect(
      screen.getByText(
        t("precedents.bulkManifestHash", { hash: REPORT.manifestHash!.slice(0, 12) }),
        { exact: false },
      ),
    ).toBeTruthy();
    for (const item of REPORT.items) {
      expect(screen.getByText(item.fileName)).toBeTruthy();
      expect(screen.getAllByText(bulkOutcomeLabel(item)).length).toBeGreaterThan(0);
    }
    // Çakışan nüsha sessizce birleştirilmedi: not yazılır.
    expect(screen.getByText(t("precedents.bulkConflictNote"))).toBeTruthy();
    expect(screen.getByText(REPORT.manifestWarnings[0]!)).toBeTruthy();
    // Yeni kayıtlar için liste tazelenir.
    await waitFor(() => expect(mocks.precedentList).toHaveBeenCalledTimes(2));
  });

  it("atıf kontrolü boyutları ayrı yazar; boş atıf gönderilmez, kontrol tam sözleşmeyle kaydedilir", async () => {
    mocks.citationCheckCreate.mockResolvedValue(CHECK);
    setup({ checks: async () => [CHECK] });

    await waitFor(() => expect(mocks.citationCheckList).toHaveBeenCalledWith("v1"));
    expect(await screen.findByText(t("precedents.checkSub", { count: 1 }))).toBeTruthy();
    expect(screen.getByText(CHECK.extractedCitation)).toBeTruthy();
    expect(screen.getByText(t("precedents.badgeIdentity", { value: "mismatch" }))).toBeTruthy();
    expect(screen.getByText(t("precedents.badgeQuote", { value: "not_checked" }))).toBeTruthy();
    expect(
      screen.getByText(t("precedents.badgeMeaning", { value: t("precedents.meaningNotReviewed") })),
    ).toBeTruthy();

    // Boş atıf sunucuya gitmez.
    fireEvent.click(screen.getByRole("button", { name: t("precedents.checkSave") }));
    expect(await screen.findByText("E_CITATION")).toBeTruthy();
    expect(mocks.citationCheckCreate).not.toHaveBeenCalled();

    fireEvent.change(byId<HTMLInputElement>("c-text"), {
      target: { value: "  Yargıtay 1. HD 2024/123  " },
    });
    fireEvent.change(byId<HTMLSelectElement>("c-id"), { target: { value: "mismatch" } });
    fireEvent.change(byId<HTMLSelectElement>("c-quote"), { target: { value: "not_checked" } });
    fireEvent.change(byId<HTMLSelectElement>("c-cand"), { target: { value: "p1" } });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.checkSave") }));

    await waitFor(() => expect(mocks.citationCheckCreate).toHaveBeenCalledTimes(1));
    expect(mocks.citationCheckCreate).toHaveBeenCalledWith({
      inputDocumentVersionId: "v1",
      extractedCitation: "Yargıtay 1. HD 2024/123",
      candidatePrecedentId: "p1",
      identityMatch: "mismatch",
      quoteMatch: "not_checked",
      meaningReview: "not_reviewed",
      // Alıntı "kontrol edilmedi" ise nedeni kayda yazılır; sessizce "match" sayılmaz.
      reasonCodes: ["text_not_checked"],
    });
    expect(byId<HTMLInputElement>("c-text").value).toBe("");
    expect(mocks.citationCheckList).toHaveBeenCalledTimes(2);
    await waitFor(() => expect(screen.queryByText("E_CITATION")).toBeNull());

    // Alıntı boyutu "match" ise neden kodu yazılmaz.
    fireEvent.change(byId<HTMLInputElement>("c-text"), {
      target: { value: "Yargıtay 1. HD 2024/124" },
    });
    fireEvent.change(byId<HTMLSelectElement>("c-quote"), { target: { value: "match" } });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.checkSave") }));
    await waitFor(() => expect(mocks.citationCheckCreate).toHaveBeenCalledTimes(2));
    expect(mocks.citationCheckCreate.mock.calls[1]![0]).toMatchObject({
      quoteMatch: "match",
      reasonCodes: [],
    });
  });

  it("anlam incelemesi bağlamı gerçek metinden gösterir ve hükmü gerekçesiyle kaydeder", async () => {
    mocks.citationMeaningReview.mockResolvedValue({ ...CHECK, meaningReview: "misleading" });
    setup({ checks: async () => [CHECK] });

    await waitFor(() => expect(mocks.citationCheckList).toHaveBeenCalledWith("v1"));
    fireEvent.click(await screen.findByRole("button", { name: t("precedents.meaning") }));

    await waitFor(() => expect(mocks.citationContext).toHaveBeenCalledWith("ch1"));
    expect(await screen.findByText(CONTEXT.quote, { selector: "mark" })).toBeTruthy();
    expect(screen.getByText(new RegExp(CONTEXT.after))).toBeTruthy();

    fireEvent.change(byId<HTMLInputElement>("mn-ch1"), {
      target: { value: "karar tersine çevrilmiş" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.meaningMisleading") }));

    await waitFor(() =>
      expect(mocks.citationMeaningReview).toHaveBeenCalledWith(
        "ch1",
        "misleading",
        "karar tersine çevrilmiş",
      ),
    );
    // Panel kapanır ve liste tazelenir.
    await waitFor(() =>
      expect(screen.getByRole("button", { name: t("precedents.meaning") })).toBeTruthy(),
    );
    expect(mocks.citationCheckList).toHaveBeenCalledTimes(2);
    expect(screen.queryByText(t("precedents.contextLoading"))).toBeNull();

    // Bağlam bulunamazsa uydurulmaz: neden yazılır.
    cleanup();
    setup({ checks: async () => [CHECK] });
    mocks.citationContext.mockResolvedValue({
      ...CONTEXT,
      found: false,
      before: "",
      quote: "",
      after: "",
      charPosition: null,
      state: "missing",
      reason: "text_not_found",
    });
    await waitFor(() => expect(mocks.citationCheckList).toHaveBeenCalled());
    fireEvent.click(await screen.findByRole("button", { name: t("precedents.meaning") }));
    expect(
      await screen.findByText(t("precedents.contextMissing", { reason: "text_not_found" }), {
        exact: false,
      }),
    ).toBeTruthy();
  });

  it("emsal değişince önceki emsalin notu, sürümü, künyesi ve koşul formu ekranda kalmaz", async () => {
    // B'nin dört isteği askıda: temizlik penceresi yarışa bırakılmadan ölçülür.
    let resolveTerms: (value: PrecedentUsageTerms) => void = () => undefined;
    const pendingTerms = new Promise<PrecedentUsageTerms>((resolve) => {
      resolveTerms = resolve;
    });
    mocks.precedentTermsSet.mockImplementation(async (precedentId: string) => ({
      precedentId,
      usageBasis: "unknown" as const,
      redistribution: "unknown" as const,
      termsUrl: null,
      termsNote: "",
      termsRecordedAt: null,
    }));
    setup({
      precedents: [PREC, PREC2],
      notes: async (id) => (id === "p1" ? [NOTE] : new Promise<PrecedentNoteRow[]>(() => undefined)),
      precedentVersions: async (id) =>
        id === "p1" ? [PV_TEXT] : new Promise<PrecedentVersionRow[]>(() => undefined),
      bibliography: async (id) =>
        id === "p1" ? BIB : new Promise<PrecedentBibliography>(() => undefined),
      terms: async (id) => (id === "p1" ? TERMS_A : pendingTerms),
    });

    await selectPrecedent(PREC.title!);
    expect(await screen.findByText(NOTE.similarityNote)).toBeTruthy();
    expect(await screen.findByText(versionStateText(PV_TEXT))).toBeTruthy();
    expect(await screen.findByText(BIB.text)).toBeTruthy();
    await waitFor(() => expect(byId<HTMLTextAreaElement>("t-note").value).toBe("A dayanağı"));

    fireEvent.click(screen.getByRole("button", { name: /İhtarın geçersizliği/ }));

    // Yeni emsalin verisi gelmeden önceki emsalin verisi ekranda olmamalı.
    expect(await screen.findByText(t("precedents.notesSub", { count: 0 }))).toBeTruthy();
    expect(screen.queryAllByText(NOTE.similarityNote)).toHaveLength(0);
    expect(screen.queryAllByText(versionStateText(PV_TEXT))).toHaveLength(0);
    expect(screen.queryAllByText(BIB.text)).toHaveLength(0);
    expect(screen.getByText(t("precedents.textEmpty"))).toBeTruthy();
    expect(screen.getByText(t("precedents.bibLoading"))).toBeTruthy();

    // Koşul formu da önceki emsalin kaydını taşımaz: "Kaydet" yanlış emsale
    // başka bir kararın izin kaydını yazmamalı.
    expect(byId<HTMLSelectElement>("t-basis").value).toBe("unknown");
    expect(byId<HTMLSelectElement>("t-redistribution").value).toBe("unknown");
    expect(byId<HTMLInputElement>("t-url").value).toBe("");
    expect(byId<HTMLTextAreaElement>("t-note").value).toBe("");
    fireEvent.click(screen.getByRole("button", { name: t("precedents.termsSave") }));
    await waitFor(() =>
      expect(mocks.precedentTermsSet).toHaveBeenCalledWith("p2", {
        usageBasis: "unknown",
        redistribution: "unknown",
        termsUrl: null,
        termsNote: "",
      }),
    );

    // Yeni emsalin verisi gelince kartlar gerçekten dolar (temizlik yüklemeyi kesmez).
    resolveTerms({
      precedentId: "p2",
      usageBasis: "unknown",
      redistribution: "not_allowed",
      termsUrl: null,
      termsNote: "B dayanağı",
      termsRecordedAt: null,
    });
    await waitFor(() => expect(byId<HTMLTextAreaElement>("t-note").value).toBe("B dayanağı"));
    expect(byId<HTMLSelectElement>("t-redistribution").value).toBe("not_allowed");
  });

  it("dosya değişince belge sürümü seçimi ve atıf kontrolleri temizlenir; kontrol eski belgeye yazılmaz", async () => {
    let resolveDocs: (rows: DocumentRow[]) => void = () => undefined;
    const pendingDocs = new Promise<DocumentRow[]>((resolve) => {
      resolveDocs = resolve;
    });
    setup({
      cases: [CASE, CASE2],
      documents: (caseId) => (caseId === "c1" ? Promise.resolve([DOC]) : pendingDocs),
      checks: async (versionId) => (versionId === "v1" ? [CHECK] : []),
    });

    await waitFor(() => expect(mocks.citationCheckList).toHaveBeenCalledWith("v1"));
    expect(await screen.findByText(CHECK.extractedCitation)).toBeTruthy();
    expect(byId<HTMLSelectElement>("c-doc").value).toBe("v1");

    fireEvent.change(byId<HTMLSelectElement>("c-case"), { target: { value: "c2" } });

    // Yeni dosyanın belgeleri henüz gelmedi: eski sürüm seçili kalmamalı.
    await waitFor(() => expect(byId<HTMLSelectElement>("c-doc").value).toBe(""));
    expect(screen.queryAllByText(CHECK.extractedCitation)).toHaveLength(0);

    // Bu pencerede "Kontrolü kaydet" eski dosyanın belgesine yazmaz.
    fireEvent.change(byId<HTMLInputElement>("c-text"), {
      target: { value: "Yargıtay 1. HD 2024/123" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.checkSave") }));
    expect(await screen.findByText("E_CITATION")).toBeTruthy();
    expect(mocks.citationCheckCreate).not.toHaveBeenCalled();

    // Yeni dosyanın belgesi gelince seçim gerçek sürüme geçer.
    resolveDocs([DOC2]);
    await waitFor(() => expect(byId<HTMLSelectElement>("c-doc").value).toBe("v2"));
  });

  it("başarısız alıntı doğrulaması eski sonucu bırakmaz; başarılı doğrulama eski hatayı siler", async () => {
    mocks.precedentQuoteVerify
      .mockResolvedValueOnce(VERIFY_MATCH)
      .mockRejectedValueOnce({
        code: "E_QUOTE_VERIFY",
        message: "doğrulama hatası",
        retryable: true,
      })
      .mockResolvedValue(VERIFY_OLD);
    setup({ precedentVersions: async () => [PV_TEXT] });

    await selectPrecedent(PREC.title!);
    await waitFor(() => expect(screen.getByText(versionStateText(PV_TEXT))).toBeTruthy());

    fireEvent.change(byId<HTMLInputElement>("p-quote"), {
      target: { value: "kira bedeli ödenmedi" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.quoteVerify") }));
    await waitFor(() =>
      expect(statusTexts().some((text) => text.includes(quoteStateText(VERIFY_MATCH)))).toBe(true),
    );

    // Yeni alıntı doğrulanamadı: eski hüküm ekranda kalmamalı (başka metne ait).
    fireEvent.change(byId<HTMLInputElement>("p-quote"), {
      target: { value: "başka bir cümle" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("precedents.quoteVerify") }));
    expect(await screen.findByText("E_QUOTE_VERIFY")).toBeTruthy();
    expect(statusTexts().some((text) => text.includes(quoteStateText(VERIFY_MATCH)))).toBe(false);

    // Başarılı doğrulama eski hata bildirimini bırakmaz (çelişkili ekran olmaz).
    fireEvent.click(screen.getByRole("button", { name: t("precedents.quoteVerify") }));
    await waitFor(() =>
      expect(statusTexts().some((text) => text.includes(quoteStateText(VERIFY_OLD)))).toBe(true),
    );
    expect(screen.queryByText("E_QUOTE_VERIFY")).toBeNull();
  });
});
