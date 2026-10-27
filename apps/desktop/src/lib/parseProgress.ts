import type { ParseProgressEvent } from "./api";

/**
 * Ayrıştırma ilerlemesinin arayüz dili (F04-T07).
 *
 * Worker `parse-progress` olayıyla aşama ve sayaç bildirir; buradaki yardımcılar
 * yalnız **gösterim** üretir. Sayaç uydurulmaz: `total` yoksa oran hesaplanmaz,
 * `done` toplamı aşarsa toplam gösterilir (olay bozuksa bile "5 / 3" yazılmaz).
 */

/** Worker'ın bildirdiği aşamalar; bilinmeyen aşama olduğu gibi gösterilir. */
const STAGE_LABEL: Record<string, string> = {
  start: "Hazırlanıyor",
  // §17.3: eşzamanlılık kapısı doluysa iş sırada bekler — sessiz bekleyiş yok.
  queued: "Sırada (en fazla 2 ayrıştırma)",
  open: "Belge açıldı",
  pages: "Sayfalar okunuyor",
  store: "Kasaya yazılıyor",
};

export function parseStageLabel(stage: string): string {
  const key = stage.trim();
  if (!key) return "Ayrıştırılıyor";
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
    return event.done > 0 ? `${label}: ${event.done}` : label;
  }
  const done = Math.min(Math.max(event.done, 0), event.total);
  return `${label}: ${done} / ${event.total} · %${percent}`;
}

/** İlerleme çubuğu için 0–100; oran yoksa `null`. */
export function parseProgressWidth(event: ProgressCounters | null): number | null {
  if (!event) return null;
  return parseProgressPercent(event);
}
