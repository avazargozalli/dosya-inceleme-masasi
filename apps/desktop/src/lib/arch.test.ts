import { describe, expect, it } from "vitest";

import type { AdapterArch, CliArchAdapter, CliArchReport } from "./api";
import {
  adapterArchBadge,
  archBadgeText,
  archFormatText,
  archInterpreterText,
  archList,
  archPathText,
  archProbedAt,
  archReasonText,
  archSummaryText,
  archTone,
  archWarnings,
  archWorst,
} from "./arch";

function archAdapter(overrides: Partial<CliArchAdapter> = {}): CliArchAdapter {
  return {
    adapterId: "claude-code",
    displayName: "Claude Code",
    command: "claude",
    installed: true,
    path: "/Users/me/.npm-global/bin/claude",
    resolved: "/Users/me/.npm-global/lib/node_modules/@anthropic-ai/claude-code/bin/claude.exe",
    symlink: true,
    format: "mach_o",
    formatLabel: "Mach-O",
    architectures: ["aarch64"],
    comparison: "same",
    comparisonLabel: "aynı mimari",
    containsAppArch: true,
    summary: "aarch64 · aynı mimari",
    interpreter: null,
    reason: null,
    headerBytes: 4096,
    ...overrides,
  };
}

function report(overrides: Partial<CliArchReport> = {}): CliArchReport {
  return {
    appArch: "aarch64",
    probedAtUnix: 1_793_000_000,
    adapters: [archAdapter()],
    same: 1,
    different: 0,
    fat: 0,
    unknown: 0,
    notes: [],
    ...overrides,
  };
}

