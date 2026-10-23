import { describe, expect, it } from "vitest";
import { checkPassword, checkPasswordConfirmation, normalizeRecoveryInput } from "./vault-input";

describe("parola doğrulama", () => {
  it("kısa parolayı reddeder", () => {
    const r = checkPassword("kisa");
    expect(r.ok).toBe(false);
  });

  it("yaygın parolayı reddeder", () => {
    expect(checkPassword("avukat123456").ok).toBe(false);
  });

  it("sadece rakamı reddeder", () => {
    expect(checkPassword("1234567890123").ok).toBe(false);
  });

  it("tekrar eden karakteri reddeder", () => {
    expect(checkPassword("aaaaaaaaaaaaaa").ok).toBe(false);
  });

  it("güçlü parolayı kabul eder", () => {
    expect(checkPassword("Kira-2026-dosya").ok).toBe(true);
  });

  it("onay uyuşmazlığını bildirir", () => {
    const r = checkPasswordConfirmation("Kira-2026-dosya", "Kira-2026-dosyA");
    expect(r).toEqual({ ok: false, reason: "Parolalar eşleşmiyor." });
  });
});

describe("kurtarma anahtarı normalizasyonu", () => {
  it("tire ve boşlukları temizler, büyük harfe çevirir", () => {
    expect(normalizeRecoveryInput("abcd-efgh ijkl")).toBe("ABCDEFGHIJKL");
  });
});
