// AUTO-GENERATED — elle düzenlemeyin. Kaynak: schemas/case.v1.schema.json
// Üretim: node packages/contracts/scripts/generate.mjs --write

import type { IsoTimestamp } from "./common.v1.js";
import type { Uuid } from "./common.v1.js";

export type Case = {
  "id": Uuid;
  "title": string;
  "caseKind": "review" | "reference_archive";
  /** Esas numarası dosyanın tek kimliği değildir. */
  "referenceCode"?: string | null;
  "jurisdictionLabel"?: string | null;
  "practiceArea"?: string | null;
  "status": "active" | "archived" | "deleted";
  "sensitivity": "normal" | "high" | "client_confidential";
  "defaultAiPolicy": "off" | "local_only" | "cli_allowed" | "api_allowed";
  "archivedAt"?: string | null;
  "retentionHold"?: boolean;
  "createdAt": IsoTimestamp;
  "updatedAt": IsoTimestamp;
  "rowVersion": number;
};