describe("mimari uyum sunumu (F10-T01)", () => {
  it("tonlar: aynı yeşil, farklı kırmızı, evrensel sarı, bilinmiyor nötr", () => {
    expect(archTone("same")).toBe("ok");
    expect(archTone("different")).toBe("danger");
    expect(archTone("fat")).toBe("warn");
    expect(archTone("unknown")).toBe("plain");
    expect(archTone("başka")).toBe("plain");
  });

  it("rozet metni evrenselde uygulama mimarisini ayırır", () => {
    expect(archBadgeText({ comparison: "same", containsAppArch: true })).toBe("mimari aynı");
    expect(archBadgeText({ comparison: "different", containsAppArch: false })).toBe(
      "mimari farklı",
    );
    expect(archBadgeText({ comparison: "fat", containsAppArch: true })).toContain(
      "uygulama mimarisi var",
    );
    expect(archBadgeText({ comparison: "fat", containsAppArch: false })).toContain(
      "uygulama mimarisi yok",
    );
    expect(archBadgeText({ comparison: "fat", containsAppArch: null })).toBe("evrensel");
    expect(archBadgeText({ comparison: "unknown", containsAppArch: null })).toBe(
      "mimari bilinmiyor",
    );
  });

  it("mimari listesi boşsa tire gösterir, doluysa artı ile birleştirir", () => {
    expect(archList([])).toBe("—");
    expect(archList(["aarch64"])).toBe("aarch64");
    expect(archList(["x86_64", "aarch64"])).toBe("x86_64 + aarch64");
  });

  it("biçim satırı sembolik bağı yazar", () => {
    expect(archFormatText({ formatLabel: "Mach-O", symlink: false })).toBe("Mach-O");
    expect(archFormatText({ formatLabel: "Mach-O", symlink: true })).toBe(
      "Mach-O · sembolik bağ",
    );
  });

  it("yol satırı: kurulu değilse açıkça söyler, bağ çözümünü gösterir", () => {
    expect(archPathText({ installed: false, path: null, resolved: null })).toBe(
      "PATH üzerinde bulunamadı",
    );
    expect(
      archPathText({ installed: true, path: "/usr/local/bin/x", resolved: "/usr/local/bin/x" }),
    ).toBe("/usr/local/bin/x");
    expect(
      archPathText({ installed: true, path: "/usr/local/bin/x", resolved: "/opt/x" }),
    ).toBe("/usr/local/bin/x → /opt/x");
  });

  it("neden kodlarını Türkçeye çevirir, bilinmeyeni olduğu gibi bırakır", () => {
    expect(archReasonText(null)).toBeNull();
    expect(archReasonText("magic_unrecognized")).toBe("başlık tanınmadı");
    expect(archReasonText("interpreter_unresolved")).toBe(
      "shebang yorumlayıcısı bulunamadı",
    );
    expect(archReasonText("pe_offset_too_large")).toBe("PE başlık konumu sınır dışı");
    expect(archReasonText("yeni_kod")).toBe("yeni_kod");
  });

  it("shebang satırı yorumlayıcı yolunu ve mimarisini taşır", () => {
    expect(archInterpreterText({ interpreter: null })).toBeNull();
    const text = archInterpreterText({
      interpreter: {
        command: "/usr/bin/env python",
        path: "/usr/bin/python3",
        resolved: "/opt/python3",
        format: "mach_o",
        architectures: ["aarch64", "x86_64"],
        comparison: "fat",
        reason: null,
      },
    });
    expect(text).toContain("/usr/bin/env python");
    expect(text).toContain("/opt/python3");
    expect(text).toContain("aarch64 + x86_64");
  });

  it("özet satırı uygulama mimarisini ve sayımları yazar", () => {
    const text = archSummaryText(
      report({ same: 2, different: 1, fat: 1, unknown: 0, adapters: [archAdapter(), archAdapter()] }),
    );
    expect(text).toContain("uygulama aarch64");
    expect(text).toContain("2 adaptör");
    expect(text).toContain("2 aynı");
    expect(text).toContain("1 farklı");
    expect(text).toContain("1 evrensel");
  });

  it("en kötü karar sırası: farklı > doğrulanamadı > evrensel > aynı", () => {
    expect(archWorst({ same: 3, different: 1, fat: 1, unknown: 1 }).tone).toBe("danger");
    expect(archWorst({ same: 3, different: 0, fat: 1, unknown: 1 }).tone).toBe("plain");
    expect(archWorst({ same: 3, different: 0, fat: 1, unknown: 0 }).tone).toBe("warn");
    expect(archWorst({ same: 3, different: 0, fat: 0, unknown: 0 }).tone).toBe("ok");
    expect(archWorst({ same: 0, different: 0, fat: 0, unknown: 0 }).label).toBe(
      "denetlenecek ikili yok",
    );
  });

  it("farklı mimari uyarısı adaptör adlarını listeler", () => {
    const warnings = archWarnings(
      report({
        adapters: [
          archAdapter(),
          archAdapter({ adapterId: "codex-cli", displayName: "Codex CLI", comparison: "different" }),
        ],
        different: 1,
      }),
    );
    expect(warnings).toHaveLength(1);
    expect(warnings[0]).toContain("Codex CLI");
    expect(warnings[0]).toContain("farklı mimaride");
  });

  it("evrensel ama uygulama mimarisi yoksa ayrı uyarı verir", () => {
    const warnings = archWarnings(
      report({
        adapters: [
          archAdapter({
            comparison: "fat",
            containsAppArch: false,
            architectures: ["x86_64", "ppc"],
          }),
        ],
        same: 0,
        fat: 1,
      }),
    );
    expect(warnings.some((line) => line.includes("uygulama mimarisini içermiyor"))).toBe(true);
  });

  it("doğrulanamayan adaptör 'aynı' sayılmaz ve uyarılır", () => {
    const warnings = archWarnings(
      report({
        adapters: [
          archAdapter({ comparison: "unknown", containsAppArch: null, reason: "not_found" }),
        ],
        same: 0,
        unknown: 1,
      }),
    );
    expect(warnings.some((line) => line.includes("doğrulanamadı"))).toBe(true);
    expect(warnings.some((line) => line.includes('"aynı" sayılmaz'))).toBe(true);
  });

  it("temiz raporda uyarı yok", () => {
    expect(archWarnings(report())).toEqual([]);
  });

  it("adaptör kartı rozeti veri yoksa nötr kalır", () => {
    const missing = adapterArchBadge(null);
    expect(missing.tone).toBe("plain");
    expect(missing.label).toBe("mimari bilinmiyor");
    const arch: AdapterArch = {
      format: "mach_o",
      formatLabel: "Mach-O",
      architectures: ["aarch64"],
      comparison: "same",
      comparisonLabel: "aynı mimari",
      containsAppArch: true,
      reason: null,
      summary: "aarch64 · aynı mimari",
    };
    expect(adapterArchBadge(arch)).toEqual({ label: "mimari aynı", tone: "ok" });
  });

  it("zaman damgası geçersizse tire döner", () => {
    expect(archProbedAt(0)).toBe("—");
    expect(archProbedAt(Number.NaN)).toBe("—");
    expect(archProbedAt(1_793_000_000)).not.toBe("—");
  });
});
