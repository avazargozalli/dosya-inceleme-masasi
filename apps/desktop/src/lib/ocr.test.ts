// OCR dili kapısı: varsayılan dil **tek kaynak** (Rust) ile aynı olmalı.
//
// Ölçülen kusur: aynı belge tek ayrıştırmada `tur+eng`, toplu ayrıştırmada ve
// eski iş yeniden denemesinde `eng` ile okunuyordu. `eng` modeli Türkçe
// diyakritiklerin **0/79**'ını koruyor (F15 ölçümü) — yani aynı belge iki
// yoldan biriyle sessizce bozuluyordu. Bu test üç aynayı birbirine bağlar.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { DEFAULT_OCR_LANG, defaultOcrLanguageLabel, ocrLanguageLabel } from "./ocr";

const REPO = fileURLToPath(new URL("../../../../", import.meta.url));

function read(path: string): string {
  return readFileSync(`${REPO}${path}`, "utf8");
}

/** Rust sabitini kaynaktan okur: `pub const DEFAULT_OCR_LANG: &str = "…";` */
function rustDefaultOcrLang(): string {
  const source = read("crates/storage/src/queue.rs");
  const match = source.match(/pub const DEFAULT_OCR_LANG: &str = "([^"]+)";/);
  if (!match) throw new Error("queue.rs içinde DEFAULT_OCR_LANG bulunamadı");
  return match[1]!;
}

/** Python aynasını kaynaktan okur: `DEFAULT_OCR_LANG = "…"` */
function pythonDefaultOcrLang(): string {
  const source = read("workers/document/dim_worker/engines.py");
  const match = source.match(/^DEFAULT_OCR_LANG = "([^"]+)"/m);
  if (!match) throw new Error("engines.py içinde DEFAULT_OCR_LANG bulunamadı");
  return match[1]!;
}

describe("OCR dili tek kaynak", () => {
  it("arayüz varsayılanı Rust sabitiyle aynı", () => {
    expect(DEFAULT_OCR_LANG).toBe(rustDefaultOcrLang());
  });

  it("worker varsayılanı Rust sabitiyle aynı", () => {
    expect(pythonDefaultOcrLang()).toBe(DEFAULT_OCR_LANG);
  });

  it("varsayılan Türkçe modelini içerir (diyakritik kaybı olmasın)", () => {
    expect(DEFAULT_OCR_LANG.split("+").map((code) => code.trim())).toContain("tur");
  });

  it("arayüzde dil kodu gömülü kalmaz: api varsayılanı sabitten gelir", () => {
    const api = read("apps/desktop/src/lib/api.ts");
    expect(api).toMatch(/ocrLang = DEFAULT_OCR_LANG/);
    expect(api).not.toMatch(/ocrLang = "eng"/);
  });
});

describe("OCR dili etiketi", () => {
  it("birleşik dili okunur yazar", () => {
    expect(ocrLanguageLabel("tur+eng")).toBe("Türkçe + İngilizce");
  });

  it("tek dili okunur yazar", () => {
    expect(ocrLanguageLabel("tur")).toBe("Türkçe");
  });

  it("tanınmayan kodu uydurmadan olduğu gibi yazar", () => {
    expect(ocrLanguageLabel("aze_cyrl")).toBe("aze_cyrl");
  });

  it("varsayılan etiket Türkçe'yi adıyla söyler", () => {
    expect(defaultOcrLanguageLabel()).toBe("Türkçe + İngilizce");
  });
});
