// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import {
  BACKUP_ATTEMPT_KEY,
  DEFAULT_BACKUP_DAYS,
  clearBackupAttempt,
  isBackupDue,
  nextBackupAt,
  parseBackupAttempt,
  readBackupAttempt,
  readBackupStatus,
  writeBackupAttempt,
} from "./backup-schedule";

const DAY = 24 * 60 * 60 * 1000;

beforeEach(() => {
  localStorage.clear();
});

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

describe("nextBackupAt", () => {
  it("son başarılı yedekten aralık kadar sonrasını verir", () => {
    const last = "2026-11-01T00:00:00Z";
    expect(nextBackupAt(last, 3)).toBe(Date.parse(last) + 3 * DAY);
  });

  it("tarih yoksa plan yok", () => {
    expect(nextBackupAt(null)).toBeNull();
    expect(nextBackupAt("bozuk")).toBeNull();
  });

  it("geçersiz aralıkta plan yok", () => {
    expect(nextBackupAt("2026-11-01T00:00:00Z", 0)).toBeNull();
    expect(nextBackupAt("2026-11-01T00:00:00Z", Number.NaN)).toBeNull();
  });
});

describe("parseBackupAttempt", () => {
  it("geçerli kaydı okur", () => {
    const at = "2026-11-07T10:00:00.000Z";
    expect(parseBackupAttempt(JSON.stringify({ at, ok: true, code: null }))).toEqual({
      at,
      ok: true,
      code: null,
    });
  });

  it("başarısız kayıtta kodu korur, kırpar", () => {
    const at = "2026-11-07T10:00:00.000Z";
    expect(parseBackupAttempt(JSON.stringify({ at, ok: false, code: "  E_IO  " }))).toEqual({
      at,
      ok: false,
      code: "E_IO",
    });
  });

  it("başarılı kayda yazılmış kodu yok sayar (uydurma neden gösterme)", () => {
    const at = "2026-11-07T10:00:00.000Z";
    expect(parseBackupAttempt(JSON.stringify({ at, ok: true, code: "E_IO" }))).toEqual({
      at,
      ok: true,
      code: null,
    });
  });

  it("bozuk kayıtları reddeder", () => {
    expect(parseBackupAttempt(null)).toBeNull();
    expect(parseBackupAttempt("")).toBeNull();
    expect(parseBackupAttempt("bozuk")).toBeNull();
    expect(parseBackupAttempt("null")).toBeNull();
    expect(parseBackupAttempt(JSON.stringify({ ok: true }))).toBeNull();
    expect(parseBackupAttempt(JSON.stringify({ at: "bozuk", ok: true }))).toBeNull();
    expect(parseBackupAttempt(JSON.stringify({ at: "2026-11-07T10:00:00Z", ok: "evet" }))).toBeNull();
  });
});

describe("deneme kaydı (localStorage)", () => {
  it("yazar ve geri okur", () => {
    const at = "2026-11-07T10:00:00.000Z";
    writeBackupAttempt({ at, ok: false, code: "E_DISK_FULL" });
    expect(readBackupAttempt()).toEqual({ at, ok: false, code: "E_DISK_FULL" });
    expect(readBackupStatus().attempt).toEqual({ at, ok: false, code: "E_DISK_FULL" });
  });

  it("bozuk kayıt 'deneme yok' sayılır", () => {
    localStorage.setItem(BACKUP_ATTEMPT_KEY, "bozuk");
    expect(readBackupAttempt()).toBeNull();
  });

  it("temizler", () => {
    writeBackupAttempt({ at: "2026-11-07T10:00:00.000Z", ok: true, code: null });
    clearBackupAttempt();
    expect(localStorage.getItem(BACKUP_ATTEMPT_KEY)).toBeNull();
    expect(readBackupAttempt()).toBeNull();
  });
});
