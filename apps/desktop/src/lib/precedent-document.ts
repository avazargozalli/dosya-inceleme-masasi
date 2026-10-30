import type { DocumentVersionRow, PrecedentDocumentText } from "./api";
import { formatTimestamp } from "./format";
import { t } from "./i18n";

/**
 * F12-T13 — kasadaki belgeden emsale tam metin aktarma metinleri.
 *
 * Kural: aktarım "belgenin kopyası" değildir; ayrıştırılmış sayfa metni
 * birleştirilir. OCR ve metinsiz sayfa sayısı gizlenmez, kısmi ayrıştırma
 * "tam" gibi gösterilmez.
 */

/** Sürüm seçeneği: `v2 · 12.03.2025 14:05`. Numarayı şema verir, uydurulmaz. */
export function versionOptionLabel(
  version: Pick<DocumentVersionRow, "versionNo" | "importedAt">,
): string {
  return t("precedentDoc.versionOption", {
    version: `v${version.versionNo}`,
    at: formatTimestamp(version.importedAt),
  });
}

/**
 * Aktarım düğmesinin neden kapalı olduğunu söyler; `null` = açık.
 *
 * Sürüm seçilmeden aktarım başlamaz; varsayılan seçim kutuda görünür, gizli
 * bir "en yeni sürümü al" davranışı yoktur.
 */
export function transferBlockedReason(input: {
  hasPrecedent: boolean;
  documentCount: number;
  versionCount: number;
  selectedVersionId: string;
}): string | null {
  if (!input.hasPrecedent) return t("precedentDoc.blockedNoPrecedent");
  if (input.documentCount === 0) return t("precedentDoc.blockedNoDocument");
  if (input.versionCount === 0) return t("precedentDoc.blockedNoVersion");
  if (input.selectedVersionId.trim().length === 0) return t("precedentDoc.blockedNoSelection");
  return null;
}

/**
 * Sonuç özeti: `12.03.2025 14:05 · 12 sayfa · 8.412 karakter · sha256 ab12…`.
 * Sürüm numarası göstermiyoruz: emsal sürümünün kendi sırası yok, kimliğin son
 * dört hanesi "v3" gibi okunurdu (uydurma numara).
 */
export function documentTextSummary(result: PrecedentDocumentText): string {
  return t("precedentDoc.summary", {
    at: formatTimestamp(result.version.acquiredAt),
    pages: result.pages,
    characters: result.charCount,
    hash: result.version.bodyHash.slice(0, 12),
  });
}

/** Kısmi ayrıştırma / OCR / metinsiz sayfa uyarıları; yoksa boş dizi. */
export function documentTextWarnings(result: PrecedentDocumentText): string[] {
  const warnings: string[] = [];
  if (result.parseStatus === "partial") {
    warnings.push(t("precedentDoc.warnPartial"));
  }
  if (result.ocrPages > 0) {
    warnings.push(t("precedentDoc.warnOcr", { pages: result.ocrPages }));
  }
  if (result.emptyPages > 0) {
    warnings.push(t("precedentDoc.warnEmpty", { pages: result.emptyPages }));
  }
  if (!result.written) {
    warnings.push(t("precedentDoc.warnDuplicate"));
  }
  return warnings;
}
