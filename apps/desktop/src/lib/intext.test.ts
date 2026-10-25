import { describe, expect, it } from "vitest";
import type { InTextQuoteCheck, InTextQuoteFinding, InTextQuotePosition } from "./api";
import {
  bindingInput,
  canBind,
  candidateLabel,
  positionLabel,
  quoteCheckSummary,
  QUOTE_STATE_LABEL,
  quoteStateTone,
  scopeLabel,
  skippedLabel,
} from "./intext";

function position(overrides: Partial<InTextQuotePosition> = {}): InTextQuotePosition {
  return {
    documentId: "doc1",
    documentName: "Dilekçe",
    documentVersionId: "ver1",
    parseRunId: "run1",
    pageId: "page1",
    segmentId: "seg1",
    pageIndex: 0,
    charStart: 19,
    charEnd: 56,
    matchKind: "exact",
    textOrigin: "native",
    sourceText: "kira bedelini ödemediğini tespit etti",
    ...overrides,
  };
}

function finding(overrides: Partial<InTextQuoteFinding> = {}): InTextQuoteFinding {
  return {
    index: 0,
    quote: "kira bedelini ödemediğini tespit etti",
    state: "exact",
    reason: null,
    occurrences: 1,
    distinctDocuments: 1,
    position: position(),
    candidates: [],
    ...overrides,
  };
}

function check(overrides: Partial<InTextQuoteCheck> = {}): InTextQuoteCheck {
  return {
    caseId: "c1",
    scope: "case",
    scopeDocumentId: null,
    scopeDocumentName: null,
    textChars: 80,
    spansFound: 1,
    skippedShort: 0,
    skippedLong: 0,
    unclosed: 0,
    quotes: [finding()],
    exact: 1,
    normalized: 0,
    ambiguous: 0,
    notFound: 0,
    unverifiable: 0,
    documentsTotal: 2,
    documentsWithText: 2,
    documentsWithoutText: 0,
    pagesScanned: 4,
    notes: [],
    checkedAt: "2026-10-25T10:00:00Z",
    ...overrides,
  };
}

describe("metin içi alıntı yardımcıları", () => {
  it("her durum için Türkçe etiket ve ton verir", () => {
    expect(QUOTE_STATE_LABEL["exact"]).toBe("Birebir · tek konum");
    expect(quoteStateTone("exact")).toBe("ok");
    expect(quoteStateTone("normalized")).toBe("warn");
    expect(quoteStateTone("ambiguous")).toBe("warn");
    expect(quoteStateTone("not_found")).toBe("plain");
    expect(quoteStateTone("unverifiable")).toBe("danger");
  });

  it("özet satırı tüm sayımları taşır", () => {
    expect(
      quoteCheckSummary(
        check({ exact: 1, normalized: 1, ambiguous: 1, notFound: 1, unverifiable: 1, spansFound: 5 }),
      ),
    ).toBe("5 alıntı · 1 birebir · 1 birebir değil · 1 belirsiz · 1 bulunamadı · 1 doğrulanamadı");
  });

  it("yalnız birebir tek konum bağlanabilir", () => {
    expect(canBind(finding())).toBe(true);
    expect(canBind(finding({ state: "normalized" }))).toBe(false);
    expect(canBind(finding({ state: "ambiguous" }))).toBe(false);
    expect(canBind(finding({ state: "not_found", position: null }))).toBe(false);
    expect(canBind(finding({ position: position({ segmentId: null }) }))).toBe(false);
    expect(canBind(finding({ position: position({ matchKind: "whitespace" }) }))).toBe(false);
  });

  it("bağlama yükü gerçek konumdan üretilir", () => {
    expect(bindingInput("c1", finding())).toEqual({
      caseId: "c1",
      documentVersionId: "ver1",
      parseRunId: "run1",
      segmentId: "seg1",
      startOffset: 19,
      endOffset: 56,
      exactQuote: "kira bedelini ödemediğini tespit etti",
    });
    expect(bindingInput("c1", finding({ state: "ambiguous", position: null }))).toBeNull();
  });

  it("konum etiketi belge, sayfa ve eşleşme türünü yazar", () => {
    expect(positionLabel(finding())).toBe("Dilekçe · s. 1 · birebir · metin katmanı");
    expect(
      positionLabel(
        finding({ position: position({ pageIndex: 1, matchKind: "turkish_fold", textOrigin: "ocr" }) }),
      ),
    ).toBe("Dilekçe · s. 2 · harf farkı · OCR");
  });

  it("aday etiketi karakter konumunu gösterir", () => {
    expect(
      candidateLabel({
        documentName: "Rapor",
        pageIndex: 2,
        charStart: 120,
        matchKind: "whitespace",
      }),
    ).toBe("Rapor · s. 3 · karakter 120 · boşluk farkı");
  });

  it("sayılmayan alıntıları ve kapsamı yazar", () => {
    expect(skippedLabel(check())).toBe("");
    expect(skippedLabel(check({ skippedShort: 2, skippedLong: 1, unclosed: 3 }))).toBe(
      "Sayılmayan: 2 kısa, 1 aşırı uzun, 3 kapanmamış",
    );
    expect(scopeLabel(check())).toBe("Kapsam: dosyadaki 2 belgenin güncel sürümü");
    expect(
      scopeLabel(check({ scope: "document", scopeDocumentName: "Rapor", documentsWithText: 1 })),
    ).toBe("Kapsam: yalnız “Rapor” belgesinin güncel sürümü");
  });
});
