// Yedek saklama (retention) politikasının **cihaz/UI** tercihi.
//
// Politika kasa politikası değildir: hangi yedeklerin ne kadar tutulacağı
// kullanıcının bu cihazdaki tercihidir ve çekirdeğe **açık parametre** olarak
// geçer. Çekirdek aynı sınırları (günlük 1..365, haftalık 0..52) yeniden
// doğrular; buradaki doğrulama yalnız arayüzün bozuk değeri göndermesini
// engeller.

export type BackupRetentionPolicy = { daily: number; weekly: number };

/** Şartnamenin önerisi: son 7 günlük + son 4 haftalık yedek. */
export const DEFAULT_RETENTION_POLICY: BackupRetentionPolicy = { daily: 7, weekly: 4 };
export const MIN_RETENTION_DAILY = 1;
export const MAX_RETENTION_DAILY = 365;
export const MIN_RETENTION_WEEKLY = 0;
export const MAX_RETENTION_WEEKLY = 52;

const STORAGE_KEY = "dim.backup.retention";

function validInteger(value: unknown, min: number, max: number): number | null {
  if (typeof value !== "number" || !Number.isInteger(value)) return null;
  if (value < min || value > max) return null;
  return value;
}

/**
 * Saklanan/verilen değeri doğrular. Bozuk değer `null`dur — "varsayılana
 * düşür" **çağıranın** kararıdır; sessizce tamir edilmiş bir politika
 * kullanıcının gördüğünden farklı yedek silmemelidir.
 */
export function parseRetentionPolicy(raw: unknown): BackupRetentionPolicy | null {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) return null;
  const value = raw as { daily?: unknown; weekly?: unknown };
  const daily = validInteger(value.daily, MIN_RETENTION_DAILY, MAX_RETENTION_DAILY);
  const weekly = validInteger(value.weekly, MIN_RETENTION_WEEKLY, MAX_RETENTION_WEEKLY);
  if (daily === null || weekly === null) return null;
  return { daily, weekly };
}

/** Cihaz tercihini okur; yok/bozuksa **varsayılanı** döndürür (gizlemez). */
export function readRetentionPolicy(): BackupRetentionPolicy {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === null) return { ...DEFAULT_RETENTION_POLICY };
    const parsed = parseRetentionPolicy(JSON.parse(raw));
    return parsed ?? { ...DEFAULT_RETENTION_POLICY };
  } catch {
    return { ...DEFAULT_RETENTION_POLICY };
  }
}

/** Cihaz tercihini yazar; yazılan (doğrulanmış) değeri döndürür. */
export function writeRetentionPolicy(policy: BackupRetentionPolicy): BackupRetentionPolicy {
  const normalized = parseRetentionPolicy(policy);
  if (!normalized) {
    throw new Error("E_BACKUP_RETENTION");
  }
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(normalized));
  } catch {
    // Depolama kapalıysa tercih kalıcı olmaz ama **tarama** çalışır; çağıran
    // hatayı kullanıcıya göstermez (bu bir yedekleme hatası değildir).
  }
  return normalized;
}
