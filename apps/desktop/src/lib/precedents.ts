import type { PrecedentVersionRow, QuoteVerificationRow } from "./api";
import { formatNumber } from "./format";

/** Tam metin sürümünün kısa durumu. Metin yoksa "yok" denir; tahmin yok. */
export function versionStateText(version: {
  hasText: boolean;
  charCount: number;
}): string {
  if (!version.hasText) return "tam metin yok";
  return `tam metin · ${formatNumber(version.charCount)} karakter`;
}

/** Alıntı doğrulamasının tek cümlelik, dürüst özeti. "Bulunamadı" asla
 *  "uydurulmuş" diye çevrilmez; kısa alıntı doğrulanmış sayılmaz. */
export function quoteStateText(verification: QuoteVerificationRow): string {
  const at =
    verification.matchedStart === null
      ? ""
      : ` (konum ${formatNumber(verification.matchedStart + 1)})`;
  const repeated =
    verification.occurrences > 1 ? ` · metinde ${verification.occurrences} kez geçiyor` : "";
  // Eski sürümde bulunan alıntı gizlenmez: güncel metinde geçmiyor.
  const oldNote = verification.foundInOldVersion
    ? " Güncel tam metinde geçmiyor; karar metni güncellenmiş olabilir."
    : "";
  const where = verification.foundInOldVersion ? "eski bir sürümde" : "metinde";
  switch (verification.state) {
    case "match": {
      if (verification.method === "exact")
        return `Alıntı ${where} birebir bulundu${at}${repeated}.${oldNote}`;
      if (verification.method === "whitespace")
        return `Alıntı yalnız satır/boşluk farkıyla ${where} bulundu${at}${repeated}. Birebir değil.${oldNote}`;
      if (verification.method === "dehyphenated")
        return `Alıntı yalnız satır sonunda tire ile bölünmüş kelime birleştirilerek ${where} bulundu${at}${repeated}. Birebir değil, gözle doğrulayın.${oldNote}`;
      if (verification.method === "ligature")
        return `Alıntı yalnız bitişik yazı (ligatür) açılarak ${where} bulundu${at}${repeated}. PDF fontu “fl”/“fi” çiftini tek glif yazmış; birebir değil, gözle doğrulayın.${oldNote}`;
      return `Alıntı Türkçe harf farkıyla ${where} bulundu${at}${repeated}. Birebir değil, gözle doğrulayın.${oldNote}`;
    }
    case "mismatch": {
      const scope =
        verification.historicalVersionsScanned > 0
          ? `güncel metinde ve taranan ${verification.historicalVersionsScanned} eski sürümde`
          : "bu tam metinde";
      const cut = verification.historicalTruncated
        ? " Daha eski sürümler taranmadı; sonuç kesin değildir."
        : "";
      return `Alıntı ${scope} bulunamadı. Bu tek başına “uydurulmuş” demek değildir; başka sürüm veya çeviri olabilir.${cut}`;
    }
    default:
      if (verification.reason === "quote_too_short")
        return "Alıntı çok kısa (en az 8 karakter gerekir); doğrulanmadı.";
      return "Kararın tam metni yok; alıntı doğrulanamadı. Bu “uydurulmuş” demek değildir.";
  }
}

/** Kaç sürümün tarandığını söyleyen tek satır; sayı çekirdekten gelir. */
export function quoteScanNote(verification: QuoteVerificationRow): string | null {
  if (verification.state === "not_checked") return null;
  const parts = [`${verification.versionsScanned} sürüm tarandı`];
  if (verification.historicalVersionsScanned > 0)
    parts.push(`${verification.historicalVersionsScanned} eski sürüm`);
  if (verification.historicalTruncated) parts.push("daha eski sürümler taranmadı");
  return parts.join(" · ");
}

