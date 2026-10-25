import { describe, expect, it } from "vitest";

import type { PreUpdateBackup, RollbackResult } from "./api";
import {
  backupStatusText,
  backupVersionLabel,
  backupWarnings,
  canConfirmRollback,
  formatBackupDate,
  rollbackConsequences,
  rollbackDisabledReason,
  rollbackSummary,
  ROLLBACK_CONFIRMATION,
} from "./rollback";

function backup(overrides: Partial<PreUpdateBackup> = {}): PreUpdateBackup {
  return {
    fileName: "vault.db.v12-pre-update.bak",
    claimedVersion: 12,
    verifiedVersion: 12,
    sizeBytes: 4096,
    modifiedAt: "2026-10-24T09:00:00Z",
    problem: null,
    usable: true,
    nameMismatch: false,
    ...overrides,
  };
}

describe("rollback helpers", () => {
  it("onay metnini birebir arar", () => {
    expect(ROLLBACK_CONFIRMATION).toBe("GERİ DÖN");
    expect(canConfirmRollback("GERİ DÖN")).toBe(true);
    expect(canConfirmRollback("  GERİ DÖN ")).toBe(true);
    expect(canConfirmRollback("GERI DON")).toBe(false);
    expect(canConfirmRollback("")).toBe(false);
  });

  it("rozet doğrulanmış sürümü gösterir", () => {
    expect(backupVersionLabel(backup())).toBe("v12");
    expect(backupVersionLabel(backup({ verifiedVersion: null }))).toBe(
      "v12 · doğrulanmadı",
    );
  });

  it("kullanılamayan yedeğin nedenini yazar, gizlemez", () => {
    const corrupt = backup({
      verifiedVersion: null,
      usable: false,
      problem: "E_DB_MIGRATION_BACKUP: dosya açılamadı",
    });
    expect(backupStatusText(corrupt)).toContain("kullanılamaz");
    expect(backupStatusText(corrupt)).toContain("E_DB_MIGRATION_BACKUP");
    expect(backupStatusText(backup())).toBe("doğrulandı · şema v12");
  });

  it("ad ile dosya sürümü ayrışınca uyarır", () => {
    const renamed = backup({
      claimedVersion: 3,
      verifiedVersion: 17,
      nameMismatch: true,
    });
    expect(backupStatusText(renamed)).toBe(
      "doğrulandı · dosya v17, ad v3",
    );
    expect(backupWarnings(renamed)[0]).toContain("yeniden adlandırılmış");
    expect(backupWarnings(backup())).toEqual([]);
  });

  it("tarih yoksa zaman uydurmaz", () => {
    expect(formatBackupDate(null)).toBe("tarih bilinmiyor");
    expect(formatBackupDate("bozuk")).toBe("tarih okunamadı");
    expect(formatBackupDate("2026-10-24T09:00:00Z")).toMatch(/2026/);
  });

  it("geri dönüşün sınırlarını açıkça listeler", () => {
    const lines = rollbackConsequences();
    expect(lines).toHaveLength(4);
    expect(lines.join(" ")).toContain("güvenlik kopyası");
    expect(lines.join(" ")).toContain("ikili (uygulama) sürümü düşürülmez");
  });

  it("düğme gerekçesini sırayla verir", () => {
    expect(rollbackDisabledReason([], "", "")).toBe(
      "Kasa klasöründe güncelleme öncesi yedek yok.",
    );
    expect(rollbackDisabledReason([backup()], "", "")).toBe(
      "Önce bir yedek seçin.",
    );
    expect(
      rollbackDisabledReason(
        [backup({ usable: false })],
        "vault.db.v12-pre-update.bak",
        "GERİ DÖN",
      ),
    ).toBe("Seçilen yedek doğrulanamadı; geri dönüş yapılmaz.");
    expect(
      rollbackDisabledReason([backup()], "vault.db.v12-pre-update.bak", ""),
    ).toBe("Onay için “GERİ DÖN” yazın.");
    expect(
      rollbackDisabledReason([backup()], "vault.db.v12-pre-update.bak", "GERİ DÖN"),
    ).toBeNull();
  });

  it("sonucu iki sürümle anlatır", () => {
    const result: RollbackResult = {
      backupFile: "vault.db.v12-pre-update.bak",
      restoredVersion: 12,
      resumedVersion: 17,
      safetyCopy: "/tmp/vault.db.rollback-1.safety",
    };
    expect(rollbackSummary(result)).toBe(
      "Veri v12 sürümüne döndü; kasa güncel şema v17 olarak yeniden açıldı.",
    );
  });
});
