import { describe, expect, it } from "vitest";
import type {
  LibraryInTextCheck,
  LibraryQuoteFinding,
  LibraryQuoteMatch,
  PrecedentInTextCheck,
  PrecedentVersionRow,
  QuoteVerificationRow,
} from "./api";
import {
  accessBasisLabel,
  bibliographyMissingText,
  bulkOutcomeLabel,
  bulkOutcomeTone,
  bulkReportSummary,
  libraryCheckSkipped,
  libraryCheckSummary,
  libraryMatchCitation,
  libraryQuoteFindingText,
  libraryQuoteFindingTone,
  precedentInTextSkipped,
  precedentInTextSummary,
  precedentMethodLabel,
  precedentPositionText,
  precedentQuoteFindingText,
  precedentQuoteFindingTone,
  quoteScanNote,
  quoteStateText,
  quoteStateTone,
  quoteToCitationMatch,
  summarizeVersions,
  versionConflictText,
  versionSourceLabel,
  versionStateText,
} from "./precedents";

function verification(overrides: Partial<QuoteVerificationRow>): QuoteVerificationRow {
  return {
    precedentId: "p1",
    versionId: "pv1",
    state: "not_checked",
    method: null,
    matchedStart: null,
    matchedLength: null,
    occurrences: 0,
    reason: null,
    versionsScanned: 0,
    historicalVersionsScanned: 0,
    foundInOldVersion: false,
    historicalTruncated: false,
    quote: "itirazın reddine",
    checkedAt: "2026-10-24T00:00:00Z",
    ...overrides,
  };
}

function version(overrides: Partial<PrecedentVersionRow>): PrecedentVersionRow {
  return {
    id: "pv1",
    precedentId: "p1",
    documentVersionId: null,
    acquiredAt: "2026-10-24T00:00:00Z",
    sourceKind: "paste",
    bodyHash: "a".repeat(64),
    verificationState: "text_verified",
    charCount: 1200,
    hasText: true,
    identityConflict: false,
    conflictWithVersionId: null,
    ...overrides,
  };
}

describe("tam metin durumu (F12-T05)", () => {
  it("metin varsa karakter sayısını yazar, yoksa 'yok' der", () => {
    expect(versionStateText({ hasText: true, charCount: 12345 })).toContain("12.345");
    expect(versionStateText({ hasText: false, charCount: 0 })).toBe("tam metin yok");
  });

  it("kaynak türünü Türkçe etiketler", () => {
    expect(versionSourceLabel({ sourceKind: "paste" })).toBe("yapıştırıldı");
    expect(versionSourceLabel({ sourceKind: "document" })).toBe("kasadaki belgeden");
    expect(versionSourceLabel({ sourceKind: "file" })).toBe("dosyadan");
  });

  it("özet en yeni sürümün metinli olup olmadığını söyler", () => {
    const summary = summarizeVersions([
      version({ id: "pv2", hasText: false, charCount: 0 }),
      version({ id: "pv1" }),
    ]);
    expect(summary.withText).toBe(1);
    expect(summary.latestHasText).toBe(false);
    expect(summarizeVersions([]).latestHasText).toBe(false);
  });
});

