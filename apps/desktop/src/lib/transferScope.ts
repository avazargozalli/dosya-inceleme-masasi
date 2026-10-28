import type { ConsentPreview, ScopeSkip } from "./api";
import { formatSize } from "./format";

/**
 * Kapsam dışı kalma nedeninin okunur etiketi. Sunucu yeni bir neden eklerse
 * "bulunamadı" der — bilinmeyen neden **sessizce yutulmaz**, sayılır.
 */
export function skipReasonLabel(reason: string): string {
  switch (reason) {
    case "not_parsed":
      return "ayrıştırılmamış";
    case "empty_text":
      return "metin yok";
    case "no_current_version":
      return "sürüm yok";
    case "missing":
      return "bulunamadı";
    default:
      return "bulunamadı";
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
  return `Gönderilmeyen ${skipped.length} belge: ${parts.join(", ")}`;
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
    return `Onay kapsamı: ${selectedDocuments} belge · içerik önizlemesi alınmadı`;
  }
  return `Onay kapsamı: ${selectedDocuments} belge · gönderilecek: ${preview.documents} belge, ${preview.segments} segment, ${formatSize(preview.bytes)}`;
}
