import { describe, expect, it } from "vitest";

import type {
  CliAttackAdapter,
  CliAttackFixture,
  CliAttackVector,
  CliAttackVerdict,
} from "./api";
import {
  attackAdapterWarnings,
  attackCommand,
  attackCounts,
  attackFixtureSummary,
  attackFixtureWarnings,
  attackGuardText,
  attackProductText,
  attackSkipText,
  attackTotalHits,
  attackVerdictText,
  attackVerdictTone,
  attackWorstVerdict,
} from "./attack";

function vector(overrides: Partial<CliAttackVector> = {}): CliAttackVector {
  return {
    vector: "hook",
    label: "Hook betiği",
    note: "Hook, ayar dosyasındaki komutun oturum başında çalıştırılmasıdır.",
    guard: "declared",
    guardLabel: "kapatma yolu bildirildi (denenmedi)",
    guardFlags: ["--bare", "--strict-mcp-config"],
    hit: false,
    source: "none",
    sourceLabel: "işaret yok",
    verdict: "silent_guarded",
    verdictLabel: "sessiz · kapatma yolu bildirildi",
    ...overrides,
  };
}

function adapter(overrides: Partial<CliAttackAdapter> = {}): CliAttackAdapter {
  return {
    adapterId: "claude-code",
    displayName: "Claude Code",
    version: "2.1.303",
    argv: ["--bare", "--help"],
    status: "exited_ok",
    exitCode: 0,
    durationMs: 700,
    hits: 0,
    helpRead: true,
    helpError: null,
    link: "declared",
    linkLabel: "bildirildi · denenmedi",
    gateReasons: [],
    vectors: [vector()],
    skipped: null,
    ...overrides,
  };
}

function fixture(overrides: Partial<CliAttackFixture> = {}): CliAttackFixture {
  return {
    detectorProven: true,
    detectorControlClean: true,
    pollutedHits: 4,
    cleanHits: 0,
    vectorTotal: 4,
    gateRejected: true,
    gateCode: "E_CLI_WORKDIR_NOT_EMPTY",
    productConfig: [
      { vector: "project_instruction", label: "Proje talimatı", hit: false },
      { vector: "hook", label: "Hook betiği", hit: false },
      { vector: "mcp_server", label: "MCP sunucusu", hit: false },
      { vector: "canary", label: "Kapsam dışı canary", hit: true },
    ],
    canaryReadInProductConfig: true,
    ...overrides,
  };
}

describe("attackVerdictTone", () => {
  it("tetiklenen vektörü kırmızı, korumasızı sarı, ölçülemeyeni nötr gösterir", () => {
    expect(attackVerdictTone("triggered")).toBe("danger");
    expect(attackVerdictTone("silent_unguarded")).toBe("warn");
    expect(attackVerdictTone("inconclusive")).toBe("plain");
    // En iyi durum bile yeşil değil: davranış denenmedi.
    expect(attackVerdictTone("silent_guarded")).toBe("warn");
  });
});

describe("attackVerdictText", () => {
  it("tetiklenmede işaretin kaynağını yazar", () => {
    expect(
      attackVerdictText(vector({ verdict: "triggered", hit: true, sourceLabel: "işaretçi dosyası" })),
    ).toBe("tetiklendi · işaretçi dosyası");
  });

  it("sessiz ama korumasız durumu koruma saymaz", () => {
    expect(attackVerdictText(vector({ verdict: "silent_unguarded" }))).toBe(
      "işaret yok · koruma kanıtlanmadı",
    );
  });

  it("kanıtsız dedektörde sonucu ölçülemedi diye işaretler", () => {
    expect(attackVerdictText(vector({ verdict: "inconclusive" }))).toBe(
      "ölçülemedi · dedektör kanıtlanmadı",
    );
  });

  it("en iyi durumda davranışın denenmediğini açıkça yazar", () => {
    expect(attackVerdictText(vector())).toContain("davranış denenmedi");
  });

  it("bilinmeyen kararda sunucu etiketine düşer", () => {
    expect(
      attackVerdictText(
        vector({ verdict: "yeni" as CliAttackVerdict, verdictLabel: "bilinmeyen karar" }),
      ),
    ).toBe("bilinmeyen karar");
  });
});

describe("attackGuardText", () => {
  it("bayrakları boşlukla birleştirir", () => {
    expect(attackGuardText(vector())).toBe("--bare --strict-mcp-config");
  });

  it("bayrak yoksa gerekçeyi yazar", () => {
    expect(
      attackGuardText(vector({ guardFlags: [], guardLabel: "doğrulanmış kapatma yolu yok" })),
    ).toBe("doğrulanmış kapatma yolu yok");
  });
});

describe("attackCommand", () => {
  it("sürümü ve argv'yi birlikte yazar", () => {
    expect(attackCommand(adapter())).toBe("2.1.303 · --bare --help");
  });

  it("koşu yoksa tire gösterir", () => {
    expect(attackCommand(adapter({ version: null, argv: [] }))).toBe("—");
  });
});

describe("attackSkipText", () => {
  it("koşulmadı nedenlerini Türkçe yazar", () => {
    expect(attackSkipText("not_installed")).toContain("kurulu değil");
    expect(attackSkipText("interactive_only")).toContain("etkileşimli-only");
    expect(attackSkipText("executable_missing")).toContain("yürütülebilir bulunamadı");
    expect(attackSkipText("run_failed")).toContain("kanıt sayılmaz");
    expect(attackSkipText("E_CLI_EXEC_SPAWN")).toContain("E_CLI_EXEC_SPAWN");
  });

  it("koşu yapıldıysa null döner", () => {
    expect(attackSkipText(null)).toBeNull();
  });
});