export function quoteStateTone(
  verification: QuoteVerificationRow,
): "ok" | "warn" | "muted" {
  if (verification.state === "match") {
    // Eski sürümdeki birebir eşleşme bugünkü metni doğrulamaz → uyarı tonu.
    if (verification.foundInOldVersion) return "warn";
    return verification.method === "exact" ? "ok" : "warn";
  }
  if (verification.state === "mismatch") return "warn";
  return "muted";
}

/** Doğrulama sonucunu atıf kontrolü boyutuna taşır; boyutlar ayrı kalır. */
export function quoteToCitationMatch(
  verification: QuoteVerificationRow,
): "match" | "mismatch" | "not_checked" {
  if (verification.state === "match") return "match";
  if (verification.state === "mismatch") return "mismatch";
  return "not_checked";
}

/** Sürüm rozeti: metin yoksa "metin yok", varsa kaynak türü. */
export function versionSourceLabel(version: { sourceKind: string }): string {
  switch (version.sourceKind) {
    case "paste":
      return "yapıştırıldı";
    case "document":
      return "kasadaki belgeden";
    case "file":
      return "dosyadan";
    default:
      return version.sourceKind;
  }
}

export type PrecedentVersionSummary = {
  versions: PrecedentVersionRow[];
  withText: number;
  latestHasText: boolean;
};

/** Liste özeti: kaç sürümde tam metin var, en yenisi metinli mi. */
export function summarizeVersions(versions: PrecedentVersionRow[]): PrecedentVersionSummary {
  const withText = versions.filter((v) => v.hasText).length;
  const latest = versions[0];
  return { versions, withText, latestHasText: latest ? latest.hasText : false };
}

/** Toplu içe alma sonucunun dosya başına dürüst etiketi (F12-T06). */
export function bulkOutcomeLabel(item: {
  outcome: string;
  code: string | null;
  message: string | null;
  charCount: number;
}): string {
  switch (item.outcome) {
    case "created":
      return `eklendi · ${formatNumber(item.charCount)} karakter`;
    case "duplicate":
      return "aynı nüsha zaten var; yeniden yazılmadı";
    case "conflict":
      return "aynı künye, farklı metin — inceleyin";
    default: {
      const code = item.code ? `${item.code}: ` : "";
      return `${code}${item.message ?? "içe alınamadı"}`;
    }
  }
}

export function bulkOutcomeTone(item: { outcome: string }): "ok" | "warn" | "danger" {
  if (item.outcome === "created") return "ok";
  if (item.outcome === "duplicate") return "ok";
  if (item.outcome === "conflict") return "warn";
  return "danger";
}

/** Tek cümlelik toplu içe alma özeti; sayılar raporun kendisinden gelir. */
export function bulkReportSummary(report: {
  created: number;
  duplicates: number;
  conflicts: number;
  failed: number;
}): string {
  const parts = [
    `${report.created} eklendi`,
    `${report.duplicates} zaten vardı`,
    `${report.conflicts} çakışma`,
  ];
  if (report.failed > 0) parts.push(`${report.failed} hata`);
  return parts.join(" · ");
}

/** Sürüm rozeti: künye çakışması varsa metin durumundan önce gelir. */
export function versionConflictText(version: {
  identityConflict: boolean;
}): string | null {
  return version.identityConflict
    ? "aynı künye, farklı metin — iki nüshayı karşılaştırın"
    : null;
}

/** Eksik künye alanları tek satırda; eksik yoksa "künye tam". */
export function bibliographyMissingText(missing: string[]): string {
  return missing.length === 0 ? "künye tam" : `eksik: ${missing.join(", ")}`;
}

/**
 * Erişim tarihinin nereden geldiğini söyler. Tam metin varsa edinim zamanı,
 * yoksa künye kaydının oluşturulma zamanı kullanılır — bu ayrım gizlenmez.
 */
export function accessBasisLabel(basis: string): string {
  return basis === "version"
    ? "erişim tarihi: en yeni tam metnin edinimi"
    : "erişim tarihi: künye kaydının oluşturulması (tam metin edinimi yok)";
}
