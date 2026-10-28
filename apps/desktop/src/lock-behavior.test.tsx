// @vitest-environment jsdom
//
// Gerçek ekran davranışı: Dosyalar ekranı açıkken kasa kilitlenirse belge
// listesi DOM'dan kalkmalı. `canShowScreen` birim testi bunu kanıtlamaz;
// burada bileşen ağacı gerçekten render edilip kilitleniyor.

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  vaultStatus: vi.fn(),
  caseList: vi.fn(),
  documentList: vi.fn(),
  lockVault: vi.fn(),
}));

vi.mock("./lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/api")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      status: mocks.vaultStatus,
      vaultStatus: mocks.vaultStatus,
      caseList: mocks.caseList,
      documentList: mocks.documentList,
      lockVault: mocks.lockVault,
    },
  };
});

const { App } = await import("./App");

const UNLOCKED = { unlocked: true, schemaVersion: 23 };
const LOCKED = { unlocked: false, schemaVersion: 23 };

const CASE = {
  id: "c1",
  title: "Kira dosyası",
  status: "active",
  rowVersion: 1,
  createdAt: "2026-10-01T00:00:00Z",
  updatedAt: "2026-10-01T00:00:00Z",
};

describe("kilit ekran davranışı", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("Dosyalar açıkken kilitlenince belge listesi ekrandan kalkar", async () => {
    mocks.vaultStatus.mockResolvedValue(UNLOCKED);
    mocks.caseList.mockResolvedValue([CASE]);
    mocks.documentList.mockResolvedValue([]);
    mocks.lockVault.mockImplementation(async () => {
      mocks.vaultStatus.mockResolvedValue(LOCKED);
    });

    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: /Dosyalar/ }));
    expect(await screen.findByText("Kira dosyası")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: /Kasayı kilitle/ }));

    await waitFor(() => expect(screen.queryByText("Kira dosyası")).toBeNull());
    expect(screen.getByRole("heading", { name: "Kasa kilitli" })).toBeTruthy();
  });

  it("kilitliyken başka ekrana geçilemez", async () => {
    mocks.vaultStatus.mockResolvedValue(LOCKED);

    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: /Dosyalar/ }));

    expect(await screen.findByRole("heading", { name: "Kasa kilitli" })).toBeTruthy();
    expect(screen.queryByText("Kira dosyası")).toBeNull();
  });
});
