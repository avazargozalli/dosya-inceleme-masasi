import { describe, expect, it } from "vitest";
import type { InTextQuoteCheck, InTextQuoteFinding, InTextQuotePosition } from "./api";
import {
  bindHint,
  bindingInput,
  canBind,
  candidateLabel,
  oldVersionNote,
  pageSpanLabel,
  positionLabel,
  quoteCheckSummary,
  QUOTE_STATE_LABEL,
  quoteStateTone,
  scopeLabel,
  skippedLabel,
  versionsLabel,
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
    endPageIndex: 0,
    spansPages: false,
    versionNo: 1,
    isCurrentVersion: true,
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
    versionsScanned: 2,
    historicalVersionsScanned: 0,
    foundInOldVersion: 0,
    dehyphenated: 0,
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
    // Sayfa sınırını aşan birebir eşleşme tek sayfalık anchor'a sığmaz.
    expect(
      canBind(finding({ position: position({ spansPages: true, endPageIndex: 1 }) })),
    ).toBe(false);
  });

  it("sayfa sınırını aşan alıntıda aralığı ve bağlanamama nedenini yazar", () => {
    const spanning = finding({
      position: position({ pageIndex: 2, endPageIndex: 3, spansPages: true }),
    });
    expect(pageSpanLabel(2, 3)).toBe("s. 3–4");
    expect(pageSpanLabel(2, 2)).toBe("s. 3");
    expect(positionLabel(spanning)).toBe(
      "Dilekçe · s. 3–4 · birebir · metin katmanı · sayfa sınırını aşıyor",
    );
    expect(bindHint(spanning)).toBe(
      "Alıntı sayfa sınırını aşıyor; tek sayfalık anchor'a bağlanmaz.",
    );
    expect(bindHint(finding())).toBe("");
    expect(bindHint(finding({ state: "normalized" }))).toBe(
      "Yalnız birebir tek konum kaynağa bağlanır.",
    );
    expect(
      candidateLabel({
        documentName: "Rapor",
        pageIndex: 2,
        endPageIndex: 3,
        spansPages: true,
        charStart: 120,
        matchKind: "whitespace",
      }),
    ).toBe("Rapor · s. 3–4 · karakter 120 · boşluk farkı · sayfa sınırını aşıyor");
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

  it("eski sürümde bulunan alıntıyı etiketler ve bağlama uyarısını yazar", () => {
    const historical = finding({
      position: position({ versionNo: 1, isCurrentVersion: false }),
    });
    expect(positionLabel(historical)).toBe(
      "Dilekçe · s. 1 · birebir · metin katmanı · eski sürüm (v1)",
    );
    // Anchor sürümü kaydettiği için bağlanabilir; kullanıcı yine de uyarılır.
    expect(canBind(historical)).toBe(true);
    expect(oldVersionNote(historical)).toBe(
      "Kaynak v1 sürümüne bağlanır; alıntı güncel sürümde geçmiyor.",
    );
    expect(oldVersionNote(finding())).toBe("");
    expect(oldVersionNote(finding({ state: "not_found", position: null }))).toBe("");
    expect(
      oldVersionNote(
        finding({ position: position({ versionNo: 3, isCurrentVersion: false, matchKind: "whitespace" }) }),
      ),
    ).toBe("Alıntı güncel sürümde geçmiyor; yalnız v3 sürümünde bulundu.");
  });

  it("tire ile bölünmüş kelime eşleşmesini etiketler ve bağlanamama nedenini yazar", () => {
    const hyphenated = finding({
      state: "normalized",
      position: position({
        matchKind: "dehyphenated",
        charStart: 19,
        charEnd: 44,
        sourceText: "kira bedel-\nlini ödemediğini",
      }),
    });
    expect(positionLabel(hyphenated)).toBe(
      "Dilekçe · s. 1 · tire birleştirme · metin katmanı",
    );
    expect(canBind(hyphenated)).toBe(false);
    expect(bindHint(hyphenated)).toBe(
      "Alıntı ancak satır sonunda tire ile bölünmüş kelime birleştirilerek bulundu; birebir sayılmaz, kaynağa bağlanmaz.",
    );
    // Özet satırı yalnız sayı sıfırdan büyükse tire satırını ekler.
    expect(quoteCheckSummary(check({ exact: 0, normalized: 1, dehyphenated: 1 }))).toBe(
      "1 alıntı · 0 birebir · 1 birebir değil · 0 belirsiz · 0 bulunamadı · 0 doğrulanamadı · 1 tire birleştirme",
    );
    expect(quoteCheckSummary(check())).not.toContain("tire birleştirme");
    // Aday listesinde de tire etiketi görünür.
    expect(
      candidateLabel({
        documentName: "Rapor",
        pageIndex: 0,
        charStart: 5,
        matchKind: "dehyphenated",
      }),
    ).toBe("Rapor · s. 1 · karakter 5 · tire birleştirme");
  });

  it("sürüm sayacı eski sürüm taramasını yazar", () => {
    expect(versionsLabel(check({ versionsScanned: 3, historicalVersionsScanned: 2 }))).toBe(
      "3 sürüm tarandı · 2 eski sürüm",
    );
    expect(versionsLabel(check({ versionsScanned: 1, historicalVersionsScanned: 0 }))).toBe(
      "1 sürüm tarandı",
    );
    expect(versionsLabel(check({ versionsScanned: 0, historicalVersionsScanned: 0 }))).toBe("");
  });
});
