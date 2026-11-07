// Otomatik yedek planının **kasa içi** (`settings`, kapsam `vault`) okuma/yazma
// yolu. Saf karar `lib/backup-schedule.ts`te; burada yalnız G/Ç ve göç var.
//
// Neden: plan kasa politikasıdır (şartname: kasa ayarı ile cihaz/UI ayarı ayrı
// depolanır). Kasa ayarı yokken cihaz kaydında (localStorage) eski bir plan
// varsa **bir kez** kasaya taşınır; taşıma yazılamazsa yedek durmaz ama ekran
// "kasadan okundu" demez — kaynak `legacy` ve hata kodu ayrı döner.
//
// `invalid`: kasa ayarı **var ama bozuk**. Böyle bir değer cihaz kaydıyla
// ezilmez (bozuk ayar sessizce "ayar yok"a çevrilmez); yedek alınmaz ve neden
// görünür.

import { api, toApiError, type ApiError } from "./api";
import {
  BACKUP_SCHEDULE_SETTING,
  backupScheduleValue,
  cacheBackupSchedule,
  clearLegacyBackupSchedule,
  latestBackupSuccess,
  parseBackupSchedule,
  readBackupLast,
  readLegacyBackupSchedule,
  type BackupSchedule,
} from "./backup-schedule";
import { t } from "./i18n";

/**
 * Plan nereden geldi:
 * - `vault`: kasa ayarından okundu (normal yol)
 * - `migrated`: cihaz kaydından kasaya **taşındı** (tek seferlik göç)
 * - `legacy`: kasa okunamadı/ya da göç yazılamadı; cihaz önbelleği kullanıldı
 * - `invalid`: kasa ayarı var ama bozuk → plan yok, yedek alınmaz
 * - `none`: hiç plan yok
 */
export type BackupScheduleSource = "vault" | "migrated" | "legacy" | "invalid" | "none";

export type BackupScheduleLoad = {
  schedule: BackupSchedule | null;
  source: BackupScheduleSource;
  /** Okuma/taşıma hatasının **kodu**; `null` ise hata yok. */
  error: string | null;
};

/** Kasa ayarı yazıldı ama geri okunan değer beklenen değil (sessiz kabul yok). */
export const E_BACKUP_SETTING_VERIFY = "E_BACKUP_SETTING_VERIFY";

function apiError(code: string, message: string): ApiError {
  return { code, message, retryable: false };
}

/**
 * Planı **kasadan** okur. Kasa ayarı yoksa cihaz kaydındaki eski plan bir kez
 * kasaya taşınır (yazım geri okunarak doğrulanır). Göç yazılamazsa plan yine
 * döner (`source: "legacy"`) — yedek sessizce durmaz, hata görünür.
 */
export async function loadBackupSchedule(): Promise<BackupScheduleLoad> {
  let row: Awaited<ReturnType<typeof api.vaultSettingGet>>;
  try {
    row = await api.vaultSettingGet(BACKUP_SCHEDULE_SETTING);
  } catch (e) {
    // Kasa okunamadı (kilitli/IO). Önbellek varsa yedek durmaz; "kasadan
    // okundu" denmez ve hata kodu çağırana taşınır.
    const cached = readLegacyBackupSchedule();
    return {
      schedule: cached,
      source: cached ? "legacy" : "none",
      error: toApiError(e).code,
    };
  }

  if (row != null) {
    const parsed = parseBackupSchedule(row.value);
    return parsed
      ? { schedule: parsed, source: "vault", error: null }
      : { schedule: null, source: "invalid", error: "E_DB_SETTING" };
  }

  const legacy = readLegacyBackupSchedule();
  if (!legacy) return { schedule: null, source: "none", error: null };

  try {
    const saved = await api.vaultSettingSet(BACKUP_SCHEDULE_SETTING, backupScheduleValue(legacy));
    const confirmed = parseBackupSchedule(saved.value);
    if (!confirmed) {
      return { schedule: legacy, source: "legacy", error: E_BACKUP_SETTING_VERIFY };
    }
    // Yalnız **doğrulanmış** göçten sonra eski anahtarlar silinir.
    clearLegacyBackupSchedule();
    return { schedule: confirmed, source: "migrated", error: null };
  } catch (e) {
    return { schedule: legacy, source: "legacy", error: toApiError(e).code };
  }
}

/**
 * Planı kasaya yazar. Yazım **geri okunarak** doğrulanır; dönen plan diskteki
 * (kasadaki) değerdir. Doğrulanmayan yazım önbelleğe de işlenmez.
 */
export async function saveBackupSchedule(schedule: BackupSchedule): Promise<BackupSchedule> {
  const normalized = parseBackupSchedule(backupScheduleValue(schedule));
  if (!normalized) {
    throw apiError("E_DB_SETTING", t("settings.autoScheduleInvalid"));
  }
  const saved = await api.vaultSettingSet(
    BACKUP_SCHEDULE_SETTING,
    backupScheduleValue(normalized),
  );
  const confirmed = parseBackupSchedule(saved.value);
  if (!confirmed || confirmed.dir !== normalized.dir || confirmed.days !== normalized.days) {
    throw apiError(E_BACKUP_SETTING_VERIFY, t("settings.autoScheduleVerify"));
  }
  cacheBackupSchedule(confirmed);
  return confirmed;
}

/** Planı kasadan siler (otomatik yedek kapanır); cihaz önbelleği de temizlenir. */
export async function clearBackupSchedule(): Promise<void> {
  await api.vaultSettingDelete(BACKUP_SCHEDULE_SETTING);
  clearLegacyBackupSchedule();
}

/* ------------------------------------------------------------ tazelik --- */

/** Kasa geçmişinden tazelik için okunan satır sayısı (en yeni önce). */
export const BACKUP_FRESHNESS_LIMIT = 20;

/**
 * "Son başarılı yedek" bilgisi.
 * - `vault`: kasa geçmişinden okundu (kanıt; yedekle birlikte taşınır)
 * - `device`: yalnız cihaz kaydı (kasa geçmişi okunamadı)
 * - `none`: kanıt yok
 */
export type BackupFreshness = {
  iso: string | null;
  source: "vault" | "device" | "none";
  /** Cihaz kaydındaki zaman — çelişki gizlenmesin diye ayrı taşınır. */
  deviceIso: string | null;
  /** Kasa okuması düştüyse hata **kodu**; `null` ise okuma başarılı. */
  error: string | null;
};

/**
 * Tazeliği **kasadan** okur (`backup_runs`); cihaz kaydı yalnız yedektir.
 *
 * Kasa okunamazsa cihaz kaydına düşer ve hata kodunu taşır: yedek sessizce
 * durmaz (fail-open, veri kaybı yönünde değil) ama ekran "kasadan okundu"
 * demez. Kasa okunup **başarılı satır yoksa** kaynak `none`dur — cihaz kaydı
 * olsa bile tazelik iddiası kurulmaz (o kayıt başka bir kasanın olabilir).
 */
export async function loadBackupFreshness(): Promise<BackupFreshness> {
  const deviceIso = readBackupLast();
  try {
    const runs = await api.backupRunList(BACKUP_FRESHNESS_LIMIT);
    const iso = latestBackupSuccess(runs);
    return { iso, source: iso ? "vault" : "none", deviceIso, error: null };
  } catch (e) {
    return {
      iso: deviceIso,
      source: deviceIso ? "device" : "none",
      deviceIso,
      error: toApiError(e).code,
    };
  }
}
