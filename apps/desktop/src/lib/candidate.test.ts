import { describe, expect, it } from "vitest";
import type { CandidateReport, QuoteMatchReport } from "./api";
import { tr } from "./i18n";
import {
  CANDIDATE_KINDS,
  candidateKindLabel,
  candidateStateLabel,
  candidateStateTone,
  draftJsonError,
  errorCount,
  parseIssues,
  promoteBlockedReason,
  quoteLineStateLabel,
  quoteLineTone,
  quotePositionLabel,
  quoteStateLabel,
  quoteStateTone,
  quoteSummary,
  warningCount,
} from "./candidate";

function quote(overrides: Partial<QuoteMatchReport>): QuoteMatchReport {
  return {
    index: 0,
    documentVersionId: "v1",
    quote: "ödeme yapılmadığını belirtmiştir",
    label: null,
    state: "exact",
    reason: null,
    claimedPageIndex: 0,
    pageIndex: 0,
    charStart: 30,
    occurrences: 1,
    candidates: [],
    textOrigin: "native",
    parseRunId: "r1",
    segmentId: "s1",
    ...overrides,
  };
}

function report(overrides: Partial<CandidateReport>): CandidateReport {
  return {
    kind: "claim",
    state: "valid",
    schemaState: "valid",
    quoteState: "verified",
    issues: [],
    quotes: [quote({})],
    notes: [],
    checkedAt: "2026-10-24T20:00:00Z",
    ...overrides,
  };
}

describe("aday türü ve durum metinleri", () => {
  it("altı aday türünün hepsini listeler", () => {
    expect(CANDIDATE_KINDS.map((kind) => kind.value)).toEqual([
      "event_candidate",
      "person_candidate",
      "date_candidate",
      "claim_candidate",
      "summary_candidate",
      "answer_candidate",
    ]);
  });

  it("bilinmeyen türü olduğu gibi yazar", () => {
    expect(candidateKindLabel("claim_candidate")).toBe("İddia");
    expect(candidateKindLabel("gizemli")).toBe("gizemli");
  });

  it("geçerli/kısmi/geçersiz durumlarını ayırır", () => {
    expect(candidateStateLabel("valid")).toBe("geçerli");
    expect(candidateStateLabel("partial")).toBe("kısmi");
    expect(candidateStateLabel("invalid")).toBe("geçersiz");
    expect(candidateStateTone("valid")).toBe("ok");
    expect(candidateStateTone("partial")).toBe("warn");
    expect(candidateStateTone("invalid")).toBe("danger");
    expect(candidateStateTone("legacy")).toBe("plain");
  });

  it("eski kaydı doğrulanmış gibi göstermez", () => {
    expect(candidateStateLabel("legacy")).toContain("doğrulanmamış");
    expect(quoteStateLabel("legacy")).toContain("denetlenmemiş");
    expect(quoteStateTone("legacy")).toBe("plain");
  });

  it("alıntı durumu etiketleri", () => {
    expect(quoteStateLabel("verified")).toContain("birebir");
    expect(quoteStateTone("verified")).toBe("ok");
    expect(quoteStateTone("partial")).toBe("warn");
    expect(quoteStateTone("failed")).toBe("danger");
  });

  it("alıntı satırı durumunu nedeniyle birlikte yazar", () => {
    expect(quoteLineStateLabel(quote({}))).toBe("birebir bulundu");
    expect(
      quoteLineStateLabel(quote({ state: "unverifiable", reason: "version_not_found" })),
    ).toBe("belge sürümü bulunamadı");
    expect(quoteLineStateLabel(quote({ state: "unverifiable", reason: "no_text" }))).toBe(
      "belgenin metni yok",
    );
    expect(quoteLineStateLabel(quote({ state: "page_mismatch" }))).toBe("başka sayfada bulundu");
    expect(quoteLineStateLabel(quote({ state: "not_found" }))).toBe("metinde yok");
    expect(quoteLineTone("not_found")).toBe("danger");
    expect(quoteLineTone("ambiguous")).toBe("warn");
  });
});

describe("alıntı konumu ve özet", () => {
  it("0 tabanlı dizini 1 tabanlı sayfa olarak yazar", () => {
    expect(quotePositionLabel(quote({ pageIndex: 0, charStart: 12 }))).toBe(
      "sayfa 1 · karakter 12",
    );
    expect(quotePositionLabel(quote({ pageIndex: 4, charStart: null }))).toBe("sayfa 5");
    expect(quotePositionLabel(quote({ pageIndex: null, charStart: null }))).toBe("konum yok");
  });

  it("birebir bulunan alıntı sayısını verir", () => {
    expect(quoteSummary(report({}))).toBe("1/1 alıntı birebir");
    expect(
      quoteSummary(
        report({
          quotes: [
            quote({ index: 0 }),
            quote({ index: 1, state: "not_found", pageIndex: null, charStart: null }),
          ],
        }),
      ),
    ).toBe("1/2 alıntı birebir");
    expect(quoteSummary(report({ quotes: [] }))).toBe("alıntı yok");
  });
});

