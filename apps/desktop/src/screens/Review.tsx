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
  type EvidenceLinkRow,
  type ReviewItemRow,
} from "../lib/api";
import { formatTimestamp } from "../lib/format";
import {
  buildReviewTarget,
  itemText,
  TARGET_KIND_LABEL,
  type ReviewTargetKind,
} from "../lib/reviewTarget";

const CLAIM_KIND: Record<string, string> = {
  allegation: "İddia",
  defense: "Savunma",
  admission: "Kabul",
  court_finding: "Mahkeme tespiti",
  unknown: "Bilinmiyor",
};

const RELATION: Record<string, string> = {
  submitted_for: "Sunuldu",
  supports_candidate: "Destekliyor (aday)",
  contradicts_candidate: "Çelişiyor (aday)",
  context: "Bağlam",
};

const CLAIM_RELATION: Record<string, string> = {
  response_to: "Yanıt (yanıtlanan iddia)",
  supports: "Destekliyor",
  contradicts: "Çelişiyor",
  context: "Bağlam",
};

const ITEM_KIND: Record<string, string> = {
  claim_candidate: "İddia adayı",
  event_candidate: "Olay adayı",
  note_candidate: "Not adayı",
  task_candidate: "Görev adayı",
  contradiction_candidate: "Çelişki adayı",
};

const VERDICT: Record<string, string> = {
  conflict: "Çelişki var",
  explained: "Açıklanabilir fark",
  not_conflict: "Çelişki yok",
  needs_more_evidence: "Delil gerekiyor",
};

