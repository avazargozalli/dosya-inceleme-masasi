import type { PrecedentVersionRow, QuoteVerificationRow } from "./api";
import { formatNumber } from "./format";
import { t } from "./i18n";

/** Tam metin sürümünün kısa durumu. Metin yoksa "yok" denir; tahmin yok. */
export function versionStateText(version: {
  hasText: boolean;
  charCount: number;
}): string {
  if (!version.hasText) return t("precedentLib.versionNoText");
  return t("precedentLib.versionText", { characters: formatNumber(version.charCount) });
}

/** Alıntı doğrulamasının tek cümlelik, dürüst özeti. "Bulunamadı" asla
 *  "uydurulmuş" diye çevrilmez; kısa alıntı doğrulanmış sayılmaz. */
export function quoteStateText(verification: QuoteVerificationRow): string {
  const at =
    verification.matchedStart === null
      ? ""
      : ` ${t("precedentLib.quoteAt", {
          position: formatNumber(verification.matchedStart + 1),
        })}`;
  const repeated =
    verification.occurrences > 1
      ? ` · ${t("precedentLib.quoteRepeated", { count: verification.occurrences })}`
      : "";
  // Eski sürümde bulunan alıntı gizlenmez: güncel metinde geçmiyor.
  const oldNote = verification.foundInOldVersion
    ? ` ${t("precedentLib.quoteOldSuffix")}`
    : "";
  const where = verification.foundInOldVersion
    ? t("precedentLib.quoteWhereOld")
    : t("precedentLib.quoteWhereCurrent");
  switch (verification.state) {
    case "match": {
      const parts = { where, at, repeated, old: oldNote };
      if (verification.method === "exact") return t("precedentLib.quoteExact", parts);
      if (verification.method === "whitespace")
        return t("precedentLib.quoteWhitespace", parts);
      if (verification.method === "dehyphenated")
        return t("precedentLib.quoteDehyphenated", parts);
      if (verification.method === "ligature") return t("precedentLib.quoteLigature", parts);
      return t("precedentLib.quoteTurkishFold", parts);
    }
    case "mismatch": {
      const scope =
        verification.historicalVersionsScanned > 0
          ? t("precedentLib.quoteMismatchHistorical", {
              versions: verification.historicalVersionsScanned,
            })
          : t("precedentLib.quoteMismatchCurrent");
      const cut = verification.historicalTruncated
        ? ` ${t("precedentLib.quoteMismatchTruncated")}`
        : "";
      return t("precedentLib.quoteMismatch", { scope, cut });
    }
    default:
      if (verification.reason === "quote_too_short") return t("precedentLib.quoteTooShort");
      return t("precedentLib.quoteNoText");
  }
}

/** Kaç sürümün tarandığını söyleyen tek satır; sayı çekirdekten gelir. */
export function quoteScanNote(verification: QuoteVerificationRow): string | null {
  if (verification.state === "not_checked") return null;
  const parts = [t("precedentLib.scanVersions", { count: verification.versionsScanned })];
  if (verification.historicalVersionsScanned > 0)
    parts.push(
      t("precedentLib.scanHistorical", { count: verification.historicalVersionsScanned }),
    );
  if (verification.historicalTruncated) parts.push(t("precedentLib.scanTruncated"));
  return parts.join(" · ");
}

export function quoteStateTone(
  verification: QuoteVerificationRow,
): "ok" | "warn" | "muted" {
  if (verification.state === "match") {
    // Eski sürümdeki birebir eşleşme bugünkü metni doğrulamaz → uyarı tonu.
    if (verification.foundInOldVersion) return "warn";
    return verification.method === "exact" ? "ok" : "warn";
  }
  if (verification.state === "mismatch") return "warn";
  return "muted";
}

/** Doğrulama sonucunu atıf kontrolü boyutuna taşır; boyutlar ayrı kalır. */
export function quoteToCitationMatch(
  verification: QuoteVerificationRow,
): "match" | "mismatch" | "not_checked" {
  if (verification.state === "match") return "match";
  if (verification.state === "mismatch") return "mismatch";
  return "not_checked";
}

/** Sürüm rozeti: metin yoksa "metin yok", varsa kaynak türü. */
export function versionSourceLabel(version: { sourceKind: string }): string {
  switch (version.sourceKind) {
    case "paste":
      return t("precedentLib.sourcePaste");
    case "document":
      return t("precedentLib.sourceDocument");
    case "file":
      return t("precedentLib.sourceFile");
    default:
      return version.sourceKind;
  }
}

export type PrecedentVersionSummary = {
  versions: PrecedentVersionRow[];
  withText: number;
  latestHasText: boolean;
};

/** Liste özeti: kaç sürümde tam metin var, en yenisi metinli mi. */
export function summarizeVersions(versions: PrecedentVersionRow[]): PrecedentVersionSummary {
  const withText = versions.filter((v) => v.hasText).length;
  const latest = versions[0];
  return { versions, withText, latestHasText: latest ? latest.hasText : false };
}

/** Toplu içe alma sonucunun dosya başına dürüst etiketi (F12-T06). */
export function bulkOutcomeLabel(item: {
  outcome: string;
  code: string | null;
  message: string | null;
  charCount: number;
}): string {
  switch (item.outcome) {
    case "created":
      return t("precedentLib.bulkCreated", { characters: formatNumber(item.charCount) });
    case "duplicate":
      return t("precedentLib.bulkDuplicate");
    case "conflict":
      return t("precedentLib.bulkConflict");
    default: {
      const code = item.code ? `${item.code}: ` : "";
      return `${code}${item.message ?? t("precedentLib.bulkFailed")}`;
    }
  }
}

export function bulkOutcomeTone(item: { outcome: string }): "ok" | "warn" | "danger" {
  if (item.outcome === "created") return "ok";
  if (item.outcome === "duplicate") return "ok";
  if (item.outcome === "conflict") return "warn";
  return "danger";
}

/** Tek cümlelik toplu içe alma özeti; sayılar raporun kendisinden gelir. */
export function bulkReportSummary(report: {
  created: number;
  duplicates: number;
  conflicts: number;
  failed: number;
}): string {
  const parts = [
    t("precedentLib.bulkReportCreated", { count: report.created }),
    t("precedentLib.bulkReportDuplicates", { count: report.duplicates }),
    t("precedentLib.bulkReportConflicts", { count: report.conflicts }),
  ];
  if (report.failed > 0)
    parts.push(t("precedentLib.bulkReportFailed", { count: report.failed }));
  return parts.join(" · ");
}

/** Sürüm rozeti: künye çakışması varsa metin durumundan önce gelir. */
export function versionConflictText(version: {
  identityConflict: boolean;
}): string | null {
  return version.identityConflict ? t("precedentLib.conflictText") : null;
}

/** Eksik künye alanları tek satırda; eksik yoksa "künye tam". */
export function bibliographyMissingText(missing: string[]): string {
  return missing.length === 0
    ? t("precedentLib.bibliographyComplete")
    : t("precedentLib.bibliographyMissing", { fields: missing.join(", ") });
}

/**
 * Erişim tarihinin nereden geldiğini söyler. Tam metin varsa edinim zamanı,
 * yoksa künye kaydının oluşturulma zamanı kullanılır — bu ayrım gizlenmez.
 */
export function accessBasisLabel(basis: string): string {
  return basis === "version"
    ? t("precedentLib.accessBasisVersion")
    : t("precedentLib.accessBasisRecord");
}
