import { describe, expect, it } from "vitest";
import { messageKeys, t, tr } from "./i18n";

describe("sözlük", () => {
  it("her yaprak anahtar boş olmayan metin verir", () => {
    const keys = messageKeys();
    expect(keys.length).toBeGreaterThan(60);
    const broken: string[] = [];
    for (const key of keys) {
      const raw = (() => {
        try {
          return t(key);
        } catch {
          return null; // parametre isteyen anahtar aşağıda ayrıca sınanır
        }
      })();
      if (raw === null) continue;
      if (raw.trim().length === 0) broken.push(`${key}: boş`);
      // Türkçe metinde çift boşluk ya da baş/son boşluk olmaz.
      if (raw !== raw.trim() || raw.includes("  ")) broken.push(`${key}: boşluk`);
    }
    expect(broken).toEqual([]);
  });

  it("parametre isteyen anahtar parametresiz çağrılırsa hata atar", () => {
    expect(() => t("shell.version")).toThrow(/parametre/);
    expect(() => t("shell.comingLead")).toThrow(/parametre/);
  });

  it("parametreleri doldurur", () => {
    expect(t("shell.version", { version: "0.1.0" })).toBe("v0.1.0");
    expect(t("shell.comingLead", { phase: "F09" })).toContain("F09");
    expect(t("shell.comingLead", { phase: "F09" })).not.toContain("{");
  });

  it("eksik parametrede yarım cümle göstermez, hata atar", () => {
    expect(() => t("shell.version", { sürüm: "0.1.0" })).toThrow(/eksik parametre/);
  });

  it("bilinmeyen anahtar sessizce boş dönmez", () => {
    // Tip sistemi bunu derlemede engeller; çalışma zamanı kapısı da var.
    expect(() => t("yok.boyle.bir.anahtar" as never)).toThrow(/bilinmeyen anahtar/);
  });

  it("etiket haritaları sözlükten türetilir (kopya metin yok)", () => {
    expect(tr.labels.caseKind.review).toBe("İnceleme dosyası");
    expect(tr.labels.sensitivity.client_confidential).toBe("Müvekkil sırrı");
    expect(tr.labels.aiPolicy.local_only).toBe("Yalnız yerel model");
  });
});
