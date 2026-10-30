// "Bağlantıyı sına" sonucunun sunumu. Saf fonksiyonlar: ekran yalnız bunları
// çağırır, böylece etiket/uyarı mantığı DOM olmadan test edilir.
// Metinler F04-T08 gereği merkezi sözlükten gelir (`lib/i18n.ts`).

import { formatSize } from "./format";
import { t } from "./i18n";
import type {
  CliCapabilityAdapter,
  CliCapabilityMatrix,
  CliCapabilityRow,
  CliProbeView,
  CliResidueCleanup,
  CliResidueMeasure,
  CliResiduePath,
  CliResidueScan,
  CliSandboxSweep,
} from "./api";

export type ProbeTone = "ok" | "warn" | "danger" | "plain";

/** Durum etiketi ve rozet tonu. Çıkış kodu 0 değilse başarısız sayılır. */
export function probeStatusLabel(
  view: Pick<CliProbeView, "status" | "exitCode">,
): { label: string; tone: ProbeTone } {
  switch (view.status) {
    case "exited":
      return view.exitCode === 0
        ? { label: t("probe.statusDone"), tone: "ok" }
        : { label: t("probe.statusExit", { code: view.exitCode ?? "?" }), tone: "danger" };
    case "timed_out":
      return { label: t("probe.statusTimedOut"), tone: "warn" };
    case "cancelled":
      return { label: t("probe.statusCancelled"), tone: "warn" };
    case "signaled":
      return { label: t("probe.statusSignaled"), tone: "warn" };
    default:
      return { label: t("probe.statusUnknown"), tone: "plain" };
  }
}

/** Gerçekten çalıştırılan komut satırı; kullanıcı ne çalıştığını görür. */
export function probeArgv(view: Pick<CliProbeView, "executable" | "args">): string {
  return [view.executable, ...view.args].join(" ");
}

/** Tek satır özet: süre, çıkış kodu, süre sınırı. */
export function probeSummary(
  view: Pick<CliProbeView, "exitCode" | "durationMs" | "timeoutMs">,
): string {
  const parts = [t("probe.summaryDuration", { ms: view.durationMs })];
  if (view.exitCode !== null) parts.push(t("probe.summaryExit", { code: view.exitCode }));
  parts.push(t("probe.summaryTimeout", { seconds: Math.round(view.timeoutMs / 1000) }));
  return parts.join(" · ");
}

/** Kesilen çıktı "tam" sayılmaz; kullanıcıya açıkça söylenir. */
export function probeWarnings(
  view: Pick<CliProbeView, "stdoutTruncated" | "stderrTruncated">,
): string[] {
  const out: string[] = [];
  if (view.stdoutTruncated) out.push(t("probe.warnStdoutTruncated"));
  if (view.stderrTruncated) out.push(t("probe.warnStderrTruncated"));
  return out;
}

/** Boş olmayan akışlar; ikisi de varsa ikisi de gösterilir. */
export function probeOutputs(
  view: Pick<CliProbeView, "stdoutHead" | "stderrHead">,
): Array<{ stream: "stdout" | "stderr"; text: string }> {
  const out: Array<{ stream: "stdout" | "stderr"; text: string }> = [];
  if (view.stdoutHead.trim() !== "") out.push({ stream: "stdout", text: view.stdoutHead });
  if (view.stderrHead.trim() !== "") out.push({ stream: "stderr", text: view.stderrHead });
  return out;
}

/** Düğme yalnız kurulu ve otomatik yürütmeye açık sağlayıcıda etkin. */
export function probeCanRun(
  row: { state: string; batchForbidden: boolean },
  running: string | null,
): boolean {
  return row.state === "installed" && !row.batchForbidden && running === null;
}

/**
 * Bayt → okunur boyut. **Tek uygulama** `format.ts::formatSize`'dır (F04-T08):
 * aynı değer üç ayrı biçimde yazılıyordu ("1.5 KB" / "1,5 KiB" / "1.5 MB").
 * Buradaki ad yalnız geriye uyum için durur.
 */
export const formatBytes = formatSize;

