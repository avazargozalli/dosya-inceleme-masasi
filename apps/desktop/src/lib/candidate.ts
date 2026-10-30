import type { AiCandidateRow, CandidateReport, QuoteMatchReport, ValidationIssue } from "./api";
import { t } from "./i18n";

/**
 * F11-T05 — aday şema doğrulaması ve alıntı eşleştirmesi metinleri.
 *
 * Kural: doğrulama durumu **hesaplanır**; arayüz yalnız gösterir. Şeması
 * geçersiz aday `valid` gibi çizilmez, metinde bulunmayan alıntı kaynak
 * sayılmaz. Sayfa numaraları veride 0 tabanlıdır; insan için 1 tabanlı yazılır.
 *
 * Metinler F04-T08 gereği `lib/i18n.ts`ten gelir. `CANDIDATE_KINDS` modül
 * düzeyinde kurulduğu için `t()` **import anında** çözülür (sözlük `as const`,
 * tek dilli ürün kararı — dil değiştirme yok).
 */

export type CandidateKindOption = {
  value: string;
  label: string;
  hint: string;
  template: string;
};

function template(fields: Record<string, unknown>): string {
  return JSON.stringify({ schemaVersion: 1, ...fields }, null, 2);
}

export const CANDIDATE_KINDS: CandidateKindOption[] = [
  {
    value: "event_candidate",
    label: t("candidate.kind.event"),
    hint: t("candidate.kindHint.event"),
    template: template({
      title: t("candidate.sample.eventTitle"),
      date: t("candidate.sample.eventDate"),
      quotes: [],
    }),
  },
  {
    value: "person_candidate",
    label: t("candidate.kind.person"),
    hint: t("candidate.kindHint.person"),
    template: template({
      name: t("candidate.sample.personName"),
      role: t("candidate.sample.personRole"),
      quotes: [],
    }),
  },
  {
    value: "date_candidate",
    label: t("candidate.kind.date"),
    hint: t("candidate.kindHint.date"),
    template: template({
      rawText: t("candidate.sample.dateRaw"),
      kind: "document",
      isoDate: "2024-03-12",
      precision: "day",
      quotes: [],
    }),
  },
  {
    value: "claim_candidate",
    label: t("candidate.kind.claim"),
    hint: t("candidate.kindHint.claim"),
    template: template({ text: t("candidate.sample.claimText"), quotes: [] }),
  },
  {
    value: "summary_candidate",
    label: t("candidate.kind.summary"),
    hint: t("candidate.kindHint.summary"),
    template: template({ text: t("candidate.sample.summaryText"), quotes: [] }),
  },
  {
    value: "answer_candidate",
    label: t("candidate.kind.answer"),
    hint: t("candidate.kindHint.answer"),
    template: template({ text: t("candidate.sample.answerText"), quotes: [] }),
  },
];

export function candidateKindLabel(value: string): string {
  return CANDIDATE_KINDS.find((kind) => kind.value === value)?.label ?? value;
}

export function candidateStateLabel(state: string): string {
  switch (state) {
    case "valid":
      return t("candidate.state.valid");
    case "partial":
      return t("candidate.state.partial");
    case "invalid":
      return t("candidate.state.invalid");
    case "legacy":
      return t("candidate.state.legacy");
    default:
      return state;
  }
}

export function candidateStateTone(state: string): "ok" | "warn" | "danger" | "plain" {
  switch (state) {
    case "valid":
      return "ok";
    case "partial":
      return "warn";
    case "invalid":
      return "danger";
    default:
      return "plain";
  }
}

export function quoteStateLabel(state: string): string {
  switch (state) {
    case "verified":
      return t("candidate.quoteState.verified");
    case "partial":
      return t("candidate.quoteState.partial");
    case "failed":
      return t("candidate.quoteState.failed");
    case "no_quotes":
      return t("candidate.quoteState.noQuotes");
    case "not_checked":
      return t("candidate.quoteState.notChecked");
    case "legacy":
      return t("candidate.quoteState.legacy");
    default:
      return state;
  }
}