describe("attackFixtureSummary", () => {
  it("dedektörü ve kapıyı tek satırda özetler", () => {
    const text = attackFixtureSummary(fixture());
    expect(text).toContain("Dedektör kanıtlı");
    expect(text).toContain("kirli 4/4");
    expect(text).toContain("temiz 0/4");
    expect(text).toContain("E_CLI_WORKDIR_NOT_EMPTY");
  });

  it("kapı kabul ettiyse bunu gizlemez", () => {
    expect(attackFixtureSummary(fixture({ gateRejected: false, gateCode: null }))).toContain(
      "kabul edildi",
    );
  });
});

describe("attackProductText", () => {
  it("ürün yapılandırmasındaki erişimi yazar", () => {
    expect(attackProductText({ vector: "canary", label: "Kapsam dışı canary", hit: true })).toBe(
      "Kapsam dışı canary: ulaşıldı",
    );
    expect(attackProductText({ vector: "hook", label: "Hook betiği", hit: false })).toBe(
      "Hook betiği: ulaşılamadı",
    );
  });
});

describe("attackFixtureWarnings", () => {
  it("kanıtsız dedektörü, yanlış pozitifi ve kapıyı uyarır", () => {
    const warnings = attackFixtureWarnings(
      fixture({ detectorProven: false, detectorControlClean: false, gateRejected: false }),
    );
    expect(warnings.some((w) => w.includes("Dedektör kanıtlanmadı"))).toBe(true);
    expect(warnings.some((w) => w.includes("yanlış pozitif"))).toBe(true);
    expect(warnings.some((w) => w.includes("kabul edildi"))).toBe(true);
  });

  it("canary sınırını her zaman yazar", () => {
    const warnings = attackFixtureWarnings(fixture());
    expect(warnings).toHaveLength(1);
    expect(warnings[0]).toContain("mutlak yolla okunabildi");
  });

  it("canary dışı bir vektör tetiklenirse ayrıca uyarır", () => {
    const warnings = attackFixtureWarnings(
      fixture({
        productConfig: [
          { vector: "hook", label: "Hook betiği", hit: true },
          { vector: "canary", label: "Kapsam dışı canary", hit: false },
        ],
        canaryReadInProductConfig: false,
      }),
    );
    expect(warnings.some((w) => w.includes("Hook betiği"))).toBe(true);
  });
});

describe("attackAdapterWarnings", () => {
  it("tetiklenen vektörde bağlantıyı kapatmayı söyler", () => {
    const warnings = attackAdapterWarnings(
      adapter({ hits: 1, vectors: [vector({ verdict: "triggered", hit: true })] }),
    );
    expect(warnings[0]).toContain("otomatik inceleme açılmamalı");
  });

  it("kapatma yolu bildirilmeyen vektörü sessizlik diye yumuşatmaz", () => {
    const warnings = attackAdapterWarnings(
      adapter({ vectors: [vector({ verdict: "silent_unguarded", guardFlags: [], guard: "unverified" })] }),
    );
    expect(warnings.some((w) => w.includes("kapatma yolu bildirilmedi"))).toBe(true);
  });

  it("yardım okunamadıysa ve koşu atlandıysa nedenini yazar", () => {
    const warnings = attackAdapterWarnings(
      adapter({ helpRead: false, skipped: "not_installed", vectors: [vector()] }),
    );
    expect(warnings.some((w) => w.includes("Yardım metni okunamadı"))).toBe(true);
    expect(warnings.some((w) => w.includes("kurulu değil"))).toBe(true);
  });

  it("temiz ve bildirilmiş koşuda uyarı üretmez", () => {
    expect(attackAdapterWarnings(adapter())).toHaveLength(0);
  });
});

describe("attackCounts / attackWorstVerdict / attackTotalHits", () => {
  it("kararları sayar", () => {
    const counts = attackCounts([
      adapter({
        vectors: [
          vector({ verdict: "silent_guarded" }),
          vector({ verdict: "silent_unguarded" }),
          vector({ verdict: "triggered", hit: true }),
        ],
      }),
    ]);
    expect(counts.triggered).toBe(1);
    expect(counts.silent_guarded).toBe(1);
    expect(counts.silent_unguarded).toBe(1);
    expect(counts.inconclusive).toBe(0);
  });

  it("en kötü kararı sıralar: tetiklendi > korumasız > ölçülemedi > sessiz", () => {
    expect(
      attackWorstVerdict([
        adapter({
          vectors: [
            vector({ verdict: "silent_guarded" }),
            vector({ verdict: "triggered", hit: true }),
          ],
        }),
      ]).tone,
    ).toBe("danger");
    expect(
      attackWorstVerdict([adapter({ vectors: [vector({ verdict: "silent_unguarded" })] })]).tone,
    ).toBe("warn");
    expect(
      attackWorstVerdict([adapter({ vectors: [vector({ verdict: "inconclusive" })] })]).tone,
    ).toBe("plain");
    expect(attackWorstVerdict([adapter({ vectors: [] })]).label).toBe("koşu yapılmadı");
  });

  it("toplam işareti adaptörlerden toplar", () => {
    expect(attackTotalHits({ adapters: [adapter({ hits: 2 }), adapter({ hits: 1 })] })).toBe(3);
  });
});
