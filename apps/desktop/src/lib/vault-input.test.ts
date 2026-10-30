import { describe, expect, it } from "vitest";
import { t } from "./i18n";
import { MIN_PASSWORD_LENGTH, checkPassword, checkPasswordConfirmation, normalizeRecoveryInput } from "./vault-input";

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
    expect(r).toEqual({ ok: false, reason: t("vaultInput.mismatch") });
  });

  it("gerekçeler sözlükten gelir; parametreli metin yarım kalmaz", () => {
    const short = checkPassword("kisa");
    expect(short.ok).toBe(false);
    if (!short.ok) {
      expect(short.reason).toBe(t("vaultInput.tooShort", { min: MIN_PASSWORD_LENGTH }));
      expect(short.reason).not.toContain("{");
      expect(short.reason).toContain(String(MIN_PASSWORD_LENGTH));
    }
    const common = checkPassword("avukat123456");
    if (!common.ok) expect(common.reason).toBe(t("vaultInput.common"));
    const repeated = checkPassword("aaaaaaaaaaaaaa");
    if (!repeated.ok) expect(repeated.reason).toBe(t("vaultInput.repeated"));
    const digits = checkPassword("1234567890123");
    if (!digits.ok) expect(digits.reason).toBe(t("vaultInput.digitsOnly"));
  });
});

describe("kurtarma anahtarı normalizasyonu", () => {
  it("tire ve boşlukları temizler, büyük harfe çevirir", () => {
    expect(normalizeRecoveryInput("abcd-efgh ijkl")).toBe("ABCDEFGHIJKL");
  });
});
