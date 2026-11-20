// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { App } from "./App";
import { api } from "./lib/api";
import { t } from "./lib/i18n";

async function mount(unlocked: boolean) {
  vi.spyOn(api, "status").mockResolvedValue({
    unlocked, vaultPath: null, vaultId: null, schemaVersion: null, preUpdateBackup: null,
  });
  vi.spyOn(api, "vaultSettingGet").mockResolvedValue(null);
  vi.spyOn(api, "caseList").mockResolvedValue([]);
  render(<App />);
  await screen.findByRole("heading", { name: t(unlocked ? "workspace.title" : "unlock.title") });
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

it("dosya listesine geçiş odağı çalışma alanına taşır", async () => {
  await mount(true);
  const openFiles = screen.getByRole("button", { name: t("workspace.openFiles") });
  openFiles.focus();
  fireEvent.click(openFiles);

  const heading = await screen.findByRole("heading", { name: t("nav.files") });
  expect(document.activeElement).toBe(heading.closest("#workspace-content"));
});

it("masaüstü gezinmesinde odak düğmede kalır; kilit bildiriminden dönüşte kasa içeriğine gider", async () => {
  await mount(false);
  const files = screen.getByRole("button", { name: t("nav.files") });
  files.focus();
  fireEvent.click(files);
  const goVault = await screen.findByRole("button", { name: t("shell.goVault") });
  expect(files.getAttribute("aria-current")).toBe("page");
  expect(document.activeElement).toBe(files);

  goVault.focus();
  fireEvent.click(goVault);
  const heading = await screen.findByRole("heading", { name: t("unlock.title") });
  expect(document.activeElement).toBe(heading.closest("#workspace-content"));
});

it("mobil gezinmede art arda ekran değiştirirken odak seçicide kalır", async () => {
  await mount(false);
  const menu = screen.getByRole("combobox", { name: t("nav.aria") });
  menu.focus();
  fireEvent.change(menu, { target: { value: "files" } });
  expect(await screen.findByRole("heading", { name: t("shell.locked") })).toBeTruthy();
  expect(document.activeElement).toBe(menu);

  fireEvent.change(menu, { target: { value: "vault" } });
  expect(await screen.findByRole("heading", { name: t("unlock.title") })).toBeTruthy();
  expect(document.activeElement).toBe(menu);
});
