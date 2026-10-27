import type { WorkerBudgetProfile, WorkerBudgetView } from "./api";

/**
 * §17.3 kaynak bütçesi metinleri.
 *
 * Sınırlar **çekirdekte** tanımlıdır (`crates/ingest/src/budget.rs`); bu modül
 * yalnız biçimlendirir. Sayı uydurulmaz: gözlenen değer yoksa "0 MiB" yazar,
 * sınır yoksa "uygulanmaz".
 */

const MIB = 1024 * 1024;

/** Bayt → okunur MiB. 100 MiB üstünde tam sayı, altında tek ondalık. */
export function mibText(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 MiB";
  const mib = bytes / MIB;
  if (mib >= 100) return `${Math.round(mib)} MiB`;
  const rounded = Math.round(mib * 10) / 10;
  return `${String(rounded).replace(".", ",")} MiB`;
}

/** Saniye → "120 sn"; sıfır sınırı "uygulanmaz" (uydurma sınır yazılmaz). */
export function secondsText(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "uygulanmaz";
  return `${Math.round(seconds)} sn`;
}

/** ms → "340 ms" / "1,2 sn". */
export function elapsedText(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "0 ms";
  if (ms < 1000) return `${Math.round(ms)} ms`;
  const seconds = Math.round((ms / 1000) * 10) / 10;
  return `${String(seconds).replace(".", ",")} sn`;
}

/** Profil sınır satırı: tek satırda beş bütçe boyutu. */
export function profileLimits(profile: WorkerBudgetProfile): string {
  return [
    `CPU ${secondsText(profile.cpuSeconds)}`,
    `bellek ${mibText(profile.memoryBytes)}`,
    `çıktı ${mibText(profile.outputBytes)}`,
    `dosya ${profile.openFiles}`,
    `süre ${Math.round(profile.wallClockSeconds)} sn`,
  ].join(" · ");
}

/** İş sonrası gerçek kullanım satırı. */
export function budgetSummary(view: WorkerBudgetView): string {
  return [
    view.label,
    `tepe bellek ${mibText(view.peakMemoryBytes)}`,
    `yanıt ${mibText(view.observedOutputBytes)}`,
    elapsedText(view.elapsedMs),
  ].join(" · ");
}

const DIMENSION_LABELS: Record<string, string> = {
  cpu: "CPU süresi",
  memory: "bellek",
  output: "çıktı boyutu",
  open_files: "açık dosya sayısı",
  time: "süre",
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
      return `${view.openFiles} dosya`;
    case "time":
      return `${Math.round(view.wallClockSeconds)} sn`;
    default:
      return null;
  }
}

/** Bütçe ihlali uyarısı; ihlal yoksa `null`. */
export function budgetWarning(view: WorkerBudgetView): string | null {
  if (!view.violated) return null;
  const limit = dimensionLimit(view, view.violated);
  const label = dimensionLabel(view.violated);
  return limit ? `Bütçe aşıldı: ${label} (sınır ${limit})` : `Bütçe aşıldı: ${label}`;
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
