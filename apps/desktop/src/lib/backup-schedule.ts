// Otomatik yedek zamanlaması: saf karar fonksiyonları + yerel kayıt.
// Saat/test edilebilirlik için `now` dışarıdan verilir; karar fonksiyonları
// yan etkisizdir. Kayıt (localStorage) yalnız ince bir sarmalayıcıdır.

export const DEFAULT_BACKUP_DAYS = 7;
export const BACKUP_CHECK_MS = 60 * 60 * 1000; // saatte bir kontrol
export const BACKUP_DIR_KEY = "dim.backup.dir";
export const BACKUP_LAST_KEY = "dim.backup.last";
export const BACKUP_DAYS_KEY = "dim.backup.days";
export const BACKUP_ATTEMPT_KEY = "dim.backup.attempt";

const DAY_MS = 24 * 60 * 60 * 1000;

export function isBackupDue(
  lastIso: string | null,
  nowMs: number,
  everyDays: number = DEFAULT_BACKUP_DAYS,
): boolean {
  if (!Number.isFinite(nowMs) || everyDays <= 0) return false;
  if (!lastIso) return true;
  const last = Date.parse(lastIso);
  if (Number.isNaN(last)) return true; // bozuk kayıt: yedek iste
  // Saat geri alınmışsa yedek iste; "süresi geçmedi" varsayımı yapma.
  if (nowMs < last) return true;
  return nowMs - last >= everyDays * DAY_MS;
}

/** Son **başarılı** yedekten sonraki plan anı (ms). Bilinmiyorsa `null`. */
export function nextBackupAt(
  lastIso: string | null,
  everyDays: number = DEFAULT_BACKUP_DAYS,
): number | null {
  if (!lastIso || !Number.isFinite(everyDays) || everyDays <= 0) return null;
  const last = Date.parse(lastIso);
  if (Number.isNaN(last)) return null;
  return last + everyDays * DAY_MS;
}

/**
 * Son yedek **denemesi**. Başarısız deneme tazeliği ilerletmez; yalnız
 * "ne zaman, sonuç ne, neden" sorusunu yanıtlar. `code` yalnız başarısız
 * denemede anlamlıdır — başarılı kayda kod yazılmışsa yok sayılır
 * (uydurma neden gösterme).
 */
export type BackupAttempt = {
  at: string;
  ok: boolean;
  code: string | null;
};

/** Ham JSON'u doğrular. Bozuk/eksik kayıt `null` — "deneme yok" ile karışmaz. */
export function parseBackupAttempt(raw: string | null): BackupAttempt | null {
  if (!raw) return null;
  try {
    const value: unknown = JSON.parse(raw);
    if (typeof value !== "object" || value === null) return null;
    const v = value as { at?: unknown; ok?: unknown; code?: unknown };
    if (typeof v.at !== "string" || Number.isNaN(Date.parse(v.at))) return null;
    if (typeof v.ok !== "boolean") return null;
    const code = typeof v.code === "string" && v.code.trim() !== "" ? v.code.trim() : null;
    return { at: v.at, ok: v.ok, code: v.ok ? null : code };
  } catch {
    return null;
  }
}

function storage(): Storage | null {
  try {
    return localStorage;
  } catch {
    return null;
  }
}

export function readBackupSettings(): { dir: string; days: number; last: string | null } {
  try {
    const dir = localStorage.getItem(BACKUP_DIR_KEY) ?? "";
    const days = Number(localStorage.getItem(BACKUP_DAYS_KEY) ?? DEFAULT_BACKUP_DAYS);
    const last = localStorage.getItem(BACKUP_LAST_KEY);
    return {
      dir,
      days: Number.isFinite(days) && days > 0 ? days : DEFAULT_BACKUP_DAYS,
      last,
    };
  } catch {
    return { dir: "", days: DEFAULT_BACKUP_DAYS, last: null };
  }
}

export function readBackupAttempt(): BackupAttempt | null {
  try {
    return parseBackupAttempt(localStorage.getItem(BACKUP_ATTEMPT_KEY));
  } catch {
    return null;
  }
}

/** Ayar + son deneme birlikte. Ekran "son yedek" ile "son deneme"yi ayrı yazar. */
export function readBackupStatus(): {
  dir: string;
  days: number;
  last: string | null;
  attempt: BackupAttempt | null;
} {
  const settings = readBackupSettings();
  return { ...settings, attempt: readBackupAttempt() };
}

/** Deneme kaydı yazılamazsa yedek akışı durmaz (kayıt ikincildir). */
export function writeBackupAttempt(attempt: BackupAttempt): void {
  const store = storage();
  if (!store) return;
  try {
    store.setItem(BACKUP_ATTEMPT_KEY, JSON.stringify(attempt));
  } catch {
    // sessiz: kota/izin hatası yedek denemesini engellemez
  }
}

export function clearBackupAttempt(): void {
  const store = storage();
  if (!store) return;
  try {
    store.removeItem(BACKUP_ATTEMPT_KEY);
  } catch {
    // sessiz
  }
}
