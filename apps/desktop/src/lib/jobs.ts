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
 *
 * Metinler F04-T08 gereği `lib/i18n.ts`ten gelir; haritalar modül düzeyinde
 * kurulduğu için `t()` **import anında** çözülür (sözlük `as const`, tek dilli
 * ürün kararı — dil değiştirme yok).
 */
import type { JobQueueSummary, JobRetryView, JobRow } from "./api";
import { t } from "./i18n";
import { parseProgressLine, parseProgressWidth, type ProgressCounters } from "./parseProgress";

const STATE_LABELS: Record<string, string> = {
  queued: t("jobs.state.queued"),
  blocked: t("jobs.state.blocked"),
  running: t("jobs.state.running"),
  validating: t("jobs.state.validating"),
  retry_wait: t("jobs.state.retry_wait"),
  canceling: t("jobs.state.canceling"),
  interrupted: t("jobs.state.interrupted"),
  partial: t("jobs.state.partial"),
  completed: t("jobs.state.completed"),
  failed: t("jobs.state.failed"),
  canceled: t("jobs.state.canceled"),
  // Eski adlar (0001–0020): veritabanı 0021 ile yeniden yazılır; görünümde
  // karşılaşılırsa gizlenmez.
  leased: t("jobs.state.leased"),
  succeeded: t("jobs.state.succeeded"),
  cancelled: t("jobs.state.cancelled"),
};

/**
 * Özet satırında durum adı **cümle içinde** küçük harfle yazılır
 * ("3 bekliyor"). Küçük harfli biçim sözlükte tutulur: `toLocaleLowerCase("tr")`
 * çağrısı biçim katmanını böler ve tek harfli İ/ı dönüşümünü dil ayarına bırakırdı.
 */
const STATE_LOWER: Record<string, string> = {
  running: t("jobs.stateLower.running"),
  validating: t("jobs.stateLower.validating"),
  queued: t("jobs.stateLower.queued"),
  blocked: t("jobs.stateLower.blocked"),
  retry_wait: t("jobs.stateLower.retry_wait"),
  canceling: t("jobs.stateLower.canceling"),
  interrupted: t("jobs.stateLower.interrupted"),
  partial: t("jobs.stateLower.partial"),
  completed: t("jobs.stateLower.completed"),
  failed: t("jobs.stateLower.failed"),
  canceled: t("jobs.stateLower.canceled"),
};

const TYPE_LABELS: Record<string, string> = {
  parse_document: t("jobs.type.parse_document"),
  import: t("jobs.type.import"),
  ocr_page: t("jobs.type.ocr_page"),
  index: t("jobs.type.index"),
  export: t("jobs.type.export"),
  backup: t("jobs.type.backup"),
};

const BLOCKED_REASONS: Record<string, string> = {
  password: t("jobs.blockedReason.password"),
  dependency: t("jobs.blockedReason.dependency"),
  quota: t("jobs.blockedReason.quota"),
  permission: t("jobs.blockedReason.permission"),
  vault_locked: t("jobs.blockedReason.vault_locked"),
};

/** Yeni adlara indirger: `succeeded` → `completed`, `leased` → `running`. */
export function canonicalJobState(state: string): string {
  if (state === "succeeded") return "completed";
  if (state === "cancelled") return "canceled";
  if (state === "leased") return "running";
  return state;
}

export function jobStateLabel(state: string): string {
  return STATE_LABELS[state] ?? t("jobs.unknownState", { state });
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
  return BLOCKED_REASONS[reason] ?? t("jobs.unknownBlocked", { reason });
}

/** Satırda hangi eylem gösterilmeli? Sunucu durumu belirler, buton uydurulmaz. */
export type JobAction = "cancel" | "retry" | "run" | "unblock";

/**
 * `queued` bir iş **sahipsizdir** (kuyrukta kendiliğinden çalışan zamanlayıcı
 * yok). Çökme sonrası kurtarılan ayrıştırma işi bu yüzden "Bekliyor" kalır;
 * "Başlat" gerçek `job_run` komutunu çağırır. Maskeleme gibi yürütücüsü
 * olmayan türlerde düğme gösterilmez (arka planda habersiz iş başlatılmaz).
 */
