// F11-T12 — kalite ölçümünün saf aritmetiği.
//
// Ölçülen kararlar:
// 1. Kaynak doğruluğu **yalnız ölçülen** alıntıları sayar; `doğrulanamayan`
//    paydaya girmez (modelin uydurduğunun kanıtı değildir) — payda 0 ise oran
//    `null`, «%0» değil.
// 2. Öneri isabeti **son karar** üzerinden: `ek kanıt` hüküm değildir, karar
//    bekleyen ve yerine geçmiş öğeler paydaya girmez.
// 3. Düzeltme emeği = düzenleme / hüküm; süre ölçülmez (vekil gösterge).

import { describe, expect, it } from "vitest";
import type { AiQualityGroup } from "./api";
import {
  candidateSummary,
  directAcceptShare,
  editShare,
  quoteAccuracy,
  quoteSummary,
  reviewSummary,
  usableShare,
} from "./ai-quality";

function group(overrides: Partial<AiQualityGroup> = {}): AiQualityGroup {
  return {
    modelSnapshot: "m1",
    promptVersion: "p1",
    runs: 0,
    runsSucceeded: 0,
    runsFailed: 0,
    runsUnfinished: 0,
    candidates: 0,
    candidatesValid: 0,
    candidatesPartial: 0,
    candidatesInvalid: 0,
    candidatesOther: 0,
    candidatesNoQuotes: 0,
    candidatesUnmeasured: 0,
    quotesChecked: 0,
    quotesExact: 0,
    quotesNormalized: 0,
    quotesAmbiguous: 0,
    quotesPageMismatch: 0,
    quotesNotFound: 0,
    quotesUnverifiable: 0,
    quotesOther: 0,
    itemsPromoted: 0,
    itemsSuperseded: 0,
    decidedAccept: 0,
    decidedEdit: 0,
    decidedReject: 0,
    decidedRequestEvidence: 0,
    decidedOther: 0,
    ...overrides,
  };
}

describe("quoteSummary / quoteAccuracy", () => {
  it("unverifiable alıntı doğrulama paydasına girmez", () => {
    const g = group({
      quotesChecked: 4,
      quotesExact: 2,
      quotesNormalized: 1,
      quotesNotFound: 1,
      quotesUnverifiable: 3,
      quotesOther: 1,
    });
    const summary = quoteSummary(g);
    expect(summary.checked).toBe(4);
    expect(summary.grounded).toBe(3);
    expect(summary.notFound).toBe(1);
    expect(summary.unverifiable).toBe(3);
    expect(quoteAccuracy(g)).toBeCloseTo(0.75, 10);
  });

  it("ölçülen alıntı yoksa oran null — «%0» uydurulmaz", () => {
    expect(quoteAccuracy(group({ quotesUnverifiable: 2 }))).toBeNull();
    expect(quoteAccuracy(group({}))).toBeNull();
  });

  it("zayıf hükümler (belirsiz + başka sayfa) doğrulanmış sayılmaz", () => {
    const g = group({
      quotesChecked: 4,
      quotesAmbiguous: 1,
      quotesPageMismatch: 1,
      quotesExact: 2,
    });
    const summary = quoteSummary(g);
    expect(summary.weak).toBe(2);
    expect(quoteAccuracy(g)).toBeCloseTo(0.5, 10);
  });

  it("ölçülemeyen ve alıntısız adaylar ayrı sayılır", () => {
    const summary = quoteSummary(
      group({ candidatesNoQuotes: 2, candidatesUnmeasured: 3 }),
    );
    expect(summary.noQuotes).toBe(2);
    expect(summary.unmeasuredCandidates).toBe(3);
  });
});

describe("reviewSummary ve oranlar", () => {
  it("ek kanıt isteği hüküm sayılmaz; karar bekleyen paydaya girmez", () => {
    const g = group({
      itemsPromoted: 6,
      itemsSuperseded: 1,
      decidedAccept: 2,
      decidedEdit: 1,
      decidedReject: 1,
      decidedRequestEvidence: 1,
    });
    const summary = reviewSummary(g);
    expect(summary.verdicts).toBe(4);
    expect(summary.needsEvidence).toBe(1);
    expect(summary.pending).toBe(0);
    expect(summary.superseded).toBe(1);
    expect(directAcceptShare(g)).toBeCloseTo(0.5, 10);
    expect(usableShare(g)).toBeCloseTo(0.75, 10);
    expect(editShare(g)).toBeCloseTo(0.25, 10);
  });

  it("hüküm yoksa oranlar null — sıfır sanılmaz", () => {
    const g = group({ itemsPromoted: 2, decidedRequestEvidence: 1, itemsSuperseded: 1 });
    expect(reviewSummary(g).verdicts).toBe(0);
    expect(directAcceptShare(g)).toBeNull();
    expect(usableShare(g)).toBeNull();
    expect(editShare(g)).toBeNull();
  });

  it("karar bekleyen öğe sayısı kalanları toplar", () => {
    const summary = reviewSummary(
      group({ itemsPromoted: 5, itemsSuperseded: 1, decidedAccept: 2 }),
    );
    expect(summary.pending).toBe(2);
  });
});

describe("candidateSummary", () => {
  it("bilinmeyen doğrulama durumu geçerli sayılmaz", () => {
    const summary = candidateSummary(
      group({
        candidates: 4,
        candidatesValid: 2,
        candidatesPartial: 0,
        candidatesInvalid: 1,
        candidatesOther: 1,
      }),
    );
    expect(summary.total).toBe(4);
    expect(summary.valid).toBe(2);
    expect(summary.other).toBe(1);
  });
});
