// @vitest-environment jsdom
//
// Kabuk davranışı: otomatik kilit (hareketsizlik sayacı), elle kilit ve
// otomatik yedek. `lib/autolock.ts` / `lib/backup-schedule.ts` testleri yalnız
// **saf** karar fonksiyonlarını doğruluyor; `lock-behavior.test.tsx` üç App
// davranışını gerçek zamanlayıcıyla ölçüyor. Bu dosya 294 satırlık kabuğun
// **zamanlayıcılarını** gerçekten çalıştırır: aralık tetiklendiğinde hangi
// çekirdek komutu gider, yanıt düşünce arayüz ne yazar, kasa açılınca yedek
// gerçekten alınır mı. `api` kısmi mock; `t()`, `lib/autolock`,
// `lib/backup-schedule` gerçektir.
//
// Sabitlenen davranışlar:
//
// 1. Kasa açıkken on dakika hareketsizlik kasayı kilitler; açık ekran DOM'dan
//    kalkar (kilitliyken başka ekran render edilmez).
// 2. Kasa kilitliyken süre geçse de kilit isteği gönderilmez.
// 3. Kullanıcı hareketi (pointerdown) sayacı sıfırlar.
// 4. Otomatik kilit düşerse sessiz kalmaz: hata görünür ve **otuz saniye
//    sonraki** kontrolde yeniden denenir (tam hareketsizlik süresi beklenmez).
// 5. Kilit başarılı ama durum okuması düşerse arayüz **kilitli** kalır: kasa
//    gerçekten kapandıysa açık ekran ve içeriği görünür kalmaz.
// 6. Elle kilit de aynı kurala uyar (5).
// 7. Elle kilit çift tıklandığında tek istek gider.
// 8. Başarılı kilit önceki kırmızı hatayı siler (bayat hata kalmaz).
// 9. Kasa açılır açılmaz süresi dolmuş otomatik yedek alınır; son tarih
//    yazılır ve saatlik kontrolde yedek tekrarlanmaz.
// 10. Yedek ayarı yoksa / süresi dolmadıysa istek gönderilmez.
// 11. Yedek düşerse sessiz kalır (kullanıcıya gürültü yok), sonraki saatlik
//     kontrolde yeniden denenir.
// 12. Hareketsizlik sayacı kasa **açıldığında** sıfırlanır: kilitliyken geçen
//     süre "kasa açıkken hareketsizlik" sayılmaz.
// 13. Başlangıçta durum okunamazsa kasa kilitli varsayılır (fail-closed) ve
//     hata görünür; kilit düğmesi çizilmez.

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AUTO_LOCK_CHECK_MS } from "./lib/autolock";
import {
  BACKUP_CHECK_MS,
  BACKUP_DIR_KEY,
  BACKUP_LAST_KEY,
  readBackupAttempt,
  readBackupForce,
  writeBackupForce,
} from "./lib/backup-schedule";

const mocks = vi.hoisted(() => ({
  status: vi.fn(),
  lockVault: vi.fn(),
  openVault: vi.fn(),
  vaultBackup: vi.fn(),
  vaultSettingGet: vi.fn(),
  vaultSettingSet: vi.fn(),
  vaultSettingDelete: vi.fn(),
  backupRunList: vi.fn(),
  caseList: vi.fn(),
  documentList: vi.fn(),
}));

vi.mock("./lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/api")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      status: mocks.status,
      lockVault: mocks.lockVault,
      openVault: mocks.openVault,
      vaultBackup: mocks.vaultBackup,
      vaultSettingGet: mocks.vaultSettingGet,
      vaultSettingSet: mocks.vaultSettingSet,
      vaultSettingDelete: mocks.vaultSettingDelete,
      backupRunList: mocks.backupRunList,
      caseList: mocks.caseList,
      documentList: mocks.documentList,
    },
  };
});

const { App } = await import("./App");

const UNLOCKED = {
  unlocked: true,
  vaultPath: "/tmp/vault.dimvault",
  vaultId: "vault-1",
  schemaVersion: 23,
  preUpdateBackup: null,
};
const LOCKED = { ...UNLOCKED, unlocked: false };

