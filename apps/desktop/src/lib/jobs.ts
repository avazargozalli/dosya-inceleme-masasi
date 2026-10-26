/**
 * İş kuyruğu görünümü (F03-T07/T08).
 *
 * Buradaki yardımcılar **saf** fonksiyonlardır: sunucudan gelen sayıları
 * yorumlar, sayı uydurmaz. Kuyruk boşsa "0 iş" yazar; bir durum bilinmiyorsa
 * etiket "bilinmeyen" olur ve gizlenmez.
 *
 * Durum adları şartname §17.2'nin kümesidir (`completed`, `canceled`,
 * `blocked`, `validating`, `canceling`, `interrupted`, `partial`). Eski
 * kasalardan gelen `succeeded`/`cancelled`/`leased` adları da okunur.
 */
import type { JobQueueSummary, JobRow } from "./api";

const STATE_LABELS: Record<string, string> = {
  queued: "Bekliyor",
  blocked: "Engelli",
  running: "Yürüyor",
  validating: "Doğrulanıyor",
  retry_wait: "Tekrar bekliyor",
  canceling: "İptal ediliyor",
  interrupted: "Kesildi",
  partial: "Kısmi bitti",
  completed: "Bitti",
  failed: "Başarısız",
  canceled: "İptal edildi",
  // Eski adlar (0001–0020): veritabanı 0021 ile yeniden yazılır; görünümde
  // karşılaşılırsa gizlenmez.
  leased: "Sahiplenildi",
  succeeded: "Bitti",
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

const BLOCKED_REASONS: Record<string, string> = {
  password: "parola bekliyor",
  dependency: "bağımlılık bekliyor",
  quota: "kota bekliyor",
  permission: "izin bekliyor",
  vault_locked: "kasa kilitli",
};

/** Yeni adlara indirger: `succeeded` → `completed`, `leased` → `running`. */
export function canonicalJobState(state: string): string {
  if (state === "succeeded") return "completed";
  if (state === "cancelled") return "canceled";
  if (state === "leased") return "running";
  return state;
}

export function jobStateLabel(state: string): string {
  return STATE_LABELS[state] ?? `bilinmeyen (${state})`;
}

export function jobTypeLabel(jobType: string): string {
  return TYPE_LABELS[jobType] ?? jobType;
}

export function jobStateBadgeClass(state: string): string {
  const canonical = canonicalJobState(state);
  if (canonical === "completed") return "ok";
  if (canonical === "failed") return "danger";
  if (canonical === "canceled") return "";
  if (
    canonical === "blocked" ||
    canonical === "partial" ||
    canonical === "running" ||
    canonical === "validating" ||
    canonical === "canceling"
  ) {
    return "warn";
  }
  return "";
}

/** Engelli işin nedeni okunur metne çevrilir; bilinmeyen neden gizlenmez. */
export function blockedReasonLabel(reason: string | null): string | null {
  if (!reason) return null;
  return BLOCKED_REASONS[reason] ?? `engel: ${reason}`;
}

/** Satırda hangi eylem gösterilmeli? Sunucu durumu belirler, buton uydurulmaz. */
export type JobAction = "cancel" | "retry" | "unblock";

export function jobActions(job: JobRow): JobAction[] {
  switch (canonicalJobState(job.state)) {
    case "queued":
    case "retry_wait":
    case "running":
    case "validating":
      return ["cancel"];
    case "blocked":
      return ["unblock", "cancel"];
    case "failed":
      return ["retry"];
    default:
      return [];
  }
}

export function jobActionLabel(action: JobAction): string {
  if (action === "cancel") return "İptal et";
  if (action === "retry") return "Yeniden dene";
  return "Engeli kaldır";
}

export function countOf(summary: JobQueueSummary, state: string): number {
  const canonical = canonicalJobState(state);
  return summary.byState
    .filter((entry) => canonicalJobState(entry.state) === canonical)
    .reduce((total, entry) => total + entry.count, 0);
}

/** Tek satır özet: "12 iş · 3 bekliyor · 1 başarısız". */
export function queueSummaryLine(summary: JobQueueSummary): string {
  if (summary.total === 0) return "Kuyruk boş — bekleyen iş yok.";
  const parts: string[] = [`${summary.total} iş`];
  const order = [
    "running",
    "validating",
    "queued",
    "blocked",
    "retry_wait",
    "canceling",
    "interrupted",
    "partial",
    "completed",
    "failed",
    "canceled",
  ];
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
  const blocked = countOf(summary, "blocked");
  const retry = countOf(summary, "retry_wait");
  const interrupted = countOf(summary, "interrupted");
  const lines: string[] = [];
  if (failed > 0) {
    lines.push(
      `${failed} iş başarısız. Hata kodu iş listesinde; "Yeniden dene" yeni bir deneme açar.`,
    );
  }
  if (blocked > 0) {
    lines.push(
      `${blocked} iş engelli (parola/bağımlılık/kota). Engel kalkınca "Engeli kaldır" ile sıraya döner.`,
    );
  }
  if (summary.blockedByDependency > 0) {
    lines.push(
      `${summary.blockedByDependency} iş bağımlılığı bittiği için hiç çalıştırılmadı (sessizce beklemez).`,
    );
  }
  if (retry > 0) {
    lines.push(`${retry} iş tekrar denemesi bekliyor; süresi gelince kendiliğinden alınır.`);
  }
  if (interrupted > 0) {
    lines.push(
      `${interrupted} iş kesintiye uğradı; "Bekleyen işleri kurtar" ile yeniden sıraya alınır.`,
    );
  }
  return lines.length > 0 ? lines.join(" ") : null;
}

/** "deneme 1/3 · 2026-10-26 14:05" gibi tek satır. */
export function jobMetaLine(job: JobRow): string {
  const attempt = `${job.attemptCount}/${job.maxAttempts}`;
  const stamp = job.createdAt.replace("T", " ").slice(0, 16);
  const code = job.errorCode ? ` · ${job.errorCode}` : "";
  const blocked = blockedReasonLabel(job.blockedReason);
  const reason = blocked ? ` · ${blocked}` : "";
  return `deneme ${attempt} · ${stamp}${code}${reason}`;
}

export function jobProgressPercent(job: JobRow): number {
  if (canonicalJobState(job.state) === "completed") return 100;
  const value = Math.round(job.progress * 100);
  if (!Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, value));
}

export type RecoveryReport = {
  interrupted: number;
  requeued: number;
  failed: number;
  canceled: number;
};

/** Kurtarma sonucu tek satır; sayı sıfırsa "yarıda kalmış iş yok". */
export function recoveryLine(report: RecoveryReport): string {
  const parts: string[] = [];
  if (report.requeued > 0) parts.push(`${report.requeued} iş yeniden sıraya alındı`);
  if (report.canceled > 0) parts.push(`${report.canceled} iş iptal edildi`);
  if (report.failed > 0) parts.push(`${report.failed} iş tekrar bütçesi bittiği için kapandı`);
  if (parts.length === 0) {
    if (report.interrupted > 0) {
      return `${report.interrupted} iş kesildi olarak işaretlendi; yeniden sıraya alınacak bir şey yok.`;
    }
    return "Yarıda kalmış iş yok; yeniden sıraya alınacak bir şey bulunmadı.";
  }
  return `${parts.join(", ")}.`;
}
