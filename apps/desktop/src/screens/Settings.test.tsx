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
//
// Uçuş yolları (2026-11-04, F16-T03 ek turu):
// 19. **Ölçülen kusur (F):** onay metni seçilen yedeğe bağlı değildi — bir yedek
//     için yazılan "GERİ DÖN" başka yedeğe geçilince de düğmeyi açık tutuyor,
//     tek tıkla **başka** yedek geri konabiliyordu. Seçim değişince metin düşer.
// 20. **Ölçülen kusur (G):** "Yedekleri tara" seçimi (ilk kullanılabilir yedek)
//     değiştirse de yazılan onay kalıyordu; aynı kapı taramada da geçerli.
// 21. **Ölçülen kusur (H):** geri dönüş başarılı olup liste tazelenemezse eski
//     satırlar seçilebilir kalıyordu (kullanılmış yedek yeniden hedef olabilir);
//     liste fail-closed boşaltılır, "tamamlandı" paneli korunur.
// 22. **Ölçülen kusur (I):** parola değişimi uçuşta iken yazılan yeni değerler
//     başarı yanıtıyla siliniyordu; artık yalnız gönderilen değerler temizlenir.
// 23. **Ölçülen kusur (J):** parola başarı satırı alan değişince kalkmıyordu.
// 24. **Ölçülen kusur (K):** açık kasada yedek kartı "Kasa kilitliyken yedek
//     alınamaz" yazıyordu (durum yalanı).
// 25. **Ölçülen kusur (L):** klasör seçici açılamazsa ekranda hiçbir şey
//     olmuyordu (sessiz başarısızlık); artık E_DIALOG yazılır, istek gitmez.


import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { version as appVersion } from "../../package.json";
import {
  BACKUP_ATTEMPT_KEY,
  BACKUP_DAYS_KEY,
  BACKUP_DIR_KEY,
  BACKUP_LAST_KEY,
  DEFAULT_BACKUP_DAYS,
  readBackupForce,
  writeBackupAttempt,
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
  AuditChainStatus,
  AuditEventRow,
  BackupResult,
  BackupRunRow,
  JobQueueView,
  PreUpdateBackup,
  RestoreResult,
  RollbackResult,
  SettingRow,
  SystemInfo,
  VaultGcResult,
  VaultStatus,
  WorkerBudgetProfile,
} from "../lib/api";

