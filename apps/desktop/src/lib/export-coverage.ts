import type { ExportCoverage } from "./api";

/**
 * F11-T06 — kapsam bloğunun arayüz tarafı.
 *
 * Sayılar çekirdekten gelir (`case_coverage` / dışa aktarma yanıtı); buradaki
 * yardımcılar **yeniden hesaplamaz**, yalnız okunur biçime çevirir. Amaç
 * ekranda da tek cümleyi görünür tutmak: “incelenmedi ≠ temiz”.
 */

export type CoverageTone = "ok" | "warn" | "plain";

export type CoverageRow = {
  label: string;
  value: number;
  /** Sıfırdan farkı uyarı gerektiren satırlar (boş sayfa, ayrıştırılmamış…). */
  tone: CoverageTone;
};

export type CoverageState = {
  label: string;
  tone: CoverageTone;
  detail: string;
};

/** Rapor tablosunun satırları; sıra çekirdeğin `rows()` çıktısıyla aynıdır. */
export function coverageRows(coverage: ExportCoverage): CoverageRow[] {
  return [
    { label: "Belge", value: coverage.documents, tone: coverage.documents === 0 ? "warn" : "plain" },
    { label: "Belge sürümü", value: coverage.versions, tone: "plain" },
    { label: "Ayrıştırma koşusu", value: coverage.parseRuns, tone: "plain" },
    { label: "Sayfa", value: coverage.pages, tone: "plain" },
    { label: "Metinli sayfa", value: coverage.textPages, tone: "plain" },
    { label: "OCR ile okunan sayfa", value: coverage.ocrPages, tone: coverage.ocrPages > 0 ? "warn" : "plain" },
    { label: "Boş sayfa", value: coverage.emptyPages, tone: coverage.emptyPages > 0 ? "warn" : "plain" },
    { label: "Metin segmenti", value: coverage.segments, tone: "plain" },
    {
      label: "Ayrıştırılmamış belge",
      value: coverage.unparsedDocuments,
      tone: coverage.unparsedDocuments > 0 ? "warn" : "plain",
    },
    {
      label: "Başarısız ayrıştırma koşusu",
      value: coverage.failedParseRuns,
      tone: coverage.failedParseRuns > 0 ? "warn" : "plain",
    },
  ];
}

/** Tek satırlık özet (çekirdeğin `headline()` metniyle aynı biçim). */
export function coverageHeadline(coverage: ExportCoverage): string {
  return (
    `${coverage.documents} belge · ${coverage.pages} sayfa ` +
    `(${coverage.textPages} metinli, ${coverage.ocrPages} OCR, ${coverage.emptyPages} boş) · ` +
    `${coverage.unparsedDocuments} ayrıştırılmamış belge`
  );
}

/** Kapsam durumu rozeti. `complete=false` “temiz” anlamına gelmez. */
export function coverageState(coverage: ExportCoverage): CoverageState {
  if (coverage.documents === 0) {
    return {
      label: "Kapsam yok",
      tone: "warn",
      detail: "Dosyada belge yok; bu çıktı hiçbir incelemeyi kapsamaz.",
    };
  }
  if (coverage.complete) {
    return {
      label: "Kapsam tam",
      tone: "ok",
      detail: "Her belge ayrıştırıldı ve her sayfada metin var.",
    };
  }
  return {
    label: "Kapsam kısmi",
    tone: "warn",
    detail: `Kapsam dışı içerik var (${coverage.warnings.length} uyarı); incelenmedi ≠ temiz.`,
  };
}

/** Uyarılar çekirdekten gelir; arayüz kendi cümlesini uydurmaz. */
export function coverageWarnings(coverage: ExportCoverage): string[] {
  return coverage.warnings;
}

/** Kapsamın gömüldüğü çıktı türleri (arayüzde tek yerde yazılır). */
export const COVERAGE_EMBED_TARGETS: Array<{ id: string; label: string; where: string }> = [
  { id: "json", label: "JSON", where: "coverage bloğu" },
  { id: "csv", label: "CSV", where: "baştaki # yorum satırları" },
  { id: "docx", label: "DOCX", where: "“İncelenen kapsam” bölümü" },
  { id: "pdf", label: "PDF", where: "“İncelenen kapsam” bölümü" },
  { id: "package", label: "Devir paketi", where: "KAPSAM.md + manifest.coverage" },
];

/** Dışa aktarma sonrası “ne yazıldı” cümlesi. */
export function coverageEmbedNote(format: string, coverage: ExportCoverage): string {
  const target = COVERAGE_EMBED_TARGETS.find((entry) => entry.id === format);
  const where = target ? ` (${target.where})` : "";
  return `Kapsam çıktıya gömüldü${where}: ${coverageHeadline(coverage)}`;
}

/** Uyarı satırı var mı? (rozet rengi için) */
export function hasCoverageGaps(coverage: ExportCoverage): boolean {
  return !coverage.complete || coverage.warnings.length > 0;
}

/** Kapsamı olmayan (belgesiz) dosya için dışa aktarma uyarısı. */
export function emptyCoverageWarning(coverage: ExportCoverage): string | null {
  if (coverage.documents > 0) return null;
  return "Belgesiz dosya: çıktı yalnız dosya künyesini taşır, hiçbir belge incelenmiş sayılmaz.";
}
