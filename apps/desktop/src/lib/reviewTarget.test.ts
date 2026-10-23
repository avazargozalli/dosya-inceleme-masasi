import { describe, expect, it } from "vitest";

import { buildReviewTarget, itemText } from "./reviewTarget";

describe("buildReviewTarget", () => {
  it("kayıt türü seçilmeden hedef üretmez", () => {
    expect(buildReviewTarget("", "Kira ödenmedi")).toBeNull();
  });

  it("boş metni kalıcı kayda dönüştürmez", () => {
    expect(buildReviewTarget("note", "   ")).toBeNull();
  });

  it("iddia türü açıkça seçilmeden iddia hedefi üretmez", () => {
    expect(buildReviewTarget("claim", "Kira ödenmedi")).toBeNull();
    expect(buildReviewTarget("claim", "Kira ödenmedi", "")).toBeNull();
  });

  it("iddia hedefini seçilen türle üretir", () => {
    const target = buildReviewTarget("claim", " Kira ödenmedi ", "defense");
    expect(target).toEqual({
      kind: "claim",
      claimText: "Kira ödenmedi",
      claimKind: "defense",
    });
  });

  it("not hedefinde başlığı ilk satırdan türetir", () => {
    const target = buildReviewTarget("note", "Çelişki notu\nayrıntı");
    expect(target).toEqual({
      kind: "note",
      title: "Çelişki notu",
      body: "Çelişki notu\nayrıntı",
    });
  });

  it("olay hedefi tarih uydurmaz", () => {
    const target = buildReviewTarget("event", "Tebligat yapıldı");
    expect(target).toMatchObject({
      kind: "event",
      dateValue: null,
      datePrecision: "unknown",
      assertionKind: "user",
    });
  });

  it("görev hedefi hatırlatıcıyı kullanıcı onayına bırakır", () => {
    const target = buildReviewTarget("task", "Dilekçe hazırla");
    expect(target).toEqual({
      kind: "task",
      title: "Dilekçe hazırla",
      reminderPolicy: "none",
    });
  });
});

describe("itemText", () => {
  it("payload metnini bulur", () => {
    expect(itemText({ text: "Ödeme yapılmadı" })).toBe("Ödeme yapılmadı");
    expect(itemText({ body: "gövde" })).toBe("gövde");
    expect(itemText({ other: 1 })).toBe("");
    expect(itemText(null)).toBe("");
  });
});
