// "Bağlantıyı sına" sonucunun sunumu. Saf fonksiyonlar: ekran yalnız bunları
// çağırır, böylece etiket/uyarı mantığı DOM olmadan test edilir.

import type { CliProbeView } from "./api";

export type ProbeTone = "ok" | "warn" | "danger" | "plain";

/** Durum etiketi ve rozet tonu. Çıkış kodu 0 değilse başarısız sayılır. */
export function probeStatusLabel(
  view: Pick<CliProbeView, "status" | "exitCode">,
): { label: string; tone: ProbeTone } {
  switch (view.status) {
    case "exited":
      return view.exitCode === 0
        ? { label: "tamamlandı", tone: "ok" }
        : { label: `çıkış ${view.exitCode ?? "?"}`, tone: "danger" };
    case "timed_out":
      return { label: "süre doldu", tone: "warn" };
    case "cancelled":
      return { label: "iptal edildi", tone: "warn" };
    case "signaled":
      return { label: "sinyalle bitti", tone: "warn" };
    default:
      return { label: "bilinmiyor", tone: "plain" };
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
  const parts = [`${view.durationMs} ms`];
  if (view.exitCode !== null) parts.push(`çıkış kodu ${view.exitCode}`);
  parts.push(`sınır ${Math.round(view.timeoutMs / 1000)} sn`);
  return parts.join(" · ");
}

/** Kesilen çıktı "tam" sayılmaz; kullanıcıya açıkça söylenir. */
export function probeWarnings(
  view: Pick<CliProbeView, "stdoutTruncated" | "stderrTruncated">,
): string[] {
  const out: string[] = [];
  if (view.stdoutTruncated) out.push("stdout 64 KiB'te kesildi; çıktı tam değil.");
  if (view.stderrTruncated) out.push("stderr 64 KiB'te kesildi; çıktı tam değil.");
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

/** Bayt → okunur boyut (Türkçe ondalık virgül). */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kib = bytes / 1024;
  if (kib < 1024) return `${kib.toFixed(kib < 10 ? 1 : 0).replace(".", ",")} KiB`;
  return `${(kib / 1024).toFixed(1).replace(".", ",")} MiB`;
}

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
      ? "yok"
      : `${box.files.length} dosya · ${formatBytes(box.totalBytes)}`;
  return [
    {
      label: "İzin",
      value: box.workdirMode === null ? "doğrulanamadı (Unix dışı)" : `${box.workdirMode} yalnız kullanıcı`,
    },
    {
      label: "HOME / TMPDIR",
      value: box.homeIsolation === "sandbox" ? "kum havuzunda" : "kullanıcı ev dizini (açık seçim)",
    },
    { label: "stdin", value: `${formatBytes(box.stdinBytes)} / ${formatBytes(box.stdinLimitBytes)}` },
    { label: "Bırakılan dosya", value: files },
  ];
}

/** Kapsam sınırı aşıldıysa ya da politika zayıfsa kullanıcıya söylenir. */
export function probeScopeWarnings(view: Pick<CliProbeView, "sandbox">): string[] {
  const box = view.sandbox;
  const out: string[] = [];
  if (box.sizeLimitBytes > 0 && box.totalBytes > box.sizeLimitBytes) {
    out.push(
      `Kum havuzunda ${formatBytes(box.totalBytes)} dosya var; ${formatBytes(box.sizeLimitBytes)} sınırı aşıldı. Sınır sonradan ölçülür, süreç durdurulmadı.`,
    );
  }
  if (box.filesTruncated) {
    out.push(
      `Dosya listesi ${box.entryLimit} girdide kesildi; toplam bayt yalnız listelenen dosyaları kapsar.`,
    );
  }
  if (box.homeIsolation === "user_home") {
    out.push(
      "HOME/TMPDIR kullanıcı ev dizininde: alt süreç varsayılan yollarda kişisel yapılandırmayı görebilir.",
    );
  }
  if (box.workdirMode === null) {
    out.push("İş dizini izni bu platformda doğrulanamadı.");
  }
  return out;
}
