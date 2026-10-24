import type { DocumentVersionRow, PrecedentDocumentText } from "./api";
import { formatTimestamp } from "./format";

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
  return `v${version.versionNo} · ${formatTimestamp(version.importedAt)}`;
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
  if (!input.hasPrecedent) return "Önce bir emsal seçin.";
  if (input.documentCount === 0) return "Kasadaki bu dosyada belge yok.";
  if (input.versionCount === 0) return "Seçilen belgenin sürümü yok.";
  if (input.selectedVersionId.trim().length === 0) return "Aktarılacak sürümü seçin.";
  return null;
}

/**
 * Sonuç özeti: `12.03.2025 14:05 · 12 sayfa · 8.412 karakter · sha256 ab12…`.
 * Sürüm numarası göstermiyoruz: emsal sürümünün kendi sırası yok, kimliğin son
 * dört hanesi "v3" gibi okunurdu (uydurma numara).
 */
export function documentTextSummary(result: PrecedentDocumentText): string {
  const hash = result.version.bodyHash.slice(0, 12);
  return `${formatTimestamp(result.version.acquiredAt)} · ${result.pages} sayfa · ${result.charCount} karakter · sha256 ${hash}…`;
}

/** Kısmi ayrıştırma / OCR / metinsiz sayfa uyarıları; yoksa boş dizi. */
export function documentTextWarnings(result: PrecedentDocumentText): string[] {
  const warnings: string[] = [];
  if (result.parseStatus === "partial") {
    warnings.push("Ayrıştırma kısmi (partial): eksik sayfa olabilir.");
  }
  if (result.ocrPages > 0) {
    warnings.push(
      `${result.ocrPages} sayfa OCR ile çıkarıldı; metin görüntüden okundu, yazım hataları olabilir.`,
    );
  }
  if (result.emptyPages > 0) {
    warnings.push(`${result.emptyPages} sayfa metinsizdi ve birleştirmeye alınmadı.`);
  }
  if (!result.written) {
    warnings.push("Aynı metin (aynı SHA-256) zaten kayıtlıydı; yeni sürüm yazılmadı.");
  }
  return warnings;
}
