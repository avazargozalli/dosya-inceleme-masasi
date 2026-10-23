import { useCallback, useEffect, useState } from "react";
import { api, toApiError, type ApiError, type CaseRow, type NewCaseInput } from "../lib/api";
import { AI_POLICY_LABEL, CASE_KIND_LABEL, formatTimestamp, SENSITIVITY_LABEL } from "../lib/format";

const EMPTY_FORM: NewCaseInput = {
  title: "",
  caseKind: "review",
  referenceCode: null,
  jurisdictionLabel: null,
  practiceArea: null,
  sensitivity: "normal",
  defaultAiPolicy: "off",
};

export function Cases() {
  const [rows, setRows] = useState<CaseRow[]>([]);
  const [showArchived, setShowArchived] = useState(false);
  const [selected, setSelected] = useState<CaseRow | null>(null);
  const [creating, setCreating] = useState(false);
  const [form, setForm] = useState<NewCaseInput>(EMPTY_FORM);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setError(null);
      setRows(await api.caseList(showArchived));
    } catch (e) {
      setError(toApiError(e));
    }
  }, [showArchived]);

  useEffect(() => {
    void load();
  }, [load]);

  async function create() {
    if (form.title.trim().length === 0) {
      setError({ code: "E_TITLE", message: "Dosya başlığı zorunlu.", retryable: false });
      return;
    }
    setBusy(true);
    try {
      const created = await api.caseCreate({
        ...form,
        title: form.title.trim(),
        referenceCode: form.referenceCode?.trim() || null,
        practiceArea: form.practiceArea?.trim() || null,
      });
      setCreating(false);
      setForm(EMPTY_FORM);
      await load();
      setSelected(created);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function mutate(action: "archive" | "restore" | "delete") {
    if (!selected) return;
    setBusy(true);
    try {
      if (action === "archive") await api.caseArchive(selected.id);
      if (action === "restore") await api.caseRestore(selected.id);
      if (action === "delete") await api.caseDelete(selected.id);
      setSelected(null);
      await load();
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page" style={{ maxWidth: 980 }}>
      <div className="page-head">
        <h1>Dosyalar</h1>
        <p className="lead">
          Her dosya kendi belge, not ve kronolojisini taşır. Bir dosyanın verisi başka dosyanın
          sorgusuna örtük olarak eklenmez (INV-05).
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="card-head">
          <div>
            <h2>İnceleme dosyaları</h2>
            <p className="card-sub">{rows.length} kayıt · {showArchived ? "arşiv dahil" : "yalnız aktif"}</p>
          </div>
          <div className="row" style={{ flexWrap: "nowrap" }}>
            <label className="row" style={{ alignItems: "center", gap: 6 }}>
              <input
                type="checkbox"
                checked={showArchived}
                onChange={(e) => setShowArchived(e.target.checked)}
                style={{ width: 15, height: 15 }}
              />
              <span className="small muted">Arşiv</span>
            </label>
            <button className="btn primary" onClick={() => setCreating((v) => !v)}>
              {creating ? "Vazgeç" : "Yeni dosya"}
            </button>
          </div>
        </div>

        {creating ? (
          <div className="card tight" style={{ background: "var(--surface)" }}>
            <div className="row">
              <div className="field" style={{ flex: "2 1 320px" }}>
                <label htmlFor="case-title">Başlık</label>
                <input
                  id="case-title"
                  className="input"
                  value={form.title}
                  onChange={(e) => setForm({ ...form, title: e.target.value })}
                  placeholder="Örn. Kira alacağı — 2026"
                  autoFocus
                />
              </div>
              <div className="field">
                <label htmlFor="case-kind">Tür</label>
                <select
                  id="case-kind"
                  className="input"
                  value={form.caseKind}
                  onChange={(e) => setForm({ ...form, caseKind: e.target.value as NewCaseInput["caseKind"] })}
                >
                  <option value="review">İnceleme dosyası</option>
                  <option value="reference_archive">Kaynak arşivi</option>
                </select>
              </div>
            </div>
            <div className="row">
              <div className="field">
                <label htmlFor="case-ref">Esas / referans</label>
                <input
                  id="case-ref"
                  className="input"
                  value={form.referenceCode ?? ""}
                  onChange={(e) => setForm({ ...form, referenceCode: e.target.value })}
                  placeholder="2026/0000"
                />
              </div>
              <div className="field">
                <label htmlFor="case-area">Alan</label>
                <input
                  id="case-area"
                  className="input"
                  value={form.practiceArea ?? ""}
                  onChange={(e) => setForm({ ...form, practiceArea: e.target.value })}
                  placeholder="Kira, iş, aile…"
                />
              </div>
            </div>
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button className="btn primary" disabled={busy} onClick={() => void create()}>
                {busy ? "Oluşturuluyor…" : "Dosyayı oluştur"}
              </button>
            </div>
          </div>
        ) : null}

        {rows.length === 0 ? (
          <div className="empty">
            <p style={{ margin: 0 }}>
              {showArchived ? "Bu kasada kayıt yok." : "Henüz aktif dosya yok. “Yeni dosya” ile başlayın."}
            </p>
          </div>
        ) : (
          <div style={{ border: "1px solid var(--line)", borderRadius: "var(--radius)" }}>
            {rows.map((row, index) => (
              <button
                key={row.id}
                onClick={() => setSelected(row)}
                style={{
                  display: "grid",
                  gridTemplateColumns: "1fr 120px 130px 150px",
                  gap: "var(--space-4)",
                  alignItems: "center",
                  width: "100%",
                  textAlign: "left",
                  padding: "10px var(--space-4)",
                  border: 0,
                  borderTop: index === 0 ? 0 : "1px solid var(--line)",
                  background: selected?.id === row.id ? "var(--surface)" : "transparent",
                  cursor: "pointer",
                }}
              >
                <span>
                  <span style={{ fontWeight: 500 }}>{row.title}</span>
                  <span className="muted small" style={{ display: "block" }}>
                    {row.referenceCode ?? "referans yok"} · {row.practiceArea ?? "alan belirtilmedi"}
                  </span>
                </span>
                <span className="badge plain">{CASE_KIND_LABEL[row.caseKind] ?? row.caseKind}</span>
                <span className="badge plain">{SENSITIVITY_LABEL[row.sensitivity] ?? row.sensitivity}</span>
                <span className="muted small">{formatTimestamp(row.updatedAt)}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      {selected ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>{selected.title}</h2>
              <p className="card-sub">
                {CASE_KIND_LABEL[selected.caseKind]} · {selected.status} · sürüm {selected.rowVersion}
              </p>
            </div>
            <button className="btn ghost" onClick={() => setSelected(null)}>Kapat</button>
          </div>
          <dl className="kv">
            <dt>Esas / referans</dt><dd>{selected.referenceCode ?? "—"}</dd>
            <dt>Yargı yeri</dt><dd>{selected.jurisdictionLabel ?? "—"}</dd>
            <dt>Hassasiyet</dt><dd>{SENSITIVITY_LABEL[selected.sensitivity]}</dd>
            <dt>AI politikası</dt><dd>{AI_POLICY_LABEL[selected.defaultAiPolicy]}</dd>
            <dt>Oluşturma</dt><dd>{formatTimestamp(selected.createdAt)}</dd>
            <dt>Son değişiklik</dt><dd>{formatTimestamp(selected.updatedAt)}</dd>
            <dt>Kimlik</dt><dd style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{selected.id}</dd>
          </dl>
          <div className="row" style={{ justifyContent: "flex-end" }}>
            {selected.status === "active" ? (
              <button className="btn" disabled={busy} onClick={() => void mutate("archive")}>Arşivle</button>
            ) : (
              <button className="btn" disabled={busy} onClick={() => void mutate("restore")}>Geri al</button>
            )}
            <button className="btn" disabled={busy} onClick={() => void mutate("delete")}>Sil (geri dönüşümlü)</button>
          </div>
          <p className="hint" style={{ margin: 0 }}>
            Belge ekleme F05'te açılır. Bu ekran yalnız dosya kaydını yönetir.
          </p>
        </div>
      ) : null}
    </div>
  );
}
