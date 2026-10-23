import { describe, expect, it } from "vitest";

import { localIsoDay, REMINDER_TEXT, reminderLabel } from "./reminders";

describe("localIsoDay", () => {
  it("yerel günü iki haneli biçimde verir", () => {
    expect(localIsoDay(new Date(2026, 9, 3))).toBe("2026-10-03");
    expect(localIsoDay(new Date(2026, 11, 31))).toBe("2026-12-31");
  });

  it("ay/gün sınırında UTC kayması yapmaz", () => {
    // 00:30 yerel saat: UTC'ye çevrilse gün değişebilirdi.
    expect(localIsoDay(new Date(2026, 9, 23, 0, 30))).toBe("2026-10-23");
    expect(localIsoDay(new Date(2026, 9, 23, 23, 30))).toBe("2026-10-23");
  });
});

describe("reminder metni", () => {
  it("dosya veya görev bilgisi taşımaz", () => {
    expect(REMINDER_TEXT).not.toMatch(/\{|\}/);
    expect(REMINDER_TEXT.toLowerCase()).not.toContain("müvekkil");
  });

  it("politika etiketlerini ayırır", () => {
    expect(reminderLabel("on_day")).toContain("Bugün");
    expect(reminderLabel("day_before")).toContain("Yarın");
    expect(reminderLabel("none")).toContain("kapalı");
  });
});
