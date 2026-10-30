// F04-T08 kapısı: yerelleştirme **tek yerde** yaşar.
//
// 1) Tarih/sayı biçimlendirmesi `lib/format.ts` dışında yapılmaz — bileşenler
//    `toLocaleString`/`new Intl` çağırırsa aynı değer iki ayrı biçimde görünür
//    (ölçüldü: "1.5 KB" / "1,5 KiB" / "1.5 MB").
// 2) Sözlük anahtarı kullanılmadan bırakılmaz (çürüyen sözlük sessizce büyür).
// 3) Sözlüğe taşınmış ekranlara gömülü metin geri sızmaz.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { messageKeys } from "./i18n";

const SRC = fileURLToPath(new URL("..", import.meta.url));

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      out.push(...walk(path));
      continue;
    }
    if (name.endsWith(".ts") || name.endsWith(".tsx")) out.push(path);
  }
  return out;
}

const files = walk(SRC)
  .filter((path) => !path.includes(".test."))
  .map((path) => ({ path, rel: relative(SRC, path).replaceAll("\\", "/"), source: readFileSync(path, "utf8") }));

/** Biçimlendirme yalnız burada yapılır. */
const FORMAT_MODULE = "lib/format.ts";

/**
 * Yorumları ayıklar; **satır sayısı korunur** (rapor edilen satır numarası koda
 * işaret etsin). Blok yorumlar boşlukla, satır başı `//` yorumları boş satırla
 * değiştirilir.
 *
 * Neden: kapı metni ham kaynakta arıyordu ve `/// Kasa kilitliyken …` gibi bir
 * **yorum** "gömülü metin sızdı" diye kırmızı verdi (2026-10-28'de iki CI koşusu
 * bu yüzden düştü — kusur #62). Yorum kullanıcıya görünmez; kapı yalnız gerçekten
 * render edilen metni aramalıdır.
 *
 * Sınır: satır **sonundaki** `// …` yorumu ayıklanmaz (dize içindeki `://` ile
 * karışmaması için bilinçli); böyle bir yorumda gömülü metin geçerse kapı yine
 * yanlış pozitif verir.
 */
export function stripComments(source: string): string {
  const withoutBlocks = source.replace(/\/\*[\s\S]*?\*\//g, (block) =>
    block.replace(/[^\n]/g, " "),
  );
  return withoutBlocks
    .split("\n")
    .map((line) => (line.trimStart().startsWith("//") ? "" : line))
    .join("\n");
}

const FORBIDDEN: Array<{ name: string; pattern: RegExp }> = [
  { name: "toLocaleString", pattern: /toLocaleString\s*\(/ },
  { name: "toLocaleDateString", pattern: /toLocaleDateString\s*\(/ },
  { name: "toLocaleTimeString", pattern: /toLocaleTimeString\s*\(/ },
  { name: "new Intl.", pattern: /new Intl\./ },
  { name: "tr-TR sabiti", pattern: /["']tr-TR["']/ },
];

/** Sözlüğe taşınmış dosyalar: gömülü Türkçe metin taşımaz. */
const MIGRATED = [
  "App.tsx",
  "screens/Unlock.tsx",
  "screens/Workspace.tsx",
  "screens/Tasks.tsx",
  "screens/Exports.tsx",
  "screens/Redaction.tsx",
  "screens/Timeline.tsx",
  "screens/Checklists.tsx",
  "screens/Ai.tsx",
  "lib/checklist.ts",
  "lib/candidate.ts",
  "lib/transferScope.ts",
];

const TURKISH_LETTER = /[ığüşöçİĞÜŞÖÇ]/;

/** Yorumlar ayıklandıktan sonra Türkçe harf taşıyan satır numaraları. */
export function turkishLines(source: string): number[] {
  const out: number[] = [];
  stripComments(source)
    .split("\n")
    .forEach((line, index) => {
      if (TURKISH_LETTER.test(line)) out.push(index + 1);
    });
  return out;
}

describe("yerelleştirme kapısı", () => {
  it("tarih/sayı biçimlendirmesi yalnız lib/format.ts içinde", () => {
    const offences: string[] = [];
    for (const { rel, source } of files) {
      if (rel === FORMAT_MODULE) continue;
      const lines = stripComments(source).split("\n");
      for (const { name, pattern } of FORBIDDEN) {
        lines.forEach((line, index) => {
          if (pattern.test(line)) offences.push(`${rel}:${index + 1} ${name}`);
        });
      }
    }
    expect(offences).toEqual([]);
  });

  it("sözlükte kullanılmayan anahtar kalmaz", () => {
    const haystack = files
      .filter(({ rel }) => rel !== "lib/i18n.ts")
      .map(({ source }) => source)
      .join("\n");
    // `labels.*` anahtarları `format.ts`te harita olarak dışa açılır; metin
    // anahtarı olarak geçmezler.
    const unused = messageKeys().filter(
      (key) => !key.startsWith("labels.") && !haystack.includes(`"${key}"`),
    );
    expect(unused).toEqual([]);
  });

  it("sözlüğe taşınmış ekranlarda gömülü Türkçe metin kalmaz", () => {
    const leaks: string[] = [];
    for (const { rel, source } of files) {
      if (!MIGRATED.includes(rel)) continue;
      for (const line of turkishLines(source)) leaks.push(`${rel}:${line}`);
    }
    expect(leaks).toEqual([]);
  });

  it("gömülü metin dedektörü gerçekten yakalar (canary)", () => {
    // Geçmişte tek tek metin listesiyle aranıyordu ve yeni bir metin sızınca
    // kapı sessiz kalıyordu. Bu ölçüm dedektörün kör olmadığını gösterir.
    expect(turkishLines('const label = "Görev listesi";')).toEqual([1]);
    expect(turkishLines('const label = {t("tasks.listTitle")};')).toEqual([]);
    expect(turkishLines('// yorumda Türkçe serbest\nconst x = 1;')).toEqual([]);
  });

  it("kabuk sürümü paketten okunur, gömülü sürüm etiketi yok", () => {
    const app = files.find(({ rel }) => rel === "App.tsx");
    expect(app).toBeDefined();
    expect(app!.source).toContain('from "../package.json"');
    expect(stripComments(app!.source)).not.toContain("v0.1.0 · F02");
  });
});

describe("yorum ayıklama (kapı yanlış pozitif üretmesin)", () => {
  it("satır başı ve blok yorumdaki metin sızıntı sayılmaz", () => {
    const source = [
      "/// Kasa kilitliyken ekran ağacı sökülür.",
      "const a = 1;",
      "/* Kasa kilitli */",
      "const b = 2;",
    ].join("\n");
    const code = stripComments(source);
    expect(code).not.toContain("Kasa kilitli");
    expect(code).toContain("const a = 1;");
    expect(code).toContain("const b = 2;");
  });

  it("kod içindeki gerçek metin korunur (kapı hâlâ yakalar)", () => {
    expect(stripComments('const label = "Kasa kilitli";')).toContain("Kasa kilitli");
    expect(stripComments('throw new Error("toLocaleString(");')).toContain("toLocaleString(");
  });

  it("blok yorum satır numaralarını kaydırmaz", () => {
    const source = ["/*", " * toLocaleString(", " */", "const x = 1;"].join("\n");
    const lines = stripComments(source).split("\n");
    expect(lines).toHaveLength(4);
    expect(lines[3]).toBe("const x = 1;");
  });
});
