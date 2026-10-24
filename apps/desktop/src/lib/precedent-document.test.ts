import { describe, expect, it } from "vitest";
import type { PrecedentDocumentText, PrecedentVersionRow } from "./api";
import {
  documentTextSummary,
  documentTextWarnings,
  transferBlockedReason,
  versionOptionLabel,
} from "./precedent-document";

function versionRow(overrides: Partial<PrecedentVersionRow>): PrecedentVersionRow {
  return {
    id: "pv1",
    precedentId: "p1",
    documentVersionId: "dv1",
    acquiredAt: "2026-10-24T09:15:00Z",
    sourceKind: "document",
    bodyHash: "abcd1234ef567890".padEnd(64, "0"),
    verificationState: "text_verified",
    charCount: 8412,
    hasText: true,
    identityConflict: false,
    conflictWithVersionId: null,
    ...overrides,
  };
}

function result(overrides: Partial<PrecedentDocumentText>): PrecedentDocumentText {
  return {
    version: versionRow({}),
    parseRunId: "run-1",
    engine: "pymupdf",
    parseStatus: "succeeded",
    pages: 12,
    ocrPages: 0,
    emptyPages: 0,
    charCount: 8412,
    written: true,
    notes: [],
    ...overrides,
  };
}

describe("versionOptionLabel", () => {
  it("sürüm numarasını şemadan alır, uydurmaz", () => {
    const label = versionOptionLabel({ versionNo: 2, importedAt: "2026-03-12T11:05:00Z" });
    expect(label.startsWith("v2 · ")).toBe(true);
    expect(label).toContain("2026");
  });

  it("geçersiz tarihte sürüm numarasını yine gösterir", () => {
    const label = versionOptionLabel({ versionNo: 7, importedAt: "bozuk" });
    expect(label).toBe("v7 · —");
  });
});

describe("transferBlockedReason", () => {
  const open = {
    hasPrecedent: true,
    documentCount: 3,
    versionCount: 2,
    selectedVersionId: "dv1",
  };

  it("seçimler tamamsa engel yok", () => {
    expect(transferBlockedReason(open)).toBeNull();
  });

  it("emsal seçilmeden aktarım başlamaz", () => {
    expect(transferBlockedReason({ ...open, hasPrecedent: false })).toContain("emsal");
  });

  it("belge yoksa nedenini yazar", () => {
    expect(transferBlockedReason({ ...open, documentCount: 0 })).toContain("belge yok");
  });

  it("sürüm yoksa nedenini yazar", () => {
    expect(transferBlockedReason({ ...open, versionCount: 0 })).toContain("sürümü yok");
  });

  it("sürüm seçilmeden sessizce devam etmez", () => {
    expect(transferBlockedReason({ ...open, selectedVersionId: "   " })).toContain("seçin");
  });
});

describe("documentTextSummary", () => {
  it("sayfa, karakter ve hash özetini verir", () => {
    const text = documentTextSummary(result({}));
    expect(text).toContain("12 sayfa");
    expect(text).toContain("8412 karakter");
    expect(text).toContain("sha256 abcd1234ef56…");
  });
});

describe("documentTextWarnings", () => {
  it("temiz aktarımda uyarı üretmez", () => {
    expect(documentTextWarnings(result({}))).toEqual([]);
  });

  it("kısmi ayrıştırmayı tam gibi göstermez", () => {
    const warnings = documentTextWarnings(result({ parseStatus: "partial" }));
    expect(warnings.some((w) => w.includes("kısmi"))).toBe(true);
  });

  it("OCR ve metinsiz sayfaları ayrı ayrı yazar", () => {
    const warnings = documentTextWarnings(result({ ocrPages: 3, emptyPages: 2 }));
    expect(warnings.some((w) => w.includes("3 sayfa OCR"))).toBe(true);
    expect(warnings.some((w) => w.includes("2 sayfa metinsiz"))).toBe(true);
  });

  it("tekrar yazılmayan metni 'yazıldı' gibi göstermez", () => {
    const warnings = documentTextWarnings(result({ written: false }));
    expect(warnings.some((w) => w.includes("yeni sürüm yazılmadı"))).toBe(true);
  });
});
