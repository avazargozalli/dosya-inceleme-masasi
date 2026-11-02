// @vitest-environment jsdom
//
// Ayarlar ekranı davranış testi. `lib/rollback` ve `lib/backup-schedule` testleri
// kural/metin düzeyini doğrular; burada 624 satırlık ekran gerçekten render edilip
// F16 (güncelleme öncesi yedek, geri dönüş, tatbikat), yedek alma, GC, geri
// yükleme ve parola değiştirme kararlarının arayüzde **uygulandığı** ölçülür.
// `api` kısmi mock; `t()`, `lib/rollback`, `lib/format` ve `lib/backup-schedule`
// gerçektir. Sabitlenen davranışlar:
//
// 1. Kasa kilitliyken künye "—", geri dönüş/kuyruk/bütçe kartları kilitli yazar;
//    yedek taraması **istenmez** ve yedek alma düğmesi kapalıdır.
// 2. Açık kasa künyesi sunucudan yazılır; sürüm **paketten** okunur (gömülü
//    "0.2.0" yok); yedek listesi kendiliğinden taranır ve ilk **kullanılabilir**
//    yedek seçilir.
// 3. Yedek satırı doğrulanmış sürümü, durumu, tarih/boyutu ve ad uyuşmazlığı
//    uyarısını yazar; kullanılamayan yedeğin nedeni gizlenmez, radyosu kapalıdır.
// 4. Onay belirteci birebir yazılmadan geri dönüş düğmesi açılmaz; gerekçe
//    düğmenin altında yazılıdır. Seçim yokken gerekçe "önce bir yedek seçin"dir.
// 5. Geri dönüş **seçilen dosya ve onayla** çekirdeğe gider, onay alanı temizlenir,
//    kasa durumu ve yedek listesi tazelenir.
// 6. **Ölçülen kusur (A):** başarı bildirimi `preUpdate.length > 0` bloğunun
//    içindeydi; tazeleme listeyi boşaltınca "Geri dönüş tamamlandı" paneli hiç
//    görünmüyordu (işlem gerçekten oldu, kullanıcı görmüyor).
// 7. Geri dönüş düşerse hata yazılır, başarı paneli kalmaz, kasa durumu yeniden
//    okunur; kasa kilitliye düştüyse yedek listesi ve onay metni temizlenir.
// 8. Yedek alma: klasör seçilmezse istek gitmez; sonuç künyesi ve varsa tek dosya
//    arşivi yazılır.
// 9. **Ölçülen kusur (B):** yedek alma düşünce önceki "Yedek hazır" bildirimi
//    ekranda kalıyordu (kırmızı hatayla birlikte yeşil başarı).
// 10. Otomatik yedek ayarları yerel kayıttan okunur; "kapat"/"sıfırla" yalnız
//     kendi anahtarını siler.
// 11. GC: tarama adayları yazar, temizleme aday yokken kapalıdır; silme sonucu ve
//     eksik dosya uyarısı yazılır.
// 12. **Ölçülen kusur (C):** GC isteği düşünce önceki tarama/silme bildirimi
//     ekranda kalıyordu.
// 13. Geri yükleme hedef konum ister (kırpılır); iptal edilen seçim istek üretmez.
// 14. Geri yükleme sonucu doğrulanan dosya/boyut/süre ve **5 + "…"** ile
//     sayılmayan dosyaları yazar.
// 15. **Ölçülen kusur (D):** geri yükleme düşünce önceki "tamamlandı" bildirimi
//     ekranda kalıyordu.
// 16. Parola: 10 karakterden kısa yeni parola ve boş mevcut parola düğmeyi kapalı
//     tutar; eşleşmeyen parola çekirdeğe **gitmez**.
// 17. **Ölçülen kusur (E):** başarılı parola değişiminden sonra eşleşmeyen parola
//     denemesinde yeşil "Parola değiştirildi" bildirimi kırmızı hatayla birlikte
//     ekranda kalıyordu.
// 18. Parola düşerse yazılanlar korunur ve hata kodu yazılır.

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { version as appVersion } from "../../package.json";
import {
  BACKUP_DAYS_KEY,
  BACKUP_DIR_KEY,
  BACKUP_LAST_KEY,
} from "../lib/backup-schedule";
import { formatDateTime, formatSize } from "../lib/format";
import { t } from "../lib/i18n";
import {
  backupStatusText,
  backupVersionLabel,
  backupWarnings,
  rollbackConsequences,
  rollbackSummary,
} from "../lib/rollback";
import type {
  BackupResult,
  JobQueueView,
  PreUpdateBackup,
  RestoreResult,
  RollbackResult,
  VaultGcResult,
  VaultStatus,
  WorkerBudgetProfile,
} from "../lib/api";

