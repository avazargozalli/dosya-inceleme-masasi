// @vitest-environment jsdom
//
// Arama (Search) ekranı davranış testi. Lib düzeyindeki testler
// (`search`, `search-scope`, `i18n-bindings`) parçaların **metnini** doğrular;
// burada ekran gerçekten render edilip ürün kararlarının **arayüzde**
// uygulandığı ölçülür. Ölçülen kararlar:
//
// 1. Arama düğmesi boş sorguda kapalıdır; Enter aramayı başlatır ve istek
//    sunucu sözleşmesine **tam** argümanlarla gider (`includeTotal: false` —
//    toplam sayı sayfayı bekletmez, F15).
// 2. İlk arama **taslak** filtrelerle koşar, sonuç ekranda **sunucunun
//    döndürdüğü** filtrelerle etiketlenir; toplam ayrı istekte ve aynı
//    filtrelerle istenir.
// 3. Sonuç varken filtre ya da kip değişirse "bayat" uyarısı çıkar ve
//    "yeni ayarlarla ara" gerçekten yeni ayarlarla koşar (eski sonuç yeni
//    filtreyle gösterilmez).
// 4. "Daha fazla sonuç" yalnız sunucunun imleciyle çağrılır; gelen sayfa
//    öncekiyle **tekilleştirilerek** birleşir.
// 5. Kapsam değişince öteki kapsamın sonuçları ekranda kalmaz; emsal
//    kütüphanesi araması ayrı yoldan gider ve sayfalanmaz.
// 6. Dosya değişince önceki dosyanın sonuçları ve aranan metin temizlenir.
// 7. Toplam sayımı başarısız olursa **sayı uydurulmaz**, gerçek hata yazılır.
// 8. Sürümü olmayan sonuç açılamaz; vurgu **aranan** metne bağlı kalır
//    (kutu değişse de sonuçlar eski sorguya aittir).

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../lib/i18n";
import { precedentSnippetForCopy, SEARCH_SCOPE_LABELS } from "../lib/search-scope";
import type {
  CaseRow,
  CaseTagRow,
  DocumentRow,
  PrecedentSearchPage,
  SearchFilters,
  SearchHit,
  SearchPage,
  SearchTotal,
} from "../lib/api";

const mocks = vi.hoisted(() => ({
  caseList: vi.fn(),
  documentList: vi.fn(),
  caseTags: vi.fn(),
  searchPage: vi.fn(),
  searchTotal: vi.fn(),
  precedentSearch: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      caseList: mocks.caseList,
      documentList: mocks.documentList,
      caseTags: mocks.caseTags,
      searchPage: mocks.searchPage,
      searchTotal: mocks.searchTotal,
      precedentSearch: mocks.precedentSearch,
    },
  };
});

const { Search } = await import("./Search");

const ISO = "2026-10-01T00:00:00Z";

const CASE: CaseRow = {
  id: "c1",
  title: "Kira uyuşmazlığı",
  caseKind: "review",
  referenceCode: null,
  jurisdictionLabel: null,
  practiceArea: null,
  status: "active",
  sensitivity: "normal",
  defaultAiPolicy: "local_only",
  archivedAt: null,
  retentionHold: false,
  createdAt: ISO,
  updatedAt: ISO,
  rowVersion: 1,
};

const CASE2: CaseRow = { ...CASE, id: "c2", title: "İhtar dosyası" };

const DOCUMENTS: DocumentRow[] = [
  {
    id: "d1",
    caseId: "c1",
    displayName: "Kira sözleşmesi",
    documentType: "pdf",
    status: "parsed",
    currentVersionId: "v1",
    declaredDocumentDate: null,
    createdAt: ISO,
    updatedAt: ISO,
    rowVersion: 1,
  },
];

const TAGS: CaseTagRow[] = [{ tag: "kira", documentCount: 1 }];

/** Boş filtre (taslak): `ocrStatus` arayüzde "any", sunucuda `null`. */
const DRAFT_EMPTY: SearchFilters = {
  documentTypes: [],
  dateRole: null,
  tags: [],
  ocrStatus: "any",
};

/** Sunucunun döndürdüğü kanonik boş filtre. */
const SERVER_EMPTY: SearchFilters = {
  documentTypes: [],
  dateRole: null,
  tags: [],
  ocrStatus: null,
};

const SERVER_PDF: SearchFilters = { ...SERVER_EMPTY, documentTypes: ["pdf"] };

function hit(overrides: Partial<SearchHit> & { segmentId: string }): SearchHit {
  return {
    pageId: "p1",
    documentVersionId: "v1",
    documentId: "d1",
    documentName: "Kira sözleşmesi",
    documentType: "pdf",
    documentStatus: "active",
    textOrigin: "native",
    caseId: "c1",
    tags: [],
    rawText: "kira bedeli ödenmedi",
    pageIndex: 0,
    rank: 1,
    ...overrides,
  };
}

function page(overrides: Partial<SearchPage> = {}): SearchPage {
  return {
    caseId: "c1",
    query: "kira",
    resolvedQuery: "kira",
    mode: "flexible",
    scope: "case",
    limit: 25,
    returned: 1,
    hasMore: false,
    nextCursor: null,
    total: null,
    filters: { ...SERVER_EMPTY },
    hits: [hit({ segmentId: "aaaaaaaa-1" })],
    ...overrides,
  };
}

function totalLine(overrides: Partial<SearchTotal> = {}): SearchTotal {
  return {
    caseId: "c1",
    query: "kira",
    resolvedQuery: "kira",
    mode: "flexible",
    scope: "case",
    total: 1,
    tookMs: 3,
    filters: { ...SERVER_EMPTY },
    ...overrides,
  };
}

function libraryPage(overrides: Partial<PrecedentSearchPage> = {}): PrecedentSearchPage {
  return {
    scope: "precedent_library",
    mode: "flexible",
    query: "kira",
    resolvedQuery: "kira",
    hits: [
      {
        precedentId: "pr1",
        court: "Yargıtay",
        chamber: "3. Hukuk Dairesi",
        docketNo: "2020/123",
        decisionNo: "2021/456",
        decisionDate: "2021-03-01",
        title: "Kira alacağı",
        versionId: "pv-12345678",
        acquiredAt: "2026-01-02T00:00:00Z",
        sourceKind: "manual",
        snippet: "kira bedeli ödenmediğinde",
        snippetStart: 0,
        matchStart: 0,
        matchLength: 4,
        matchSource: "full_text",
      },
    ],
    scanned: 10,
    matched: 1,
    withoutText: 0,
    limit: 25,
    truncated: false,
    notes: [],
    ...overrides,
  };
}

type SetupOptions = {
  pages?: SearchPage[];
  total?: SearchTotal;
  totalError?: unknown;
  library?: PrecedentSearchPage;
};

