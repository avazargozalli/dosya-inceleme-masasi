// Başlangıç saldırı testi (F10-T12) sunumu. Saf fonksiyonlar: ekran yalnız
// bunları çağırır, böylece karar/uyarı mantığı DOM olmadan test edilir.
// Metinler F04-T08 gereği merkezi sözlükten gelir (`lib/i18n.ts`).

import type {
  CliAttackAdapter,
  CliAttackFixture,
  CliAttackProduct,
  CliAttackReport,
  CliAttackVector,
  CliAttackVerdict,
} from "./api";
import { t } from "./i18n";

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
      return t("attack.verdictTriggered", { source: view.sourceLabel });
    case "silent_guarded":
      return t("attack.verdictSilentGuarded");
    case "silent_unguarded":
      return t("attack.verdictSilentUnguarded");
    case "inconclusive":
      return t("attack.verdictInconclusive");
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
      : t("common.unknown");
  return adapter.version ? `${adapter.version} · ${argv}` : argv;
}

/** Koşu yapılmadıysa nedeni; yapıldıysa `null`. */
export function attackSkipText(skipped: string | null): string | null {
  if (!skipped) return null;
  switch (skipped) {
    case "not_installed":
      return t("attack.skipNotInstalled");
    case "interactive_only":
      return t("attack.skipInteractiveOnly");
    case "executable_missing":
      return t("attack.skipExecutableMissing");
    case "run_failed":
      return t("attack.skipRunFailed");
    default:
      return t("attack.skipOther", { code: skipped });
  }
}

/** Fikstür özeti: dedektör kanıtı ve iş dizini kapısı. */
export function attackFixtureSummary(fixture: CliAttackFixture): string {
  const proven = fixture.detectorProven ? t("attack.provenYes") : t("attack.provenNo");
  const gate = fixture.gateRejected
    ? fixture.gateCode
      ? t("attack.gateRejectedCode", { code: fixture.gateCode })
      : t("attack.gateRejected")
    : t("attack.gateAccepted");
  return t("attack.fixtureSummary", {
    proven,
    polluted: fixture.pollutedHits,
    total: fixture.vectorTotal,
    clean: fixture.cleanHits,
    gate,
  });
}

/** Ürün yapılandırmasındaki vektör satırı. */
export function attackProductText(item: CliAttackProduct): string {
  return t("attack.productLine", {
    label: item.label,
    state: item.hit ? t("attack.productHit") : t("attack.productMiss"),
  });
}

/** Fikstür düzeyinde uyarılar (boş dizi = uyarı yok). */
export function attackFixtureWarnings(fixture: CliAttackFixture): string[] {
  const warnings: string[] = [];
  if (!fixture.detectorProven) {
    warnings.push(t("attack.warnDetectorUnproven"));
  }
  if (!fixture.detectorControlClean) {
    warnings.push(t("attack.warnControlDirty"));
  }
  if (!fixture.gateRejected) {
    warnings.push(t("attack.warnGateAccepted"));
  }
  for (const item of fixture.productConfig) {
    if (item.hit && item.vector !== "canary") {
      warnings.push(t("attack.warnProductVector", { label: item.label }));
    }
  }
  if (fixture.canaryReadInProductConfig) {
    warnings.push(t("attack.warnCanary"));
  }
  return warnings;
}

/** Adaptör düzeyinde uyarılar. */
export function attackAdapterWarnings(adapter: CliAttackAdapter): string[] {
  const warnings: string[] = [];
  if (adapter.hits > 0) {
    warnings.push(t("attack.warnHits", { count: adapter.hits }));
  }
  const unguarded = adapter.vectors.filter((v) => v.verdict === "silent_unguarded").length;
  if (unguarded > 0) {
    warnings.push(t("attack.warnUnguarded", { count: unguarded }));
  }
  if (adapter.vectors.length > 0 && !adapter.helpRead) {
    warnings.push(t("attack.warnHelpUnread"));
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
    return { label: t("attack.worstTriggered", { count: counts.triggered }), tone: "danger" };
  }
  if (counts.silent_unguarded > 0) {
    return { label: t("attack.worstUnguarded", { count: counts.silent_unguarded }), tone: "warn" };
  }
  if (counts.inconclusive > 0) {
    return { label: t("attack.worstInconclusive", { count: counts.inconclusive }), tone: "plain" };
  }
  if (counts.silent_guarded > 0) {
    return { label: t("attack.worstGuarded"), tone: "warn" };
  }
  return { label: t("attack.worstNone"), tone: "plain" };
}

/** Raporun toplam işaret sayısı. */
export function attackTotalHits(report: Pick<CliAttackReport, "adapters">): number {
  return report.adapters.reduce((sum, adapter) => sum + adapter.hits, 0);
}