const mocks = vi.hoisted(() => ({
  status: vi.fn(),
  vaultPreUpdateBackups: vi.fn(),
  vaultRollbackPreUpdate: vi.fn(),
  vaultBackup: vi.fn(),
  backupRunList: vi.fn(),
  vaultSettingGet: vi.fn(),
  vaultSettingSet: vi.fn(),
  vaultSettingDelete: vi.fn(),
  vaultGc: vi.fn(),
  vaultRestore: vi.fn(),
  vaultChangePassword: vi.fn(),
  jobQueueStatus: vi.fn(),
  workerBudgets: vi.fn(),
  auditEventList: vi.fn(),
  auditChainCheck: vi.fn(),
  systemInfo: vi.fn(),
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
  historyRecorded: true,
  historyError: null,
};

/** Yedek geçmişi satırları: biri doğrulanmış, biri tanınmayan durum taşır. */
const HISTORY_VERIFIED: BackupRunRow = {
  id: "run-1",
  snapshotId: "DimYedek-20261107-1830",
  schemaVersion: 24,
  manifestHash: "abcdef0123456789abcdef",
  status: "verified",
  verificationResult: "integrity_check",
  destinationLabel: "/Volumes/Yedek",
  createdAt: ISO,
  completedAt: ISO,
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
  audit?: AuditEventRow[];
  auditChain?: AuditChainStatus;
  /** Yedek geçmişi (varsayılan: boş ama **başarılı** okuma). */
  history?: BackupRunRow[];
  /** İlk geçmiş okuması düşsün (senaryo: "kayıt yok" denmemeli). */
  historyError?: { code: string; message: string; retryable: boolean };
  /** Teşhis kartının çekirdekten okuyacağı künye (varsayılan: paket sürümüyle aynı). */
  core?: SystemInfo;
  /** Kasa ayarındaki otomatik yedek planı (varsayılan: kayıt yok → göç/önbellek). */
  scheduleRow?: SettingRow | null;
};

function setup(options: SetupOptions = {}) {
  mocks.status.mockResolvedValue(options.status ?? UNLOCKED);
  mocks.vaultPreUpdateBackups.mockResolvedValue(options.backups ?? [B_OK, B_BAD]);
  mocks.jobQueueStatus.mockResolvedValue(QUEUE_EMPTY);
  mocks.workerBudgets.mockResolvedValue(options.budgets ?? []);
  // Denetim defteri varsayılanı: boş ama **başarılı** okuma (ekran "kayıt yok"
  // yazabilir). Testler kendi senaryosunda ezer.
  mocks.auditEventList.mockResolvedValue(options.audit ?? []);
  mocks.auditChainCheck.mockResolvedValue(
    options.auditChain ?? {
      checked: 0,
      intact: true,
      brokenEventId: null,
      brokenReason: null,
    },
  );
  // Teşhis: varsayılan **aynı** sürüm (uyarı yok); senaryolar `core` ile ezer.
  mocks.systemInfo.mockResolvedValue(
    options.core ?? { appVersion, protocolVersion: 1, platform: "macos" },
  );
  // Yedek geçmişi: varsayılan boş ve **başarılı** (ekran "kayıt yok" diyebilir).
  if (options.historyError) {
    mocks.backupRunList.mockRejectedValue(options.historyError);
  } else {
    mocks.backupRunList.mockResolvedValue(options.history ?? []);
  }
  // Otomatik yedek planı kasa ayarından okunur; varsayılan **kayıt yok**.
  // Yazım, geri okunan satırı döndürür (doğrulama yolu gerçek).
  mocks.vaultSettingGet.mockResolvedValue(options.scheduleRow ?? null);
  mocks.vaultSettingSet.mockImplementation((key: string, value: unknown) =>
    Promise.resolve({ scope: "vault", key, value, version: 1, updatedAt: ISO }),
  );
  mocks.vaultSettingDelete.mockResolvedValue(true);
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

  it("otomatik yedek planı kasadan okunur; kapat planı ve önbelleği birlikte siler", async () => {
    localStorage.setItem(BACKUP_LAST_KEY, "2026-10-28T06:00:00.000Z");
    setup({
      scheduleRow: {
        scope: "vault",
        key: "backup.schedule",
        value: { destination: "/Volumes/OtoYedek", days: 3 },
        version: 2,
        updatedAt: ISO,
      },
      // Tazelik kanıtı kasadadır; cihaz kaydı ayrı (daha eski) zamandır.
      history: [
        {
          id: "run-1",
          snapshotId: "DimYedek-20261102-0900",
          schemaVersion: 24,
          manifestHash: "e".repeat(64),
          status: "verified",
          verificationResult: "integrity_check",
          destinationLabel: "/Volumes/OtoYedek",
          createdAt: "2026-11-02T09:00:00.000Z",
          completedAt: "2026-11-02T09:01:00.000Z",
        },
      ],
    });

    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();
    const auto = cardOf(t("settings.autoTitle"));
    expect(auto.textContent).toContain("/Volumes/OtoYedek");
    expect(auto.textContent).toContain(t("settings.autoDaysValue", { days: 3 }));
    // "Son yedek" **kasa kaydından** yazılır ve kaynağını söyler.
    expect(auto.textContent).toContain(
      t("settings.autoLastVault", { time: formatDateTime("2026-11-02T09:01:00.000Z") }),
    );
    // Cihaz kaydı farklıysa çelişki gizlenmez (ayrı satırda yazılır).
    expect(auto.textContent).toContain(
      t("settings.autoLastDeviceMismatch", { time: formatDateTime("2026-10-28T06:00:00.000Z") }),
    );
    // Plan kasadan geldi: okuma hatası/uyarısı yok.
    expect(within(auto).queryByRole("alert")).toBeNull();

    // "Sıfırla" **plan dururken** denenir: kasa kaydı silinmez, cihaz
    // önbelleği temizlenir ve sonraki kontrol için zorunlu yedek işaretlenir.
    const reset = within(auto).getByRole("button", { name: t("settings.autoReset") }) as HTMLButtonElement;
    expect(reset.disabled).toBe(false);
    fireEvent.click(reset);
    expect(localStorage.getItem(BACKUP_LAST_KEY)).toBeNull();
    expect(readBackupForce()).toBe(true);
    expect(cardOf(t("settings.autoTitle")).textContent).toContain(
      t("settings.autoLastVault", { time: formatDateTime("2026-11-02T09:01:00.000Z") }),
    );
    expect(cardOf(t("settings.autoTitle")).textContent).toContain(t("settings.autoForceQueued"));

    const disable = within(cardOf(t("settings.autoTitle"))).getByRole("button", {
      name: t("settings.autoDisable"),
    }) as HTMLButtonElement;
    expect(disable.disabled).toBe(false);
    fireEvent.click(disable);
    await waitFor(() => expect(mocks.vaultSettingDelete).toHaveBeenCalledWith("backup.schedule"));
    expect(cardOf(t("settings.autoTitle")).textContent).toContain(t("settings.autoDirNone"));
    // "Kapat" yalnız planı siler; son yedek kaydı korunur.
    expect(localStorage.getItem(BACKUP_LAST_KEY)).toBeNull();
    expect(readBackupForce()).toBe(true);
  });

  it("klasör seçimi planı kasaya yazar; cihaz kaydı yalnız doğrulanmış önbellektir", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();
    const auto = cardOf(t("settings.autoTitle"));
    dialog.open.mockResolvedValue("/Volumes/YeniYedek");
    fireEvent.click(within(auto).getByRole("button", { name: t("settings.autoPick") }));

    await waitFor(() =>
      expect(mocks.vaultSettingSet).toHaveBeenCalledWith("backup.schedule", {
        destination: "/Volumes/YeniYedek",
        days: DEFAULT_BACKUP_DAYS,
      }),
    );
    // Ekran **geri okunan** değeri yazar; seçimden bağımsız "etkin" denmez.
    expect(cardOf(t("settings.autoTitle")).textContent).toContain("/Volumes/YeniYedek");
    expect(localStorage.getItem(BACKUP_DIR_KEY)).toBe("/Volumes/YeniYedek");
    expect(localStorage.getItem(BACKUP_DAYS_KEY)).toBe(String(DEFAULT_BACKUP_DAYS));
    expect(within(cardOf(t("settings.autoTitle"))).queryByRole("alert")).toBeNull();
  });

  it("kasa ayarı yazılamazsa plan etkin sayılmaz: klasör seçilmedi kalır, neden yazılır", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();
    const auto = cardOf(t("settings.autoTitle"));
    dialog.open.mockResolvedValue("/Volumes/YeniYedek");
    mocks.vaultSettingSet.mockRejectedValue({
      code: "E_IO",
      message: "hedef disk yok",
      retryable: true,
    });
    fireEvent.click(within(auto).getByRole("button", { name: t("settings.autoPick") }));

    await waitFor(() => expect(mocks.vaultSettingSet).toHaveBeenCalled());
    expect(cardOf(t("settings.autoTitle")).textContent).toContain(t("settings.autoDirNone"));
    expect(localStorage.getItem(BACKUP_DIR_KEY)).toBeNull();
    expect(notices().join("\n")).toContain("E_IO");
  });

  it("kasadaki bozuk plan yedek planı sayılmaz; neden görünür, cihaz kaydıyla ezilmez", async () => {
    setup({
      scheduleRow: {
        scope: "vault",
        key: "backup.schedule",
        value: { destination: "   ", days: 0 },
        version: 1,
        updatedAt: ISO,
      },
    });
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();
    const auto = cardOf(t("settings.autoTitle"));
    await waitFor(() =>
      expect(auto.textContent).toContain(
        t("settings.autoSettingInvalid", { code: "E_DB_SETTING" }),
      ),
    );
    expect(auto.textContent).toContain(t("settings.autoDirNone"));
    // Bozuk kasa ayarı "yok" sayılıp cihaz kaydıyla ezilmez.
    expect(mocks.vaultSettingSet).not.toHaveBeenCalled();
  });

  it("cihaz kaydındaki eski plan kasaya bir kez taşınır, eski anahtarlar silinir", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/Volumes/EskiYedek");
    localStorage.setItem(BACKUP_DAYS_KEY, "5");
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    await waitFor(() =>
      expect(mocks.vaultSettingSet).toHaveBeenCalledWith("backup.schedule", {
        destination: "/Volumes/EskiYedek",
        days: 5,
      }),
    );
    const auto = cardOf(t("settings.autoTitle"));
    expect(auto.textContent).toContain(t("settings.autoSettingMigrated"));
    // Taşındıktan sonra eski anahtarlar silinir: bayat değer geri gelmez.
    expect(localStorage.getItem(BACKUP_DIR_KEY)).toBeNull();
    expect(localStorage.getItem(BACKUP_DAYS_KEY)).toBeNull();
  });

  it("son başarılı yedek ile son deneme ayrı gösterilir; başarısız denemenin nedeni yazılır", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/Volumes/OtoYedek");
    localStorage.setItem(BACKUP_DAYS_KEY, "7");
    localStorage.setItem(BACKUP_LAST_KEY, "2026-10-28T06:00:00.000Z");
    writeBackupAttempt({ at: "2026-11-07T02:00:00.000Z", ok: false, code: "E_IO" });
    setup();

    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();
    const auto = cardOf(t("settings.autoTitle"));
    // Son **başarılı** yedek ayrı satırda kalır: başarısız deneme tazeliği ilerletmez.
    expect(auto.textContent).toContain(formatDateTime("2026-10-28T06:00:00.000Z"));
    expect(auto.textContent).toContain(formatDateTime("2026-11-07T02:00:00.000Z"));
    expect(auto.textContent).toContain(t("settings.autoAttemptFailed"));
    expect(auto.textContent).not.toContain(t("settings.autoAttemptNone"));
    expect(auto.textContent).toContain(t("settings.autoNextDue"));
    const alert = within(auto).getByRole("alert");
    expect(alert.textContent).toContain("E_IO");
    expect(alert.textContent).toContain(t("settings.autoAttemptReason", { code: "E_IO" }));
  });

  it("başarılı son deneme 'son yedek' ile karışmaz; sıradaki plan son başarılı yedekten hesaplanır", async () => {
    const last = new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString();
    localStorage.setItem(BACKUP_DIR_KEY, "/Volumes/OtoYedek");
    localStorage.setItem(BACKUP_DAYS_KEY, "7");
    localStorage.setItem(BACKUP_LAST_KEY, last);
    writeBackupAttempt({ at: new Date().toISOString(), ok: true, code: null });
    // Tazelik kanıtı kasadadır: son başarılı yedek geçmişteki doğrulanmış satırdır.
    setup({
      history: [
        {
          id: "run-2",
          snapshotId: "DimYedek-20261105-0900",
          schemaVersion: 24,
          manifestHash: "f".repeat(64),
          status: "verified",
          verificationResult: "integrity_check",
          destinationLabel: "/Volumes/OtoYedek",
          createdAt: last,
          completedAt: last,
        },
      ],
    });

    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();
    const auto = cardOf(t("settings.autoTitle"));
    expect(auto.textContent).toContain(t("settings.autoAttemptOk"));
    expect(within(auto).queryByRole("alert")).toBeNull();
    const nextIso = new Date(Date.parse(last) + 7 * 24 * 60 * 60 * 1000).toISOString();
    expect(auto.textContent).toContain(formatDateTime(nextIso));
    expect(auto.textContent).not.toContain(t("settings.autoNextDue"));
  });

  it("bozuk deneme kaydı uydurulmaz: 'henüz deneme yok' yazılır", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/Volumes/OtoYedek");
    localStorage.setItem(BACKUP_ATTEMPT_KEY, "bozuk");
    setup();

    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();
    const auto = cardOf(t("settings.autoTitle"));
    expect(auto.textContent).toContain(t("settings.autoAttemptNone"));
    expect(within(auto).queryByRole("alert")).toBeNull();
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

  it("kasa geçmişi okunamazsa 'son yedek: hiç' denmez; cihaz kaydı kaynağıyla yazılır", async () => {
    localStorage.setItem(BACKUP_LAST_KEY, "2026-11-06T07:00:00.000Z");
    setup({
      historyError: { code: "E_DB", message: "geçmiş okunamadı", retryable: true },
    });

    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();
    const auto = cardOf(t("settings.autoTitle"));
    await waitFor(() =>
      expect(auto.textContent).toContain(
        t("settings.autoLastDeviceFallback", {
          time: formatDateTime("2026-11-06T07:00:00.000Z"),
          code: "E_DB",
        }),
      ),
    );
    // "Hiç yedek yok" demek yalan olurdu: kayıt var ama kasadan okunamadı.
    expect(auto.textContent).not.toContain(t("settings.autoLastNone"));
  });

  it("kasa kaydı yoksa cihaz kaydı tazelik sayılmaz; çelişki ayrı satırda yazılır", async () => {
    localStorage.setItem(BACKUP_LAST_KEY, "2026-11-06T07:00:00.000Z");
    setup({ history: [] });

    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();
    const auto = cardOf(t("settings.autoTitle"));
    await waitFor(() => expect(auto.textContent).toContain(t("settings.autoLastNone")));
    expect(auto.textContent).toContain(
      t("settings.autoLastDeviceMismatch", { time: formatDateTime("2026-11-06T07:00:00.000Z") }),
    );
  });

  it("kasa kaydı tazeyken sıfırla zorunlu yedek işaretler; plan süresi doldu yazar", async () => {
    const fresh = new Date(Date.now() - 60 * 60 * 1000).toISOString();
    setup({
      scheduleRow: {
        scope: "vault",
        key: "backup.schedule",
        value: { destination: "/Volumes/OtoYedek", days: 7 },
        version: 1,
        updatedAt: ISO,
      },
      history: [
        {
          id: "run-3",
          snapshotId: "DimYedek-20261107-0900",
          schemaVersion: 24,
          manifestHash: "a1".repeat(32),
          status: "verified",
          verificationResult: "integrity_check",
          destinationLabel: "/Volumes/OtoYedek",
          createdAt: fresh,
          completedAt: fresh,
        },
      ],
    });

    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();
    const auto = cardOf(t("settings.autoTitle"));
    // Kasa kaydı taze: plan gelecekte bir tarih gösterir.
    await waitFor(() =>
      expect(auto.textContent).toContain(
        t("settings.autoLastVault", { time: formatDateTime(fresh) }),
      ),
    );
    expect(auto.textContent).not.toContain(t("settings.autoNextDue"));

    fireEvent.click(within(auto).getByRole("button", { name: t("settings.autoReset") }) as HTMLButtonElement);

    expect(readBackupForce()).toBe(true);
    expect(auto.textContent).toContain(t("settings.autoForceQueued"));
    // "Sıfırla" sözü: bir sonraki kontrol yedek alır → plan "süresi doldu" olur.
    expect(auto.textContent).toContain(t("settings.autoNextDue"));
  });
});

