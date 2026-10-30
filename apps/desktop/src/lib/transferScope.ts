import type { ConsentPreview, ScopeSkip } from "./api";
import { formatSize } from "./format";
import { t } from "./i18n";

/**
 * Kapsam dışı kalma nedeninin okunur etiketi. Sunucu yeni bir neden eklerse
 * "bulunamadı" der — bilinmeyen neden **sessizce yutulmaz**, sayılır.
 */
export function skipReasonLabel(reason: string): string {
  switch (reason) {
    case "not_parsed":
      return t("transferScope.skip.notParsed");
    case "empty_text":
      return t("transferScope.skip.emptyText");
    case "no_current_version":
      return t("transferScope.skip.noVersion");
    case "missing":
      return t("transferScope.skip.missing");
    default:
      return t("transferScope.skip.missing");
  }
}

/** "Gönderilmeyen 2 belge: 1 ayrıştırılmamış, 1 metin yok" — boşsa boş dize. */
export function skippedSummary(skipped: readonly ScopeSkip[]): string {
  if (skipped.length === 0) return "";
  const counts = new Map<string, number>();
  for (const skip of skipped) {
    const label = skipReasonLabel(skip.reason);
    counts.set(label, (counts.get(label) ?? 0) + 1);
  }
  const parts = [...counts.entries()].map(([label, count]) => `${count} ${label}`);
  return t("transferScope.skippedSummary", { total: skipped.length, parts: parts.join(", ") });
}

/**
 * Onayın kapsadığı belge sayısı ile **gerçekten** gönderilecek içeriği birlikte
 * yazar. Önizleme yoksa yalnız onay sayısı; ikisi çelişirse sayı **açıkça**
 * yazılır (kullanıcı onayladığından fazlasının gittiğini sanmaz).
 */
export function consentScopeLine(
  selectedDocuments: number,
  preview: ConsentPreview | null,
): string {
  if (!preview) {
    return t("transferScope.scopeNoPreview", { documents: selectedDocuments });
  }
  return t("transferScope.scopeWithPreview", {
    documents: selectedDocuments,
    previewDocuments: preview.documents,
    segments: preview.segments,
    bytes: formatSize(preview.bytes),
  });
}
