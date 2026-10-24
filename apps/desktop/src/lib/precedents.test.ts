import { describe, expect, it } from "vitest";
import type { PrecedentVersionRow, QuoteVerificationRow } from "./api";
import {
  quoteStateText,
  quoteStateTone,
  quoteToCitationMatch,
  summarizeVersions,
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

  it("atıf kontrolü boyutuna yalnız üç değer taşır", () => {
    expect(quoteToCitationMatch(verification({ state: "match", method: "whitespace" }))).toBe(
      "match",
    );
    expect(quoteToCitationMatch(verification({ state: "mismatch" }))).toBe("mismatch");
    expect(quoteToCitationMatch(verification({ state: "not_checked" }))).toBe("not_checked");
  });
});