describe("doğrulama sorunları", () => {
  it("bozuk JSON'ı boş listeye çevirir", () => {
    expect(parseIssues("")).toEqual([]);
    expect(parseIssues("bozuk")).toEqual([]);
    expect(parseIssues("{}")).toEqual([]);
  });

  it("geçerli sorunları okur ve sayar", () => {
    const issues = parseIssues(
      JSON.stringify([
        { code: "missing_field", severity: "error", field: "title", detail: "yok" },
        { code: "unknown_field", severity: "warning", field: "ekstra", detail: "yok sayıldı" },
      ]),
    );
    expect(issues).toHaveLength(2);
    expect(errorCount(issues)).toBe(1);
    expect(warningCount(issues)).toBe(1);
  });

  it("eksik alanlı nesneleri eler", () => {
    const issues = parseIssues(JSON.stringify([{ code: "x" }, null, 3]));
    expect(issues).toEqual([]);
  });
});

describe("taslak ve terfi kapısı", () => {
  it("taslak JSON biçimini denetler", () => {
    expect(draftJsonError("")).toBe("Aday yükü boş.");
    expect(draftJsonError("[1,2]")).toBe("Aday yükü bir JSON nesnesi olmalı.");
    expect(draftJsonError("{")).toContain("JSON çözülemedi");
    expect(draftJsonError('{"schemaVersion":1}')).toBeNull();
  });

  it("uydurma alıntı taşıyan adayı kuyruğa göndertmez", () => {
    expect(
      promoteBlockedReason({ validationState: "invalid", quoteState: "failed", schemaState: "valid" }),
    ).toContain("bulunamadı");
    expect(
      promoteBlockedReason({ validationState: "invalid", quoteState: "not_checked", schemaState: "invalid" }),
    ).toContain("şeması geçersiz");
    expect(
      promoteBlockedReason({ validationState: "valid", quoteState: "verified", schemaState: "valid" }),
    ).toBeNull();
    expect(
      promoteBlockedReason({ validationState: "partial", quoteState: "no_quotes", schemaState: "valid" }),
    ).toBeNull();
    expect(
      promoteBlockedReason({ validationState: "valid", quoteState: "legacy", schemaState: "legacy" }),
    ).toContain("yeniden doğrulayın");
  });
});

describe("sözlük bağı (F04-T08)", () => {
  it("etiket, ipucu ve kapı metinleri doğrudan sözlükten gelir", () => {
    expect(candidateKindLabel("event_candidate")).toBe(tr.candidate.kind.event);
    expect(candidateKindLabel("answer_candidate")).toBe(tr.candidate.kind.answer);
    expect(candidateStateLabel("legacy")).toBe(tr.candidate.state.legacy);
    expect(quoteStateLabel("verified")).toBe(tr.candidate.quoteState.verified);
    expect(quoteLineStateLabel(quote({ state: "not_found" }))).toBe(tr.candidate.line.notFound);
    expect(
      quoteLineStateLabel(quote({ state: "unverifiable", reason: "case_mismatch" })),
    ).toBe(tr.candidate.unverifiableReason.caseMismatch);
    expect(
      promoteBlockedReason({
        validationState: "invalid",
        quoteState: "not_checked",
        schemaState: "valid",
      }),
    ).toBe(tr.candidate.blocked.invalidSchema);
    expect(draftJsonError("")).toBe(tr.candidate.draft.empty);
    expect(draftJsonError("[")?.startsWith(tr.candidate.draft.unparsable.split("{")[0]!)).toBe(true);
  });

  it("modül düzeyi sabitler de sözlükten kurulur (metin lib'e geri gömülemez)", () => {
    const claim = CANDIDATE_KINDS.find((option) => option.value === "claim_candidate")!;
    expect(claim.label).toBe(tr.candidate.kind.claim);
    expect(claim.hint).toBe(tr.candidate.kindHint.claim);
    expect(claim.template).toContain(tr.candidate.sample.claimText);
    const person = CANDIDATE_KINDS.find((option) => option.value === "person_candidate")!;
    expect(person.template).toContain(tr.candidate.sample.personName);
  });

  it("parametreli metinlerde yer tutucu kalmaz", () => {
    const position = quotePositionLabel(quote({ pageIndex: 2, charStart: 45 }));
    expect(position).toContain("3");
    expect(position).toContain("45");
    expect(position).not.toContain("{");
    expect(quotePositionLabel(quote({ pageIndex: 4, charStart: null }))).not.toContain("{");
    expect(quoteSummary(report({}))).not.toContain("{");
    expect(quoteSummary(report({ quotes: [] }))).toBe(tr.candidate.summary.none);
  });
});