const mocks = vi.hoisted(() => ({
  status: vi.fn(),
  vaultPreUpdateBackups: vi.fn(),
  vaultRollbackPreUpdate: vi.fn(),
  vaultBackup: vi.fn(),
  vaultGc: vi.fn(),
  vaultRestore: vi.fn(),
  vaultChangePassword: vi.fn(),
  jobQueueStatus: vi.fn(),
  workerBudgets: vi.fn(),
}));

const dialog = vi.hoisted(() => ({ open: vi.fn() }));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

// Tauri olmadan ekranı ölçmek için klasör seçici sahtelenir.
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: dialog.open }));

const { Settings } = await import("./Settings");

/* ------------------------------------------------------------------ fixtures */

const ISO = "2026-10-30T09:15:00.000Z";

const UNLOCKED: VaultStatus = {
  unlocked: true,
  vaultPath: "/Users/avukat/Kasa/vault.dimvault",
  vaultId: "vault-9f2c",
  schemaVersion: 23,
  preUpdateBackup: null,
};

const LOCKED: VaultStatus = {
  unlocked: false,
  vaultPath: null,
  vaultId: null,
  schemaVersion: null,
  preUpdateBackup: null,
};

/** Doğrulanmış, adıyla taşıdığı sürüm aynı. */
const B_OK: PreUpdateBackup = {
  fileName: "vault.db.pre-v23.dimbackup",
  claimedVersion: 23,
  verifiedVersion: 23,
  sizeBytes: 3_500_000,
  modifiedAt: ISO,
  problem: null,
  usable: true,
  nameMismatch: false,
};

/** Adı v22 diyor, dosya v21 taşıyor: ad uyuşmazlığı uyarısı olmalı. */
const B_RENAMED: PreUpdateBackup = {
  fileName: "vault.db.pre-v22.dimbackup",
  claimedVersion: 22,
  verifiedVersion: 21,
  sizeBytes: 2048,
  modifiedAt: null,
  problem: null,
  usable: true,
  nameMismatch: true,
};

/** Doğrulanamadı: nedeni yazılmalı, radyosu kapalı olmalı. */
const B_BAD: PreUpdateBackup = {
  fileName: "bozuk-yedek.dimbackup",
  claimedVersion: 20,
  verifiedVersion: null,
  sizeBytes: 0,
  modifiedAt: null,
  problem: "manifest okunamadı",
  usable: false,
  nameMismatch: false,
};

const QUEUE_EMPTY: JobQueueView = {
  summary: { total: 0, byState: [], dueNow: 0, oldestQueuedAt: null, blockedByDependency: 0 },
  jobs: [],
};

const ROLLBACK_RESULT: RollbackResult = {
  backupFile: B_OK.fileName,
  restoredVersion: 22,
  resumedVersion: 23,
  safetyCopy: "vault.db.rollback-20261030.safety",
};

const BACKUP_RESULT: BackupResult = {
  path: "/Volumes/Yedek/kasa-20261030",
  fileCount: 12,
  totalBytes: 3_500_000,
  manifestHash: "0123456789abcdef0123",
  archivePath: null,
};

const RESTORE_RESULT: RestoreResult = {
  vaultPath: "/tmp/yeni-kasa/vault.dimvault",
  fileCount: 12,
  verifiedFiles: 12,
  verifiedBytes: 5_000_000,
  verifyMs: 42,
  unlistedBlobs: ["a.bin", "b.bin", "c.bin", "d.bin", "e.bin", "f.bin"],
};

const GC_SCAN: VaultGcResult = {
  dryRun: true,
  candidates: [
    { id: "b1", storageId: "s1", encryptedSize: 1_048_576 },
    { id: "b2", storageId: "s2", encryptedSize: 524_288 },
  ],
  deleted: [],
  freedBytes: 0,
  missingFiles: [],
};

const GC_CLEAN: VaultGcResult = {
  dryRun: false,
  candidates: [],
  deleted: [{ id: "b1", storageId: "s1", encryptedSize: 1_048_576 }],
  freedBytes: 1_048_576,
  missingFiles: ["blob-7.bin"],
};

/* ------------------------------------------------------------------- harness */

type SetupOptions = {
  status?: VaultStatus;
  backups?: PreUpdateBackup[];
  budgets?: WorkerBudgetProfile[];
};

function setup(options: SetupOptions = {}) {
  mocks.status.mockResolvedValue(options.status ?? UNLOCKED);
  mocks.vaultPreUpdateBackups.mockResolvedValue(options.backups ?? [B_OK, B_BAD]);
  mocks.jobQueueStatus.mockResolvedValue(QUEUE_EMPTY);
  mocks.workerBudgets.mockResolvedValue(options.budgets ?? []);
  dialog.open.mockResolvedValue(null);
  render(<Settings />);
}