/* ------------------------------------------------------- uçuş yolları (F16-T03) */

/** Elle çözülen söz: yanıt askıdayken ekranın ne yaptığını ölçmek için. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function rollbackSubmit(): HTMLButtonElement {
  return within(cardOf(t("settings.rollbackTitle"))).getByRole("button", {
    name: t("settings.rollbackSubmit"),
  }) as HTMLButtonElement;
}

describe("ayarlar ekranı uçuş yolları", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    localStorage.clear();
  });

  afterEach(() => {
    cleanup();
  });

  it("onay metni seçilen yedeğe bağlıdır: hedef değişince yeniden yazılmalı", async () => {
    setup({ backups: [B_OK, B_RENAMED] });
    expect(await screen.findByText(B_RENAMED.fileName)).toBeTruthy();

    // A (ilk kullanılabilir) seçili; onay A için yazıldı.
    fireEvent.change(byId("rollback-confirm"), { target: { value: t("rollback.confirmToken") } });
    expect(rollbackSubmit().disabled).toBe(false);

    // Hedef B'ye çevrilir: yazılan onay B için verilmemiş bir irade beyanıdır.
    fireEvent.click(within(radioRow(B_RENAMED.fileName)).getByRole("radio"));
    expect(byId<HTMLInputElement>("rollback-confirm").value).toBe("");
    expect(rollbackSubmit().disabled).toBe(true);
    fireEvent.click(rollbackSubmit());
    expect(mocks.vaultRollbackPreUpdate).not.toHaveBeenCalled();

    // Yeniden yazılınca istek **B** için gider.
    mocks.vaultRollbackPreUpdate.mockResolvedValue({
      ...ROLLBACK_RESULT,
      backupFile: B_RENAMED.fileName,
    });
    fireEvent.change(byId("rollback-confirm"), { target: { value: t("rollback.confirmToken") } });
    expect(rollbackSubmit().disabled).toBe(false);
    fireEvent.click(rollbackSubmit());
    await waitFor(() =>
      expect(mocks.vaultRollbackPreUpdate).toHaveBeenCalledWith(
        B_RENAMED.fileName,
        t("rollback.confirmToken"),
      ),
    );
  });

  it("yeniden tarama seçimi değiştirirse yazılan onay düşer", async () => {
    setup({ backups: [B_OK] });
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    fireEvent.change(byId("rollback-confirm"), { target: { value: t("rollback.confirmToken") } });
    expect(rollbackSubmit().disabled).toBe(false);

    // Tarama başka bir kullanılabilir yedekle döner: seçim kendiliğinden B olur.
    mocks.vaultPreUpdateBackups.mockResolvedValue([B_RENAMED]);
    fireEvent.click(
      within(cardOf(t("settings.rollbackTitle"))).getByRole("button", {
        name: t("settings.scanBackups"),
      }),
    );

    expect(await screen.findByText(B_RENAMED.fileName)).toBeTruthy();
    expect(byId<HTMLInputElement>("rollback-confirm").value).toBe("");
    expect(rollbackSubmit().disabled).toBe(true);
  });

  it("geri dönüş başarılı olup liste tazelenemezse eski satırlar seçilebilir kalmaz", async () => {
    setup({ backups: [B_OK] });
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    mocks.vaultRollbackPreUpdate.mockResolvedValue(ROLLBACK_RESULT);
    mocks.vaultPreUpdateBackups.mockRejectedValue({
      code: "E_IO",
      message: "Yedek klasörü okunamadı.",
      retryable: true,
    });

    fireEvent.change(byId("rollback-confirm"), { target: { value: t("rollback.confirmToken") } });
    fireEvent.click(rollbackSubmit());

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_IO");
    // Liste bilinmiyor: satır kalmaz, hedef seçilemez, onay boş.
    await waitFor(() =>
      expect(
        within(cardOf(t("settings.rollbackTitle"))).queryAllByRole("radio"),
      ).toHaveLength(0),
    );
    const rollback = cardOf(t("settings.rollbackTitle"));
    // Dosya adı yalnız **sonuç panelinde** (gerçekleşmiş işlemin künyesi) kalır;
    // seçilebilir satır kalmadı.
    expect(rollback.textContent).toContain(t("settings.rollbackEmpty"));
    expect(within(rollback).queryAllByRole("radio")).toHaveLength(0);
    // Liste boşken düğme hiç çizilmez: hedef yok.
    expect(
      within(rollback).queryByRole("button", { name: t("settings.rollbackSubmit") }),
    ).toBeNull();
    // İşlem gerçekten oldu: "tamamlandı" paneli ekranda kalır.
    expect(notices().join("\n")).toContain(t("settings.rollbackDoneTitle"));
  });

  it("parola değişimi sürerken yazılan yeni değerler sessizce silinmez", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    const inFlight = deferred<undefined>();
    mocks.vaultChangePassword.mockReturnValue(inFlight.promise);

    fireEvent.change(byId("pw-old"), { target: { value: "eski-parola" } });
    fireEvent.change(byId("pw-new"), { target: { value: "1234567890" } });
    fireEvent.change(byId("pw-new2"), { target: { value: "1234567890" } });
    fireEvent.click(
      within(cardOf(t("settings.passwordTitle"))).getByRole("button", {
        name: t("settings.passwordSubmit"),
      }),
    );
    await waitFor(() => expect(mocks.vaultChangePassword).toHaveBeenCalledTimes(1));

    // Uçuş sürerken kullanıcı yeni bir deneme yazıyor.
    fireEvent.change(byId("pw-old"), { target: { value: "eski-parola-2" } });
    fireEvent.change(byId("pw-new"), { target: { value: "yeni-parola-22" } });
    fireEvent.change(byId("pw-new2"), { target: { value: "yeni-parola-22" } });

    inFlight.resolve(undefined);
    await waitFor(() => expect(notices().join("\n")).toContain(t("settings.passwordDone")));

    expect(byId<HTMLInputElement>("pw-old").value).toBe("eski-parola-2");
    expect(byId<HTMLInputElement>("pw-new").value).toBe("yeni-parola-22");
    expect(byId<HTMLInputElement>("pw-new2").value).toBe("yeni-parola-22");
  });

  it("parola başarı bildirimi alan değişince kalkar, alanlar temizlenir", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    mocks.vaultChangePassword.mockResolvedValue(undefined);
    fireEvent.change(byId("pw-old"), { target: { value: "eski-parola" } });
    fireEvent.change(byId("pw-new"), { target: { value: "1234567890" } });
    fireEvent.change(byId("pw-new2"), { target: { value: "1234567890" } });
    fireEvent.click(
      within(cardOf(t("settings.passwordTitle"))).getByRole("button", {
        name: t("settings.passwordSubmit"),
      }),
    );

    await waitFor(() => expect(notices().join("\n")).toContain(t("settings.passwordDone")));
    expect(byId<HTMLInputElement>("pw-old").value).toBe("");
    expect(byId<HTMLInputElement>("pw-new").value).toBe("");

    // Yeni bir parola yazılmaya başlanınca eski başarı satırı güncel değildir.
    fireEvent.change(byId("pw-old"), { target: { value: "baska-parola" } });
    expect(notices().join("\n")).not.toContain(t("settings.passwordDone"));
  });

  it("açık kasada yedek kartı 'kasa kilitli' demez", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    const backupCard = cardOf(t("settings.backupTitle"));
    expect(backupCard.textContent).toContain(t("settings.backupNone"));
    expect(backupCard.textContent).not.toContain(t("settings.backupLocked"));
  });

  it("klasör seçici açılamazsa sessiz kalmaz ve istek üretmez", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    dialog.open.mockRejectedValue(new Error("seçici yok"));

    fireEvent.click(
      within(cardOf(t("settings.backupTitle"))).getByRole("button", {
        name: t("settings.backupPick"),
      }),
    );
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DIALOG");
    expect(alert.textContent).toContain(t("settings.pickDirFailed"));
    expect(mocks.vaultBackup).not.toHaveBeenCalled();
  });

  it("geri yükleme seçicisi de düşerse sessiz kalmaz", async () => {
    setup();
    expect(await screen.findByText(B_OK.fileName)).toBeTruthy();

    dialog.open.mockRejectedValue(new Error("seçici yok"));

    fireEvent.change(byId("restore-dest"), { target: { value: "/tmp/yeni-kasa" } });
    fireEvent.click(
      within(cardOf(t("settings.restoreTitle"))).getByRole("button", {
        name: t("settings.restorePick"),
      }),
    );

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DIALOG");
    expect(alert.textContent).toContain(t("settings.pickDirFailed"));
    expect(mocks.vaultRestore).not.toHaveBeenCalled();
  });
});

/* --------------------------------------------------- denetim zinciri (F03) */

