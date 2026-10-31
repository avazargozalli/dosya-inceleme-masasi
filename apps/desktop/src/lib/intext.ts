import type { InTextQuoteCheck, InTextQuoteFinding } from "./api";
import { t } from "./i18n";

/** Alıntı denetimi durum etiketleri. "Bulunamadı" uydurma hükmü değildir. */
export const QUOTE_STATE_LABEL: Record<string, string> = {
  exact: t("intext.stateExact"),
  normalized: t("intext.stateNormalized"),
  ambiguous: t("intext.stateAmbiguous"),
  not_found: t("intext.stateNotFound"),
  unverifiable: t("intext.stateUnverifiable"),
};

export const MATCH_KIND_LABEL: Record<string, string> = {
  exact: t("intext.matchExact"),
  whitespace: t("intext.matchWhitespace"),
  ligature: t("intext.matchLigature"),
  turkish_fold: t("intext.matchTurkishFold"),
  dehyphenated: t("intext.matchDehyphenated"),
};

const TEXT_ORIGIN_LABEL: Record<string, string> = {
  native: t("intext.originNative"),
  ocr: t("intext.originOcr"),
  mixed: t("intext.originMixed"),
};

export function quoteStateTone(state: string): "ok" | "warn" | "danger" | "plain" {
  switch (state) {
    case "exact":
      return "ok";
    case "normalized":
    case "ambiguous":
      return "warn";
    case "unverifiable":
      return "danger";
    default:
      return "plain";
  }
}

export function quoteCheckSummary(check: InTextQuoteCheck): string {
  const parts = [
    t("intext.summaryQuotes", { count: check.spansFound }),
    t("intext.summaryExact", { count: check.exact }),
    t("intext.summaryNormalized", { count: check.normalized }),
    t("intext.summaryAmbiguous", { count: check.ambiguous }),
    t("intext.summaryNotFound", { count: check.notFound }),
    t("intext.summaryUnverifiable", { count: check.unverifiable }),
  ];
  if (check.dehyphenated > 0) {
    parts.push(t("intext.summaryDehyphenated", { count: check.dehyphenated }));
  }
  if (check.ligature > 0) {
    parts.push(t("intext.summaryLigature", { count: check.ligature }));
  }
  return parts.join(" · ");
}

/** Yalnız tek konumda birebir bulunan alıntı kaynağa bağlanabilir. Sayfa
 * sınırını aşan eşleşme tek sayfalık anchor olarak ifade edilemez. */
export function canBind(finding: InTextQuoteFinding): boolean {
  const position = finding.position;
  return (
    finding.state === "exact" &&
    position !== null &&
    position.matchKind === "exact" &&
    !position.spansPages &&
    position.segmentId !== null &&
    position.charEnd > position.charStart
  );
}

/** Sayfa aralığı etiketi: `s. 3` ya da `s. 3–4`. */
export function pageSpanLabel(pageIndex: number, endPageIndex: number): string {
  return endPageIndex > pageIndex
    ? t("intext.pageSpanRange", { page: pageIndex + 1, end: endPageIndex + 1 })
    : t("intext.pageSpanSingle", { page: pageIndex + 1 });
}

/** Bağlanamayan alıntı için nedeni yazar; bağlanabilende boştur. */
export function bindHint(finding: InTextQuoteFinding): string {
  if (canBind(finding)) return "";
  if (finding.state === "exact" && finding.position?.spansPages) {
    return t("intext.bindSpansPages");
  }
  if (
    finding.position?.matchKind === "ligature" ||
    finding.candidates.some((candidate) => candidate.matchKind === "ligature")
  ) {
    return t("intext.bindLigature");
  }
  if (
    finding.position?.matchKind === "dehyphenated" ||
    finding.candidates.some((candidate) => candidate.matchKind === "dehyphenated")
  ) {
    return t("intext.bindDehyphenated");
  }
  return t("intext.bindOnlyExact");
}

export function bindingInput(caseId: string, finding: InTextQuoteFinding) {
  if (!canBind(finding) || finding.position === null) return null;
  const position = finding.position;
  return {
    caseId,
    documentVersionId: position.documentVersionId,
    parseRunId: position.parseRunId,
    segmentId: position.segmentId as string,
    startOffset: position.charStart,
    endOffset: position.charEnd,
    exactQuote: position.sourceText,
  };
}

