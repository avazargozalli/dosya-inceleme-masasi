import { useCallback, useEffect, useState } from "react";
import {
  api,
  toApiError,
  type AnchorRow,
  type ApiError,
  type CaseRow,
  type ClaimRelationRow,
  type ClaimRow,
  type ContradictionAssessmentRow,
  type DocumentRow,
  type EvidenceLinkRow,
  type InTextQuoteCheck,
  type InTextQuoteFinding,
  type ReviewItemRow,
} from "../lib/api";
import {
  bindHint,
  bindingInput,
  canBind,
  candidateLabel,
  oldVersionNote,
  positionLabel,
  quoteCheckSummary,
  QUOTE_STATE_LABEL,
  quoteStateTone,
  scopeLabel,
  skippedLabel,
  versionsLabel,
} from "../lib/intext";
import { formatTimestamp } from "../lib/format";
import { t } from "../lib/i18n";
import {
  buildReviewTarget,
  itemText,
  TARGET_KIND_LABEL,
  type ReviewTargetKind,
} from "../lib/reviewTarget";

const CLAIM_KIND: Record<string, string> = {
  allegation: t("review.claimKindAllegation"),
  defense: t("review.claimKindDefense"),
  admission: t("review.claimKindAdmission"),
  court_finding: t("review.claimKindCourtFinding"),
  unknown: t("review.claimKindUnknown"),
};

const RELATION: Record<string, string> = {
  submitted_for: t("review.relationSubmittedFor"),
  supports_candidate: t("review.relationSupportsCandidate"),
  contradicts_candidate: t("review.relationContradictsCandidate"),
  context: t("review.relationContext"),
};

const CLAIM_RELATION: Record<string, string> = {
  response_to: t("review.claimRelationResponseTo"),
  supports: t("review.claimRelationSupports"),
  contradicts: t("review.claimRelationContradicts"),
  context: t("review.claimRelationContext"),
};

const ITEM_KIND: Record<string, string> = {
  claim_candidate: t("review.itemClaim"),
  event_candidate: t("review.itemEvent"),
  note_candidate: t("review.itemNote"),
  task_candidate: t("review.itemTask"),
  contradiction_candidate: t("review.itemContradiction"),
};

const VERDICT: Record<string, string> = {
  conflict: t("review.verdictConflict"),
  explained: t("review.verdictExplained"),
  not_conflict: t("review.verdictNotConflict"),
  needs_more_evidence: t("review.verdictNeedsEvidence"),
};

const ITEM_STATE: Record<string, string> = {
  pending: t("review.statePending"),
  accepted: t("review.stateAccepted"),
  rejected: t("review.stateRejected"),
  edited: t("review.stateEdited"),
  needs_more_evidence: t("review.stateNeedsEvidence"),
  superseded: t("review.stateSuperseded"),
};

