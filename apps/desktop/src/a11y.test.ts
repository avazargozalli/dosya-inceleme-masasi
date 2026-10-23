// Erişilebilirlik regresyonu: etiketsiz form alanı, altsız görsel ve metinsiz
// buton arayüze sızarsa burada yakalanır. Kurallar bilerek muhafazakâr: bir
// alanın `id`'si varsa aynı dosyada `htmlFor` ya da `aria-label` beklenir.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const SRC = fileURLToPath(new URL(".", import.meta.url));

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    const info = statSync(path);
    if (info.isDirectory()) {
      out.push(...walk(path));
      continue;
    }
    if (name.endsWith(".tsx")) out.push(path);
  }
  return out;
}

const files = walk(SRC).map((path) => ({
  path: path.replace(SRC, ""),
  source: readFileSync(path, "utf8"),
}));

describe("erişilebilirlik", () => {
  it("her form alanının erişilebilir adı var", () => {
    const problems: string[] = [];
    for (const { path, source } of files) {
      for (const match of source.matchAll(/<(input|select|textarea)\b[^>]*?\bid="([^"]+)"/g)) {
        const id = match[2]!;
        const hasLabel = source.includes(`htmlFor="${id}"`) || source.includes(`htmlFor={\`${id}\`}`);
        const hasAria =
          source.includes(`aria-label`) && source.includes(`id="${id}"`) ? true : false;
        if (!hasLabel && !hasAria) problems.push(`${path}: <${match[1]} id="${id}">`);
      }
    }
    expect(problems).toEqual([]);
  });

  it("her görselin alt metni var", () => {
    const problems: string[] = [];
    for (const { path, source } of files) {
      for (const match of source.matchAll(/<img\b[^>]*>/g)) {
        if (!/\balt=/.test(match[0])) problems.push(`${path}: ${match[0].slice(0, 60)}`);
      }
    }
    expect(problems).toEqual([]);
  });

  it("her butonun erişilebilir adı var", () => {
    const problems: string[] = [];
    for (const { path, source } of files) {
      // Kendi kapanan ya da içi boş butonlar; ikon-only butonlar aria-label taşımalı.
      for (const match of source.matchAll(/<button\b([^>]*)>(\s*)<\/button>/g)) {
        if (!/aria-label=/.test(match[1]!)) problems.push(`${path}: boş <button${match[1]}>`);
      }
    }
    expect(problems).toEqual([]);
  });

});
