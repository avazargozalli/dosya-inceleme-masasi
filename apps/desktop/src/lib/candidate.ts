import type {
  AiCandidateRow,
  CandidateReport,
  ContradictionBasis,
  QuoteMatchReport,
  SentenceAttribution,
  ValidationIssue,
} from "./api";
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
  {
    value: "contradiction_candidate",
    label: t("candidate.kind.contradiction"),
    hint: t("candidate.kindHint.contradiction"),
    template: template({
      text: t("candidate.sample.contradictionText"),
      quotes: [],
      sides: [
        { quoteIndex: 0, dateKind: "event", dateText: t("candidate.sample.contradictionDateA") },
        { quoteIndex: 1, dateKind: "event", dateText: t("candidate.sample.contradictionDateB") },
      ],
    }),
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
  row: Pick<AiCandidateRow, "validationState" | "quoteState" | "schemaState"> & {
    candidateType?: string;
    contradictionBasis?: ContradictionBasis | null;
  },
  answerLimited = false,
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
  // F11-T08 — dayanağı doğrulanmamış yanıt inceleme kuyruğuna gönderilmez;
  // kapı çekirdekte (`E_AI_ANSWER_LIMITED`), burada yalnız nedeni yazılır.
  if (answerLimited) {
    return t("candidate.blocked.answerLimited");
  }
  // F11-T09 — çelişki **iki doğrulanmış kaynakla** kuyruğa girer; tarih rolü
  // farkı otomatik çelişki sayılmaz. Kapı çekirdekte (`E_AI_CONTRADICTION`).
  if (isContradictionType(row.candidateType) && row.contradictionBasis) {
    switch (row.contradictionBasis.state) {
      case "role_difference":
        return t("candidate.blocked.contradictionRoleDifference");
      case "sources_unverified":
        return t("candidate.blocked.contradictionSourcesUnverified");
      case "unlinked":
        return t("candidate.blocked.contradictionUnlinked");
      default:
        break;
    }
  }
  return null;
}

/** Çelişki adayı türü (`contradiction` ya da `contradiction_candidate`). */
export function isContradictionType(value: string | undefined): boolean {
  return value === "contradiction" || value === "contradiction_candidate";
}

/**
 * F11-T09 — çelişki dayanağı rozeti. `presentable` **çelişki hükmü değildir**:
 * iki kaynağın birebir doğrulandığını söyler; hüküm avukatındır. Tanınmayan
 * durum ham yazılır — ekran bilmediği değeri "çelişki var" saymaz.
 */
export function contradictionStateTone(state: string): "ok" | "warn" | "danger" | "plain" {
  if (state === "presentable") return "ok";
  if (state === "role_difference" || state === "sources_unverified") return "warn";
  if (state === "unlinked") return "danger";
  return "plain";
}

export function contradictionStateLabel(state: string): string {
  switch (state) {
    case "presentable":
      return t("candidate.contradiction.presentable");
    case "role_difference":
      return t("candidate.contradiction.roleDifference");
    case "sources_unverified":
      return t("candidate.contradiction.sourcesUnverified");
    case "unlinked":
      return t("candidate.contradiction.unlinked");
    default:
      return state;
  }
}

export function contradictionReasonLabel(code: string): string {
  switch (code) {
    case "date_roles_differ":
      return t("candidate.contradictionReason.dateRolesDiffer");
    case "side_missing":
      return t("candidate.contradictionReason.sideMissing");
    case "side_not_exact":
      return t("candidate.contradictionReason.sideNotExact");
    case "sides_missing":
      return t("candidate.contradictionReason.sidesMissing");
    case "sides_not_two":
      return t("candidate.contradictionReason.sidesNotTwo");
    default:
      return code;
  }
}

