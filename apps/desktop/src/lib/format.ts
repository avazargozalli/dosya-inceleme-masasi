/** Teknik zaman (UTC ISO) → yerel okunur tarih. Ham değer yoksa "—". */
export function formatTimestamp(iso: string | null | undefined): string {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat("tr-TR", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

export const CASE_KIND_LABEL: Record<string, string> = {
  review: "İnceleme dosyası",
  reference_archive: "Kaynak arşivi",
};

export const SENSITIVITY_LABEL: Record<string, string> = {
  normal: "Normal",
  high: "Yüksek",
  client_confidential: "Müvekkil sırrı",
};

export const AI_POLICY_LABEL: Record<string, string> = {
  off: "AI kapalı",
  local_only: "Yalnız yerel model",
  cli_allowed: "CLI izinli",
  api_allowed: "API izinli",
};
