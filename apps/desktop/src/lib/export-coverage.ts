import type { ExportCoverage } from "./api";
import { t } from "./i18n";

/**
 * F11-T06 — kapsam bloğunun arayüz tarafı.
 *
 * Sayılar çekirdekten gelir (`case_coverage` / dışa aktarma yanıtı); buradaki
 * yardımcılar **yeniden hesaplamaz**, yalnız okunur biçime çevirir. Amaç
 * ekranda da tek cümleyi görünür tutmak: “incelenmedi ≠ temiz”.
 *
 * Metinler F04-T08 gereği `lib/i18n.ts`ten gelir.
 */

export type CoverageTone = "ok" | "warn" | "plain";

export type CoverageRow = {
  label: string;
  value: number;
  /** Sıfırdan farkı uyarı gerektiren satırlar (boş sayfa, ayrıştırılmamış…). */
  tone: CoverageTone;
};

export type CoverageState = {
  label: string;
  tone: CoverageTone;
  detail: string;
};

/** Rapor tablosunun satırları; sıra çekirdeğin `rows()` çıktısıyla aynıdır. */
export function coverageRows(coverage: ExportCoverage): CoverageRow[] {
  return [
    {
      label: t("exportCoverage.row.documents"),
      value: coverage.documents,
      tone: coverage.documents === 0 ? "warn" : "plain",
    },
    { label: t("exportCoverage.row.versions"), value: coverage.versions, tone: "plain" },
    { label: t("exportCoverage.row.parseRuns"), value: coverage.parseRuns, tone: "plain" },
    { label: t("exportCoverage.row.pages"), value: coverage.pages, tone: "plain" },
    { label: t("exportCoverage.row.textPages"), value: coverage.textPages, tone: "plain" },
    {
      label: t("exportCoverage.row.ocrPages"),
      value: coverage.ocrPages,
      tone: coverage.ocrPages > 0 ? "warn" : "plain",
    },
    {
      label: t("exportCoverage.row.emptyPages"),
      value: coverage.emptyPages,
      tone: coverage.emptyPages > 0 ? "warn" : "plain",
    },
    { label: t("exportCoverage.row.segments"), value: coverage.segments, tone: "plain" },
    {
      label: t("exportCoverage.row.unparsed"),
      value: coverage.unparsedDocuments,
      tone: coverage.unparsedDocuments > 0 ? "warn" : "plain",
    },
    {
      label: t("exportCoverage.row.failedRuns"),
      value: coverage.failedParseRuns,
      tone: coverage.failedParseRuns > 0 ? "warn" : "plain",
    },
  ];
}

/** Tek satırlık özet (çekirdeğin `headline()` metniyle aynı biçim). */
export function coverageHeadline(coverage: ExportCoverage): string {
  return t("exportCoverage.headline", {
    documents: coverage.documents,
    pages: coverage.pages,
    textPages: coverage.textPages,
    ocrPages: coverage.ocrPages,
    emptyPages: coverage.emptyPages,
    unparsed: coverage.unparsedDocuments,
  });
}

/** Kapsam durumu rozeti. `complete=false` “temiz” anlamına gelmez. */
export function coverageState(coverage: ExportCoverage): CoverageState {
  if (coverage.documents === 0) {
    return {
      label: t("exportCoverage.state.none"),
      tone: "warn",
      detail: t("exportCoverage.stateDetail.none"),
    };
  }
  if (coverage.complete) {
    return {
      label: t("exportCoverage.state.full"),
      tone: "ok",
      detail: t("exportCoverage.stateDetail.full"),
    };
  }
  return {
    label: t("exportCoverage.state.partial"),
    tone: "warn",
    detail: t("exportCoverage.stateDetail.partial", { count: coverage.warnings.length }),
  };
}

/** Uyarılar çekirdekten gelir; arayüz kendi cümlesini uydurmaz. */
export function coverageWarnings(coverage: ExportCoverage): string[] {
  return coverage.warnings;
}

/** Kapsamın gömüldüğü çıktı türleri (arayüzde tek yerde yazılır). */
export const COVERAGE_EMBED_TARGETS: Array<{ id: string; label: string; where: string }> = [
  { id: "json", label: t("exportCoverage.embedTarget.json"), where: t("exportCoverage.embedWhere.json") },
  { id: "csv", label: t("exportCoverage.embedTarget.csv"), where: t("exportCoverage.embedWhere.csv") },
  { id: "docx", label: t("exportCoverage.embedTarget.docx"), where: t("exportCoverage.embedWhere.docx") },
  { id: "pdf", label: t("exportCoverage.embedTarget.pdf"), where: t("exportCoverage.embedWhere.pdf") },
  {
    id: "package",
    label: t("exportCoverage.embedTarget.package"),
    where: t("exportCoverage.embedWhere.package"),
  },
];

/** Dışa aktarma sonrası “ne yazıldı” cümlesi. */
export function coverageEmbedNote(format: string, coverage: ExportCoverage): string {
  const target = COVERAGE_EMBED_TARGETS.find((entry) => entry.id === format);
  const headline = coverageHeadline(coverage);
  return target
    ? t("exportCoverage.embedNoteWhere", { where: target.where, headline })
    : t("exportCoverage.embedNote", { headline });
}

/** Uyarı satırı var mı? (rozet rengi için) */
export function hasCoverageGaps(coverage: ExportCoverage): boolean {
  return !coverage.complete || coverage.warnings.length > 0;
}

/** Kapsamı olmayan (belgesiz) dosya için dışa aktarma uyarısı. */
export function emptyCoverageWarning(coverage: ExportCoverage): string | null {
  if (coverage.documents > 0) return null;
  return t("exportCoverage.emptyWarning");
}
