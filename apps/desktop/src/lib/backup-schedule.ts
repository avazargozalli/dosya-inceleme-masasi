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

/* ------------------------------------------------- kasa ayarı (settings) --- */

/**
 * Otomatik yedek planının **kasa içi** anahtarı (`settings`, kapsam `vault`).
 * Plan kasa politikasıdır: kasayla taşınır, uygulama verisi silinince kaybolmaz.
 * Cihaz kaydındaki eski anahtarlar yalnız göç kaynağı ve önbellektir.
 */
export const BACKUP_SCHEDULE_SETTING = "backup.schedule";

/** Otomatik yedek planı: hedef klasör + aralık (gün). */
export type BackupSchedule = { dir: string; days: number };

/**
 * Kasa ayarının değerini doğrular: `{ destination, days }`. Bozuk/eksik değer
 * `null`dur — "plan yok" ile karışmaz (çağıran ikisini ayrı yazar).
 */
export function parseBackupSchedule(value: unknown): BackupSchedule | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const v = value as { destination?: unknown; days?: unknown };
  if (typeof v.destination !== "string") return null;
  const dir = v.destination.trim();
  if (!dir) return null;
  if (typeof v.days !== "number" || !Number.isInteger(v.days)) return null;
  // 1..365: sıfır/negatif aralık "her kontrol yedek al" demek olurdu; üst sınır
  // yıllık bir planı kabul etmez (kaza ile yedeksiz kalmayı önler).
  if (v.days < 1 || v.days > 365) return null;
  return { dir, days: v.days };
}

/** Planı kasa ayarına yazılacak JSON değerine çevirir (kırpılmış hedef). */
export function backupScheduleValue(schedule: BackupSchedule): {
  destination: string;
  days: number;
} {
  return { destination: schedule.dir.trim(), days: schedule.days };
}

/**
 * Cihaz kaydındaki **eski** plan (göç kaynağı / okunamazsa önbellek).
 * Hedef boşsa `null`; bozuk aralık varsayılana döner (uydurma değer yazılmaz).
 */
export function readLegacyBackupSchedule(): BackupSchedule | null {
  try {
    const dir = (localStorage.getItem(BACKUP_DIR_KEY) ?? "").trim();
    if (!dir) return null;
    const raw = Number(localStorage.getItem(BACKUP_DAYS_KEY) ?? DEFAULT_BACKUP_DAYS);
    const days = Number.isInteger(raw) && raw >= 1 && raw <= 365 ? raw : DEFAULT_BACKUP_DAYS;
    return { dir, days };
  } catch {
    return null;
  }
}

/** Göç tamamlandıktan sonra eski anahtarlar silinir (bayat değer geri gelmesin). */
export function clearLegacyBackupSchedule(): void {
  const store = storage();
  if (!store) return;
  try {
    store.removeItem(BACKUP_DIR_KEY);
    store.removeItem(BACKUP_DAYS_KEY);
  } catch {
    // sessiz: kota/izin hatası göçü geri almaz
  }
}

/** Son **başarılı** yedek zamanı (cihaz kaydı). Kasa geçmişi `backup_runs`ta. */
export function readBackupLast(): string | null {
  try {
    return localStorage.getItem(BACKUP_LAST_KEY);
  } catch {
    return null;
  }
}

/* ---------------------------------------------- kasa geçmişi (backup_runs) --- */

/**
 * Kasa geçmişindeki (`backup_runs`) **son başarılı** yedeğin zamanı.
 *
 * Tazelik kanıtı kasadır: yedek kaydı yedeğin içinde taşınır. Yalnız
 * `verified`/`succeeded` satırlar kanıttır — `running`/`failed`/tanınmayan durum
 * tazeliği **ilerletmez** (uydurma tarih yok). Tarihi çözülemeyen satır atlanır;
 * hiç kanıt yoksa `null` döner ("yedek yok" ile "bilinmiyor" çağıranda ayrılır).
 */
export function latestBackupSuccess(
  runs: ReadonlyArray<{ status: string; completedAt: string | null; createdAt: string }>,
): string | null {
  let bestMs: number | null = null;
  let bestIso: string | null = null;
  for (const run of runs) {
    if (run.status !== "verified" && run.status !== "succeeded") continue;
    const iso = run.completedAt ?? run.createdAt;
    const ms = Date.parse(iso);
    if (Number.isNaN(ms)) continue;
    if (bestMs === null || ms > bestMs) {
      bestMs = ms;
      bestIso = iso;
    }
  }
  return bestIso;
}

/**
 * "Sıfırla" düğmesinin işareti: kasa kaydı silinemez (kanıttır), bu yüzden
 * düğme bir sonraki kontrolün yedeği **zorunlu** almasını ister. Başarılı
 * yedekten sonra temizlenir; başarısız denemede kalır (söz tutulur).
 */
export const BACKUP_FORCE_KEY = "dim.backup.force";

export function readBackupForce(): boolean {
  try {
    return localStorage.getItem(BACKUP_FORCE_KEY) === "1";
  } catch {
    return false;
  }
}

export function writeBackupForce(): void {
  const store = storage();
  if (!store) return;
  try {
    store.setItem(BACKUP_FORCE_KEY, "1");
  } catch {
    // sessiz: işaret yazılamazsa düğme yalnız cihaz kaydını temizler
  }
}

export function clearBackupForce(): void {
  const store = storage();
  if (!store) return;
  try {
    store.removeItem(BACKUP_FORCE_KEY);
  } catch {
    // sessiz
  }
}

/**
 * Kasa ayarı **doğrulanarak** yazıldıktan sonra cihaz önbelleği tazelenir.
 * Önbellek yalnız kasa okunamadığında kullanılır; kaynak yine kasadır.
 */
export function cacheBackupSchedule(schedule: BackupSchedule): void {
  const store = storage();
  if (!store) return;
  try {
    store.setItem(BACKUP_DIR_KEY, schedule.dir);
    store.setItem(BACKUP_DAYS_KEY, String(schedule.days));
  } catch {
    // sessiz: önbellek yazılamazsa yedek planı kasadan okunur
  }
}
