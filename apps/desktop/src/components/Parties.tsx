import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  api,
  toApiError,
  type ActorMergeRow,
  type ApiError,
  type CaseParties,
} from "../lib/api";
import { formatTimestamp } from "../lib/format";
import { t } from "../lib/i18n";

/**
 * Dosya tarafları kartı (şartname F08-T01 / REQ-010).
 *
 * `actors` ve `case_participants` tabloları şemanın ilk sürümünden beri vardı
 * ama üründe **hiç kullanılmıyordu**: kişi/kurum kaydı, dosya rolü ve
 * birleştirme yoktu. Bu kart gerçek çekirdek komutlarını çağırır
 * (`case_party_list/add/remove`, `actor_merge`, `actor_merge_undo`); yerel
 * sahte liste tutmaz.
 *
 * Kapılar:
 * - Dosya değişince liste, form, birleştirme seçimi ve bildirim **birlikte**
 *   düşer; düşen/geç gelen eski yanıt yeni dosyanın altına yazılmaz (`seq`).
 * - Düşen okuma "taraf yok" **demez** (`role=alert`); liste temizlenir.
 * - Birleştirme yalnız onay kutusu işaretliyken ve farklı iki tarafla çağrılır;
 *   geri alma kaydı çekirdekten okunur (oturum belleğine bağlı değildir).
 * - Bilinmeyen tür/rol ham gösterilir; etiket uydurulmaz.
 */
const KIND_LABEL: Record<string, string> = {
  person: t("parties.kindPerson"),
  organization: t("parties.kindOrganization"),
  unknown: t("parties.kindUnknown"),
};