function setup(options: SetupOptions = {}) {
  // `mockResolvedValueOnce` kuyrukları testler arasında sızmasın: her kurulum
  // kendi sözleşmesini sıfırdan kurar (yarış testleri çağrı sayısına duyarlı).
  for (const mock of Object.values(mocks)) mock.mockReset();
  const pages = options.pages ?? [page()];
  mocks.caseList.mockResolvedValue([CASE, CASE2]);
  mocks.documentList.mockResolvedValue(DOCUMENTS);
  mocks.caseTags.mockResolvedValue(TAGS);
  for (const next of pages) mocks.searchPage.mockResolvedValueOnce(next);
  mocks.searchPage.mockResolvedValue(pages[pages.length - 1]!);
  if (options.totalError !== undefined) {
    mocks.searchTotal.mockRejectedValue(options.totalError);
  } else {
    mocks.searchTotal.mockResolvedValue(options.total ?? totalLine());
  }
  mocks.precedentSearch.mockResolvedValue(options.library ?? libraryPage());
  render(<Search />);
}

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) throw new Error(`#${id} yok`);
  return element as T;
}

/** Sorguyu yazar ve aramayı başlatır (düğme ya da Enter). */
async function searchFor(query: string, how: "click" | "enter" = "click") {
  fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: query } });
  const submit = await screen.findByRole("button", { name: t("search.submit") });
  if (how === "enter") {
    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
  } else {
    fireEvent.click(submit);
  }
}

/**
 * Sonuç kartının üstündeki bayatlık uyarısı. Kapsam bildirimi de `role=status`
 * ve `data-scope` taşıdığı için onu ayıklarız; metin ayrımı kırılmış düğümlerle
 * değil, uyarının **hangi** cümleyle başladığıyla yapılır.
 */
function staleKind(): "filters" | "mode" | null {
  const notice = screen
    .queryAllByRole("status")
    .find((element) => !element.hasAttribute("data-scope"));
  if (!notice) return null;
  const text = notice.textContent ?? "";
  if (text.startsWith(t("search.staleFilters"))) return "filters";
  if (text.startsWith(t("search.staleMode"))) return "mode";
  return null;
}