const CASE = {
  id: "c1",
  title: "Kira dosyası",
  status: "active",
  rowVersion: 1,
  createdAt: "2026-10-01T00:00:00Z",
  updatedAt: "2026-10-01T00:00:00Z",
};

const MINUTE = 60_000;
const IDLE_LIMIT_TICKS = (10 * MINUTE) / AUTO_LOCK_CHECK_MS; // 20 kontrol

/** Mikro görevleri boşalt (sahte zamanlayıcıda `findBy`/`waitFor` kullanılmaz). */
async function flush(rounds = 10): Promise<void> {
  await act(async () => {
    for (let i = 0; i < rounds; i += 1) await Promise.resolve();
  });
}

/** Sahte saati ilerlet; aralıklar tetiklenir, yanıtlar boşaltılır. */
async function tick(ms: number): Promise<void> {
  await act(async () => {
    vi.advanceTimersByTime(ms);
  });
  await flush();
}

/**
 * Uzun süreleri **hareketli** kullanıcı gibi ilerlet: her dokuz dakikada bir
 * `pointerdown` gönderilir, böylece otomatik kilit araya girmez ve yedek
 * aralığı ölçülebilir. Sahte saattek adımlar saniyeler içinde koşar.
 */
async function tickActive(ms: number): Promise<void> {
  let left = ms;
  while (left > 9 * MINUTE) {
    fireEvent.pointerDown(window);
    await tick(9 * MINUTE);
    left -= 9 * MINUTE;
  }
  fireEvent.pointerDown(window);
  await tick(left);
}

async function mount() {
  const utils = render(<App />);
  await flush();
  return utils;
}

function openFiles(): void {
  fireEvent.click(screen.getByRole("button", { name: /Dosyalar/ }));
}

function lockButton(): HTMLButtonElement {
  return screen.getByRole("button", { name: "Kasayı kilitle" }) as HTMLButtonElement;
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-11-03T09:00:00.000Z"));
  localStorage.clear();
  // Varsayılanlar: kilit ve yedek çağrıları sessizce başarılı; testler
  // gerektiğinde `mockImplementation`/`mockRejectedValueOnce` ile ezer.
  mocks.lockVault.mockResolvedValue({});
  mocks.vaultBackup.mockResolvedValue({});
  mocks.caseList.mockResolvedValue([]);
  mocks.documentList.mockResolvedValue([]);
  // Otomatik yedek planı kasa ayarından okunur: varsayılan **kayıt yok**.
  // (Cihaz kaydında hedef varsa göç yolu yazar; yazım geri okunan satır döner.)
  mocks.vaultSettingGet.mockResolvedValue(null);
  mocks.vaultSettingSet.mockImplementation((key: string, value: unknown) =>
    Promise.resolve({
      scope: "vault",
      key,
      value,
      version: 1,
      updatedAt: "2026-11-03T09:00:00.000Z",
    }),
  );
  mocks.vaultSettingDelete.mockResolvedValue(true);
  // Tazelik: varsayılan **kasa geçmişi boş** (kanıt yok). Testler kendi
  // senaryosunda `mockResolvedValue([...])` ile ezer.
  mocks.backupRunList.mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.useRealTimers();
  localStorage.clear();
});

/* ------------------------------------------------------- otomatik kilit */

