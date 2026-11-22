/**
 * F11-T12 — model/prompt sürümü bazında kalite ölçümünün **saf** aritmetiği.
 *
 * Çekirdek yalnız sayımları döndürür (oran hesaplamaz); payda burada kurulur.
 * Kurallar:
 * - **Kaynak doğruluğu** = (birebir + normalize) / ölçülen alıntı. Ölçülen
 *   alıntı = birebir + normalize + belirsiz + başka sayfa + bulunamadı.
 *   `doğrulanamayan` (sürüm silinmiş, maske çözülememiş) **paydaya girmez**:
 *   modelin uydurduğunun kanıtı değildir. Payda 0 ise oran `null`.
 * - **Öneri isabeti** = son kararı verilmiş (yerine geçmemiş) öğeler üzerinden.
 *   `ek kanıt` bir hüküm değildir (öğe yeniden karara açılır), orana girmez;
 *   ayrı sayılır. Karar bekleyen öğe de paydaya girmez.
 * - **Düzeltme emeği** = düzenleme / hüküm — süre değil, vekil gösterge.
 */

import type { AiQualityGroup } from "./api";

export type QuoteSummary = {
  /** Ölçülen alıntı: doğrulanabilir hükümlerin toplamı. */
  checked: number;
  grounded: number;
  weak: number;
  notFound: number;
  /** Paydaya girmez; ayrı raporlanır. */
  unverifiable: number;
  /** Raporu okunamayan / eski kayıt: hüküm bilinmiyor. */
  unmeasuredCandidates: number;
  noQuotes: number;
};

export function quoteSummary(group: AiQualityGroup): QuoteSummary {
  return {
    checked: group.quotesChecked,
    grounded: group.quotesExact + group.quotesNormalized,
    weak: group.quotesAmbiguous + group.quotesPageMismatch,
    notFound: group.quotesNotFound,
    unverifiable: group.quotesUnverifiable,
    unmeasuredCandidates: group.candidatesUnmeasured,
    noQuotes: group.candidatesNoQuotes,
  };
}

/** Kaynak doğruluğu oranı (0–1); ölçülen alıntı yoksa `null` (≈ %0 değil). */
export function quoteAccuracy(group: AiQualityGroup): number | null {
  const { checked, grounded } = quoteSummary(group);
  return checked > 0 ? grounded / checked : null;
}

export type ReviewSummary = {
  promoted: number;
  /** Hüküm verilmiş öğeler: kabul + düzeltme + ret. */
  verdicts: number;
  accept: number;
  edit: number;
  reject: number;
  /** Hüküm değil: öğe yeniden karara açılır. */
  needsEvidence: number;
  pending: number;
  superseded: number;
};

export function reviewSummary(group: AiQualityGroup): ReviewSummary {
  const decidedElsewhere = group.decidedOther;
  const pending =
    group.itemsPromoted -
    group.itemsSuperseded -
    (group.decidedAccept + group.decidedEdit + group.decidedReject) -
    group.decidedRequestEvidence -
    decidedElsewhere;
  return {
    promoted: group.itemsPromoted,
    verdicts: group.decidedAccept + group.decidedEdit + group.decidedReject,
    accept: group.decidedAccept,
    edit: group.decidedEdit,
    reject: group.decidedReject,
    needsEvidence: group.decidedRequestEvidence,
    pending: Math.max(0, pending),
    superseded: group.itemsSuperseded,
  };
}

/** Doğrudan kabul oranı; hüküm yoksa `null`. */
export function directAcceptShare(group: AiQualityGroup): number | null {
  const { verdicts, accept } = reviewSummary(group);
  return verdicts > 0 ? accept / verdicts : null;
}

/** Düzeltmeyle kullanılabilir öneri oranı: (kabul + düzenleme) / hüküm. */
export function usableShare(group: AiQualityGroup): number | null {
  const { verdicts, accept, edit } = reviewSummary(group);
  return verdicts > 0 ? (accept + edit) / verdicts : null;
}

/** Düzeltme emeği: düzenleme / hüküm; hüküm yoksa `null`. */
export function editShare(group: AiQualityGroup): number | null {
  const { verdicts, edit } = reviewSummary(group);
  return verdicts > 0 ? edit / verdicts : null;
}

/** Aday doğrulama dağılımı: payda `candidates`; bilinmeyen ayrı sayılır. */
export function candidateSummary(group: AiQualityGroup): {
  total: number;
  valid: number;
  partial: number;
  invalid: number;
  other: number;
} {
  return {
    total: group.candidates,
    valid: group.candidatesValid,
    partial: group.candidatesPartial,
    invalid: group.candidatesInvalid,
    other: group.candidatesOther,
  };
}
