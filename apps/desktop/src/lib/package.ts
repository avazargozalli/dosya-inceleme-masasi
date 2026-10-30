import type {
  PackageProblem,
  PackageProblemKind,
  PackageVerification,
  WorkerPackageManifest,
  WorkerPackageView,
} from "./api";
import { t, type MessageKey } from "./i18n";
import { formatBytes } from "./providers";

/** Paket durumunun tek satır etiketi. `found=false` hata değil, geliştirme kipi. */
export type PackageTone = "ok" | "danger" | "warn" | "plain";

export function packageTone(view: WorkerPackageView | null): PackageTone {
  if (!view || !view.found || !view.verification) return "plain";
  // Kesilmiş tarama "kanıtlanmış kusur" değil, "eksik kanıt"tır: uyarı tonu.
  if (view.verification.truncated) return "warn";
  if (!view.verification.ok) return "danger";
  return "ok";
}

export function packageStatusText(view: WorkerPackageView | null): string {
  if (!view) return t("package.statusUnverified");
  if (!view.found) return t("package.statusMissing");
  const verification = view.verification;
  if (!verification) return t("package.statusUnverified");
  if (verification.truncated) return t("package.statusTruncated");
  if (verification.ok) return t("package.statusOk");
  return t("package.statusBroken");
}

/** `118 dosya + 18 bağ · 75,6 MiB` gibi tek satır. */
export function packageSummary(manifest: WorkerPackageManifest | null): string {
  if (!manifest) return t("common.unknown");
  const totals = manifest.totals;
  const head =
    totals.symlinks > 0
      ? t("package.summaryFilesLinks", { files: totals.files, links: totals.symlinks })
      : t("package.summaryFiles", { files: totals.files });
  return t("package.summaryLine", { head, size: formatBytes(totals.bytes) });
}

/** Rol kodu → sözlük anahtarı (modül düzeyinde metin yok: `t()` çağrı anında). */
const ROLE_KEYS: Record<string, MessageKey> = {
  bootloader: "package.role.bootloader",
  python_runtime: "package.role.python_runtime",
  stdlib_archive: "package.role.stdlib_archive",
  pdf_engine: "package.role.pdf_engine",
  docx_engine: "package.role.docx_engine",
  image_lib: "package.role.image_lib",
  ocr_bridge: "package.role.ocr_bridge",
  native_library: "package.role.native_library",
  python_module: "package.role.python_module",
  package_metadata: "package.role.package_metadata",
  data_file: "package.role.data_file",
  font: "package.role.font",
  other: "package.role.other",
  unreadable: "package.role.unreadable",
};

export function roleLabel(role: string): string {
  const key = ROLE_KEYS[role];
  return key === undefined ? role : t(key);
}

/** Roller: adetçe büyükten küçüğe, eşitse ada göre (deterministik). */
export function roleRows(manifest: WorkerPackageManifest | null): { role: string; label: string; count: number }[] {
  if (!manifest) return [];
  return Object.entries(manifest.roles)
    .map(([role, count]) => ({ role, label: roleLabel(role), count: Number(count) }))
    .filter((row) => Number.isFinite(row.count) && row.count > 0)
    .sort((a, b) => (b.count - a.count) || a.role.localeCompare(b.role, "tr"));
}

const PROBLEM_KEYS: Record<PackageProblemKind, MessageKey> = {
  missing: "package.problem.missing",
  sizeChanged: "package.problem.sizeChanged",
  hashChanged: "package.problem.hashChanged",
  linkTargetChanged: "package.problem.linkTargetChanged",
  unverifiable: "package.problem.unverifiable",
  wrongKind: "package.problem.wrongKind",
  unreadable: "package.problem.unreadable",
  unsafeLink: "package.problem.unsafeLink",
};

export function problemText(problem: PackageProblem): string {
  const key = PROBLEM_KEYS[problem.problem];
  const label = key === undefined ? problem.problem : t(key);
  return problem.detail ? t("package.problemWithDetail", { label, detail: problem.detail }) : label;
}

export function verificationCounts(verification: PackageVerification | null): {
  verified: number;
  problems: number;
  extra: number;
} {
  if (!verification) return { verified: 0, problems: 0, extra: 0 };
  return {
    verified: verification.verified,
    problems: verification.problemCount,
    extra: verification.extraCount,
  };
}

/**
 * Arayüzde gösterilecek dürüst uyarılar. Hiçbiri "geçti" iddiasını süslemez:
 * kesilmiş tarama, fazla dosya, doğrulanamayan girdi ve Tesseract yokluğu ayrı
 * ayrı yazılır.
 */
export function packageWarnings(view: WorkerPackageView | null): string[] {
  if (!view) return [];
  const warnings: string[] = [];
  if (!view.found) {
    warnings.push(view.note ?? t("package.warnNotFound"));
  }
  const verification = view.verification;
  if (verification) {
    if (verification.problemCount > 0) {
      warnings.push(t("package.warnProblems", { count: verification.problemCount }));
    }
    if (verification.extraCount > 0) {
      warnings.push(t("package.warnExtra", { count: verification.extraCount }));
    }
    if (!verification.entryPointPresent) {
      warnings.push(t("package.warnEntryPoint"));
    }
    if (verification.truncated) {
      warnings.push(t("package.warnTruncated"));
    }
    if (verification.problems.length < verification.problemCount) {
      warnings.push(
        t("package.warnListCut", {
          listed: verification.problems.length,
          total: verification.problemCount,
        }),
      );
    }
  }
  if (view.found && !view.tesseractOnPath) {
    warnings.push(t("package.warnNoTesseract"));
  }
  return warnings;
}

/** Arayüzün altındaki sınır cümlesi (tek yerde; çağrı anında sözlükten). */
export function packageLimits(): string {
  return t("package.limits");
}
