import { describe, expect, it } from "vitest";

import type { SearchFilters, SearchHit, SearchPage } from "./api";
import {
  activeFilterCount,
  canLoadMore,
  canonicalFilters,
  describeFilters,
  EMPTY_SEARCH_FILTERS,
  filtersEqual,
  filtersStale,
  isDocketQuery,
  isFiltered,
  mergeHits,
  resolveSearchMode,
  resultSummary,
  searchModeHint,
  stripWrappingQuotes,
  textOriginLabel,
  toggleValue,
} from "./search";

function filters(overrides: Partial<SearchFilters> = {}): SearchFilters {
  return { ...EMPTY_SEARCH_FILTERS, ...overrides };
}

function hit(segmentId: string): SearchHit {
  return {
    segmentId,
    pageId: null,
    documentVersionId: null,
    documentId: null,
    documentName: null,
    documentType: null,
    documentStatus: null,
    textOrigin: null,
    caseId: null,
    tags: [],
    rawText: "",
    pageIndex: null,
    rank: 0,
  };
}

function page(overrides: Partial<SearchPage> = {}): SearchPage {
  return {
    caseId: "case-1",
    query: "kira",
    resolvedQuery: "kira",
    mode: "flexible",
    scope: "case",
    limit: 25,
    returned: 2,
    hasMore: false,
    nextCursor: null,
    total: 2,
    filters: filters(),
    hits: [hit("s1"), hit("s2")],
    ...overrides,
  };
}

describe("canonicalFilters", () => {
  it("etiketleri küçültür, sıralar ve boş OCR değerini null yapar", () => {
    const canonical = canonicalFilters(
      filters({ tags: [" Temerrüt ", "kira", "kira"], documentTypes: [" notice ", ""], ocrStatus: "any" }),
    );
    expect(canonical.tags).toEqual(["kira", "temerrüt"]);
    expect(canonical.documentTypes).toEqual(["notice"]);
    expect(canonical.ocrStatus).toBeNull();
    expect(canonical.dateRole).toBeNull();
  });

  it("aynı filtre farklı yazımla eşit sayılır", () => {
    expect(
      filtersEqual(
        filters({ tags: ["Kira"], ocrStatus: "any" }),
        filters({ tags: [" kira "], ocrStatus: null }),
      ),
    ).toBe(true);
  });

  it("farklı filtre eşit sayılmaz", () => {
    expect(filtersEqual(filters({ ocrStatus: "ocr" }), filters({ ocrStatus: "native" }))).toBe(false);
  });
});

describe("filtre özeti", () => {
  it("boş filtre sayılmaz", () => {
    expect(isFiltered(EMPTY_SEARCH_FILTERS)).toBe(false);
    expect(activeFilterCount(EMPTY_SEARCH_FILTERS)).toBe(0);
    expect(describeFilters(EMPTY_SEARCH_FILTERS)).toEqual([]);
  });

  it("etkin filtreleri okunur satırlara çevirir", () => {
    const applied = filters({
      documentTypes: ["contract"],
      dateRole: "notification",
      tags: ["kira", "temerrüt"],
      ocrStatus: "ocr",
    });
    expect(activeFilterCount(applied)).toBe(5);
    const lines = describeFilters(applied);
    expect(lines.some((line) => line.includes("contract"))).toBe(true);
    expect(lines.some((line) => line.includes("Tebliğ"))).toBe(true);
    expect(lines.some((line) => line.includes("kira, temerrüt"))).toBe(true);
    expect(lines.some((line) => line.includes("OCR"))).toBe(true);
  });
});

describe("toggleValue", () => {
  it("yoksa ekler, varsa çıkarır", () => {
    expect(toggleValue([], "kira")).toEqual(["kira"]);
    expect(toggleValue(["kira"], "kira")).toEqual([]);
  });
});

describe("resultSummary", () => {
  it("toplam varsa gösterileni ayrıca yazar", () => {
    expect(resultSummary(page({ total: 12, returned: 10, hasMore: true }))).toBe(
      "12 eşleşme · 10 sonuç gösteriliyor",
    );
  });

  it("toplam yoksa iddia etmez", () => {
    expect(resultSummary(page({ total: null, returned: 5, hasMore: true }))).toBe(
      "5 sonuç gösteriliyor · devamı var",
    );
    expect(resultSummary(page({ total: null, returned: 0, hasMore: false }))).toBe(
      "0 sonuç gösteriliyor",
    );
  });
});

describe("sayfalama", () => {
  it("daha fazla yalnız imleç varken çağrılır", () => {
    expect(canLoadMore(null)).toBe(false);
    expect(canLoadMore(page({ hasMore: true, nextCursor: null }))).toBe(false);
    expect(canLoadMore(page({ hasMore: true, nextCursor: "v1.1.s1" }))).toBe(true);
  });

  it("birleştirmede aynı segment tekrar etmez", () => {
    const merged = mergeHits([hit("s1"), hit("s2")], [hit("s2"), hit("s3")]);
    expect(merged.map((item) => item.segmentId)).toEqual(["s1", "s2", "s3"]);
  });
});

describe("filtersStale", () => {
  it("sonuç yoksa uyarı yok", () => {
    expect(filtersStale(null, EMPTY_SEARCH_FILTERS)).toBe(false);
  });

  it("filtre değiştiyse eski sonuç uyarısı verir", () => {
    const applied = page({ filters: filters({ tags: ["kira"] }) });
    expect(filtersStale(applied, filters({ tags: ["kira"] }))).toBe(false);
    expect(filtersStale(applied, filters({ tags: ["kira", "temerrüt"] }))).toBe(true);
  });
});

describe("textOriginLabel", () => {
  it("bilinmeyen kaynağı uydurmaz", () => {
    expect(textOriginLabel("ocr")).toBe("OCR");
    expect(textOriginLabel("native")).toBe("Yerel metin");
    expect(textOriginLabel(null)).toBe("Kaynak bilinmiyor");
  });
});

describe("arama kipi (F07-T02)", () => {
  it("tırnaklı metni ifade kipine çözer ve tırnakları atar", () => {
    expect(stripWrappingQuotes('"kira bedeli"')).toBe("kira bedeli");
    expect(stripWrappingQuotes("“kira bedeli”")).toBe("kira bedeli");
    expect(stripWrappingQuotes("kira bedeli")).toBeNull();
    expect(stripWrappingQuotes('"  "')).toBeNull();
    expect(resolveSearchMode('"kira bedeli"', "auto")).toBe("phrase");
  });

  it("E./K. numarasını docket kipine çözer", () => {
    expect(isDocketQuery("2024/123 E.")).toBe(true);
    expect(isDocketQuery("2024 / 0123")).toBe(true);
    expect(isDocketQuery("2024/12345")).toBe(true);
    expect(isDocketQuery("2024-123")).toBe(false);
    expect(isDocketQuery("kira bedeli")).toBe(false);
    expect(resolveSearchMode("2024/123 K.", "auto")).toBe("docket");
  });

  it("kip açıkça seçildiyse auto çözümü uygulanmaz", () => {
    expect(resolveSearchMode('"kira"', "exact")).toBe("exact");
    expect(resolveSearchMode("2024/123 E.", "flexible")).toBe("flexible");
    expect(resolveSearchMode("kira", "auto")).toBe("flexible");
  });

  it("ipucu sunucunun çözeceği kipi söyler", () => {
    expect(searchModeHint('"kira bedeli"', "auto")).toContain("İfade");
    expect(searchModeHint("2024/123 E.", "auto")).toContain("E./K.");
    expect(searchModeHint("kira", "exact")).toContain("Türkçe karakterler korunur");
  });
});