export function Parties({ caseId }: { caseId: string }) {
  const [parties, setParties] = useState<CaseParties | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const [name, setName] = useState("");
  const [kind, setKind] = useState("person");
  const [role, setRole] = useState("");
  const [note, setNote] = useState("");
  const [actorId, setActorId] = useState("");

  const [mergeSource, setMergeSource] = useState("");
  const [mergeTarget, setMergeTarget] = useState("");
  const [mergeConfirmed, setMergeConfirmed] = useState(false);

  const seq = useRef(0);

  const fetchParties = useCallback(async (id: string, clearList: boolean): Promise<boolean> => {
    const tick = ++seq.current;
    setLoading(true);
    setError(null);
    if (clearList) setParties(null);
    try {
      const data = await api.casePartyList(id);
      if (seq.current !== tick) return false;
      setParties(data);
      return true;
    } catch (e) {
      if (seq.current !== tick) return false;
      // Düşen okuma "taraf yok" demez: liste temizlenir, hata ayrı satırda.
      setError(toApiError(e));
      setParties(null);
      return false;
    } finally {
      if (seq.current === tick) setLoading(false);
    }
  }, []);

  useEffect(() => {
    // Dosya değişimi: bütün kart durumu birlikte düşer (INV-05).
    seq.current += 1;
    setLoading(false);
    setBusy(false);
    setError(null);
    setNotice(null);
    setName("");
    setRole("");
    setNote("");
    setActorId("");
    setMergeSource("");
    setMergeTarget("");
    setMergeConfirmed(false);
    const id = caseId.trim();
    if (id) void fetchParties(id, true);
    return () => {
      // Kapanan kartın geç yanıtı yeni duruma yazılmaz.
      seq.current += 1;
    };
  }, [caseId, fetchParties]);

  const actors = useMemo(() => {
    const seen = new Map<string, { name: string; kind: string }>();
    for (const row of parties?.participants ?? []) {
      if (!seen.has(row.actorId)) seen.set(row.actorId, { name: row.displayName, kind: row.kind });
    }
    return [...seen.entries()].map(([id, value]) => ({ id, ...value }));
  }, [parties]);

  async function add() {
    const id = caseId.trim();
    if (!id || busy) return;
    const existing = actorId !== "";
    if (role.trim() === "" || (!existing && name.trim() === "")) return;
    const tick = seq.current;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await api.casePartyAdd(id, {
        displayName: existing ? (actors.find((actor) => actor.id === actorId)?.name ?? "") : name.trim(),
        kind: existing ? (actors.find((actor) => actor.id === actorId)?.kind ?? kind) : kind,
        role: role.trim(),
        roleNote: note.trim() === "" ? null : note.trim(),
        actorId: existing ? actorId : null,
      });
      if (seq.current !== tick) return;
      setName("");
      setRole("");
      setNote("");
      if (await fetchParties(id, false)) setNotice(t("parties.added"));
    } catch (e) {
      if (seq.current === tick) setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove(participantId: string) {
    const id = caseId.trim();
    if (!id || busy) return;
    const tick = seq.current;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await api.casePartyRemove(participantId);
      if (seq.current !== tick) return;
      if (await fetchParties(id, false)) setNotice(t("parties.removed"));
    } catch (e) {
      if (seq.current === tick) setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function merge() {
    const id = caseId.trim();
    if (!id || busy) return;
    if (mergeSource === "" || mergeTarget === "" || mergeSource === mergeTarget || !mergeConfirmed) return;
    const tick = seq.current;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await api.actorMerge(mergeSource, mergeTarget, "local-user");
      if (seq.current !== tick) return;
      setMergeSource("");
      setMergeTarget("");
      setMergeConfirmed(false);
      if (await fetchParties(id, false)) setNotice(t("parties.merged"));
    } catch (e) {
      if (seq.current === tick) setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function undo(merge: ActorMergeRow) {
    const id = caseId.trim();
    if (!id || busy || merge.undoneAt !== null) return;
    const tick = seq.current;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await api.actorMergeUndo(merge.id);
      if (seq.current !== tick) return;
      if (await fetchParties(id, false)) setNotice(t("parties.mergeUndone"));
    } catch (e) {
      if (seq.current === tick) setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  const canAdd =
    !busy && role.trim() !== "" && (actorId !== "" || name.trim() !== "");

  return (
    <div className="card">
      <div className="card-head">
        <div>
          <h2>{t("parties.title")}</h2>
          <p className="card-sub">{t("parties.lead")}</p>
        </div>
      </div>

      {caseId.trim() === "" ? (
        <p className="muted small" style={{ margin: 0 }}>
          {t("parties.noCase")}
        </p>
      ) : (
        <>
          {error ? (
            <div className="notice danger" role="alert">
              <strong>{error.code}</strong> — {error.message}
            </div>
          ) : null}
          {notice ? (
            <p className="muted small" role="status" style={{ marginTop: 0 }}>
              {notice}
            </p>
          ) : null}
          {loading ? <p className="muted small">{t("parties.loading")}</p> : null}
          {!loading && parties && parties.participants.length === 0 ? (
            <p className="muted small" style={{ margin: 0 }}>
              {t("parties.empty")}
            </p>
          ) : null}

          {parties && parties.participants.length > 0 ? (
            <table className="table">
              <thead>
                <tr>
                  <th>{t("parties.colName")}</th>
                  <th>{t("parties.colKind")}</th>
                  <th>{t("parties.colRole")}</th>
                  <th>{t("parties.colNote")}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {parties.participants.map((row) => (
                  <tr key={row.id}>
                    <td>{row.displayName}</td>
                    <td>
                      <span className="badge plain">{KIND_LABEL[row.kind] ?? row.kind}</span>
                    </td>
                    <td>{row.role}</td>
                    <td className="muted small">{row.roleNote ?? "—"}</td>
                    <td style={{ textAlign: "right" }}>
                      <button className="btn ghost" disabled={busy} onClick={() => void remove(row.id)}>
                        {t("parties.remove")}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : null}

          <div className="row" style={{ marginTop: "var(--space-3)", alignItems: "flex-end" }}>
            <div className="field" style={{ flex: "1 1 200px" }}>
              <label htmlFor="party-existing">{t("parties.existingLabel")}</label>
              <select
                id="party-existing"
                className="input"
                value={actorId}
                onChange={(e) => setActorId(e.target.value)}
                disabled={busy}
              >
                <option value="">{t("parties.existingNew")}</option>
                {actors.map((actor) => (
                  <option key={actor.id} value={actor.id}>
                    {actor.name}
                  </option>
                ))}
              </select>
            </div>
            <div className="field" style={{ flex: "1 1 200px" }}>
              <label htmlFor="party-name">{t("parties.nameLabel")}</label>
              <input
                id="party-name"
                className="input"
                value={name}
                placeholder={t("parties.namePlaceholder")}
                onChange={(e) => setName(e.target.value)}
                disabled={busy || actorId !== ""}
              />
            </div>
            <div className="field" style={{ flex: "0 1 170px" }}>
              <label htmlFor="party-kind">{t("parties.colKind")}</label>
              <select
                id="party-kind"
                className="input"
                value={kind}
                onChange={(e) => setKind(e.target.value)}
                disabled={busy || actorId !== ""}
              >
                {Object.entries(KIND_LABEL).map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="row" style={{ marginTop: "var(--space-2)", alignItems: "flex-end" }}>
            <div className="field" style={{ flex: "1 1 200px" }}>
              <label htmlFor="party-role">{t("parties.roleLabel")}</label>
              <input
                id="party-role"
                className="input"
                value={role}
                placeholder={t("parties.rolePlaceholder")}
                onChange={(e) => setRole(e.target.value)}
                disabled={busy}
              />
            </div>
            <div className="field" style={{ flex: "2 1 240px" }}>
              <label htmlFor="party-note">{t("parties.noteLabel")}</label>
              <input
                id="party-note"
                className="input"
                value={note}
                onChange={(e) => setNote(e.target.value)}
                disabled={busy}
              />
            </div>
            <button className="btn primary" disabled={!canAdd} onClick={() => void add()}>
              {t("parties.addAction")}
            </button>
          </div>

          {actors.length > 1 ? (
            <div className="card tight" style={{ background: "var(--surface)", marginTop: "var(--space-3)" }}>
              <h3 style={{ marginTop: 0 }}>{t("parties.mergeTitle")}</h3>
              <p className="muted small" style={{ marginTop: 0 }}>
                {t("parties.mergeLead")}
              </p>
              <div className="row" style={{ alignItems: "flex-end" }}>
                <div className="field" style={{ flex: "1 1 180px" }}>
                  <label htmlFor="party-merge-source">{t("parties.mergeSource")}</label>
                  <select
                    id="party-merge-source"
                    className="input"
                    value={mergeSource}
                    onChange={(e) => {
                      setMergeSource(e.target.value);
                      setMergeConfirmed(false);
                    }}
                    disabled={busy}
                  >
                    <option value="">{t("parties.mergePick")}</option>
                    {actors.map((actor) => (
                      <option key={actor.id} value={actor.id}>
                        {actor.name}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="field" style={{ flex: "1 1 180px" }}>
                  <label htmlFor="party-merge-target">{t("parties.mergeTarget")}</label>
                  <select
                    id="party-merge-target"
                    className="input"
                    value={mergeTarget}
                    onChange={(e) => {
                      setMergeTarget(e.target.value);
                      setMergeConfirmed(false);
                    }}
                    disabled={busy}
                  >
                    <option value="">{t("parties.mergePick")}</option>
                    {actors.map((actor) => (
                      <option key={actor.id} value={actor.id}>
                        {actor.name}
                      </option>
                    ))}
                  </select>
                </div>
                <label className="small" style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <input
                    type="checkbox"
                    checked={mergeConfirmed}
                    onChange={(e) => setMergeConfirmed(e.target.checked)}
                    disabled={busy}
                  />
                  {t("parties.mergeConfirm")}
                </label>
                <button
                  className="btn"
                  disabled={
                    busy ||
                    !mergeConfirmed ||
                    mergeSource === "" ||
                    mergeTarget === "" ||
                    mergeSource === mergeTarget
                  }
                  onClick={() => void merge()}
                >
                  {t("parties.mergeAction")}
                </button>
              </div>
            </div>
          ) : null}

          {parties && parties.merges.length > 0 ? (
            <div style={{ marginTop: "var(--space-3)" }}>
              <h3 style={{ marginTop: 0 }}>{t("parties.mergeHistoryTitle")}</h3>
              <table className="table">
                <thead>
                  <tr>
                    <th>{t("parties.mergeHistoryPair")}</th>
                    <th>{t("parties.mergeHistoryTime")}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {parties.merges.map((merge) => (
                    <tr key={merge.id}>
                      <td>
                        {t("parties.mergePair", {
                          source: merge.sourceDisplayName,
                          target: merge.targetDisplayName,
                        })}
                        {merge.undoneAt !== null ? (
                          <span className="badge plain" style={{ marginLeft: 8 }}>
                            {t("parties.mergePreviouslyUndone")}
                          </span>
                        ) : null}
                      </td>
                      <td>{formatTimestamp(merge.createdAt)}</td>
                      <td style={{ textAlign: "right" }}>
                        {merge.undoneAt === null ? (
                          <button className="btn ghost" disabled={busy} onClick={() => void undo(merge)}>
                            {t("parties.mergeUndo")}
                          </button>
                        ) : (
                          <span className="muted small">{formatTimestamp(merge.undoneAt)}</span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}
        </>
      )}
    </div>
  );
}