describe("kabuk: otomatik kilit", () => {
  it("on dakika hareketsizlik kasayı kilitler ve açık ekranı kaldırır", async () => {
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.caseList.mockResolvedValue([CASE]);
    mocks.documentList.mockResolvedValue([]);
    mocks.lockVault.mockImplementation(async () => {
      mocks.status.mockResolvedValue(LOCKED);
    });

    await mount();
    openFiles();
    await flush();
    expect(screen.getByText("Kira dosyası")).toBeTruthy();

    await tick(AUTO_LOCK_CHECK_MS * IDLE_LIMIT_TICKS);

    expect(mocks.lockVault).toHaveBeenCalledTimes(1);
    // Kasa gerçekten kapandı: açık dosya içeriği DOM'da kalmaz.
    expect(screen.queryByText("Kira dosyası")).toBeNull();
    expect(screen.getByRole("heading", { name: "Kasa kilitli" })).toBeTruthy();
  });

  it("kasa kilitliyken süre geçse de kilit isteği gönderilmez", async () => {
    mocks.status.mockResolvedValue(LOCKED);

    await mount();
    await tick(AUTO_LOCK_CHECK_MS * 40);

    expect(mocks.lockVault).not.toHaveBeenCalled();
  });

  it("kullanıcı hareketi hareketsizlik sayacını sıfırlar", async () => {
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.lockVault.mockImplementation(async () => {
      mocks.status.mockResolvedValue(LOCKED);
    });

    await mount();
    await tick(9 * MINUTE);
    fireEvent.pointerDown(window);
    await flush();

    await tick(2 * MINUTE);
    expect(mocks.lockVault).not.toHaveBeenCalled();

    await tick(8 * MINUTE);
    expect(mocks.lockVault).toHaveBeenCalledTimes(1);
  });

  it("otomatik kilit düşerse sessiz kalmaz ve otuz saniye sonra yeniden dener", async () => {
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.lockVault
      .mockRejectedValueOnce({
        code: "E_DB",
        message: "kasa kilitlenemedi",
        retryable: true,
      })
      .mockImplementation(async () => {
        mocks.status.mockResolvedValue(LOCKED);
      });

    await mount();
    await tick(AUTO_LOCK_CHECK_MS * IDLE_LIMIT_TICKS);

    expect(mocks.lockVault).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("alert").textContent).toContain("E_DB");

    // Kusur: başarısız denemeden sonra sayaç sıfırlanıyordu → ikinci deneme
    // ancak on dakika sonra geliyordu; kasa bu arada açık kalıyordu.
    await tick(AUTO_LOCK_CHECK_MS);
    expect(mocks.lockVault).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("kilit başarılı ama durum okuması düşerse arayüz kilitli kalır", async () => {
    mocks.status
      .mockResolvedValueOnce(UNLOCKED)
      .mockRejectedValue({
        code: "E_STATUS",
        message: "durum okunamadı",
        retryable: true,
      });
    mocks.lockVault.mockResolvedValue({});
    mocks.caseList.mockResolvedValue([CASE]);
    mocks.documentList.mockResolvedValue([]);

    await mount();
    openFiles();
    await flush();
    expect(screen.getByText("Kira dosyası")).toBeTruthy();

    await tick(AUTO_LOCK_CHECK_MS * IDLE_LIMIT_TICKS);

    expect(mocks.lockVault).toHaveBeenCalledTimes(1);
    // Kusur: `refresh()` düşünce durum "açık" kalıyor, kasa kapalı olduğu
    // hâlde dosya içeriği ekranda duruyordu.
    expect(screen.queryByText("Kira dosyası")).toBeNull();
    expect(screen.getByRole("alert").textContent).toContain("E_STATUS");
  });

  it("kasa açıldığında hareketsizlik sayacı sıfırlanır", async () => {
    mocks.status.mockResolvedValue(LOCKED);
    mocks.openVault.mockImplementation(async () => {
      mocks.status.mockResolvedValue(UNLOCKED);
    });

    await mount();
    // Kilitliyken dokuz dakika geçsin (kasa henüz açık değil).
    await tick(9 * MINUTE);

    fireEvent.click(screen.getByRole("button", { name: "Mevcut kasayı aç" }));
    fireEvent.change(screen.getByLabelText("Kasa konumu"), {
      target: { value: "/tmp/vault.dimvault" },
    });
    fireEvent.change(screen.getByLabelText("Parola"), {
      target: { value: "cok-guclu-parola-1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Kilidi aç" }));
    await flush();
    expect(screen.getByRole("heading", { name: "Kasa açık" })).toBeTruthy();

    // Kusur: sayaç uygulama açılışında başlıyordu; kasa açıldıktan iki dakika
    // sonra "on dakika hareketsiz" sayılıp kasa hemen yeniden kilitleniyordu.
    await tick(2 * MINUTE);
    expect(mocks.lockVault).not.toHaveBeenCalled();

    await tick(8 * MINUTE);
    expect(mocks.lockVault).toHaveBeenCalledTimes(1);
  });
});

/* ------------------------------------------------------------ elle kilit */

describe("kabuk: elle kilit", () => {
  it("kilit başarılı ama durum okuması düşerse açık ekran kapanır", async () => {
    mocks.status
      .mockResolvedValueOnce(UNLOCKED)
      .mockRejectedValue({
        code: "E_STATUS",
        message: "durum okunamadı",
        retryable: true,
      });
    mocks.lockVault.mockResolvedValue({});
    mocks.caseList.mockResolvedValue([CASE]);
    mocks.documentList.mockResolvedValue([]);

    await mount();
    openFiles();
    await flush();
    expect(screen.getByText("Kira dosyası")).toBeTruthy();

    fireEvent.click(lockButton());
    await flush();

    expect(mocks.lockVault).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Kira dosyası")).toBeNull();
    expect(screen.getByRole("alert").textContent).toContain("E_STATUS");
  });

  it("çift tıklandığında tek kilit isteği gönderilir", async () => {
    mocks.status.mockResolvedValue(UNLOCKED);
    let resolveLock: (() => void) | null = null;
    mocks.lockVault.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveLock = resolve;
        }),
    );

    await mount();
    const button = lockButton();
    fireEvent.click(button);
    fireEvent.click(button);
    await flush();

    expect(mocks.lockVault).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveLock?.();
    });
    await flush();
  });

  it("elle kilit uçuşta iken otomatik kontrol ikinci istek göndermez", async () => {
    mocks.status.mockResolvedValue(UNLOCKED);
    let resolveLock: (() => void) | null = null;
    mocks.lockVault.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveLock = resolve;
        }),
    );

    await mount();
    fireEvent.click(lockButton());
    await flush();
    expect(mocks.lockVault).toHaveBeenCalledTimes(1);

    // Kilit isteği hâlâ uçuşta; süre dolunca zamanlayıcı ikinci kez istememeli.
    await tick(AUTO_LOCK_CHECK_MS * IDLE_LIMIT_TICKS);
    expect(mocks.lockVault).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveLock?.();
    });
    await flush();
  });

  it("başarılı kilit önceki kırmızı hatayı siler", async () => {
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.lockVault
      .mockRejectedValueOnce({
        code: "E_DB",
        message: "kasa kilitlenemedi",
        retryable: true,
      })
      .mockImplementation(async () => {
        mocks.status.mockResolvedValue(LOCKED);
      });

    await mount();
    fireEvent.click(lockButton());
    await flush();
    expect(screen.getByRole("alert").textContent).toContain("E_DB");

    fireEvent.click(lockButton());
    await flush();

    expect(mocks.lockVault).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

/* -------------------------------------------------------- otomatik yedek */

describe("kabuk: otomatik yedek", () => {
  it("kasa açılır açılmaz süresi dolmuş yedek alınır ve son tarih yazılır", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/yedek");
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.vaultBackup.mockResolvedValue({});

    await mount();

    // Kusur: ilk `run()` montajda `unlocked.current === false` gördüğü için
    // hiç yedek almıyordu; ilk deneme ancak bir saat sonraki kontroldeydi.
    expect(mocks.vaultBackup).toHaveBeenCalledWith("/yedek");
    expect(localStorage.getItem(BACKUP_LAST_KEY)).not.toBeNull();
    // Son deneme ayrı kaydedilir: Ayarlar "son yedek" ile "son deneme"yi ayrı yazar.
    const attempt = readBackupAttempt();
    expect(attempt?.ok).toBe(true);
    expect(attempt?.code).toBeNull();
  });

  it("düşen otomatik yedek denemesi neden koduyla kaydedilir, tazelik ilerlemez", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/yedek");
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.vaultBackup.mockRejectedValue({
      code: "E_IO",
      message: "hedef disk yok",
      retryable: true,
    });

    await mount();

    expect(mocks.vaultBackup).toHaveBeenCalledTimes(1);
    const attempt = readBackupAttempt();
    expect(attempt?.ok).toBe(false);
    expect(attempt?.code).toBe("E_IO");
    // Başarısız deneme tazeliği ilerletmez: son başarılı yedek zamanı yazılmaz.
    expect(localStorage.getItem(BACKUP_LAST_KEY)).toBeNull();
  });

  it("klasör seçilmediyse deneme kaydı da yazılmaz", async () => {
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.vaultBackup.mockResolvedValue({});

    await mount();
    await tickActive(BACKUP_CHECK_MS * 2);

    expect(mocks.vaultBackup).not.toHaveBeenCalled();
    expect(readBackupAttempt()).toBeNull();
  });

  it("kasa kaydı tazeyse yedek istenmez (cihaz kaydı boş olsa da)", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/yedek");
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.vaultBackup.mockResolvedValue({});
    // Kanıt kasadadır: cihaz kaydı silinmiş olsa da yedek taze sayılır.
    mocks.backupRunList.mockResolvedValue([
      {
        id: "r1",
        snapshotId: "DimYedek-20261103-0859",
        schemaVersion: 24,
        manifestHash: "b".repeat(64),
        status: "verified",
        verificationResult: "integrity_check",
        destinationLabel: "/yedek",
        createdAt: "2026-11-03T08:00:00.000Z",
        completedAt: "2026-11-03T08:01:00.000Z",
      },
    ]);

    await mount();
    await tickActive(BACKUP_CHECK_MS);

    expect(mocks.vaultBackup).not.toHaveBeenCalled();
  });

  it("yedek ayarı yoksa istek gönderilmez", async () => {
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.vaultBackup.mockResolvedValue({});

    await mount();
    await tickActive(BACKUP_CHECK_MS * 3);

    expect(mocks.vaultBackup).not.toHaveBeenCalled();
  });

  it("plan kasadan okunur: cihaz kaydı boşken de süresi dolmuş yedek alınır", async () => {
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.vaultSettingGet.mockResolvedValue({
      scope: "vault",
      key: "backup.schedule",
      value: { destination: "/Volumes/KasaAyar", days: 7 },
      version: 1,
      updatedAt: "2026-11-01T00:00:00.000Z",
    });

    await mount();

    expect(mocks.vaultBackup).toHaveBeenCalledWith("/Volumes/KasaAyar");
    expect(readBackupAttempt()?.ok).toBe(true);
  });

  it("kasadaki bozuk plan yedek almaz; cihaz kaydındaki hedef onu ezmez", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/yedek");
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.vaultSettingGet.mockResolvedValue({
      scope: "vault",
      key: "backup.schedule",
      value: { destination: "", days: 0 },
      version: 1,
      updatedAt: "2026-11-01T00:00:00.000Z",
    });

    await mount();
    await tickActive(BACKUP_CHECK_MS * 2);

    expect(mocks.vaultBackup).not.toHaveBeenCalled();
    expect(readBackupAttempt()).toBeNull();
  });

  it("kasa kilitliyken yedek isteği gönderilmez", async () => {
    localStorage.setItem(BACKUP_DIR_KEY, "/yedek");
    mocks.status.mockResolvedValue(LOCKED);
    mocks.vaultBackup.mockResolvedValue({});

    await mount();
    await tick(BACKUP_CHECK_MS * 2);

    expect(mocks.vaultBackup).not.toHaveBeenCalled();
  });

  it("yedek düşerse sessiz kalır ve sonraki saatlik kontrolde yeniden denenir", async () => {
    // Plan kasa ayarında: ilk deneme düşse de plan yerinde kalır (göç tek seferlik).
    mocks.vaultSettingGet.mockResolvedValue({
      scope: "vault",
      key: "backup.schedule",
      value: { destination: "/yedek", days: 7 },
      version: 1,
      updatedAt: "2026-11-01T00:00:00.000Z",
    });
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.vaultBackup
      .mockRejectedValueOnce({
        code: "E_BACKUP",
        message: "yedek yazılamadı",
        retryable: true,
      })
      .mockResolvedValue({});

    await mount();
    expect(mocks.vaultBackup).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("alert")).toBeNull();

    await tickActive(BACKUP_CHECK_MS);
    expect(mocks.vaultBackup).toHaveBeenCalledTimes(2);
    expect(localStorage.getItem(BACKUP_LAST_KEY)).not.toBeNull();

    // Başarılı yedekten sonra süre dolmadığı için üçüncü istek yok.
    await tickActive(BACKUP_CHECK_MS);
    expect(mocks.vaultBackup).toHaveBeenCalledTimes(2);
  });

  it("cihaz kaydı taze olsa da kasa kaydı eskiyse yedek alınır (kanıt kasadır)", async () => {
    mocks.vaultSettingGet.mockResolvedValue({
      scope: "vault",
      key: "backup.schedule",
      value: { destination: "/yedek", days: 7 },
      version: 1,
      updatedAt: "2026-11-01T00:00:00.000Z",
    });
    // Cihaz kaydı "az önce yedeklendi" diyor; kasa geçmişi 40 gün öncesini
    // gösteriyor (başka kasadan kalmış önbellek). Tazelik kasadan hesaplanır.
    localStorage.setItem(BACKUP_LAST_KEY, new Date().toISOString());
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.backupRunList.mockResolvedValue([
      {
        id: "r-old",
        snapshotId: "DimYedek-20260924-0900",
        schemaVersion: 24,
        manifestHash: "c".repeat(64),
        status: "verified",
        verificationResult: "integrity_check",
        destinationLabel: "/yedek",
        createdAt: "2026-09-24T09:00:00.000Z",
        completedAt: "2026-09-24T09:01:00.000Z",
      },
    ]);

    await mount();

    expect(mocks.vaultBackup).toHaveBeenCalledWith("/yedek");
  });

  it("kasa geçmişi okunamazsa cihaz kaydına düşer (yedek sessizce durmaz)", async () => {
    mocks.vaultSettingGet.mockResolvedValue({
      scope: "vault",
      key: "backup.schedule",
      value: { destination: "/yedek", days: 7 },
      version: 1,
      updatedAt: "2026-11-01T00:00:00.000Z",
    });
    localStorage.setItem(BACKUP_LAST_KEY, new Date().toISOString());
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.backupRunList.mockRejectedValue({ code: "E_DB", message: "okunamadı", retryable: true });

    await mount();
    await tickActive(BACKUP_CHECK_MS);

    // Yedek kaydı okunamadı ama cihaz kaydı taze: gereksiz yedek alınmaz.
    expect(mocks.vaultBackup).not.toHaveBeenCalled();
  });

  it("sıfırla işareti kasa kaydı tazeyken de yedeği zorunlu alır ve işareti temizler", async () => {
    mocks.vaultSettingGet.mockResolvedValue({
      scope: "vault",
      key: "backup.schedule",
      value: { destination: "/yedek", days: 7 },
      version: 1,
      updatedAt: "2026-11-01T00:00:00.000Z",
    });
    mocks.status.mockResolvedValue(UNLOCKED);
    mocks.backupRunList.mockResolvedValue([
      {
        id: "r-fresh",
        snapshotId: "DimYedek-20261103-0859",
        schemaVersion: 24,
        manifestHash: "d".repeat(64),
        status: "verified",
        verificationResult: "integrity_check",
        destinationLabel: "/yedek",
        createdAt: "2026-11-03T08:00:00.000Z",
        completedAt: "2026-11-03T08:01:00.000Z",
      },
    ]);
    writeBackupForce();

    await mount();

    expect(mocks.vaultBackup).toHaveBeenCalledTimes(1);
    // Söz tutuldu: işaret başarılı yedekten sonra temizlendi.
    expect(readBackupForce()).toBe(false);
    expect(readBackupAttempt()?.ok).toBe(true);
  });
});

/* ------------------------------------------------------------- güvenlik */

describe("kabuk: durum okuma hatası", () => {
  it("başlangıçta durum okunamazsa kasa kilitli varsayılır (fail-closed)", async () => {
    mocks.status.mockRejectedValue({
      code: "E_STATUS",
      message: "durum okunamadı",
      retryable: true,
    });

    await mount();

    expect(screen.getByRole("alert").textContent).toContain("E_STATUS");
    expect(screen.queryByRole("button", { name: "Kasayı kilitle" })).toBeNull();
    expect(screen.getByRole("button", { name: "Kasayı oluştur" })).toBeTruthy();
  });
});
