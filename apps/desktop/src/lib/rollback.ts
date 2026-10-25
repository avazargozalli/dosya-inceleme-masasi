import type { PreUpdateBackup, RollbackResult } from "./api";

/**
 * Onay metni çekirdekteki `ROLLBACK_CONFIRMATION` ile birebir aynı olmalı.
 * Arayüz yalnız kolaylık sağlar; kapı Rust tarafındadır.
 */
export const ROLLBACK_CONFIRMATION = "GERİ DÖN";

/** Onay metni yeterli mi? Çekirdek kırpar, karşılaştırma birebirdir. */
export function canConfirmRollback(text: string): boolean {
  return text.trim() === ROLLBACK_CONFIRMATION;
}

/** Rozet metni: doğrulanmış sürüm varsa o, yoksa addan okunan (iddia) sürüm. */
export function backupVersionLabel(backup: PreUpdateBackup): string {
  if (backup.verifiedVersion !== null) return `v${backup.verifiedVersion}`;
  return `v${backup.claimedVersion} · doğrulanmadı`;
}

/**
 * Tek satır durum. Kullanılamayan yedeğin **nedeni** yazılır; sessizce
 * gizlenmez ve "hazır" denmez.
 */
export function backupStatusText(backup: PreUpdateBackup): string {
  if (!backup.usable) {
    return backup.problem ? `kullanılamaz — ${backup.problem}` : "kullanılamaz";
  }
  return backup.verifiedVersion === backup.claimedVersion
    ? `doğrulandı · şema v${backup.verifiedVersion}`
    : `doğrulandı · dosya v${backup.verifiedVersion}, ad v${backup.claimedVersion}`;
}

/** Kullanıcının bilmesi gereken uyarılar; boş dizi = uyarı yok. */
export function backupWarnings(backup: PreUpdateBackup): string[] {
  const warnings: string[] = [];
  if (backup.nameMismatch) {
    warnings.push(
      `Dosya adı v${backup.claimedVersion} diyor, dosya v${backup.verifiedVersion} taşıyor; ad yeniden adlandırılmış olabilir.`,
    );
  }
  if (backup.problem && backup.usable) {
    warnings.push(backup.problem);
  }
  return warnings;
}

/** `tr-TR` tarih; tarih yoksa "tarih bilinmiyor" (uydurma zaman yok). */
export function formatBackupDate(iso: string | null): string {
  if (!iso) return "tarih bilinmiyor";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "tarih okunamadı";
  return date.toLocaleString("tr-TR", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/**
 * Geri dönüşün **ne yapacağı** ve **ne yapmayacağı**. Arayüzde aynen
 * gösterilir; kullanıcı onay kutusunu görmeden düğme açılmaz.
 */
export function rollbackConsequences(): string[] {
  return [
    "Kasa kilitlenir; açık kasadaki tüm bağlantılar kapatılır.",
    "Kasa dosyası seçilen güncelleme öncesi yedekle değiştirilir; güncelleme sonrası kayıtlar kasadan çıkar.",
    "Değiştirilen dosya silinmez: aynı klasörde `vault.db.rollback-<zaman>.safety` güvenlik kopyası olarak kalır.",
    "Kasa yeniden açılır ve bu uygulamanın şema sürümüne ilerletilir; ikili (uygulama) sürümü düşürülmez.",
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
  if (backups.length === 0) return "Kasa klasöründe güncelleme öncesi yedek yok.";
  const backup = backups.find((b) => b.fileName === selected);
  if (!backup) return "Önce bir yedek seçin.";
  if (!backup.usable) return "Seçilen yedek doğrulanamadı; geri dönüş yapılmaz.";
  if (!canConfirmRollback(confirmation)) return "Onay için “GERİ DÖN” yazın.";
  return null;
}

/** Sonuç cümlesi: hangi sürüm geri kondu, kasa hangi sürümde açıldı. */
export function rollbackSummary(result: RollbackResult): string {
  return `Veri v${result.restoredVersion} sürümüne döndü; kasa güncel şema v${result.resumedVersion} olarak yeniden açıldı.`;
}
