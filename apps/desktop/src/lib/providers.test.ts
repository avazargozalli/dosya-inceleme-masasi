import { describe, expect, it } from "vitest";

import type {
  CliHardeningProbe,
  CliProbeView,
  CliResidueMeasure,
  CliResidueScan,
} from "./api";
import {
  adapterAliasName,
  capabilityWarnings,
  evidenceGateText,
  formatBytes,
  hardeningProbeText,
  measuredAccessLabel,
  measuredEvidenceLine,
  notRunLabel,
  persistenceText,
  probeArgv,
  probeCanRun,
  probeOutputs,
  probeScopeLines,
  probeScopeWarnings,
  probeStatusLabel,
  probeSummary,
  probeWarnings,
  promptShapeText,
  residueAge,
  residueCleanupSummary,
  residueCleanupTargets,
  residueMeasureSummary,
  residueMeasureWarnings,
  residuePathLine,
  residueScanWarnings,
  sandboxSweepSummary,
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
    expect(formatBytes(0)).toBe("0 bayt");
    expect(formatBytes(1023)).toBe("1.023 bayt");
    expect(formatBytes(1024)).toBe("1 KiB");
    expect(formatBytes(20480)).toBe("20 KiB");
    expect(formatBytes(1048576)).toBe("1 MiB");
  });

  it("kapsam satırları izin, HOME politikası, stdin ve dosya sayısını yazar", () => {
    const lines = probeScopeLines(view());
    expect(lines.map((l) => l.label)).toEqual(["İzin", "HOME / TMPDIR", "stdin", "Bırakılan dosya"]);
    expect(lines[0]?.value).toBe("0700 yalnız kullanıcı");
    expect(lines[1]?.value).toBe("kum havuzunda");
    expect(lines[2]?.value).toBe("0 bayt / 1 MiB");
    expect(lines[3]?.value).toBe("yok");

    const withFiles = view({
      sandbox: { ...view().sandbox, files: [{ name: "home/a.json", bytes: 12 }], totalBytes: 12 },
    });
    expect(probeScopeLines(withFiles)[3]?.value).toBe("1 dosya · 12 bayt");

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

function residueScan(overrides: Partial<CliResidueScan> = {}): CliResidueScan {
  return {
    adapterId: "claude-code",
    root: "/Users/avukat",
    paths: [
      {
        relative: ".claude/projects",
        kind: "transcript",
        note: "proje başına oturum transkriptleri",
        exists: true,
        files: 3,
        bytes: 4096,
        truncated: false,
        newestAgeSecs: 120,
      },
    ],
    totalFiles: 3,
    totalBytes: 4096,
    truncated: false,
    fileLimit: 4000,
    persistenceFlag: "--no-session-persistence",
    persistenceAppliesTo: "--print",
    persistenceVerifiedWith: "2.1.303",
    ...overrides,
  };
}

function residueMeasure(overrides: Partial<CliResidueMeasure> = {}): CliResidueMeasure {
  return {
    adapterId: "claude-code",
    root: "/Users/avukat",
    measuredAtUnix: 1_760_000_000,
    usedUserHome: true,
    status: "exited",
    exitCode: 0,
    durationMs: 220,
    args: ["--version"],
    before: residueScan({ totalFiles: 3, totalBytes: 4096 }),
    after: residueScan({ totalFiles: 4, totalBytes: 4180 }),
    created: [{ relative: ".claude/projects/yeni.jsonl", bytes: 84 }],
    modified: [],
    removed: [],
    createdDirs: [],
    bytesAdded: 84,
    cleanupHint: "Yalnız bu ölçümde oluşan dosyalar silinebilir.",
    ...overrides,
  };
}

describe("residue", () => {
  it("yaş etiketi kısa ve Türkçe", () => {
    expect(residueAge(null)).toBe("—");
    expect(residueAge(5)).toBe("az önce");
    expect(residueAge(120)).toBe("2 dk");
    expect(residueAge(3 * 3600)).toBe("3 sa");
    expect(residueAge(5 * 86400)).toBe("5 gün");
  });

  it("yol satırı yok/boş/dolu ayrımını korur", () => {
    expect(residuePathLine({ exists: false, files: 0, bytes: 0, truncated: false })).toBe("yok");
    expect(residuePathLine({ exists: true, files: 0, bytes: 0, truncated: false })).toBe("boş");
    expect(residuePathLine({ exists: true, files: 3, bytes: 4096, truncated: false })).toBe(
      "3 dosya · 4 KiB",
    );
    expect(residuePathLine({ exists: true, files: 3, bytes: 4096, truncated: true })).toContain(
      "liste kesik",
    );
  });

  it("kalıcılık satırı doğrulanmış bayrağı sürümle yazar", () => {
    expect(persistenceText(residueScan())).toContain("--no-session-persistence");
    expect(persistenceText(residueScan())).toContain("2.1.303");
    expect(
      persistenceText(
        residueScan({
          persistenceFlag: null,
          persistenceAppliesTo: "",
          persistenceVerifiedWith: null,
        }),
      ),
    ).toContain("doğrulanmış kapatma bayrağı yok");
  });

  it("tarama kesilirse toplamların eksik olduğu söylenir", () => {
    expect(residueScanWarnings(residueScan())).toEqual([]);
    const warnings = residueScanWarnings(residueScan({ truncated: true }));
    expect(warnings).toHaveLength(1);
    expect(warnings[0]).toContain("4000");
  });

  it("ölçüm uyarıları yalnız gerçek durumda üretilir", () => {
    expect(residueMeasureWarnings(residueMeasure())).toHaveLength(2); // user home + created
    expect(
      residueMeasureWarnings(residueMeasure({ usedUserHome: false, created: [] })),
    ).toEqual([]);
    expect(residueMeasureWarnings(residueMeasure({ modified: [{ relative: ".codex/log/tui.log", bytes: 9 }] }))).toHaveLength(3);
    expect(residueMeasureWarnings(residueMeasure({ status: "cancelled" }))).toHaveLength(3);
  });

  it("temizlik hedefi yalnız oluşan dosyalar", () => {
    const measure = residueMeasure({
      modified: [{ relative: ".codex/log/tui.log", bytes: 9 }],
    });
    expect(residueCleanupTargets(measure)).toEqual([".claude/projects/yeni.jsonl"]);
  });

  it("ölçüm ve temizlik özetleri sayıları taşır", () => {
    expect(residueMeasureSummary(residueMeasure())).toBe("220 ms · 1 yeni dosya · 0 değişen · +84 bayt");
    expect(
      residueCleanupSummary({
        removedFiles: 1,
        removedBytes: 84,
        removedDirs: 2,
        skipped: [{ path: "x", reason: "kapsam dışı" }],
        errors: [],
      }),
    ).toBe("1 dosya silindi · 84 bayt · 2 boş dizin · 1 atlandı");
  });

  it("kum havuzu süpürme özeti yaş eşiğini dakikaya çevirir", () => {
    expect(
      sandboxSweepSummary({ considered: 2, removed: ["a"], skipped: [], minAgeSecs: 3600 }),
    ).toBe("2 aday · 1 silindi · 0 atlandı · en az 60 dk yaş");
  });

  it("eksik yetenek uyarısı bayraksız satırda tam cümle kurar", () => {
    const warnings = capabilityWarnings({
      helpRead: true,
      helpError: null,
      rows: [
        {
          capability: "print",
          label: "Toplu yazdırma",
          required: true,
          note: "",
          state: "missing",
          stateLabel: "okunamadı",
          flags: [],
          detail: "",
          measured: null,
        },
        {
          capability: "json",
          label: "JSON çıktısı",
          required: false,
          note: "",
          state: "missing",
          stateLabel: "okunamadı",
          flags: ["--json"],
          detail: "",
          measured: null,
        },
        {
          capability: "stream",
          label: "Akış",
          required: true,
          note: "",
          state: "declared",
          stateLabel: "bildirildi",
          flags: ["--stream"],
          detail: "",
          measured: null,
        },
      ],
    });
    // Bayrak listesi boşken "{flags} yok" cümlesi çift boşluklu ve adsız
    // kalıyordu; her iki vaka ayrı ayrı okunur.
    expect(warnings).toEqual([
      "Toplu yazdırma: kurulu sürümün yardım metninde bildirilmedi.",
      "JSON çıktısı: kurulu sürümün yardım metninde --json yok.",
    ]);
    expect(warnings.join(" ")).not.toContain("  ");
    expect(warnings.join(" ")).not.toContain("metninde  yok");
  });
});

describe("promptShapeText", () => {
  it("açık istem kapısında argv biçimini yazar", () => {
    const line = promptShapeText({
      prompt: {
        argv: ["--print=<istem>"],
        printable: true,
        reason: null,
        toolFree: "verified",
        evidence: null,
        evidenceVersion: null,
        evidenceStale: false,
      },
    });
    expect(line.tone).toBe("ok");
    expect(line.text).toContain("--print=<istem>");
    expect(line.text).toContain("istem gönderimi açık");
  });

  it("araçsız kip doğrulanmadıysa kapalıdır ve neden yazılır", () => {
    const line = promptShapeText({
      prompt: {
        argv: ["--print=<istem>"],
        printable: false,
        reason: "tool_free_unverified",
        toolFree: "no_path",
        evidence: null,
        evidenceVersion: null,
        evidenceStale: false,
      },
    });
    expect(line.tone).toBe("warn");
    expect(line.text).toContain("araçsız kip doğrulanamadı");
    expect(line.text).not.toContain("açık");
  });

  it("toplu yürütme kapalıysa nedeni ayrı yazar", () => {
    const line = promptShapeText({
      prompt: {
        argv: [],
        printable: false,
        reason: "batch_forbidden",
        toolFree: "no_path",
        evidence: null,
        evidenceVersion: null,
        evidenceStale: false,
      },
    });
    expect(line.text).toContain("otomatik yürütme kapalı");
    expect(line.text).toContain("—");
  });

  // Yolu olmayan adaptör "tanımsız" demez: neden bilinir ve yazılır. Bu satır
  // eskiden `tool_free_unverified`a düşüyordu (yanlış neden).
  it("istem yolu yoksa kendi nedenini yazar", () => {
    const line = promptShapeText({
      prompt: {
        argv: [],
        printable: false,
        reason: "no_prompt_path",
        toolFree: "verified",
        evidence: null,
        evidenceVersion: null,
        evidenceStale: false,
      },
    });
    expect(line.tone).toBe("warn");
    expect(line.text).toContain("istem yolu tanımlı değil");
    expect(line.text).not.toContain("tanımsız");
  });
});

function hardeningProbe(overrides: Partial<CliHardeningProbe> = {}): CliHardeningProbe {
  return {
    adapterId: "claude-code",
    displayName: "Claude Code",
    executable: "/usr/local/bin/claude",
    argvTemplate: ["--tools", "", "--print", "<istem>"],
    timeoutMs: 180000,
    status: "exited",
    exitCode: 0,
    durationMs: 4200,
    toolAccess: "blocked",
    answered: true,
    canaryInReply: false,
    declineInReply: true,
    replyHead: "ARACSIZ",
    replyTruncated: false,
    stderrHead: "",
    homeIsolation: "user_home",
    canaryBytesAfter: 88,
    canaryToken: "DIM-CANARY-50",
    canaryFile: "KANIT.txt",
    sandboxFiles: 2,
    sandboxBytes: 4096,
    probedAtUnix: 1770000000,
    evidenceRecorded: true,
    notRun: null,
    prompt: {
      argv: ["--tools", "", "--print", "<istem>"],
      printable: true,
      reason: null,
      toolFree: "verified",
      evidence: "blocked",
      evidenceVersion: null,
      evidenceStale: false,
    },
    notes: [],
    ...overrides,
  };
}

describe("hardeningProbeText", () => {
  it("dosya okunduysa sertleştirmenin yetmediğini kırmızı yazar", () => {
    const result = hardeningProbeText(hardeningProbe({ toolAccess: "granted", canaryInReply: true }));
    expect(result.tone).toBe("danger");
    expect(result.text).toContain("KANIT.txt");
    expect(result.text).toContain("DIM-CANARY-50");
    expect(result.text).toContain("engellemiyor");
  });

  it("ret yanıtı yeşil değil ama 'ok' tonunda yazılır ve hükmü sınırlar", () => {
    const result = hardeningProbeText(hardeningProbe());
    expect(result.tone).toBe("ok");
    expect(result.text).toContain("araç çağrısı görülmedi");
    // "Araçsız kip doğrulandı" demez: gözlem budur.
    expect(result.text).not.toContain("doğrulandı");
  });

  it("sonuç vermeyen ölçüm hüküm sayılmaz", () => {
    const result = hardeningProbeText(hardeningProbe({ toolAccess: "inconclusive" }));
    expect(result.tone).toBe("warn");
    expect(result.text).toContain("bilinmiyor");
  });

  it("yanıt gelmediyse durum yazılmaz", () => {
    const result = hardeningProbeText(
      hardeningProbe({ answered: false, toolAccess: "inconclusive", status: "timed_out" }),
    );
    expect(result.tone).toBe("warn");
    expect(result.text).toContain("timed_out");
    expect(result.text).not.toContain("KANIT.txt");
  });

  // Ölçüldü (agy 1.5.6, 2026-11-12): kota doluyken süreç 3 ile çıkıyor ve
  // yanıt boş kalıyordu; cümle "hüküm yok" değil **koşulamadı** demeli.
  it("sınama koşulamadıysa nedenini yazar, hüküm yazmaz", () => {
    const result = hardeningProbeText(
      hardeningProbe({
        answered: false,
        toolAccess: "inconclusive",
        status: "exited",
        exitCode: 3,
        evidenceRecorded: false,
        notRun: {
          kind: "quota",
          reason: "Individual quota reached. Resets in 101h.",
        },
      }),
    );
    expect(result.tone).toBe("warn");
    expect(result.text).toContain("koşulamadı");
    expect(result.text).toContain("kota");
    expect(result.text).toContain("Ölçüm yazılmadı");
    expect(result.text).not.toContain("hüküm yok");
    expect(result.text).not.toContain("KANIT.txt");
  });

  it("koşulamama sınıfı kapalı kümede etiketlenir, bilinmeyen ham yazılır", () => {
    expect(notRunLabel("quota")).toBe("sağlayıcı kotası/limiti doldu");
    expect(notRunLabel("auth")).toBe("oturum/kimlik yok");
    expect(notRunLabel("process_failed")).toBe("süreç başarısız çıktı");
    expect(notRunLabel("timeout")).toBe("süre aşıldı");
    expect(notRunLabel("cancelled")).toBe("iptal edildi");
    // İleri sürüm yeni bir sınıf gönderirse uydurma etiket yok.
    expect(notRunLabel("billing_hold")).toBe("billing_hold");
  });
});

describe("adapterAliasName — eş ad çözümü (F10-T08c)", () => {
  it("bildirilen adla bulunduysa eş ad yazılmaz", () => {
    expect(adapterAliasName({ command: "claude", resolvedCommand: "claude" })).toBeNull();
  });

  it("eş adla bulunduysa bulunan ad döner", () => {
    expect(adapterAliasName({ command: "antigravity", resolvedCommand: "agy" })).toBe("agy");
  });

  it("kurulu değilse (resolvedCommand yok) eş ad uydurulmaz", () => {
    expect(adapterAliasName({ command: "antigravity", resolvedCommand: null })).toBeNull();
  });
});

describe("ölçülen araç erişimi kapıya girer (F10-T08d)", () => {
  it("ölçüm araç erişimini gösterdiyse istem kapısı kırmızı yazılır", () => {
    const line = promptShapeText({
      prompt: {
        argv: ["--print", "<istem>"],
        printable: false,
        reason: "tool_access_granted",
        toolFree: "verified",
        evidence: "granted",
        evidenceVersion: null,
        evidenceStale: false,
      },
    });
    expect(line.tone).toBe("danger");
    expect(line.text).toContain("ölçüm araç erişimini gösterdi");
    expect(line.text).not.toContain("istem gönderimi açık");
  });

  it("okunamayan kayıt nedeni yazar ve kapı kapalı kalır", () => {
    const line = promptShapeText({
      prompt: {
        argv: ["--print=<istem>"],
        printable: false,
        reason: "evidence_unreadable",
        toolFree: "verified",
        evidence: "unreadable",
        evidenceVersion: null,
        evidenceStale: false,
      },
    });
    expect(line.tone).toBe("warn");
    expect(line.text).toContain("sınama kaydı okunamadı");
  });

  it("ölçüm satırı kayıt yoksa yazılmaz, varsa tarihiyle yazılır", () => {
    expect(
      measuredEvidenceLine({
        evidence: null,
        evidenceAtUnix: null,
        evidenceVersion: null,
        evidenceStale: false,
        version: "2.1.303",
      }),
    ).toBeNull();
    const granted = measuredEvidenceLine({
      evidence: "granted",
      evidenceAtUnix: 1770000000,
      evidenceVersion: "2.1.303",
      evidenceStale: false,
      version: "2.1.303",
    });
    expect(granted?.tone).toBe("danger");
    expect(granted?.text).toContain("araç erişimi var");
    const blocked = measuredEvidenceLine({
      evidence: "blocked",
      evidenceAtUnix: null,
      evidenceVersion: "0.159.2",
      evidenceStale: false,
      version: "0.159.2",
    });
    expect(blocked?.tone).toBe("plain");
    expect(blocked?.text).toContain("araç çağrısı görülmedi");
    expect(
      measuredEvidenceLine({
        evidence: "unreadable",
        evidenceAtUnix: null,
        evidenceVersion: null,
        evidenceStale: true,
        version: "2.1.303",
      })?.tone,
    ).toBe("warn");
  });

  it("ölçüm eski sürüme aitse satır hangi sürümde ölçüldüğünü söyler", () => {
    // F10-T08g: eski ikilinin davranışı kurulu sürüme mal edilmez.
    const stale = measuredEvidenceLine({
      evidence: "blocked",
      evidenceAtUnix: 1770000000,
      evidenceVersion: "0.159.2",
      evidenceStale: true,
      version: "0.170.0",
    });
    expect(stale?.tone).toBe("warn");
    expect(stale?.text).toContain("0.159.2");
    expect(stale?.text).toContain("0.170.0");
    expect(stale?.text).toContain("ölçülmedi");

    const unknownVersion = measuredEvidenceLine({
      evidence: "granted",
      evidenceAtUnix: null,
      evidenceVersion: null,
      evidenceStale: true,
      version: "0.170.0",
    });
    expect(unknownVersion?.tone).toBe("danger");
    expect(unknownVersion?.text).toContain("sürüm yazılmamış");
  });

  it("eski sürümde alınan ölçüm 'bu sürümde araç erişimi var' demez", () => {
    const line = promptShapeText({
      prompt: {
        argv: ["exec", "<istem>"],
        printable: false,
        reason: "tool_access_granted",
        toolFree: "verified",
        evidence: "granted",
        evidenceVersion: "0.159.2",
        evidenceStale: true,
      },
    });
    expect(line.tone).toBe("danger");
    expect(line.text).toContain("eski bir sürümde ölçüldü");
    expect(line.text).not.toContain("sertleştirme dosya okumayı engellemiyor");
  });

  it("bilinmeyen ölçüm kimliği uydurma etikete düşmez", () => {
    expect(measuredAccessLabel("weird")).toBe("weird");
    expect(measuredAccessLabel("granted")).toContain("araç erişimi var");
  });

  it("sınama sonucunun kapıya etkisi yalnız kayıt yazıldıysa yazılır", () => {
    expect(evidenceGateText(hardeningProbe({ evidenceRecorded: false }))).toBeNull();
    const base = hardeningProbe();
    const closed = evidenceGateText({
      ...base,
      prompt: { ...base.prompt, printable: false, reason: "tool_access_granted", evidence: "granted" },
    });
    expect(closed).toContain("İstem gönderimi kapatıldı");
    expect(evidenceGateText(base)).toContain("açık kalıyor");
    // Kayıt yazıldı ama kapı **başka** nedenle kapalı: kapı cümlesi uydurulmaz.
    const other = evidenceGateText({
      ...base,
      prompt: { ...base.prompt, printable: false, reason: "tool_free_unverified", evidence: "blocked" },
    });
    expect(other).toBeNull();
  });
});
