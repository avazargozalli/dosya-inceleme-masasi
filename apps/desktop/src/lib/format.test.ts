import { describe, expect, it } from "vitest";
import { CASE_KIND_LABEL, formatTimestamp } from "./format";

describe("formatTimestamp", () => {
  it("boş değerde tire döner", () => {
    expect(formatTimestamp(null)).toBe("—");
    expect(formatTimestamp(undefined)).toBe("—");
  });

  it("geçersiz ISO'da tire döner", () => {
    expect(formatTimestamp("bozuk")).toBe("—");
  });

  it("geçerli ISO'yu biçimler", () => {
    const out = formatTimestamp("2026-10-23T10:00:00Z");
    expect(out).toMatch(/2026/);
    expect(out).not.toBe("—");
  });
});

describe("etiketler", () => {
  it("bilinen türleri kapsar", () => {
    expect(CASE_KIND_LABEL.review).toBe("İnceleme dosyası");
  });
});
