/**
 * Tek biçimlendirme katmanı (F04-T08).
 *
 * **Kural:** tarih/sayı/boyut/süre gösterimi **veri türünden** türer, ham string'den
 * değil. Bileşenler `toLocaleString`/`new Intl` çağırmaz; buradaki tipli
 * yardımcılardan birini seçer:
 *
 * - anlık zaman (ISO damga) → {@link formatDateTime}
 * - hukuki **takvim tarihi** (`YYYY[-MM[-DD]]`) → {@link formatLegalDate}
 * - sayı → {@link formatNumber} · oran → {@link formatPercent}
 * - bayt → {@link formatSize} · süre → {@link formatDuration}
 *
 * Ayrımın nedeni ölçülmüş iki kusur: (1) `new Date("2026-10-23")` **UTC** gece
 * yarısı sayılır, eksi saat diliminde gün bir önce kayar — takvim tarihi yerel
 * olarak kurulmalı; (2) aynı bayt değeri üç ayrı yerde üç ayrı biçimde
 * yazılıyordu (`1.5 KB`, `1,5 KiB`, `1.5 MB`) — Türkçe ondalık virgüldür ve
 * 1024'e bölen birim `KB` değil `KiB`'dir.
 *
 * Bu dosya `localization.test.ts` tarafından korunur: `src` altında başka bir
 * dosyada `toLocaleString`/`new Intl`/`"tr-TR"` görünürse test kırmızı olur.
 */
import { t, tr } from "./i18n";

const LOCALE = "tr-TR";

const DATE_ONLY = new Intl.DateTimeFormat(LOCALE, {
  day: "2-digit",
  month: "2-digit",
  year: "numeric",
});

const DATE_TIME = new Intl.DateTimeFormat(LOCALE, {
  day: "2-digit",
  month: "2-digit",
  year: "numeric",
  hour: "2-digit",
  minute: "2-digit",
});

const NUMBER = new Intl.NumberFormat(LOCALE);

/** Ondalıklı sayı: en çok bir hane, Türkçe virgül ("1,5"). */
const DECIMAL = new Intl.NumberFormat(LOCALE, {
  maximumFractionDigits: 1,
  minimumFractionDigits: 0,
});

const PERCENT = new Intl.NumberFormat(LOCALE, {
  style: "percent",
  maximumFractionDigits: 0,
});

function parseInstant(value: string | null | undefined): Date | null {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

/** Anlık zaman (ISO damga) → yerel "23.10.2026 13:05". Yoksa "—". */
export function formatDateTime(iso: string | null | undefined): string {
  const date = parseInstant(iso);
  if (date === null) return t("common.unknown");
  return DATE_TIME.format(date);
}

/** Geriye uyum adı: damga → yerel tarih+saat. */
export const formatTimestamp = formatDateTime;

/** Unix saniye (ör. `probedAtUnix`) → yerel "23.10.2026 13:05". `<= 0` "yoklama yok". */
export function formatUnixSeconds(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds) || seconds <= 0) {
    return t("common.unknown");
  }
  return formatDateTime(new Date(seconds * 1000).toISOString());
}

const LEGAL_DATE = /^(\d{4})(?:-(\d{2})(?:-(\d{2}))?)?$/;

/**
 * Hukuki takvim tarihi: gün varsa "23.10.2026", yalnız ay "10.2026", yalnız yıl
 * "2026". **Eksik gün uydurulmaz.** Zaman bileşeni taşıyan değer takvim tarihi
 * değildir → "tarih biçimi desteklenmiyor" (yanlış gün göstermektense görünür ret).
 */
export function formatLegalDate(value: string | null | undefined): string {
  if (!value) return t("common.unknown");
  const match = LEGAL_DATE.exec(value.trim());
  if (match === null) return t("dates.unsupported");

  const year = Number(match[1]);
  const month = match[2] === undefined ? null : Number(match[2]);
  const day = match[3] === undefined ? null : Number(match[3]);

  if (month !== null && (month < 1 || month > 12)) return t("dates.unreadable");
  if (day !== null) {
    if (month === null) return t("dates.unreadable");
    const probe = new Date(year, month - 1, day);
    if (
      probe.getFullYear() !== year ||
      probe.getMonth() !== month - 1 ||
      probe.getDate() !== day
    ) {
      return t("dates.unreadable");
    }
    return DATE_ONLY.format(probe);
  }
  if (month !== null) return `${String(month).padStart(2, "0")}.${year}`;
  return String(year);
}

/** Sayı → Türkçe binlik ayraçlı metin ("1.234"). Geçersizse "—". */
export function formatNumber(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return t("common.unknown");
  }
  return NUMBER.format(value);
}

/** Oran (0.3) → yüzde ("%30"). Geçersizse "—". */
export function formatPercent(ratio: number | null | undefined): string {
  if (ratio === null || ratio === undefined || !Number.isFinite(ratio)) {
    return t("common.unknown");
  }
  return PERCENT.format(ratio);
}

/** Bayt → "512 bayt" / "1,5 KiB" / "2,3 MiB" (1024 tabanı, Türkçe ondalık). */
export function formatSize(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes) || bytes < 0) {
    return t("common.unknown");
  }
  if (bytes < 1024) return `${NUMBER.format(bytes)} ${t("units.byte")}`;
  const kib = bytes / 1024;
  if (kib < 1024) return `${DECIMAL.format(kib)} ${t("units.kib")}`;
  return `${DECIMAL.format(kib / 1024)} ${t("units.mib")}`;
}

/** Süre (ms) → "340 ms" / "1,2 sn". Geçersizse "—". */
export function formatDuration(ms: number | null | undefined): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms) || ms < 0) {
    return t("common.unknown");
  }
  if (ms < 1000) return `${NUMBER.format(Math.round(ms))} ${t("units.millisecond")}`;
  const seconds = ms / 1000;
  return `${DECIMAL.format(seconds < 10 ? seconds : Math.round(seconds))} ${t("units.second")}`;
}

/** Dosya türü etiketi — sözlükten; bilinmeyen tür kendi kodunu gösterir. */
export const CASE_KIND_LABEL: Record<string, string> = { ...tr.labels.caseKind };

/** Hassasiyet etiketi — sözlükten. */
export const SENSITIVITY_LABEL: Record<string, string> = { ...tr.labels.sensitivity };

/** AI politikası etiketi — sözlükten. */
export const AI_POLICY_LABEL: Record<string, string> = { ...tr.labels.aiPolicy };
