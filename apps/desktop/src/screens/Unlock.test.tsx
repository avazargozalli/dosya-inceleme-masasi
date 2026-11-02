// @vitest-environment jsdom
//
// Kasa kapısı (`Unlock`) davranış testi. Ekran 216 satırdır ve ürünün **ilk**
// yüzüdür: kasa oluşturma, mevcut kasayı açma, kurtarma anahtarıyla açma ve
// bir kez gösterilen kurtarma anahtarının onaylatılması. `lib/vault-input`
// testleri yalnız parola kurallarını doğruluyordu; ekran bugüne dek hiç
// render edilmemişti.
//
// Sabitlenen davranışlar:
//
// 1. Varsayılan kip "Yeni kasa"; kip düğmeleri seçili kipi `aria-pressed` ile
//    bildirir; boş parolayla istek gitmez.
// 2. Parola politikası ve eşleşme gerekçesi yazılır; geçerli parola + eşleşme
//    düğmeyi açar. Kasa konumu boş/yalnız boşluksa istek üretilmez (kırpılır).
// 3. Oluşturma: kırpılmış yol + parola ile **tek** istek; istek sürerken kip
//    düğmeleri ve alanlar kilitlidir (yanıt başka kipe yazılmaz).
// 4. Kurtarma anahtarı ekranı: anahtar **yanıttan** yazılır; onay kutusu
//    işaretlenmeden "Kasayı aç" kapalıdır; "Geri" anahtarı tek tıkla düşürmez
//    (iki adımlı uyarı).
// 5. Açma: kırpılmış parolayla tek istek, başarıda parola temizlenir ve
//    `onUnlocked` çağrılır; düşerse hata kodu/ileti/`yeniden denenebilir`
//    yazılır, parola korunur, düğme yeniden açılır.
// 6. Kip değişince önceki kipin hatası ekranda kalmaz.
// 7. Kurtarma kipi: yalnız anahtar alanı; anahtar kırpılarak gider (tire/boşluk
//    normalizasyonu çekirdeğin işi), başarıda alan temizlenir.
// 8. Faz kartı **çalışan** belge içe aktarmayı "henüz yok" diye yazmaz; dört
//    satır da "gerçek" rozetini taşır (F05 tamamlandı, `document_import` var).
// 9. Anahtar ekranı App'in tuttuğu anahtardan çizilir: bileşen yeniden
//    bağlansa da anahtar kaybolmaz (gezinme kaybı App düzeyinde çözüldü).

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";

import { t } from "../lib/i18n";
import { MIN_PASSWORD_LENGTH } from "../lib/vault-input";
import type { CreateVaultResult, VaultInfo } from "../lib/api";

const mocks = vi.hoisted(() => ({
  createVault: vi.fn(),
  openVault: vi.fn(),
  openWithRecovery: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      createVault: mocks.createVault,
      openVault: mocks.openVault,
      openWithRecovery: mocks.openWithRecovery,
    },
  };
});

const { Unlock } = await import("./Unlock");

const DEFAULT_PATH = "Dosyalar/DosyaIncelemeMasasi";
const PASSWORD = "cok-guclu-parola-1";
const RECOVERY_KEY = "ABCD-EFGH-IJKL-MNOP-QRST-UVWX-YZ23-4567";

type Deferred<T> = { promise: Promise<T>; resolve: (v: T) => void; reject: (e: unknown) => void };

