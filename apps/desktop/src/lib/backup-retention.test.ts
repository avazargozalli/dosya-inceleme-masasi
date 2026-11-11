// @vitest-environment jsdom
//
// Saklama politikası tercihi: sınırlar çekirdekle **aynı** olmalı ve bozuk
// değer sessizce "tamir" edilmemeli.
import { beforeEach, describe, expect, it } from "vitest";

import {
  DEFAULT_RETENTION_POLICY,
  MAX_RETENTION_DAILY,
  MAX_RETENTION_WEEKLY,
  parseRetentionPolicy,
  readRetentionPolicy,
  writeRetentionPolicy,
} from "./backup-retention";

const STORAGE_KEY = "dim.backup.retention";

describe("yedek saklama politikası", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("varsayılan şartname önerisidir (7 gün + 4 hafta)", () => {
    expect(DEFAULT_RETENTION_POLICY).toEqual({ daily: 7, weekly: 4 });
    expect(readRetentionPolicy()).toEqual({ daily: 7, weekly: 4 });
  });

  it("sınırları çekirdekle aynı tutar", () => {
    expect(parseRetentionPolicy({ daily: 1, weekly: 0 })).toEqual({ daily: 1, weekly: 0 });
    expect(parseRetentionPolicy({ daily: MAX_RETENTION_DAILY, weekly: MAX_RETENTION_WEEKLY })).toEqual({
      daily: MAX_RETENTION_DAILY,
      weekly: MAX_RETENTION_WEEKLY,
    });
    expect(parseRetentionPolicy({ daily: 0, weekly: 4 })).toBeNull();
    expect(parseRetentionPolicy({ daily: MAX_RETENTION_DAILY + 1, weekly: 4 })).toBeNull();
    expect(parseRetentionPolicy({ daily: 7, weekly: MAX_RETENTION_WEEKLY + 1 })).toBeNull();
    expect(parseRetentionPolicy({ daily: 7.5, weekly: 4 })).toBeNull();
    expect(parseRetentionPolicy({ daily: "7", weekly: 4 })).toBeNull();
    expect(parseRetentionPolicy(null)).toBeNull();
    expect(parseRetentionPolicy([])).toBeNull();
  });

  it("bozuk cihaz kaydı varsayılana döner (gizlenmez)", () => {
    localStorage.setItem(STORAGE_KEY, "{bozuk");
    expect(readRetentionPolicy()).toEqual({ daily: 7, weekly: 4 });
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ daily: 0, weekly: 4 }));
    expect(readRetentionPolicy()).toEqual({ daily: 7, weekly: 4 });
  });

  it("yazılan politika geri okunur", () => {
    expect(writeRetentionPolicy({ daily: 14, weekly: 8 })).toEqual({ daily: 14, weekly: 8 });
    expect(readRetentionPolicy()).toEqual({ daily: 14, weekly: 8 });
  });

  it("geçersiz politika yazılmaz", () => {
    expect(() => writeRetentionPolicy({ daily: 0, weekly: 4 })).toThrow("E_BACKUP_RETENTION");
    expect(localStorage.getItem(STORAGE_KEY)).toBeNull();
  });
});
