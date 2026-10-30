import type { PreUpdateBackup, RollbackResult } from "./api";
import { formatDateTime } from "./format";
import { t } from "./i18n";

/**
 * Onay metni çekirdekteki `ROLLBACK_CONFIRMATION` ile birebir aynı olmalı.
 * Arayüz yalnız kolaylık sağlar; kapı Rust tarafındadır.
 *
 * Metin sözlükten gelir (F04-T08: arayüz metni tek yerde); `rollback.test.ts`
 * hem değeri hem de `commands.rs`teki sabiti okuyup eşleşmeyi doğrular, böylece
 * sözlük değişikliği sessizce çekirdekten ayrışamaz.
 */
export const ROLLBACK_CONFIRMATION = t("rollback.confirmToken");

/** Onay metni yeterli mi? Çekirdek kırpar, karşılaştırma birebirdir. */
export function canConfirmRollback(text: string): boolean {
  return text.trim() === ROLLBACK_CONFIRMATION;
}

/** Rozet metni: doğrulanmış sürüm varsa o, yoksa addan okunan (iddia) sürüm. */
export function backupVersionLabel(backup: PreUpdateBackup): string {
  if (backup.verifiedVersion !== null) return `v${backup.verifiedVersion}`;
  return t("rollback.versionUnverified", { version: backup.claimedVersion });
}

/**
 * Tek satır durum. Kullanılamayan yedeğin **nedeni** yazılır; sessizce
 * gizlenmez ve "hazır" denmez.
 */
export function backupStatusText(backup: PreUpdateBackup): string {
  if (!backup.usable) {
    return backup.problem
      ? t("rollback.unusableProblem", { problem: backup.problem })
      : t("rollback.unusable");
  }
  return backup.verifiedVersion === backup.claimedVersion
    ? t("rollback.verifiedSchema", { version: String(backup.verifiedVersion) })
    : t("rollback.verifiedMismatch", {
        verified: String(backup.verifiedVersion),
        claimed: backup.claimedVersion,
      });
}

/** Kullanıcının bilmesi gereken uyarılar; boş dizi = uyarı yok. */
export function backupWarnings(backup: PreUpdateBackup): string[] {
  const warnings: string[] = [];
  if (backup.nameMismatch) {
    warnings.push(
      t("rollback.nameMismatch", {
        claimed: backup.claimedVersion,
        verified: String(backup.verifiedVersion),
      }),
    );
  }
  if (backup.problem && backup.usable) {
    warnings.push(backup.problem);
  }
  return warnings;
}

/** Yerel tarih+saat; tarih yoksa "tarih bilinmiyor" (uydurma zaman yok). */
export function formatBackupDate(iso: string | null): string {
  if (!iso) return t("dates.missing");
  const out = formatDateTime(iso);
  // Biçimleyici geçersiz damgada "—" döner; yedek satırında bu "okunamadı"dır.
  return out === t("common.unknown") ? t("dates.unreadable") : out;
}

/**
 * Geri dönüşün **ne yapacağı** ve **ne yapmayacağı**. Arayüzde aynen
 * gösterilir; kullanıcı onay kutusunu görmeden düğme açılmaz.
 */
export function rollbackConsequences(): string[] {
  return [
    t("rollback.consequence1"),
    t("rollback.consequence2"),
    t("rollback.consequence3"),
    t("rollback.consequence4"),
    t("rollback.consequence5"),
    t("rollback.consequence6"),
  ];
}

/**
 * Düğmenin neden kapalı olduğunu söyler; `null` = basılabilir. Kapalı düğme
 * gerekçesiz bırakılmaz.
 */
export function rollbackDisabledReason(
  backups: PreUpdateBackup[],
  selected: string,
  confirmation: string,
): string | null {
  if (backups.length === 0) return t("rollback.noBackups");
  const backup = backups.find((b) => b.fileName === selected);
  if (!backup) return t("rollback.selectFirst");
  if (!backup.usable) return t("rollback.notVerifiedSelected");
  if (!canConfirmRollback(confirmation)) {
    return t("rollback.confirmRequired", { token: ROLLBACK_CONFIRMATION });
  }
  return null;
}

/** Sonuç cümlesi: hangi sürüm geri kondu, kasa hangi sürümde açıldı. */
export function rollbackSummary(result: RollbackResult): string {
  return t("rollback.summary", {
    restored: result.restoredVersion,
    resumed: result.resumedVersion,
  });
}
