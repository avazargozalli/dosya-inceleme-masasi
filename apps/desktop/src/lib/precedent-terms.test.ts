import { describe, expect, it } from "vitest";
import type { CorpusCoverage, PrecedentUsageTerms } from "./api";
import {
  corpusGateOpen,
  corpusGateText,
  corpusResultText,
  coverageText,
  exclusionReasonLabel,
  needsWrittenBasis,
  redistributionLabel,
  redistributionTone,
  termsBadge,
  termsRecordedText,
  usageBasisLabel,
} from "./precedent-terms";

function coverage(overrides: Partial<CorpusCoverage>): CorpusCoverage {
  return {
    total: 0,
    withText: 0,
    allowed: 0,
    notAllowed: 0,
    unknown: 0,
    blockedTotal: 0,
    blocked: [],
    ...overrides,
  };
}

function terms(overrides: Partial<PrecedentUsageTerms>): PrecedentUsageTerms {
  return {
    precedentId: "p1",
    usageBasis: "unknown",
    redistribution: "unknown",
    termsUrl: null,
    termsNote: "",
    termsRecordedAt: null,
    ...overrides,
  };
}

describe("kaynak kullanım koşulu metinleri", () => {
  it("kayıt yokluğunu izin gibi göstermez", () => {
    expect(redistributionLabel("unknown")).toBe("izin bilinmiyor");
    expect(redistributionTone("unknown")).toBe("muted");
    expect(termsBadge({ redistribution: "unknown" })).toEqual({
      text: "izin bilinmiyor",
      tone: "warn",
    });
    expect(termsRecordedText(terms({}))).toBe("koşul kaydı yok — izin varsayılmaz");
  });

  it("izin durumlarını ayrı etiketler ve tonlar", () => {
    expect(redistributionLabel("allowed")).toBe("yeniden dağıtım izinli");
    expect(redistributionLabel("not_allowed")).toBe("yeniden dağıtım izinsiz");
    expect(termsBadge({ redistribution: "allowed" })).toEqual({ text: "izinli", tone: "ok" });
    expect(termsBadge({ redistribution: "not_allowed" })).toEqual({
      text: "izinsiz",
      tone: "danger",
    });
  });

  it("dayanak türlerini Türkçe yazar, bilinmeyeni gizlemez", () => {
    expect(usageBasisLabel("official_public")).toBe("resmî yayın (kamuya açık)");
    expect(usageBasisLabel("licence")).toBe("açık lisans");
    expect(usageBasisLabel("permission")).toBe("yazılı izin");
    expect(usageBasisLabel("internal_only")).toBe("yalnız iç kullanım");
    expect(usageBasisLabel("unknown")).toBe("bilinmiyor");
  });

  it("kayıt zamanını biçimler; kayıt yoksa uydurmaz", () => {
    expect(termsRecordedText(terms({ termsRecordedAt: "2026-10-24T15:03:21+03:00" }))).toBe(
      "koşul kaydı: 2026-10-24 15:03",
    );
  });

  it("kapsam özeti her sayıyı yazar", () => {
    const text = coverageText(
      coverage({ total: 5, withText: 3, allowed: 1, notAllowed: 1, unknown: 1, blockedTotal: 2 }),
    );
    expect(text).toBe("5 karar · 3 tam metinli · 1 izinli · 1 izinsiz · 1 izin bilinmiyor");
    expect(coverageText(coverage({ total: 2, withText: 2, allowed: 2 }))).toBe(
      "2 karar · 2 tam metinli · 2 izinli · 0 izin bilinmiyor",
    );
  });

  it("kapı yalnız metinli her kayıt izinliyse açıktır", () => {
    expect(corpusGateOpen(coverage({ total: 2, withText: 2, allowed: 2 }))).toBe(true);
    expect(corpusGateOpen(coverage({ total: 2, withText: 2, allowed: 1, unknown: 1 }))).toBe(false);
    // Metinsiz kayıt dağıtılacak metin taşımaz; kapıyı kapatmaz.
    expect(corpusGateOpen(coverage({ total: 3, withText: 1, allowed: 1 }))).toBe(true);
    expect(corpusGateOpen(coverage({ total: 3, withText: 0 }))).toBe(false);
  });

  it("kapı metni nedeni ve kipi söyler", () => {
    expect(corpusGateText(coverage({ total: 2, withText: 0 }))).toBe(
      "Tam metinli karar yok; aktarılacak korpus oluşmaz.",
    );
    expect(corpusGateText(coverage({ total: 2, withText: 2, allowed: 2 }))).toContain(
      "toplu korpus aktarılabilir",
    );
    expect(
      corpusGateText(coverage({ total: 3, withText: 3, allowed: 1, unknown: 2, blockedTotal: 2 })),
    ).toContain("2 tam metinli kayıtta yeniden dağıtım izni yok");
  });

  it("dışarıda kalma gerekçesini ayrı yazar", () => {
    expect(exclusionReasonLabel("redistribution_not_allowed")).toBe("yeniden dağıtım izinsiz");
    expect(exclusionReasonLabel("redistribution_unknown")).toBe("izin kaydı yok");
  });

  it("aktarım sonucunu sayı ve hash ile özetler", () => {
    const text = corpusResultText({
      path: "/tmp/korpus.json",
      bytes: 1024,
      sha256: "abcdef0123456789",
      exported: 2,
      excluded: 1,
      contentHash: "ffff",
    });
    expect(text).toBe("2 karar aktarıldı · 1 kayıt izin nedeniyle dışarıda · sha256 abcdef012345…");
    expect(
      corpusResultText({
        path: "/tmp/k.json",
        bytes: 1,
        sha256: "0123456789abcdef",
        exported: 1,
        excluded: 0,
        contentHash: "x",
      }),
    ).toBe("1 karar aktarıldı · sha256 0123456789ab…");
  });

  it("yazılı dayanak yalnız 'izinli' için gerekir", () => {
    expect(needsWrittenBasis("allowed")).toBe(true);
    expect(needsWrittenBasis("not_allowed")).toBe(false);
    expect(needsWrittenBasis("unknown")).toBe(false);
  });
});
