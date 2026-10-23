import { useCallback, useEffect, useMemo, useState } from "react";
import {
  api,
  toApiError,
  type ApiError,
  type CaseRow,
  type DocumentRow,
  type SearchHit,
} from "../lib/api";
import { targetFromHit, type ReaderTarget } from "../lib/reader";
import { DualReader } from "../components/DualReader";

/** Alıntı içinde sorgu geçen kısmı işaretler; HTML üretmez (metin düğümü). */
function Highlight({ text, query }: { text: string; query: string }) {
  const q = query.trim();
  if (!q) return <>{text}</>;
  const lowered = text.toLocaleLowerCase("tr");
  const needle = q.toLocaleLowerCase("tr");
  const index = lowered.indexOf(needle);
  if (index < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, index)}
      <mark style={{ background: "var(--accent-soft)", color: "inherit", padding: "0 1px", borderRadius: 2 }}>
        {text.slice(index, index + needle.length)}
      </mark>
      {text.slice(index + needle.length)}
    </>
  );
}

export function Search() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState<string>("");
  const [query, setQuery] = useState("");
  /** Son aramanın sorgusu; kullanıcı kutuyu değiştirse de sonuç işaretlemesi
   *  ve okuyucu hedefi gerçekten aranan metne bağlı kalır. */
  const [searchedQuery, setSearchedQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[] | null>(null);
  const [docs, setDocs] = useState<DocumentRow[]>([]);
  const [reader, setReader] = useState<{ left: ReaderTarget | null; right: ReaderTarget | null } | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

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

  useEffect(() => {
    if (!caseId) {
      setDocs([]);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const rows = await api.documentList(caseId);
        if (!cancelled) setDocs(rows);
      } catch (e) {
        if (!cancelled) setError(toApiError(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [caseId]);

  // Dosya değişince önceki dosyanın okuyucu hedefleri taşınmaz.
  useEffect(() => {
    setReader(null);
    setHits(null);
    setSearchedQuery("");
  }, [caseId]);

  const run = useCallback(async () => {
    if (!caseId || query.trim().length === 0) return;
    const asked = query;
    setBusy(true);
    setError(null);
    try {
      setHits(await api.searchSegments(caseId, asked));
      setSearchedQuery(asked);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }, [caseId, query]);

  /** Sonucu seçilen panelde açar. Hedef, sonucun kendi sürümünü taşır;
   *  "son sürüm" varsayımı yapılmaz (F07 kabul testi). */
  function openHit(hit: SearchHit, side: "left" | "right") {
    const target = targetFromHit(hit, searchedQuery);
    if (!target) {
      setError({
        code: "E_SEARCH_VERSION",
        message: "Sonuç belge sürümü taşımıyor; okuyucuda açılamaz. Belgeyi yeniden ayrıştırın.",
        retryable: false,
      });
      return;
    }
    setError(null);
    setReader((prev) => ({
      left: side === "left" ? target : (prev?.left ?? null),
      right: side === "right" ? target : (prev?.right ?? null),
    }));
  }

  const selectedCase = useMemo(() => cases.find((c) => c.id === caseId), [cases, caseId]);

  return (
    <div className="page" style={{ maxWidth: 1100 }}>
      <div className="page-head">
        <h1>Arama</h1>
        <p className="lead">
          Arama yalnız seçili dosyanın metinleri üzerinde çalışır; başka dosyanın içeriği sonuca
          eklenmez. Sonuç, kaynak sayfası ve metniyle birlikte gösterilir; tek tıkla okuyucuda
          doğru sürüm ve sayfa açılır.
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="row">
          <div className="field" style={{ flex: "1 1 240px" }}>
            <label htmlFor="search-case">Dosya</label>
            <select id="search-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.length === 0 ? <option value="">Dosya yok</option> : null}
              {cases.map((c) => (
                <option key={c.id} value={c.id}>{c.title}</option>
              ))}
            </select>
          </div>
          <div className="field" style={{ flex: "2 1 320px" }}>
            <label htmlFor="search-q">Aranan metin</label>
            <input
              id="search-q"
              className="input"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void run()}
              placeholder="Örn. ödeme, itiraz, kira"
            />
          </div>
          <button className="btn primary" disabled={busy || !caseId || query.trim().length === 0} onClick={() => void run()}>
            {busy ? "Aranıyor…" : "Ara"}
          </button>
        </div>
        <p className="hint" style={{ margin: 0 }}>
          {selectedCase ? `${selectedCase.title} içinde aranıyor.` : "Önce bir dosya oluşturun."} Metin
          bulunamazsa belge henüz ayrıştırılmamış olabilir.
        </p>
      </div>

      {hits !== null ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>Sonuçlar</h2>
              <p className="card-sub">{hits.length} eşleşme · dosya kapsamı uygulandı</p>
            </div>
          </div>
          {hits.length === 0 ? (
            <div className="empty">
              <p style={{ margin: 0 }}>Eşleşme yok. Belgeleri “Ayrıştır” ile metne çevirdiniz mi?</p>
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
              {hits.map((hit) => (
                <div key={hit.segmentId} className="card tight" style={{ background: "var(--surface)" }}>
                  <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                    <span style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
                      <span className="badge plain">{hit.documentName ?? "belge bilinmiyor"}</span>
                      <span className="badge plain">sayfa {hit.pageIndex !== null ? hit.pageIndex + 1 : "?"}</span>
                    </span>
                    <span style={{ display: "flex", gap: 4 }}>
                      <button
                        className="btn ghost"
                        style={{ height: 26 }}
                        disabled={!hit.documentVersionId}
                        onClick={() => openHit(hit, "left")}
                        title={hit.documentVersionId ? "Sonucu sol panelde aç" : "Sonuçta sürüm kimliği yok"}
                      >
                        Solda aç
                      </button>
                      <button
                        className="btn ghost"
                        style={{ height: 26 }}
                        disabled={!hit.documentVersionId}
                        onClick={() => openHit(hit, "right")}
                        title={hit.documentVersionId ? "Sonucu sağ panelde aç" : "Sonuçta sürüm kimliği yok"}
                      >
                        Sağda aç
                      </button>
                    </span>
                  </div>
                  <p style={{ margin: 0, whiteSpace: "pre-wrap" }}>
                    <Highlight text={hit.rawText.length > 600 ? `${hit.rawText.slice(0, 600)}…` : hit.rawText} query={searchedQuery} />
                  </p>
                  <span className="muted small" style={{ fontFamily: "var(--mono)", fontSize: 11 }}>
                    segment {hit.segmentId.slice(0, 8)} · sürüm {(hit.documentVersionId ?? "—").slice(0, 8)}
                  </span>
                </div>
              ))}
            </div>
          )}
          <p className="hint" style={{ margin: 0 }}>
            “Solda aç / Sağda aç” sonucun kaydedildiği sürümü açar; iki sonucu yan yana koyup
            karşılaştırabilirsiniz.
          </p>
        </div>
      ) : null}

      {reader ? (
        <DualReader
          caseId={caseId}
          documents={docs}
          left={reader.left}
          right={reader.right}
          onClose={() => setReader(null)}
        />
      ) : null}
    </div>
  );
}
