import { useCallback, useEffect, useState } from "react";
import {
  api,
  toApiError,
  type AnchorRow,
  type ApiError,
  type CaseRow,
  type ClaimRelationRow,
  type ClaimRow,
  type EvidenceLinkRow,
  type ReviewItemRow,
} from "../lib/api";
import { formatTimestamp } from "../lib/format";

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
  const [claimKind, setClaimKind] = useState("allegation");
  const [linkAnchor, setLinkAnchor] = useState("");
  const [linkRelation, setLinkRelation] = useState("supports_candidate");
  const [linkRationale, setLinkRationale] = useState("");
  const [draftKind, setDraftKind] = useState("note_candidate");
  const [draftAnchor, setDraftAnchor] = useState("");
  const [draftText, setDraftText] = useState("");
  const [editJson, setEditJson] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);

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
      return;
    }
    void api
      .evidenceLinkList(selectedClaim.id)
      .then(setLinks)
      .catch((e) => setError(toApiError(e)));
  }, [selectedClaim]);

  async function addClaim() {
    if (!caseId || claimText.trim().length === 0) return;
    setBusy(true);
    try {
      await api.claimCreate(caseId, {
        claimantActorId: null,
        claimText: claimText.trim(),
        claimKind,
        sourceAnchorId: null,
      });
      setClaimText("");
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function addLink() {
    if (!selectedClaim || !linkAnchor) return;
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

  async function addCandidate() {
    if (!draftAnchor || draftText.trim().length === 0) return;
    setBusy(true);
    try {
      await api.reviewItemCreate(caseId, {
        itemKind: draftKind,
        payload: { text: draftText.trim() },
        sourceAnchorIds: [draftAnchor],
        aiRunId: null,
      });
      setDraftText("");
      await load(caseId);
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

  const pending = items.filter((i) => i.state === "pending" || i.state === "needs_more_evidence");
  const decided = items.filter((i) => !(i.state === "pending" || i.state === "needs_more_evidence"));

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
              {Object.entries(CLAIM_KIND).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
            </select>
          </div>
          <button className="btn primary" disabled={busy} onClick={() => void addClaim()}>Ekle</button>
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
                    {Object.entries(RELATION).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
                  </select>
                </div>
                <div className="field" style={{ flex: "2 1 200px" }}>
                  <label htmlFor="rv-rat">Gerekçe</label>
                  <input id="rv-rat" className="input" value={linkRationale} onChange={(e) => setLinkRationale(e.target.value)} />
                </div>
                <button className="btn" disabled={busy || !linkAnchor} onClick={() => void addLink()}>Bağla</button>
              </div>
            </div>
          ) : null}
        </div>

        <div className="card">
          <div className="card-head">
            <div>
              <h2>İnceleme kuyruğu</h2>
              <p className="card-sub">{pending.length} bekleyen · {decided.length} karar verilmiş</p>
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
            </div>
            <div className="field">
              <label htmlFor="rv-dtext">Aday metni</label>
              <input id="rv-dtext" className="input" value={draftText} onChange={(e) => setDraftText(e.target.value)} />
            </div>
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button className="btn" disabled={busy || !draftAnchor || draftText.trim().length === 0} onClick={() => void addCandidate()}>
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
                      <button className="btn primary" disabled={busy} onClick={() => void decide(item, "accept")}>Kabul</button>
                    </>
                  )}
                </div>
              </div>
            ))
          )}

          {decided.length > 0 ? (
            <div>
              <h2 style={{ marginTop: 8 }}>Karar verilmiş</h2>
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                {decided.map((item) => (
                  <div key={item.id} className="row" style={{ justifyContent: "space-between", padding: "6px 8px", border: "1px solid var(--line)", borderRadius: "var(--radius)" }}>
                    <span className="small">{ITEM_KIND[item.itemKind] ?? item.itemKind}</span>
                    <span className={`badge ${item.state === "accepted" ? "ok" : item.state === "rejected" ? "danger" : "warn"}`}>
                      {item.state}
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
