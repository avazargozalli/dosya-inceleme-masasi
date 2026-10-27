import { describe, expect, it } from "vitest";

import type { WorkerBudgetProfile, WorkerBudgetView } from "./api";
import {
  budgetSummary,
  budgetWarning,
  dimensionLabel,
  dimensionLimit,
  elapsedText,
  memoryUsagePercent,
  memoryUsageRatio,
  mibText,
  profileLimits,
  secondsText,
} from "./budget";

function profile(partial: Partial<WorkerBudgetProfile> = {}): WorkerBudgetProfile {
  return {
    kind: "extract",
    label: "Metin çıkarma",
    cpuSeconds: 120,
    memoryBytes: 1024 * 1024 * 1024,
    outputBytes: 32 * 1024 * 1024,
    openFiles: 256,
    wallClockSeconds: 300,
    ...partial,
  };
}

function view(partial: Partial<WorkerBudgetView> = {}): WorkerBudgetView {
  return {
    ...profile(),
    elapsedMs: 721,
    observedOutputBytes: 210_000,
    peakMemoryBytes: 88_000_000,
    violated: null,
    ...partial,
  };
}

describe("budget biçimlendirme", () => {
  it("baytı MiB'e çevirir ve sıfırı uydurmaz", () => {
    expect(mibText(0)).toBe("0 MiB");
    expect(mibText(-5)).toBe("0 MiB");
    expect(mibText(1024 * 1024)).toBe("1 MiB");
    expect(mibText(32 * 1024 * 1024)).toBe("32 MiB");
    // 100 MiB altında tek ondalık, Türkçe virgül.
    expect(mibText(88_000_000)).toBe("83,9 MiB");
    expect(mibText(2 * 1024 * 1024 * 1024)).toBe("2048 MiB");
  });

  it("saniye metni sıfır sınırını 'uygulanmaz' yazar", () => {
    expect(secondsText(0)).toBe("uygulanmaz");
    expect(secondsText(120)).toBe("120 sn");
    expect(secondsText(900.4)).toBe("900 sn");
  });

  it("geçen süreyi ms/sn olarak yazar", () => {
    expect(elapsedText(340)).toBe("340 ms");
    expect(elapsedText(999)).toBe("999 ms");
    expect(elapsedText(1000)).toBe("1 sn");
    expect(elapsedText(1234)).toBe("1,2 sn");
    expect(elapsedText(-1)).toBe("0 ms");
  });

  it("profil satırı beş bütçe boyutunu içerir", () => {
    const line = profileLimits(profile());
    expect(line).toContain("CPU 120 sn");
    expect(line).toContain("bellek 1024 MiB");
    expect(line).toContain("çıktı 32 MiB");
    expect(line).toContain("dosya 256");
    expect(line).toContain("süre 300 sn");
    // AI profilinde CPU sınırı yok.
    expect(profileLimits(profile({ kind: "ai", label: "AI çağrısı", cpuSeconds: 0 }))).toContain(
      "CPU uygulanmaz",
    );
  });

  it("iş sonrası satır gerçek gözlenen değerleri taşır", () => {
    const line = budgetSummary(view());
    expect(line).toContain("Metin çıkarma");
    expect(line).toContain("tepe bellek 83,9 MiB");
    expect(line).toContain("yanıt 0,2 MiB");
    expect(line).toContain("721 ms");
  });

  it("boyut kodu Türkçe ada ve sınıra çevrilir", () => {
    expect(dimensionLabel("memory")).toBe("bellek");
    expect(dimensionLabel("open_files")).toBe("açık dosya sayısı");
    expect(dimensionLabel("bilinmeyen")).toBe("bilinmeyen");
    expect(dimensionLimit(view(), "memory")).toBe("1024 MiB");
    expect(dimensionLimit(view(), "open_files")).toBe("256 dosya");
    expect(dimensionLimit(view({ wallClockSeconds: 900 }), "time")).toBe("900 sn");
    expect(dimensionLimit(view(), "bilinmeyen")).toBeNull();
  });

  it("ihlal yoksa uyarı üretmez, varsa sınırı yazar", () => {
    expect(budgetWarning(view())).toBeNull();
    expect(budgetWarning(view({ violated: "memory" }))).toBe("Bütçe aşıldı: bellek (sınır 1024 MiB)");
    expect(budgetWarning(view({ violated: "output" }))).toBe(
      "Bütçe aşıldı: çıktı boyutu (sınır 32 MiB)",
    );
    // Bilinmeyen boyut: sınır uydurulmaz, kod adı olduğu gibi kalır.
    expect(budgetWarning(view({ violated: "yeni" }))).toBe("Bütçe aşıldı: yeni");
  });

  it("bellek oranı sınır yoksa null, varsa 0..1 arası", () => {
    expect(memoryUsageRatio(view({ memoryBytes: 0 }))).toBeNull();
    expect(memoryUsagePercent(view({ memoryBytes: 0 }))).toBeNull();
    expect(memoryUsagePercent(view())).toBe(8);
    const over = memoryUsageRatio(view({ peakMemoryBytes: 4 * 1024 * 1024 * 1024 }));
    expect(over).toBe(1);
  });
});
