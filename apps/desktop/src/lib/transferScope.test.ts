import { describe, expect, it } from "vitest";
import type { ConsentPreview } from "./api";
import { consentScopeLine, skipReasonLabel, skippedSummary } from "./transferScope";

function preview(overrides: Partial<ConsentPreview> = {}): ConsentPreview {
  return { documents: 2, segments: 5, bytes: 120, sample: "", skipped: [], ...overrides };
}

describe("aktarım kapsamı", () => {
  it("kapsam dışı nedenleri okunur yazar", () => {
    expect(skipReasonLabel("not_parsed")).toBe("ayrıştırılmamış");
    expect(skipReasonLabel("empty_text")).toBe("metin yok");
    expect(skipReasonLabel("no_current_version")).toBe("sürüm yok");
    expect(skipReasonLabel("missing")).toBe("bulunamadı");
    // Bilinmeyen neden yutulmaz: sayılır ve "bulunamadı" diye yazılır.
    expect(skipReasonLabel("yeni_neden")).toBe("bulunamadı");
  });

  it("kapsam dışı belgeleri nedene göre sayar", () => {
    expect(skippedSummary([])).toBe("");
    expect(
      skippedSummary([
        { documentId: "d1", reason: "not_parsed" },
        { documentId: "d2", reason: "not_parsed" },
        { documentId: "d3", reason: "empty_text" },
      ]),
    ).toBe("Gönderilmeyen 3 belge: 2 ayrıştırılmamış, 1 metin yok");
  });

  it("onay sayısı ile gönderilecek içeriği birlikte yazar", () => {
    expect(consentScopeLine(3, null)).toBe(
      "Onay kapsamı: 3 belge · içerik önizlemesi alınmadı",
    );
    expect(consentScopeLine(3, preview())).toBe(
      "Onay kapsamı: 3 belge · gönderilecek: 2 belge, 5 segment, 120 bayt",
    );
  });
});