function deferred<T>(): Deferred<T> {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function vaultInfo(): VaultInfo {
  return {
    vaultPath: `${DEFAULT_PATH}/vault.dimvault`,
    vaultId: "vault-1",
    version: 23,
    createdAtUnix: 1_760_000_000,
    locked: false,
    schemaVersion: 23,
  };
}

function created(): CreateVaultResult {
  return { vaultPath: `${DEFAULT_PATH}/vault.dimvault`, recoveryKey: RECOVERY_KEY, vaultId: "vault-1" };
}

// `App`in sözleşmesinin aynısı: anahtarın sahibi üst bileşen.
function Harness({ onUnlocked }: { onUnlocked: () => Promise<void> }) {
  const [key, setKey] = useState<string | null>(null);
  return (
    <Unlock
      pendingKey={key}
      onKeyCreated={setKey}
      onKeyDismissed={() => setKey(null)}
      onUnlocked={onUnlocked}
    />
  );
}

function renderUnlock() {
  const onUnlocked = vi.fn(async () => {});
  render(<Harness onUnlocked={onUnlocked} />);
  return { onUnlocked };
}

function fillCreate(password = PASSWORD, confirm = password) {
  fireEvent.change(screen.getByLabelText(t("unlock.passwordCreate")), { target: { value: password } });
  fireEvent.change(screen.getByLabelText(t("unlock.passwordConfirm")), { target: { value: confirm } });
}

function submitButton(name: string) {
  return screen.getByRole("button", { name }) as HTMLButtonElement;
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("kasa kapısı", () => {
  it("varsayılan kip 'Yeni kasa'; seçili kip aria-pressed ile bildirilir, boş parolayla istek yok", () => {
    renderUnlock();

    expect(screen.getByRole("heading", { level: 1, name: t("unlock.title") })).toBeTruthy();
    expect((screen.getByLabelText(t("unlock.pathLabel")) as HTMLInputElement).value).toBe(DEFAULT_PATH);
    expect(submitButton(t("unlock.submitCreate")).disabled).toBe(true);

    expect(screen.getByRole("button", { name: t("unlock.modeCreate") }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: t("unlock.modeOpen") }).getAttribute("aria-pressed")).toBe("false");
    expect(screen.getByRole("button", { name: t("unlock.modeRecovery") }).getAttribute("aria-pressed")).toBe("false");

    expect(mocks.createVault).not.toHaveBeenCalled();
    expect(mocks.openVault).not.toHaveBeenCalled();
    expect(mocks.openWithRecovery).not.toHaveBeenCalled();
  });

  it("parola politikası ve eşleşme gerekçesini yazar, geçerli parolada düğmeyi açar", () => {
    renderUnlock();

    const password = screen.getByLabelText(t("unlock.passwordCreate"));
    const confirm = screen.getByLabelText(t("unlock.passwordConfirm"));
    fireEvent.change(password, { target: { value: "kisa" } });
    fireEvent.change(confirm, { target: { value: "kisa" } });
    expect(screen.getByText(t("vaultInput.tooShort", { min: MIN_PASSWORD_LENGTH }))).toBeTruthy();
    expect(submitButton(t("unlock.submitCreate")).disabled).toBe(true);

    fireEvent.change(password, { target: { value: PASSWORD } });
    expect(screen.getByText(t("vaultInput.mismatch"))).toBeTruthy();
    expect(submitButton(t("unlock.submitCreate")).disabled).toBe(true);

    fireEvent.change(confirm, { target: { value: PASSWORD } });
    expect(screen.queryByText(t("vaultInput.mismatch"))).toBeNull();
    expect(screen.getByText(t("unlock.passwordHint"))).toBeTruthy();
    expect(submitButton(t("unlock.submitCreate")).disabled).toBe(false);
  });

  it("boş veya yalnız boşluk olan kasa konumu istek üretmez", () => {
    renderUnlock();
    fillCreate();

    const path = screen.getByLabelText(t("unlock.pathLabel"));
    fireEvent.change(path, { target: { value: "   " } });
    expect(submitButton(t("unlock.submitCreate")).disabled).toBe(true);

    fireEvent.change(path, { target: { value: "  Kasa/yeni.dimvault  " } });
    expect(submitButton(t("unlock.submitCreate")).disabled).toBe(false);
  });

  it("oluşturma: kırpılmış yol ve parolayla tek istek; istek sürerken kip ve alanlar kilitli", async () => {
    const pending = deferred<CreateVaultResult>();
    mocks.createVault.mockReturnValue(pending.promise);
    renderUnlock();
    fillCreate();

    fireEvent.click(submitButton(t("unlock.submitCreate")));

    expect(mocks.createVault).toHaveBeenCalledTimes(1);
    expect(mocks.createVault).toHaveBeenCalledWith(DEFAULT_PATH, PASSWORD);
    expect(submitButton(t("common.busy")).disabled).toBe(true);

    // İstek sürerken kip değiştirilemez ve alanlar yazılamaz: yanıt, ekranda
    // görünen kipten bağımsız bir yere yazılmasın.
    for (const label of [t("unlock.modeCreate"), t("unlock.modeOpen"), t("unlock.modeRecovery")]) {
      expect(submitButton(label).disabled).toBe(true);
    }
    expect((screen.getByLabelText(t("unlock.pathLabel")) as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByLabelText(t("unlock.passwordCreate")) as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByLabelText(t("unlock.passwordConfirm")) as HTMLInputElement).disabled).toBe(true);

    pending.resolve(created());

    expect(await screen.findByText(RECOVERY_KEY)).toBeTruthy();
    expect(screen.getByLabelText(t("unlock.recoveryAria")).textContent).toBe(RECOVERY_KEY);
  });

  it("kurtarma anahtarı ekranı: onay kutusu olmadan 'Kasayı aç' kapalı, işaretlenince açılır", async () => {
    mocks.createVault.mockResolvedValue(created());
    const { onUnlocked } = renderUnlock();
    fillCreate();
    fireEvent.click(submitButton(t("unlock.submitCreate")));
    await screen.findByText(RECOVERY_KEY);

    expect(submitButton(t("unlock.openVault")).disabled).toBe(true);
    fireEvent.click(submitButton(t("unlock.openVault")));
    expect(onUnlocked).not.toHaveBeenCalled();

    fireEvent.click(screen.getByLabelText(t("unlock.savedCheck")));
    expect(submitButton(t("unlock.openVault")).disabled).toBe(false);
    fireEvent.click(submitButton(t("unlock.openVault")));

    await waitFor(() => expect(onUnlocked).toHaveBeenCalledTimes(1));
  });

  it("'Geri' tek tıkla kurtarma anahtarını düşürmez; iki adımlı uyarıdan sonra forma döner", async () => {
    mocks.createVault.mockResolvedValue(created());
    const { onUnlocked } = renderUnlock();
    fillCreate();
    fireEvent.click(submitButton(t("unlock.submitCreate")));
    await screen.findByText(RECOVERY_KEY);

    // İlk tık yalnız uyarır: anahtar ekranda kalır.
    fireEvent.click(submitButton(t("common.back")));
    expect(screen.getByText(t("unlock.leaveWarning"))).toBeTruthy();
    expect(screen.getByText(RECOVERY_KEY)).toBeTruthy();

    fireEvent.click(submitButton(t("unlock.leaveCancel")));
    expect(screen.queryByText(t("unlock.leaveWarning"))).toBeNull();
    expect(screen.getByText(RECOVERY_KEY)).toBeTruthy();

    fireEvent.click(submitButton(t("common.back")));
    fireEvent.click(submitButton(t("unlock.leaveConfirm")));

    expect(screen.queryByText(RECOVERY_KEY)).toBeNull();
    // Anahtar ekranı kapandı, "mevcut kasayı aç" formuna dönüldü; parola ve
    // eşleşme alanları oluşturma sonrası temizlenmişti.
    expect(submitButton(t("unlock.submitOpen"))).toBeTruthy();
    expect((screen.getByLabelText(t("unlock.password")) as HTMLInputElement).value).toBe("");
    expect(screen.queryByLabelText(t("unlock.passwordConfirm"))).toBeNull();
    expect(onUnlocked).not.toHaveBeenCalled();
  });

  it("mevcut kasayı açma: kırpılmış parolayla tek istek, başarıda parola temizlenir", async () => {
    mocks.openVault.mockResolvedValue(vaultInfo());
    const { onUnlocked } = renderUnlock();

    fireEvent.click(submitButton(t("unlock.modeOpen")));
    fireEvent.change(screen.getByLabelText(t("unlock.pathLabel")), { target: { value: "  Kasa/profil.dimvault  " } });
    fireEvent.change(screen.getByLabelText(t("unlock.password")), { target: { value: PASSWORD } });
    fireEvent.click(submitButton(t("unlock.submitOpen")));

    expect(mocks.openVault).toHaveBeenCalledTimes(1);
    expect(mocks.openVault).toHaveBeenCalledWith("Kasa/profil.dimvault", PASSWORD);
    await waitFor(() => expect(onUnlocked).toHaveBeenCalledTimes(1));
    expect((screen.getByLabelText(t("unlock.password")) as HTMLInputElement).value).toBe("");
    expect(mocks.createVault).not.toHaveBeenCalled();
  });

  it("açma düşerse hata kodu, ileti ve yeniden denenebilir soneki yazılır; parola korunur", async () => {
    mocks.openVault.mockRejectedValueOnce({
      code: "E_VAULT_NOT_FOUND",
      message: "Kasa dosyası bulunamadı.",
      retryable: true,
    });
    const { onUnlocked } = renderUnlock();

    fireEvent.click(submitButton(t("unlock.modeOpen")));
    fireEvent.change(screen.getByLabelText(t("unlock.password")), { target: { value: PASSWORD } });
    fireEvent.click(submitButton(t("unlock.submitOpen")));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_VAULT_NOT_FOUND");
    expect(alert.textContent).toContain("Kasa dosyası bulunamadı.");
    expect(alert.textContent).toContain(t("common.retryableSuffix"));
    expect((screen.getByLabelText(t("unlock.password")) as HTMLInputElement).value).toBe(PASSWORD);
    expect(submitButton(t("unlock.submitOpen")).disabled).toBe(false);
    expect(onUnlocked).not.toHaveBeenCalled();
  });

  it("kip değişince önceki kipin hatası ekranda kalmaz", async () => {
    mocks.openVault.mockRejectedValueOnce({ code: "E_DB", message: "Kasa okunamadı." });
    renderUnlock();

    fireEvent.click(submitButton(t("unlock.modeOpen")));
    fireEvent.change(screen.getByLabelText(t("unlock.password")), { target: { value: PASSWORD } });
    fireEvent.click(submitButton(t("unlock.submitOpen")));
    expect(await screen.findByRole("alert")).toBeTruthy();

    fireEvent.click(submitButton(t("unlock.modeRecovery")));
    expect(screen.queryByRole("alert")).toBeNull();
    fireEvent.click(submitButton(t("unlock.modeCreate")));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByRole("button", { name: t("unlock.modeCreate") }).getAttribute("aria-pressed")).toBe("true");
  });

  it("kurtarma kipi: yalnız anahtar alanı, boşluklu anahtar kırpılarak gönderilir", async () => {
    mocks.openWithRecovery.mockResolvedValue(vaultInfo());
    const { onUnlocked } = renderUnlock();

    fireEvent.click(submitButton(t("unlock.modeRecovery")));
    expect(screen.queryByLabelText(t("unlock.password"))).toBeNull();

    const keyInput = screen.getByLabelText(t("unlock.recoveryKeyLabel"));
    fireEvent.change(keyInput, { target: { value: "   " } });
    expect(submitButton(t("unlock.submitRecovery")).disabled).toBe(true);

    fireEvent.change(keyInput, { target: { value: "  abcd-efgh  ijkl  " } });
    fireEvent.click(submitButton(t("unlock.submitRecovery")));

    expect(mocks.openWithRecovery).toHaveBeenCalledTimes(1);
    expect(mocks.openWithRecovery).toHaveBeenCalledWith(DEFAULT_PATH, "abcd-efgh  ijkl");
    await waitFor(() => expect(onUnlocked).toHaveBeenCalledTimes(1));
    expect((screen.getByLabelText(t("unlock.recoveryKeyLabel")) as HTMLInputElement).value).toBe("");
  });

  it("kurtarma düşerse hata yazılır, girilen anahtar korunur", async () => {
    mocks.openWithRecovery.mockRejectedValueOnce({ code: "E_RECOVERY", message: "Anahtar geçersiz." });
    renderUnlock();

    fireEvent.click(submitButton(t("unlock.modeRecovery")));
    fireEvent.change(screen.getByLabelText(t("unlock.recoveryKeyLabel")), { target: { value: "abcd" } });
    fireEvent.click(submitButton(t("unlock.submitRecovery")));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_RECOVERY");
    expect((screen.getByLabelText(t("unlock.recoveryKeyLabel")) as HTMLInputElement).value).toBe("abcd");
  });

  it("oluşturma düşerse anahtar ekranı açılmaz, parola korunur", async () => {
    mocks.createVault.mockRejectedValueOnce({ code: "E_VAULT_EXISTS", message: "Kasa zaten var." });
    renderUnlock();
    fillCreate();

    fireEvent.click(submitButton(t("unlock.submitCreate")));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_VAULT_EXISTS");
    expect(screen.queryByText(RECOVERY_KEY)).toBeNull();
    expect((screen.getByLabelText(t("unlock.passwordCreate")) as HTMLInputElement).value).toBe(PASSWORD);
    expect(submitButton(t("unlock.submitCreate")).disabled).toBe(false);
  });

  it("faz kartı çalışan belge içe aktarmayı 'henüz yok' diye yazmaz", () => {
    renderUnlock();

    const row = screen.getByText(t("unlock.phaseImport")).nextElementSibling;
    expect(row).toBeTruthy();
    expect(row!.textContent).toContain(t("unlock.phaseImportValue"));
    expect(row!.querySelector(".badge")?.textContent).toBe(t("unlock.badgeReal"));
    // İçe aktarma F05'te tamamlandı (`document_import` + sürükle-bırak);
    // "henüz yok" demek kullanıcıya yanlış bilgi olurdu.
    expect(t("unlock.phaseImportValue")).not.toContain("henüz yok");
    expect(screen.queryByText("F05")).toBeNull();
  });

  it("anahtar ekranı App'in tuttuğu anahtardan çizilir: bileşen yeniden bağlansa da kaybolmaz", () => {
    const onUnlocked = vi.fn(async () => {});
    const noop = () => {};
    const first = render(
      <Unlock pendingKey={RECOVERY_KEY} onUnlocked={onUnlocked} onKeyCreated={noop} onKeyDismissed={noop} />,
    );
    expect(screen.getByText(RECOVERY_KEY)).toBeTruthy();
    first.unmount();

    render(
      <Unlock pendingKey={RECOVERY_KEY} onUnlocked={onUnlocked} onKeyCreated={noop} onKeyDismissed={noop} />,
    );
    expect(screen.getByText(RECOVERY_KEY)).toBeTruthy();
    expect(mocks.createVault).not.toHaveBeenCalled();
  });
});
