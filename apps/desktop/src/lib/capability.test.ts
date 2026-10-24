import { describe, expect, it } from "vitest";

import type { CliCapabilityAdapter, CliCapabilityMatrix, CliCapabilityRow } from "./api";
import {
  capabilityDetailText,
  capabilityGateText,
  capabilityHelpCommand,
  capabilityReasonLines,
  capabilitySummary,
  capabilityTone,
  capabilityWarnings,
  linkTone,
  matrixWorstLink,
} from "./providers";

function row(overrides: Partial<CliCapabilityRow> = {}): CliCapabilityRow {
  return {
    capability: "tool_free",
    label: "Araçsız çalışma",
    required: true,
    note: "`--tools \"\"` tüm araçları kapatır.",
    state: "declared",
    stateLabel: "bildirildi · denenmedi",
    flags: ["--tools"],
    detail: "--tools",
    ...overrides,
  };
}

function adapter(overrides: Partial<CliCapabilityAdapter> = {}): CliCapabilityAdapter {
  return {
    adapterId: "claude-code",
    displayName: "Claude Code",
    command: "claude",
    version: "2.1.303",
    helpArgs: ["--help"],
    helpRead: true,
    helpError: null,
    link: "declared",
    linkLabel: "bildirildi · denenmedi",
    gateOpen: true,
    gateReasons: [],
    rows: [
      row(),
      row({ capability: "hook_mcp_off", label: "Hook / MCP kapalı", flags: ["--bare", "--strict-mcp-config"] }),
      row({ capability: "structured_output", label: "Şemalı çıktı", flags: ["--json-schema"] }),
      row({ capability: "session_persistence_off", label: "Oturum kalıcılığı kapalı", flags: ["--no-session-persistence"] }),
    ],
    ...overrides,
  };
}

describe("yetenek matrisi sunumu (F10-T11)", () => {
  it("durum tonları: eksik kırmızı, doğrulanamayan sarı, bildirilen de yeşil değil", () => {
    expect(capabilityTone("missing")).toBe("danger");
    expect(capabilityTone("no_path")).toBe("warn");
    expect(capabilityTone("unread")).toBe("warn");
    // `declared` davranış kanıtı değildir; yeşil gösterilmez.
    expect(capabilityTone("declared")).toBe("warn");
    expect(capabilityTone("bilinmiyor")).toBe("plain");
  });

  it("bağlantı tonları uyumsuzu kırmızı, sınırlıyı sarı gösterir", () => {
    expect(linkTone("incompatible")).toBe("danger");
    expect(linkTone("limited")).toBe("warn");
    expect(linkTone("declared")).toBe("warn");
    expect(linkTone("not_installed")).toBe("plain");
  });

  it("özet bildirilen / yok / doğrulanamadı sayılarını ayırır", () => {
    expect(capabilitySummary(adapter())).toBe("4/4 bildirildi · davranış denenmedi");
    const codex = adapter({
      adapterId: "codex-cli",
      link: "limited",
      gateOpen: false,
      gateReasons: ["tool_free:no_path"],
      rows: [
        row({ state: "no_path", stateLabel: "doğrulanmış yol yok", flags: [], detail: "codex exec araçsız kip bildirmiyor" }),
        row({ capability: "hook_mcp_off", label: "Hook / MCP kapalı" }),
        row({ capability: "structured_output", label: "Şemalı çıktı" }),
        row({ capability: "session_persistence_off", label: "Oturum kalıcılığı kapalı" }),
      ],
    });
    expect(capabilitySummary(codex)).toBe("3/4 bildirildi · 1 doğrulanamadı · davranış denenmedi");
    const missing = adapter({
      rows: [row({ state: "missing", stateLabel: "kurulu sürümde yok", flags: ["--json-schema"] }), row(), row(), row()],
    });
    expect(capabilitySummary(missing)).toBe("3/4 bildirildi · 1 yok · davranış denenmedi");
  });

  it("kapı metni açık/kapalı durumu ve gerekçeyi yazar", () => {
    expect(capabilityGateText(adapter())).toContain("Yetenek kapısı açık");
    const limited = adapter({
      link: "limited",
      linkLabel: "sınırlı",
      gateOpen: false,
      gateReasons: ["tool_free:no_path"],
      rows: [
        row({ state: "no_path", stateLabel: "doğrulanmış yol yok", flags: [] }),
        row({ capability: "hook_mcp_off" }),
        row({ capability: "structured_output" }),
        row({ capability: "session_persistence_off" }),
      ],
    });
    const text = capabilityGateText(limited);
    expect(text).toContain("Yetenek kapısı kapalı (sınırlı)");
    expect(text).toContain("Araçsız çalışma: doğrulanmış yol yok");
    const absent = adapter({ link: "not_installed", linkLabel: "kurulu değil", gateOpen: false, rows: [], gateReasons: [] });
    expect(capabilityGateText(absent)).toContain("araç kurulu değil");
  });

  it("gerekçe kodları okunur satıra çevrilir", () => {
    const lines = capabilityReasonLines(
      adapter({
        gateReasons: ["structured_output:missing", "tool_free:no_path", "bilinmeyen:unread"],
        rows: [
          row({ capability: "structured_output", label: "Şemalı çıktı", state: "missing", flags: ["--json-schema"] }),
          row({ state: "no_path", flags: [] }),
        ],
      }),
    );
    expect(lines).toEqual([
      "Şemalı çıktı: kurulu sürümde yok",
      "Araçsız çalışma: doğrulanmış yol yok",
      "bilinmeyen:unread",
    ]);
  });

  it("uyarılar okunamayan yardımı ve eksik bayrağı açıkça söyler", () => {
    const unreadable = adapter({ helpRead: false, helpError: "E_CLI_PROBE", rows: [] });
    expect(capabilityWarnings(unreadable)[0]).toContain("E_CLI_PROBE");
    const missing = adapter({
      rows: [row({ state: "missing", stateLabel: "kurulu sürümde yok", flags: ["--json-schema"] })],
    });
    expect(capabilityWarnings(missing)[0]).toContain("--json-schema");
    expect(capabilityWarnings(adapter())).toEqual([]);
  });

  it("en kötü bağlantı durumu uyumsuzu seçer", () => {
    const matrix: CliCapabilityMatrix = {
      adapters: [
        adapter({ adapterId: "claude-code" }),
        adapter({ adapterId: "codex-cli", link: "limited", linkLabel: "sınırlı" }),
        adapter({ adapterId: "kimi-cli", link: "incompatible", linkLabel: "uyumsuz" }),
        adapter({ adapterId: "antigravity-cli", link: "not_installed", linkLabel: "kurulu değil" }),
      ],
      probedAtUnix: 1,
      helpTimeoutMs: 30000,
      notes: [],
    };
    expect(matrixWorstLink(matrix)).toEqual({ link: "incompatible", label: "uyumsuz" });
    expect(matrixWorstLink({ adapters: [] })).toEqual({ link: "unknown", label: "bilinmiyor" });
  });

  it("yoklanan komut ve ayrıntı metni gerçeği yansıtır", () => {
    expect(capabilityHelpCommand(adapter())).toBe("claude --help");
    expect(capabilityHelpCommand(adapter({ command: "codex", helpArgs: ["exec", "--help"] }))).toBe(
      "codex exec --help",
    );
    expect(capabilityDetailText(row({ detail: "  " }))).toBe("—");
    expect(capabilityDetailText(row({ detail: "eksik: --json-schema" }))).toBe("eksik: --json-schema");
  });
});
