/**
 * Yerel hatırlatıcı yardımcıları (F08-T10).
 *
 * Bildirim metni bilerek **dosya/görev bilgisi taşımaz**: kilit ekranında ya da
 * bildirim merkezinde müvekkil adı, dosya adı veya görev başlığı görünmez.
 * Ayrıntı yalnız uygulama içinde, kasa açıkken gösterilir.
 *
 * Metinler F04-T08 gereği `lib/i18n.ts`ten gelir (modül düzeyi sabitler import
 * anında çözülür — tek dilli ürün kararı).
 */
import { t } from "./i18n";

/** Kullanıcının yerel saat diliminde `YYYY-MM-DD`. */
export function localIsoDay(date: Date = new Date()): string {
  const year = date.getFullYear();
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/** Bildirim gövdesi: kimlik ve tarih dışında hiçbir şey sızdırmaz. */
export const REMINDER_TEXT = t("reminders.notification");

export function reminderLabel(policy: string): string {
  switch (policy) {
    case "day_before":
      return t("reminders.label.day_before");
    case "on_day":
      return t("reminders.label.on_day");
    default:
      return t("reminders.label.off");
  }
}

export const REMINDER_POLICY_LABEL: Record<string, string> = {
  none: t("reminders.policy.none"),
  on_day: t("reminders.policy.on_day"),
  day_before: t("reminders.policy.day_before"),
};
