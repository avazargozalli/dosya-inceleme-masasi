// @vitest-environment jsdom
//
// Teşhis kartı davranış testi (REQ-033).
//
// `lib/diagnostics.ts` testleri saf yüklemi doğrular; burada kartın **çekirdekten**
// (`system_info`) gelen sürümü ekrana taşıdığı ve ayrışmayı gizlemediği ölçülür.
// Sabitlenen davranışlar:
//
// 1. Sürüm **çekirdekten** yazılır — arayüz sürümü ayrı satırda durur; ikisi
//    aynıyken uyarı basılmaz (gürültü yok).
// 2. **Ölçülen kusur:** ekranın tek sürüm satırı `package.json`dan geliyordu;
//    çalışan ikili ayrı sürümdeyse ekran bunu söylemiyordu. Ayrışmada uyarı
//    yazılır, iki değer de görünür.
// 3. Protokol sürümü çekirdekten yazılır; farklıysa uyumsuzluk uyarısı basılır.
// 4. Tanınmayan platform kodu **ham** yazılır (uydurma ad yok).
// 5. Okuma düşerse sürüm **uydurulmaz**: satır "okunamadı" yazar, hata kodu ve
//    ileti ayrı satırda görünür; eski değer de bırakılmaz.
// 6. "Yenile" çekirdeği yeniden okur.
// 7. Geç yanıt kapısı: StrictMode'un çift effect'inde yavaş dönen eski okuma yeni
//    değeri ezmez.
// 8. Gizlilik sınırı ekranda yazılıdır: yalnız sürüm/protokol/platform.

import React from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { t } from "../lib/i18n";
import type { SystemInfo } from "../lib/api";

const mocks = vi.hoisted(() => ({ systemInfo: vi.fn() }));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

const { DiagnosticsCard } = await import("./DiagnosticsCard");

/* ------------------------------------------------------------------ fixtures */

const UI_VERSION = "0.1.0";

const CORE: SystemInfo = {
  appVersion: "0.1.0",
  protocolVersion: 1,
  platform: "macos",
};

/** Arayüz paketi ile çekirdek ayrışmış kurulum. */
const CORE_DRIFT: SystemInfo = {
  appVersion: "0.2.0",
  protocolVersion: 1,
  platform: "windows",
};

/**
 * Geç dönen **ilk** okumanın künyesi. Sürüm bilinçli olarak arayüz sürümünden
 * farklı: aksi hâlde "eski değer ekranda kalmadı" iddiası, arayüz sürümü satırı
 * yüzünden boş yere yeşil kalırdı.
 */
