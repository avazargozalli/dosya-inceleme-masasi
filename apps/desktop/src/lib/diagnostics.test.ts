// Teşhis yardımcıları (REQ-033): saf yüklem + **sözleşme kapısı**.
//
// Kart davranışı `components/DiagnosticsCard.test.tsx`te ölçülür. Buradaki ikinci
// blok kaynak düzeyinde bir kapıdır: kart "sürüm çekirdekten" diyorsa komut
// gerçekten crate üstverisinden okumalı, kayıtlı olmalı ve arayüz bağı bulunmalı.
// Kapı, iddianın sessizce bozulmasını (gömülü sabit sürüm, kaldırılmış kayıt,
// kopmuş `api` bağı) yakalar.

import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import {
  SUPPORTED_PROTOCOL,
  parseSystemInfo,
  platformLabel,
  protocolDrift,
  versionDrift,
} from "./diagnostics";

describe("teşhis yardımcıları", () => {
  it("bilinen platform kodlarını okunur ada çevirir", () => {
    expect(platformLabel("macos")).toBe("macOS");
    expect(platformLabel("windows")).toBe("Windows");
    expect(platformLabel("linux")).toBe("Linux");
    expect(platformLabel("MACOS")).toBe("macOS");
  });

  it("tanınmayan platform kodunu ham bırakır (uydurma ad yok)", () => {
    expect(platformLabel("plan9")).toBe("plan9");
    expect(platformLabel("  haiku  ")).toBe("haiku");
  });

  it("sürüm ayrışmasını kırpılmış karşılaştırmayla bulur", () => {
    expect(versionDrift("0.1.0", "0.1.0")).toBe(false);
    expect(versionDrift(" 0.1.0 ", "0.1.0")).toBe(false);
    expect(versionDrift("0.1.0", "0.2.0")).toBe(true);
    expect(versionDrift("0.1.0", "")).toBe(true);
  });

  it("protokol ayrışmasını çekirdeğin bildirdiği sürümden bulur", () => {
    expect(protocolDrift(SUPPORTED_PROTOCOL)).toBe(false);
    expect(protocolDrift(SUPPORTED_PROTOCOL + 1)).toBe(true);
  });

  it("sözleşme dışı künyeyi reddeder (eksik alan sürüm sayılmaz)", () => {
    expect(parseSystemInfo({ appVersion: "0.1.0", protocolVersion: 1, platform: "macos" })).toEqual({
      appVersion: "0.1.0",
      protocolVersion: 1,
      platform: "macos",
    });
    expect(parseSystemInfo({ protocolVersion: 1, platform: "macos" })).toBeNull();
    expect(parseSystemInfo({ appVersion: "  ", protocolVersion: 1, platform: "macos" })).toBeNull();
    expect(parseSystemInfo({ appVersion: "0.1.0", protocolVersion: 1.5, platform: "macos" })).toBeNull();
    expect(parseSystemInfo({ appVersion: "0.1.0", protocolVersion: "1", platform: "macos" })).toBeNull();
    expect(parseSystemInfo({ appVersion: "0.1.0", protocolVersion: 1, platform: "" })).toBeNull();
    expect(parseSystemInfo(null)).toBeNull();
    expect(parseSystemInfo(undefined)).toBeNull();
    expect(parseSystemInfo("0.1.0")).toBeNull();
  });
});

describe("sözleşme: teşhis komutu gerçekten bağlı", () => {
  const commands = readFileSync(
    new URL("../../src-tauri/src/commands.rs", import.meta.url),
    "utf8",
  );
  const libRs = readFileSync(new URL("../../src-tauri/src/lib.rs", import.meta.url), "utf8");
  const apiSource = readFileSync(new URL("./api.ts", import.meta.url), "utf8");
  const start = commands.indexOf("pub fn system_info()");
  const body = start < 0 ? "" : commands.slice(start, commands.indexOf("#[tauri::command]", start));

  it("çekirdek sürümü crate üstverisinden okunur, protokol sözleşmesi aynıdır", () => {
    expect(body).toContain('env!("CARGO_PKG_VERSION")');
    expect(body).toContain("protocol_version: 1");
    expect(SUPPORTED_PROTOCOL).toBe(1);
  });

  it("komut kayıtlıdır ve arayüz bağı vardır", () => {
    expect(libRs).toContain("commands::system_info");
    expect(apiSource).toContain('invoke<SystemInfo>("system_info")');
  });
});
