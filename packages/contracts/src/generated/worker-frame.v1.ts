// AUTO-GENERATED — elle düzenlemeyin. Kaynak: schemas/worker-frame.v1.schema.json
// Üretim: node packages/contracts/scripts/generate.mjs --write

import type { ErrorBody } from "./error.v1.js";

export type WorkerFrame = {
  "v": 1;
  "id": string;
  "method": "ping" | "capabilities" | "extract_text" | "shutdown";
  "params"?: {
};
} | {
  "v": 1;
  "id": string | null;
  "ok": boolean;
  "result"?: {
};
  "error"?: ErrorBody;
};
