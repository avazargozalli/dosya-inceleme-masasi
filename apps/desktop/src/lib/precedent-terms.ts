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
      return "resmî yayın (kamuya açık)";
    case "licence":
      return "açık lisans";
    case "permission":
      return "yazılı izin";
    case "internal_only":
      return "yalnız iç kullanım";
    case "unknown":
      return "bilinmiyor";
    default:
      return basis;
  }
}

export function redistributionLabel(value: Redistribution | string): string {
  switch (value) {
    case "allowed":
      return "yeniden dağıtım izinli";
    case "not_allowed":
      return "yeniden dağıtım izinsiz";
    default:
      return "izin bilinmiyor";
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
        ? "izinli"
        : row.redistribution === "not_allowed"
          ? "izinsiz"
          : "izin bilinmiyor",
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
  if (!terms.termsRecordedAt) return "koşul kaydı yok — izin varsayılmaz";
  return `koşul kaydı: ${terms.termsRecordedAt.slice(0, 16).replace("T", " ")}`;
}

/** Dağıtım tablosunun tek satırlık, gizlemesiz özeti. */
export function coverageText(coverage: CorpusCoverage): string {
  const parts = [
    `${coverage.total} karar`,
    `${coverage.withText} tam metinli`,
    `${coverage.allowed} izinli`,
  ];
  if (coverage.notAllowed > 0) parts.push(`${coverage.notAllowed} izinsiz`);
  parts.push(`${coverage.unknown} izin bilinmiyor`);
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
    return "Tam metinli karar yok; aktarılacak korpus oluşmaz.";
  }
  if (corpusGateOpen(coverage)) {
    return "Tam metinli tüm kararlar için izin kaydı var; toplu korpus aktarılabilir.";
  }
  return `${coverage.blockedTotal} tam metinli kayıtta yeniden dağıtım izni yok; “tümü” kipi kapalı, yalnız izinli kararlar aktarılabilir.`;
}

export function exclusionReasonLabel(reason: string): string {
  return reason === "redistribution_not_allowed"
    ? "yeniden dağıtım izinsiz"
    : "izin kaydı yok";
}

/** Aktarım sonucu: yazılan dosya, sayılar ve içerik hash'i. */
export function corpusResultText(result: CorpusExportResult): string {
  const excluded = result.excluded > 0 ? ` · ${result.excluded} kayıt izin nedeniyle dışarıda` : "";
  return `${result.exported} karar aktarıldı${excluded} · sha256 ${result.sha256.slice(0, 12)}…`;
}

/**
 * "İzinli" işaretlemek için yazılı dayanak gerekip gerekmediği. Arayüz bunu
 * kaydetmeden önce gösterir; asıl kapı Rust tarafındadır.
 */
export function needsWrittenBasis(redistribution: Redistribution | string): boolean {
  return redistribution === "allowed";
}