describe("alıntı doğrulama metni (F12-T05)", () => {
  it("birebir eşleşmeyi konumla birlikte yazar", () => {
    const text = quoteStateText(
      verification({ state: "match", method: "exact", matchedStart: 1233, occurrences: 1 }),
    );
    expect(text).toContain("birebir bulundu");
    expect(text).toContain("1.234");
    expect(text).not.toContain("Birebir değil");
  });

  it("boşluk ve harf farkını 'birebir değil' diye işaretler", () => {
    const spaced = quoteStateText(verification({ state: "match", method: "whitespace" }));
    expect(spaced).toContain("satır/boşluk");
    expect(spaced).toContain("Birebir değil");
    const folded = quoteStateText(
      verification({ state: "match", method: "turkish_fold", matchedStart: 10 }),
    );
    expect(folded).toContain("harf farkıyla");
    expect(folded).toContain("gözle doğrulayın");
  });

  it("tire ile bölünmüş kelimeyi ayrı ve zayıf bir eşleşme olarak yazar", () => {
    const hyphenated = quoteStateText(
      verification({ state: "match", method: "dehyphenated", matchedStart: 0, occurrences: 1 }),
    );
    expect(hyphenated).toContain("tire ile bölünmüş");
    expect(hyphenated).toContain("Birebir değil");
    expect(hyphenated).not.toContain("harf farkıyla");
    expect(quoteStateTone(verification({ state: "match", method: "dehyphenated" }))).toBe("warn");
  });

  it("ligatür açılarak bulunan alıntıyı ayrı ve zayıf bir eşleşme olarak yazar", () => {
    const ligature = quoteStateText(
      verification({ state: "match", method: "ligature", matchedStart: 9, occurrences: 1 }),
    );
    expect(ligature).toContain("bitişik yazı (ligatür)");
    expect(ligature).toContain("birebir değil");
    expect(ligature).not.toContain("tire ile bölünmüş");
    expect(quoteStateTone(verification({ state: "match", method: "ligature" }))).toBe("warn");
  });

  it("tekrarlanan alıntıda kaç kez geçtiğini yazar", () => {
    const text = quoteStateText(
      verification({ state: "match", method: "exact", matchedStart: 0, occurrences: 3 }),
    );
    expect(text).toContain("3 kez geçiyor");
  });

  it("bulunamadıyı 'uydurulmuş' diye çevirmez", () => {
    const text = quoteStateText(verification({ state: "mismatch", reason: "not_found" }));
    expect(text).toContain("bulunamadı");
    expect(text).toContain("uydurulmuş");
    expect(text).toContain("demek değildir");
    expect(text.toLowerCase()).not.toContain("sahte");
  });

  it("tam metin yokken doğrulanamadı der", () => {
    const text = quoteStateText(verification({ state: "not_checked", reason: "no_full_text" }));
    expect(text).toContain("tam metni yok");
    expect(text).toContain("doğrulanamadı");
    expect(quoteStateTone(verification({ state: "not_checked" }))).toBe("muted");
  });

  it("kısa alıntıyı doğrulanmış saymaz", () => {
    const text = quoteStateText(
      verification({ state: "not_checked", reason: "quote_too_short", quote: "reddine" }),
    );
    expect(text).toContain("çok kısa");
    expect(text).not.toContain("bulundu");
  });

  it("renk tonu eşleşme türüne göre ayrılır", () => {
    expect(quoteStateTone(verification({ state: "match", method: "exact" }))).toBe("ok");
    expect(quoteStateTone(verification({ state: "match", method: "turkish_fold" }))).toBe("warn");
    expect(quoteStateTone(verification({ state: "mismatch" }))).toBe("warn");
  });

  it("eski sürümde bulunan alıntıyı gizlemez ve uyarı tonuna alır", () => {
    const text = quoteStateText(
      verification({
        state: "match",
        method: "exact",
        matchedStart: 4,
        versionsScanned: 3,
        historicalVersionsScanned: 2,
        foundInOldVersion: true,
      }),
    );
    expect(text).toContain("eski bir sürümde birebir bulundu");
    expect(text).toContain("Güncel tam metinde geçmiyor");
    // Birebir olsa da bugünkü metni doğrulamaz.
    expect(quoteStateTone(verification({ state: "match", method: "exact", foundInOldVersion: true }))).toBe(
      "warn",
    );
  });

  it("taranan sürüm sayısını ve kesmeyi söyler", () => {
    const scanned = quoteScanNote(
      verification({ state: "mismatch", versionsScanned: 3, historicalVersionsScanned: 2 }),
    );
    expect(scanned).toBe("3 sürüm tarandı · 2 eski sürüm");
    const cut = quoteScanNote(
      verification({
        state: "mismatch",
        versionsScanned: 6,
        historicalVersionsScanned: 5,
        historicalTruncated: true,
      }),
    );
    expect(cut).toContain("daha eski sürümler taranmadı");
    // Sınır kesildiyse "bulunamadı" kesin değildir.
    expect(
      quoteStateText(
        verification({
          state: "mismatch",
          versionsScanned: 6,
          historicalVersionsScanned: 5,
          historicalTruncated: true,
        }),
      ),
    ).toContain("sonuç kesin değildir");
    // Hiç taranmayan durumda satır üretilmez.
    expect(quoteScanNote(verification({ state: "not_checked" }))).toBeNull();
  });

  it("atıf kontrolü boyutuna yalnız üç değer taşır", () => {
    expect(quoteToCitationMatch(verification({ state: "match", method: "whitespace" }))).toBe(
      "match",
    );
    expect(quoteToCitationMatch(verification({ state: "mismatch" }))).toBe("mismatch");
    expect(quoteToCitationMatch(verification({ state: "not_checked" }))).toBe("not_checked");
  });
});