/**
 * Kapsam kanıtı satırları: iş dizini izni, `HOME`/`TMPDIR` politikası, stdin
 * boyutu ve alt sürecin bıraktığı dosyalar. Değerler çekirdekten gelir; ekran
 * bunları olduğu gibi gösterir.
 */
export function probeScopeLines(
  view: Pick<CliProbeView, "sandbox">,
): Array<{ label: string; value: string }> {
  const box = view.sandbox;
  const files =
    box.files.length === 0
      ? t("probe.filesNone")
      : t("probe.filesWithSize", {
          count: box.files.length,
          size: formatBytes(box.totalBytes),
        });
  return [
    {
      label: t("probe.scopePermLabel"),
      value:
        box.workdirMode === null
          ? t("probe.scopePermUnknown")
          : t("probe.scopePermValue", { mode: box.workdirMode }),
    },
    {
      label: t("probe.scopeHomeLabel"),
      value: box.homeIsolation === "sandbox" ? t("probe.scopeHomeSandbox") : t("probe.scopeHomeUser"),
    },
    {
      label: t("probe.scopeStdinLabel"),
      value: t("probe.scopeStdinValue", {
        used: formatBytes(box.stdinBytes),
        limit: formatBytes(box.stdinLimitBytes),
      }),
    },
    { label: t("probe.scopeFilesLabel"), value: files },
  ];
}

/** Kapsam sınırı aşıldıysa ya da politika zayıfsa kullanıcıya söylenir. */
export function probeScopeWarnings(view: Pick<CliProbeView, "sandbox">): string[] {
  const box = view.sandbox;
  const out: string[] = [];
  if (box.sizeLimitBytes > 0 && box.totalBytes > box.sizeLimitBytes) {
    out.push(
      t("probe.warnSizeLimit", {
        total: formatBytes(box.totalBytes),
        limit: formatBytes(box.sizeLimitBytes),
      }),
    );
  }
  if (box.filesTruncated) {
    out.push(t("probe.warnListTruncated", { entries: box.entryLimit }));
  }
  if (box.homeIsolation === "user_home") {
    out.push(t("probe.warnUserHome"));
  }
  if (box.workdirMode === null) {
    out.push(t("probe.warnWorkdirUnknown"));
  }
  return out;
}

// --- F10-T13: kalıntı envanteri ve ölçümü ---

/** Kalıntının yaşı: "az önce", "12 dk", "3 sa", "5 gün". */
export function residueAge(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds < 0) return t("common.unknown");
  if (seconds < 60) return t("probe.residueAgeNow");
  if (seconds < 3600) return t("probe.residueAgeMinutes", { count: Math.floor(seconds / 60) });
  if (seconds < 86400) return t("probe.residueAgeHours", { count: Math.floor(seconds / 3600) });
  return t("probe.residueAgeDays", { count: Math.floor(seconds / 86400) });
}

/** Bildirilen tek yolun satır özeti; "yok" ile "boş" ayrı gösterilir. */
export function residuePathLine(
  entry: Pick<CliResiduePath, "exists" | "files" | "bytes" | "truncated">,
): string {
  if (!entry.exists) return t("probe.residuePathMissing");
  if (entry.files === 0) return t("probe.residuePathEmpty");
  const suffix = entry.truncated ? ` · ${t("probe.residuePathTruncated")}` : "";
  return `${t("probe.filesWithSize", { count: entry.files, size: formatBytes(entry.bytes) })}${suffix}`;
}

/** Oturum kalıcılığını kapatma yolu; doğrulanmadıysa bunu açıkça söyler. */
export function persistenceText(
  scan: Pick<
    CliResidueScan,
    "persistenceFlag" | "persistenceAppliesTo" | "persistenceVerifiedWith"
  >,
): string {
  if (scan.persistenceFlag === null) {
    return t("probe.persistenceNone");
  }
  return t("probe.persistenceLine", {
    flag: scan.persistenceFlag,
    scope: scan.persistenceAppliesTo ?? "?",
    version: scan.persistenceVerifiedWith ?? "?",
  });
}

