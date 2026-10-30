import { t } from "./i18n";
import type {
  CorpusCoverage,
  CorpusExportResult,
  PrecedentRow,
  PrecedentUsageTerms,
  Redistribution,
  UsageBasis,
} from "./api";

/**
 * F12-T11 — kaynak kullanım koşulu metinleri.
 *
 * Kural: kayıt yokluğu izin sayılmaz. `unknown` her yerde açıkça "bilinmiyor"
 * diye yazılır; hiçbir etiket "izinli" imasında bulunmaz.
 */

export function usageBasisLabel(basis: UsageBasis | string): string {
  switch (basis) {
    case "official_public":
      return t("terms.basisOfficialPublic");
    case "licence":
      return t("terms.basisLicence");
    case "permission":
      return t("terms.basisPermission");
    case "internal_only":
      return t("terms.basisInternalOnly");
    case "unknown":
      return t("terms.basisUnknown");
    default:
      return basis;
  }
}

export function redistributionLabel(value: Redistribution | string): string {
  switch (value) {
    case "allowed":
      return t("terms.redistributionAllowed");
    case "not_allowed":
      return t("terms.redistributionNotAllowed");
    default:
      return t("terms.redistributionUnknown");
  }
}

export function redistributionTone(value: Redistribution | string): "ok" | "danger" | "muted" {
  if (value === "allowed") return "ok";
  if (value === "not_allowed") return "danger";
  return "muted";
}

/** Liste rozeti: izin durumu tek kelimeyle; kayıt yoksa "izin bilinmiyor". */
export function termsBadge(row: Pick<PrecedentRow, "redistribution">): {
  text: string;
  tone: "ok" | "danger" | "warn";
} {
  return {
    text:
      row.redistribution === "allowed"
        ? t("terms.badgeAllowed")
        : row.redistribution === "not_allowed"
          ? t("terms.badgeNotAllowed")
          : t("terms.badgeUnknown"),
    // Kayıt yokluğu uyarıdır: sessizce "sorun yok" görünmez.
    tone:
      row.redistribution === "allowed"
        ? "ok"
        : row.redistribution === "not_allowed"
          ? "danger"
          : "warn",
  };
}

/** Koşul kaydının zamanı; kayıt yoksa bunu açıkça söyler. */
export function termsRecordedText(terms: PrecedentUsageTerms): string {
  if (!terms.termsRecordedAt) return t("terms.recordedNone");
  return t("terms.recordedAt", {
    at: terms.termsRecordedAt.slice(0, 16).replace("T", " "),
  });
}

/** Dağıtım tablosunun tek satırlık, gizlemesiz özeti. */
export function coverageText(coverage: CorpusCoverage): string {
  const parts = [
    t("terms.coverageTotal", { total: coverage.total }),
    t("terms.coverageWithText", { withText: coverage.withText }),
    t("terms.coverageAllowed", { allowed: coverage.allowed }),
  ];
  if (coverage.notAllowed > 0)
    parts.push(t("terms.coverageNotAllowed", { notAllowed: coverage.notAllowed }));
  parts.push(t("terms.coverageUnknown", { unknown: coverage.unknown }));
  return parts.join(" · ");
}

/**
 * "Tümü" kipi açık mı: metinli her kayıt izinli olmalı. Metinsiz kayıtlar
 * dağıtılacak metin taşımadığı için kapıyı kapatmaz.
 */
export function corpusGateOpen(coverage: CorpusCoverage): boolean {
  return coverage.withText > 0 && coverage.allowed === coverage.withText;
}

/** Kapının durumunu gerekçesiyle yazar; "izinli" görünen ama metinsiz kütüphane yoktur. */
export function corpusGateText(coverage: CorpusCoverage): string {
  if (coverage.withText === 0) {
    return t("terms.gateNoText");
  }
  if (corpusGateOpen(coverage)) {
    return t("terms.gateOpen");
  }
  return t("terms.gateBlocked", { blocked: coverage.blockedTotal });
}

export function exclusionReasonLabel(reason: string): string {
  return reason === "redistribution_not_allowed"
    ? t("terms.exclusionNotAllowed")
    : t("terms.exclusionUnknown");
}

/** Aktarım sonucu: yazılan dosya, sayılar ve içerik hash'i. */
export function corpusResultText(result: CorpusExportResult): string {
  const parts = [t("terms.resultExported", { exported: result.exported })];
  if (result.excluded > 0) parts.push(t("terms.resultExcluded", { excluded: result.excluded }));
  parts.push(t("terms.resultHash", { hash: result.sha256.slice(0, 12) }));
  return parts.join(" · ");
}

/**
 * "İzinli" işaretlemek için yazılı dayanak gerekip gerekmediği. Arayüz bunu
 * kaydetmeden önce gösterir; asıl kapı Rust tarafındadır.
 */
export function needsWrittenBasis(redistribution: Redistribution | string): boolean {
  return redistribution === "allowed";
}
