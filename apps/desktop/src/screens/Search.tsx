import { useCallback, useEffect, useMemo, useState } from "react";
import {
  api,
  toApiError,
  type ApiError,
  type CaseRow,
  type CaseTagRow,
  type DocumentRow,
  type SearchFilters,
  type SearchHit,
  type SearchPage,
} from "../lib/api";
import { targetFromHit, type ReaderTarget } from "../lib/reader";
import { DualReader } from "../components/DualReader";
import {
  activeFilterCount,
  canLoadMore,
  DATE_ROLE_LABELS,
  describeFilters,
  EMPTY_SEARCH_FILTERS,
  filtersStale,
  mergeHits,
  OCR_STATUS_LABELS,
  resultSummary,
  textOriginLabel,
  toggleValue,
} from "../lib/search";

const PAGE_SIZE = 25;

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
  const [query, setQuery] = useState<string>("");
  /** Son aramanın sorgusu; kullanıcı kutuyu değiştirse de sonuç işaretlemesi
   *  ve okuyucu hedefi gerçekten aranan metne bağlı kalır. */
  const [searchedQuery, setSearchedQuery] = useState<string>("");
  /** Son aramanın filtreleri; çipte gösterilen "uygulanan" filtre budur. */
  const [appliedFilters, setAppliedFilters] = useState<SearchFilters>(EMPTY_SEARCH_FILTERS);
  const [draftFilters, setDraftFilters] = useState<SearchFilters>(EMPTY_SEARCH_FILTERS);
  const [page, setPage] = useState<SearchPage | null>(null);
  const [docs, setDocs] = useState<DocumentRow[]>([]);
  const [caseTags, setCaseTags] = useState<CaseTagRow[]>([]);
  const [reader, setReader] = useState<{ left: ReaderTarget | null; right: ReaderTarget | null } | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);

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
      setCaseTags([]);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const [rows, tags] = await Promise.all([
          api.documentList(caseId),
          api.caseTags(caseId),
        ]);
        if (!cancelled) {
          setDocs(rows);
          setCaseTags(tags);
        }
      } catch (e) {
        if (!cancelled) setError(toApiError(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [caseId]);

  // Dosya değişince önceki dosyanın okuyucu hedefleri ve sonuçları taşınmaz.
  useEffect(() => {
    setReader(null);
    setPage(null);
    setSearchedQuery("");
  }, [caseId]);

  const run = useCallback(
    async (mode: "fresh" | "more" = "fresh") => {
      if (!caseId) return;
      const asked = mode === "fresh" ? query : searchedQuery;
      if (asked.trim().length === 0) return;
      const applied = mode === "fresh" ? draftFilters : appliedFilters;
      const cursor = mode === "more" ? page?.nextCursor ?? null : null;
      if (mode === "more" && !cursor) return;

      if (mode === "fresh") {
        setBusy(true);
      } else {
        setLoadingMore(true);
      }
      setError(null);
      try {
        const next = await api.searchPage(caseId, asked, applied, {
          limit: PAGE_SIZE,
          cursor,
          includeTotal: mode === "fresh",
        });
        setAppliedFilters(next.filters);
        setSearchedQuery(asked);
        if (mode === "more" && page) {
          const merged = mergeHits(page.hits, next.hits);
          setPage({ ...next, hits: merged, returned: merged.length, total: page.total });
        } else {
          setPage(next);
        }
      } catch (e) {
        setError(toApiError(e));
      } finally {
        setBusy(false);
        setLoadingMore(false);
      }
    },
    [appliedFilters, caseId, draftFilters, page, query, searchedQuery],
  );

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
  const documentTypes = useMemo(
    () => Array.from(new Set(docs.map((doc) => doc.documentType).filter((value) => value.length > 0))).sort(),
    [docs],
  );
  const filterLines = describeFilters(appliedFilters);
  const filterCount = activeFilterCount(appliedFilters);
  const stale = page !== null && filtersStale(page, draftFilters);
  const hits = page?.hits ?? null;

  function updateFilter(patch: Partial<SearchFilters>) {
    setDraftFilters((prev) => ({ ...prev, ...patch }));
  }

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
              onKeyDown={(e) => e.key === "Enter" && void run("fresh")}
              placeholder="Örn. ödeme, itiraz, kira"
            />
          </div>
          <button
            className="btn primary"
            disabled={busy || !caseId || query.trim().length === 0}
            onClick={() => void run("fresh")}
          >
            {busy ? "Aranıyor…" : "Ara"}
          </button>
        </div>

        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
            gap: "var(--space-3)",
            marginTop: "var(--space-3)",
          }}
        >
          <div className="field">
            <label htmlFor="filter-type">Belge türü</label>
            <select
              id="filter-type"
              className="input"
              value={draftFilters.documentTypes[0] ?? ""}
              onChange={(e) => updateFilter({ documentTypes: e.target.value ? [e.target.value] : [] })}
            >
              <option value="">Tüm türler</option>
              {documentTypes.map((type) => (
                <option key={type} value={type}>{type}</option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="filter-ocr">Metin kaynağı</label>
            <select
              id="filter-ocr"
              className="input"
              value={draftFilters.ocrStatus ?? "any"}
              onChange={(e) => updateFilter({ ocrStatus: e.target.value })}
            >
              {Object.entries(OCR_STATUS_LABELS).map(([value, label]) => (
                <option key={value} value={value}>{label}</option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="filter-role">Tarih rolü (belgeye bağlı olay)</label>
            <select
              id="filter-role"
              className="input"
              value={draftFilters.dateRole ?? ""}
              onChange={(e) => updateFilter({ dateRole: e.target.value || null })}
            >
              <option value="">Tüm roller</option>
              {Object.entries(DATE_ROLE_LABELS).map(([value, label]) => (
                <option key={value} value={value}>{label}</option>
              ))}
            </select>
          </div>
        </div>

        <div style={{ marginTop: "var(--space-3)" }}>
          <span className="muted small" style={{ display: "block", marginBottom: 4 }}>
            Etiketler (seçilenlerin <strong>hepsi</strong> belgede olmalı)
          </span>
          {caseTags.length === 0 ? (
            <p className="hint" style={{ margin: 0 }}>
              Bu dosyada henüz etiket yok. Belge listesinden (Dosyalar ekranı) etiket ekleyin.
            </p>
          ) : (
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
              {caseTags.map((tag) => {
                const active = draftFilters.tags.includes(tag.tag);
                return (
                  <button
                    key={tag.tag}
                    className={active ? "btn" : "btn ghost"}
                    style={{ height: 26 }}
                    onClick={() => updateFilter({ tags: toggleValue(draftFilters.tags, tag.tag) })}
                    title={`${tag.documentCount} belgede`}
                  >
                    {tag.tag} <span className="muted small">({tag.documentCount})</span>
                  </button>
                );
              })}
            </div>
          )}
        </div>

        <div className="row" style={{ justifyContent: "space-between", alignItems: "center", marginTop: "var(--space-3)" }}>
          <p className="hint" style={{ margin: 0 }}>
            {selectedCase ? `${selectedCase.title} içinde aranıyor.` : "Önce bir dosya oluşturun."}{" "}
            Filtreler sunucuda uygulanır; sayfa başına {PAGE_SIZE} sonuç gelir.
          </p>
          <button
            className="btn ghost"
            disabled={activeFilterCount(draftFilters) === 0}
            onClick={() => setDraftFilters(EMPTY_SEARCH_FILTERS)}
          >
            Filtreleri temizle
          </button>
        </div>
      </div>

      {stale ? (
        <div className="notice" role="status">
          Filtreler değişti; aşağıdaki sonuçlar <strong>önceki</strong> filtreyle alındı.{" "}
          <button className="btn ghost" style={{ height: 26 }} onClick={() => void run("fresh")}>
            Yeni filtreyle ara
          </button>
        </div>
      ) : null}

      {page !== null ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>Sonuçlar</h2>
              <p className="card-sub">
                {resultSummary(page)} · kapsam: yalnız bu dosya
                {filterCount > 0 ? ` · ${filterCount} filtre` : ""}
              </p>
              {filterLines.length > 0 ? (
                <p className="hint" style={{ margin: 0 }}>{filterLines.join(" · ")}</p>
              ) : null}
            </div>
          </div>
          {page.hits.length === 0 ? (
            <div className="empty">
              <p style={{ margin: 0 }}>
                Eşleşme yok. {filterCount > 0 ? "Filtreleri gevşetmeyi deneyin. " : ""}
                Belgeleri “Ayrıştır” ile metne çevirdiniz mi?
              </p>
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
              {page.hits.map((hit) => (
                <div key={hit.segmentId} className="card tight" style={{ background: "var(--surface)" }}>
                  <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                    <span style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
                      <span className="badge plain">{hit.documentName ?? "belge bilinmiyor"}</span>
                      <span className="badge plain">sayfa {hit.pageIndex !== null ? hit.pageIndex + 1 : "?"}</span>
                      {hit.documentType ? <span className="badge plain">{hit.documentType}</span> : null}
                      <span className="badge plain">{textOriginLabel(hit.textOrigin)}</span>
                      {hit.documentStatus && hit.documentStatus !== "active" ? (
                        <span className="badge plain">{hit.documentStatus}</span>
                      ) : null}
                      {hit.tags.map((tag) => (
                        <span key={`${hit.segmentId}-${tag}`} className="badge plain">#{tag}</span>
                      ))}
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
                    <Highlight
                      text={hit.rawText.length > 600 ? `${hit.rawText.slice(0, 600)}…` : hit.rawText}
                      query={searchedQuery}
                    />
                  </p>
                  <span className="muted small" style={{ fontFamily: "var(--mono)", fontSize: 11 }}>
                    segment {hit.segmentId.slice(0, 8)} · sürüm {(hit.documentVersionId ?? "—").slice(0, 8)}
                  </span>
                </div>
              ))}
            </div>
          )}
          {canLoadMore(page) ? (
            <div className="row" style={{ justifyContent: "center", marginTop: "var(--space-3)" }}>
              <button className="btn" disabled={loadingMore || busy} onClick={() => void run("more")}>
                {loadingMore ? "Yükleniyor…" : "Daha fazla sonuç"}
              </button>
            </div>
          ) : page.hasMore ? (
            <p className="hint" style={{ margin: 0 }}>Devamı var ancak imleç gelmedi; yeniden arayın.</p>
          ) : page.hits.length > 0 ? (
            <p className="hint" style={{ margin: 0 }}>Tüm sonuçlar gösterildi.</p>
          ) : null}
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
