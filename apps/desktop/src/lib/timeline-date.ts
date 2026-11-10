/**
 * Kronoloji tarih yardımcıları (şartname F08-T03).
 *
 * Kurallar:
 * - **Eksik gün uydurulmaz**: kesinlik `month`/`year` ise saklanan değer ve
 *   ekranda yazılan metin gün taşımaz. Eski kayıtlarda ay kesinliğiyle tam gün
 *   saklanmış olabilir (arayüz eskiden tam gün zorunlu tutuyordu); görüntü
 *   kesinliğe göre kırpılır.
 * - **Aralık** iki tam tarih ister (`YYYY-AA-GG/YYYY-AA-GG`); başlangıç ≤ bitiş.
 * - **Yaklaşık** tarih ay çözünürlüğünde girilir (`YYYY-AA`).
 * - Biçim katıdır: sıfır dolgulu ISO; "10.04.2026" gibi ham ifadeler tarih
 *   alanına değil, **ham metin** alanına yazılır.
 *
 * Bu yardımcılar çekirdeğin `normalize_event_date` sözleşmesini arayüzde
 * yansıtır; çekirdek yine doğrular (`E_EVENT_DATE`).
 */

export type DateInputMode = "date" | "month" | "year" | "range" | "none";

/** Kesinliğe göre tarih giriş türü. */
export function dateInputMode(precision: string): DateInputMode {
  switch (precision) {
    case "day":
      return "date";
    case "month":
    case "approximate":
      return "month";
    case "year":
      return "year";
    case "range":
      return "range";
    case "unknown":
      return "none";
    default:
      return "date";
  }
}

const DAY_RE = /^(\d{4})-(\d{2})-(\d{2})$/;
const MONTH_RE = /^(\d{4})-(\d{2})$/;
const YEAR_RE = /^\d{4}$/;

/** `YYYY-AA-GG` ve takvimde **var olan** gün (31 Şubat reddedilir). */
export function isRealDay(value: string): boolean {
  const match = DAY_RE.exec(value);
  if (!match) return false;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (month < 1 || month > 12 || day < 1 || day > 31) return false;
  const date = new Date(Date.UTC(year, month - 1, day));
  return (
    date.getUTCFullYear() === year &&
    date.getUTCMonth() === month - 1 &&
    date.getUTCDate() === day
  );
}

/** `YYYY-AA`, ay 01–12. */
export function isRealMonth(value: string): boolean {
  const match = MONTH_RE.exec(value);
  if (!match) return false;
  const month = Number(match[2]);
  return month >= 1 && month <= 12;
}

/** `YYYY` (dört hane). */
export function isYear(value: string): boolean {
  return YEAR_RE.test(value);
}

export type DateDraftIssue =
  | "needDay"
  | "needMonth"
  | "needYear"
  | "needRange"
  | "rangeOrder";

/**
 * Taslaktaki tarih alanlarını kesinliğe göre doğrular. `null` = geçerli.
 * Boş tarih **geçersizdir**: "gün" kesinliğinde tarihsiz olay, sıralamada
 * bilinmeyen gibi görünürdü (kesinlik yalanı).
 */
export function dateDraftIssue(
  precision: string,
  date: string,
  dateEnd: string,
): DateDraftIssue | null {
  const start = date.trim();
  const end = dateEnd.trim();
  switch (precision) {
    case "unknown":
      return null;
    case "day":
      return isRealDay(start) ? null : "needDay";
    case "month":
    case "approximate":
      return isRealMonth(start) ? null : "needMonth";
    case "year":
      return isYear(start) ? null : "needYear";
    case "range":
      if (!isRealDay(start) || !isRealDay(end)) return "needRange";
      // ISO tarihler sözlük sırasıyla da kronolojiktir.
      return start <= end ? null : "rangeOrder";
    default:
      return "needDay";
  }
}

/** Çekirdeğe gönderilen `dateValue`: aralık `başlangıç/bitiş`, diğerleri tek değer. */
export function composeDateValue(
  precision: string,
  date: string,
  dateEnd: string,
): string | null {
  if (precision === "unknown") return null;
  const start = date.trim();
  if (precision === "range") return `${start}/${dateEnd.trim()}`;
  return start.length > 0 ? start : null;
}

/**
 * Ekranda/çıktıda yazılan tarih metni. Ay/yıl kesinliğinde **gün yazılmaz**;
 * eski kayıtlarda tam gün saklanmış olsa bile kırpılır.
 */
export function eventDateText(event: {
  datePrecision: string;
  dateValue: string | null;
  sortableDate: string | null;
}): string | null {
  const value = event.dateValue ?? event.sortableDate;
  if (value === null || value.length === 0) return null;
  switch (event.datePrecision) {
    case "unknown":
      return null;
    case "range":
      return value.split("/").join(" – ");
    case "month":
      return value.length >= 7 ? value.slice(0, 7) : value;
    case "year":
      return value.length >= 4 ? value.slice(0, 4) : value;
    default:
      return value;
  }
}

/**
 * Düzenleme formunun başlangıç değerleri. Eski kayıtlar kesinliğe uymayan tam
 * gün taşıyabilir; giriş alanı kesinliğe kırpılır (**ham metin** ayrı alanda
 * korunur, kaybolmaz).
 */
export function editDateInputs(
  precision: string,
  dateValue: string | null,
): { date: string; dateEnd: string } {
  const value = dateValue ?? "";
  if (precision === "range") {
    const [start, end] = value.split("/");
    return { date: start ?? "", dateEnd: end ?? "" };
  }
  if (precision === "month" || precision === "approximate") {
    return { date: value.length >= 7 ? value.slice(0, 7) : value, dateEnd: "" };
  }
  if (precision === "year") {
    return { date: value.length >= 4 ? value.slice(0, 4) : value, dateEnd: "" };
  }
  return { date: value, dateEnd: "" };
}
