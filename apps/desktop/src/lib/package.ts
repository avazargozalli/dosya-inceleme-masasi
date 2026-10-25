import type {
  PackageProblem,
  PackageProblemKind,
  PackageVerification,
  WorkerPackageManifest,
  WorkerPackageView,
} from "./api";
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
  if (!view) return "Doğrulanmadı";
  if (!view.found) return "Paket yok";
  const verification = view.verification;
  if (!verification) return "Doğrulanmadı";
  if (verification.truncated) return "Sınırda kesildi";
  if (verification.ok) return "Bütünlük doğrulandı";
  return "Kusurlu";
}

/** `118 dosya + 18 bağ · 75,6 MB` gibi tek satır. */
export function packageSummary(manifest: WorkerPackageManifest | null): string {
  if (!manifest) return "—";
  const totals = manifest.totals;
  const head =
    totals.symlinks > 0 ? `${totals.files} dosya + ${totals.symlinks} bağ` : `${totals.files} dosya`;
  return `${head} · ${formatBytes(totals.bytes)}`;
}

export const ROLE_LABELS: Record<string, string> = {
  bootloader: "Başlatıcı",
  python_runtime: "Python çalışma zamanı",
  stdlib_archive: "Standart kitaplık",
  pdf_engine: "PDF motoru",
  docx_engine: "DOCX motoru",
  image_lib: "Görüntü kitaplığı",
  ocr_bridge: "OCR köprüsü",
  native_library: "Yerel kitaplık",
  python_module: "Python modülü",
  package_metadata: "Paket künyesi",
  data_file: "Veri dosyası",
  font: "Yazı tipi",
  other: "Diğer",
  unreadable: "Okunamadı",
};

export function roleLabel(role: string): string {
  return ROLE_LABELS[role] ?? role;
}

/** Roller: adetçe büyükten küçüğe, eşitse ada göre (deterministik). */
export function roleRows(manifest: WorkerPackageManifest | null): { role: string; label: string; count: number }[] {
  if (!manifest) return [];
  return Object.entries(manifest.roles)
    .map(([role, count]) => ({ role, label: roleLabel(role), count: Number(count) }))
    .filter((row) => Number.isFinite(row.count) && row.count > 0)
    .sort((a, b) => (b.count - a.count) || a.role.localeCompare(b.role, "tr"));
}

export const PROBLEM_LABELS: Record<PackageProblemKind, string> = {
  missing: "eksik",
  sizeChanged: "boyut değişti",
  hashChanged: "içerik değişti",
  linkTargetChanged: "bağ hedefi değişti",
  unverifiable: "doğrulanamaz",
  wrongKind: "tür uyuşmuyor",
  unreadable: "okunamadı",
  unsafeLink: "güvensiz bağ",
};

export function problemText(problem: PackageProblem): string {
  const label = PROBLEM_LABELS[problem.problem] ?? problem.problem;
  return problem.detail ? `${label} (${problem.detail})` : label;
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
    warnings.push(view.note ?? "Paketlenmiş worker bulunamadı.");
  }
  const verification = view.verification;
  if (verification) {
    if (verification.problemCount > 0) {
      warnings.push(`${verification.problemCount} dosya manifestle uyuşmuyor.`);
    }
    if (verification.extraCount > 0) {
      warnings.push(`Manifestte olmayan ${verification.extraCount} dosya var (paket beklenenden fazla).`);
    }
    if (!verification.entryPointPresent) {
      warnings.push("Giriş noktası ikilisi diskte yok.");
    }
    if (verification.truncated) {
      warnings.push("Tarama girdi sınırına takıldı: sonuç eksik olabilir.");
    }
    if (verification.problems.length < verification.problemCount) {
      warnings.push(
        `Sorunların yalnız ilk ${verification.problems.length} tanesi listeleniyor (toplam ${verification.problemCount}).`,
      );
    }
  }
  if (view.found && !view.tesseractOnPath) {
    warnings.push("Tesseract sistemde bulunamadı: OCR çalışmaz (paket Tesseract içermez).");
  }
  return warnings;
}

/** Arayüzün altındaki sınır cümlesi (tek yerde). */
export const PACKAGE_LIMITS =
  "Doğrulama bütünlük kanıtıdır, imza değildir: manifesti üreten süreçle doğrulayan süreç aynı güven sınırındadır. Tesseract ikilisi ve dil verisi pakete gömülmez; OCR için sistemde kurulu olmalı. Paket yalnız bu makinenin mimarisi için üretilir.";