const OLD_CORE: SystemInfo = {
  appVersion: "0.0.1-old",
  protocolVersion: 1,
  platform: "linux",
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** Kartın künye satırları: [çekirdek, arayüz, protokol, platform]. */
function rows(): string[] {
  const card = screen.getByRole("heading", { name: t("settings.diagnosticsTitle") }).closest(".card");
  if (!card) throw new Error("teşhis kartı bulunamadı");
  return Array.from(card.querySelectorAll("dd")).map((dd) => dd.textContent ?? "");
}

/* -------------------------------------------------------------------- tests */

describe("teşhis kartı", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("sürümü çekirdekten yazar, arayüz sürümünü ayrı satırda tutar", async () => {
    mocks.systemInfo.mockResolvedValue(CORE);
    render(<DiagnosticsCard uiVersion={UI_VERSION} />);

    expect(await screen.findByText(CORE.appVersion)).toBeTruthy();
    expect(rows()).toEqual([CORE.appVersion, UI_VERSION, "1", "macOS"]);
    expect(mocks.systemInfo).toHaveBeenCalledTimes(1);
    // Aynı sürüm: uyarı yok.
    expect(screen.queryAllByRole("alert")).toHaveLength(0);
  });

  it("arayüz ile çekirdek sürümü ayrışırsa uyarır, iki değeri de gizlemez", async () => {
    mocks.systemInfo.mockResolvedValue(CORE_DRIFT);
    render(<DiagnosticsCard uiVersion={UI_VERSION} />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(t("settings.diagnosticsDrift"));
    expect(rows()).toEqual([CORE_DRIFT.appVersion, UI_VERSION, "1", "Windows"]);
  });

  it("çekirdeğin protokolü arayüzün beklediğinden farklıysa uyumsuzluk yazar", async () => {
    mocks.systemInfo.mockResolvedValue({ ...CORE, protocolVersion: 2 });
    render(<DiagnosticsCard uiVersion={UI_VERSION} />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(
      t("settings.diagnosticsProtocolMismatch", { ui: 1, core: 2 }),
    );
    expect(rows()[2]).toBe("2");
  });

  it("tanınmayan platform kodunu ham yazar", async () => {
    mocks.systemInfo.mockResolvedValue({ ...CORE, platform: "plan9" });
    render(<DiagnosticsCard uiVersion={UI_VERSION} />);

    expect(await screen.findByText("plan9")).toBeTruthy();
    expect(rows()[3]).toBe("plan9");
  });

  it("okuma düşerse sürüm uydurulmaz: 'okunamadı' ve hata kodu yazılır", async () => {
    mocks.systemInfo.mockRejectedValue({
      code: "E_IO",
      message: "sürüm bilgisi okunamadı",
      retryable: true,
    });
    render(<DiagnosticsCard uiVersion={UI_VERSION} />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_IO");
    expect(alert.textContent).toContain("sürüm bilgisi okunamadı");
    // Çekirdek/protokol/platform satırları "okunamadı" yazar; sürüm uydurulmaz.
    expect(rows().slice(0, 1)).toEqual([t("settings.diagnosticsNotRead")]);
    expect(rows()[2]).toBe(t("settings.diagnosticsNotRead"));
    expect(rows()[3]).toBe(t("settings.diagnosticsNotRead"));
    // Arayüz sürümü paketten bilinir: o satır durur.
    expect(rows()[1]).toBe(UI_VERSION);
  });

  it("'Yenile' çekirdeği yeniden okur ve yeni değeri yazar", async () => {
    mocks.systemInfo.mockResolvedValue(CORE);
    render(<DiagnosticsCard uiVersion={UI_VERSION} />);
    expect(await screen.findByText(CORE.appVersion)).toBeTruthy();
    expect(mocks.systemInfo).toHaveBeenCalledTimes(1);

    mocks.systemInfo.mockResolvedValue(CORE_DRIFT);
    fireEvent.click(screen.getByRole("button", { name: t("settings.diagnosticsRefresh") }));

    expect(await screen.findByText(CORE_DRIFT.appVersion)).toBeTruthy();
    expect(mocks.systemInfo).toHaveBeenCalledTimes(2);
  });

  it("geç yanıt kapısı: StrictMode'un çift okumasında eski yanıt yenisini ezmez", async () => {
    const first = deferred<SystemInfo>();
    const second = deferred<SystemInfo>();
    mocks.systemInfo
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);

    render(
      <React.StrictMode>
        <DiagnosticsCard uiVersion={UI_VERSION} />
      </React.StrictMode>,
    );

    // StrictMode effect'i iki kez koşar: iki okuma uçuşta.
    await vi.waitFor(() => expect(mocks.systemInfo).toHaveBeenCalledTimes(2));

    second.resolve(CORE_DRIFT);
    expect(await screen.findByText(CORE_DRIFT.appVersion)).toBeTruthy();

    // Yavaş dönen **eski** okuma sonradan gelir: ekranı geri almamalı.
    first.resolve(OLD_CORE);
    // Geç yanıtın işlenmesini bekle (mikro görev + render); yarışa bırakma —
    // aksi hâlde waitFor eski değer daha yazılmadan geçer ve kapı boş yeşil kalır.
    await new Promise((resolve) => setTimeout(resolve, 25));
    expect(rows()[0]).toBe(CORE_DRIFT.appVersion);
    expect(screen.queryByText(OLD_CORE.appVersion)).toBeNull();
  });

  it("geç dönen hata yeni veriyi silmez", async () => {
    const first = deferred<SystemInfo>();
    const second = deferred<SystemInfo>();
    mocks.systemInfo.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    render(
      <React.StrictMode>
        <DiagnosticsCard uiVersion={UI_VERSION} />
      </React.StrictMode>,
    );
    await vi.waitFor(() => expect(mocks.systemInfo).toHaveBeenCalledTimes(2));

    second.resolve(CORE_DRIFT);
    expect(await screen.findByText(CORE_DRIFT.appVersion)).toBeTruthy();

    // Eski okuma sonradan düşer: geçerli künye silinmemeli, hata satırı basılmamalı.
    first.reject({ code: "E_IO", message: "geç hata", retryable: false });
    await new Promise((resolve) => setTimeout(resolve, 25));
    expect(rows()[0]).toBe(CORE_DRIFT.appVersion);
    expect(rows()[0]).not.toBe(t("settings.diagnosticsNotRead"));
    expect(document.body.textContent ?? "").not.toContain("geç hata");
  });

  it("sözleşme dışı künye sürüm diye yazılmaz", async () => {
    // Gelecek/uyumsuz çekirdek: alanlar eksik. Eksik alanı "sürüm" saymak uydurma olur.
    mocks.systemInfo.mockResolvedValue({ protocolVersion: 1, platform: "macos" });
    render(<DiagnosticsCard uiVersion={UI_VERSION} />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_SYSTEM_INFO");
    expect(alert.textContent).toContain(t("settings.diagnosticsBadPayload"));
    expect(rows()[0]).toBe(t("settings.diagnosticsNotRead"));
  });

  it("gizlilik sınırı ekranda yazılıdır", async () => {
    mocks.systemInfo.mockResolvedValue(CORE);
    render(<DiagnosticsCard uiVersion={UI_VERSION} />);

    expect(await screen.findByText(t("settings.diagnosticsPrivacy"))).toBeTruthy();
  });
});
