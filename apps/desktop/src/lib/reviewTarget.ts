/**
 * İnceleme kararının hedef kaydını **açık seçimden** üretir (F08-T08).
 *
 * Kurallar:
 * - Kayıt türü seçilmeden hedef üretilmez; "varsayılan iddia" yok.
 * - İddia türü de açıkça seçilir (F08-T05 kuralı).
 * - Olay hedefi tarih uydurmaz: hassasiyet `unknown`, tarih `null`.
 * - Boş metin kalıcı kayda dönüşmez.
 */
export type ReviewTargetKind = "" | "claim" | "note" | "task" | "event";

export type ReviewTargetInput =
  | { kind: "claim"; claimText: string; claimKind: string }
  | { kind: "note"; title: string; body: string }
  | { kind: "task"; title: string; reminderPolicy: "none" }
  | {
      kind: "event";
      eventKind: string;
      title: string;
      dateValue: null;
      datePrecision: "unknown";
      dateRole: "event";
      dateRaw: string;
      description: string;
      assertionKind: "user";
    };

export const TARGET_KIND_LABEL: Record<Exclude<ReviewTargetKind, "">, string> = {
  claim: "İddia",
  note: "Not",
  task: "Görev",
  event: "Olay",
};

/** Adayın payload'ından düz metni çıkarır; yoksa boş dize. */
export function itemText(payload: unknown): string {
  if (typeof payload === "string") return payload.trim();
  if (payload && typeof payload === "object") {
    const record = payload as Record<string, unknown>;
    for (const key of ["text", "body", "claimText", "title"]) {
      const value = record[key];
      if (typeof value === "string" && value.trim().length > 0) return value.trim();
    }
  }
  return "";
}

function firstLine(text: string, max: number): string {
  const line = text.split("\n")[0]?.trim() ?? "";
  return line.length > max ? line.slice(0, max) : line;
}

/**
 * Hedef türü + metin + (iddiada) iddia türünden gerçek hedef kayıt girdisi.
 * Eksik seçim varsa `null` döner: buton açılmaz.
 */
export function buildReviewTarget(
  kind: ReviewTargetKind,
  text: string,
  claimKind = "",
): ReviewTargetInput | null {
  const body = text.trim();
  if (body.length === 0) return null;
  switch (kind) {
    case "claim": {
      if (claimKind === "") return null;
      return { kind: "claim", claimText: body, claimKind };
    }
    case "note":
      return { kind: "note", title: firstLine(body, 80), body };
    case "task":
      return { kind: "task", title: firstLine(body, 300), reminderPolicy: "none" };
    case "event":
      return {
        kind: "event",
        eventKind: "other",
        title: firstLine(body, 300),
        dateValue: null,
        datePrecision: "unknown",
        dateRole: "event",
        dateRaw: "",
        description: body,
        assertionKind: "user",
      };
    default:
      return null;
  }
}