/** Başlığından kartın kendisini bul (ekran tek `<h2>` hiyerarşisi kullanıyor). */
function cardOf(title: string): HTMLElement {
  const heading = screen.getByRole("heading", { name: title, level: 2 });
  const card = heading.closest(".card");
  if (!card) throw new Error(`"${title}" kartı bulunamadı`);
  return card as HTMLElement;
}

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) throw new Error(`#${id} yok`);
  return element as T;
}

/** Ekrandaki tüm bildirim/uyarı kutularının metni (bayat bildirim ölçümü). */
function notices(): string[] {
  return Array.from(document.querySelectorAll(".notice")).map((node) => node.textContent ?? "");
}

function radioRow(prefix: string): HTMLElement {
  const node = screen.getByText(prefix, { exact: false });
  const row = node.closest("label");
  if (!row) throw new Error(`"${prefix}" satırı bulunamadı`);
  return row as HTMLElement;
}

describe("ayarlar ekranı davranışı", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    localStorage.clear();
  });

  afterEach(() => {
    cleanup();
  });

  it("kasa kilitliyken künye, geri dönüş, kuyruk ve bütçe kilitli yazar; tarama istenmez", async () => {
    setup({ status: LOCKED });

    expect(await screen.findByText(t("settings.vaultLockedBadge"))).toBeTruthy();
    expect(screen.getByText(t("settings.vaultLockedShort"))).toBeTruthy();
    expect(screen.queryByText(t("settings.vaultOpenBadge"))).toBeNull();

    // Künye uydurulmaz: kimlik/konum/şema yerine "—"/"bilinmiyor".
    const vault = cardOf(t("settings.vaultTitle"));
    const values = Array.from(vault.querySelectorAll("dd")).map((dd) => dd.textContent);
    expect(values.slice(0, 3)).toEqual(["—", "—", t("common.unknown")]);

    const rollback = cardOf(t("settings.rollbackTitle"));
    expect(within(rollback).getByText(t("settings.rollbackLocked"))).toBeTruthy();
    const scan = within(rollback).getByRole("button", { name: t("settings.scanBackups") }) as HTMLButtonElement;
    expect(scan.disabled).toBe(true);
    expect(mocks.vaultPreUpdateBackups).not.toHaveBeenCalled();

    const backup = cardOf(t("settings.backupTitle"));
    expect(within(backup).getByText(t("settings.backupLocked"))).toBeTruthy();
    const backupButton = within(backup).getByRole("button", { name: t("settings.backupPick") }) as HTMLButtonElement;
    expect(backupButton.disabled).toBe(true);

    expect(screen.getByText(t("jobQueue.locked"))).toBeTruthy();
    expect(screen.getByText(t("workerBudget.locked"))).toBeTruthy();
    expect(mocks.jobQueueStatus).not.toHaveBeenCalled();
    expect(mocks.workerBudgets).not.toHaveBeenCalled();
  });

  it("açık kasa künyesini ve sürümü paketten yazar; yedekleri tarar ve ilk kullanılabilir yedeği seçer", async () => {
    setup();

    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();
    expect(mocks.vaultPreUpdateBackups).toHaveBeenCalledTimes(1);

    const vault = cardOf(t("settings.vaultTitle"));
    expect(vault.textContent).toContain(UNLOCKED.vaultId!);
    expect(vault.textContent).toContain(UNLOCKED.vaultPath!);
    expect(vault.textContent).toContain("23");
    // Sürüm gömülü değil, `package.json`tan.
    expect(vault.textContent).toContain(t("settings.versionValue", { version: appVersion }));
    expect(vault.textContent).not.toContain("0.2.0");

    const rollback = cardOf(t("settings.rollbackTitle"));
    const radios = within(rollback).getAllByRole("radio") as HTMLInputElement[];
    expect(radios).toHaveLength(2);
    expect(radios[0]!.checked).toBe(true);
    expect(radios[1]!.checked).toBe(false);
    // Kullanılamayan yedeğin radyosu kapalı.
    expect(radios[1]!.disabled).toBe(true);

    // Sonuçların ne yapacağı yazılı; onaysız düğme açılmıyor.
    for (const line of rollbackConsequences()) {
      expect(within(rollback).getByText(line)).toBeTruthy();
    }
    const submit = within(rollback).getByRole("button", { name: t("settings.rollbackSubmit") }) as HTMLButtonElement;
    expect(submit.disabled).toBe(true);
    expect(
      within(rollback).getByText(t("rollback.confirmRequired", { token: t("rollback.confirmToken") })),
    ).toBeTruthy();
  });

  it("yedek satırı doğrulanmış sürümü, durumu ve ad uyuşmazlığı uyarısını yazar", async () => {
    setup({ backups: [B_RENAMED, B_BAD] });

    expect(await screen.findByText(B_RENAMED.fileName)).toBeTruthy();

    const good = radioRow(B_RENAMED.fileName);
    expect(good.textContent).toContain(backupVersionLabel(B_RENAMED));
    expect(backupVersionLabel(B_RENAMED)).toBe("v21");
    expect(good.textContent).toContain(t("rollback.usable"));
    expect(good.textContent).toContain(backupStatusText(B_RENAMED));
    expect(good.textContent).toContain(formatSize(B_RENAMED.sizeBytes));
    // Tarih yoksa uydurulmaz.
    expect(good.textContent).toContain(t("dates.missing"));
    expect(good.textContent).toContain(backupWarnings(B_RENAMED)[0]);

    const bad = radioRow(B_BAD.fileName);
    expect(bad.textContent).toContain(t("rollback.unusable"));
    expect(bad.textContent).toContain(backupStatusText(B_BAD));
    expect(backupStatusText(B_BAD)).toContain(B_BAD.problem!);
    expect((within(bad).getByRole("radio") as HTMLInputElement).disabled).toBe(true);
  });

  it("seçilebilir yedek yokken geri dönüş düğmesi kapalı ve gerekçesi yazılı", async () => {
    setup({ backups: [B_BAD] });

    expect(await screen.findByText(B_BAD.fileName)).toBeTruthy();
    const rollback = cardOf(t("settings.rollbackTitle"));
    const submit = within(rollback).getByRole("button", { name: t("settings.rollbackSubmit") }) as HTMLButtonElement;
    expect(submit.disabled).toBe(true);
    expect(within(rollback).getByText(t("rollback.selectFirst"))).toBeTruthy();

    // Doğru belirteç yazılsa da seçim yoksa düğme açılmaz ve istek gitmez.
    fireEvent.change(byId("rollback-confirm"), { target: { value: t("rollback.confirmToken") } });
    expect(submit.disabled).toBe(true);
    fireEvent.click(submit);
    expect(mocks.vaultRollbackPreUpdate).not.toHaveBeenCalled();
  });

  it("geri dönüş seçilen dosya ve onayla gider; onay temizlenir, başarı paneli liste boşalsa da görünür", async () => {
    setup({ backups: [B_OK] });
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    // Geri dönüş sonrası çekirdek kullanılan yedeği listeden düşürür.
    mocks.vaultPreUpdateBackups.mockResolvedValue([]);
    mocks.vaultRollbackPreUpdate.mockResolvedValue(ROLLBACK_RESULT);

    fireEvent.change(byId("rollback-confirm"), { target: { value: t("rollback.confirmToken") } });
    const rollback = cardOf(t("settings.rollbackTitle"));
    const submit = within(rollback).getByRole("button", { name: t("settings.rollbackSubmit") }) as HTMLButtonElement;
    expect(submit.disabled).toBe(false);
    fireEvent.click(submit);

    await waitFor(() =>
      expect(mocks.vaultRollbackPreUpdate).toHaveBeenCalledWith(B_OK.fileName, t("rollback.confirmToken")),
    );
    // Durum ve liste tazelenir; onay alanı temizlenir.
    await waitFor(() => expect(mocks.status).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(mocks.vaultPreUpdateBackups).toHaveBeenCalledTimes(2));
    // Liste boşaldı: radyo satırı kalmadı (sonuç paneli yedek adını taşımaya devam eder).
    await waitFor(() =>
      expect(within(cardOf(t("settings.rollbackTitle"))).queryAllByRole("radio")).toHaveLength(0),
    );
    expect(document.getElementById("rollback-confirm")).toBeNull();
    expect(within(cardOf(t("settings.rollbackTitle"))).getByText(t("settings.rollbackEmpty"))).toBeTruthy();

    // ÖLÇÜLEN KUSUR (A): panel liste boşken kaybolmamalı.
    const text = notices().join("\n");
    expect(text).toContain(t("settings.rollbackDoneTitle"));
    expect(text).toContain(rollbackSummary(ROLLBACK_RESULT));
    expect(text).toContain(ROLLBACK_RESULT.backupFile);
    expect(text).toContain(ROLLBACK_RESULT.safetyCopy);
  });

  it("geri dönüş sonrası liste dolu kalırsa onay alanı boş gelir", async () => {
    setup({ backups: [B_OK] });
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    // Tazeleme başka bir kullanılabilir yedekle döner: onay alanı yine de boş.
    mocks.vaultPreUpdateBackups.mockResolvedValue([B_RENAMED]);
    mocks.vaultRollbackPreUpdate.mockResolvedValue(ROLLBACK_RESULT);

    fireEvent.change(byId("rollback-confirm"), { target: { value: t("rollback.confirmToken") } });
    fireEvent.click(
      within(cardOf(t("settings.rollbackTitle"))).getByRole("button", {
        name: t("settings.rollbackSubmit"),
      }),
    );

    await waitFor(() =>
      expect(within(cardOf(t("settings.rollbackTitle"))).getAllByRole("radio")).toHaveLength(1),
    );
    expect(screen.getByText(B_RENAMED.fileName)).toBeTruthy();
    expect(screen.queryAllByText(B_OK.fileName, { selector: "label span" })).toHaveLength(0);
    expect(byId<HTMLInputElement>("rollback-confirm").value).toBe("");
    const submit = within(cardOf(t("settings.rollbackTitle"))).getByRole("button", {
      name: t("settings.rollbackSubmit"),
    }) as HTMLButtonElement;
    expect(submit.disabled).toBe(true);
    expect(
      within(cardOf(t("settings.rollbackTitle"))).getByText(
        t("rollback.confirmRequired", { token: t("rollback.confirmToken") }),
      ),
    ).toBeTruthy();
    expect(notices().join("\n")).toContain(t("settings.rollbackDoneTitle"));
  });

  it("geri dönüş düşerse hata yazılır, başarı paneli kalmaz, kasa durumu yeniden okunur", async () => {
    setup({ backups: [B_OK] });
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    // Geri dönüş başarısız ve kasa kilitli kaldı: liste/onay temizlenmeli.
    mocks.status.mockResolvedValue(LOCKED);
    mocks.vaultRollbackPreUpdate.mockRejectedValue({
      code: "E_DB_ROLLBACK_REOPEN",
      message: "Geri dönüş sonrası kasa açılamadı.",
      retryable: false,
    });

    fireEvent.change(byId("rollback-confirm"), { target: { value: t("rollback.confirmToken") } });
    fireEvent.click(
      within(cardOf(t("settings.rollbackTitle"))).getByRole("button", {
        name: t("settings.rollbackSubmit"),
      }),
    );

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DB_ROLLBACK_REOPEN");
    expect(alert.textContent).toContain("Geri dönüş sonrası kasa açılamadı.");
    expect(notices().join("\n")).not.toContain(t("settings.rollbackDoneTitle"));
    expect(mocks.status).toHaveBeenCalledTimes(2);

    await waitFor(() => expect(screen.getByText(t("settings.vaultLockedBadge"))).toBeTruthy());
    const rollback = cardOf(t("settings.rollbackTitle"));
    expect(within(rollback).getByText(t("settings.rollbackLocked"))).toBeTruthy();
    expect(within(rollback).queryAllByRole("radio")).toHaveLength(0);
    expect(rollback.textContent).not.toContain(B_OK.fileName);
  });

  it("yedek al: klasör seçilmezse istek gitmez; sonuç künyesi ve tek dosya arşivi yazılır", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    const backupCard = cardOf(t("settings.backupTitle"));
    const button = within(backupCard).getByRole("button", { name: t("settings.backupPick") });

    // İptal: istek yok, bildirim yok.
    fireEvent.click(button);
    await waitFor(() => expect(dialog.open).toHaveBeenCalledTimes(1));
    expect(mocks.vaultBackup).not.toHaveBeenCalled();
    expect(notices().join("\n")).not.toContain(t("settings.backupReady"));

    dialog.open.mockResolvedValue("/Volumes/Yedek");
    mocks.vaultBackup.mockResolvedValue({ ...BACKUP_RESULT, archivePath: "/Volumes/Yedek/kasa.zip" });
    fireEvent.click(button);

    await waitFor(() => expect(mocks.vaultBackup).toHaveBeenCalledWith("/Volumes/Yedek"));
    const text = notices().join("\n");
    expect(text).toContain(t("settings.backupReady"));
    expect(text).toContain(BACKUP_RESULT.path);
    expect(
      text,
    ).toContain(
      t("settings.backupSummary", {
        count: BACKUP_RESULT.fileCount,
        size: formatSize(BACKUP_RESULT.totalBytes),
        hash: BACKUP_RESULT.manifestHash.slice(0, 16),
      }),
    );
    expect(text).toContain(t("settings.backupArchive"));
    expect(text).toContain("/Volumes/Yedek/kasa.zip");
  });

  it("yedek alma düşerse önceki 'yok hazır' bildirimi ekranda kalmaz", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    const backupCard = cardOf(t("settings.backupTitle"));
    const button = within(backupCard).getByRole("button", { name: t("settings.backupPick") });

    dialog.open.mockResolvedValue("/Volumes/Yedek");
    mocks.vaultBackup.mockResolvedValue(BACKUP_RESULT);
    fireEvent.click(button);
    await waitFor(() => expect(notices().join("\n")).toContain(BACKUP_RESULT.path));

    mocks.vaultBackup.mockRejectedValue({
      code: "E_DISK_FULL",
      message: "Disk dolu.",
      retryable: false,
    });
    fireEvent.click(button);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DISK_FULL");
    // ÖLÇÜLEN KUSUR (B): başarısız denemeden sonra eski "Yedek hazır" kalmamalı.
    const text = notices().join("\n");
    expect(text).not.toContain(t("settings.backupReady"));
    expect(text).not.toContain(BACKUP_RESULT.path);
  });

  it("otomatik yedek ayarları yerel kayıttan okunur; kapat/sıfırla yalnız kendi anahtarını siler", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/Volumes/OtoYedek");
    localStorage.setItem(BACKUP_DAYS_KEY, "3");
    localStorage.setItem(BACKUP_LAST_KEY, "2026-10-28T06:00:00.000Z");
    setup();

    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();
    const auto = cardOf(t("settings.autoTitle"));
    expect(auto.textContent).toContain("/Volumes/OtoYedek");
    expect(auto.textContent).toContain(t("settings.autoDaysValue", { days: 3 }));
    expect(auto.textContent).toContain(formatDateTime("2026-10-28T06:00:00.000Z"));

    const disable = within(auto).getByRole("button", { name: t("settings.autoDisable") }) as HTMLButtonElement;
    expect(disable.disabled).toBe(false);
    fireEvent.click(disable);
    expect(localStorage.getItem(BACKUP_DIR_KEY)).toBeNull();
    // Aralık ve son yedek kaydı korunur.
    expect(localStorage.getItem(BACKUP_DAYS_KEY)).toBe("3");
    expect(localStorage.getItem(BACKUP_LAST_KEY)).not.toBeNull();
    expect(cardOf(t("settings.autoTitle")).textContent).toContain(t("settings.autoDirNone"));

    const reset = within(cardOf(t("settings.autoTitle"))).getByRole("button", {
      name: t("settings.autoReset"),
    }) as HTMLButtonElement;
    expect(reset.disabled).toBe(false);
    fireEvent.click(reset);
    expect(localStorage.getItem(BACKUP_LAST_KEY)).toBeNull();
    expect(cardOf(t("settings.autoTitle")).textContent).toContain(t("settings.autoLastNone"));
  });

  it("gc tarama adayları yazar, temizleme aday yokken kapalıdır, silme sonucu ve eksik dosya yazılır", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    const gcCard = cardOf(t("settings.gcTitle"));
    const clean = within(gcCard).getByRole("button", { name: t("settings.gcClean") }) as HTMLButtonElement;
    expect(clean.disabled).toBe(true);
    expect(within(gcCard).getByText(t("settings.gcHint"))).toBeTruthy();

    mocks.vaultGc.mockResolvedValueOnce(GC_SCAN);
    fireEvent.click(within(gcCard).getByRole("button", { name: t("settings.gcScan") }));

    const scanned = await screen.findByText(
      t("settings.gcCandidates", {
        count: GC_SCAN.candidates.length,
        size: formatSize(1_048_576 + 524_288),
      }),
    );
    expect(scanned).toBeTruthy();
    expect(mocks.vaultGc).toHaveBeenNthCalledWith(1, true);
    expect(
      (within(cardOf(t("settings.gcTitle"))).getByRole("button", {
        name: t("settings.gcClean"),
      }) as HTMLButtonElement).disabled,
    ).toBe(false);

    mocks.vaultGc.mockResolvedValueOnce(GC_CLEAN);
    fireEvent.click(
      within(cardOf(t("settings.gcTitle"))).getByRole("button", { name: t("settings.gcClean") }),
    );

    await waitFor(() => expect(mocks.vaultGc).toHaveBeenNthCalledWith(2, false));
    const text = notices().join("\n");
    expect(text).toContain(
      t("settings.gcDeleted", { count: 1, size: formatSize(GC_CLEAN.freedBytes) }),
    );
    expect(text).toContain(t("settings.gcMissing", { files: "blob-7.bin" }));
  });

  it("gc düşerse önceki tarama/silme bildirimi ekranda kalmaz", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    mocks.vaultGc.mockResolvedValueOnce(GC_SCAN);
    fireEvent.click(
      within(cardOf(t("settings.gcTitle"))).getByRole("button", { name: t("settings.gcScan") }),
    );
    const scanText = t("settings.gcCandidates", {
      count: GC_SCAN.candidates.length,
      size: formatSize(1_048_576 + 524_288),
    });
    await waitFor(() => expect(notices().join("\n")).toContain(scanText));

    mocks.vaultGc.mockRejectedValueOnce({
      code: "E_IO",
      message: "Blob silinemedi.",
      retryable: true,
    });
    fireEvent.click(
      within(cardOf(t("settings.gcTitle"))).getByRole("button", { name: t("settings.gcClean") }),
    );

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_IO");
    // ÖLÇÜLEN KUSUR (C): düşen işlem önceki "N aday" bildirimini bırakmamalı.
    expect(notices().join("\n")).not.toContain(scanText);
    expect(within(cardOf(t("settings.gcTitle"))).getByText(t("settings.gcHint"))).toBeTruthy();
  });

  it("geri yükleme hedef konum ister; iptal edilen seçim istek üretmez, hedef kırpılarak gider", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    const restore = cardOf(t("settings.restoreTitle"));
    const button = within(restore).getByRole("button", { name: t("settings.restorePick") }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);

    fireEvent.change(byId("restore-dest"), { target: { value: "   " } });
    expect(button.disabled).toBe(true);

    fireEvent.change(byId("restore-dest"), { target: { value: "  /tmp/yeni-kasa  " } });
    expect(button.disabled).toBe(false);

    // İptal: ne istek ne "seçilen yedek" satırı.
    fireEvent.click(button);
    await waitFor(() => expect(dialog.open).toHaveBeenCalledTimes(1));
    expect(mocks.vaultRestore).not.toHaveBeenCalled();
    expect(screen.queryByText(t("settings.restoreSelected", { dir: "/tmp/yedek-yolu" }))).toBeNull();

    dialog.open.mockResolvedValue("/tmp/yedek-yolu");
    mocks.vaultRestore.mockResolvedValue(RESTORE_RESULT);
    fireEvent.click(button);

    await waitFor(() =>
      expect(mocks.vaultRestore).toHaveBeenCalledWith("/tmp/yedek-yolu", "/tmp/yeni-kasa"),
    );
    expect(
      screen.getByText(t("settings.restoreSelected", { dir: "/tmp/yedek-yolu" })),
    ).toBeTruthy();
  });

  it("geri yükleme sonucu doğrulanan dosya/boyut/süre ve sayılmayan dosyaları yazar", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    fireEvent.change(byId("restore-dest"), { target: { value: "/tmp/yeni-kasa" } });
    dialog.open.mockResolvedValue("/tmp/yedek-yolu");
    mocks.vaultRestore.mockResolvedValue(RESTORE_RESULT);
    fireEvent.click(
      within(cardOf(t("settings.restoreTitle"))).getByRole("button", {
        name: t("settings.restorePick"),
      }),
    );

    await waitFor(() => expect(mocks.vaultRestore).toHaveBeenCalled());
    const text = notices().join("\n");
    expect(text).toContain(t("settings.restoreDone"));
    expect(text).toContain(RESTORE_RESULT.vaultPath);
    expect(text).toContain(t("settings.restoreVerifiedLead"));
    expect(text).toContain(String(RESTORE_RESULT.verifiedFiles));
    expect(
      text,
    ).toContain(
      t("settings.restoreVerifiedTail", {
        size: formatSize(RESTORE_RESULT.verifiedBytes),
        ms: RESTORE_RESULT.verifyMs,
      }),
    );
    // 6 dosya: ilk beşi yazılır, kırpıldığı "…" ile belli edilir.
    expect(text).toContain(
      t("settings.restoreUnlisted", { count: RESTORE_RESULT.unlistedBlobs.length }),
    );
    expect(text).toContain("a.bin, b.bin, c.bin, d.bin, e.bin");
    expect(text).not.toContain("f.bin");
    expect(text).toContain("…");
  });

  it("geri yükleme düşerse önceki tamamlanma bildirimi ekranda kalmaz", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    fireEvent.change(byId("restore-dest"), { target: { value: "/tmp/yeni-kasa" } });
    dialog.open.mockResolvedValue("/tmp/yedek-yolu");
    mocks.vaultRestore.mockResolvedValue(RESTORE_RESULT);
    fireEvent.click(
      within(cardOf(t("settings.restoreTitle"))).getByRole("button", {
        name: t("settings.restorePick"),
      }),
    );
    await waitFor(() => expect(notices().join("\n")).toContain(RESTORE_RESULT.vaultPath));

    mocks.vaultRestore.mockRejectedValue({
      code: "E_BACKUP_VERIFY",
      message: "Yedek manifesti doğrulanamadı.",
      retryable: false,
    });
    fireEvent.click(
      within(cardOf(t("settings.restoreTitle"))).getByRole("button", {
        name: t("settings.restorePick"),
      }),
    );

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_BACKUP_VERIFY");
    // ÖLÇÜLEN KUSUR (D): başarısız geri yükleme eski "tamamlandı" panelini bırakmamalı.
    const text = notices().join("\n");
    expect(text).not.toContain(t("settings.restoreDone"));
    expect(text).not.toContain(RESTORE_RESULT.vaultPath);
  });

  it("parola: kısa/eşleşmeyen parola çekirdeğe gitmez ve önceki başarı bildirimi silinir", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    const card = cardOf(t("settings.passwordTitle"));
    const submit = within(card).getByRole("button", { name: t("settings.passwordSubmit") }) as HTMLButtonElement;
    const oldInput = byId<HTMLInputElement>("pw-old");
    const newInput = byId<HTMLInputElement>("pw-new");
    const new2Input = byId<HTMLInputElement>("pw-new2");

    expect(submit.disabled).toBe(true);
    fireEvent.change(newInput, { target: { value: "123456789" } });
    expect(submit.disabled).toBe(true); // 9 karakter + mevcut parola boş
    fireEvent.change(oldInput, { target: { value: "eski-parola" } });
    expect(submit.disabled).toBe(true);
    fireEvent.change(newInput, { target: { value: "1234567890" } });
    expect(submit.disabled).toBe(false);

    // Eşleşmezse çekirdeğe gitmez.
    fireEvent.change(new2Input, { target: { value: "1234567891" } });
    fireEvent.click(submit);
    const mismatch = await screen.findByRole("alert");
    expect(mismatch.textContent).toContain("E_PASSWORD_MISMATCH");
    expect(mismatch.textContent).toContain(t("settings.passwordMismatch"));
    expect(mocks.vaultChangePassword).not.toHaveBeenCalled();

    // Başarılı değişim alanları temizler ve yeşil bildirim yazar.
    mocks.vaultChangePassword.mockResolvedValue(undefined);
    fireEvent.change(new2Input, { target: { value: "1234567890" } });
    fireEvent.click(within(cardOf(t("settings.passwordTitle"))).getByRole("button", {
      name: t("settings.passwordSubmit"),
    }));
    await waitFor(() =>
      expect(mocks.vaultChangePassword).toHaveBeenCalledWith("eski-parola", "1234567890"),
    );
    await waitFor(() => expect(byId<HTMLInputElement>("pw-old").value).toBe(""));
    expect(byId<HTMLInputElement>("pw-new").value).toBe("");
    expect(byId<HTMLInputElement>("pw-new2").value).toBe("");
    expect(notices().join("\n")).toContain(t("settings.passwordDone"));

    // ÖLÇÜLEN KUSUR (E): yeni bir eşleşmeyen deneme eski yeşil bildirimi bırakmamalı.
    fireEvent.change(byId<HTMLInputElement>("pw-old"), { target: { value: "eski-parola" } });
    fireEvent.change(byId<HTMLInputElement>("pw-new"), { target: { value: "1234567890" } });
    fireEvent.change(byId<HTMLInputElement>("pw-new2"), { target: { value: "farkli-parola" } });
    fireEvent.click(
      within(cardOf(t("settings.passwordTitle"))).getByRole("button", {
        name: t("settings.passwordSubmit"),
      }),
    );
    await waitFor(() =>
      expect(notices().join("\n")).not.toContain(t("settings.passwordDone")),
    );
    expect(mocks.vaultChangePassword).toHaveBeenCalledTimes(1);
  });

  it("parola değişimi düşerse yazılanlar korunur ve hata kodu yazılır", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    fireEvent.change(byId("pw-old"), { target: { value: "yanlis-parola" } });
    fireEvent.change(byId("pw-new"), { target: { value: "1234567890" } });
    fireEvent.change(byId("pw-new2"), { target: { value: "1234567890" } });
    mocks.vaultChangePassword.mockRejectedValue({
      code: "E_VAULT_PASSWORD",
      message: "Mevcut parola doğrulanamadı.",
      retryable: false,
    });

    fireEvent.click(
      within(cardOf(t("settings.passwordTitle"))).getByRole("button", {
        name: t("settings.passwordSubmit"),
      }),
    );

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_VAULT_PASSWORD");
    expect(alert.textContent).toContain("Mevcut parola doğrulanamadı.");
    // Başarısız denemede kullanıcı girdisi silinmez.
    expect(byId<HTMLInputElement>("pw-old").value).toBe("yanlis-parola");
    expect(byId<HTMLInputElement>("pw-new").value).toBe("1234567890");
    expect(notices().join("\n")).not.toContain(t("settings.passwordDone"));
  });
});
