import { describe, expect, it } from "vitest";
import type { PrecedentSearchHit, PrecedentSearchPage } from "./api";
import {
  precedentCitation,
  precedentMatchLabel,
  precedentSearchSummary,
  precedentSnippetForCopy,
  scopeNotice,
  snippetHighlightSpan,
  SEARCH_SCOPE_LABELS,
} from "./search-scope";

const hit: PrecedentSearchHit = {
  precedentId: "p1",
  court: "Yargıtay",
  chamber: "1. Hukuk Dairesi",
  docketNo: "2024/123",
  decisionNo: "2025/456",
  decisionDate: "2025-06-01",
  title: "Kira alacağı",
  versionId: "version-abcdef123456",
  acquiredAt: "2026-10-24T08:00:00Z",
  sourceKind: "paste",
  snippet: "…itirazın reddine karar verdi…",
  snippetStart: 40,
  matchStart: 44,
  matchLength: 18,
  matchSource: "full_text",
};

const page: PrecedentSearchPage = {
  scope: "precedent_library",
  mode: "flexible",
  query: "itiraz",
  resolvedQuery: "itiraz",
  hits: [hit],
  scanned: 3,
  matched: 1,
  withoutText: 0,
  limit: 25,
  truncated: false,
  notes: ["Kapsam yalnız emsal kütüphanesidir; aktif dosyanın belgeleri bu aramada taranmaz."],
};

describe("arama kapsamı", () => {
  it("iki kapsamı adıyla ayırır ve uyarıda dosya/emsal ayrımını söyler", () => {
    expect(SEARCH_SCOPE_LABELS.case_documents).toBe("Dosya belgeleri");
    expect(SEARCH_SCOPE_LABELS.precedent_library).toBe("Emsal kütüphanesi");
    expect(scopeNotice("precedent_library")).toContain("dosya aramasına karışmaz");
    expect(scopeNotice("case_documents")).toContain("Emsal kütüphanesi bu aramaya dahil edilmez");
  });

  it("künyede yalnız var olan alanları yazar, eksikte tahmin etmez", () => {
    expect(precedentCitation(hit)).toBe(
      "Yargıtay 1. Hukuk Dairesi, E. 2024/123, K. 2025/456, 2025-06-01",
    );
    expect(
      precedentCitation({ ...hit, chamber: null, docketNo: null, decisionNo: null, decisionDate: null }),
    ).toBe("Yargıtay");
  });

  it("eşleşmenin kaynağını etiketler", () => {
    expect(precedentMatchLabel(hit)).toBe("karar metni");
    expect(precedentMatchLabel({ ...hit, matchSource: "metadata" })).toBe("künye numarası");
  });

  it("özet tam metinsiz kayıtları gizlemez", () => {
    expect(precedentSearchSummary(page)).toBe("1 eşleşme · 3 tam metin tarandı");
    expect(precedentSearchSummary({ ...page, withoutText: 2 })).toContain(
      "2 kayıt tam metinsiz (aranmadı)",
    );
  });

  it("kopya metni kütüphane kaynağını ve sürümü taşır", () => {
    const text = precedentSnippetForCopy(hit);
    expect(text).toContain("emsal kütüphanesi");
    expect(text).toContain("version-");
    expect(text).toContain("itirazın reddine");
  });
});

/**
 * F12-T14v: kütüphane vurgusu eşleştiricinin **ölçtüğü kaynak aralığından**
 * çizilir. Snippet penceresi metnin ortasından başlarsa başına `…` eklenir
 * (`snippet_window`); konum çevrimi bu öneki ve **kod noktası** sayımını
 * bilmeli. Aralık pencereyle kesişmiyorsa vurgu **uydurulmaz**.
 */
describe("snippet vurgu aralığı", () => {
  /** Kaynak metnin `snippetStart`ten başlayan penceresi. */
  function windowed(
    snippet: string,
    snippetStart: number,
    matchStart: number | null,
    matchLength: number | null,
    matchSource: PrecedentSearchHit["matchSource"] = "full_text",
  ): PrecedentSearchHit {
    return { ...hit, snippet, snippetStart, matchStart, matchLength, matchSource };
  }

  it("kaynak konumunu snippet kod noktasına çevirir (baştaki … sayılır)", () => {
    // Pencere 200'den başlar; `itirazın` kaynakta 205'te, snippet'te 6'da.
    const snippet = "…dava itirazın reddine…";
    const span = snippetHighlightSpan(windowed(snippet, 200, 205, 8));
    expect(span).toEqual({ start: 6, length: 8 });
    expect(Array.from(snippet).slice(6, 14).join("")).toBe("itirazın");
  });

  it("satır sonu tiresiyle birleşen eşleşmeyi kaynak aralığıyla verir", () => {
    // `bedel-\nlini` 11 kod noktasıdır: tire ve satır sonu aralığa dahildir.
    const snippet = "…ödenen bedel-\nlini tutarı…";
    const span = snippetHighlightSpan(windowed(snippet, 200, 207, 11));
    expect(span).toEqual({ start: 8, length: 11 });
    expect(Array.from(snippet).slice(8, 19).join("")).toBe("bedel-\nlini");
  });

  it("pencere metnin başından başlıyorsa kaydırma yapmaz", () => {
    const span = snippetHighlightSpan(windowed("kira bedeli", 0, 0, 4));
    expect(span).toEqual({ start: 0, length: 4 });
  });

  it("pencere sonunu taşan aralığı görünen kısımla kırpar", () => {
    // Sorgu pencereden uzun: işaret görünen sona kadar çizilir, taşmaz.
    const span = snippetHighlightSpan(windowed("…kira", 10, 10, 40));
    expect(span).toEqual({ start: 1, length: 4 });
  });

  it("konumları kod noktası sayar (astral karakter kaydırmaz)", () => {
    // 😀 tek kod noktası, iki UTF-16 birimi: yanlış sayım vurguyu kaydırırdı.
    const snippet = "…😀 kira";
    const span = snippetHighlightSpan(windowed(snippet, 5, 7, 4));
    expect(span).toEqual({ start: 3, length: 4 });
    expect(Array.from(snippet).slice(3, 7).join("")).toBe("kira");
  });

  it("konum yoksa ya da pencereyle kesişmiyorsa vurgu üretmez", () => {
    expect(snippetHighlightSpan({ ...hit, matchStart: null })).toBeNull();
    expect(snippetHighlightSpan({ ...hit, matchLength: null })).toBeNull();
    expect(snippetHighlightSpan({ ...hit, matchLength: 0 })).toBeNull();
    // Eşleşme pencerenin solunda / sağında: uydurma vurgu yok.
    expect(snippetHighlightSpan(windowed("…itiraz", 100, 90, 6))).toBeNull();
    expect(snippetHighlightSpan(windowed("…itiraz", 100, 400, 6))).toBeNull();
  });
});
