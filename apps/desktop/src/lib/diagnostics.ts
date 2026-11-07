/**
 * Teşhis yardımcıları (REQ-033).
 *
 * Arayüzün **çekirdekten** okuduğu sürüm/protokol/platform bilgisini yorumlar.
 * Kural: tanınmayan değer gizlenmez — platform kodu **ham** yazılır (uydurma ad
 * yok); sürüm ya da protokol ayrışırsa yardımcı `true` döner ve kart uyarı basar.
 *
 * Burada kullanıcıya görünen cümle yok: metin `lib/i18n.ts`ten gelir. Platform
 * adları marka adıdır (çevrilmez), bilinmeyen kod ham değeriyle görünür.
 */

import type { SystemInfo } from "./api";

/** Arayüzün anladığı IPC protokol sürümü — `commands.rs` `system_info` ile aynı. */
export const SUPPORTED_PROTOCOL = 1;

const PLATFORM_LABELS: Record<string, string> = {
  macos: "macOS",
  windows: "Windows",
  linux: "Linux",
  ios: "iOS",
  android: "Android",
  freebsd: "FreeBSD",
};

/** Bilinen platform kodunu okunur ada çevirir; tanınmayanı **ham** bırakır. */
export function platformLabel(platform: string): string {
  const raw = platform.trim();
  return PLATFORM_LABELS[raw.toLowerCase()] ?? raw;
}

/**
 * Arayüz paketi ile çalışan çekirdek aynı sürümde mi?
 *
 * Bugüne kadar ekran yalnız `package.json` sürümünü gösteriyordu; paketle çalışan
 * ikili ayrışırsa (yanlış kurulum, elle kopyalanmış arayüz) ekran farkında
 * olmadan yanlış sürüm yazıyordu.
 */
export function versionDrift(uiVersion: string, coreVersion: string): boolean {
  return uiVersion.trim() !== coreVersion.trim();
}

/** Çekirdeğin bildirdiği protokol, arayüzün beklediğiyle aynı mı? */
export function protocolDrift(coreProtocol: number): boolean {
  return coreProtocol !== SUPPORTED_PROTOCOL;
}

/**
 * `system_info` yanıtını doğrular.
 *
 * Sözleşme dışı bir gövde (eksik alan, yanlış tip, boş sürüm) **kabul edilmez**:
 * eksik yanıtı `SystemInfo` sayıp ekrana yazmak "0.1.0" gibi uydurma bir sürüm
 * göstermekle aynı şeydir. Geçersiz gövde `null` döner; kart bunu "okunamadı"
 * diye yazar.
 */
export function parseSystemInfo(raw: unknown): SystemInfo | null {
  if (typeof raw !== "object" || raw === null) return null;
  const value = raw as Record<string, unknown>;
  const appVersion = value.appVersion;
  const protocolVersion = value.protocolVersion;
  const platform = value.platform;
  if (typeof appVersion !== "string" || appVersion.trim() === "") return null;
  if (typeof protocolVersion !== "number" || !Number.isInteger(protocolVersion)) return null;
  if (typeof platform !== "string" || platform.trim() === "") return null;
  return { appVersion, protocolVersion, platform };
}
