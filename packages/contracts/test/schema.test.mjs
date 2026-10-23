import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const dir = new URL("../schemas", import.meta.url).pathname;
const files = readdirSync(dir).filter((f) => f.endsWith(".schema.json"));

function load(file) {
  return JSON.parse(readFileSync(join(dir, file), "utf8"));
}

function collectRefs(node, acc = []) {
  if (Array.isArray(node)) {
    for (const item of node) collectRefs(item, acc);
  } else if (node && typeof node === "object") {
    for (const [k, v] of Object.entries(node)) {
      if (k === "$ref") acc.push(v);
      else collectRefs(v, acc);
    }
  }
  return acc;
}

test("her şema sürümlü $id ve başlık taşır", () => {
  for (const file of files) {
    const s = load(file);
    assert.ok(s.$id, `${file}: $id yok`);
    assert.match(s.$id, /^https:\/\/dim\.local\/schemas\/.+\.v1\.schema\.json$/);
    assert.ok(s.title, `${file}: title yok`);
  }
});

test("tüm $ref hedefleri çözülür", () => {
  const defsByFile = new Map(files.map((f) => [f, new Set(Object.keys(load(f).$defs ?? {}))]));
  for (const file of files) {
    for (const ref of collectRefs(load(file))) {
      const [target, pointer] = ref.split("#");
      const name = pointer?.replace(/^\/\$defs\//, "");
      const targetFile = target || file;
      assert.ok(defsByFile.has(targetFile), `${file}: bilinmeyen şema dosyası ${targetFile}`);
      if (name) {
        assert.ok(defsByFile.get(targetFile).has(name), `${file}: çözülemeyen $ref ${ref}`);
      } else {
        // Dosya köküne işaret eden $ref: hedef dosyanın ana tipi kullanılır.
        assert.ok(load(targetFile).title, `${file}: kök $ref hedefi başlıksız: ${ref}`);
      }
    }
  }
});

test("sourceRef zorunlu alanları eksiksiz", () => {
  const common = load("common.v1.schema.json");
  const required = new Set(common.$defs.SourceRef.required);
  for (const key of ["caseId", "documentVersionId", "parseRunId", "segmentId", "location", "textRange", "exactQuote", "quoteHash"]) {
    assert.ok(required.has(key), `SourceRef.${key} zorunlu olmalı`);
  }
});

test("worker frame 1 MiB üstünü kabul etmez (şema belgesi)", () => {
  const frame = load("worker-frame.v1.schema.json");
  const request = frame.oneOf[0];
  assert.equal(request.properties.v.const, 1);
  assert.ok(request.properties.method.enum.includes("ping"));
});