const ENTITY_LONG = "11111111-2222-3333-4444-555555555555";

/**
 * Çekirdek **en yeni önce** döndürür (`sequence DESC`). İki satırın
 * `timestamp`i bilinçli olarak ters verilir: arayüz kendi başına sıralarsa
 * (yanlış) sıra değişir, bu test onu yakalar.
 */
const AUDIT_ROWS: AuditEventRow[] = [
  {
    sequence: 12,
    id: "ev-12",
    category: "document",
    entityId: ENTITY_LONG,
    operation: "import",
    actorLabel: "local-user",
    metadata: { bytes: 10 },
    priorHash: "aa11",
    eventHash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    timestamp: "2026-11-01T09:00:00.000Z",
  },
  {
    sequence: 11,
    id: "ev-11",
    category: "yeni_kategori",
    entityId: null,
    operation: "create",
    actorLabel: "local-user",
    metadata: null,
    priorHash: null,
    eventHash: "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    timestamp: "2026-11-01T10:00:00.000Z",
  },
];

function auditCard(): HTMLElement {
  return cardOf(t("settings.auditTitle"));
}

describe("denetim zinciri kartı", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    localStorage.clear();
  });

  afterEach(() => {
    cleanup();
  });

  it("açık kasada defter okunur; sıra çekirdekten gelir ve bilinmeyen kategori gizlenmez", async () => {
    setup({ audit: AUDIT_ROWS });

    const table = await within(auditCard()).findByRole("table");
    expect(mocks.auditEventList).toHaveBeenCalledTimes(1);
    expect(mocks.auditEventList).toHaveBeenCalledWith(50);

    const rows = within(table).getAllByRole("row");
    // Başlık + iki kayıt; ilk veri satırı çekirdeğin verdiği ilk satırdır.
    expect(rows).toHaveLength(3);
    const first = rows[1]!;
    const second = rows[2]!;
    expect(first.textContent).toContain("import");
    expect(second.textContent).toContain("yeni_kategori");
    expect(second.textContent).toContain("create");
    // Uzun kimlik kısaltılır, tam değer title'da durur (kaybolmaz).
    const entityCell = within(first).getByTitle(ENTITY_LONG);
    expect(entityCell.textContent).toBe("11111111…");
    // Özet kısaltılır ama tam özet title'dadır.
    expect(within(first).getByTitle(AUDIT_ROWS[0]!.eventHash)).toBeTruthy();
    expect(within(auditCard()).queryByText(t("settings.auditEmpty"))).toBeNull();
    // Şartname sınırı ekranda yazılı: bu defter resmî delil iddiası taşımaz.
    expect(within(auditCard()).getByText(t("settings.auditSub"))).toBeTruthy();
  });

  it("kasa kilitliyken defter istenmez, doğrulama düğmesi kapalıdır", async () => {
    setup({ status: LOCKED, audit: AUDIT_ROWS });

    expect(await screen.findByText(t("settings.vaultLockedBadge"))).toBeTruthy();
    expect(within(auditCard()).getByText(t("settings.auditLocked"))).toBeTruthy();
    const verify = within(auditCard()).getByRole("button", {
      name: t("settings.auditVerify"),
    }) as HTMLButtonElement;
    expect(verify.disabled).toBe(true);
    expect(mocks.auditEventList).not.toHaveBeenCalled();
    expect(mocks.auditChainCheck).not.toHaveBeenCalled();
  });

  it("tutarlı zincir doğrulanan kayıt sayısıyla yazılır", async () => {
    setup({
      audit: AUDIT_ROWS,
      auditChain: { checked: 12, intact: true, brokenEventId: null, brokenReason: null },
    });
    await within(auditCard()).findByRole("table");

    fireEvent.click(
      within(auditCard()).getByRole("button", { name: t("settings.auditVerify") }),
    );

    const status = await within(auditCard()).findByRole("status");
    expect(status.textContent).toContain(t("settings.auditIntact"));
    expect(status.textContent).toContain(t("settings.auditIntactCount", { count: 12 }));
    expect(within(auditCard()).queryByText(t("settings.auditBroken"))).toBeNull();
  });

  it("kopuklukta neden ve kayıt kimliği yazılır, 'doğrulandı' denmez", async () => {
    setup({
      audit: AUDIT_ROWS,
      auditChain: {
        checked: 3,
        intact: false,
        brokenEventId: "ev-9",
        brokenReason: "event_hash",
      },
    });
    await within(auditCard()).findByRole("table");

    fireEvent.click(
      within(auditCard()).getByRole("button", { name: t("settings.auditVerify") }),
    );

    const alert = await within(auditCard()).findByRole("alert");
    expect(alert.textContent).toContain(t("settings.auditBroken"));
    expect(alert.textContent).toContain(t("settings.auditBrokenLead", { count: 3 }));
    expect(alert.textContent).toContain("ev-9");
    expect(alert.textContent).toContain(t("settings.auditBrokenHash"));
    expect(within(auditCard()).queryByText(t("settings.auditIntact"))).toBeNull();
  });

  it("bilinmeyen kopukluk nedeni ham değeriyle yazılır", async () => {
    setup({
      audit: AUDIT_ROWS,
      auditChain: {
        checked: 0,
        intact: false,
        brokenEventId: "ev-1",
        brokenReason: "yeni_tur",
      },
    });
    await within(auditCard()).findByRole("table");

    fireEvent.click(
      within(auditCard()).getByRole("button", { name: t("settings.auditVerify") }),
    );

    const alert = await within(auditCard()).findByRole("alert");
    expect(alert.textContent).toContain(
      t("settings.auditBrokenUnknown", { reason: "yeni_tur" }),
    );
  });

  it("doğrulama düşerse önceki 'tutarlı' satırı kalmaz", async () => {
    setup({
      audit: AUDIT_ROWS,
      auditChain: { checked: 4, intact: true, brokenEventId: null, brokenReason: null },
    });
    await within(auditCard()).findByRole("table");
    const verify = () =>
      fireEvent.click(
        within(auditCard()).getByRole("button", { name: t("settings.auditVerify") }),
      );

    verify();
    expect(await within(auditCard()).findByRole("status")).toBeTruthy();

    mocks.auditChainCheck.mockRejectedValue({
      code: "E_DB_INTEGRITY",
      message: "denetim okunamadı",
      retryable: false,
    });
    verify();

    const alert = await within(auditCard()).findByRole("alert");
    expect(alert.textContent).toContain(t("settings.auditVerifyFailed"));
    expect(alert.textContent).toContain("E_DB_INTEGRITY");
    expect(within(auditCard()).queryByRole("status")).toBeNull();
  });

  it("okuma düşerse 'kayıt yok' denmez, hata satırı yazılır", async () => {
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.vaultPreUpdateBackups.mockResolvedValue([B_OK]);
    mocks.jobQueueStatus.mockResolvedValue(QUEUE_EMPTY);
    mocks.workerBudgets.mockResolvedValue([]);
    mocks.auditEventList.mockRejectedValue({
      code: "E_DB_INTEGRITY",
      message: "denetim defteri okunamadı",
      retryable: false,
    });
    // Teşhis kartı kendi okumasını yapar; bu senaryo denetim defterini ölçüyor.
    mocks.systemInfo.mockResolvedValue({ appVersion, protocolVersion: 1, platform: "macos" });
    // Yedek geçmişi bu senaryonun konusu değil; boş ama **başarılı** okuma.
    mocks.backupRunList.mockResolvedValue([]);
    dialog.open.mockResolvedValue(null);
    render(<Settings />);

    const alert = await within(auditCard()).findByRole("alert");
    expect(alert.textContent).toContain("E_DB_INTEGRITY");
    expect(within(auditCard()).queryByText(t("settings.auditEmpty"))).toBeNull();
    expect(within(auditCard()).queryByRole("table")).toBeNull();
  });

  it("tazeleme eski satırları istekten önce temizler", async () => {
    setup({ audit: AUDIT_ROWS });
    await within(auditCard()).findByRole("table");

    const pending = deferred<AuditEventRow[]>();
    mocks.auditEventList.mockReturnValueOnce(pending.promise);
    fireEvent.click(
      within(auditCard()).getByRole("button", { name: t("settings.auditRefresh") }),
    );

    // Yanıt gelmeden eski satırlar ekranda kalmamalı.
    expect(within(auditCard()).queryByRole("table")).toBeNull();
    expect(
      within(auditCard()).getAllByText(t("settings.auditLoading")).length,
    ).toBeGreaterThan(0);

    pending.resolve([]);
    expect(await within(auditCard()).findByText(t("settings.auditEmpty"))).toBeTruthy();
  });
});

