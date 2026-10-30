import type { WorkerBudgetProfile, WorkerBudgetView } from "./api";
import { formatDecimal } from "./format";
import { t } from "./i18n";

/**
 * §17.3 kaynak bütçesi metinleri.
 *
 * Sınırlar **çekirdekte** tanımlıdır (`crates/ingest/src/budget.rs`); bu modül
 * yalnız biçimlendirir. Sayı uydurulmaz: gözlenen değer yoksa "0 MiB" yazar,
 * sınır yoksa "uygulanmaz".
 *
 * Metinler F04-T08 gereği `lib/i18n.ts`ten, ondalık biçimi `lib/format.ts`ten
 * gelir (tek biçim katmanı; burada `replace(".", ",")` yok).
 */

const MIB = 1024 * 1024;

/** Bayt → okunur MiB. 100 MiB üstünde tam sayı, altında tek ondalık. */
export function mibText(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return t("budget.zeroMib");
  const mib = bytes / MIB;
  if (mib >= 100) return t("budget.mib", { value: Math.round(mib) });
  return t("budget.mib", { value: formatDecimal(mib) });
}

/** Saniye → "120 sn"; sıfır sınırı "uygulanmaz" (uydurma sınır yazılmaz). */
export function secondsText(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return t("budget.notApplicable");
  return t("budget.seconds", { value: Math.round(seconds) });
}

/** ms → "340 ms" / "1,2 sn". */
export function elapsedText(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return t("budget.zeroMs");
  if (ms < 1000) return t("budget.milliseconds", { value: Math.round(ms) });
  return t("budget.seconds", { value: formatDecimal(ms / 1000) });
}

/** Profil sınır satırı: tek satırda beş bütçe boyutu. */
export function profileLimits(profile: WorkerBudgetProfile): string {
  return [
    t("budget.cpuLine", { value: secondsText(profile.cpuSeconds) }),
    t("budget.memoryLine", { value: mibText(profile.memoryBytes) }),
    t("budget.outputLine", { value: mibText(profile.outputBytes) }),
    t("budget.filesLine", { value: profile.openFiles }),
    t("budget.timeLine", { value: Math.round(profile.wallClockSeconds) }),
  ].join(" · ");
}

/** İş sonrası gerçek kullanım satırı. */
export function budgetSummary(view: WorkerBudgetView): string {
  return [
    view.label,
    t("budget.peakMemory", { value: mibText(view.peakMemoryBytes) }),
    t("budget.response", { value: mibText(view.observedOutputBytes) }),
    elapsedText(view.elapsedMs),
  ].join(" · ");
}

const DIMENSION_LABELS: Record<string, string> = {
  cpu: t("budget.dimension.cpu"),
  memory: t("budget.dimension.memory"),
  output: t("budget.dimension.output"),
  open_files: t("budget.dimension.open_files"),
  time: t("budget.dimension.time"),
};

/** Boyut kodunun Türkçe adı; bilinmeyen kod olduğu gibi döner (uydurulmaz). */
export function dimensionLabel(code: string): string {
  return DIMENSION_LABELS[code] ?? code;
}

/** İhlal edilen boyutun sınır metni; bilinmeyen boyutta `null`. */
export function dimensionLimit(view: WorkerBudgetView, code: string): string | null {
  switch (code) {
    case "cpu":
      return secondsText(view.cpuSeconds);
    case "memory":
      return mibText(view.memoryBytes);
    case "output":
      return mibText(view.outputBytes);
    case "open_files":
      return t("budget.openFiles", { count: view.openFiles });
    case "time":
      return t("budget.seconds", { value: Math.round(view.wallClockSeconds) });
    default:
      return null;
  }
}

/** Bütçe ihlali uyarısı; ihlal yoksa `null`. */
export function budgetWarning(view: WorkerBudgetView): string | null {
  if (!view.violated) return null;
  const limit = dimensionLimit(view, view.violated);
  const label = dimensionLabel(view.violated);
  return limit
    ? t("budget.warningLimit", { label, limit })
    : t("budget.warning", { label });
}

/**
 * Tepe belleğin sınıra oranı (0..1). Sınır tanımlı değilse `null` — çubuk
 * çizilmez (sıfıra bölme ya da uydurma dolu çubuk yok).
 */
export function memoryUsageRatio(view: WorkerBudgetView): number | null {
  if (view.memoryBytes <= 0) return null;
  return Math.min(1, view.peakMemoryBytes / view.memoryBytes);
}

/** Yüzde (0..100) — rozet/çubuk için; sınır yoksa `null`. */
export function memoryUsagePercent(view: WorkerBudgetView): number | null {
  const ratio = memoryUsageRatio(view);
  if (ratio === null) return null;
  return Math.round(ratio * 100);
}
