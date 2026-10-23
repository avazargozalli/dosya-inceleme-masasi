import { describe, expect, it } from "vitest";
import { AUTO_LOCK_MINUTES, shouldAutoLock } from "./autolock";

const MINUTE = 60_000;

describe("shouldAutoLock", () => {
  it("süre dolmadan kilitlemez", () => {
    expect(shouldAutoLock(0, (AUTO_LOCK_MINUTES - 1) * MINUTE)).toBe(false);
  });

  it("süre dolunca kilitler", () => {
    expect(shouldAutoLock(0, AUTO_LOCK_MINUTES * MINUTE)).toBe(true);
  });

  it("hareket süreyi sıfırlar", () => {
    expect(shouldAutoLock(9 * MINUTE, 12 * MINUTE)).toBe(false);
    expect(shouldAutoLock(9 * MINUTE, 20 * MINUTE)).toBe(true);
  });

  it("geçersiz veya sıfır sınırda kilitlemez", () => {
    expect(shouldAutoLock(Number.NaN, 1)).toBe(false);
    expect(shouldAutoLock(0, 10 * MINUTE, 0)).toBe(false);
  });

  it("saat geri alınmışsa güvenli tarafta kalır", () => {
    expect(shouldAutoLock(10 * MINUTE, 1 * MINUTE)).toBe(true);
  });
});