/**
 * Ekran düzeyinde bağlantı (2026-11-07 turu): teşhis kartı **çekirdekten** mi
 * okuyor?
 *
 * `components/DiagnosticsCard.test.tsx` kartın kendi davranışını ölçer; burada
 * ölçülen şey kartın Ayarlar ekranına gerçekten **bağlı** olduğu ve gösterdiği
 * sürümün paket sürümünden ayrılabildiğidir. Ölçülen kusur: ekranın tek sürüm
 * satırı `package.json`dan geliyordu (`system_info` hiç çağrılmıyordu) — çalışan
 * ikili ayrı sürümdeyse ekran bunu söyleyemiyordu.
 */
describe("teşhis kartı ekrana bağlı", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    localStorage.clear();
  });

  afterEach(() => {
    cleanup();
  });

  it("sürüm çekirdekten okunur; paket sürümü ayrı satırda durur ve ayrışma uyarır", async () => {
    setup({ core: { appVersion: "0.9.9-core", protocolVersion: 1, platform: "macos" } });

    const card = cardOf(t("settings.diagnosticsTitle"));
    expect(await within(card).findByText("0.9.9-core")).toBeTruthy();
    // Arayüz sürümü de yazılı: iki değer karşılaştırılabilir.
    expect(card.textContent).toContain(appVersion);
    // Ayrışma gizlenmez.
    expect(within(card).getByRole("alert").textContent).toBe(t("settings.diagnosticsDrift"));
    expect(mocks.systemInfo).toHaveBeenCalledTimes(1);
  });
});

