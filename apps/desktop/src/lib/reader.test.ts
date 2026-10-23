import { describe, expect, it } from "vitest";
import type { AnchorLocationRow, PageRow, SearchHit } from "./api";
import {
  offsetSlice,
  parseWarnings,
  querySlice,
  resolvePageIndex,
  sliceForPage,
  targetFromAnchor,
  targetFromHit,
} from "./reader";

const pages: PageRow[] = [
  {
    id: "p1",
    segmentId: "s1",
    pageIndex: 0,
    text: "Birinci sayfa",
    textOrigin: "native",
    qualityFlags: "[]",
    width: null,
    height: null,
  },
  {
    id: "p2",
    segmentId: "s2",
    pageIndex: 1,
    text: "İkinci sayfa: İTİRAZIN reddine karar verildi.",
    textOrigin: "ocr",
    qualityFlags: "[]",
    width: null,
    height: null,
  },
];

const hit: SearchHit = {
  segmentId: "s2",
  pageId: "p2",
  documentVersionId: "v-old",
  documentId: "d1",
  documentName: "Dilekçe.pdf",
  rawText: "İkinci sayfa: İTİRAZIN reddine karar verildi.",
  pageIndex: 1,
  rank: 0.5,
};

const anchor: AnchorLocationRow = {
  anchorId: "a1",
  caseId: "c1",
  documentVersionId: "v-old",
  documentId: "d1",
  documentName: "Dilekçe.pdf",
  parseRunId: "r1",
  pageId: "p2",
  pageIndex: 1,
  segmentId: "s2",
  startOffset: 14,
  endOffset: 30,
  exactQuote: "İTİRAZIN reddine",
  anchorStatus: "active",
  quoteIntact: true,
};

describe("targetFromHit", () => {
  it("sonucun kendi sürümünü taşır, en son sürümü tahmin etmez", () => {
    const target = targetFromHit(hit, "itiraz");
    expect(target?.versionId).toBe("v-old");
    expect(target?.pageId).toBe("p2");
    expect(target?.query).toBe("itiraz");
  });

  it("sürüm kimliği yoksa hedef üretmez", () => {
    expect(targetFromHit({ ...hit, documentVersionId: null }, "itiraz")).toBeNull();
  });
});

describe("targetFromAnchor", () => {
  it("sayfa, offset ve kopma durumunu taşır", () => {
    const target = targetFromAnchor(anchor);
    expect(target.versionId).toBe("v-old");
    expect(target.startOffset).toBe(14);
    expect(target.quoteIntact).toBe(true);
    expect(target.anchorId).toBe("a1");
  });
});

describe("resolvePageIndex", () => {
  it("sayfa kimliğini fiziksel indekse tercih eder", () => {
    expect(resolvePageIndex(pages, { ...targetFromAnchor(anchor), pageIndex: 0 })).toBe(1);
  });

  it("segment kimliğiyle çözer", () => {
    expect(resolvePageIndex(pages, targetFromHit(hit, "itiraz"))).toBe(1);
  });

  it("aralık dışı indeksi kırpar", () => {
    const target = { ...targetFromAnchor(anchor), pageId: null, segmentId: null, pageIndex: 99 };
    expect(resolvePageIndex(pages, target)).toBe(1);
  });

  it("boş belgede -1 döner", () => {
    expect(resolvePageIndex([], targetFromAnchor(anchor))).toBe(-1);
  });
});

describe("offsetSlice", () => {
  it("Unicode scalar aralığını keser", () => {
    const text = "A😀BğC";
    expect(offsetSlice(text, 1, 3)).toEqual({ before: "A", hit: "😀B", after: "ğC" });
  });

  it("geçersiz aralıkta null döner", () => {
    expect(offsetSlice("kısa", 0, 99)).toBeNull();
    expect(offsetSlice("kısa", 3, 1)).toBeNull();
    expect(offsetSlice("kısa", null, 1)).toBeNull();
  });
});

describe("querySlice", () => {
  it("Türkçe büyük harfli metinde sorguyu işaretler", () => {
    const slice = querySlice("İkinci sayfa: İTİRAZIN reddine", "itiraz");
    expect(slice?.hit).toBe("İTİRAZ");
    expect(slice?.before).toBe("İkinci sayfa: ");
  });

  it("bulunmayan sorguda null döner", () => {
    expect(querySlice("metin", "kira")).toBeNull();
    expect(querySlice("metin", "   ")).toBeNull();
  });
});

describe("parseWarnings", () => {
  it("JSON uyarı dizisini çözer", () => {
    expect(parseWarnings('["UDF imza: tespit edildi — denetlenmedi"]')).toEqual([
      "UDF imza: tespit edildi — denetlenmedi",
    ]);
  });

  it("bozuk veya dizi olmayan kayıtta boş liste döner", () => {
    expect(parseWarnings("bozuk")).toEqual([]);
    expect(parseWarnings('{"a":1}')).toEqual([]);
    expect(parseWarnings(null)).toEqual([]);
  });
});

describe("sliceForPage", () => {
  it("anchor yalnız kendi sayfasında işaretlenir", () => {
    const target = targetFromAnchor(anchor);
    expect(sliceForPage(pages[1]!.text, target, pages[1]!)).not.toBeNull();
    expect(sliceForPage(pages[0]!.text, target, pages[0]!)).toBeNull();
  });

  it("offset yoksa sorgu geçişini işaretler", () => {
    const target = targetFromHit(hit, "reddine");
    expect(sliceForPage(pages[1]!.text, target, pages[1]!)?.hit).toBe("reddine");
  });
});
