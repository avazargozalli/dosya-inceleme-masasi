// @vitest-environment jsdom
//
// Otomatik yedek planının **kasa ayarı** (`settings`, kapsam `vault`) üzerinden
// okuma/yazma/göç sözleşmesi. Saf karar fonksiyonları `backup-schedule.test.ts`te;
// burada G/Ç ve "hangi kaynaktan geldi" ölçülür.
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  BACKUP_DAYS_KEY,
  BACKUP_DIR_KEY,
  DEFAULT_BACKUP_DAYS,
  backupScheduleValue,
  parseBackupSchedule,
  readLegacyBackupSchedule,
} from "./backup-schedule";

const mocks = vi.hoisted(() => ({
  vaultSettingGet: vi.fn(),
  vaultSettingSet: vi.fn(),
  vaultSettingDelete: vi.fn(),
}));

vi.mock("./api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

const {
  E_BACKUP_SETTING_VERIFY,
  clearBackupSchedule,
  loadBackupSchedule,
  saveBackupSchedule,
} = await import("./backup-config");

function row(value: unknown, version = 1) {
  return {
    scope: "vault",
    key: "backup.schedule",
    value,
    version,
    updatedAt: "2026-11-07T00:00:00.000Z",
  };
}

beforeEach(() => {
  vi.resetAllMocks();
  localStorage.clear();
});

describe("parseBackupSchedule", () => {
  it("geçerli planı kırparak okur", () => {
    expect(parseBackupSchedule({ destination: "  /Volumes/Yedek ", days: 7 })).toEqual({
      dir: "/Volumes/Yedek",
      days: 7,
    });
  });

  it("boş hedefi, bozuk aralığı ve nesne olmayanı reddeder", () => {
    for (const value of [
      null,
      undefined,
      "metin",
      [],
      {},
      { destination: "", days: 7 },
      { destination: "   ", days: 7 },
      { destination: "/x" },
      { destination: "/x", days: "7" },
      { destination: "/x", days: 7.5 },
      { destination: "/x", days: 0 },
      { destination: "/x", days: 366 },
      { destination: "/x", days: -1 },
    ]) {
      expect(parseBackupSchedule(value)).toBeNull();
    }
  });

  it("yazılacak değeri kırpılmış hedefle üretir", () => {
    expect(backupScheduleValue({ dir: " /Volumes/Yedek ", days: 3 })).toEqual({
      destination: "/Volumes/Yedek",
      days: 3,
    });
  });
});

describe("loadBackupSchedule", () => {
  it("planı kasadan okur; cihaz kaydına dokunmaz", async () => {
    mocks.vaultSettingGet.mockResolvedValue(row({ destination: "/Volumes/Kasa", days: 5 }));
    const load = await loadBackupSchedule();
    expect(load).toEqual({
      schedule: { dir: "/Volumes/Kasa", days: 5 },
      source: "vault",
      error: null,
    });
    expect(mocks.vaultSettingSet).not.toHaveBeenCalled();
  });

  it("bozuk kasa ayarı 'plan yok' sayılmaz ve cihaz kaydıyla ezilmez", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/Volumes/Eski");
    mocks.vaultSettingGet.mockResolvedValue(row({ destination: "", days: 0 }));
    const load = await loadBackupSchedule();
    expect(load.schedule).toBeNull();
    expect(load.source).toBe("invalid");
    expect(load.error).toBe("E_DB_SETTING");
    expect(mocks.vaultSettingSet).not.toHaveBeenCalled();
    expect(localStorage.getItem(BACKUP_DIR_KEY)).toBe("/Volumes/Eski");
  });

  it("kasa ayarı yoksa cihaz kaydındaki plan bir kez kasaya taşınır", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, " /Volumes/Eski ");
    localStorage.setItem(BACKUP_DAYS_KEY, "5");
    mocks.vaultSettingGet.mockResolvedValue(null);
    mocks.vaultSettingSet.mockImplementation((key: string, value: unknown) =>
      Promise.resolve({ ...row(value, 2), key }),
    );

    const load = await loadBackupSchedule();
    expect(mocks.vaultSettingSet).toHaveBeenCalledWith("backup.schedule", {
      destination: "/Volumes/Eski",
      days: 5,
    });
    expect(load).toEqual({
      schedule: { dir: "/Volumes/Eski", days: 5 },
      source: "migrated",
      error: null,
    });
    // Yalnız doğrulanmış göçten sonra eski anahtarlar silinir.
    expect(localStorage.getItem(BACKUP_DIR_KEY)).toBeNull();
    expect(localStorage.getItem(BACKUP_DAYS_KEY)).toBeNull();
  });

  it("göç yazılamazsa plan kullanılmaya devam eder ama kaynak 'legacy' ve kod yazılır", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/Volumes/Eski");
    localStorage.setItem(BACKUP_DAYS_KEY, "3");
    mocks.vaultSettingGet.mockResolvedValue(null);
    mocks.vaultSettingSet.mockRejectedValue({
      code: "E_DB_SETTING",
      message: "yazılamadı",
      retryable: false,
    });

    const load = await loadBackupSchedule();
    expect(load).toEqual({
      schedule: { dir: "/Volumes/Eski", days: 3 },
      source: "legacy",
      error: "E_DB_SETTING",
    });
    // Göç yazılamadı: eski anahtarlar **silinmez** (veri kaybı yok).
    expect(localStorage.getItem(BACKUP_DIR_KEY)).toBe("/Volumes/Eski");
  });

  it("göç yazıldı ama geri okunan değer bozuksa uydurma plan denmez", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/Volumes/Eski");
    mocks.vaultSettingGet.mockResolvedValue(null);
    mocks.vaultSettingSet.mockResolvedValue(row({ destination: null, days: 0 }));

    const load = await loadBackupSchedule();
    expect(load.source).toBe("legacy");
    expect(load.schedule).toEqual({ dir: "/Volumes/Eski", days: DEFAULT_BACKUP_DAYS });
    expect(load.error).toBe(E_BACKUP_SETTING_VERIFY);
    expect(localStorage.getItem(BACKUP_DIR_KEY)).toBe("/Volumes/Eski");
  });

  it("kasa okunamazsa cihaz önbelleği kullanılır ve hata kodu taşınır", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/Volumes/Onbellek");
    localStorage.setItem(BACKUP_DAYS_KEY, "4");
    mocks.vaultSettingGet.mockRejectedValue({
      code: "E_VAULT_LOCKED",
      message: "kasa kilitli",
      retryable: false,
    });

    const load = await loadBackupSchedule();
    expect(load).toEqual({
      schedule: { dir: "/Volumes/Onbellek", days: 4 },
      source: "legacy",
      error: "E_VAULT_LOCKED",
    });
  });

  it("kasa okunamazsa ve önbellek yoksa plan yoktur (uydurulmaz)", async () => {
    mocks.vaultSettingGet.mockRejectedValue({
      code: "E_VAULT_LOCKED",
      message: "kasa kilitli",
      retryable: false,
    });
    const load = await loadBackupSchedule();
    expect(load).toEqual({ schedule: null, source: "none", error: "E_VAULT_LOCKED" });
  });
});