describe("arama ekranı davranışı", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Toplam sayımı ilk boyamadan sonra istenir; testte iki rAF de hemen koşsun
    // ki istek sayısı testler arasında sızmasın.
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      callback(0);
      return 0;
    });
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("boş sorguda arama kapalıdır; Enter isteği tam sözleşmeyle gönderir", async () => {
    setup();
    const submit = (await screen.findByRole("button", {
      name: t("search.submit"),
    })) as HTMLButtonElement;
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    expect(submit.disabled).toBe(true);

    fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: "kira" } });
    expect(submit.disabled).toBe(false);

    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));
    // Toplam sayı sayfadan ayrı istenir (F15): sayfa onu beklemez.
    expect(mocks.searchPage).toHaveBeenCalledWith("c1", "kira", DRAFT_EMPTY, {
      limit: 25,
      cursor: null,
      includeTotal: false,
      mode: "auto",
    });
  });

  it("ilk arama taslak filtreyle koşar, sonuç sunucunun filtreleriyle etiketlenir ve toplam aynı filtreyle istenir", async () => {
    setup({
      pages: [page({ filters: { ...SERVER_PDF } })],
      total: totalLine({ filters: { ...SERVER_PDF } }),
    });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));

    fireEvent.change(byId<HTMLSelectElement>("filter-type"), { target: { value: "pdf" } });
    await searchFor("kira");

    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));
    expect(mocks.searchPage).toHaveBeenCalledWith(
      "c1",
      "kira",
      { documentTypes: ["pdf"], dateRole: null, tags: [], ocrStatus: "any" },
      { limit: 25, cursor: null, includeTotal: false, mode: "auto" },
    );

    // Toplam, **sunucunun** döndürdüğü filtrelerle istenir (taslak değil).
    await waitFor(() => expect(mocks.searchTotal).toHaveBeenCalledTimes(1));
    expect(mocks.searchTotal).toHaveBeenCalledWith("c1", "kira", { ...SERVER_PDF }, "auto");
    // Filtre sayısı sunucunun filtresinden: 1 etkin filtre.
    expect(
      await screen.findByText(t("search.filterCount", { count: 1 }), { exact: false }),
    ).toBeTruthy();
  });

  it("filtre/kip değişince bayat uyarısı çıkar ve yeniden arama yeni ayarlarla koşar", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));

    // Sonuç, kendi filtresiyle etiketli: bayat uyarısı yok.
    expect(staleKind()).toBeNull();

    mocks.searchPage.mockResolvedValue(page({ filters: { ...SERVER_PDF } }));
    fireEvent.change(byId<HTMLSelectElement>("filter-type"), { target: { value: "pdf" } });
    await waitFor(() => expect(staleKind()).toBe("filters"));

    fireEvent.click(screen.getByRole("button", { name: t("search.searchAgain") }));
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(2));
    expect(mocks.searchPage).toHaveBeenLastCalledWith(
      "c1",
      "kira",
      { documentTypes: ["pdf"], dateRole: null, tags: [], ocrStatus: "any" },
      { limit: 25, cursor: null, includeTotal: false, mode: "auto" },
    );
    await waitFor(() => expect(staleKind()).toBeNull());

    // Kip değişimi ayrı bayatlık: eski kipin sonucu yeni kip sanılmasın.
    fireEvent.change(byId<HTMLSelectElement>("search-mode"), { target: { value: "exact" } });
    await waitFor(() => expect(staleKind()).toBe("mode"));
    expect(mocks.searchPage).toHaveBeenCalledTimes(2);
  });

  it("daha fazla sonuç imleçle istenir ve gelen sayfa tekilleştirilerek birleşir", async () => {
    setup({
      pages: [
        page({ hasMore: true, nextCursor: "cur-1", returned: 1 }),
        page({
          hits: [hit({ segmentId: "aaaaaaaa-1" }), hit({ segmentId: "bbbbbbbb-2" })],
          returned: 2,
          hasMore: false,
          nextCursor: null,
        }),
      ],
    });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");

    const more = await screen.findByRole("button", { name: t("search.loadMore") });
    fireEvent.click(more);

    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(2));
    expect(mocks.searchPage).toHaveBeenLastCalledWith("c1", "kira", { ...SERVER_EMPTY }, {
      limit: 25,
      cursor: "cur-1",
      includeTotal: false,
      mode: "auto",
    });

    // Aynı segment iki kez görünmez: birleşik sayfa 2 sonuç.
    expect(
      await screen.findByText(t("search.hitMeta", { segment: "bbbbbbbb", version: "v1" })),
    ).toBeTruthy();
    expect(
      screen.getAllByText(t("search.hitMeta", { segment: "aaaaaaaa", version: "v1" })),
    ).toHaveLength(1);
    expect(screen.queryByRole("button", { name: t("search.loadMore") })).toBeNull();
    expect(screen.getByText(t("search.allShown"))).toBeTruthy();
  });

  it("kapsam değişince önceki sonuçlar silinir; kütüphane araması ayrı yoldan gider", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");
    expect(await screen.findByText(t("search.resultsTitle"))).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: SEARCH_SCOPE_LABELS.precedent_library }));
    // Öteki kapsamın sonuçları ekranda kalmaz.
    expect(screen.queryByText(t("search.resultsTitle"))).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: t("search.submit") }));
    await waitFor(() => expect(mocks.precedentSearch).toHaveBeenCalledTimes(1));
    expect(mocks.precedentSearch).toHaveBeenCalledWith({ text: "kira", mode: "auto", limit: 25 });

    expect(await screen.findByText(t("search.libraryTitle"))).toBeTruthy();
    expect(screen.getByRole("button", { name: t("search.copyQuote") })).toBeTruthy();
    // Kütüphane sonucu sayfalanmaz: "daha fazla" düğmesi yok.
    expect(screen.queryByRole("button", { name: t("search.loadMore") })).toBeNull();
    // Dosya araması ikinci kez koşmadı.
    expect(mocks.searchPage).toHaveBeenCalledTimes(1);
  });

  it("dosya değişince önceki dosyanın sonuçları ve aranan metin temizlenir", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");
    expect(await screen.findByText(t("search.resultsTitle"))).toBeTruthy();
    await waitFor(() => expect(mocks.searchTotal).toHaveBeenCalledTimes(1));

    fireEvent.change(byId<HTMLSelectElement>("search-case"), { target: { value: "c2" } });
    await waitFor(() => expect(screen.queryByText(t("search.resultsTitle"))).toBeNull());
    // Dosya değişimi sayımı tetiklemez: eski sayı yeni dosyaya taşınmaz.
    expect(mocks.searchTotal).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: t("search.submit") }));
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(2));
    expect(mocks.searchPage).toHaveBeenLastCalledWith(
      "c2",
      "kira",
      { documentTypes: [], dateRole: null, tags: [], ocrStatus: "any" },
      { limit: 25, cursor: null, includeTotal: false, mode: "auto" },
    );
  });

  it("toplam sayımı başarısız olursa sayı uydurulmaz, gerçek hata yazılır", async () => {
    setup({ totalError: { code: "E_DB", message: "sayım hatası", retryable: true } });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");

    expect(
      await screen.findByText(t("searchFilters.total.failed", { message: "sayım hatası" })),
    ).toBeTruthy();
    // Sayı satırı yazılmadı: başarısız sayım toplam gibi gösterilmez.
    expect(screen.queryByText(t("searchFilters.total.counting"))).toBeNull();
  });

  it("sürümü olmayan sonuç açılamaz; vurgu aranan metne bağlı kalır", async () => {
    setup({
      pages: [
        page({
          hits: [
            hit({ segmentId: "aaaaaaaa-1", documentVersionId: null, rawText: "kira bedeli" }),
            hit({ segmentId: "bbbbbbbb-2", rawText: "kira" }),
          ],
          returned: 2,
        }),
      ],
    });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");

    const openLeft = await screen.findAllByRole("button", { name: t("search.openLeft") });
    expect(openLeft).toHaveLength(2);
    expect((openLeft[0] as HTMLButtonElement).disabled).toBe(true);
    expect((openLeft[1] as HTMLButtonElement).disabled).toBe(false);

    // Sonuç vurgusu **aranan** metne bağlı: kutu değişse de eski sonuç eski sorguya ait.
    expect(document.querySelector("mark")?.textContent).toBe("kira");
    fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: "ödeme" } });
    expect(document.querySelector("mark")?.textContent).toBe("kira");
    expect(mocks.searchPage).toHaveBeenCalledTimes(1);
  });

  it("başarısız arama önceki sorgunun sonuçlarını yeni sorgunun sonucu gibi göstermez", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");
    expect(await screen.findByText(t("search.resultsTitle"))).toBeTruthy();

    mocks.searchPage.mockRejectedValue({ code: "E_SEARCH", message: "arama hatası", retryable: true });
    await searchFor("ödeme");

    expect(await screen.findByRole("alert")).toBeTruthy();
    // Yeni sorgu başarısız oldu: önceki sorgunun sonuçları ekranda kalmaz.
    expect(screen.queryByText(t("search.resultsTitle"))).toBeNull();
  });

  it("başarısız kütüphane araması önceki kütüphane sonucunu bırakmaz", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    fireEvent.click(screen.getByRole("button", { name: SEARCH_SCOPE_LABELS.precedent_library }));
    await searchFor("kira");
    expect(await screen.findByText(t("search.libraryTitle"))).toBeTruthy();

    mocks.precedentSearch.mockRejectedValue({
      code: "E_SEARCH",
      message: "kütüphane hatası",
      retryable: true,
    });
    await searchFor("ödeme");

    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.queryByText(t("search.libraryTitle"))).toBeNull();
  });
});

