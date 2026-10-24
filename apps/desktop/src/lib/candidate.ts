import type { AiCandidateRow, CandidateReport, QuoteMatchReport, ValidationIssue } from "./api";

/**
 * F11-T05 — aday şema doğrulaması ve alıntı eşleştirmesi metinleri.
 *
 * Kural: doğrulama durumu **hesaplanır**; arayüz yalnız gösterir. Şeması
 * geçersiz aday `valid` gibi çizilmez, metinde bulunmayan alıntı kaynak
 * sayılmaz. Sayfa numaraları veride 0 tabanlıdır; insan için 1 tabanlı yazılır.
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
    label: "Olay",
    hint: "title zorunlu; date, participants, description isteğe bağlı.",
    template: template({ title: "Ödeme yapılmadı", date: "12 Mart 2024", quotes: [] }),
  },
  {
    value: "person_candidate",
    label: "Kişi",
    hint: "name zorunlu; role, organization isteğe bağlı.",
    template: template({ name: "Ahmet Yılmaz", role: "davaci", quotes: [] }),
  },
  {
    value: "date_candidate",
    label: "Tarih",
    hint: "rawText + kind zorunlu; isoDate yalnız gün kesinse (YYYY-AA-GG).",
    template: template({
      rawText: "12 Mart 2024",
      kind: "document",
      isoDate: "2024-03-12",
      precision: "day",
      quotes: [],
    }),
  },
  {
    value: "claim_candidate",
    label: "İddia",
    hint: "text zorunlu; assertedBy, amountText isteğe bağlı.",
    template: template({ text: "Kira bedeli ödenmedi.", quotes: [] }),
  },
  {
    value: "summary_candidate",
    label: "Özet",
    hint: "text zorunlu; en az bir alıntı taşımalı.",
    template: template({ text: "Dosyada ödeme yapılmadığı ileri sürülüyor.", quotes: [] }),
  },
  {
    value: "answer_candidate",
    label: "Yanıt",
    hint: "text zorunlu; question isteğe bağlı.",
    template: template({ text: "Ödeme yapıldığına dair belge yok.", quotes: [] }),
  },
];

export function candidateKindLabel(value: string): string {
  return CANDIDATE_KINDS.find((kind) => kind.value === value)?.label ?? value;
}

export function candidateStateLabel(state: string): string {
  switch (state) {
    case "valid":
      return "geçerli";
    case "partial":
      return "kısmi";
    case "invalid":
      return "geçersiz";
    case "legacy":
      return "doğrulanmamış (eski kayıt)";
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
      return "alıntılar birebir doğrulandı";
    case "partial":
      return "alıntı kısmen doğrulandı";
    case "failed":
      return "alıntı doğrulanamadı";
    case "no_quotes":
      return "alıntı yok";
    case "not_checked":
      return "alıntı denetlenmedi";
    case "legacy":
      return "denetlenmemiş (eski kayıt)";
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
      return "birebir bulundu";
    case "normalized":
      return "normalizasyonla eşleşti";
    case "ambiguous":
      return "birden fazla yerde";
    case "page_mismatch":
      return "başka sayfada bulundu";
    case "not_found":
      return "metinde yok";
    case "unverifiable":
      switch (report.reason) {
        case "version_not_found":
          return "belge sürümü bulunamadı";
        case "case_mismatch":
          return "belge başka dosyaya ait";
        case "no_text":
          return "belgenin metni yok";
        default:
          return "doğrulanamadı";
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

/** Alıntının insan okur konumu; konum bilinmiyorsa "konum bilinmiyor". */
export function quotePositionLabel(report: QuoteMatchReport): string {
  if (report.pageIndex === null) {
    return "konum yok";
  }
  const page = `sayfa ${report.pageIndex + 1}`;
  if (report.charStart === null) {
    return page;
  }
  return `${page} · karakter ${report.charStart}`;
}

export function quoteSummary(report: CandidateReport): string {
  if (report.quotes.length === 0) {
    return "alıntı yok";
  }
  const exact = report.quotes.filter((quote) => quote.state === "exact").length;
  return `${exact}/${report.quotes.length} alıntı birebir`;
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
    return "Aday yükü boş.";
  }
  try {
    const parsed: unknown = JSON.parse(text);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      return "Aday yükü bir JSON nesnesi olmalı.";
    }
    return null;
  } catch (error) {
    return `JSON çözülemedi: ${error instanceof Error ? error.message : "bilinmeyen hata"}`;
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
    return "Aday bu sürümde doğrulanmamış; önce yeniden doğrulayın.";
  }
  if (row.quoteState === "failed") {
    return "Adaydaki alıntı belge metninde bulunamadı; kaynak olarak kullanılamaz.";
  }
  if (row.validationState === "invalid") {
    return "Aday şeması geçersiz; yükü düzeltin.";
  }
  return null;
}