describe("toplu içe alma etiketleri (F12-T06)", () => {
  it("eklenen dosyayı karakter sayısıyla söyler", () => {
    expect(
      bulkOutcomeLabel({ outcome: "created", code: null, message: null, charCount: 1234 }),
    ).toBe("eklendi · 1.234 karakter");
  });

  it("aynı nüshayı 'zaten var' der, çakışmayı incelemeye bırakır", () => {
    expect(
      bulkOutcomeLabel({ outcome: "duplicate", code: null, message: null, charCount: 10 }),
    ).toBe("aynı nüsha zaten var; yeniden yazılmadı");
    expect(
      bulkOutcomeLabel({ outcome: "conflict", code: null, message: null, charCount: 10 }),
    ).toBe("aynı künye, farklı metin — inceleyin");
  });

  it("hatayı kod ve mesajla gösterir, sessizce yutmaz", () => {
    expect(
      bulkOutcomeLabel({
        outcome: "error",
        code: "E_BULK_COURT",
        message: "Mahkeme bilinmiyor.",
        charCount: 0,
      }),
    ).toBe("E_BULK_COURT: Mahkeme bilinmiyor.");
    expect(bulkOutcomeTone({ outcome: "error" })).toBe("danger");
    expect(bulkOutcomeTone({ outcome: "conflict" })).toBe("warn");
    expect(bulkOutcomeTone({ outcome: "created" })).toBe("ok");
  });

  it("özet yalnız sıfır olmayan hata sayısını ekler", () => {
    expect(
      bulkReportSummary({ created: 3, duplicates: 1, conflicts: 1, failed: 0 }),
    ).toBe("3 eklendi · 1 zaten vardı · 1 çakışma");
    expect(
      bulkReportSummary({ created: 0, duplicates: 0, conflicts: 0, failed: 2 }),
    ).toBe("0 eklendi · 0 zaten vardı · 0 çakışma · 2 hata");
  });

  it("çakışma metni yalnız gerçek çakışmada döner", () => {
    expect(versionConflictText({ identityConflict: true })).toContain("farklı metin");
    expect(versionConflictText({ identityConflict: false })).toBeNull();
  });

  it("bibliyografya eksik alanları listeler, eksik yoksa künye tam der", () => {
    expect(bibliographyMissingText([])).toBe("künye tam");
    expect(bibliographyMissingText(["karar tarihi", "kaynak bağlantısı"])).toBe(
      "eksik: karar tarihi, kaynak bağlantısı",
    );
  });

  it("erişim tarihinin kaynağını gizlemez", () => {
    expect(accessBasisLabel("version")).toContain("tam metnin edinimi");
    expect(accessBasisLabel("record")).toContain("tam metin edinimi yok");
  });
});

