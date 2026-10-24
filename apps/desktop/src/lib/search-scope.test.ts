import { describe, expect, it } from "vitest";
import type { PrecedentSearchHit, PrecedentSearchPage } from "./api";
import {
  precedentCitation,
  precedentMatchLabel,
  precedentSearchSummary,
  precedentSnippetForCopy,
  scopeNotice,
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
