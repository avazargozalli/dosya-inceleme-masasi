import { describe, expect, it } from "vitest";
import { DEFAULT_BACKUP_DAYS, isBackupDue } from "./backup-schedule";

const DAY = 24 * 60 * 60 * 1000;

describe("isBackupDue", () => {
  it("hiç yedek yoksa ister", () => {
    expect(isBackupDue(null, Date.now())).toBe(true);
  });

  it("süre dolmadan istemez", () => {
    const now = Date.parse("2026-10-24T12:00:00Z");
    const last = new Date(now - (DEFAULT_BACKUP_DAYS - 1) * DAY).toISOString();
    expect(isBackupDue(last, now)).toBe(false);
  });

  it("süre dolunca ister", () => {
    const now = Date.parse("2026-10-24T12:00:00Z");
    const last = new Date(now - DEFAULT_BACKUP_DAYS * DAY).toISOString();
    expect(isBackupDue(last, now)).toBe(true);
  });

  it("bozuk tarihte güvenli tarafta kalır", () => {
    expect(isBackupDue("bozuk", Date.now())).toBe(true);
  });

  it("saat geri alınmışsa yedek ister", () => {
    const now = Date.parse("2026-10-24T12:00:00Z");
    expect(isBackupDue("2026-11-01T12:00:00Z", now)).toBe(true);
  });

  it("geçersiz aralıkta istemez", () => {
    expect(isBackupDue(null, Date.now(), 0)).toBe(false);
  });
});