describe("metindeki alıntıların karar metninde denetimi (F12-T14j)", () => {
  function report(overrides: Partial<PrecedentInTextCheck> = {}): PrecedentInTextCheck {
    return {
      precedentId: "p1",
      textChars: 40,
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
      dehyphenated: 0,
      ligature: 0,
      foundInOldVersion: 0,
      versionsTotal: 1,
      versionsScanned: 1,
      historicalVersionsScanned: 0,
      historicalTruncated: false,
      notes: [],
      checkedAt: "2026-11-21T00:00:00Z",
      ...overrides,
    };
  }

  it("eşleşme katmanını Türkçe etiketler", () => {
    expect(precedentMethodLabel("exact")).toBe("birebir");
    expect(precedentMethodLabel("whitespace")).toBe("boşluk farkı");
    expect(precedentMethodLabel("ligature")).toBe("ligatür");
    expect(precedentMethodLabel("turkish_fold")).toBe("harf farkı");
    expect(precedentMethodLabel("dehyphenated")).toBe("tire birleştirme");
    expect(precedentMethodLabel(null)).toBe("");
  });

  it("her hükmü dürüst metne çevirir; 'bulunamadı' uydurma demek değildir", () => {
    expect(precedentQuoteFindingText({ ...finding(), state: "exact" })).toBe("birebir");
    expect(
      precedentQuoteFindingText({ ...finding(), state: "normalized", method: "turkish_fold" }),
    ).toBe("normalize · harf farkı");
    expect(
      precedentQuoteFindingText({ ...finding(), state: "ambiguous", occurrences: 3 }),
    ).toBe("belirsiz · 3 konum");
    expect(precedentQuoteFindingText({ ...finding(), state: "not_found" })).toBe("bulunamadı");
    expect(precedentQuoteFindingText({ ...finding(), state: "unverifiable" })).toBe(
      "doğrulanamadı",
    );
  });

  it("ton: birebir yeşil, eski sürüm ve normalize uyarı, doğrulanamadı sessiz", () => {
    expect(precedentQuoteFindingTone({ ...finding(), state: "exact" })).toBe("ok");
    expect(
      precedentQuoteFindingTone({ ...finding(), state: "exact", foundInOldVersion: true }),
    ).toBe("warn");
    expect(precedentQuoteFindingTone({ ...finding(), state: "normalized" })).toBe("warn");
    expect(precedentQuoteFindingTone({ ...finding(), state: "not_found" })).toBe("warn");
    expect(precedentQuoteFindingTone({ ...finding(), state: "unverifiable" })).toBe("muted");
  });

  it("konumu 1 tabanlı yazar ve güncel/eski sürümü söyler", () => {
    expect(
      precedentPositionText({
        versionId: "pv1",
        isCurrentVersion: true,
        charStart: 118,
        charEnd: 136,
        sourceText: "kira bedeli ödenmedi",
      }),
    ).toBe("konum 119–136 · güncel sürüm");
    expect(
      precedentPositionText({
        versionId: "pv0",
        isCurrentVersion: false,
        charStart: 0,
        charEnd: 10,
        sourceText: "eski metin",
      }),
    ).toBe("konum 1–10 · eski sürüm");
  });

  it("özet sayıları çekirdekten taşır; atlanan varsa açıkça yazar", () => {
    expect(
      precedentInTextSummary(
        report({ exact: 1, normalized: 1, ambiguous: 0, notFound: 2, unverifiable: 0 }),
      ),
    ).toBe("1 alıntı · 1 birebir · 1 normalize · 0 belirsiz · 2 bulunamadı · 0 doğrulanamadı");
    expect(precedentInTextSkipped(report())).toBeNull();
    expect(
      precedentInTextSkipped(report({ skippedShort: 2, skippedLong: 1, unclosed: 1 })),
    ).toContain("2 alıntı 8 karakterden kısa");
  });
});

function finding(overrides: Partial<PrecedentInTextCheck["quotes"][number]> = {}) {
  return {
    index: 0,
    quote: "kira bedeli ödenmedi",
    state: "exact" as const,
    reason: null,
    method: "exact" as const,
    occurrences: 1,
    position: null,
    candidates: [],
    foundInOldVersion: false,
    ...overrides,
  };
}

// ---- F12-T14k: kütüphane geneli arama etiketleri ----