/**
 * Uçuştaki aramalar: ekranın bugüne dek hiç ölçülmemiş yüzü.
 *
 * `Search` beş api çağrısı yapar ve hepsi **asenkron**; mevcut testler yalnız
 * "istek gitti / sonuç geldi" sırasını ölçüyordu. Burada yanıtların **hangi
 * aramaya** yazıldığı ölçülür: geç gelen yanıt, kullanıcı yeni bir arama
 * başlattıysa ya da dosya/kapsam değiştiyse ekrana yazılmamalıdır.
 */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("arama ekranında uçuş yarışları", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      callback(0);
      return 0;
    });
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("geç gelen eski arama yanıtı yeni aramanın sonuçlarını ezmez", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));

    const first = deferred<SearchPage>();
    const second = deferred<SearchPage>();
    mocks.searchPage.mockReset();
    mocks.searchPage.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    await searchFor("kira");
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));

    // Düğme uçuşta kapalı ama **Enter** hâlâ aramayı başlatır: ikinci arama
    // birincisi bitmeden uçuşa girer.
    fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: "ödeme" } });
    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(2));

    const secondMeta = t("search.hitMeta", { segment: "bbbbbbbb", version: "v1" });
    await act(async () => {
      second.resolve(
        page({
          query: "ödeme",
          resolvedQuery: "ödeme",
          hits: [hit({ segmentId: "bbbbbbbb-2", rawText: "ödeme yapılmadı" })],
        }),
      );
    });
    expect(await screen.findByText(secondMeta)).toBeTruthy();

    // Eski aramanın yanıtı **sonra** gelir: yeni sonucun üstüne yazılmamalı.
    await act(async () => {
      first.resolve(page({ hits: [hit({ segmentId: "aaaaaaaa-1", rawText: "kira bedeli ödenmedi" })] }));
    });
    expect(screen.queryByText(t("search.hitMeta", { segment: "aaaaaaaa", version: "v1" }))).toBeNull();
    expect(screen.getByText(secondMeta)).toBeTruthy();
  });

  it("eski yanıt gelince kilit erken açılmaz; düğme ikinci arama bitene kadar kapalı", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));

    const first = deferred<SearchPage>();
    const second = deferred<SearchPage>();
    mocks.searchPage.mockReset();
    mocks.searchPage.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    await searchFor("kira");
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));
    fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: "ödeme" } });
    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(2));

    await act(async () => {
      first.resolve(page({ hits: [hit({ segmentId: "aaaaaaaa-1" })] }));
    });
    // İkinci arama hâlâ uçuşta: düğme "Aranıyor…" yazmalı ve kapalı kalmalı.
    const busyButton = screen.getByRole("button", { name: t("search.searching") }) as HTMLButtonElement;
    expect(busyButton.disabled).toBe(true);

    await act(async () => {
      second.resolve(page({ hits: [hit({ segmentId: "bbbbbbbb-2" })] }));
    });
    const ready = (await screen.findByRole("button", { name: t("search.submit") })) as HTMLButtonElement;
    expect(ready.disabled).toBe(false);
  });

  it("arama uçuşta iken dosya değişirse eski dosyanın sonucu yeni dosyada çizilmez", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));

    const pending = deferred<SearchPage>();
    mocks.searchPage.mockReset();
    mocks.searchPage.mockReturnValueOnce(pending.promise);

    await searchFor("kira");
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));

    fireEvent.change(byId<HTMLSelectElement>("search-case"), { target: { value: "c2" } });
    await act(async () => {
      pending.resolve(page());
    });

    // Sonuç `c1`e ait; `c2` seçiliyken sonuç kartı çizilmemeli.
    expect(screen.queryByText(t("search.resultsTitle"))).toBeNull();
    expect(screen.queryByText(t("search.hitMeta", { segment: "aaaaaaaa", version: "v1" }))).toBeNull();
  });

  it("arama uçuşta iken kapsam değişirse sonuç kartı öteki kapsamda çizilmez", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));

    const pending = deferred<SearchPage>();
    mocks.searchPage.mockReset();
    mocks.searchPage.mockReturnValueOnce(pending.promise);

    await searchFor("kira");
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole("button", { name: SEARCH_SCOPE_LABELS.precedent_library }));
    await act(async () => {
      pending.resolve(page());
    });

    // Kapsam değişti: dosya sonuçları kütüphane kapsamında görünmemeli.
    expect(screen.queryByText(t("search.resultsTitle"))).toBeNull();
    expect(screen.queryByText(t("search.libraryTitle"))).toBeNull();
  });

  it("düşen belge listesi önceki dosyanın tür ve etiketlerini bırakmaz", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    // `c1`in belge türü (pdf) filtre seçeneğine, etiketi (kira) çipe düştü.
    expect(byId<HTMLSelectElement>("filter-type").querySelectorAll("option")).toHaveLength(2);
    expect(screen.getByRole("button", { name: /kira/ })).toBeTruthy();

    mocks.documentList.mockRejectedValueOnce({
      code: "E_DB",
      message: "belge hatası",
      retryable: true,
    });
    fireEvent.change(byId<HTMLSelectElement>("search-case"), { target: { value: "c2" } });
    expect(await screen.findByRole("alert")).toBeTruthy();

    // Yeni dosyanın belgeleri okunamadı: önceki dosyanın türü/etiketi kalıntı olarak kalmaz.
    expect(byId<HTMLSelectElement>("filter-type").querySelectorAll("option")).toHaveLength(1);
    expect(screen.queryByRole("button", { name: /kira/ })).toBeNull();
    expect(screen.getByText(t("search.tagsEmpty"))).toBeTruthy();
  });

  it("\"daha fazla\" uçuşta iken dosya değişirse eski sayfa yeni dosyaya yazılmaz", async () => {
    setup({ pages: [page({ hasMore: true, nextCursor: "cur-1" })] });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");

    const more = await screen.findByRole("button", { name: t("search.loadMore") });
    await waitFor(() => expect((more as HTMLButtonElement).disabled).toBe(false));
    const pending = deferred<SearchPage>();
    mocks.searchPage.mockReset();
    mocks.searchPage.mockReturnValueOnce(pending.promise);
    fireEvent.click(more);
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));
    expect(mocks.searchPage).toHaveBeenLastCalledWith("c1", "kira", { ...SERVER_EMPTY }, {
      limit: 25,
      cursor: "cur-1",
      includeTotal: false,
      mode: "auto",
    });

    fireEvent.change(byId<HTMLSelectElement>("search-case"), { target: { value: "c2" } });
    await act(async () => {
      pending.resolve(page({ hits: [hit({ segmentId: "cccccccc-3" })], hasMore: false, nextCursor: null }));
    });

    expect(screen.queryByText(t("search.resultsTitle"))).toBeNull();
    expect(screen.queryByText(t("search.hitMeta", { segment: "cccccccc", version: "v1" }))).toBeNull();
  });

  it("uçuştaki aramanın hatası dosya değiştikten sonra yeni dosyaya yazılmaz", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));

    const pending = deferred<SearchPage>();
    mocks.searchPage.mockReset();
    mocks.searchPage.mockReturnValueOnce(pending.promise);

    await searchFor("kira");
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));

    fireEvent.change(byId<HTMLSelectElement>("search-case"), { target: { value: "c2" } });
    await act(async () => {
      pending.reject({ code: "E_SEARCH", message: "arama hatası", retryable: true });
    });

    // Eski dosyanın araması düştü: yeni dosyanın ekranında hata olarak görünmemeli.
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("uçuşta kapsam değişince eski kütüphane aramasının hatası yeni kapsama yazılmaz", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    fireEvent.click(screen.getByRole("button", { name: SEARCH_SCOPE_LABELS.precedent_library }));

    const pending = deferred<PrecedentSearchPage>();
    mocks.precedentSearch.mockReset();
    mocks.precedentSearch.mockReturnValueOnce(pending.promise);

    await searchFor("kira");
    await waitFor(() => expect(mocks.precedentSearch).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole("button", { name: SEARCH_SCOPE_LABELS.case_documents }));
    await act(async () => {
      pending.reject({ code: "E_SEARCH", message: "kütüphane hatası", retryable: true });
    });

    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByText(t("search.libraryTitle"))).toBeNull();
  });

  it("geç gelen eski kütüphane yanıtı yeni kütüphane aramasını ezmez", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    fireEvent.click(screen.getByRole("button", { name: SEARCH_SCOPE_LABELS.precedent_library }));

    const first = deferred<PrecedentSearchPage>();
    const second = deferred<PrecedentSearchPage>();
    mocks.precedentSearch.mockReset();
    mocks.precedentSearch.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    await searchFor("kira");
    await waitFor(() => expect(mocks.precedentSearch).toHaveBeenCalledTimes(1));
    fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: "ödeme" } });
    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
    await waitFor(() => expect(mocks.precedentSearch).toHaveBeenCalledTimes(2));

    await act(async () => {
      second.resolve(libraryPage({ query: "ödeme", resolvedQuery: "ödeme" }));
    });
    expect(await screen.findByText(t("search.libraryQuery", { query: "ödeme" }))).toBeTruthy();

    await act(async () => {
      first.resolve(libraryPage({ query: "kira", resolvedQuery: "kira" }));
    });
    // Eski kütüphane yanıtı yeni sorgunun satırını ezmemeli.
    expect(screen.queryByText(t("search.libraryQuery", { query: "kira" }))).toBeNull();
    expect(screen.getByText(t("search.libraryQuery", { query: "ödeme" }))).toBeTruthy();
  });

  it("kütüphane aramasında eski yanıt gelince kilit erken açılmaz", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    fireEvent.click(screen.getByRole("button", { name: SEARCH_SCOPE_LABELS.precedent_library }));

    const first = deferred<PrecedentSearchPage>();
    const second = deferred<PrecedentSearchPage>();
    mocks.precedentSearch.mockReset();
    mocks.precedentSearch.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    await searchFor("kira");
    await waitFor(() => expect(mocks.precedentSearch).toHaveBeenCalledTimes(1));
    fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: "ödeme" } });
    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
    await waitFor(() => expect(mocks.precedentSearch).toHaveBeenCalledTimes(2));

    await act(async () => {
      first.resolve(libraryPage());
    });
    // İkinci kütüphane araması hâlâ uçuşta: düğme kapalı kalmalı.
    const busyButton = screen.getByRole("button", { name: t("search.searching") }) as HTMLButtonElement;
    expect(busyButton.disabled).toBe(true);

    await act(async () => {
      second.resolve(libraryPage({ query: "ödeme", resolvedQuery: "ödeme" }));
    });
    const ready = (await screen.findByRole("button", { name: t("search.submit") })) as HTMLButtonElement;
    expect(ready.disabled).toBe(false);
  });
});