export function positionLabel(finding: InTextQuoteFinding): string {
  const position = finding.position ?? finding.candidates[0] ?? null;
  if (!position) return "";
  const origin = TEXT_ORIGIN_LABEL[position.textOrigin] ?? position.textOrigin;
  const kind = MATCH_KIND_LABEL[position.matchKind] ?? position.matchKind;
  const pages = pageSpanLabel(position.pageIndex, position.endPageIndex);
  const parts = [position.documentName, pages, kind, origin];
  if (position.spansPages) parts.push(t("intext.spansPages"));
  if (position.isCurrentVersion === false) {
    parts.push(t("intext.oldVersionSuffix", { version: position.versionNo }));
  }
  return parts.join(" · ");
}

/** Eski sürümde bulunan alıntı için bağlama uyarısı; güncel sürümde boştur. */
export function oldVersionNote(finding: InTextQuoteFinding): string {
  const position = finding.position;
  if (position) {
    if (position.isCurrentVersion !== false) return "";
    if (!canBind(finding)) {
      return t("intext.oldOnlyFound", { version: position.versionNo });
    }
    return t("intext.oldOnlyBinds", { version: position.versionNo });
  }
  // Belirsiz eşleşmede tek konum yoktur; ama adayların **hepsi** eski sürümdeyse
  // “alıntı güncel metinde geçmiyor” hükmü yine verilmelidir — aksi hâlde en
  // önemli olgu yalnız katlanmış “denetimin sınırları” notunda kalır.
  const candidates = finding.candidates;
  if (candidates.length === 0) return "";
  if (candidates.some((candidate) => candidate.isCurrentVersion !== false)) return "";
  const versions = [...new Set(candidates.map((candidate) => candidate.versionNo))].sort(
    (left, right) => right - left,
  );
  const where =
    versions.length === 1
      ? t("intext.oldWhereSingle", { version: versions[0] ?? 0 })
      : t("intext.oldWhereMany", {
          versions: versions.map((version) => `v${version}`).join(", "),
        });
  return t("intext.oldOnlyCandidates", { where, occurrences: finding.occurrences });
}

/** Taranan sürüm sayısı satırı; hiç sürüm yoksa boştur. */
export function versionsLabel(check: InTextQuoteCheck): string {
  if (!check.versionsScanned) return "";
  const parts = [t("intext.versionsScanned", { count: check.versionsScanned })];
  if (check.historicalVersionsScanned) {
    parts.push(t("intext.historicalVersions", { count: check.historicalVersionsScanned }));
  }
  return parts.join(" · ");
}

export function candidateLabel(candidate: {
  documentName: string;
  pageIndex: number;
  endPageIndex?: number;
  spansPages?: boolean;
  charStart: number;
  matchKind: string;
  versionNo?: number;
  isCurrentVersion?: boolean;
}): string {
  const kind = MATCH_KIND_LABEL[candidate.matchKind] ?? candidate.matchKind;
  const pages = pageSpanLabel(candidate.pageIndex, candidate.endPageIndex ?? candidate.pageIndex);
  const parts = [
    candidate.documentName,
    pages,
    t("intext.charAt", { char: candidate.charStart }),
    kind,
  ];
  if (candidate.spansPages) parts.push(t("intext.spansPages"));
  if (candidate.isCurrentVersion === false) {
    parts.push(
      candidate.versionNo === undefined
        ? t("intext.oldVersionUnknown")
        : t("intext.oldVersionSuffix", { version: candidate.versionNo }),
    );
  }
  return parts.join(" · ");
}

export function skippedLabel(check: InTextQuoteCheck): string {
  const parts: string[] = [];
  if (check.skippedShort > 0) parts.push(t("intext.skippedShort", { count: check.skippedShort }));
  if (check.skippedLong > 0) parts.push(t("intext.skippedLong", { count: check.skippedLong }));
  if (check.unclosed > 0) parts.push(t("intext.skippedUnclosed", { count: check.unclosed }));
  return parts.length === 0 ? "" : t("intext.notCounted", { parts: parts.join(", ") });
}

export function scopeLabel(check: InTextQuoteCheck): string {
  if (check.scope === "document" && check.scopeDocumentName) {
    return t("intext.scopeDocument", { name: check.scopeDocumentName });
  }
  return t("intext.scopeCase", { count: check.documentsWithText });
}
