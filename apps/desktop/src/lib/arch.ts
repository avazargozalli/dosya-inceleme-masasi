// F10-T01 — mimari uyum denetiminin sunumu. Saf fonksiyonlar: ekran yalnız
// bunları çağırır, böylece rozet/uyarı mantığı DOM olmadan test edilir.
//
// Değerler çekirdekten gelir (başlık baytlarından okunan biçim + mimari);
// burada mimari **tahmin edilmez**, yalnız etiketlenir. Metinler F04-T08
// gereği merkezi sözlükten gelir (`lib/i18n.ts`).

import type { AdapterArch, CliArchAdapter, CliArchReport } from "./api";
import { formatUnixSeconds } from "./format";
import { t, type MessageKey } from "./i18n";

export type ArchTone = "ok" | "danger" | "warn" | "plain";

/** Karşılaştırma tonu: aynı → yeşil, farklı → kırmızı, evrensel → sarı. */
export function archTone(comparison: string): ArchTone {
  switch (comparison) {
    case "same":
      return "ok";
    case "different":
      return "danger";
    case "fat":
      return "warn";
    default:
      return "plain";
  }
}

/** Rozet metni. Evrenselde uygulama mimarisinin içerilip içerilmediği yazılır. */
export function archBadgeText(arch: { comparison: string; containsAppArch: boolean | null }): string {
  switch (arch.comparison) {
    case "same":
      return t("arch.badgeSame");
    case "different":
      return t("arch.badgeDifferent");
    case "fat":
      if (arch.containsAppArch === true) return t("arch.badgeFatWithApp");
      if (arch.containsAppArch === false) return t("arch.badgeFatWithoutApp");
      return t("arch.badgeFat");
    default:
      return t("arch.badgeUnknown");
  }
}

/** Mimari listesi: "aarch64 + x86_64"; boşsa tire. */
export function archList(architectures: string[]): string {
  return architectures.length === 0 ? t("common.unknown") : architectures.join(" + ");
}

/** Biçim satırı: Mach-O/ELF/PE/betik + sembolik bağ notu. */
export function archFormatText(row: Pick<CliArchAdapter, "formatLabel" | "symlink">): string {
  return row.symlink ? `${row.formatLabel} · ${t("arch.symlink")}` : row.formatLabel;
}

/** Yol: PATH'te yoksa açıkça söylenir; bağ çözümü ayrı satırda verilir. */
export function archPathText(row: Pick<CliArchAdapter, "installed" | "path" | "resolved">): string {
  if (!row.installed || row.path === null) return t("arch.notOnPath");
  if (row.resolved !== null && row.resolved !== row.path) return `${row.path} → ${row.resolved}`;
  return row.path;
}

/** Neden kodu → sözlük anahtarı. Bilinmeyen kod olduğu gibi gösterilir. */
const REASON_KEYS: Record<string, MessageKey> = {
  not_found: "arch.reasonNotFound",
  not_a_file: "arch.reasonNotAFile",
  read_error: "arch.reasonReadError",
  empty_file: "arch.reasonEmptyFile",
  header_too_short: "arch.reasonHeaderTooShort",
  magic_unrecognized: "arch.reasonMagicUnrecognized",
  fat_truncated: "arch.reasonFatTruncated",
  fat_empty: "arch.reasonFatEmpty",
  fat_too_many: "arch.reasonFatTooMany",
  elf_unknown_endian: "arch.reasonElfUnknownEndian",
  elf_unknown_class: "arch.reasonElfUnknownClass",
  elf_machine_none: "arch.reasonElfMachineNone",
  pe_signature_missing: "arch.reasonPeSignatureMissing",
  pe_offset_too_large: "arch.reasonPeOffsetTooLarge",
  pe_machine_none: "arch.reasonPeMachineNone",
  interpreter_unresolved: "arch.reasonInterpreterUnresolved",
  empty_shebang: "arch.reasonEmptyShebang",
  nested_script: "arch.reasonNestedScript",
  interpreter_chain: "arch.reasonInterpreterChain",
};

/** Neden kodu → okunur metin. Bilinmeyen kod olduğu gibi gösterilir. */
export function archReasonText(reason: string | null): string | null {
  if (reason === null) return null;
  const key = REASON_KEYS[reason];
  return key === undefined ? reason : t(key);
}

/** Shebang satırı: komut ve çözülen yol. Yorumlayıcı yoksa `null`. */
export function archInterpreterText(row: Pick<CliArchAdapter, "interpreter">): string | null {
  const interp = row.interpreter;
  if (interp === null) return null;
  const target = interp.resolved ?? interp.path ?? t("arch.interpreterMissing");
  const arch = interp.architectures.length > 0 ? ` · ${archList(interp.architectures)}` : "";
  return `${t("arch.interpreterLine", { command: interp.command, target })}${arch}`;
}

/** Tek satır özet: uygulama mimarisi + sayımlar. */
export function archSummaryText(
  report: Pick<CliArchReport, "appArch" | "same" | "different" | "fat" | "unknown" | "adapters">,
): string {
  return t("arch.summary", {
    appArch: report.appArch,
    adapters: report.adapters.length,
    same: report.same,
    different: report.different,
    fat: report.fat,
    unknown: report.unknown,
  });
}

/** En kötü karar: farklı > doğrulanamadı > evrensel > aynı. */
export function archWorst(
  report: Pick<CliArchReport, "same" | "different" | "fat" | "unknown">,
): { label: string; tone: ArchTone } {
  if (report.different > 0) return { label: t("arch.worstDifferent"), tone: "danger" };
  if (report.unknown > 0) return { label: t("arch.worstUnknown"), tone: "plain" };
  if (report.fat > 0) return { label: t("arch.worstFat"), tone: "warn" };
  if (report.same > 0) return { label: t("arch.worstSame"), tone: "ok" };
  return { label: t("arch.worstNone"), tone: "plain" };
}

/** Dürüst uyarılar: farklı/doğrulanamayan/evrensel-uyumsuz ikili varsa. */
export function archWarnings(
  report: Pick<CliArchReport, "adapters" | "different" | "unknown" | "fat">,
): string[] {
  const out: string[] = [];
  if (report.different > 0) {
    const names = report.adapters
      .filter((row) => row.comparison === "different")
      .map((row) => row.displayName);
    out.push(t("arch.warnDifferent", { count: report.different, names: names.join(", ") }));
  }
  const fatWithoutApp = report.adapters.filter(
    (row) => row.comparison === "fat" && row.containsAppArch === false,
  );
  if (fatWithoutApp.length > 0) {
    out.push(
      t("arch.warnFatWithoutApp", {
        names: fatWithoutApp.map((row) => row.displayName).join(", "),
      }),
    );
  }
  if (report.unknown > 0) {
    const names = report.adapters
      .filter((row) => row.comparison === "unknown")
      .map((row) => row.displayName);
    out.push(t("arch.warnUnknown", { names: names.join(", ") }));
  }
  return out;
}

/** Adaptör kartındaki mimari rozeti; veri yoksa nötr. */
export function adapterArchBadge(arch: AdapterArch | null): { label: string; tone: ArchTone } {
  if (arch === null) return { label: t("arch.badgeUnknown"), tone: "plain" };
  return { label: archBadgeText(arch), tone: archTone(arch.comparison) };
}

/** Eski kalıntı/ölçüm testlerinde kullanılan zaman biçimiyle tutarlı damga. */
export function archProbedAt(unixSeconds: number): string {
  return formatUnixSeconds(unixSeconds);
}