/**
 * Emsal kütüphanesi alıntısını kopyalama: panelin bugüne dek **hiç** ölçülmemiş
 * yüzü.
 *
 * `copySnippet` panoya künye + pencere + sürüm yazar ve düğmeyi "Kopyalandı"
 * yapar. Lib testleri (`search-scope`, `i18n-bindings`) yalnız **metni**
 * doğruluyordu; burada ekranın kararları ölçülür: yazılan metin sonucun kendisi
 * mi, etiket **hangi** karta konuyor, pano reddederse ne oluyor, yeni arama eski
 * etiketi taşıyor mu, uçuştaki kopya yeni sonuç kümesine yazılıyor mu.
 */
describe("emsal alıntısını kopyalama", () => {
  const clipboard = { writeText: vi.fn() };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      callback(0);
      return 0;
    });
    clipboard.writeText.mockReset();
    // jsdom `navigator.clipboard` sağlamaz: pano sözleşmesi testten kurulur.
    Object.defineProperty(navigator, "clipboard", { value: clipboard, configurable: true });
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    Reflect.deleteProperty(navigator, "clipboard");
  });

  /** İki sonuçlu kütüphane sayfası: etiketin **hangi** karta konduğu ölçülebilsin. */
  function twoHits(): PrecedentSearchPage {
    const base = libraryPage();
    const first = base.hits[0]!;
    return {
      ...base,
      hits: [
        first,
        {
          ...first,
          precedentId: "pr2",
          versionId: "pv-87654321",
          title: "Ödeme emsali",
          snippet: "ödeme yapılmadığında temerrüt",
          matchStart: 0,
          matchLength: 6,
        },
      ],
      matched: 2,
    };
  }

  /** Kütüphane kapsamına geçer, arar ve sonuç kartlarının kopyala düğmelerini verir. */
  async function openLibrary(page: PrecedentSearchPage = twoHits()): Promise<HTMLElement[]> {
    setup({ library: page });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    fireEvent.click(screen.getByRole("button", { name: SEARCH_SCOPE_LABELS.precedent_library }));
    await searchFor("kira");
    return screen.findAllByRole("button", { name: t("search.copyQuote") });
  }

  /** Kartın başlık rozetinden hangi sonuca ait olduğunu okur. */
  function cardTitle(button: HTMLElement): string | null | undefined {
    return button.closest(".card")?.textContent?.includes("Ödeme emsali") ? "Ödeme emsali" : "Kira alacağı";
  }

  it("panoya sonucun tam alıntısını yazar ve etiketi yalnız o sonuca koyar", async () => {
    const hits = await openLibrary();
    expect(hits).toHaveLength(2);
    clipboard.writeText.mockResolvedValue(undefined);

    await act(async () => {
      fireEvent.click(hits[0]!);
    });

    const page = twoHits();
    expect(clipboard.writeText).toHaveBeenCalledTimes(1);
    expect(clipboard.writeText).toHaveBeenCalledWith(precedentSnippetForCopy(page.hits[0]!));
    const copied = await screen.findByRole("button", { name: t("search.copied") });
    expect(cardTitle(copied)).toBe("Kira alacağı");
    // Öteki sonucun düğmesi hâlâ kopyalanabilir durumda.
    expect(screen.getAllByRole("button", { name: t("search.copyQuote") })).toHaveLength(1);
  });

  it("pano reddederse hata görünür ve Kopyalandı yazılmaz", async () => {
    const hits = await openLibrary();
    clipboard.writeText.mockRejectedValue(new Error("pano izni yok"));

    await act(async () => {
      fireEvent.click(hits[0]!);
    });

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_CLIPBOARD");
    expect(alert.textContent).toContain(t("search.errorClipboard"));
    expect(screen.queryByRole("button", { name: t("search.copied") })).toBeNull();
    expect(screen.getAllByRole("button", { name: t("search.copyQuote") })).toHaveLength(2);
  });

  it("düşen kopyadan sonra başarılı kopya bayat hatayı siler", async () => {
    const hits = await openLibrary();
    clipboard.writeText.mockRejectedValueOnce(new Error("pano izni yok"));
    await act(async () => {
      fireEvent.click(hits[0]!);
    });
    expect(await screen.findByRole("alert")).toBeTruthy();

    clipboard.writeText.mockResolvedValue(undefined);
    await act(async () => {
      fireEvent.click(hits[1]!);
    });

    expect(screen.queryByRole("alert")).toBeNull();
    const copied = await screen.findByRole("button", { name: t("search.copied") });
    expect(cardTitle(copied)).toBe("Ödeme emsali");
  });

  it("yeni kütüphane araması eski Kopyalandı etiketini bırakmaz", async () => {
    const hits = await openLibrary();
    clipboard.writeText.mockResolvedValue(undefined);
    await act(async () => {
      fireEvent.click(hits[0]!);
    });
    expect(await screen.findByRole("button", { name: t("search.copied") })).toBeTruthy();

    // Aynı sürüm yeni aramada da döner: etiket **bu** aramanın kopyası değildir.
    mocks.precedentSearch.mockResolvedValue({
      ...twoHits(),
      query: "ödeme",
      resolvedQuery: "ödeme",
    });
    fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: "ödeme" } });
    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
    await waitFor(() => expect(mocks.precedentSearch).toHaveBeenCalledTimes(2));
    expect(await screen.findByText(t("search.libraryQuery", { query: "ödeme" }))).toBeTruthy();

    expect(screen.queryByRole("button", { name: t("search.copied") })).toBeNull();
    expect(screen.getAllByRole("button", { name: t("search.copyQuote") })).toHaveLength(2);
  });

  it("kopya uçuşta iken yeni arama yapılırsa geç yanıt yeni sonuca yazılmaz", async () => {
    const pending = deferred<void>();
    const hits = await openLibrary();
    clipboard.writeText.mockReturnValue(pending.promise);
    await act(async () => {
      fireEvent.click(hits[0]!);
    });

    mocks.precedentSearch.mockResolvedValue({
      ...twoHits(),
      query: "ödeme",
      resolvedQuery: "ödeme",
    });
    fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: "ödeme" } });
    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
    await waitFor(() => expect(mocks.precedentSearch).toHaveBeenCalledTimes(2));
    expect(await screen.findByText(t("search.libraryQuery", { query: "ödeme" }))).toBeTruthy();

    await act(async () => {
      pending.resolve();
    });
    expect(screen.queryByRole("button", { name: t("search.copied") })).toBeNull();
  });

  it("iki sonuca hızlı kopyada son tıklanan etiket olur; geç çözülen ilk kopya onu ezmez", async () => {
    const first = deferred<void>();
    const second = deferred<void>();
    const hits = await openLibrary();
    clipboard.writeText.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    await act(async () => {
      fireEvent.click(hits[0]!);
    });
    await act(async () => {
      fireEvent.click(hits[1]!);
    });

    // İkinci (son) kopya önce döner, birinci sonra: pano ikinci metni taşır.
    await act(async () => {
      second.resolve();
    });
    await act(async () => {
      first.resolve();
    });

    const copied = await screen.findByRole("button", { name: t("search.copied") });
    expect(cardTitle(copied)).toBe("Ödeme emsali");
    expect(screen.getAllByRole("button", { name: t("search.copied") })).toHaveLength(1);
  });

  it("düşen kopya önceki Kopyalandı satırını bırakmaz", async () => {
    const hits = await openLibrary();
    clipboard.writeText.mockResolvedValueOnce(undefined);
    await act(async () => {
      fireEvent.click(hits[0]!);
    });
    expect(await screen.findByRole("button", { name: t("search.copied") })).toBeTruthy();

    clipboard.writeText.mockRejectedValueOnce(new Error("pano izni yok"));
    await act(async () => {
      fireEvent.click(hits[1]!);
    });

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_CLIPBOARD");
    // Yeşil etiket "bu deneme kopyalandı" diye okunmamalı.
    expect(screen.queryByRole("button", { name: t("search.copied") })).toBeNull();
  });

  it("kopya uçuşta iken yeni arama yapılırsa geç gelen kopya hatası yeni sonuca yazılmaz", async () => {
    const pending = deferred<void>();
    const hits = await openLibrary();
    clipboard.writeText.mockReturnValue(pending.promise);
    await act(async () => {
      fireEvent.click(hits[0]!);
    });

    mocks.precedentSearch.mockResolvedValue({
      ...twoHits(),
      query: "ödeme",
      resolvedQuery: "ödeme",
    });
    fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: "ödeme" } });
    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
    await waitFor(() => expect(mocks.precedentSearch).toHaveBeenCalledTimes(2));
    expect(await screen.findByText(t("search.libraryQuery", { query: "ödeme" }))).toBeTruthy();

    // Kopya **başarısız** döner ama ait olduğu sonuç kümesi artık ekranda değil.
    await act(async () => {
      pending.reject(new Error("pano izni yok"));
    });
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("kopya uçuşta iken kapsam değişirse geç kopya hatası öteki kapsama yazılmaz", async () => {
    const pending = deferred<void>();
    const hits = await openLibrary();
    clipboard.writeText.mockReturnValue(pending.promise);
    await act(async () => {
      fireEvent.click(hits[0]!);
    });

    fireEvent.click(screen.getByRole("button", { name: SEARCH_SCOPE_LABELS.case_documents }));
    expect(screen.queryByText(t("search.libraryTitle"))).toBeNull();

    await act(async () => {
      pending.reject(new Error("pano izni yok"));
    });
    // Dosya kapsamında kopyalama yapılmadı: clipboard hatası orada görünmemeli.
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("kopya sürerken düşen arama hatası kopya başarısıyla silinmez", async () => {
    const pending = deferred<void>();
    const hits = await openLibrary();
    clipboard.writeText.mockReturnValue(pending.promise);
    await act(async () => {
      fireEvent.click(hits[0]!);
    });

    mocks.precedentSearch.mockRejectedValueOnce({
      code: "E_DB",
      message: "kütüphane hatası",
      retryable: true,
    });
    fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: "ödeme" } });
    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("kütüphane hatası");

    // Kopya, aramadan **sonra** döner: aramanın hatasını silmemeli.
    await act(async () => {
      pending.resolve();
    });
    const after = screen.getByRole("alert");
    expect(after.textContent).toContain("kütüphane hatası");
  });
});

