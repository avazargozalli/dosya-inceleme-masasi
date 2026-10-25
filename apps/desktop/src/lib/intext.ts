import type { InTextQuoteCheck, InTextQuoteFinding } from "./api";

/** Alıntı denetimi durum etiketleri. "Bulunamadı" uydurma hükmü değildir. */
export const QUOTE_STATE_LABEL: Record<string, string> = {
  exact: "Birebir · tek konum",
  normalized: "Birebir değil",
  ambiguous: "Birden fazla konum",
  not_found: "Bulunamadı",
  unverifiable: "Doğrulanamadı",
};

export const MATCH_KIND_LABEL: Record<string, string> = {
  exact: "birebir",
  whitespace: "boşluk farkı",
  turkish_fold: "harf farkı",
};

const TEXT_ORIGIN_LABEL: Record<string, string> = {
  native: "metin katmanı",
  ocr: "OCR",
  mixed: "OCR + metin",
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
  return [
    `${check.spansFound} alıntı`,
    `${check.exact} birebir`,
    `${check.normalized} birebir değil`,
    `${check.ambiguous} belirsiz`,
    `${check.notFound} bulunamadı`,
    `${check.unverifiable} doğrulanamadı`,
  ].join(" · ");
}

/** Yalnız tek konumda birebir bulunan alıntı kaynağa bağlanabilir. */
export function canBind(finding: InTextQuoteFinding): boolean {
  const position = finding.position;
  return (
    finding.state === "exact" &&
    position !== null &&
    position.matchKind === "exact" &&
    position.segmentId !== null &&
    position.charEnd > position.charStart
  );
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
  return `${position.documentName} · s. ${position.pageIndex + 1} · ${kind} · ${origin}`;
}

export function candidateLabel(candidate: {
  documentName: string;
  pageIndex: number;
  charStart: number;
  matchKind: string;
}): string {
  const kind = MATCH_KIND_LABEL[candidate.matchKind] ?? candidate.matchKind;
  return `${candidate.documentName} · s. ${candidate.pageIndex + 1} · karakter ${candidate.charStart} · ${kind}`;
}

export function skippedLabel(check: InTextQuoteCheck): string {
  const parts: string[] = [];
  if (check.skippedShort > 0) parts.push(`${check.skippedShort} kısa`);
  if (check.skippedLong > 0) parts.push(`${check.skippedLong} aşırı uzun`);
  if (check.unclosed > 0) parts.push(`${check.unclosed} kapanmamış`);
  return parts.length === 0 ? "" : `Sayılmayan: ${parts.join(", ")}`;
}

export function scopeLabel(check: InTextQuoteCheck): string {
  if (check.scope === "document" && check.scopeDocumentName) {
    return `Kapsam: yalnız “${check.scopeDocumentName}” belgesinin güncel sürümü`;
  }
  return `Kapsam: dosyadaki ${check.documentsWithText} belgenin güncel sürümü`;
}
