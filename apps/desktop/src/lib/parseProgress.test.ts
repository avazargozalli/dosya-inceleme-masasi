import { describe, expect, it } from "vitest";

import type { ParseProgressEvent } from "./api";
import {
  parseProgressLine,
  parseProgressPercent,
  parseProgressWidth,
  parseStageLabel,
} from "./parseProgress";

const progress = (over: Partial<ParseProgressEvent> = {}): ParseProgressEvent => ({
  versionId: "v-1",
  stage: "pages",
  done: 12,
  total: 40,
  ...over,
});

describe("parseStageLabel", () => {
  it("bilinen aşamaları Türkçe yazar", () => {
    expect(parseStageLabel("start")).toBe("Hazırlanıyor");
    expect(parseStageLabel("open")).toBe("Belge açıldı");
    expect(parseStageLabel("pages")).toBe("Sayfalar okunuyor");
    expect(parseStageLabel("store")).toBe("Kasaya yazılıyor");
  });

  it("bilinmeyen aşamayı olduğu gibi bırakır (uydurma etiket yok)", () => {
    expect(parseStageLabel("extract")).toBe("extract");
  });

  it("boş aşamada nötr etiket döner", () => {
    expect(parseStageLabel("   ")).toBe("Ayrıştırılıyor");
  });
});

describe("parseProgressPercent", () => {
  it("sayılan/toplam oranını yuvarlar", () => {
    expect(parseProgressPercent(progress({ done: 12, total: 40 }))).toBe(30);
    expect(parseProgressPercent(progress({ done: 1, total: 3 }))).toBe(33);
  });

  it("toplam yoksa oran üretmez", () => {
    expect(parseProgressPercent(progress({ done: 5, total: null }))).toBeNull();
    expect(parseProgressPercent(progress({ total: 0 }))).toBeNull();
  });

  it("toplamı aşan sayacı %100'de tutar", () => {
    expect(parseProgressPercent(progress({ done: 50, total: 40 }))).toBe(100);
  });
});

describe("parseProgressLine", () => {
  it("ilerleme yoksa satır üretmez", () => {
    expect(parseProgressLine(null)).toBeNull();
  });

  it("toplam varken sayılan/toplam ve yüzde yazar", () => {
    expect(parseProgressLine(progress({ done: 12, total: 40 }))).toBe(
      "Sayfalar okunuyor: 12 / 40 · %30",
    );
  });

  it("toplam yokken yalnız sayılanı yazar", () => {
    expect(parseProgressLine(progress({ stage: "start", done: 0, total: null }))).toBe("Hazırlanıyor");
    expect(parseProgressLine(progress({ stage: "store", done: 3, total: null }))).toBe(
      "Kasaya yazılıyor: 3",
    );
  });

  it("bozuk sayaçta toplamı aşan değeri göstermez", () => {
    expect(parseProgressLine(progress({ done: 90, total: 40 }))).toBe(
      "Sayfalar okunuyor: 40 / 40 · %100",
    );
  });
});

describe("parseProgressWidth", () => {
  it("çubuk genişliğini 0–100 aralığında verir", () => {
    expect(parseProgressWidth(progress({ done: 0, total: 10 }))).toBe(0);
    expect(parseProgressWidth(progress({ done: 5, total: 10 }))).toBe(50);
    expect(parseProgressWidth(null)).toBeNull();
  });
});