/**
 * Ertelenmiş toplam sayımı (F15): sayfa onu beklemez, sayım iki çerçeve
 * sonrasına zamanlanır. Bu describe, çerçeve kuyruğunu testin eline verir —
 * böylece **gönderim anı** ölçülür: geçersizleşen sayım yalnız yanıtta değil,
 * çekirdeğe giden istekte de düşmelidir. Ayrıca yeni arama başlarken bekleyen
 * sayımın düşmesi, tamamlanmış sayının korunması ve sayfa kendi toplamını
 * taşıyorsa ayrı sayım istenmemesi ölçülür.
 */
describe("ertelenmiş toplam sayımı (F15)", () => {
  let frames: FrameRequestCallback[] = [];

  /** Kuyruktaki çerçeveleri (zincirleme iki `rAF` dâhil) akıtır. */
  async function flushFrames(): Promise<void> {
    for (let guard = 0; guard < 10 && frames.length > 0; guard += 1) {
      const queued = frames;
      frames = [];
      await act(async () => {
        for (const callback of queued) callback(0);
      });
    }
  }

  beforeEach(() => {
    vi.clearAllMocks();
    frames = [];
    // İlk boyama gecikmesini taklit et: çerçeve hemen koşmaz, kuyruğa girer.
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      frames.push(callback);
      return frames.length;
    });
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("sayım çerçeve beklerken dosya değişirse eski dosyanın sayımı çekirdeğe gönderilmez", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));
    expect(await screen.findByText(t("search.resultsTitle"))).toBeTruthy();
    // Sayım iki çerçeve sonraya zamanlandı: henüz istek yok.
    expect(mocks.searchTotal).not.toHaveBeenCalled();

    fireEvent.change(byId<HTMLSelectElement>("search-case"), { target: { value: "c2" } });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c2"));

    await flushFrames();
    // Dosya değişimi sayımı geçersiz kıldı: eski dosya için istek **gitmemeli**
    // (yanıtın ekrana yazılmaması yetmez; çekirdek boşa saymamalı).
    expect(mocks.searchTotal).not.toHaveBeenCalled();
  });

  it("sayım beklerken aynı sorgu yeniden aranırsa eski sayım düşer; sorgu iki kez sayılmaz", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));
    expect(await screen.findByText(t("search.resultsTitle"))).toBeTruthy();
    expect(mocks.searchTotal).not.toHaveBeenCalled();

    const again = deferred<SearchPage>();
    mocks.searchPage.mockReset();
    mocks.searchPage.mockReturnValueOnce(again.promise);

    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));

    // İkinci arama uçuşta; ilk sayımın çerçeveleri şimdi akar. Yeni arama
    // başlarken bekleyen sayım düşmeliydi.
    await flushFrames();
    expect(mocks.searchTotal).not.toHaveBeenCalled();
    // Bekleyen sayım düştü: "Sayılıyor…" satırı da asılı kalmaz.
    expect(screen.queryByText(t("searchFilters.total.counting"))).toBeNull();

    await act(async () => {
      again.resolve(page());
    });
    await flushFrames();
    // Aynı sorgu için tek sayım: ne eski ne yeni sayım ikinci kez koşar.
    expect(mocks.searchTotal).toHaveBeenCalledTimes(1);
    expect(mocks.searchTotal).toHaveBeenCalledWith("c1", "kira", { ...SERVER_EMPTY }, "auto");
  });

  it("tamamlanmış sayım yeni arama başlarken korunur, yeni sayfa gelince yerini bırakır", async () => {
    setup();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");
    await flushFrames();
    const done = t("searchFilters.total.done", { total: 1, ms: 3 });
    expect(await screen.findByText(done)).toBeTruthy();

    const again = deferred<SearchPage>();
    mocks.searchPage.mockReset();
    mocks.searchPage.mockReturnValueOnce(again.promise);
    mocks.searchTotal.mockResolvedValueOnce(
      totalLine({ query: "ödeme", resolvedQuery: "ödeme", total: 9 }),
    );

    fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: "ödeme" } });
    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));

    // Yeni arama uçuşta ama ekranda hâlâ **eski** sonuçlar var: onların sayısı
    // da durur (yeni aramanın sayısı gibi gösterilmez, ama silinmez).
    expect(screen.getByText(done)).toBeTruthy();

    await act(async () => {
      again.resolve(page({ query: "ödeme", resolvedQuery: "ödeme" }));
    });
    await flushFrames();
    // İlk sayım (kira) + yeni sayım (ödeme): ikisi de birer kez koştu.
    expect(mocks.searchTotal).toHaveBeenCalledTimes(2);
    expect(mocks.searchTotal).toHaveBeenLastCalledWith("c1", "ödeme", { ...SERVER_EMPTY }, "auto");
    // Yeni sayı geldi: eski satır kalktı, yenisi yazıldı.
    expect(screen.queryByText(done)).toBeNull();
    expect(
      await screen.findByText(t("searchFilters.total.done", { total: 9, ms: 3 })),
    ).toBeTruthy();
  });

  it("düşen sayım satırı yeni arama başlarken korunur", async () => {
    setup({ totalError: { code: "E_DB", message: "sayım hatası", retryable: true } });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");
    await flushFrames();
    const failed = t("searchFilters.total.failed", { message: "sayım hatası" });
    expect(await screen.findByText(failed)).toBeTruthy();

    const again = deferred<SearchPage>();
    mocks.searchPage.mockReset();
    mocks.searchPage.mockReturnValueOnce(again.promise);

    fireEvent.change(byId<HTMLInputElement>("search-q"), { target: { value: "ödeme" } });
    fireEvent.keyDown(byId<HTMLInputElement>("search-q"), { key: "Enter" });
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));

    // Düşen sayım da **tamamlanmış** bir sonuçtur: eski sonuçlar ekranda
    // dururken satırı silinmez (yeni sayfa gelince yerini yenisine bırakır).
    expect(screen.getByText(failed)).toBeTruthy();
  });

  it("sayfa kendi toplamını taşıyorsa ayrı sayım istenmez", async () => {
    setup({ pages: [page({ total: 7 })] });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    await searchFor("kira");
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));
    await flushFrames();

    // İki kaynak aynı anda gösterilmez: sayfa toplamı varken çekirdek boşa saymaz.
    expect(mocks.searchTotal).not.toHaveBeenCalled();
    const shown = t("searchFilters.result.shown", { returned: 1 });
    expect(
      await screen.findByText(t("searchFilters.result.withTotal", { total: 7, shown }), {
        exact: false,
      }),
    ).toBeTruthy();
  });
});

