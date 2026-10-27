// Başlangıç saldırı testi (F10-T12) sunumu. Saf fonksiyonlar: ekran yalnız
// bunları çağırır, böylece karar/uyarı mantığı DOM olmadan test edilir.

import type {
  CliAttackAdapter,
  CliAttackFixture,
  CliAttackProduct,
  CliAttackReport,
  CliAttackVector,
  CliAttackVerdict,
} from "./api";

export type AttackTone = "ok" | "warn" | "danger" | "plain";

/** Karar rozetinin tonu. Hiçbir karar "yeşil" değildir: en iyi durum
 * "sessiz · kapatma yolu bildirildi" (davranış denenmedi). */
export function attackVerdictTone(verdict: CliAttackVerdict): AttackTone {
  switch (verdict) {
    case "triggered":
      return "danger";
    case "silent_unguarded":
      return "warn";
    case "inconclusive":
      return "plain";
    case "silent_guarded":
    default:
      return "warn";
  }
}

/** Karar metni: kaynağı ve neyin ölçülmediğini birlikte söyler. */
export function attackVerdictText(
  view: Pick<CliAttackVector, "verdict" | "verdictLabel" | "sourceLabel" | "hit">,
): string {
  switch (view.verdict) {
    case "triggered":
      return `tetiklendi · ${view.sourceLabel}`;
    case "silent_guarded":
      return "işaret yok · kapatma yolu bildirildi (davranış denenmedi)";
    case "silent_unguarded":
      return "işaret yok · koruma kanıtlanmadı";
    case "inconclusive":
      return "ölçülemedi · dedektör kanıtlanmadı";
    default:
      return view.verdictLabel;
  }
}

/** Vektörün kapatma yolu: bildirilen bayraklar, yoksa gerekçe. */
export function attackGuardText(
  view: Pick<CliAttackVector, "guardFlags" | "guardLabel">,
): string {
  return view.guardFlags.length > 0 ? view.guardFlags.join(" ") : view.guardLabel;
}

/** Gerçekten çalıştırılan komut satırı (sürüm başta).
 *
 * Boş argüman `""` olarak yazılır: sertleştirme `--tools ""` biçimini kullanır
 * ve boşlukla birleştirme onu görünmez kılardı (`--tools  --help`). */
export function attackCommand(
  adapter: Pick<CliAttackAdapter, "version" | "argv">,
): string {
  const argv =
    adapter.argv.length > 0
      ? adapter.argv.map((arg) => (arg === "" ? '""' : arg)).join(" ")
      : "—";
  return adapter.version ? `${adapter.version} · ${argv}` : argv;
}

/** Koşu yapılmadıysa nedeni; yapıldıysa `null`. */
export function attackSkipText(skipped: string | null): string | null {
  if (!skipped) return null;
  switch (skipped) {
    case "not_installed":
      return "kurulu değil · saldırı koşusu yapılmadı";
    case "interactive_only":
      return "etkileşimli-only · otomatik saldırı koşusu yapılmadı";
    case "executable_missing":
      return "yürütülebilir bulunamadı · saldırı koşusu yapılmadı";
    case "run_failed":
      return "koşu sıfır dışı bitti · kararlar kanıt sayılmaz";
    default:
      return `saldırı koşusu yapılmadı (${skipped})`;
  }
}

/** Fikstür özeti: dedektör kanıtı ve iş dizini kapısı. */
export function attackFixtureSummary(fixture: CliAttackFixture): string {
  const proven = fixture.detectorProven ? "kanıtlı" : "kanıtsız";
  const gate = fixture.gateRejected
    ? `reddedildi${fixture.gateCode ? ` (${fixture.gateCode})` : ""}`
    : "kabul edildi";
  return `Dedektör ${proven}: kirli ${fixture.pollutedHits}/${fixture.vectorTotal} · temiz ${fixture.cleanHits}/${fixture.vectorTotal} · kirli proje dizini iş dizini olarak ${gate}`;
}