/**
 * Yedek geçmişi kartı (2026-11-07 turu).
 *
 * Ölçülen kusur: `vault_backup` yedeği diske yazıp doğruluyordu ama kasa
 * `backup_runs` tablosuna **hiç yazmıyordu**; "ne zaman, nereye, hangi
 * manifestle, doğrulandı mı" yalnız hedef klasördeki manifest dosyasındaydı
 * ve uygulamadan okunamıyordu. Kart, okuma yolunu kilitli/açık, düşen okuma,
 * tazeleme ve geç yanıt kapılarıyla ölçer.
 */
describe("yedek geçmişi kartı", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    localStorage.clear();
  });

  afterEach(() => {
    cleanup();
  });

  it("kasa kilitliyken geçmiş okunmaz; kart kilitli yazar", async () => {
    setup({ status: LOCKED });
    expect(await screen.findByText(t("settings.vaultLockedBadge"))).toBeTruthy();
    const card = cardOf(t("settings.backupHistoryTitle"));
    expect(mocks.backupRunList).not.toHaveBeenCalled();
    expect(card.textContent).toContain(t("settings.backupHistoryLocked"));
    expect(card.textContent).not.toContain(t("settings.backupHistoryEmpty"));
  });

  it("açık kasada satırlar hedef, snapshot, manifest ve durumla yazılır; tanınmayan durum gizlenmez", async () => {
    setup({
      history: [
        HISTORY_VERIFIED,
        { ...HISTORY_VERIFIED, id: "run-2", snapshotId: "DimYedek-bilinmeyen", status: "weird" },
      ],
    });
    const card = cardOf(t("settings.backupHistoryTitle"));
    expect(await within(card).findByText(HISTORY_VERIFIED.snapshotId)).toBeTruthy();
    const text = card.textContent ?? "";
    expect(text).toContain(HISTORY_VERIFIED.destinationLabel);
    expect(text).toContain(`${HISTORY_VERIFIED.manifestHash.slice(0, 10)}…`);
    expect(text).toContain(t("settings.backupHistoryStatusVerified"));
    expect(text).toContain("integrity_check");
    // Tanınmayan durum "doğrulandı" diye yuvarlanmaz: ham değeri görünür.
    expect(text).toContain("weird");
    expect(text).toContain(t("settings.backupHistoryLimitNote", { count: 20 }));
    expect(mocks.backupRunList).toHaveBeenCalledWith(20);
  });

  it("geçmiş okuması düşerse 'kayıt yok' denmez; hata kodu yazılır", async () => {
    setup({ historyError: { code: "E_VAULT_IO", message: "okunamadı", retryable: false } });
    const card = cardOf(t("settings.backupHistoryTitle"));
    const alert = await within(card).findByRole("alert");
    expect(alert.textContent).toContain("E_VAULT_IO");
    expect(card.textContent).not.toContain(t("settings.backupHistoryEmpty"));
  });

  it("yedek başarısı geçmişi tazeler; kayıt yazılamadıysa sonuç kartı uyarır", async () => {
    setup({ history: [HISTORY_VERIFIED] });
    expect(await screen.findByText(HISTORY_VERIFIED.snapshotId)).toBeTruthy();
    const before = mocks.backupRunList.mock.calls.length;

    dialog.open.mockResolvedValue("/Volumes/Yedek");
    mocks.vaultBackup.mockResolvedValue({
      ...BACKUP_RESULT,
      historyRecorded: false,
      historyError: "E_VAULT_IO",
    });
    fireEvent.click(
      within(cardOf(t("settings.backupTitle"))).getByRole("button", {
        name: t("settings.backupPick"),
      }),
    );

    await waitFor(() => expect(mocks.vaultBackup).toHaveBeenCalledWith("/Volumes/Yedek"));
    await waitFor(() =>
      expect(mocks.backupRunList.mock.calls.length).toBeGreaterThan(before),
    );
    const warning = await within(cardOf(t("settings.backupTitle"))).findByRole("alert");
    expect(warning.textContent).toContain("E_VAULT_IO");
    expect(warning.textContent).toContain(
      t("settings.backupHistoryNotRecorded", { code: "E_VAULT_IO" }),
    );
  });

  it("geç gelen geçmiş yanıtı yeni tazelemenin listesini ezmez", async () => {
    setup();
    const card = cardOf(t("settings.backupHistoryTitle"));
    expect(await within(card).findByText(t("settings.backupHistoryEmpty"))).toBeTruthy();

    // 1) Elle tazeleme askıda.
    const first = deferred<BackupRunRow[]>();
    mocks.backupRunList.mockReturnValueOnce(first.promise);
    fireEvent.click(
      within(card).getByRole("button", { name: t("settings.backupHistoryRefresh") }),
    );
    await waitFor(() => expect(mocks.backupRunList).toHaveBeenCalledTimes(2));

    // 2) Yedek başarısı ikinci okumayı başlatır; önce o yazılır.
    const second = deferred<BackupRunRow[]>();
    mocks.backupRunList.mockReturnValueOnce(second.promise);
    dialog.open.mockResolvedValue("/Volumes/Yedek");
    mocks.vaultBackup.mockResolvedValue(BACKUP_RESULT);
    fireEvent.click(
      within(cardOf(t("settings.backupTitle"))).getByRole("button", {
        name: t("settings.backupPick"),
      }),
    );
    await waitFor(() => expect(mocks.backupRunList).toHaveBeenCalledTimes(3));

    second.resolve([HISTORY_VERIFIED]);
    expect(await within(card).findByText(HISTORY_VERIFIED.snapshotId)).toBeTruthy();

    // 3) Birinci istek geç döner: yeni listeyi ezmemeli.
    first.resolve([{ ...HISTORY_VERIFIED, id: "eski", snapshotId: "ESKI-YEDEK" }]);
    await waitFor(() => expect(within(card).queryByText("ESKI-YEDEK")).toBeNull());
    expect(within(card).getByText(HISTORY_VERIFIED.snapshotId)).toBeTruthy();
  });
});
