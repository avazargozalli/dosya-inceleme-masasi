// F10-T01 — mimari uyum denetiminin sunumu. Saf fonksiyonlar: ekran yalnız
// bunları çağırır, böylece rozet/uyarı mantığı DOM olmadan test edilir.
//
// Değerler çekirdekten gelir (başlık baytlarından okunan biçim + mimari);
// burada mimari **tahmin edilmez**, yalnız etiketlenir.

import type { AdapterArch, CliArchAdapter, CliArchReport } from "./api";
import { formatUnixSeconds } from "./format";

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
      return "mimari aynı";
    case "different":
      return "mimari farklı";
    case "fat":
      if (arch.containsAppArch === true) return "evrensel · uygulama mimarisi var";
      if (arch.containsAppArch === false) return "evrensel · uygulama mimarisi yok";
      return "evrensel";
    default:
      return "mimari bilinmiyor";
  }
}

/** Mimari listesi: "aarch64 + x86_64"; boşsa "—". */
export function archList(architectures: string[]): string {
  return architectures.length === 0 ? "—" : architectures.join(" + ");
}

/** Biçim satırı: Mach-O/ELF/PE/betik + sembolik bağ notu. */
export function archFormatText(row: Pick<CliArchAdapter, "formatLabel" | "symlink">): string {
  return row.symlink ? `${row.formatLabel} · sembolik bağ` : row.formatLabel;
}

/** Yol: PATH'te yoksa açıkça söylenir; bağ çözümü ayrı satırda verilir. */
export function archPathText(row: Pick<CliArchAdapter, "installed" | "path" | "resolved">): string {
  if (!row.installed || row.path === null) return "PATH üzerinde bulunamadı";
  if (row.resolved !== null && row.resolved !== row.path) return `${row.path} → ${row.resolved}`;
  return row.path;
}

/** Neden kodu → okunur metin. Bilinmeyen kod olduğu gibi gösterilir. */
export function archReasonText(reason: string | null): string | null {
  if (reason === null) return null;
  const map: Record<string, string> = {
    not_found: "yol bulunamadı",
    not_a_file: "yol bir dosya değil",
    read_error: "dosya okunamadı",
    empty_file: "dosya boş",
    header_too_short: "başlık çok kısa",
    magic_unrecognized: "başlık tanınmadı",
    fat_truncated: "evrensel başlık kesik",
    fat_empty: "evrensel başlıkta dilim yok",
    fat_too_many: "evrensel başlıkta çok fazla dilim",
    elf_unknown_endian: "ELF bayt sırası bilinmiyor",
    elf_unknown_class: "ELF sınıfı bilinmiyor",
    elf_machine_none: "ELF makine alanı boş",
    pe_signature_missing: "PE imzası okunamadı",
    pe_offset_too_large: "PE başlık konumu sınır dışı",
    pe_machine_none: "PE makine alanı boş",
    interpreter_unresolved: "shebang yorumlayıcısı bulunamadı",
    empty_shebang: "shebang boş",
    nested_script: "yorumlayıcı da betik (zincir çözülmedi)",
    interpreter_chain: "yorumlayıcı zinciri çok derin",
  };
  return map[reason] ?? reason;
}

/** Shebang satırı: komut ve çözülen yol. Yorumlayıcı yoksa `null`. */
export function archInterpreterText(row: Pick<CliArchAdapter, "interpreter">): string | null {
  const interp = row.interpreter;
  if (interp === null) return null;
  const target = interp.resolved ?? interp.path ?? "bulunamadı";
  const arch = interp.architectures.length > 0 ? ` · ${archList(interp.architectures)}` : "";
  return `shebang: ${interp.command} → ${target}${arch}`;
}

/** Tek satır özet: uygulama mimarisi + sayımlar. */
export function archSummaryText(
  report: Pick<CliArchReport, "appArch" | "same" | "different" | "fat" | "unknown" | "adapters">,
): string {
  return (
    `uygulama ${report.appArch} · ${report.adapters.length} adaptör · ` +
    `${report.same} aynı · ${report.different} farklı · ${report.fat} evrensel · ` +
    `${report.unknown} doğrulanamadı`
  );
}

/** En kötü karar: farklı > doğrulanamadı > evrensel > aynı. */
export function archWorst(
  report: Pick<CliArchReport, "same" | "different" | "fat" | "unknown">,
): { label: string; tone: ArchTone } {
  if (report.different > 0) return { label: "farklı mimari var", tone: "danger" };
  if (report.unknown > 0) return { label: "doğrulanamayan var", tone: "plain" };
  if (report.fat > 0) return { label: "evrensel ikili var", tone: "warn" };
  if (report.same > 0) return { label: "hepsi aynı mimari", tone: "ok" };
  return { label: "denetlenecek ikili yok", tone: "plain" };
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
    out.push(
      `${report.different} adaptör uygulamadan farklı mimaride (${names.join(", ")}); ` +
        "bu ikili uygulamanın süreciyle aynı mimaride değil.",
    );
  }
  const fatWithoutApp = report.adapters.filter(
    (row) => row.comparison === "fat" && row.containsAppArch === false,
  );
  if (fatWithoutApp.length > 0) {
    out.push(
      `${fatWithoutApp.map((row) => row.displayName).join(", ")} evrensel ama uygulama mimarisini içermiyor; ` +
        "bu makinede çalışması ek bir yorumlayıcı/çeviri katmanına bağlı olabilir.",
    );
  }
  if (report.unknown > 0) {
    const names = report.adapters
      .filter((row) => row.comparison === "unknown")
      .map((row) => row.displayName);
    out.push(
      `${names.join(", ")} için mimari doğrulanamadı; "aynı" sayılmaz. ` +
        "Neden satırına bakın (yol yok, başlık tanınmadı ya da yorumlayıcı bulunamadı).",
    );
  }
  return out;
}

/** Adaptör kartındaki mimari rozeti; veri yoksa nötr. */
export function adapterArchBadge(arch: AdapterArch | null): { label: string; tone: ArchTone } {
  if (arch === null) return { label: "mimari bilinmiyor", tone: "plain" };
  return { label: archBadgeText(arch), tone: archTone(arch.comparison) };
}

/** Eski kalıntı/ölçüm testlerinde kullanılan zaman biçimiyle tutarlı damga. */
export function archProbedAt(unixSeconds: number): string {
  return formatUnixSeconds(unixSeconds);
}
