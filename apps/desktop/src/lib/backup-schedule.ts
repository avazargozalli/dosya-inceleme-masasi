// Otomatik yedek zamanlaması: saf karar fonksiyonu. Saat/test edilebilirlik
// için `now` dışarıdan verilir; yan etki yok.

export const DEFAULT_BACKUP_DAYS = 7;
export const BACKUP_CHECK_MS = 60 * 60 * 1000; // saatte bir kontrol
export const BACKUP_DIR_KEY = "dim.backup.dir";
export const BACKUP_LAST_KEY = "dim.backup.last";
export const BACKUP_DAYS_KEY = "dim.backup.days";

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
  return nowMs - last >= everyDays * 24 * 60 * 60 * 1000;
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
