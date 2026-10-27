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
    ? `s. ${pageIndex + 1}–${endPageIndex + 1}`
    : `s. ${pageIndex + 1}`;
}

/** Bağlanamayan alıntı için nedeni yazar; bağlanabilende boştur. */
export function bindHint(finding: InTextQuoteFinding): string {
  if (canBind(finding)) return "";
  if (finding.state === "exact" && finding.position?.spansPages) {
    return "Alıntı sayfa sınırını aşıyor; tek sayfalık anchor'a bağlanmaz.";
  }
  return "Yalnız birebir tek konum kaynağa bağlanır.";
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
  const span = position.spansPages ? " · sayfa sınırını aşıyor" : "";
  const old = position.isCurrentVersion === false
    ? ` · eski sürüm (v${position.versionNo})`
    : "";
  return `${position.documentName} · ${pages} · ${kind} · ${origin}${span}${old}`;
}

/** Eski sürümde bulunan alıntı için bağlama uyarısı; güncel sürümde boştur. */
export function oldVersionNote(finding: InTextQuoteFinding): string {
  const position = finding.position;
  if (!position || position.isCurrentVersion !== false) return "";
  if (!canBind(finding)) {
    return `Alıntı güncel sürümde geçmiyor; yalnız v${position.versionNo} sürümünde bulundu.`;
  }
  return `Kaynak v${position.versionNo} sürümüne bağlanır; alıntı güncel sürümde geçmiyor.`;
}

/** Taranan sürüm sayısı satırı; hiç sürüm yoksa boştur. */
export function versionsLabel(check: InTextQuoteCheck): string {
  if (!check.versionsScanned) return "";
  const historical = check.historicalVersionsScanned
    ? ` · ${check.historicalVersionsScanned} eski sürüm`
    : "";
  return `${check.versionsScanned} sürüm tarandı${historical}`;
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
  const span = candidate.spansPages ? " · sayfa sınırını aşıyor" : "";
  const old = candidate.isCurrentVersion === false
    ? ` · eski sürüm (v${candidate.versionNo ?? "?"})`
    : "";
  return `${candidate.documentName} · ${pages} · karakter ${candidate.charStart} · ${kind}${span}${old}`;
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
