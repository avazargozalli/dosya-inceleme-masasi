import { describe, expect, it } from "vitest";
import type { ExportCoverage } from "./api";
import {
  COVERAGE_EMBED_TARGETS,
  coverageEmbedNote,
  coverageHeadline,
  coverageRows,
  coverageState,
  coverageWarnings,
  emptyCoverageWarning,
  hasCoverageGaps,
} from "./export-coverage";

function coverage(overrides: Partial<ExportCoverage> = {}): ExportCoverage {
  return {
    documents: 4,
    versions: 5,
    parseRuns: 6,
    pages: 40,
    textPages: 38,
    ocrPages: 3,
    segments: 120,
    emptyPages: 2,
    unparsedDocuments: 1,
    failedParseRuns: 1,
    complete: false,
    warnings: [
      "1 belge hiç ayrıştırılmadı; içerikleri incelenmedi.",
      "2 sayfada metin yok (taranmış görüntü olabilir); bu sayfalar okunmadı.",
    ],
    statement: "İncelenmedi ≠ temiz: bu çıktı yalnız kasadaki ayrıştırılmış metni kapsar.",
    generatedAt: "2026-10-25T09:00:00Z",
    ...overrides,
  };
}

describe("coverageRows", () => {
  it("çekirdek sırasını korur ve on satır döner", () => {
    const rows = coverageRows(coverage());
    expect(rows).toHaveLength(10);
    expect(rows[0]?.label).toBe("Belge");
    expect(rows[4]?.label).toBe("Metinli sayfa");
    expect(rows[4]?.value).toBe(38);
    expect(rows[9]?.label).toBe("Başarısız ayrıştırma koşusu");
  });

  it("eksik kapsamı uyarı tonuyla işaretler", () => {
    const rows = coverageRows(coverage());
    expect(rows.find((r) => r.label === "Boş sayfa")?.tone).toBe("warn");
    expect(rows.find((r) => r.label === "OCR ile okunan sayfa")?.tone).toBe("warn");
    expect(rows.find((r) => r.label === "Metinli sayfa")?.tone).toBe("plain");
  });

  it("belgesiz dosyada belge satırı uyarıdır", () => {
    const rows = coverageRows(coverage({ documents: 0 }));
    expect(rows[0]?.tone).toBe("warn");
  });
});

describe("coverageHeadline", () => {
  it("sayıları tek satırda toplar", () => {
    expect(coverageHeadline(coverage())).toBe(
      "4 belge · 40 sayfa (38 metinli, 3 OCR, 2 boş) · 1 ayrıştırılmamış belge",
    );
  });
});

describe("coverageState", () => {
  it("tam kapsamı yeşil gösterir", () => {
    const state = coverageState(
      coverage({ complete: true, warnings: [], emptyPages: 0, unparsedDocuments: 0, failedParseRuns: 0, ocrPages: 0 }),
    );
    expect(state.label).toBe("Kapsam tam");
    expect(state.tone).toBe("ok");
  });

  it("kısmi kapsamı uyarı olarak gösterir", () => {
    const state = coverageState(coverage());
    expect(state.label).toBe("Kapsam kısmi");
    expect(state.tone).toBe("warn");
    expect(state.detail).toContain("incelenmedi ≠ temiz");
  });

  it("belgesiz dosyayı 'kapsam yok' sayar", () => {
    const state = coverageState(coverage({ documents: 0 }));
    expect(state.label).toBe("Kapsam yok");
    expect(state.tone).toBe("warn");
  });
});

describe("coverageWarnings", () => {
  it("çekirdeğin cümlelerini olduğu gibi taşır", () => {
    const warnings = coverageWarnings(coverage());
    expect(warnings).toHaveLength(2);
    expect(warnings[0]).toContain("hiç ayrıştırılmadı");
  });
});

describe("coverageEmbedNote", () => {
  it("her format için gömüldüğü yeri yazar", () => {
    for (const target of COVERAGE_EMBED_TARGETS) {
      const note = coverageEmbedNote(target.id, coverage());
      expect(note).toContain("Kapsam çıktıya gömüldü");
      expect(note).toContain(target.where);
      expect(note).toContain("4 belge · 40 sayfa");
    }
  });

  it("bilinmeyen formatta yeri uydurmaz", () => {
    const note = coverageEmbedNote("xml", coverage());
    expect(note).toBe(
      "Kapsam çıktıya gömüldü: 4 belge · 40 sayfa (38 metinli, 3 OCR, 2 boş) · 1 ayrıştırılmamış belge",
    );
  });
});

describe("hasCoverageGaps", () => {
  it("tam kapsamda boşluk yok", () => {
    expect(hasCoverageGaps(coverage({ complete: true, warnings: [] }))).toBe(false);
  });

  it("uyarı varsa boşluk var", () => {
    expect(hasCoverageGaps(coverage())).toBe(true);
  });
});

describe("emptyCoverageWarning", () => {
  it("belgesiz dosyada uyarı verir", () => {
    expect(emptyCoverageWarning(coverage({ documents: 0 }))).toContain("Belgesiz dosya");
  });

  it("belge varsa null döner", () => {
    expect(emptyCoverageWarning(coverage())).toBeNull();
  });
});
