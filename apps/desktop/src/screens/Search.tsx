import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  toApiError,
  type ApiError,
  type CaseRow,
  type CaseTagRow,
  type DocumentRow,
  type PrecedentSearchHit,
  type PrecedentSearchPage,
  type SearchFilters,
  type SearchHit,
  type SearchMode,
  type SearchPage,
} from "../lib/api";
import { targetFromHit, type ReaderTarget } from "../lib/reader";
import { t } from "../lib/i18n";
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
  SEARCH_MODE_LABELS,
  searchModeHint,
  textOriginLabel,
  toggleValue,
  TOTAL_PROBE_IDLE,
  totalSummary,
  type TotalProbe,
  verificationSummary,
} from "../lib/search";
import {
  precedentCitation,
  precedentMatchLabel,
  precedentSearchSummary,
  precedentSnippetForCopy,
  scopeNotice,
  SEARCH_SCOPE_LABELS,
  type SearchScope,
} from "../lib/search-scope";

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
  /** Arama kapsamı (F12-T10): dosya belgeleri ↔ emsal kütüphanesi. */
  const [scope, setScope] = useState<SearchScope>("case_documents");
  const [query, setQuery] = useState<string>("");
  /** Seçili arama kipi (F07-T02); sunucu `auto`yu çözer. */
  const [mode, setMode] = useState<SearchMode>("auto");
  /** Son aramanın kipi; sonuçlar bu kipe ait. */
  const [searchedMode, setSearchedMode] = useState<SearchMode>("auto");
  /** Son aramanın sorgusu; kullanıcı kutuyu değiştirse de sonuç işaretlemesi
   *  ve okuyucu hedefi gerçekten aranan metne bağlı kalır. */
  const [searchedQuery, setSearchedQuery] = useState<string>("");
  /** Son aramanın filtreleri; çipte gösterilen "uygulanan" filtre budur. */
  const [appliedFilters, setAppliedFilters] = useState<SearchFilters>(EMPTY_SEARCH_FILTERS);
  const [draftFilters, setDraftFilters] = useState<SearchFilters>(EMPTY_SEARCH_FILTERS);
  const [page, setPage] = useState<SearchPage | null>(null);
  /** Kütüphane kapsamının sonucu; dosya sonuçlarıyla **aynı listede tutulmaz**. */
  const [libraryPage, setLibraryPage] = useState<PrecedentSearchPage | null>(null);
  const [copiedHit, setCopiedHit] = useState<string>("");
  const [docs, setDocs] = useState<DocumentRow[]>([]);
  const [caseTags, setCaseTags] = useState<CaseTagRow[]>([]);
  const [reader, setReader] = useState<{ left: ReaderTarget | null; right: ReaderTarget | null } | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  /** Ertelenmiş toplam sayım (F15): sonuçlar gösterildikten **sonra** istenir. */
  const [totalProbe, setTotalProbe] = useState<TotalProbe>(TOTAL_PROBE_IDLE);
  /** Sayım istek sırası: geç gelen eski yanıt yeni sayıyı ezmesin. */
  const totalSeq = useRef(0);

  /**
   * Toplam sayıyı ilk boyamadan sonra ister (F15). Ölçüm: sayım sayfa
   * sorgusuna p50'de +643 ms ekliyordu; bu yüzden sayfa onu beklemez.
   * Sayı gelmezse satır gerçek hatayı yazar, sayı uydurulmaz.
   */
  const requestTotal = useCallback(
    (targetCaseId: string, asked: string, applied: SearchFilters, askedMode: SearchMode) => {
      const seq = ++totalSeq.current;
      setTotalProbe({ status: "counting" });
      const fire = () => {
        void api
          .searchTotal(targetCaseId, asked, applied, askedMode)
          .then((result) => {
            if (seq !== totalSeq.current) return;
            setTotalProbe({
              status: "done",
              total: result.total,
              tookMs: result.tookMs,
              resolvedQuery: result.resolvedQuery,
              mode: result.mode,
            });
          })
          .catch((e) => {
            if (seq !== totalSeq.current) return;
            setTotalProbe({ status: "failed", message: toApiError(e).message });
          });
      };
      // İlk boyama tamamlanmadan sayım gönderilirse kazanç kaybolur.
      if (typeof requestAnimationFrame === "function") {
        requestAnimationFrame(() => requestAnimationFrame(fire));
      } else {
        setTimeout(fire, 0);
      }
    },
    [],
  );

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
    totalSeq.current += 1;
    setTotalProbe(TOTAL_PROBE_IDLE);
  }, [caseId]);

  // Kapsam değişince öteki kapsamın sonuçları ekranda kalmaz: her sonuç
  // kendi kapsamıyla etiketlidir, karışık liste gösterilmez.
  useEffect(() => {
    setPage(null);
    setLibraryPage(null);
    setSearchedQuery("");
    setCopiedHit("");
    totalSeq.current += 1;
    setTotalProbe(TOTAL_PROBE_IDLE);
  }, [scope]);

  const run = useCallback(
    async (kind: "fresh" | "more" = "fresh") => {
      const asked = kind === "fresh" ? query : searchedQuery;
      if (asked.trim().length === 0) return;
      const askedMode = kind === "fresh" ? mode : searchedMode;
      setError(null);
      if (scope === "precedent_library") {
        if (kind === "more") return;
        setBusy(true);
        try {
          const library = await api.precedentSearch({
            text: asked,
            mode: askedMode,
            limit: PAGE_SIZE,
          });
          setSearchedQuery(asked);
          setSearchedMode(askedMode);
          setLibraryPage(library);
        } catch (e) {
          setError(toApiError(e));
        } finally {
          setBusy(false);
        }
        return;
      }
      if (!caseId) return;
      const applied = kind === "fresh" ? draftFilters : appliedFilters;
      const cursor = kind === "more" ? page?.nextCursor ?? null : null;
      if (kind === "more" && !cursor) return;

      if (kind === "fresh") {
        setBusy(true);
      } else {
        setLoadingMore(true);
      }
      setError(null);
      try {
        const next = await api.searchPage(caseId, asked, applied, {
          limit: PAGE_SIZE,
          cursor,
          // Sayım artık sayfadan ayrı istenir (F15): ilk sonuçlar beklemez.
          includeTotal: false,
          mode: askedMode,
        });
        setAppliedFilters(next.filters);
        setSearchedQuery(asked);
        setSearchedMode(askedMode);
        if (kind === "more" && page) {
          const merged = mergeHits(page.hits, next.hits);
          setPage({ ...next, hits: merged, returned: merged.length, total: page.total });
        } else {
          setPage(next);
          requestTotal(caseId, asked, next.filters, askedMode);
        }
      } catch (e) {
        setError(toApiError(e));
        if (kind === "fresh") {
          totalSeq.current += 1;
          setTotalProbe(TOTAL_PROBE_IDLE);
        }
      } finally {
        setBusy(false);
        setLoadingMore(false);
      }
    },
    [
      appliedFilters,
      caseId,
      draftFilters,
      mode,
      page,
      query,
      requestTotal,
      scope,
      searchedMode,
      searchedQuery,
    ],
  );

  /** Sonucu seçilen panelde açar. Hedef, sonucun kendi sürümünü taşır;
   *  "son sürüm" varsayımı yapılmaz (F07 kabul testi). */
  function openHit(hit: SearchHit, side: "left" | "right") {
    const target = targetFromHit(hit, searchedQuery);
    if (!target) {
      setError({
        code: "E_SEARCH_VERSION",
        message: t("search.errorNoVersion"),
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

  /** Kütüphane sonucundan alıntı kopyalama; pano reddederse sessiz kalmaz. */
  async function copySnippet(hit: PrecedentSearchHit) {
    try {
      await navigator.clipboard.writeText(precedentSnippetForCopy(hit));
      setCopiedHit(hit.versionId);
      setError(null);
    } catch {
      setError({
        code: "E_CLIPBOARD",
        message: t("search.errorClipboard"),
        retryable: false,
      });
    }
  }

  const selectedCase = useMemo(() => cases.find((c) => c.id === caseId), [cases, caseId]);
  const documentTypes = useMemo(
    () => Array.from(new Set(docs.map((doc) => doc.documentType).filter((value) => value.length > 0))).sort(),
    [docs],
  );
  const filterLines = describeFilters(appliedFilters);
  const filterCount = activeFilterCount(appliedFilters);
  const stale = page !== null && filtersStale(page, draftFilters);
  const modeStale = page !== null && searchedMode !== mode;
  /** Ertelenmiş toplam satırı (F15); sayfa toplamı sunucudan geldiyse boş. */
  const totalLine = totalSummary(page, totalProbe);
  /** Aday penceresi satırı (F15): kaç adayın gerçekten doğrulandığı. */
  const verificationLine = verificationSummary(page);
  const hits = page?.hits ?? null;

  function updateFilter(patch: Partial<SearchFilters>) {
    setDraftFilters((prev) => ({ ...prev, ...patch }));
  }

  return (
    <div className="page" style={{ maxWidth: 1100 }}>
      <div className="page-head">
        <h1>{t("nav.search")}</h1>
        <p className="lead">
          {t("search.leadStart")} <strong>{t("search.leadEmphasis")}</strong>
          {t("search.leadRest")}
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div
        className="row"
        role="group"
        aria-label={t("search.scopeGroup")}
        style={{ gap: 6, alignItems: "center", marginBottom: "var(--space-2)" }}
      >
        {(Object.keys(SEARCH_SCOPE_LABELS) as SearchScope[]).map((value) => (
          <button
            key={value}
            className={scope === value ? "btn primary" : "btn ghost"}
            aria-pressed={scope === value}
            onClick={() => setScope(value)}
            title={scopeNotice(value)}
          >
            {SEARCH_SCOPE_LABELS[value]}
          </button>
        ))}
      </div>

      <div className="notice" role="status" data-scope={scope}>
        <strong>{SEARCH_SCOPE_LABELS[scope]}</strong> — {scopeNotice(scope)}
      </div>

      <div className="card">
        <div className="row">
          {scope === "case_documents" ? (
            <div className="field" style={{ flex: "1 1 240px" }}>
              <label htmlFor="search-case">{t("common.case")}</label>
              <select id="search-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
                {cases.length === 0 ? <option value="">{t("common.noCase")}</option> : null}
                {cases.map((c) => (
                  <option key={c.id} value={c.id}>{c.title}</option>
                ))}
              </select>
            </div>
          ) : null}
          <div className="field" style={{ flex: "2 1 320px" }}>
            <label htmlFor="search-q">{t("search.queryLabel")}</label>
            <input
              id="search-q"
              className="input"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void run("fresh")}
              placeholder={t("search.queryPlaceholder")}
            />
          </div>
          <div className="field" style={{ flex: "0 1 170px" }}>
            <label htmlFor="search-mode">{t("search.modeLabel")}</label>
            <select
              id="search-mode"
              className="input"
              value={mode}
              onChange={(e) => setMode(e.target.value as SearchMode)}
            >
              {(Object.keys(SEARCH_MODE_LABELS) as SearchMode[]).map((value) => (
                <option key={value} value={value}>{SEARCH_MODE_LABELS[value]}</option>
              ))}
            </select>
          </div>
          <button
            className="btn primary"
            disabled={busy || (scope === "case_documents" && !caseId) || query.trim().length === 0}
            onClick={() => void run("fresh")}
          >
            {busy ? t("search.searching") : t("search.submit")}
          </button>
        </div>

        <p className="hint" style={{ marginTop: "var(--space-2)", marginBottom: 0 }}>
          {searchModeHint(query, mode)}
        </p>

        {scope === "case_documents" ? (
          <>
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
            gap: "var(--space-3)",
            marginTop: "var(--space-3)",
          }}
        >
          <div className="field">
            <label htmlFor="filter-type">{t("search.filterType")}</label>
            <select
              id="filter-type"
              className="input"
              value={draftFilters.documentTypes[0] ?? ""}
              onChange={(e) => updateFilter({ documentTypes: e.target.value ? [e.target.value] : [] })}
            >
              <option value="">{t("search.filterTypeAll")}</option>
              {documentTypes.map((type) => (
                <option key={type} value={type}>{type}</option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="filter-ocr">{t("search.filterOcr")}</label>
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
            <label htmlFor="filter-role">{t("search.filterDateRole")}</label>
            <select
              id="filter-role"
              className="input"
              value={draftFilters.dateRole ?? ""}
              onChange={(e) => updateFilter({ dateRole: e.target.value || null })}
            >
              <option value="">{t("search.filterDateRoleAll")}</option>
              {Object.entries(DATE_ROLE_LABELS).map(([value, label]) => (
                <option key={value} value={value}>{label}</option>
              ))}
            </select>
          </div>
        </div>

        <div style={{ marginTop: "var(--space-3)" }}>
          <span className="muted small" style={{ display: "block", marginBottom: 4 }}>
            {t("search.tagsLabelStart")} <strong>{t("search.tagsLabelEmphasis")}</strong>{" "}
            {t("search.tagsLabelRest")}
          </span>
          {caseTags.length === 0 ? (
            <p className="hint" style={{ margin: 0 }}>
              {t("search.tagsEmpty")}
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
                    title={t("search.tagCount", { count: tag.documentCount })}
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
            {selectedCase
              ? t("search.searchingIn", { title: selectedCase.title })
              : t("search.noCaseYet")}{" "}
            {t("search.serverFilters", { size: PAGE_SIZE })}
          </p>
          <button
            className="btn ghost"
            disabled={activeFilterCount(draftFilters) === 0}
            onClick={() => setDraftFilters(EMPTY_SEARCH_FILTERS)}
          >
            {t("search.clearFilters")}
          </button>
        </div>
          </>
        ) : (
          <p className="hint" style={{ marginTop: "var(--space-2)", marginBottom: 0 }}>
            {t("search.libraryFiltersNote", { size: PAGE_SIZE })}
          </p>
        )}
      </div>

      {stale || modeStale ? (
        <div className="notice" role="status">
          {stale ? t("search.staleFilters") : t("search.staleMode")} {t("search.staleTail")}{" "}
          <strong>{t("search.staleEmphasis")}</strong> {t("search.staleRest")}{" "}
          <button className="btn ghost" style={{ height: 26 }} onClick={() => void run("fresh")}>
            {t("search.searchAgain")}
          </button>
        </div>
      ) : null}

      {page !== null ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("search.resultsTitle")}</h2>
              <p className="card-sub">
                {t("search.resultsSub", {
                  summary: resultSummary(page),
                  mode: SEARCH_MODE_LABELS[page.mode],
                })}
                {filterCount > 0 ? ` · ${t("search.filterCount", { count: filterCount })}` : ""}
              </p>
              {totalLine !== null ? (
                <p className="hint" style={{ margin: 0 }}>
                  {totalLine}
                </p>
              ) : null}
              {verificationLine !== null ? (
                <p className="hint" style={{ margin: 0 }}>{verificationLine}</p>
              ) : null}
              {filterLines.length > 0 ? (
                <p className="hint" style={{ margin: 0 }}>{filterLines.join(" · ")}</p>
              ) : null}
            </div>
          </div>
          {page.hits.length === 0 ? (
            <div className="empty">
              <p style={{ margin: 0 }}>
                {t("search.emptyNoMatch")}{" "}
                {filterCount > 0 ? `${t("search.emptyRelax")} ` : ""}
                {t("search.emptyParsed")}
              </p>
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
              {page.hits.map((hit) => (
                <div key={hit.segmentId} className="card tight" style={{ background: "var(--surface)" }}>
                  <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                    <span style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
                      <span className="badge plain">{hit.documentName ?? t("search.unknownDocument")}</span>
                      <span className="badge plain">
                        {t("search.pageBadge", { page: hit.pageIndex !== null ? hit.pageIndex + 1 : "?" })}
                      </span>
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
                        title={hit.documentVersionId ? t("search.openLeftTitle") : t("search.noVersionTitle")}
                      >
                        {t("search.openLeft")}
                      </button>
                      <button
                        className="btn ghost"
                        style={{ height: 26 }}
                        disabled={!hit.documentVersionId}
                        onClick={() => openHit(hit, "right")}
                        title={hit.documentVersionId ? t("search.openRightTitle") : t("search.noVersionTitle")}
                      >
                        {t("search.openRight")}
                      </button>
                    </span>
                  </div>
                  <p style={{ margin: 0, whiteSpace: "pre-wrap" }}>
                    <Highlight
                      text={hit.rawText.length > 600 ? `${hit.rawText.slice(0, 600)}…` : hit.rawText}
                      query={page.resolvedQuery || searchedQuery}
                    />
                  </p>
                  <span className="muted small" style={{ fontFamily: "var(--mono)", fontSize: 11 }}>
                    {t("search.hitMeta", {
                      segment: hit.segmentId.slice(0, 8),
                      version: (hit.documentVersionId ?? "—").slice(0, 8),
                    })}
                  </span>
                </div>
              ))}
            </div>
          )}
          {canLoadMore(page) ? (
            <div className="row" style={{ justifyContent: "center", marginTop: "var(--space-3)" }}>
              <button className="btn" disabled={loadingMore || busy} onClick={() => void run("more")}>
                {loadingMore ? t("search.loadingMore") : t("search.loadMore")}
              </button>
            </div>
          ) : page.hasMore ? (
            <p className="hint" style={{ margin: 0 }}>{t("search.cursorMissing")}</p>
          ) : page.hits.length > 0 ? (
            <p className="hint" style={{ margin: 0 }}>{t("search.allShown")}</p>
          ) : null}
          <p className="hint" style={{ margin: 0 }}>
            {t("search.openExplain")}
          </p>
        </div>
      ) : null}

      {scope === "precedent_library" && libraryPage !== null ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("search.libraryTitle")}</h2>
              <p className="card-sub">
                {t("search.librarySubPrefix", { summary: precedentSearchSummary(libraryPage) })}{" "}
                <strong>{t("searchScope.label.precedent_library")}</strong>{" "}
                {t("search.librarySubSuffix", { mode: SEARCH_MODE_LABELS[libraryPage.mode] })}
              </p>
              <p className="hint" style={{ margin: 0 }}>
                {t("search.libraryQuery", { query: libraryPage.resolvedQuery })}
              </p>
            </div>
          </div>
          {libraryPage.notes.length > 0 ? (
            <ul className="hint" style={{ margin: 0, paddingLeft: 18 }}>
              {libraryPage.notes.map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
          ) : null}
          {libraryPage.hits.length === 0 ? (
            <div className="empty">
              <p style={{ margin: 0 }}>
                {t("search.libraryEmpty")}
              </p>
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
              {libraryPage.hits.map((hit) => (
                <div key={hit.versionId} className="card tight" style={{ background: "var(--surface)" }}>
                  <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                    <span style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
                      <span className="badge plain">{t("search.libraryBadge")}</span>
                      <span className="badge plain">{precedentCitation(hit)}</span>
                      <span className="badge plain">
                        {t("search.matchBadge", { label: precedentMatchLabel(hit) })}
                      </span>
                      {hit.title ? <span className="badge plain">{hit.title}</span> : null}
                    </span>
                    <button
                      className="btn ghost"
                      style={{ height: 26 }}
                      onClick={() => void copySnippet(hit)}
                    >
                      {copiedHit === hit.versionId ? t("search.copied") : t("search.copyQuote")}
                    </button>
                  </div>
                  <p style={{ margin: 0, whiteSpace: "pre-wrap" }}>
                    <Highlight text={hit.snippet} query={libraryPage.resolvedQuery} />
                  </p>
                  <span className="muted small" style={{ fontFamily: "var(--mono)", fontSize: 11 }}>
                    {t("search.libraryMeta", {
                      version: hit.versionId.slice(0, 8),
                      date: hit.acquiredAt.slice(0, 10),
                    })}
                    {" · "}
                    {hit.matchStart !== null
                      ? t("search.positionLine", { position: hit.matchStart })
                      : t("search.positionMissing")}
                  </span>
                </div>
              ))}
            </div>
          )}
          <p className="hint" style={{ margin: 0 }}>
            {t("search.libraryNote")}
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