export function jobActions(job: JobRow): JobAction[] {
  switch (canonicalJobState(job.state)) {
    case "queued":
      return job.jobType === "parse_document" ? ["run", "cancel"] : ["cancel"];
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
  if (action === "cancel") return t("jobs.action.cancel");
  if (action === "retry") return t("jobs.action.retry");
  if (action === "run") return t("jobs.action.run");
  return t("jobs.action.unblock");
}

/**
 * İptal sonucunun tek satırı. Yalnız **gerçekleşen** sonuç yazılır
 * (şartname §17.5): süreç durmadıysa "iptal tamam" denmez, durdurulduysa
 * "bekleniyor" denmez.
 */
export function cancelOutcomeLine(result: {
  outcome: "canceled" | "requested";
  workerStopped: boolean;
}): string {
  if (result.outcome === "canceled") {
    return t("jobs.cancelNeverRan");
  }
  if (result.workerStopped) {
    return t("jobs.cancelStopped");
  }
  return t("jobs.cancelRequested");
}

export function countOf(summary: JobQueueSummary, state: string): number {
  const canonical = canonicalJobState(state);
  return summary.byState
    .filter((entry) => canonicalJobState(entry.state) === canonical)
    .reduce((total, entry) => total + entry.count, 0);
}

/** Tek satır özet: "12 iş · 3 bekliyor · 1 başarısız". */
export function queueSummaryLine(summary: JobQueueSummary): string {
  if (summary.total === 0) return t("jobs.summaryEmpty");
  const parts: string[] = [t("jobs.summaryTotal", { count: summary.total })];
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
    if (count > 0) {
      parts.push(t("jobs.summaryPart", { count, state: STATE_LOWER[state] ?? jobStateLabel(state) }));
    }
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
    lines.push(t("jobs.attentionFailed", { count: failed }));
  }
  if (blocked > 0) {
    lines.push(t("jobs.attentionBlocked", { count: blocked }));
  }
  if (summary.blockedByDependency > 0) {
    lines.push(t("jobs.attentionDependency", { count: summary.blockedByDependency }));
  }
  if (retry > 0) {
    lines.push(t("jobs.attentionRetry", { count: retry }));
  }
  if (interrupted > 0) {
    lines.push(t("jobs.attentionInterrupted", { count: interrupted }));
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
  return `${t("jobs.metaAttempt", { attempt })} · ${stamp}${code}${reason}`;
}

export function jobProgressPercent(job: JobRow): number {
  if (canonicalJobState(job.state) === "completed") return 100;
  const value = Math.round(job.progress * 100);
  if (!Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, value));
}

/**
 * Kuyruk satırındaki **canlı** sayaç (0022): `progress_stage/done/total`
 * sütunları ayrıştırma sırasında yazılır.
 *
 * Ayrıştırma satırıyla **aynı** dili kullanır (`lib/parseProgress.ts`); iki
 * yerde farklı biçim olmaz. Terminal işte sayaç `null` döner: bitmiş işte
 * önceki denemenin "12 / 40" kalıntısı yanıltıcı olur (durumu rozet söyler).
 */
export function jobProgressCounters(job: JobRow): ProgressCounters | null {
  const canonical = canonicalJobState(job.state);
  if (canonical !== "running" && canonical !== "validating" && canonical !== "canceling") {
    return null;
  }
  if (!job.progressStage) return null;
  // Sunucu toplamsız sayaç yazmaz; yine de savunmalı davran: toplam yoksa yüzde yok.
  return {
    stage: job.progressStage,
    done: job.progressDone ?? 0,
    total: job.progressTotal,
  };
}

export function jobProgressLine(job: JobRow): string | null {
  return parseProgressLine(jobProgressCounters(job));
}

export function jobProgressWidth(job: JobRow): number | null {
  return parseProgressWidth(jobProgressCounters(job));
}