export function quoteStateTone(state: string): "ok" | "warn" | "danger" | "plain" {
  switch (state) {
    case "verified":
      return "ok";
    case "partial":
      return "warn";
    case "failed":
      return "danger";
    default:
      return "plain";
  }
}

export function quoteLineStateLabel(report: QuoteMatchReport): string {
  switch (report.state) {
    case "exact":
      return t("candidate.line.exact");
    case "normalized":
      return t("candidate.line.normalized");
    case "ambiguous":
      return t("candidate.line.ambiguous");
    case "page_mismatch":
      return t("candidate.line.pageMismatch");
    case "not_found":
      return t("candidate.line.notFound");
    case "unverifiable":
      switch (report.reason) {
        case "version_not_found":
          return t("candidate.unverifiableReason.versionNotFound");
        case "case_mismatch":
          return t("candidate.unverifiableReason.caseMismatch");
        case "no_text":
          return t("candidate.unverifiableReason.noText");
        default:
          return t("candidate.line.unverifiable");
      }
    default:
      return report.state;
  }
}

export function quoteLineTone(state: string): "ok" | "warn" | "danger" | "plain" {
  switch (state) {
    case "exact":
      return "ok";
    case "normalized":
    case "ambiguous":
    case "page_mismatch":
    case "unverifiable":
      return "warn";
    case "not_found":
      return "danger";
    default:
      return "plain";
  }
}

/** Alıntının insan okur konumu; konum bilinmiyorsa "konum yok". */
export function quotePositionLabel(report: QuoteMatchReport): string {
  if (report.pageIndex === null) {
    return t("candidate.position.missing");
  }
  const page = report.pageIndex + 1;
  if (report.charStart === null) {
    return t("candidate.position.page", { page });
  }
  return t("candidate.position.pageChar", { page, char: report.charStart });
}

export function quoteSummary(report: CandidateReport): string {
  if (report.quotes.length === 0) {
    return t("candidate.summary.none");
  }
  const exact = report.quotes.filter((quote) => quote.state === "exact").length;
  return t("candidate.summary.exact", { exact, total: report.quotes.length });
}

/** `validation_errors` metni bozuk/boş olsa da arayüz çökmez. */
export function parseIssues(raw: string): ValidationIssue[] {
  if (!raw.trim()) {
    return [];
  }
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed.filter((item): item is ValidationIssue => {
      if (typeof item !== "object" || item === null) {
        return false;
      }
      const record = item as Record<string, unknown>;
      return (
        typeof record.code === "string" &&
        typeof record.severity === "string" &&
        typeof record.field === "string" &&
        typeof record.detail === "string"
      );
    });
  } catch {
    return [];
  }
}

export function errorCount(issues: ValidationIssue[]): number {
  return issues.filter((issue) => issue.severity === "error").length;
}

export function warningCount(issues: ValidationIssue[]): number {
  return issues.filter((issue) => issue.severity === "warning").length;
}

/** Taslak JSON'ı göndermeden önce yerel biçim denetimi (şema değil). */
export function draftJsonError(text: string): string | null {
  if (!text.trim()) {
    return t("candidate.draft.empty");
  }
  try {
    const parsed: unknown = JSON.parse(text);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      return t("candidate.draft.notObject");
    }
    return null;
  } catch (error) {
    return t("candidate.draft.unparsable", {
      message: error instanceof Error ? error.message : t("candidate.draft.unknownError"),
    });
  }
}

/**
 * İnceleme kuyruğuna gönderme kapısı. Depodaki kapıyla aynı gerekçeyi yazar;
 * burada yalnız düğmeyi kapalı tutmak için kullanılır.
 */
export function promoteBlockedReason(
  row: Pick<AiCandidateRow, "validationState" | "quoteState" | "schemaState">,
): string | null {
  if (row.schemaState === "legacy" || row.quoteState === "legacy") {
    return t("candidate.blocked.legacy");
  }
  if (row.quoteState === "failed") {
    return t("candidate.blocked.quoteFailed");
  }
  if (row.validationState === "invalid") {
    return t("candidate.blocked.invalidSchema");
  }
  return null;
}
