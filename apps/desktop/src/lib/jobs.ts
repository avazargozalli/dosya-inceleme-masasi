/**
 * İş kuyruğu görünümü (F03-T07/T08).
 *
 * Buradaki yardımcılar **saf** fonksiyonlardır: sunucudan gelen sayıları
 * yorumlar, sayı uydurmaz. Kuyruk boşsa "0 iş" yazar; bir durum bilinmiyorsa
 * etiket "bilinmeyen" olur ve gizlenmez.
 */
import type { JobQueueSummary, JobRow } from "./api";

const STATE_LABELS: Record<string, string> = {
  queued: "Bekliyor",
  retry_wait: "Tekrar bekliyor",
  running: "Yürüyor",
  leased: "Sahiplenildi",
  succeeded: "Bitti",
  failed: "Başarısız",
  cancelled: "İptal edildi",
};

const TYPE_LABELS: Record<string, string> = {
  parse_document: "Belge ayrıştırma",
  import: "İçe aktarma",
  ocr_page: "OCR",
  index: "Dizinleme",
  export: "Çıktı",
  backup: "Yedek",
};

export function jobStateLabel(state: string): string {
  return STATE_LABELS[state] ?? `bilinmeyen (${state})`;
}

export function jobTypeLabel(jobType: string): string {
  return TYPE_LABELS[jobType] ?? jobType;
}

export function jobStateBadgeClass(state: string): string {
  if (state === "succeeded") return "ok";
  if (state === "failed") return "danger";
  if (state === "cancelled") return "";
  if (state === "running" || state === "leased") return "warn";
  return "";
}

export function countOf(summary: JobQueueSummary, state: string): number {
  return summary.byState.find((entry) => entry.state === state)?.count ?? 0;
}

/** Tek satır özet: "12 iş · 3 bekliyor · 1 başarısız". */
export function queueSummaryLine(summary: JobQueueSummary): string {
  if (summary.total === 0) return "Kuyruk boş — bekleyen iş yok.";
  const parts: string[] = [`${summary.total} iş`];
  const order = ["running", "queued", "retry_wait", "succeeded", "failed", "cancelled"];
  for (const state of order) {
    const count = countOf(summary, state);
    if (count > 0) parts.push(`${count} ${jobStateLabel(state).toLocaleLowerCase("tr")}`);
  }
  return parts.join(" · ");
}

/**
 * Dikkat gerektiren durum varsa gerekçe döner; yoksa `null`.
 * Kuyrukta hiç iş yokken uyarı üretilmez.
 */
export function queueAttention(summary: JobQueueSummary): string | null {
  const failed = countOf(summary, "failed");
  const retry = countOf(summary, "retry_wait");
  const lines: string[] = [];
  if (failed > 0) {
    lines.push(
      `${failed} iş başarısız. Hata kodu iş listesinde; belge yeniden ayrıştırılabilir.`,
    );
  }
  if (summary.blockedByDependency > 0) {
    lines.push(
      `${summary.blockedByDependency} iş bağımlılığı bittiği için çalıştırılmadı (sessizce beklemez).`,
    );
  }
  if (retry > 0) {
    lines.push(`${retry} iş tekrar denemesi bekliyor; süresi gelince kendiliğinden alınır.`);
  }
  return lines.length > 0 ? lines.join(" ") : null;
}

/** "deneme 1/3 · 2026-10-26 14:05" gibi tek satır. */
export function jobMetaLine(job: JobRow): string {
  const attempt = `${job.attemptCount}/${job.maxAttempts}`;
  const stamp = job.createdAt.replace("T", " ").slice(0, 16);
  const code = job.errorCode ? ` · ${job.errorCode}` : "";
  return `deneme ${attempt} · ${stamp}${code}`;
}

export function jobProgressPercent(job: JobRow): number {
  if (job.state === "succeeded") return 100;
  const value = Math.round(job.progress * 100);
  if (!Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, value));
}

/** Kurtarma sonucu tek satır; sayı sıfırsa "yarıda kalmış iş yok". */
export function recoveryLine(interrupted: number, cancelled: number): string {
  if (interrupted === 0 && cancelled === 0) {
    return "Yarıda kalmış iş yok; yeniden sıraya alınacak bir şey bulunmadı.";
  }
  const parts: string[] = [];
  if (interrupted > 0) parts.push(`${interrupted} iş yeniden sıraya alındı`);
  if (cancelled > 0) parts.push(`${cancelled} iş iptal edildi`);
  return `${parts.join(", ")}.`;
}