/** Tarama sınırına takılma ve eksik envanter uyarıları. */
export function residueScanWarnings(
  scan: Pick<CliResidueScan, "truncated" | "totalFiles" | "fileLimit" | "paths">,
): string[] {
  const out: string[] = [];
  if (scan.truncated) {
    out.push(t("probe.scanTruncated", { limit: scan.fileLimit }));
  }
  if (scan.paths.some((path) => path.truncated)) {
    out.push(t("probe.scanPathTruncated"));
  }
  return out;
}

/** Ölçüm sonucunun dürüst uyarıları: yeni/değişen dosya, kesik tarama. */
export function residueMeasureWarnings(
  view: Pick<
    CliResidueMeasure,
    "created" | "modified" | "after" | "usedUserHome" | "status"
  >,
): string[] {
  const out: string[] = [];
  if (view.usedUserHome) {
    out.push(t("probe.measureUserHome"));
  }
  if (view.created.length > 0) {
    out.push(t("probe.measureCreated", { count: view.created.length }));
  }
  if (view.modified.length > 0) {
    out.push(t("probe.measureModified", { count: view.modified.length }));
  }
  if (view.status !== "exited") {
    out.push(t("probe.measureIncomplete"));
  }
  if (view.after.truncated) {
    out.push(t("probe.measureAfterTruncated"));
  }
  return out;
}

/** Temizlenecek dosyalar: yalnız ölçümde **oluşanlar**. */
export function residueCleanupTargets(view: Pick<CliResidueMeasure, "created">): string[] {
  return view.created.map((file) => file.relative);
}

/** Ölçüm özeti: süre, yeni/değişen dosya, eklenen bayt. */
export function residueMeasureSummary(
  view: Pick<CliResidueMeasure, "durationMs" | "created" | "modified" | "bytesAdded">,
): string {
  return [
    t("probe.summaryDuration", { ms: view.durationMs }),
    t("probe.summaryCreated", { count: view.created.length }),
    t("probe.summaryModified", { count: view.modified.length }),
    t("probe.summaryAdded", { size: formatBytes(view.bytesAdded) }),
  ].join(" · ");
}

/** Temizlik sonucu tek satır; atlanan/hata varsa ayrıca yazılır. */
export function residueCleanupSummary(
  view: Pick<CliResidueCleanup, "removedFiles" | "removedBytes" | "removedDirs" | "skipped" | "errors">,
): string {
  const parts = [
    t("probe.cleanupRemoved", { count: view.removedFiles }),
    formatBytes(view.removedBytes),
    t("probe.cleanupDirs", { count: view.removedDirs }),
  ];
  if (view.skipped.length > 0) parts.push(t("probe.cleanupSkipped", { count: view.skipped.length }));
  if (view.errors.length > 0) parts.push(t("probe.cleanupErrors", { count: view.errors.length }));
  return parts.join(" · ");
}

/** Kum havuzu süpürme özeti. */
export function sandboxSweepSummary(
  view: Pick<CliSandboxSweep, "considered" | "removed" | "skipped" | "minAgeSecs">,
): string {
  return t("probe.sweep", {
    considered: view.considered,
    removed: view.removed.length,
    skipped: view.skipped.length,
    minutes: Math.round(view.minAgeSecs / 60),
  });
}

// --- F10-T11: yetenek matrisi ---

/**
 * Yetenek durumunun rozet tonu. `declared` **yeşil değildir**: bayrak yardım
 * metninde bildirildi ama davranış (araç çağrısı/oturum yazımı) denenmedi.
 */
export function capabilityTone(state: string): ProbeTone {
  switch (state) {
    case "missing":
      return "danger";
    case "no_path":
    case "unread":
      return "warn";
    case "declared":
      return "warn";
    default:
      return "plain";
  }
}

/** Bağlantı durumu rozeti: uyumsuz kırmızı, sınırlı/bildirildi sarı. */
export function linkTone(link: string): ProbeTone {
  switch (link) {
    case "incompatible":
      return "danger";
    case "limited":
    case "declared":
      return "warn";
    default:
      return "plain";
  }
}