/** Kart kendini tazelemeli mi? Yürüyen/doğrulanan/iptal edilen iş varsa evet. */
export function queueHasLiveJobs(jobs: JobRow[]): boolean {
  return jobs.some((job) => {
    const canonical = canonicalJobState(job.state);
    return canonical === "running" || canonical === "validating" || canonical === "canceling";
  });
}

/** Terminal durumlar: bu işler için canlı tazeleme anlamsızdır. */
const TERMINAL_STATES = new Set([
  "completed",
  "partial",
  "failed",
  "canceled",
  "interrupted",
]);

export function jobIsTerminal(state: string): boolean {
  return TERMINAL_STATES.has(canonicalJobState(state));
}

/**
 * Yeniden deneme **gerçekten başladıysa** kuyruk satırı kısa süre `queued`
 * görünür: yürütücü işi sahiplenene kadar canlı sayılmaz. Kart bu yüzden
 * `queueHasLiveJobs` yanında "izlenen iş" ile de tazelenir; aksi hâlde satır
 * `completed` olana kadar donmuş görünürdü.
 */
export function shouldPollQueue(jobs: JobRow[], watchJobId: string | null): boolean {
  if (queueHasLiveJobs(jobs)) return true;
  if (!watchJobId) return false;
  const watched = jobs.find((job) => job.id === watchJobId);
  return watched ? !jobIsTerminal(watched.state) : false;
}

/**
 * `job_retry` sonucunun **dürüst** satırı. "Sıraya alındı" ile "başladı"
 * ayrılır; başlamadıysa neden yazılır (sahte "çalışıyor" görüntüsü yok).
 */
export function retryOutcomeLine(view: JobRetryView): string {
  return startOutcomeLine(view, { verb: t("jobs.start.verbRetry"), requeued: true });
}

/**
 * Sırada bekleyen işi başlatan `job_run` sonucunun dürüst satırı. Yeniden
 * denemeyle aynı sözleşme (`JobRetryView`); yalnız eylem adı değişir ve
 * "yeniden sıraya alındı" gibi **olmayan** bir geçiş yazılmaz.
 */
export function runOutcomeLine(view: JobRetryView): string {
  return startOutcomeLine(view, { verb: t("jobs.start.verbRun"), requeued: false });
}

function startOutcomeLine(
  view: JobRetryView,
  wording: { verb: string; requeued: boolean },
): string {
  if (view.started) {
    const applied = view.applied;
    const settings = applied
      ? t("jobs.start.settings", {
          ocr: applied.ocr
            ? t("jobs.start.ocrOn", { lang: applied.ocrLang })
            : t("jobs.start.ocrOff"),
          encoding: applied.textEncoding,
        })
      : t("jobs.start.settingsDefault");
    // Kayıt yoksa bunu **söyle**: "aynı ayarla koştu" demek yanlış olurdu.
    const source = applied?.restored
      ? t("jobs.start.sourceStored")
      : t("jobs.start.sourceDefault");
    return t("jobs.start.line", { verb: wording.verb, source, settings });
  }
  const requeued = wording.requeued;
  if (view.skipped === "password_required") {
    return requeued ? t("jobs.start.passwordRetry") : t("jobs.start.passwordRun");
  }
  if (view.skipped === "type_not_runnable") {
    return requeued ? t("jobs.start.notRunnableRetry") : t("jobs.start.notRunnableRun");
  }
  if (view.skipped === "no_input") {
    return requeued ? t("jobs.start.noInputRetry") : t("jobs.start.noInputRun");
  }
  return requeued ? t("jobs.start.fallbackRetry") : t("jobs.start.fallbackRun");
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
  if (report.requeued > 0) parts.push(t("jobs.recovery.requeued", { count: report.requeued }));
  if (report.canceled > 0) parts.push(t("jobs.recovery.canceled", { count: report.canceled }));
  if (report.failed > 0) parts.push(t("jobs.recovery.failed", { count: report.failed }));
  if (parts.length === 0) {
    if (report.interrupted > 0) {
      return t("jobs.recovery.interruptedOnly", { count: report.interrupted });
    }
    return t("jobs.recovery.none");
  }
  return `${parts.join(", ")}.`;
}