/** Tarafların bildirdiği tarihin **rolü**; rol yoksa null (uydurulmaz). */
export function contradictionDateRoleLabel(kind: string | null): string | null {
  switch (kind) {
    case "document":
      return t("candidate.dateRole.document");
    case "event":
      return t("candidate.dateRole.event");
    case "notification":
      return t("candidate.dateRole.notification");
    case "hearing":
      return t("candidate.dateRole.hearing");
    case "unknown":
      return t("candidate.dateRole.unknown");
    default:
      return kind;
  }
}

/** Bir tarafın alıntı durumu; `missing` tarafın alıntısı hiç ölçülmedi. */
export function contradictionSideStateLabel(state: string): string {
  switch (state) {
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
      return t("candidate.line.unverifiable");
    case "missing":
      return t("candidate.contradiction.quoteMissing");
    default:
      return state;
  }
}

/** Soru-cevap dayanak durumu: `sufficient` · `limited`. */
export function answerBasisTone(state: string): "ok" | "warn" | "plain" {
  if (state === "sufficient") return "ok";
  if (state === "limited") return "warn";
  return "plain";
}

export function answerBasisStateLabel(state: string): string {
  switch (state) {
    case "sufficient":
      return t("candidate.answerBasis.sufficient");
    case "limited":
      return t("candidate.answerBasis.limited");
    default:
      return state;
  }
}

/** Sınır nedenleri kararlı kod taşır; ekran bilinenleri Türkçeleştirir. */
export function answerBasisReasonLabel(code: string): string {
  switch (code) {
    case "no_scope":
      return t("candidate.answerReason.noScope");
    case "no_answer":
      return t("candidate.answerReason.noAnswer");
    case "answer_invalid":
      return t("candidate.answerReason.answerInvalid");
    case "no_quotes":
      return t("candidate.answerReason.noQuotes");
    case "quotes_not_found":
      return t("candidate.answerReason.quotesNotFound");
    default:
      return code;
  }
}

/**
 * F11-T07 — özet cümlesi rozeti. `backed` birebir/normalize bulunmuş alıntıya
 * dayanır; `weak` yalnız belirsiz/başka sayfa eşleşmesidir (varlık doğrulandı,
 * konum tek değil); `unsourced` **kaynaksız yorum**dur. Bilinmeyen durum ham
 * yazılır — ekran tanımadığı değeri "dayanaklı" saymaz.
 */
export function sentenceStateTone(state: string): "ok" | "warn" | "danger" | "plain" {
  if (state === "backed") return "ok";
  if (state === "weak") return "warn";
  if (state === "unsourced") return "danger";
  return "plain";
}

export function sentenceStateLabel(state: string): string {
  switch (state) {
    case "backed":
      return t("candidate.sentence.backed");
    case "weak":
      return t("candidate.sentence.weak");
    case "unsourced":
      return t("candidate.sentence.unsourced");
    default:
      return state;
  }
}

/** Cümle bağının bütünü: `linked` · `partial` · `unlinked`. */
export function attributionStateTone(state: string): "ok" | "warn" | "plain" {
  if (state === "linked") return "ok";
  if (state === "partial") return "warn";
  return "plain";
}

export function attributionStateLabel(state: string): string {
  switch (state) {
    case "linked":
      return t("candidate.attribution.linked");
    case "partial":
      return t("candidate.attribution.partial");
    case "unlinked":
      return t("candidate.attribution.unlinked");
    default:
      return state;
  }
}

/** «3 cümle · 2 dayanaklı · 0 zayıf · 1 kaynaksız» */
export function attributionSummary(attribution: SentenceAttribution): string {
  return t("candidate.attribution.counts", {
    sentences: attribution.sentences.length,
    backed: attribution.backed,
    weak: attribution.weak,
    unsourced: attribution.unsourced,
  });
}

/** Cümleyi dayanak gösteren alıntıların **1 tabanlı** listesi. */
export function sentenceQuoteRefs(quoteIndexes: number[]): string {
  if (quoteIndexes.length === 0) return "";
  return t("candidate.attribution.quoteRefs", {
    list: quoteIndexes.map((index) => index + 1).join(", "),
  });
}
