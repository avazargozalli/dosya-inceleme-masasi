// AUTO-GENERATED — elle düzenlemeyin. Kaynak: schemas/common.v1.schema.json
// Üretim: node packages/contracts/scripts/generate.mjs --write

/** UUIDv7; sıralanabilir, anlam taşımayan kimlik. */
export type Uuid = string;

/** UTC ISO-8601 teknik zaman. */
export type IsoTimestamp = string;

/** Unicode scalar aralığı; bayt veya UTF-16 değil. */
export type TextRange = {
  "start": number;
  "end": number;
};

export type Location = {
  "kind": "pdf";
  "pageIndex": number;
  "pageLabel"?: string;
  "quads": Array<Array<number>>;
  "coordinateSpace": "unrotated_page_points";
} | {
  "kind": "structure";
  "part": string;
  "elementPath": string;
  "paragraphIndex"?: number;
} | {
  "kind": "image";
  "frameIndex": number;
  "quads": Array<Array<number>>;
};

/** Alıntının sürüm, konum ve metin bağı. Çekirdek doğrulamadan kabul edilmez. */
export type SourceRef = {
  "caseId": Uuid;
  "documentVersionId": Uuid;
  "parseRunId": Uuid;
  "segmentId": Uuid;
  "location": Location;
  "textRange": TextRange;
  "exactQuote": string;
  "quoteHash": string;
  "prefix": string;
  "suffix": string;
};


export type Common = {
};