/**
 * Dosya değişiminde filtre sözlüğü (F07-T03).
 *
 * Belge türü ve etiket filtrelerinin **seçenekleri dosyadan** gelir (`docs`,
 * `caseTags`); OCR durumu ve tarih rolü sabit sözlüktür. Dosya değişince ilk
 * ikisi anlamını yitirir: seçenek listesinde karşılığı olmayan bir filtre
 * ekranda **görünmez** kalır ama aramayı sessizce daraltır — avukat "belge
 * yok" sanır. Ölçülen kararlar:
 *
 * 1. Önceki dosyanın etiket filtresi yeni dosyanın aramasına taşınmaz;
 *    görünürde filtre yokken "Temizle" kapalıdır.
 * 2. Önceki dosyanın belge türü, seçici "Tümü" gösterirken uygulanmaz.
 * 3. Yeni dosyanın belgeleri/etiketleri gelene kadar önceki dosyanın
 *    seçenekleri ve çipleri ekranda (tıklanabilir) kalmaz.
 * 4. Sabit sözlüklü **görünür** filtreler (OCR, tarih rolü) korunur — düzeltme
 *    bütün filtreleri sıfırlamaz.
 */
const DOCX: DocumentRow = {
  ...DOCUMENTS[0]!,
  id: "d2",
  caseId: "c2",
  displayName: "İhtar metni",
  documentType: "docx",
  currentVersionId: "v2",
};