function libraryMatch(overrides: Partial<LibraryQuoteMatch> = {}): LibraryQuoteMatch {
  return {
    precedentId: "p1",
    court: "Yargıtay",
    chamber: "1. Hukuk Dairesi",
    docketNo: "2024/100",
    decisionNo: "2025/77",
    decisionDate: "2025-06-01",
    title: "Kira alacağı",
    versionId: "pv1",
    method: "exact",
    occurrences: 1,
    charStart: 10,
    charEnd: 28,
    sourceText: "kira bedeli ödenmedi",
    ...overrides,
  };
}

function libraryFinding(overrides: Partial<LibraryQuoteFinding> = {}): LibraryQuoteFinding {
  return {
    index: 0,
    quote: "kira bedeli ödenmedi",
    state: "found",
    matches: [libraryMatch()],
    matchedPrecedents: 1,
    hiddenMatches: 0,
    ...overrides,
  };
}

function libraryCheck(overrides: Partial<LibraryInTextCheck> = {}): LibraryInTextCheck {
  return {
    textChars: 42,
    spansFound: 2,
    skippedShort: 0,
    skippedLong: 0,
    unclosed: 0,
    quotes: [libraryFinding()],
    found: 1,
    notFound: 1,
    partialScan: 0,
    noLibraryText: 0,
    libraryTotal: 3,
    scanned: 3,
    truncated: false,
    dehyphenated: 0,
    ligature: 0,
    notes: [],
    checkedAt: "2026-05-01T10:00:00Z",
    ...overrides,
  };
}

describe("kütüphane geneli alıntı araması (F12-T14k)", () => {
  it("özet kütüphane, tarama ve sonuç sayılarını taşır", () => {
    expect(libraryCheckSummary(libraryCheck())).toBe(
      "3 emsalin tam metni var · 3 tarandı · 1 alıntı bulundu · 1 bulunamadı · 0 kesin değil · 0 kütüphane metni yok",
    );
    expect(libraryCheckSkipped(libraryCheck())).toBeNull();
    expect(
      libraryCheckSkipped(libraryCheck({ skippedShort: 2, skippedLong: 1, unclosed: 1 })),
    ).toContain("2 alıntı 8 karakterden kısa");
  });

  it("bulundu / bulunamadı / kesin değil / kütüphane metni yok ayrılır", () => {
    const found = libraryFinding({ matchedPrecedents: 3 });
    expect(libraryQuoteFindingText(found)).toBe("bulundu · 3 emsal");
    expect(libraryQuoteFindingTone(found)).toBe("ok");
    const missing = libraryFinding({ state: "not_found", matches: [], matchedPrecedents: 0 });
    expect(libraryQuoteFindingText(missing)).toBe("bulunamadı");
    expect(libraryQuoteFindingTone(missing)).toBe("warn");
    const partial = libraryFinding({ state: "partial_scan", matches: [], matchedPrecedents: 0 });
    expect(libraryQuoteFindingText(partial)).toBe("kesin değil · tarama sınırlı");
    expect(libraryQuoteFindingTone(partial)).toBe("warn");
    const noText = libraryFinding({ state: "no_library_text", matches: [], matchedPrecedents: 0 });
    expect(libraryQuoteFindingText(noText)).toBe("kütüphane metni yok");
    expect(libraryQuoteFindingTone(noText)).toBe("muted");
    // `partial_scan` asla "bulunamadı" diye yazılmaz.
    expect(libraryQuoteFindingText(partial)).not.toBe("bulunamadı");
  });

  it("eşleşen emsalin künyesi boş alanları atlar", () => {
    expect(libraryMatchCitation(libraryMatch())).toBe(
      "Yargıtay 1. Hukuk Dairesi · 2025/77 · 2025-06-01",
    );
    expect(
      libraryMatchCitation(
        libraryMatch({ chamber: null, decisionNo: null, decisionDate: null }),
      ),
    ).toBe("Yargıtay · 2024/100");
  });
});
