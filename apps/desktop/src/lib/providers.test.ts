import { describe, expect, it } from "vitest";

import type { CliProbeView } from "./api";
import {
  formatBytes,
  probeArgv,
  probeCanRun,
  probeOutputs,
  probeScopeLines,
  probeScopeWarnings,
  probeStatusLabel,
  probeSummary,
  probeWarnings,
} from "./providers";

function view(overrides: Partial<CliProbeView> = {}): CliProbeView {
  return {
    adapterId: "claude-code",
    displayName: "Claude Code",
    executable: "/usr/local/bin/claude",
    args: ["--version"],
    envKeys: ["PATH", "HOME", "LANG"],
    workdir: "/var/folders/x/dim-cli-probe-1-0",
    timeoutMs: 15000,
    status: "exited",
    exitCode: 0,
    durationMs: 42,
    stdoutHead: "claude 1.2.3\n",
    stderrHead: "",
    stdoutTruncated: false,
    stderrTruncated: false,
    sandbox: {
      workdir: "/var/folders/x/dim-cli-sandbox-1-0",
      workdirMode: "0700",
      home: "/var/folders/x/dim-cli-sandbox-1-0/home",
      tmp: "/var/folders/x/dim-cli-sandbox-1-0/tmp",
      homeIsolation: "sandbox",
      stdinBytes: 0,
      stdinLimitBytes: 1048576,
      files: [],
      filesTruncated: false,
      totalBytes: 0,
      sizeLimitBytes: 67108864,
      entryLimit: 200,
    },
    ...overrides,
  };
}

describe("bağlantı sınaması sunumu", () => {
  it("çıkış 0 tamamlandı, sıfır olmayan çıkış hata", () => {
    expect(probeStatusLabel(view())).toEqual({ label: "tamamlandı", tone: "ok" });
    expect(probeStatusLabel(view({ exitCode: 127 }))).toEqual({
      label: "çıkış 127",
      tone: "danger",
    });
  });

  it("süre doldu ve iptal uyarı tonunda", () => {
    expect(probeStatusLabel(view({ status: "timed_out", exitCode: null })).tone).toBe("warn");
    expect(probeStatusLabel(view({ status: "cancelled", exitCode: null })).label).toBe(
      "iptal edildi",
    );
    expect(probeStatusLabel(view({ status: "signaled", exitCode: null })).label).toBe(
      "sinyalle bitti",
    );
  });

  it("çalıştırılan komut satırı yolu ve argümanları birlikte gösterir", () => {
    expect(probeArgv(view())).toBe("/usr/local/bin/claude --version");
    expect(probeArgv(view({ args: [] }))).toBe("/usr/local/bin/claude");
  });

  it("özet süre, çıkış kodu ve sınırı yazar", () => {
    expect(probeSummary(view())).toBe("42 ms · çıkış kodu 0 · sınır 15 sn");
    expect(probeSummary(view({ exitCode: null, durationMs: 15000 }))).toBe("15000 ms · sınır 15 sn");
  });

  it("kesilen çıktı için uyarı üretir", () => {
    expect(probeWarnings(view())).toEqual([]);
    expect(probeWarnings(view({ stdoutTruncated: true }))).toHaveLength(1);
    expect(probeWarnings(view({ stdoutTruncated: true, stderrTruncated: true }))).toHaveLength(2);
  });

  it("boş akışlar gösterilmez, dolu olanlar sırayla döner", () => {
    expect(probeOutputs(view({ stdoutHead: "", stderrHead: "" }))).toEqual([]);
    expect(probeOutputs(view({ stdoutHead: "  ", stderrHead: "hata\n" }))).toEqual([
      { stream: "stderr", text: "hata\n" },
    ]);
    expect(probeOutputs(view({ stdoutHead: "a", stderrHead: "b" })).map((o) => o.stream)).toEqual([
      "stdout",
      "stderr",
    ]);
  });

  it("düğme yalnız kurulu, otomatik yürütmeye açık ve boşta sağlayıcıda etkin", () => {
    expect(probeCanRun({ state: "installed", batchForbidden: false }, null)).toBe(true);
    expect(probeCanRun({ state: "installed", batchForbidden: false }, "codex-cli")).toBe(false);
    expect(probeCanRun({ state: "not_installed", batchForbidden: false }, null)).toBe(false);
    expect(probeCanRun({ state: "installed", batchForbidden: true }, null)).toBe(false);
  });

  it("bayt biçimi Türkçe ondalıkla okunur", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1023)).toBe("1023 B");
    expect(formatBytes(1024)).toBe("1,0 KiB");
    expect(formatBytes(20480)).toBe("20 KiB");
    expect(formatBytes(1048576)).toBe("1,0 MiB");
  });

  it("kapsam satırları izin, HOME politikası, stdin ve dosya sayısını yazar", () => {
    const lines = probeScopeLines(view());
    expect(lines.map((l) => l.label)).toEqual(["İzin", "HOME / TMPDIR", "stdin", "Bırakılan dosya"]);
    expect(lines[0]?.value).toBe("0700 yalnız kullanıcı");
    expect(lines[1]?.value).toBe("kum havuzunda");
    expect(lines[2]?.value).toBe("0 B / 1,0 MiB");
    expect(lines[3]?.value).toBe("yok");

    const withFiles = view({
      sandbox: { ...view().sandbox, files: [{ name: "home/a.json", bytes: 12 }], totalBytes: 12 },
    });
    expect(probeScopeLines(withFiles)[3]?.value).toBe("1 dosya · 12 B");

    const userHome = view({ sandbox: { ...view().sandbox, homeIsolation: "user_home", home: null } });
    expect(probeScopeLines(userHome)[1]?.value).toBe("kullanıcı ev dizini (açık seçim)");

    const unverified = view({ sandbox: { ...view().sandbox, workdirMode: null } });
    expect(probeScopeLines(unverified)[0]?.value).toBe("doğrulanamadı (Unix dışı)");
  });

  it("kapsam uyarıları yalnız gerçek sorunda üretilir", () => {
    expect(probeScopeWarnings(view())).toEqual([]);
    expect(
      probeScopeWarnings(view({ sandbox: { ...view().sandbox, filesTruncated: true } })),
    ).toHaveLength(1);
    expect(
      probeScopeWarnings(
        view({
          sandbox: { ...view().sandbox, totalBytes: 70 * 1024 * 1024 },
        }),
      ),
    ).toHaveLength(1);
    expect(probeScopeWarnings(view({ sandbox: { ...view().sandbox, homeIsolation: "user_home" } }))).toHaveLength(1);
    expect(probeScopeWarnings(view({ sandbox: { ...view().sandbox, workdirMode: null } }))).toHaveLength(1);
  });
});
