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

const FORBIDDEN: Array<{ name: string; pattern: RegExp }> = [
  { name: "toLocaleString", pattern: /toLocaleString\s*\(/ },
  { name: "toLocaleDateString", pattern: /toLocaleDateString\s*\(/ },
  { name: "toLocaleTimeString", pattern: /toLocaleTimeString\s*\(/ },
  { name: "new Intl.", pattern: /new Intl\./ },
  { name: "tr-TR sabiti", pattern: /["']tr-TR["']/ },
];

describe("yerelleştirme kapısı", () => {
  it("tarih/sayı biçimlendirmesi yalnız lib/format.ts içinde", () => {
    const offences: string[] = [];
    for (const { rel, source } of files) {
      if (rel === FORMAT_MODULE) continue;
      const lines = source.split("\n");
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

  it("sözlüğe taşınmış ekranlara gömülü metin geri sızmaz", () => {
    const migrated = ["App.tsx", "screens/Unlock.tsx", "screens/Workspace.tsx"];
    const leakedLiterals = [
      "Kasa açık",
      "Kasa kilitli",
      "Kasayı kilitle",
      "Kurtarma anahtarını şimdi kaydet",
      "Kullanmaya başla",
      "Anahtarı güvenli bir yere kaydettim.",
      "Bu fazda ne çalışıyor?",
      "Hazır değil — sahte veri veya pasif buton üretilmez",
    ];
    const leaks: string[] = [];
    for (const { rel, source } of files) {
      if (!migrated.includes(rel)) continue;
      for (const literal of leakedLiterals) {
        if (source.includes(literal)) leaks.push(`${rel}: ${literal}`);
      }
    }
    expect(leaks).toEqual([]);
  });

  it("kabuk sürümü paketten okunur, gömülü sürüm etiketi yok", () => {
    const app = files.find(({ rel }) => rel === "App.tsx");
    expect(app).toBeDefined();
    expect(app!.source).toContain('from "../package.json"');
    expect(app!.source).not.toContain("v0.1.0 · F02");
  });
});
