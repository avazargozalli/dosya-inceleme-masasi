import { describe, expect, it } from "vitest";
import { canShowScreen } from "./lib/lock";

describe("kilit ekranı", () => {
  it("kasa kilitliyken Dosyalar gösterilmez", () => {
    expect(canShowScreen("files", true)).toBe(false);
  });

  it("kasa kilitliyken hiçbir hassas ekran gösterilmez", () => {
    for (const screen of ["files", "search", "review", "ai", "ai-review", "precedents", "redaction", "exports", "timeline", "tasks", "checklists", "settings"]) {
      expect(canShowScreen(screen, true), screen).toBe(false);
    }
  });

  it("kasa kilitliyken yalnız Kasa ekranı gösterilir", () => {
    expect(canShowScreen("vault", true)).toBe(true);
  });

  it("kasa açıkken tüm ekranlar gösterilir", () => {
    expect(canShowScreen("files", false)).toBe(true);
    expect(canShowScreen("vault", false)).toBe(true);
  });
});
