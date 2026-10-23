/**
 * Yerel hatırlatıcı yardımcıları (F08-T10).
 *
 * Bildirim metni bilerek **dosya/görev bilgisi taşımaz**: kilit ekranında ya da
 * bildirim merkezinde müvekkil adı, dosya adı veya görev başlığı görünmez.
 * Ayrıntı yalnız uygulama içinde, kasa açıkken gösterilir.
 */

/** Kullanıcının yerel saat diliminde `YYYY-MM-DD`. */
export function localIsoDay(date: Date = new Date()): string {
  const year = date.getFullYear();
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/** Bildirim gövdesi: kimlik ve tarih dışında hiçbir şey sızdırmaz. */
export const REMINDER_TEXT = "Bu dosyada yaklaşan bir görev tarihi var.";

export function reminderLabel(policy: string): string {
  switch (policy) {
    case "day_before":
      return "Yarın için hatırlatıcı";
    case "on_day":
      return "Bugün için hatırlatıcı";
    default:
      return "Hatırlatıcı kapalı";
  }
}

export const REMINDER_POLICY_LABEL: Record<string, string> = {
  none: "Hatırlatma yok",
  on_day: "Günü gelince",
  day_before: "Bir gün önce",
};