/** Gerçekten yoklanan komut: kullanıcı ne çalıştığını görür. */
export function capabilityHelpCommand(
  view: Pick<CliCapabilityAdapter, "command" | "helpArgs">,
): string {
  return [view.command, ...view.helpArgs].join(" ");
}

/** Tek satır özet: kaç yetenek bildirildi / yok / doğrulanamadı. */
export function capabilitySummary(
  view: Pick<CliCapabilityAdapter, "rows">,
): string {
  const total = view.rows.length;
  const declared = view.rows.filter((row) => row.state === "declared").length;
  const missing = view.rows.filter((row) => row.state === "missing").length;
  const unknown = total - declared - missing;
  const parts = [t("probe.capabilityDeclared", { declared, total })];
  if (missing > 0) parts.push(t("probe.capabilityMissing", { count: missing }));
  if (unknown > 0) parts.push(t("probe.capabilityUnknown", { count: unknown }));
  if (view.rows.some((row) => row.state === "declared")) parts.push(t("probe.capabilityUntested"));
  return parts.join(" · ");
}

/** Gerekçe kodu (`tool_free:no_path`) → okunur satır (`Araçsız çalışma: …`). */
export function capabilityReasonLines(
  view: Pick<CliCapabilityAdapter, "rows" | "gateReasons">,
): string[] {
  return view.gateReasons.map((reason) => {
    const [id, state] = reason.split(":");
    const row = view.rows.find((candidate) => candidate.capability === id);
    if (!row) return reason;
    const stateLabel =
      row.state === "missing"
        ? t("probe.reasonMissing")
        : row.state === "no_path"
          ? t("probe.reasonNoPath")
          : row.state === "unread"
            ? t("probe.reasonUnread")
            : row.stateLabel;
    return t("probe.reasonLine", { label: row.label, state: stateLabel });
  });
}

/** Kapı tek satırda: açık/kapalı ve neden. */
export function capabilityGateText(
  view: Pick<CliCapabilityAdapter, "gateOpen" | "link" | "linkLabel" | "gateReasons" | "rows">,
): string {
  if (view.gateOpen) {
    return t("probe.gateOpen");
  }
  if (view.link === "not_installed") {
    return t("probe.gateNotInstalled");
  }
  const reasons = capabilityReasonLines(view);
  const detail = reasons.length > 0 ? ` — ${reasons.join("; ")}` : "";
  return t("probe.gateClosed", { link: view.linkLabel, detail });
}

/** Dürüstlük uyarıları: okunamayan yardım, kesik çıktı, eksik bayrak. */
export function capabilityWarnings(
  view: Pick<CliCapabilityAdapter, "helpError" | "helpRead" | "rows">,
): string[] {
  const out: string[] = [];
  if (view.helpError !== null && view.helpError !== "") {
    out.push(t("probe.warnHelpError", { error: view.helpError }));
  } else if (!view.helpRead && view.rows.some((row) => row.state === "unread")) {
    out.push(t("probe.warnHelpUnread"));
  }
  for (const row of view.rows.filter((candidate) => candidate.state === "missing")) {
    out.push(t("probe.warnFlagMissing", { label: row.label, flags: row.flags.join(" ") }));
  }
  return out;
}

/** Matrisin en kötü bağlantı durumu (kart başlığı için). */
export function matrixWorstLink(
  view: Pick<CliCapabilityMatrix, "adapters">,
): { link: string; label: string } {
  const severity: Record<string, number> = {
    declared: 0,
    not_installed: 1,
    limited: 2,
    incompatible: 3,
  };
  let worst: { link: string; label: string } = {
    link: "unknown",
    label: t("probe.worstUnknown"),
  };
  let worstScore = -1;
  for (const adapter of view.adapters) {
    const score = severity[adapter.link] ?? 0;
    if (score > worstScore) {
      worstScore = score;
      worst = { link: adapter.link, label: adapter.linkLabel };
    }
  }
  return worst;
}

/** Satır ayrıntısı boşsa gösterilecek metin. */
export function capabilityDetailText(row: Pick<CliCapabilityRow, "detail">): string {
  return row.detail.trim() === "" ? t("common.unknown") : row.detail;
}
