// AUTO-GENERATED — elle düzenlemeyin. Kaynak: schemas/error.v1.schema.json
// Üretim: node packages/contracts/scripts/generate.mjs --write

export type ErrorBody = {
  /** Kararlı hata kodu; ham hata metni yerine bu kullanılır. */
  "code": string;
  /** Kullanıcıya gösterilebilir, gizli veri içermeyen açıklama. */
  "message": string;
  "retryable": boolean;
  /** Allowlist'e tabi yapısal ayrıntı; belge metni içermez. */
  "details"?: {
};
};