describe("saveBackupSchedule", () => {
  it("yazar, geri okunan değeri döndürür ve önbelleği tazeler", async () => {
    mocks.vaultSettingSet.mockImplementation((key: string, value: unknown) =>
      Promise.resolve({ ...row(value, 3), key }),
    );
    const saved = await saveBackupSchedule({ dir: " /Volumes/Yeni ", days: 7 });
    expect(mocks.vaultSettingSet).toHaveBeenCalledWith("backup.schedule", {
      destination: "/Volumes/Yeni",
      days: 7,
    });
    expect(saved).toEqual({ dir: "/Volumes/Yeni", days: 7 });
    expect(localStorage.getItem(BACKUP_DIR_KEY)).toBe("/Volumes/Yeni");
    expect(localStorage.getItem(BACKUP_DAYS_KEY)).toBe("7");
  });

  it("geri okunan değer farklıysa hata: plan etkin sayılmaz, önbellek yazılmaz", async () => {
    mocks.vaultSettingSet.mockResolvedValue(row({ destination: "/Baska", days: 7 }));
    await expect(saveBackupSchedule({ dir: "/Volumes/Yeni", days: 7 })).rejects.toMatchObject({
      code: E_BACKUP_SETTING_VERIFY,
    });
    expect(localStorage.getItem(BACKUP_DIR_KEY)).toBeNull();
  });

  it("geçersiz plan kasa ayarına hiç gitmez", async () => {
    await expect(saveBackupSchedule({ dir: "   ", days: 7 })).rejects.toMatchObject({
      code: "E_DB_SETTING",
    });
    expect(mocks.vaultSettingSet).not.toHaveBeenCalled();
  });
});

describe("clearBackupSchedule", () => {
  it("kasa ayarını siler ve cihaz önbelleğini temizler", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/Volumes/Eski");
    localStorage.setItem(BACKUP_DAYS_KEY, "7");
    mocks.vaultSettingDelete.mockResolvedValue(true);
    await clearBackupSchedule();
    expect(mocks.vaultSettingDelete).toHaveBeenCalledWith("backup.schedule");
    expect(localStorage.getItem(BACKUP_DIR_KEY)).toBeNull();
    expect(localStorage.getItem(BACKUP_DAYS_KEY)).toBeNull();
  });
});

describe("readLegacyBackupSchedule", () => {
  it("boş hedefte null; bozuk aralıkta varsayılana döner", () => {
    expect(readLegacyBackupSchedule()).toBeNull();
    localStorage.setItem(BACKUP_DIR_KEY, "/Volumes/Eski");
    localStorage.setItem(BACKUP_DAYS_KEY, "abc");
    expect(readLegacyBackupSchedule()).toEqual({ dir: "/Volumes/Eski", days: DEFAULT_BACKUP_DAYS });
  });
});