/** Ürün yapılandırmasındaki vektör satırı. */
export function attackProductText(item: CliAttackProduct): string {
  return `${item.label}: ${item.hit ? "ulaşıldı" : "ulaşılamadı"}`;
}

/** Fikstür düzeyinde uyarılar (boş dizi = uyarı yok). */
export function attackFixtureWarnings(fixture: CliAttackFixture): string[] {
  const warnings: string[] = [];
  if (!fixture.detectorProven) {
    warnings.push(
      "Dedektör kanıtlanmadı: kirli fikstürde vekil yükleyici işaret bırakmadı, “işaret yok” sonuçları anlamsız.",
    );
  }
  if (!fixture.detectorControlClean) {
    warnings.push("Temiz fikstürde işaret oluştu: dedektör yanlış pozitif üretiyor.");
  }
  if (!fixture.gateRejected) {
    warnings.push("Kirli proje dizini iş dizini olarak kabul edildi (beklenmiyordu).");
  }
  for (const item of fixture.productConfig) {
    if (item.hit && item.vector !== "canary") {
      warnings.push(`Ürün yapılandırmasında “${item.label}” vektörü tetiklendi.`);
    }
  }
  if (fixture.canaryReadInProductConfig) {
    warnings.push(
      "Kapsam dışı canary ürün yapılandırmasında mutlak yolla okunabildi; bu sınır kapatılmadı, koruma istem/araç yolunun yokluğuna dayanıyor.",
    );
  }
  return warnings;
}

/** Adaptör düzeyinde uyarılar. */
export function attackAdapterWarnings(adapter: CliAttackAdapter): string[] {
  const warnings: string[] = [];
  if (adapter.hits > 0) {
    warnings.push(
      `${adapter.hits} vektör tetiklendi: bu adaptörle otomatik inceleme açılmamalı.`,
    );
  }
  const unguarded = adapter.vectors.filter((v) => v.verdict === "silent_unguarded").length;
  if (unguarded > 0) {
    warnings.push(
      `${unguarded} vektörde kapatma yolu bildirilmedi; işaret olmaması koruma kanıtı değil.`,
    );
  }
  if (adapter.vectors.length > 0 && !adapter.helpRead) {
    warnings.push("Yardım metni okunamadı; kapatma yolu doğrulanamadı.");
  }
  const skip = attackSkipText(adapter.skipped);
  if (skip) {
    warnings.push(skip);
  }
  return warnings;
}

/** Adaptör başına karar sayıları. */
export function attackCounts(adapters: CliAttackAdapter[]): Record<CliAttackVerdict, number> {
  const counts: Record<CliAttackVerdict, number> = {
    triggered: 0,
    silent_guarded: 0,
    silent_unguarded: 0,
    inconclusive: 0,
  };
  for (const adapter of adapters) {
    for (const vector of adapter.vectors) {
      counts[vector.verdict] += 1;
    }
  }
  return counts;
}

/** Raporun en kötü kararı (başlık satırı için). */
export function attackWorstVerdict(adapters: CliAttackAdapter[]): {
  label: string;
  tone: AttackTone;
} {
  const counts = attackCounts(adapters);
  if (counts.triggered > 0) {
    return { label: `${counts.triggered} vektör tetiklendi`, tone: "danger" };
  }
  if (counts.silent_unguarded > 0) {
    return { label: `${counts.silent_unguarded} vektörde koruma kanıtlanmadı`, tone: "warn" };
  }
  if (counts.inconclusive > 0) {
    return { label: `${counts.inconclusive} vektör ölçülemedi`, tone: "plain" };
  }
  if (counts.silent_guarded > 0) {
    return { label: "işaret yok · kapatma yolları bildirildi", tone: "warn" };
  }
  return { label: "koşu yapılmadı", tone: "plain" };
}

/** Raporun toplam işaret sayısı. */
export function attackTotalHits(report: Pick<CliAttackReport, "adapters">): number {
  return report.adapters.reduce((sum, adapter) => sum + adapter.hits, 0);
}