export function Review() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [claims, setClaims] = useState<ClaimRow[]>([]);
  const [selectedClaim, setSelectedClaim] = useState<ClaimRow | null>(null);
  const [links, setLinks] = useState<EvidenceLinkRow[]>([]);
  const [items, setItems] = useState<ReviewItemRow[]>([]);
  const [anchors, setAnchors] = useState<AnchorRow[]>([]);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [documents, setDocuments] = useState<DocumentRow[]>([]);
  const [quoteText, setQuoteText] = useState("");
  const [quoteScope, setQuoteScope] = useState("");
  const [quoteSource, setQuoteSource] = useState("");
  const [quoteCheck, setQuoteCheck] = useState<InTextQuoteCheck | null>(null);
  const [quoteBindings, setQuoteBindings] = useState<Record<number, string>>({});
  const [quoteBusy, setQuoteBusy] = useState(false);

  const [claimText, setClaimText] = useState("");
  const [claimKind, setClaimKind] = useState("");
  const [linkAnchor, setLinkAnchor] = useState("");
  const [linkRelation, setLinkRelation] = useState("");
  const [linkRationale, setLinkRationale] = useState("");
  const [relations, setRelations] = useState<ClaimRelationRow[]>([]);
  const [relationTarget, setRelationTarget] = useState("");
  const [relationKind, setRelationKind] = useState("");
  const [relationNote, setRelationNote] = useState("");
  const [draftKind, setDraftKind] = useState("note_candidate");
  const [draftAnchor, setDraftAnchor] = useState("");
  const [draftAnchor2, setDraftAnchor2] = useState("");
  const [draftText, setDraftText] = useState("");
  const [assessments, setAssessments] = useState<Record<string, ContradictionAssessmentRow[]>>({});
  const [assessmentVerdicts, setAssessmentVerdicts] = useState<Record<string, string>>({});
  const [assessmentNotes, setAssessmentNotes] = useState<Record<string, string>>({});
  const [editJson, setEditJson] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [supersedeTargets, setSupersedeTargets] = useState<Record<string, string>>({});
  // F08-T08: kabul, seçilen hedef kayıtla birlikte atomik yazılır.
  const [targetKinds, setTargetKinds] = useState<Record<string, ReviewTargetKind>>({});
  const [targetClaimKinds, setTargetClaimKinds] = useState<Record<string, string>>({});
  const [outboxPending, setOutboxPending] = useState(0);

  useEffect(() => {
    (async () => {
      try {
        const rows = await api.caseList(false);
        setCases(rows);
        if (rows.length > 0) setCaseId((prev) => prev || rows[0]!.id);
      } catch (e) {
        setError(toApiError(e));
      }
    })();
  }, []);

  const load = useCallback(async (id: string) => {
    if (!id) return;
    try {
      const [claimRows, itemRows, anchorRows, documentRows] = await Promise.all([
        api.claimList(id),
        api.reviewItemList(id),
        api.anchorList(id),
        api.documentList(id),
      ]);
      setClaims(claimRows);
      setItems(itemRows);
      setAnchors(anchorRows);
      setDocuments(documentRows);
      const contradictionIds = itemRows
        .filter((i) => i.itemKind === "contradiction_candidate")
        .map((i) => i.id);
      const [outboxRows, pairs] = await Promise.all([
        api.reviewOutboxPending(id),
        Promise.all(
          contradictionIds.map(
            async (itemId) => [itemId, await api.contradictionAssessments(itemId)] as const,
          ),
        ),
      ]);
      setOutboxPending(outboxRows.length);
      setAssessments(Object.fromEntries(pairs));
      setSelectedClaim((prev) => (prev ? claimRows.find((c) => c.id === prev.id) ?? null : null));
    } catch (e) {
      setError(toApiError(e));
    }
  }, []);

  useEffect(() => {
    void load(caseId);
  }, [caseId, load]);

  // Dosya değişince önceki dosyanın alıntı **hükmü** ve **belge kapsamı**
  // temizlenir: bayat sonuç ekranda kalmaz ve kapsam yeni dosyaya taşınırsa
  // denetim başka dosyanın belgesine daraltılmış gitmez. Yazılan alıntı
  // metni kullanıcının girdisidir; silinmez, yeni dosyada açıkça çalıştırılır.
  useEffect(() => {
    setQuoteCheck(null);
    setQuoteBindings({});
    setQuoteSource("");
    setQuoteScope("");
  }, [caseId]);

  useEffect(() => {
    if (!selectedClaim) {
      setLinks([]);
      setRelations([]);
      return;
    }
    void Promise.all([
      api.evidenceLinkList(selectedClaim.id),
      api.claimRelationList(selectedClaim.id),
    ])
      .then(([linkRows, relationRows]) => {
        setLinks(linkRows);
        setRelations(relationRows);
      })
      .catch((e) => setError(toApiError(e)));
  }, [selectedClaim]);

  async function runQuoteCheck() {
    if (!caseId || quoteText.trim().length === 0) return;
    setQuoteBusy(true);
    try {
      const report = await api.inTextQuoteCheck(caseId, {
        text: quoteText,
        documentId: quoteScope === "" ? null : quoteScope,
      });
      setQuoteCheck(report);
      setQuoteBindings({});
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setQuoteBusy(false);
    }
  }

  async function bindQuote(finding: InTextQuoteFinding) {
    const input = bindingInput(caseId, finding);
    if (!input) return;
    setQuoteBusy(true);
    try {
      const anchor = await api.anchorCreate(input);
      setQuoteBindings((prev) => ({ ...prev, [finding.index]: anchor.id }));
      setAnchors((prev) => [anchor, ...prev]);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setQuoteBusy(false);
    }
  }

  async function addClaim() {
    if (!caseId || claimText.trim().length === 0 || claimKind === "") return;
    setBusy(true);
    try {
      await api.claimCreate(caseId, {
        claimantActorId: null,
        claimText: claimText.trim(),
        claimKind,
        sourceAnchorId: null,
      });
      setClaimText("");
      setClaimKind("");
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function addLink() {
    if (!selectedClaim || !linkAnchor || linkRelation === "") return;
    setBusy(true);
    try {
      await api.evidenceLinkCreate(caseId, {
        claimId: selectedClaim.id,
        sourceAnchorId: linkAnchor,
        relation: linkRelation,
        rationale: linkRationale.trim(),
      });
      setLinkRationale("");
      setLinks(await api.evidenceLinkList(selectedClaim.id));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function addClaimRelation() {
    if (!selectedClaim || relationTarget === "" || relationKind === "") return;
    setBusy(true);
    try {
      await api.claimRelationCreate(caseId, selectedClaim.id, {
        relatedClaimId: relationTarget,
        relation: relationKind,
        rationale: relationNote.trim(),
      });
      setRelationTarget("");
      setRelationKind("");
      setRelationNote("");
      setRelations(await api.claimRelationList(selectedClaim.id));
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function addCandidate() {
    const anchorIds =
      draftKind === "contradiction_candidate"
        ? [draftAnchor, draftAnchor2].filter((id) => id !== "")
        : [draftAnchor];
    if (anchorIds.length === 0 || draftText.trim().length === 0) return;
    if (draftKind === "contradiction_candidate" && (anchorIds.length !== 2 || draftAnchor === draftAnchor2)) {
      setError({
        code: "E_FORM",
        message: t("review.errContradictionSources"),
        retryable: false,
      });
      return;
    }
    setBusy(true);
    try {
      await api.reviewItemCreate(caseId, {
        itemKind: draftKind,
        payload: { text: draftText.trim() },
        sourceAnchorIds: anchorIds,
        aiRunId: null,
      });
      setDraftText("");
      setDraftAnchor2("");
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function assess(item: ReviewItemRow) {
    const verdict = assessmentVerdicts[item.id];
    if (!verdict) return;
    setBusy(true);
    try {
      await api.contradictionAssess(
        caseId,
        item.id,
        verdict,
        (assessmentNotes[item.id] ?? "").trim(),
        "local-user",
      );
      const rows = await api.contradictionAssessments(item.id);
      setAssessments((prev) => ({ ...prev, [item.id]: rows }));
      setAssessmentVerdicts((prev) => ({ ...prev, [item.id]: "" }));
      setAssessmentNotes((prev) => ({ ...prev, [item.id]: "" }));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function decide(item: ReviewItemRow, decision: "accept" | "reject" | "request_evidence" | "edit") {
    setBusy(true);
    try {
      let edited: unknown = null;
      if (decision === "edit") {
        try {
          edited = JSON.parse(editJson);
        } catch {
          setError({ code: "E_JSON", message: t("review.errEditJson"), retryable: false });
          setBusy(false);
          return;
        }
      }
      await api.reviewDecide(item.id, decision, decision === "edit" ? t("review.auditUserEdit") : "", edited ?? undefined);
      setEditingId(null);
      setEditJson("");
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  /**
   * F08-T08: kabul, seçilen hedef kayıtla birlikte tek işlemde yazılır.
   * Kayıt türü (iddiada ayrıca iddia türü) seçilmeden buton açılmaz.
   */
  async function commit(item: ReviewItemRow) {
    const target = buildReviewTarget(
      targetKinds[item.id] ?? "",
      itemText(item.payload),
      targetClaimKinds[item.id] ?? "",
    );
    if (!target) {
      setError({
        code: "E_FORM",
        message: t("review.errTargetKind"),
        retryable: false,
      });
      return;
    }
    setBusy(true);
    try {
      await api.reviewCommit(item.id, "accept", target, t("review.auditUserApproval"));
      setTargetKinds((prev) => ({ ...prev, [item.id]: "" }));
      setTargetClaimKinds((prev) => ({ ...prev, [item.id]: "" }));
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  const pending = items.filter((i) => i.state === "pending" || i.state === "needs_more_evidence");
  const decided = items.filter((i) => !(i.state === "pending" || i.state === "needs_more_evidence"));
  // F08-T09: hedef kaydı üretilmiş öğe "yerine geç" listesine girmez; onaylı
  // kayıt sessizce değiştirilemez (backend de reddeder).
  const replaceable = decided.filter(
    (i) => (i.state === "accepted" || i.state === "edited") && !i.targetEntityId,
  );

  async function supersede(item: ReviewItemRow, oldItemId: string) {
    if (!oldItemId) return;
    setBusy(true);
    try {
      await api.reviewSupersede(oldItemId, item.id);
      setSupersedeTargets((prev) => ({ ...prev, [item.id]: "" }));
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page" style={{ maxWidth: 1020 }}>
      <div className="page-head">
        <h1>{t("nav.review")}</h1>
        <p className="lead">{t("review.lead")}</p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
          {error.code === "E_ROW_VERSION" ? (
            <button
              className="btn ghost"
              style={{ marginLeft: 8 }}
              onClick={() => {
                setError(null);
                void load(caseId);
              }}
            >
              {t("review.reloadCurrent")}
            </button>
          ) : null}
        </div>
      ) : null}

      <div className="card tight">
        <div className="row">
          <div className="field" style={{ flex: "1 1 260px" }}>
            <label htmlFor="rv-case">{t("common.case")}</label>
            <select id="rv-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.length === 0 ? <option value="">{t("common.noCase")}</option> : null}
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field" style={{ flex: "2 1 300px" }}>
            <label htmlFor="rv-claim">{t("review.newClaimLabel")}</label>
            <input
              id="rv-claim"
              className="input"
              value={claimText}
              onChange={(e) => setClaimText(e.target.value)}
              placeholder={t("review.claimPlaceholder")}
              onKeyDown={(e) => e.key === "Enter" && void addClaim()}
            />
          </div>
          <div className="field" style={{ flex: "0 1 170px" }}>
            <label htmlFor="rv-kind">{t("review.kindLabel")}</label>
            <select id="rv-kind" className="input" value={claimKind} onChange={(e) => setClaimKind(e.target.value)}>
              <option value="">{t("review.kindPlaceholder")}</option>
              {Object.entries(CLAIM_KIND).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
            </select>
          </div>
          <button className="btn primary" disabled={busy || claimKind === "" || claimText.trim().length === 0} onClick={() => void addClaim()}>{t("review.addAction")}</button>
        </div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-4)" }}>
        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("review.claimsTitle")}</h2>
              <p className="card-sub">{t("review.claimsCount", { count: claims.length })}</p>
            </div>
          </div>
          {claims.length === 0 ? (
            <div className="empty"><p style={{ margin: 0 }}>{t("review.claimsEmpty")}</p></div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              {claims.map((claim) => (
                <button
                  key={claim.id}
                  onClick={() => setSelectedClaim(claim)}
                  style={{
                    textAlign: "left",
                    padding: "8px 10px",
                    borderRadius: "var(--radius)",
                    border: "1px solid var(--line)",
                    background: selectedClaim?.id === claim.id ? "var(--surface)" : "var(--bg)",
                    cursor: "pointer",
                  }}
                >
                  <span style={{ display: "flex", justifyContent: "space-between", gap: 8 }}>
                    <strong style={{ fontWeight: 500 }}>{claim.claimText}</strong>
                    <span className="badge plain">{CLAIM_KIND[claim.claimKind] ?? claim.claimKind}</span>
                  </span>
                  <span className="muted small">{formatTimestamp(claim.createdAt)}</span>
                </button>
              ))}
            </div>
          )}

          {selectedClaim ? (
            <div className="card tight" style={{ background: "var(--surface)" }}>
              <h2>{t("review.evidenceTitle")}</h2>
              {links.length === 0 ? (
                <p className="hint" style={{ margin: 0 }}>{t("review.evidenceEmpty")}</p>
              ) : (
                <ul style={{ margin: 0, paddingLeft: 18 }}>
                  {links.map((link) => (
                    <li key={link.id} className="small">
                      {RELATION[link.relation] ?? link.relation} ·{" "}
                      {anchors.find((a) => a.id === link.sourceAnchorId)?.exactQuote.slice(0, 60) ?? t("review.quoteFallback")}
                    </li>
                  ))}
                </ul>
              )}
              <div className="field">
                <label htmlFor="rv-anchor">{t("review.anchorLabel")}</label>
                <select id="rv-anchor" className="input" value={linkAnchor} onChange={(e) => setLinkAnchor(e.target.value)}>
                  <option value="">{t("common.select")}</option>
                  {anchors.map((a) => (
                    <option key={a.id} value={a.id}>“{a.exactQuote.slice(0, 70)}”</option>
                  ))}
                </select>
              </div>
              <div className="row">
                <div className="field">
                  <label htmlFor="rv-rel">{t("review.relationLabel")}</label>
                  <select id="rv-rel" className="input" value={linkRelation} onChange={(e) => setLinkRelation(e.target.value)}>
                    <option value="">{t("review.relationPlaceholder")}</option>
                    {Object.entries(RELATION).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
                  </select>
                </div>
                <div className="field" style={{ flex: "2 1 200px" }}>
                  <label htmlFor="rv-rat">{t("review.rationaleLabel")}</label>
                  <input id="rv-rat" className="input" value={linkRationale} onChange={(e) => setLinkRationale(e.target.value)} />
                </div>
                <button className="btn" disabled={busy || !linkAnchor || linkRelation === ""} onClick={() => void addLink()}>{t("review.bindAction")}</button>
              </div>

              <h2 style={{ marginTop: 12 }}>{t("review.claimRelationsTitle")}</h2>
              {relations.length === 0 ? (
                <p className="hint" style={{ margin: 0 }}>{t("review.claimRelationsEmpty")}</p>
              ) : (
                <ul style={{ margin: 0, paddingLeft: 18 }}>
                  {relations.map((rel) => {
                    const outgoing = rel.claimId === selectedClaim.id;
                    const otherId = outgoing ? rel.relatedClaimId : rel.claimId;
                    const other = claims.find((c) => c.id === otherId);
                    return (
                      <li key={rel.id} className="small">
                        {outgoing ? "→" : "←"} {CLAIM_RELATION[rel.relation] ?? rel.relation} ·{" "}
                        {other?.claimText ?? t("review.claimFallback")}
                        {rel.rationale ? ` · ${rel.rationale}` : ""}
                      </li>
                    );
                  })}
                </ul>
              )}
              <div className="row">
                <div className="field" style={{ flex: "1 1 180px" }}>
                  <label htmlFor="rv-reltarget">{t("review.relatedClaimLabel")}</label>
                  <select
                    id="rv-reltarget"
                    className="input"
                    value={relationTarget}
                    onChange={(e) => setRelationTarget(e.target.value)}
                  >
                    <option value="">{t("review.claimPlaceholderSelect")}</option>
                    {claims
                      .filter((c) => c.id !== selectedClaim.id)
                      .map((c) => (
                        <option key={c.id} value={c.id}>{c.claimText}</option>
                      ))}
                  </select>
                </div>
                <div className="field" style={{ flex: "0 1 190px" }}>
                  <label htmlFor="rv-relkind">{t("review.relationKindLabel")}</label>
                  <select
                    id="rv-relkind"
                    className="input"
                    value={relationKind}
                    onChange={(e) => setRelationKind(e.target.value)}
                  >
                    <option value="">{t("review.kindPlaceholder")}</option>
                    {Object.entries(CLAIM_RELATION).map(([k, v]) => (
                      <option key={k} value={k}>{v}</option>
                    ))}
                  </select>
                </div>
                <div className="field" style={{ flex: "2 1 160px" }}>
                  <label htmlFor="rv-relnote">{t("review.rationaleLabel")}</label>
                  <input
                    id="rv-relnote"
                    className="input"
                    value={relationNote}
                    onChange={(e) => setRelationNote(e.target.value)}
                  />
                </div>
                <button
                  className="btn"
                  disabled={busy || relationTarget === "" || relationKind === ""}
                  onClick={() => void addClaimRelation()}
                >
                  {t("review.relateAction")}
                </button>
              </div>
              <p className="hint" style={{ margin: 0 }}>{t("review.relationHint")}</p>
            </div>
          ) : null}
        </div>

        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("review.queueTitle")}</h2>
              <p className="card-sub">
                {t("review.queueSummary", {
                  pending: pending.length,
                  decided: decided.length,
                  outbox: outboxPending,
                })}
              </p>
            </div>
          </div>

          <div className="card tight" style={{ background: "var(--surface)" }}>
            <h2>{t("review.draftTitle")}</h2>
            <div className="row">
              <div className="field" style={{ flex: "1 1 160px" }}>
                <label htmlFor="rv-dkind">{t("review.kindLabel")}</label>
                <select id="rv-dkind" className="input" value={draftKind} onChange={(e) => setDraftKind(e.target.value)}>
                  {Object.entries(ITEM_KIND).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
                </select>
              </div>
              <div className="field" style={{ flex: "2 1 180px" }}>
                <label htmlFor="rv-danchor">{t("review.sourceLabel")}</label>
                <select id="rv-danchor" className="input" value={draftAnchor} onChange={(e) => setDraftAnchor(e.target.value)}>
                  <option value="">{t("common.select")}</option>
                  {anchors.map((a) => (
                    <option key={a.id} value={a.id}>“{a.exactQuote.slice(0, 60)}”</option>
                  ))}
                </select>
              </div>
              {draftKind === "contradiction_candidate" ? (
                <div className="field" style={{ flex: "2 1 180px" }}>
                  <label htmlFor="rv-danchor2">{t("review.secondSourceLabel")}</label>
                  <select id="rv-danchor2" className="input" value={draftAnchor2} onChange={(e) => setDraftAnchor2(e.target.value)}>
                    <option value="">{t("common.select")}</option>
                    {anchors
                      .filter((a) => a.id !== draftAnchor)
                      .map((a) => (
                        <option key={a.id} value={a.id}>“{a.exactQuote.slice(0, 60)}”</option>
                      ))}
                  </select>
                </div>
              ) : null}
            </div>
            <div className="field">
              <label htmlFor="rv-dtext">{t("review.draftTextLabel")}</label>
              <input id="rv-dtext" className="input" value={draftText} onChange={(e) => setDraftText(e.target.value)} />
            </div>
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button className="btn" disabled={busy || !draftAnchor || draftText.trim().length === 0 || (draftKind === "contradiction_candidate" && (!draftAnchor2 || draftAnchor2 === draftAnchor))} onClick={() => void addCandidate()}>
                {t("review.queueAdd")}
              </button>
            </div>
            <p className="hint" style={{ margin: 0 }}>{t("review.queueHint")}</p>
          </div>

          {pending.length === 0 ? (
            <div className="empty"><p style={{ margin: 0 }}>{t("review.queueEmpty")}</p></div>
          ) : (
            pending.map((item) => (
              <div key={item.id} className="card tight" style={{ background: "var(--bg)" }}>
                <div className="row" style={{ justifyContent: "space-between" }}>
                  <span className="badge plain">{ITEM_KIND[item.itemKind] ?? item.itemKind}</span>
                  <span className="muted small">{formatTimestamp(item.createdAt)}</span>
                </div>
                <pre style={{ margin: 0, whiteSpace: "pre-wrap", fontFamily: "var(--mono)", fontSize: 12 }}>
                  {JSON.stringify(item.payload, null, 2)}
                </pre>
                <div className="muted small">
                  {t("review.sourceQuotes", { count: item.sourceAnchorIds.length })}
                </div>
                {item.itemKind === "contradiction_candidate" ? (
                  <div>
                    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
                      {item.sourceAnchorIds.slice(0, 2).map((anchorId, idx) => {
                        const anchor = anchors.find((a) => a.id === anchorId);
                        return (
                          <div key={anchorId} className="card tight" style={{ background: "var(--bg)", margin: 0 }}>
                            <span className="badge plain">{idx === 0 ? t("review.sourceOne") : t("review.sourceTwo")}</span>
                            <p className="small" style={{ margin: 0 }}>
                              “{anchor?.exactQuote ?? t("review.quoteMissing")}”
                            </p>
                            <span className="muted small">
                              {anchor ? `${anchor.startOffset}–${anchor.endOffset}` : ""}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                    {(() => {
                      const list = assessments[item.id] ?? [];
                      const latest = list[list.length - 1];
                      return latest ? (
                        <p className="small" style={{ margin: 0 }}>
                          {t("review.latestAssessment")} <strong>{VERDICT[latest.verdict] ?? latest.verdict}</strong>
                          {latest.rationale ? ` ${t("review.rationaleDash", { text: latest.rationale })}` : ""}
                        </p>
                      ) : (
                        <p className="hint" style={{ margin: 0 }}>
                          {t("review.noAssessment")}
                        </p>
                      );
                    })()}
                    <div className="row">
                      <div className="field" style={{ flex: "0 1 200px" }}>
                        <label htmlFor={`verdict-${item.id}`}>{t("review.verdictLabel")}</label>
                        <select
                          id={`verdict-${item.id}`}
                          className="input"
                          value={assessmentVerdicts[item.id] ?? ""}
                          onChange={(e) =>
                            setAssessmentVerdicts((prev) => ({ ...prev, [item.id]: e.target.value }))
                          }
                        >
                          <option value="">{t("common.select")}</option>
                          {Object.entries(VERDICT).map(([k, v]) => (
                            <option key={k} value={k}>{v}</option>
                          ))}
                        </select>
                      </div>
                      <div className="field" style={{ flex: "2 1 180px" }}>
                        <label htmlFor={`verdict-note-${item.id}`}>{t("review.rationaleLabel")}</label>
                        <input
                          id={`verdict-note-${item.id}`}
                          className="input"
                          value={assessmentNotes[item.id] ?? ""}
                          onChange={(e) =>
                            setAssessmentNotes((prev) => ({ ...prev, [item.id]: e.target.value }))
                          }
                        />
                      </div>
                      <button
                        className="btn"
                        disabled={busy || !assessmentVerdicts[item.id]}
                        onClick={() => void assess(item)}
                      >
                        {t("review.assessAction")}
                      </button>
                    </div>
                  </div>
                ) : null}
                {editingId === item.id ? (
                  <div className="field">
                    <label htmlFor={`edit-${item.id}`}>{t("review.editJsonLabel")}</label>
                    <textarea
                      id={`edit-${item.id}`}
                      className="input"
                      style={{ height: 80, padding: 8, fontFamily: "var(--mono)", fontSize: 12 }}
                      value={editJson}
                      onChange={(e) => setEditJson(e.target.value)}
                    />
                  </div>
                ) : null}
                {editingId === item.id ? null : (
                  <div className="row" style={{ alignItems: "flex-end" }}>
                    <div className="field" style={{ flex: "1 1 170px" }}>
                      <label htmlFor={`rk-${item.id}`}>{t("review.targetKindLabel")}</label>
                      <select
                        id={`rk-${item.id}`}
                        className="input"
                        value={targetKinds[item.id] ?? ""}
                        onChange={(e) =>
                          setTargetKinds((prev) => ({
                            ...prev,
                            [item.id]: e.target.value as ReviewTargetKind,
                          }))
                        }
                      >
                        <option value="">{t("common.select")}</option>
                        {(Object.keys(TARGET_KIND_LABEL) as Array<Exclude<ReviewTargetKind, "">>).map(
                          (k) => (
                            <option key={k} value={k}>
                              {TARGET_KIND_LABEL[k]}
                            </option>
                          ),
                        )}
                      </select>
                    </div>
                    {targetKinds[item.id] === "claim" ? (
                      <div className="field" style={{ flex: "1 1 170px" }}>
                        <label htmlFor={`ck-${item.id}`}>{t("review.targetClaimKindLabel")}</label>
                        <select
                          id={`ck-${item.id}`}
                          className="input"
                          value={targetClaimKinds[item.id] ?? ""}
                          onChange={(e) =>
                            setTargetClaimKinds((prev) => ({ ...prev, [item.id]: e.target.value }))
                          }
                        >
                          <option value="">{t("common.select")}</option>
                          {Object.entries(CLAIM_KIND).map(([k, v]) => (
                            <option key={k} value={k}>
                              {v}
                            </option>
                          ))}
                        </select>
                      </div>
                    ) : null}
                    <span className="hint" style={{ flex: "2 1 220px", margin: 0 }}>
                      {t("review.commitHint")}
                    </span>
                  </div>
                )}
                <div className="row" style={{ justifyContent: "flex-end" }}>
                  {editingId === item.id ? (
                    <>
                      <button className="btn ghost" onClick={() => setEditingId(null)}>{t("review.cancelAction")}</button>
                      <button className="btn primary" disabled={busy} onClick={() => void decide(item, "edit")}>{t("common.save")}</button>
                    </>
                  ) : (
                    <>
                      <button
                        className="btn ghost"
                        onClick={() => {
                          setEditingId(item.id);
                          setEditJson(JSON.stringify(item.payload, null, 2));
                        }}
                      >
                        {t("review.editAction")}
                      </button>
                      <button className="btn ghost" disabled={busy} onClick={() => void decide(item, "request_evidence")}>
                        {t("review.requestEvidence")}
                      </button>
                      <button className="btn ghost" disabled={busy} onClick={() => void decide(item, "reject")}>{t("review.rejectAction")}</button>
                      <button
                        className="btn primary"
                        disabled={
                          busy ||
                          !(targetKinds[item.id] ?? "") ||
                          (targetKinds[item.id] === "claim" && !(targetClaimKinds[item.id] ?? ""))
                        }
                        onClick={() => void commit(item)}
                      >
                        {t("review.acceptCommit")}
                      </button>
                    </>
                  )}
                </div>
                {replaceable.length > 0 ? (
                  <div className="row" style={{ justifyContent: "flex-end" }}>
                    <div className="field" style={{ flex: "2 1 220px" }}>
                      <label htmlFor={`sup-${item.id}`}>{t("review.supersedeLabel")}</label>
                      <select
                        id={`sup-${item.id}`}
                        className="input"
                        value={supersedeTargets[item.id] ?? ""}
                        onChange={(e) =>
                          setSupersedeTargets((prev) => ({ ...prev, [item.id]: e.target.value }))
                        }
                      >
                        <option value="">{t("review.recordPlaceholder")}</option>
                        {replaceable.map((d) => (
                          <option key={d.id} value={d.id}>
                            {ITEM_STATE[d.state] ?? d.state} · {ITEM_KIND[d.itemKind] ?? d.itemKind} · {formatTimestamp(d.createdAt)}
                          </option>
                        ))}
                      </select>
                    </div>
                    <button
                      className="btn ghost"
                      disabled={busy || !supersedeTargets[item.id]}
                      onClick={() => void supersede(item, supersedeTargets[item.id] ?? "")}
                    >
                      {t("review.supersedeAction")}
                    </button>
                  </div>
                ) : null}
              </div>
            ))
          )}

          {decided.length > 0 ? (
            <div>
              <h2 style={{ marginTop: 8 }}>{t("review.decidedTitle")}</h2>
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                {decided.map((item) => (
                  <div key={item.id} className="row" style={{ justifyContent: "space-between", padding: "6px 8px", border: "1px solid var(--line)", borderRadius: "var(--radius)" }}>
                    <span className="small">
                      {ITEM_KIND[item.itemKind] ?? item.itemKind}
                      {item.targetEntityId ? (
                        <span className="badge plain" style={{ marginLeft: 8 }}>
                          {t("review.targetBadge", { id: item.targetEntityId.slice(0, 8) })}
                        </span>
                      ) : null}
                    </span>
                    <span className={`badge ${item.state === "accepted" ? "ok" : item.state === "rejected" ? "danger" : "warn"}`}>
                      {ITEM_STATE[item.state] ?? item.state}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          ) : null}
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("review.quoteTitle")}</h2>
            <p className="card-sub">{t("review.quoteLead")}</p>
          </div>
        </div>
        <div className="row">
          <div className="field" style={{ flex: "1 1 240px" }}>
            <label htmlFor="rv-qclaim">{t("review.useClaimLabel")}</label>
            <select
              id="rv-qclaim"
              className="input"
              value={quoteSource}
              onChange={(e) => {
                const id = e.target.value;
                setQuoteSource(id);
                const claim = claims.find((c) => c.id === id);
                if (claim) {
                  setQuoteText(claim.claimText);
                  setQuoteCheck(null);
                  setQuoteBindings({});
                }
              }}
            >
              <option value="">{t("common.select")}</option>
              {claims.map((claim) => (
                <option key={claim.id} value={claim.id}>
                  {claim.claimText.slice(0, 90)}
                </option>
              ))}
            </select>
          </div>
          <div className="field" style={{ flex: "1 1 240px" }}>
            <label htmlFor="rv-qscope">{t("review.scopeLabel")}</label>
            <select
              id="rv-qscope"
              className="input"
              value={quoteScope}
              onChange={(e) => {
                setQuoteScope(e.target.value);
                setQuoteCheck(null);
                setQuoteBindings({});
              }}
            >
              <option value="">{t("review.scopeAll")}</option>
              {documents.map((document) => (
                <option key={document.id} value={document.id}>
                  {document.displayName}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="field">
          <label htmlFor="rv-qtext">{t("review.quoteTextLabel")}</label>
          <textarea
            id="rv-qtext"
            className="input"
            rows={3}
            value={quoteText}
            onChange={(e) => {
              setQuoteText(e.target.value);
              setQuoteCheck(null);
              setQuoteBindings({});
            }}
            placeholder={t("review.quotePlaceholder")}
          />
        </div>
        <div className="row" style={{ justifyContent: "flex-end" }}>
          <span className="hint" style={{ margin: 0, flex: "1 1 220px" }}>
            {t("review.quoteHint")}
          </span>
          <button
            className="btn primary"
            disabled={quoteBusy || quoteText.trim().length === 0}
            onClick={() => void runQuoteCheck()}
          >
            {quoteBusy ? t("review.quoteBusy") : t("review.quoteRun")}
          </button>
        </div>

        {quoteCheck ? (
          <div className="card tight" style={{ background: "var(--surface)" }}>
            <div className="row" style={{ justifyContent: "space-between" }}>
              <strong className="small">{quoteCheckSummary(quoteCheck)}</strong>
              <span className="muted small">
                {[
                  scopeLabel(quoteCheck),
                  t("intext.pagesScanned", { count: quoteCheck.pagesScanned }),
                  versionsLabel(quoteCheck),
                ]
                  .filter((part) => part !== "")
                  .join(" · ")}
              </span>
            </div>
            {skippedLabel(quoteCheck) ? (
              <p className="hint" style={{ margin: 0 }}>{skippedLabel(quoteCheck)}</p>
            ) : null}
            {quoteCheck.quotes.length === 0 ? (
              <p className="hint" style={{ margin: 0 }}>{t("review.quoteEmpty")}</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                {quoteCheck.quotes.map((finding) => (
                  <div
                    key={finding.index}
                    style={{
                      border: "1px solid var(--line)",
                      borderRadius: "var(--radius)",
                      padding: "8px 10px",
                      background: "var(--bg)",
                    }}
                  >
                    <div className="row" style={{ justifyContent: "space-between" }}>
                      <strong className="small">“{finding.quote}”</strong>
                      <span className={`badge ${quoteStateTone(finding.state)}`}>
                        {QUOTE_STATE_LABEL[finding.state] ?? finding.state}
                      </span>
                    </div>
                    {finding.reason === "no_text" ? (
                      <p className="hint" style={{ margin: 0 }}>
                        {t("review.noText")}
                      </p>
                    ) : null}
                    {finding.position ? (
                      <p className="muted small" style={{ margin: 0 }}>
                        {[
                          positionLabel(finding),
                          t("review.sourcePrefix", {
                            text: finding.position.sourceText
                              .replace(/\s*\n\s*/g, " ⏎ ")
                              .slice(0, 140),
                          }),
                        ].join(" · ")}
                      </p>
                    ) : null}
                    {finding.position?.spansPages ? (
                      <p className="hint" style={{ margin: 0 }}>
                        {t("review.spanPagesHint", {
                          start: finding.position.pageIndex + 1,
                          end: finding.position.endPageIndex + 1,
                        })}
                      </p>
                    ) : null}
                    {oldVersionNote(finding) ? (
                      <p className="hint" style={{ margin: 0 }}>
                        <span className="badge warn" style={{ marginRight: 6 }}>
                          {t("review.oldVersionBadge")}
                        </span>
                        {oldVersionNote(finding)}
                      </p>
                    ) : null}
                    {finding.candidates.length > 0 ? (
                      <ul className="small" style={{ margin: "4px 0 0", paddingLeft: 18 }}>
                        {finding.candidates.map((candidate, candidateIndex) => (
                          <li key={`${candidate.documentId}-${candidate.pageIndex}-${candidate.charStart}-${candidateIndex}`}>
                            {candidateLabel(candidate)}
                          </li>
                        ))}
                      </ul>
                    ) : null}
                    <div className="row" style={{ justifyContent: "flex-end", marginTop: 4 }}>
                      {quoteBindings[finding.index] ? (
                        <span className="badge ok">
                          {t("review.boundBadge", {
                            id: quoteBindings[finding.index]?.slice(0, 8) ?? "",
                          })}
                        </span>
                      ) : canBind(finding) ? (
                        <button className="btn" disabled={quoteBusy} onClick={() => void bindQuote(finding)}>
                          {t("review.bindQuoteAction")}
                        </button>
                      ) : (
                        <span className="hint" style={{ margin: 0 }}>
                          {bindHint(finding)}
                        </span>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
            {quoteCheck.notes.length > 0 ? (
              <details>
                <summary className="small">
                  {t("review.notesSummary", { count: quoteCheck.notes.length })}
                </summary>
                <ul className="small" style={{ margin: "4px 0 0", paddingLeft: 18 }}>
                  {quoteCheck.notes.map((note, noteIndex) => (
                    <li key={noteIndex}>{note}</li>
                  ))}
                </ul>
              </details>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}
