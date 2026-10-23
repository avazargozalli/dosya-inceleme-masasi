#!/usr/bin/env node
// Sürümlü JSON Schema → TypeScript tip üreticisi.
//
// Kasıtlı olarak bağımlılıksızdır: üretim hattı node_modules olmadan da
// çalışır. Desteklenen alt küme dar tutulur; desteklenmeyen anahtar
// gördüğünde sessizce geçmez, hata verir (şema sessizce kaybolmasın).
import { readFileSync, writeFileSync, readdirSync, existsSync } from "node:fs";
import { join, basename } from "node:path";

const ROOT = new URL("..", import.meta.url).pathname;
const SCHEMA_DIR = join(ROOT, "schemas");
const OUT_DIR = join(ROOT, "src", "generated");
const CHECK = process.argv.includes("--check");

const IGNORED_KEYS = new Set([
  "$schema", "$id", "title", "description", "$defs", "$comment", "deprecated", "examples", "default",
]);

function fail(msg) {
  console.error(`kod üretimi hatası: ${msg}`);
  process.exit(1);
}

const TITLES = new Map();

function schemaTitle(file) {
  if (!TITLES.has(file)) {
    const raw = JSON.parse(readFileSync(join(SCHEMA_DIR, file), "utf8"));
    TITLES.set(file, raw.title ?? file.replace(/\.schema\.json$/, ""));
  }
  return TITLES.get(file);
}

function refName(ref) {
  const [file, pointer] = ref.split("#");
  const from = file ? basename(file, ".schema.json") : null;
  const target = pointer?.replace(/^\/\$defs\//, "");
  if (target) return { from, name: target };
  // Dosya köküne işaret eden $ref: dosyanın ana tipi (title) kullanılır.
  if (from) return { from, name: schemaTitle(`${from}.schema.json`) };
  fail(`desteklenmeyen $ref: ${ref}`);
}

function unionType(list, ctx) {
  const parts = list.map((s) => tsType(s, ctx));
  return [...new Set(parts)].join(" | ");
}

function tsType(schema, ctx) {
  if (schema === true) return "unknown";
  if (!schema || typeof schema !== "object") fail(`geçersiz şema düğümü: ${JSON.stringify(schema)}`);

  if ("$ref" in schema) {
    const { from, name } = refName(schema.$ref);
    if (from && from !== ctx.self) {
      ctx.imports.add(`import type { ${name} } from "./${from}.js";`);
    }
    return name;
  }
  if ("const" in schema) return JSON.stringify(schema.const);
  if (Array.isArray(schema.enum)) return schema.enum.map((v) => JSON.stringify(v)).join(" | ");
  if (schema.oneOf) return unionType(schema.oneOf, ctx);
  if (schema.anyOf) return unionType(schema.anyOf, ctx);
  if (Array.isArray(schema.type)) {
    return schema.type.map((t) => tsType({ ...schema, type: t }, ctx)).join(" | ");
  }

  switch (schema.type) {
    case "string":
      return "string";
    case "integer":
    case "number":
      return "number";
    case "boolean":
      return "boolean";
    case "null":
      return "null";
    case "array": {
      const inner = schema.items ? tsType(schema.items, ctx) : "unknown";
      return `Array<${inner}>`;
    }
    case "object":
      return objectType(schema, ctx);
    case undefined:
      // Yalnız açıklama taşıyan düğüm: bilinçli olarak unknown.
      return "unknown";
    default:
      fail(`desteklenmeyen tip: ${schema.type}`);
  }
}

function objectType(schema, ctx) {
  const props = schema.properties ?? {};
  const required = new Set(schema.required ?? []);
  const lines = ["{"];
  for (const [key, value] of Object.entries(props)) {
    const doc = value.description ? `  /** ${value.description} */\n` : "";
    const opt = required.has(key) ? "" : "?";
    lines.push(`${doc}  ${JSON.stringify(key)}${opt}: ${tsType(value, ctx)};`);
  }
  lines.push("}");
  return lines.join("\n");
}

function generate(file) {
  const raw = readFileSync(join(SCHEMA_DIR, file), "utf8");
  let schema;
  try {
    schema = JSON.parse(raw);
  } catch (e) {
    fail(`${file} geçerli JSON değil: ${e.message}`);
  }
  const self = basename(file, ".schema.json");
  const ctx = { self, imports: new Set() };
  const name = schema.title ?? self;

  const chunks = [
    `// AUTO-GENERATED — elle düzenlemeyin. Kaynak: schemas/${file}`,
    `// Üretim: node packages/contracts/scripts/generate.mjs --write`,
    "",
  ];

  const defs = schema.$defs ?? {};
  let mainType;
  if (schema.type === "object" && schema.properties) {
    mainType = objectType(schema, ctx);
  } else if (schema.oneOf || schema.anyOf) {
    mainType = tsType(schema, ctx);
  } else {
    mainType = tsType(schema, ctx);
  }

  const body = [];
  for (const [key, value] of Object.entries(defs)) {
    const doc = value.description ? `/** ${value.description} */\n` : "";
    body.push(`${doc}export type ${key} = ${tsType(value, ctx)};\n`);
  }
  if (Object.keys(defs).length > 0) body.push("");
  body.push(`export type ${name} = ${mainType};\n`);

  // Importlar gövdeden önce, deterministik sırayla.
  const imports = [...ctx.imports].sort();
  return [...chunks, ...imports, ...(imports.length ? [""] : []), ...body].join("\n");
}

const files = readdirSync(SCHEMA_DIR).filter((f) => f.endsWith(".schema.json")).sort();
if (files.length === 0) fail("schemas/ altında şema yok");

let dirty = [];
const exports = [];
for (const file of files) {
  const out = generate(file);
  const target = join(OUT_DIR, file.replace(/\.schema\.json$/, ".ts"));
  exports.push(`export * from "./generated/${file.replace(/\.schema\.json$/, ".js")}";`);
  if (CHECK) {
    if (!existsSync(target) || readFileSync(target, "utf8") !== out) dirty.push(target);
  } else {
    writeFileSync(target, out, "utf8");
  }
}

const indexPath = join(ROOT, "src", "index.ts");
const indexBody = ["// AUTO-GENERATED — kaynak: schemas/*.schema.json", ...exports.sort(), ""].join("\n");
if (CHECK) {
  if (!existsSync(indexPath) || readFileSync(indexPath, "utf8") !== indexBody) dirty.push(indexPath);
  if (dirty.length) {
    console.error("Sözleşme dosyaları güncel değil:\n" + dirty.map((d) => " - " + d).join("\n"));
    console.error("Düzeltme: pnpm --filter @dim/contracts generate");
    process.exit(1);
  }
  console.log(`sözleşmeler güncel (${files.length} şema)`);
} else {
  writeFileSync(indexPath, indexBody, "utf8");
  console.log(`üretildi: ${files.length} şema`);
}