const ITEM_STATE: Record<string, string> = {
  pending: "Bekliyor",
  accepted: "Kabul",
  rejected: "Ret",
  edited: "Düzeltildi",
  needs_more_evidence: "Delil bekliyor",
  superseded: "Yerine geçildi",
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
      const [claimRows, itemRows, anchorRows] = await Promise.all([
        api.claimList(id),
        api.reviewItemList(id),
        api.anchorList(id),
      ]);
      setClaims(claimRows);
      setItems(itemRows);
      setAnchors(anchorRows);
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
        message: "Çelişki için iki farklı kaynak alıntı seçilmeli.",
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
          setError({ code: "E_JSON", message: "Düzeltilmiş içerik geçerli JSON değil.", retryable: false });
          setBusy(false);
          return;
        }
      }
      await api.reviewDecide(item.id, decision, decision === "edit" ? "kullanıcı düzeltmesi" : "", edited ?? undefined);
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
        message: "Kabul için kayıt türü seçilmeli; iddiada iddia türü de açıkça seçilir.",
        retryable: false,
      });
      return;
    }
    setBusy(true);
    try {
      await api.reviewCommit(item.id, "accept", target, "kullanıcı onayı");
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
        <h1>İnceleme</h1>
        <p className="lead">
          İddia ile belge arasındaki ilişki kaydedilir; iddianın doğruluğu sistemce kesinleştirilmez.
          Öneriler ancak burada karar verildikten sonra kalıcı kayda dönüşür.
        </p>
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
              Güncel kaydı yükle
            </button>
          ) : null}
        </div>
      ) : null}

      <div className="card tight">
        <div className="row">
          <div className="field" style={{ flex: "1 1 260px" }}>
            <label htmlFor="rv-case">Dosya</label>
            <select id="rv-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.length === 0 ? <option value="">Dosya yok</option> : null}
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field" style={{ flex: "2 1 300px" }}>
            <label htmlFor="rv-claim">Yeni iddia</label>
            <input
              id="rv-claim"
              className="input"
              value={claimText}
              onChange={(e) => setClaimText(e.target.value)}
              placeholder="Örn. Kiracı kira bedelini ödemedi"
              onKeyDown={(e) => e.key === "Enter" && void addClaim()}
            />
          </div>
          <div className="field" style={{ flex: "0 1 170px" }}>
            <label htmlFor="rv-kind">Tür</label>
            <select id="rv-kind" className="input" value={claimKind} onChange={(e) => setClaimKind(e.target.value)}>
              <option value="">Tür seçin…</option>
              {Object.entries(CLAIM_KIND).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
            </select>
          </div>
          <button className="btn primary" disabled={busy || claimKind === "" || claimText.trim().length === 0} onClick={() => void addClaim()}>Ekle</button>
        </div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-4)" }}>
        <div className="card">
          <div className="card-head">
            <div>
              <h2>İddialar</h2>
              <p className="card-sub">{claims.length} kayıt</p>
            </div>
          </div>
          {claims.length === 0 ? (
            <div className="empty"><p style={{ margin: 0 }}>Henüz iddia yok.</p></div>
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
              <h2>Delil bağı</h2>
              {links.length === 0 ? (
                <p className="hint" style={{ margin: 0 }}>Bu iddiaya bağlı belge yok.</p>
              ) : (
                <ul style={{ margin: 0, paddingLeft: 18 }}>
                  {links.map((link) => (
                    <li key={link.id} className="small">
                      {RELATION[link.relation] ?? link.relation} ·{" "}
                      {anchors.find((a) => a.id === link.sourceAnchorId)?.exactQuote.slice(0, 60) ?? "alıntı"}
                    </li>
                  ))}
                </ul>
              )}
              <div className="field">
                <label htmlFor="rv-anchor">Kaynak alıntı</label>
                <select id="rv-anchor" className="input" value={linkAnchor} onChange={(e) => setLinkAnchor(e.target.value)}>
                  <option value="">Seçin…</option>
                  {anchors.map((a) => (
                    <option key={a.id} value={a.id}>“{a.exactQuote.slice(0, 70)}”</option>
                  ))}
                </select>
              </div>
              <div className="row">
                <div className="field">
                  <label htmlFor="rv-rel">İlişki</label>
                  <select id="rv-rel" className="input" value={linkRelation} onChange={(e) => setLinkRelation(e.target.value)}>
                    <option value="">İlişki seçin…</option>
                    {Object.entries(RELATION).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
                  </select>
                </div>
                <div className="field" style={{ flex: "2 1 200px" }}>
                  <label htmlFor="rv-rat">Gerekçe</label>
                  <input id="rv-rat" className="input" value={linkRationale} onChange={(e) => setLinkRationale(e.target.value)} />
                </div>
                <button className="btn" disabled={busy || !linkAnchor || linkRelation === ""} onClick={() => void addLink()}>Bağla</button>
              </div>

              <h2 style={{ marginTop: 12 }}>İddia ilişkileri</h2>
              {relations.length === 0 ? (
                <p className="hint" style={{ margin: 0 }}>Bu iddiaya bağlı ilişki yok.</p>
              ) : (
                <ul style={{ margin: 0, paddingLeft: 18 }}>
                  {relations.map((rel) => {
                    const outgoing = rel.claimId === selectedClaim.id;
                    const otherId = outgoing ? rel.relatedClaimId : rel.claimId;
                    const other = claims.find((c) => c.id === otherId);
                    return (
                      <li key={rel.id} className="small">
                        {outgoing ? "→" : "←"} {CLAIM_RELATION[rel.relation] ?? rel.relation} ·{" "}
                        {other?.claimText ?? "iddia"}
                        {rel.rationale ? ` · ${rel.rationale}` : ""}
                      </li>
                    );
                  })}
                </ul>
              )}
              <div className="row">
                <div className="field" style={{ flex: "1 1 180px" }}>
                  <label htmlFor="rv-reltarget">İlgili iddia</label>
                  <select
                    id="rv-reltarget"
                    className="input"
                    value={relationTarget}
                    onChange={(e) => setRelationTarget(e.target.value)}
                  >
                    <option value="">İddia seçin…</option>
                    {claims
                      .filter((c) => c.id !== selectedClaim.id)
                      .map((c) => (
                        <option key={c.id} value={c.id}>{c.claimText}</option>
                      ))}
                  </select>
                </div>
                <div className="field" style={{ flex: "0 1 190px" }}>
                  <label htmlFor="rv-relkind">İlişki türü</label>
                  <select
                    id="rv-relkind"
                    className="input"
                    value={relationKind}
                    onChange={(e) => setRelationKind(e.target.value)}
                  >
                    <option value="">Tür seçin…</option>
                    {Object.entries(CLAIM_RELATION).map(([k, v]) => (
                      <option key={k} value={k}>{v}</option>
                    ))}
                  </select>
                </div>
                <div className="field" style={{ flex: "2 1 160px" }}>
                  <label htmlFor="rv-relnote">Gerekçe</label>
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
                  İlişkilendir
                </button>
              </div>
              <p className="hint" style={{ margin: 0 }}>
                Yanıt belgeye değil yanıtlanan iddiaya bağlanır; ilişki türü açıkça seçilir.
              </p>
            </div>
          ) : null}
        </div>

        <div className="card">
          <div className="card-head">
            <div>
              <h2>İnceleme kuyruğu</h2>
              <p className="card-sub">{pending.length} bekleyen · {decided.length} karar verilmiş · {outboxPending} bildirim bekliyor</p>
            </div>
          </div>

          <div className="card tight" style={{ background: "var(--surface)" }}>
            <h2>Alıntıdan aday oluştur</h2>
            <div className="row">
              <div className="field" style={{ flex: "1 1 160px" }}>
                <label htmlFor="rv-dkind">Tür</label>
                <select id="rv-dkind" className="input" value={draftKind} onChange={(e) => setDraftKind(e.target.value)}>
                  {Object.entries(ITEM_KIND).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
                </select>
              </div>
              <div className="field" style={{ flex: "2 1 180px" }}>
                <label htmlFor="rv-danchor">Kaynak</label>
                <select id="rv-danchor" className="input" value={draftAnchor} onChange={(e) => setDraftAnchor(e.target.value)}>
                  <option value="">Seçin…</option>
                  {anchors.map((a) => (
                    <option key={a.id} value={a.id}>“{a.exactQuote.slice(0, 60)}”</option>
                  ))}
                </select>
              </div>
              {draftKind === "contradiction_candidate" ? (
                <div className="field" style={{ flex: "2 1 180px" }}>
                  <label htmlFor="rv-danchor2">İkinci kaynak</label>
                  <select id="rv-danchor2" className="input" value={draftAnchor2} onChange={(e) => setDraftAnchor2(e.target.value)}>
                    <option value="">Seçin…</option>
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
              <label htmlFor="rv-dtext">Aday metni</label>
              <input id="rv-dtext" className="input" value={draftText} onChange={(e) => setDraftText(e.target.value)} />
            </div>
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button className="btn" disabled={busy || !draftAnchor || draftText.trim().length === 0 || (draftKind === "contradiction_candidate" && (!draftAnchor2 || draftAnchor2 === draftAnchor))} onClick={() => void addCandidate()}>
                Kuyruğa ekle
              </button>
            </div>
            <p className="hint" style={{ margin: 0 }}>
              AI bağlı değilken adayler kullanıcı tarafından eklenir; kaynak zorunludur.
            </p>
          </div>

          {pending.length === 0 ? (
            <div className="empty"><p style={{ margin: 0 }}>Bekleyen öğe yok.</p></div>
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
                <div className="muted small">{item.sourceAnchorIds.length} kaynak alıntı</div>
                {item.itemKind === "contradiction_candidate" ? (
                  <div>
                    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
                      {item.sourceAnchorIds.slice(0, 2).map((anchorId, idx) => {
                        const anchor = anchors.find((a) => a.id === anchorId);
                        return (
                          <div key={anchorId} className="card tight" style={{ background: "var(--bg)", margin: 0 }}>
                            <span className="badge plain">{idx === 0 ? "Kaynak 1" : "Kaynak 2"}</span>
                            <p className="small" style={{ margin: 0 }}>
                              “{anchor?.exactQuote ?? "alıntı bulunamadı"}”
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
                          Son değerlendirme: <strong>{VERDICT[latest.verdict] ?? latest.verdict}</strong>
                          {latest.rationale ? ` — ${latest.rationale}` : ""}
                        </p>
                      ) : (
                        <p className="hint" style={{ margin: 0 }}>
                          İki kaynak yan yana; henüz değerlendirme kaydedilmedi.
                        </p>
                      );
                    })()}
                    <div className="row">
                      <div className="field" style={{ flex: "0 1 200px" }}>
                        <label htmlFor={`verdict-${item.id}`}>Değerlendirme</label>
                        <select
                          id={`verdict-${item.id}`}
                          className="input"
                          value={assessmentVerdicts[item.id] ?? ""}
                          onChange={(e) =>
                            setAssessmentVerdicts((prev) => ({ ...prev, [item.id]: e.target.value }))
                          }
                        >
                          <option value="">Seçin…</option>
                          {Object.entries(VERDICT).map(([k, v]) => (
                            <option key={k} value={k}>{v}</option>
                          ))}
                        </select>
                      </div>
                      <div className="field" style={{ flex: "2 1 180px" }}>
                        <label htmlFor={`verdict-note-${item.id}`}>Gerekçe</label>
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
                        Değerlendir
                      </button>
                    </div>
                  </div>
                ) : null}
                {editingId === item.id ? (
                  <div className="field">
                    <label htmlFor={`edit-${item.id}`}>Düzeltilmiş JSON</label>
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
                      <label htmlFor={`rk-${item.id}`}>Kabulde kayıt türü</label>
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
                        <option value="">Seçin…</option>
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
                        <label htmlFor={`ck-${item.id}`}>İddia türü</label>
                        <select
                          id={`ck-${item.id}`}
                          className="input"
                          value={targetClaimKinds[item.id] ?? ""}
                          onChange={(e) =>
                            setTargetClaimKinds((prev) => ({ ...prev, [item.id]: e.target.value }))
                          }
                        >
                          <option value="">Seçin…</option>
                          {Object.entries(CLAIM_KIND).map(([k, v]) => (
                            <option key={k} value={k}>
                              {v}
                            </option>
                          ))}
                        </select>
                      </div>
                    ) : null}
                    <span className="hint" style={{ flex: "2 1 220px", margin: 0 }}>
                      Karar, hedef kayıt, denetim kaydı ve bildirim tek işlemde yazılır; biri
                      düşerse hiçbiri kalmaz.
                    </span>
                  </div>
                )}
                <div className="row" style={{ justifyContent: "flex-end" }}>
                  {editingId === item.id ? (
                    <>
                      <button className="btn ghost" onClick={() => setEditingId(null)}>Vazgeç</button>
                      <button className="btn primary" disabled={busy} onClick={() => void decide(item, "edit")}>Kaydet</button>
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
                        Düzelt
                      </button>
                      <button className="btn ghost" disabled={busy} onClick={() => void decide(item, "request_evidence")}>
                        Delil iste
                      </button>
                      <button className="btn ghost" disabled={busy} onClick={() => void decide(item, "reject")}>Ret</button>
                      <button
                        className="btn primary"
                        disabled={
                          busy ||
                          !(targetKinds[item.id] ?? "") ||
                          (targetKinds[item.id] === "claim" && !(targetClaimKinds[item.id] ?? ""))
                        }
                        onClick={() => void commit(item)}
                      >
                        Kabul et ve kaydet
                      </button>
                    </>
                  )}
                </div>
                {replaceable.length > 0 ? (
                  <div className="row" style={{ justifyContent: "flex-end" }}>
                    <div className="field" style={{ flex: "2 1 220px" }}>
                      <label htmlFor={`sup-${item.id}`}>Yerine geçeceği kayıt</label>
                      <select
                        id={`sup-${item.id}`}
                        className="input"
                        value={supersedeTargets[item.id] ?? ""}
                        onChange={(e) =>
                          setSupersedeTargets((prev) => ({ ...prev, [item.id]: e.target.value }))
                        }
                      >
                        <option value="">Kayıt seçin…</option>
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
                      Yerine geç
                    </button>
                  </div>
                ) : null}
              </div>
            ))
          )}

          {decided.length > 0 ? (
            <div>
              <h2 style={{ marginTop: 8 }}>Karar verilmiş</h2>
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                {decided.map((item) => (
                  <div key={item.id} className="row" style={{ justifyContent: "space-between", padding: "6px 8px", border: "1px solid var(--line)", borderRadius: "var(--radius)" }}>
                    <span className="small">
                      {ITEM_KIND[item.itemKind] ?? item.itemKind}
                      {item.targetEntityId ? (
                        <span className="badge plain" style={{ marginLeft: 8 }}>
                          kayıt: {item.targetEntityId.slice(0, 8)}
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
    </div>
  );
}
