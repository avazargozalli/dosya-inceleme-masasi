import { describe, expect, it } from "vitest";
import type { PrecedentVersionRow, QuoteVerificationRow } from "./api";
import {
  accessBasisLabel,
  bibliographyMissingText,
  bulkOutcomeLabel,
  bulkOutcomeTone,
  bulkReportSummary,
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
