import { describe, expect, it } from "vitest";
import { fileStem } from "./filename";

describe("dosya adı gövdesi", () => {
  it("Türkçe harfleri korur, yasak karakterleri alt çizgiye çevirir", () => {
    expect(fileStem("Açık: Dava/2")).toBe("Açık_ Dava_2");
    expect(fileStem("Şüpheli ığüşöçİĞÜŞÖÇ")).toBe("Şüpheli ığüşöçİĞÜŞÖÇ");
  });

  it("izin verilen küme dışında karakter bırakmaz", () => {
    const stem = fileStem('Rapor "2026" *?<>|\\/:');
    expect(stem).toMatch(/^[\w\- ığüşöçİĞÜŞÖÇ]*$/);
    expect(stem).not.toMatch(/["*?<>|\\/:]/);
  });

  it("tire, alt çizgi ve boşluğu olduğu gibi bırakır", () => {
    expect(fileStem("2026-10-30_duruşma notu")).toBe("2026-10-30_duruşma notu");
  });

  it("boş başlıkta boş gövde verir (ad uydurmaz)", () => {
    expect(fileStem("")).toBe("");
  });
});