const TAGS2: CaseTagRow[] = [{ tag: "ihtar", documentCount: 1 }];

/** İki dosyanın **farklı** tür/etiket sözlüğü: filtrenin taşınıp taşınmadığı ölçülebilir. */
function setupTwoVocabularies() {
  for (const mock of Object.values(mocks)) mock.mockReset();
  mocks.caseList.mockResolvedValue([CASE, CASE2]);
  mocks.documentList.mockImplementation(async (id: string) => (id === "c2" ? [DOCX] : DOCUMENTS));
  mocks.caseTags.mockImplementation(async (id: string) => (id === "c2" ? TAGS2 : TAGS));
  mocks.searchPage.mockResolvedValue(page());
  mocks.searchTotal.mockResolvedValue(totalLine());
  mocks.precedentSearch.mockResolvedValue(libraryPage());
  render(<Search />);
}

function typeOptions(): string[] {
  return Array.from(byId<HTMLSelectElement>("filter-type").options).map((option) => option.value);
}

function clearFiltersButton(): HTMLButtonElement {
  return screen.getByRole("button", { name: t("search.clearFilters") }) as HTMLButtonElement;
}

describe("dosya değişiminde filtre sözlüğü", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      callback(0);
      return 0;
    });
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("önceki dosyanın etiket filtresi yeni dosyanın aramasına taşınmaz", async () => {
    setupTwoVocabularies();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));

    // c1 sözlüğü: "kira" çipi. Taslağa gerçekten girdi mi — "Temizle" açık.
    fireEvent.click(await screen.findByRole("button", { name: /kira/ }));
    expect(clearFiltersButton().disabled).toBe(false);

    fireEvent.change(byId<HTMLSelectElement>("search-case"), { target: { value: "c2" } });
    // c2'nin kendi çipi geldi; önceki dosyanın çipi ekranda yok.
    expect(await screen.findByRole("button", { name: /ihtar/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /kira/ })).toBeNull();

    await searchFor("ihtar");
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));
    // Görünmez filtre yok: istek etiketsiz gider.
    expect(mocks.searchPage).toHaveBeenCalledWith(
      "c2",
      "ihtar",
      { documentTypes: [], dateRole: null, tags: [], ocrStatus: "any" },
      { limit: 25, cursor: null, includeTotal: false, mode: "auto" },
    );
    // Görünürde filtre kalmadı: "Temizle" kapalı (görünmez filtre yok).
    expect(clearFiltersButton().disabled).toBe(true);
  });

  it('önceki dosyanın belge türü seçici "Tümü" gösterirken filtre uygulanmaz', async () => {
    setupTwoVocabularies();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    expect(typeOptions()).toEqual(["", "pdf"]);
    fireEvent.change(byId<HTMLSelectElement>("filter-type"), { target: { value: "pdf" } });
    expect(byId<HTMLSelectElement>("filter-type").value).toBe("pdf");

    fireEvent.change(byId<HTMLSelectElement>("search-case"), { target: { value: "c2" } });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c2"));
    // Yeni dosyada "pdf" yok: seçici "Tümü" gösteriyor...
    expect(typeOptions()).toEqual(["", "docx"]);
    expect(byId<HTMLSelectElement>("filter-type").value).toBe("");
    // ...ve arama gerçekten filtresiz koşar (görünmez filtre yok).
    await searchFor("ihtar");
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));
    expect(mocks.searchPage.mock.calls[0]![2]).toEqual({
      documentTypes: [],
      dateRole: null,
      tags: [],
      ocrStatus: "any",
    });
  });

  it("yeni dosyanın sözlüğü gelene kadar önceki dosyanın tür ve etiketleri gösterilmez", async () => {
    setupTwoVocabularies();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    expect(typeOptions()).toEqual(["", "pdf"]);
    expect(screen.getByRole("button", { name: /kira/ })).toBeTruthy();

    const pendingDocs = deferred<DocumentRow[]>();
    const pendingTags = deferred<CaseTagRow[]>();
    mocks.documentList.mockReset();
    mocks.caseTags.mockReset();
    mocks.documentList.mockReturnValueOnce(pendingDocs.promise);
    mocks.caseTags.mockReturnValueOnce(pendingTags.promise);

    fireEvent.change(byId<HTMLSelectElement>("search-case"), { target: { value: "c2" } });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c2"));

    // İstek uçuşta: önceki dosyanın türü/etiketi yeni dosyada tıklanabilir kalmaz.
    expect(typeOptions()).toEqual([""]);
    expect(screen.queryByRole("button", { name: /kira/ })).toBeNull();
    expect(screen.getByText(t("search.tagsEmpty"))).toBeTruthy();

    await act(async () => {
      pendingDocs.resolve([DOCX]);
      pendingTags.resolve(TAGS2);
    });
    expect(typeOptions()).toEqual(["", "docx"]);
    expect(screen.getByRole("button", { name: /ihtar/ })).toBeTruthy();
  });

  it("görünür sabit filtreler (OCR, tarih rolü) dosya değişiminde korunur", async () => {
    setupTwoVocabularies();
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c1"));
    fireEvent.change(byId<HTMLSelectElement>("filter-ocr"), { target: { value: "ocr" } });
    fireEvent.change(byId<HTMLSelectElement>("filter-role"), { target: { value: "event" } });

    fireEvent.change(byId<HTMLSelectElement>("search-case"), { target: { value: "c2" } });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("c2"));
    // Sabit sözlük: seçim görünür ve korunur (düzeltme her şeyi sıfırlamaz).
    expect(byId<HTMLSelectElement>("filter-ocr").value).toBe("ocr");
    expect(byId<HTMLSelectElement>("filter-role").value).toBe("event");

    mocks.searchPage.mockResolvedValue(
      page({ filters: { documentTypes: [], dateRole: "event", tags: [], ocrStatus: "ocr" } }),
    );
    await searchFor("ihtar");
    await waitFor(() => expect(mocks.searchPage).toHaveBeenCalledTimes(1));
    expect(mocks.searchPage.mock.calls[0]![2]).toEqual({
      documentTypes: [],
      dateRole: "event",
      tags: [],
      ocrStatus: "ocr",
    });
  });
});
