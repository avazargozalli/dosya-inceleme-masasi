// AUTO-GENERATED — elle düzenlemeyin. Kaynak: schemas/review-item.v1.schema.json
// Üretim: node packages/contracts/scripts/generate.mjs --write

import type { IsoTimestamp } from "./common.v1.js";
import type { Uuid } from "./common.v1.js";

export type ReviewItem = {
  "id": Uuid;
  "caseId": Uuid;
  "itemKind": "event_candidate" | "claim_candidate" | "note_candidate" | "task_candidate";
  "payloadSchemaVersion": 1;
  "payload": {
};
  "sourceAnchorIds": Array<Uuid>;
  "aiRunId"?: string | null;
  "state": "pending" | "accepted" | "rejected" | "edited" | "needs_more_evidence";
  "targetEntityId"?: string | null;
  "createdAt": IsoTimestamp;
};
