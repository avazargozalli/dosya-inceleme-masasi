import type { ParseProgressEvent } from "./api";
import { t } from "./i18n";

/**
 * Ayrıştırma ilerlemesinin arayüz dili (F04-T07).
 *
 * Worker `parse-progress` olayıyla aşama ve sayaç bildirir; buradaki yardımcılar
 * yalnız **gösterim** üretir. Sayaç uydurulmaz: `total` yoksa oran hesaplanmaz,
 * `done` toplamı aşarsa toplam gösterilir (olay bozuksa bile "5 / 3" yazılmaz).
 *
 * Metinler F04-T08 gereği `lib/i18n.ts`ten gelir (harita modül düzeyinde kurulur,
 * `t()` import anında çözülür — tek dilli ürün kararı).
 */

/** Worker'ın bildirdiği aşamalar; bilinmeyen aşama olduğu gibi gösterilir. */
const STAGE_LABEL: Record<string, string> = {
  start: t("parseProgress.stage.start"),
  // §17.3: eşzamanlılık kapısı doluysa iş sırada bekler — sessiz bekleyiş yok.
  queued: t("parseProgress.stage.queued"),
  open: t("parseProgress.stage.open"),
  pages: t("parseProgress.stage.pages"),
  store: t("parseProgress.stage.store"),
  // F04-T07 maskeleme aşamaları (aynı kuyruk satırı ve olay biçimi).
  redact: t("parseProgress.stage.redact"),
  pack: t("parseProgress.stage.pack"),
  verify: t("parseProgress.stage.verify"),
  scan: t("parseProgress.stage.scan"),
};

export function parseStageLabel(stage: string): string {
  const key = stage.trim();
  if (!key) return t("parseProgress.fallback");
  return STAGE_LABEL[key] ?? key;
}

/**
 * Gösterim için gereken **en az** alan kümesi: aynı dil hem worker olayında
 * (`ParseProgressEvent`) hem kuyruk satırında (`jobs.progress_stage/done/total`,
 * 0022) kullanılır — iki yerde farklı biçim olmaz.
 */
export type ProgressCounters = Pick<ParseProgressEvent, "stage" | "done" | "total">;

/** Yüzde; toplam yoksa veya sıfırsa `null` (oran uydurulmaz). */
export function parseProgressPercent(event: ProgressCounters): number | null {
  if (event.total === null || !Number.isFinite(event.total) || event.total <= 0) return null;
  if (!Number.isFinite(event.done)) return null;
  const ratio = event.done / event.total;
  const percent = Math.round(ratio * 100);
  return Math.min(100, Math.max(0, percent));
}

/**
 * Tek satır gösterim. Toplam varsa "N / M · %X", yoksa yalnız sayılan
 * ("0" bilgi taşımıyorsa aşama etiketi tek başına döner).
 */
export function parseProgressLine(event: ProgressCounters | null): string | null {
  if (!event) return null;
  const label = parseStageLabel(event.stage);
  const percent = parseProgressPercent(event);
  if (percent === null || event.total === null) {
    return event.done > 0 ? t("parseProgress.countLine", { label, done: event.done }) : label;
  }
  const done = Math.min(Math.max(event.done, 0), event.total);
  return t("parseProgress.line", { label, done, total: event.total, percent });
}

/** İlerleme çubuğu için 0–100; oran yoksa `null`. */
export function parseProgressWidth(event: ProgressCounters | null): number | null {
  if (!event) return null;
  return parseProgressPercent(event);
}
