// @vitest-environment jsdom
//
// Çalışma alanı ekranı davranış testi.
//
// Ekran 49 satırdır ve tamamen **sunucunun** verdiği kasa künyesini gösterir;
// kendi başına istek atmaz, sayı uydurmaz. Testin değeri şu üç noktada:
//
// 1. Kurulum adımları tek bir `aria-label` ile işaretlenir ve üç adım
//    (kasa/recovery/kullanmaya başla) **sözlükten** yazılır.
// 2. Kasa kimliği ve konumu **status**tan gelir; eksikse "—" yazılır
//    (boş hücre değil, uydurma yol değil).
// 3. Şifreleme/kurtarma satırları ve "Sıradaki" kartı sözlükten gelir;
//    ekran kendi metnini yazmaz (F04-T08).
//
// Dürüst sınır: bu bir **görünüm** testidir — ekran Tauri penceresinde
// açılmadı, CSS/yerleşim ölçülmedi.

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { t } from "../lib/i18n";
import type { VaultStatus } from "../lib/api";

const { Workspace } = await import("./Workspace");

const STATUS: VaultStatus = {
  unlocked: true,
  vaultPath: "/Users/avukat/Kasa/vault.dimvault",
  vaultId: "vault-9f2c",
  schemaVersion: 23,
  preUpdateBackup: null,
};

afterEach(cleanup);

describe("çalışma alanı ekranı", () => {
  it("kurulum adımlarını ve kasa rozetini yazar", () => {
    render(<Workspace status={STATUS} />);

    const steps = screen.getByLabelText(t("steps.aria"));
    expect(steps.textContent).toContain(t("steps.vault"));
    expect(steps.textContent).toContain(t("steps.recovery"));
    expect(steps.textContent).toContain(t("steps.start"));
    expect(steps.querySelectorAll(".step").length).toBe(3);
    // İlk iki adım tamam, üçüncüsü etkin.
    expect(steps.querySelectorAll(".step.done").length).toBe(2);
    expect(steps.querySelectorAll(".step.active").length).toBe(1);

    expect(screen.getByRole("heading", { level: 1, name: t("workspace.title") })).toBeTruthy();
    expect(screen.getByText(t("workspace.lead"))).toBeTruthy();
    expect(screen.getByText(t("workspace.badgeOpen"))).toBeTruthy();
  });

  it("kasa kimliğini ve konumunu sunucudan yazar", () => {
    render(<Workspace status={STATUS} />);

    const idTerm = screen.getByText(t("workspace.idLabel"));
    expect(idTerm.nextElementSibling?.textContent).toBe("vault-9f2c");
    const pathTerm = screen.getByText(t("workspace.pathLabel"));
    expect(pathTerm.nextElementSibling?.textContent).toBe(STATUS.vaultPath);

    // Şifreleme/kurtarma satırları sözlükten; ekran kendi metnini yazmaz.
    expect(screen.getByText(t("workspace.encryptionValue"))).toBeTruthy();
    expect(screen.getByText(t("workspace.recoveryValue"))).toBeTruthy();
  });

  it("eksik kimlik/konumda boş hücre değil '—' yazar", () => {
    render(<Workspace status={{ ...STATUS, vaultId: null, vaultPath: null }} />);

    const idTerm = screen.getByText(t("workspace.idLabel"));
    expect(idTerm.nextElementSibling?.textContent).toBe(t("common.unknown"));
    const pathTerm = screen.getByText(t("workspace.pathLabel"));
    expect(pathTerm.nextElementSibling?.textContent).toBe(t("common.unknown"));
  });

  it("'Sıradaki' kartını sözlükten yazar", () => {
    render(<Workspace status={STATUS} />);

    expect(screen.getByRole("heading", { level: 2, name: t("workspace.cardTitle") })).toBeTruthy();
    expect(screen.getByText(t("workspace.cardSub"))).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: t("workspace.nextTitle") })).toBeTruthy();
    expect(screen.getByText(t("workspace.nextBody"))).toBeTruthy();
  });
});
