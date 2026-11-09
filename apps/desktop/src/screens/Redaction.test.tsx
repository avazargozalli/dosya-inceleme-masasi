// @vitest-environment jsdom
//
// Maskeleme ekranı davranış testi. `lib/parseProgress` testleri yalnız sayaç
// biçimini doğrular; burada 410 satırlık ekran gerçekten render edilip
// F13'ün (terim/bölge maskeleme, önizleme, bağımsız doğrulama sonucu) ve
// §13.5'in ("maskeleme planı özgün sürüme bağlanır; farklı sürüme
// kendiliğinden uygulanmaz") arayüzde **uygulandığı** ölçülür. `api` kısmi
// mock; `t()` ve `lib/parseProgress` gerçektir.
//
// Sabitlenen davranışlar:
//
// 1. Dosya listesi `caseList(false)`, ilk dosya ve ilk **sürümlü** belge
//    seçilir; sürümsüz belge seçeneklere girmez (maskelenecek sürüm yok).
// 2. Boş/yalnız ayırıcı terim listesi çekirdeğe **gitmez** (`E_TERMS`).
// 3. Terimler satır/virgülle ayrılır, kırpılır, boşlar atılır ve çekirdeğe
//    tam listeyle gider; çalışırken düğme “Maskeleniyor…” olur.
// 4. Sonuç künyesi **yanıttan** yazılır: yeni belge, sürüm no, isabet,
//    kalan terim (yoksa “yok”), motorlar, akışlar, açılamayan akış (kırmızı),
//    gizli nesne temizliği (yalnız >0), OCR durumu, uyarılar ve plan kimliği.
// 5. Düşen maskeleme önceki yeşil sonucu bırakmaz; başarılı koşu hatayı siler.
// 6. Sayfa önizlemesi istenen sürüm/sayfa için istenir, dönüş `dpi`siyle
//    ölçeklenir; görüntü **hangi sayfaya aitse** onunla etiketlenir.
// 7. **Ölçülen kusur (D):** sayfa getirilirken paylaşılan `busy` yüzünden
//    “Maskeleniyor…” yazıyor ve maskeleme ilerleme satırı çiziliyordu; sayfa
//    yükleme ayrı durumdur, maskeleme varmış gibi gösterilmez.
// 8. **Ölçülen kusur (C):** önizleme yüklendiği sayfaya bağlı değildi; sayfa
//    alanı değiştirilince eski görüntü **yeni sayfa** gibi etiketleniyor ve
//    üzerine çizilen bölge **yanlış sayfa indeksine** yazılıyordu (başka
//    sayfanın alanı maskelenir). Bölge her zaman görüntünün sayfasına gider.
// 9. Sürükleme punto koordinatına çevrilir (`72 / dpi`); 2 puntodan küçük
//    seçim bölge üretmez; “Seçimleri temizle” sayıyı sıfırlar.
// 10. **Ölçülen kusur (B):** dosya değişince önceki dosyanın sonucu,
//     önizlemesi ve bölgeleri ekranda kalıyordu → bölgeler **başka sürüme**
//     uygulanabiliyordu (§13.5 ihlali). Dosya/sürüm değişimi bunları temizler.
// 11. **Ölçülen kusur (A, INV-05):** geç gelen eski `documentList` yanıtı yeni
//     dosyanın belgelerini/sürümünü eziyordu; düşen istek önceki dosyanın
//     belgelerini bırakıyordu. İstek sırası korunur, düşen istek listeyi boşaltır.
// 12. Maskeleme sürerken hedef (dosya/belge) değiştirilemez: sonuç başka
//     dosyanın künyesi altında görünmez.
// 13. İlerleme olayı yalnız **çalışan sürüm** için yazılır; olay yoksa sayaç
//     uydurulmaz (“hazırlanıyor…”), olay varsa aşama/sayı/% çizilir.
// 14. Dosya listesi düşerse dürüst hata yazılır, sahte liste üretilmez.

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { t } from "../lib/i18n";
import { parseProgressLine } from "../lib/parseProgress";
import type {
  CaseRow,
  DocumentRow,
  DocumentVersionRow,
  ParseProgressEvent,
  RedactionPlanRow,
  RedactionRegionRow,
  RedactionResult,
} from "../lib/api";

const mocks = vi.hoisted(() => ({
  caseList: vi.fn(),
  documentList: vi.fn(),
  documentPagePreview: vi.fn(),
  documentRedact: vi.fn(),
  documentRedactRegions: vi.fn(),
  redactionPlanList: vi.fn(),
  redactionPlanRegions: vi.fn(),
}));

const events = vi.hoisted(() => ({ listen: vi.fn() }));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

// Tauri olmadan ekranı ölçmek için olay dinleyicisi sahtelenir; yakalanan
// işleyici ile `redact-progress` çerçeveleri testten gönderilir.
vi.mock("@tauri-apps/api/event", () => ({ listen: events.listen }));

const { Redaction } = await import("./Redaction");

/* ------------------------------------------------------------------ fixtures */

const ISO = "2026-11-02T09:00:00.000Z";

function caseRow(id: string, title: string): CaseRow {
  return {
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
    id,
    title,
  };
}

const P1 = caseRow("p1", "Kira uyuşmazlığı");
const P2 = caseRow("p2", "Ceza dosyası");

function docRow(id: string, displayName: string, currentVersionId: string | null): DocumentRow {
  return {
    id,
    caseId: "p1",
    displayName,
    documentType: "pdf",
    status: "active",
    currentVersionId,
    declaredDocumentDate: null,
    createdAt: ISO,
    updatedAt: ISO,
    rowVersion: 1,
  };
}

const D1 = docRow("d1", "Kira sözleşmesi.pdf", "v1");
const D2 = docRow("d2", "Ödeme makbuzu.pdf", "v2");
const D3 = docRow("d3", "Sürümsüz belge.pdf", null);
const D4 = docRow("d4", "Ceza dosyası belgesi.pdf", "v9");

function versionRow(overrides: Partial<DocumentVersionRow> = {}): DocumentVersionRow {
  return {
    id: "v9",
    documentId: "d9",
    caseId: "p1",
    versionNo: 2,
    blobId: "blob-9",
    originalSha256: "a".repeat(64),
    byteSize: 4096,
    detectedMime: "application/pdf",
    importedAt: ISO,
    ...overrides,
  };
}

/** Plan satırı — çekirdeğin döndürdüğü alanların tamamı (sicil kartı). */
function planRow(overrides: Partial<RedactionPlanRow> = {}): RedactionPlanRow {
  return {
    id: "plan-1",
    caseId: "p1",
    documentVersionId: "v1",
    parseRunId: null,
    renderHash: "b".repeat(64),
    coordinateSpace: "pdf_page_points",
    planVersion: 1,
    state: "applied",
    reviewer: null,
    outputProfile: "share",
    createdAt: ISO,
    updatedAt: ISO,
    documentTitle: "Kira sözleşmesi.pdf",
    versionNo: 1,
    regionCount: 3,
    outputVersionId: "v9",
    outputVersionNo: 1,
    outputDocumentTitle: "Kira sözleşmesi (maskeli).pdf",
    ...overrides,
  };
}

function regionRow(overrides: Partial<RedactionRegionRow> = {}): RedactionRegionRow {
  return {
    id: "region-1",
    pageIndex: 0,
    rects: [[1, 2, 3, 4]],
    coordinateSpace: "pdf_page_points",
    category: "user_term",
    detectedTextHash: "h".repeat(64),
    userDecision: "redact",
    ...overrides,
  };
}

function redactionResult(overrides: Partial<RedactionResult> = {}): RedactionResult {
  return {
    document: docRow("d9", "Kira sözleşmesi (maskeli).pdf", "v9"),
    version: versionRow(),
    plan: planRow(),
    hits: 3,
    remaining: [],
    ocrVerified: true,
    engines: ["pypdf", "ocr"],
    streamsScanned: 4,
    streamsBinary: 1,
    streamsUnreadable: 0,
    annotationsRemoved: 0,
    offpageSpansRemoved: 0,
    scriptsRemoved: 0,
    warnings: [],
    ...overrides,
  };
}

type Deferred<T> = { promise: Promise<T>; resolve: (v: T) => void; reject: (e: unknown) => void };

function deferred<T>(): Deferred<T> {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function apiError(code: string, message: string, retryable = false) {
  return { code, message, retryable };
}

function preview(dpi = 144, pngBase64 = "iVBORw0KGgo=") {
  return {
    versionId: "v1",
    pageIndex: 0,
    dpi,
    pngBase64,
    byteSize: 68,
    cached: false,
  };
}

let progressHandlers: Array<(event: { payload: ParseProgressEvent }) => void> = [];

beforeEach(() => {
  // `resetAllMocks`: `clearAllMocks` bir kezlik (`mockReturnValueOnce`)
  // kuyruğunu bırakır; düşen bir testin artığı sonraki testin yanıtını
  // sahte biçimde belirler. Tüm uygulamalar bu blokta yeniden kuruluyor.
  vi.resetAllMocks();
  progressHandlers = [];
  events.listen.mockImplementation(
    async (_name: string, handler: (event: { payload: ParseProgressEvent }) => void) => {
      progressHandlers.push(handler);
      return () => undefined;
    },
  );
  mocks.caseList.mockResolvedValue([P1, P2]);
  mocks.documentList.mockImplementation((caseId: string) =>
    Promise.resolve(caseId === "p1" ? [D1, D2, D3] : [D4]),
  );
  mocks.documentPagePreview.mockResolvedValue(preview());
  mocks.documentRedact.mockResolvedValue(redactionResult());
  mocks.documentRedactRegions.mockResolvedValue(
    redactionResult({ document: docRow("d9", "Kira sözleşmesi (bölge maskeli).pdf", "v9") }),
  );
  // Maskeleme sicili varsayılan olarak **boş**: mevcut testler bu kartı
  // ölçmez; kartın varlığı başka iddiaları bozmaz.
  mocks.redactionPlanList.mockResolvedValue([]);
  mocks.redactionPlanRegions.mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
});

async function renderScreen() {
  render(<Redaction />);
  await screen.findByRole("option", { name: D1.displayName });
}

function caseSelect(): HTMLSelectElement {
  return screen.getByLabelText(t("common.case")) as HTMLSelectElement;
}

function docSelect(): HTMLSelectElement {
  return screen.getByLabelText(t("redaction.docLabel")) as HTMLSelectElement;
}

function termsArea(): HTMLTextAreaElement {
  return screen.getByLabelText(t("redaction.termsLabel")) as HTMLTextAreaElement;
}

function runButton(): HTMLButtonElement {
  return screen.getByRole("button", { name: t("redaction.runTerms") }) as HTMLButtonElement;
}

function regionsRunButton(count: number): HTMLButtonElement {
  return screen.getByRole("button", {
    name: t("redaction.runRegions", { count }),
  }) as HTMLButtonElement;
}

async function resultCard(): Promise<HTMLElement> {
  await screen.findByText(t("redaction.resultTitle"));
  return screen.getByText(t("redaction.resultTitle")).closest(".card") as HTMLElement;
}

/** Önizlemeyi açar ve görüntüyü döndürür (bölge çizmek için kapsayıcı gerekir). */
async function loadPreview(page = 0): Promise<{ image: HTMLElement; box: HTMLElement }> {
  fireEvent.click(screen.getByRole("button", { name: t("redaction.modeRegions") }));
  const pageInput = screen.getByLabelText(t("redaction.pageLabel"));
  fireEvent.change(pageInput, { target: { value: String(page) } });
  fireEvent.click(screen.getByRole("button", { name: t("redaction.loadPage") }));
  const image = await screen.findByAltText(t("redaction.previewAlt", { page: page + 1 }));
  return { image, box: image.parentElement as HTMLElement };
}

function drag(box: HTMLElement, from: [number, number], to: [number, number]) {
  // jsdom `PointerEvent` üretmez; React `pointerdown` dinleyicisi için
  // `clientX/clientY` taşıyan bir olay yeterlidir.
  fireEvent(box, new MouseEvent("pointerdown", { bubbles: true, clientX: from[0], clientY: from[1] }));
  fireEvent(box, new MouseEvent("pointermove", { bubbles: true, clientX: to[0], clientY: to[1] }));
  fireEvent(box, new MouseEvent("pointerup", { bubbles: true, clientX: to[0], clientY: to[1] }));
}

/* -------------------------------------------------------------------- tests */

describe("Maskeleme ekranı", () => {
  it("dosya ve belge listesini kasadan ister, ilk sürümlü belgeyi seçer", async () => {
    await renderScreen();

    expect(mocks.caseList).toHaveBeenCalledWith(false);
    expect(mocks.documentList).toHaveBeenCalledWith("p1");
    expect(caseSelect().value).toBe("p1");
    expect(docSelect().value).toBe("v1");

    const docOptions = within(docSelect()).getAllByRole("option");
    expect(docOptions).toHaveLength(3); // “Seçin…” + iki sürümlü belge
    expect(screen.getByRole("option", { name: D1.displayName })).toBeTruthy();
    expect(screen.getByRole("option", { name: D2.displayName })).toBeTruthy();
    expect(screen.queryByRole("option", { name: D3.displayName })).toBeNull();

    expect(runButton().disabled).toBe(false);
    expect(screen.getByText(t("redaction.termsHint"))).toBeTruthy();

    // Önizleme alanı yalnız bölge kipinde çizilir; orada dürüst ipucu görünür.
    fireEvent.click(screen.getByRole("button", { name: t("redaction.modeRegions") }));
    expect(screen.getByText(t("redaction.previewHint"))).toBeTruthy();
  });

  it("boş veya yalnız ayırıcıdan oluşan terim listesini çekirdeğe göndermez", async () => {
    await renderScreen();

    fireEvent.change(termsArea(), { target: { value: " , \n ,, " } });
    fireEvent.click(runButton());

    expect(mocks.documentRedact).not.toHaveBeenCalled();
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("E_TERMS");
    expect(alert.textContent).toContain(t("redaction.termsRequired"));
  });

  it("terimleri satır/virgülle ayırır, kırpar ve tam listeyle gönderir", async () => {
    await renderScreen();

    fireEvent.change(termsArea(), {
      target: { value: "  Ahmet Yılmaz \n\n00000000000, TCKN ,\n " },
    });
    fireEvent.click(runButton());

    expect(mocks.documentRedact).toHaveBeenCalledTimes(1);
    expect(mocks.documentRedact).toHaveBeenCalledWith(
      "v1",
      ["Ahmet Yılmaz", "00000000000", "TCKN"],
      expect.any(String),
    );
  });

  it("sonuç künyesini yanıttan yazar; kalan terim ve akış ayrıntıları uydurulmaz", async () => {
    mocks.documentRedact.mockResolvedValue(
      redactionResult({
        remaining: ["Veli Kaya"],
        engines: ["pypdf"],
        streamsScanned: 4,
        streamsBinary: 1,
        streamsUnreadable: 2,
        annotationsRemoved: 1,
        offpageSpansRemoved: 2,
        scriptsRemoved: 1,
        ocrVerified: false,
        warnings: ["iki akış okunamadı", "gömülü yazı tipi"],
        hits: 5,
        version: versionRow({ id: "v10", versionNo: 3 }),
        document: docRow("d10", "Kira sözleşmesi (maskeli).pdf", "v10"),
      }),
    );

    await renderScreen();
    fireEvent.change(termsArea(), { target: { value: "Ahmet Yılmaz" } });
    fireEvent.click(runButton());

    const card = await resultCard();
    const q = within(card);
    expect(q.getByText("Kira sözleşmesi (maskeli).pdf")).toBeTruthy();
    expect(q.getByText("3")).toBeTruthy(); // sürüm no
    expect(q.getByText("5")).toBeTruthy(); // maskelenen bölge
    expect(q.getByText("Veli Kaya")).toBeTruthy(); // kalan terim
    expect(q.getByText("pypdf")).toBeTruthy();
    expect(q.getByText(/4 akış açıldı · 1 ikili akış/)).toBeTruthy();
    expect(q.getByText(t("redaction.unreadableStreams", { count: 2 }))).toBeTruthy();
    expect(
      q.getByText(
        t("redaction.hiddenObjectsLine", { annotations: 1, offpage: 2, scripts: 1 }),
      ),
    ).toBeTruthy();
    expect(q.getByText(t("redaction.ocrNo"))).toBeTruthy();
    expect(q.getByText("iki akış okunamadı")).toBeTruthy();
    expect(q.getByText("gömülü yazı tipi")).toBeTruthy();
    expect(q.getByText("plan-1")).toBeTruthy();
    expect(q.getByText(t("redaction.verified"))).toBeTruthy();
  });

  it("kalan terim yoksa “yok” yazar; sıfır gizli nesne satırı çizilmez", async () => {
    await renderScreen();
    fireEvent.change(termsArea(), { target: { value: "Ahmet Yılmaz" } });
    fireEvent.click(runButton());

    const card = await resultCard();
    const q = within(card);
    expect(q.getByText(t("common.none"))).toBeTruthy();
    expect(q.queryByText(t("redaction.hiddenObjects"))).toBeNull();
    expect(q.getByText(t("redaction.ocrYes"))).toBeTruthy();
  });

  it("düşen maskeleme önceki yeşil sonucu bırakmaz, başarılı koşu hatayı siler", async () => {
    await renderScreen();
    fireEvent.change(termsArea(), { target: { value: "Ahmet Yılmaz" } });
    fireEvent.click(runButton());
    await resultCard();

    mocks.documentRedact.mockRejectedValueOnce(
      apiError("E_REDACT_VERIFY", "Maskeli çıktıda hâlâ aranan metin var."),
    );
    fireEvent.click(runButton());

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_REDACT_VERIFY");
    expect(screen.queryByText(t("redaction.resultTitle"))).toBeNull();

    fireEvent.click(runButton());
    await resultCard();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("sayfa önizlemesini seçilen sürüm ve sayfa için ister, dönüş dpi'sini taşır", async () => {
    await renderScreen();
    const { image } = await loadPreview(3);

    expect(mocks.documentPagePreview).toHaveBeenCalledTimes(1);
    expect(mocks.documentPagePreview).toHaveBeenCalledWith("v1", 3);
    expect(image.getAttribute("src")).toBe("data:image/png;base64,iVBORw0KGgo=");
    expect(screen.getByText(t("redaction.previewCaption", { page: 4 }))).toBeTruthy();
    expect(screen.queryByText(t("redaction.previewHint"))).toBeNull();
  });

  it("sayfa getirilirken “Maskeleniyor…” yazmaz, maskeleme ilerlemesi çizmez", async () => {
    const pending = deferred<ReturnType<typeof preview>>();
    mocks.documentPagePreview.mockReturnValue(pending.promise);

    await renderScreen();
    fireEvent.click(screen.getByRole("button", { name: t("redaction.modeRegions") }));
    fireEvent.click(screen.getByRole("button", { name: t("redaction.loadPage") }));

    // Buton “Sayfa getiriliyor…” olur; maskeleme düğmesi ve ilerleme satırı
    // maskeleme yapılmadığı hâlde maskeleme iddia etmez.
    expect(
      screen.getByRole("button", { name: t("redaction.loadingPage") }),
    ).toBeTruthy();
    expect(regionsRunButton(0).disabled).toBe(true);
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.queryByText(t("redaction.preparing"))).toBeNull();
    expect(screen.queryByText(t("redaction.masking"))).toBeNull();

    await act(async () => {
      pending.resolve(preview());
      await pending.promise;
    });
    await screen.findByAltText(t("redaction.previewAlt", { page: 1 }));
  });

  it("önizleme yüklendiği sayfaya bağlı kalır; sayfa alanı değişince eski görüntü yeni sayfa gibi etiketlenmez", async () => {
    await renderScreen();
    await loadPreview(0);

    fireEvent.change(screen.getByLabelText(t("redaction.pageLabel")), {
      target: { value: "4" },
    });

    // Görüntü hâlâ sayfa 1'in; alt/caption bunu söyler, seçili sayfa ayrı yazılır.
    expect(screen.getByAltText(t("redaction.previewAlt", { page: 1 }))).toBeTruthy();
    expect(screen.getByText(t("redaction.previewCaption", { page: 1 }))).toBeTruthy();
    expect(
      screen.getByText(t("redaction.previewMismatch", { selected: 5 })),
    ).toBeTruthy();

    // Düşen yeniden yükleme de eski görüntüyü yeni sayfa gibi etiketlemez.
    mocks.documentPagePreview.mockRejectedValueOnce(apiError("E_PDF", "Sayfa okunamadı."));
    fireEvent.change(screen.getByLabelText(t("redaction.pageLabel")), {
      target: { value: "2" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("redaction.loadPage") }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_PDF");
    expect(screen.getByAltText(t("redaction.previewAlt", { page: 1 }))).toBeTruthy();
    expect(screen.getByText(t("redaction.previewCaption", { page: 1 }))).toBeTruthy();
  });

  it("sürüklenen bölgeyi punto koordinatına çevirir ve görüntünün sayfasıyla gönderir", async () => {
    await renderScreen();
    const { box } = await loadPreview(0);

    drag(box, [100, 50], [300, 150]);

    expect(regionsRunButton(1).disabled).toBe(false);
    const overlay = box.querySelector("span") as HTMLElement;
    expect(overlay).toBeTruthy();
    // Görüntü 144 dpi: punto → piksel çarpanı dpi/72 = 2.
    expect(overlay.style.left).toBe("100px");
    expect(overlay.style.top).toBe("50px");
    expect(overlay.style.width).toBe("200px");
    expect(overlay.style.height).toBe("100px");

    fireEvent.click(regionsRunButton(1));

    expect(mocks.documentRedactRegions).toHaveBeenCalledTimes(1);
    expect(mocks.documentRedactRegions).toHaveBeenCalledWith(
      "v1",
      [{ pageIndex: 0, rect: [50, 25, 150, 75] }],
      expect.any(String),
    );
    const card = await resultCard();
    expect(within(card).getByText("Kira sözleşmesi (bölge maskeli).pdf")).toBeTruthy();
    // Başarılı maskeden sonra seçim temizlenir.
    expect(regionsRunButton(0).disabled).toBe(true);
    expect(screen.queryByText(t("redaction.clearRegions", { count: 1 }))).toBeNull();
  });

  it("2 puntodan küçük sürüklemeyi bölge saymaz", async () => {
    await renderScreen();
    const { box } = await loadPreview(0);

    drag(box, [10, 10], [11, 11]);

    expect(regionsRunButton(0).disabled).toBe(true);
    expect(box.querySelector("span")).toBeNull();
    expect(mocks.documentRedactRegions).not.toHaveBeenCalled();
  });

  it("sayfa alanı değiştirilip yüklemeden çizilen bölgeyi görüntünün sayfasına yazar", async () => {
    await renderScreen();
    const { box } = await loadPreview(0);

    fireEvent.change(screen.getByLabelText(t("redaction.pageLabel")), {
      target: { value: "4" },
    });
    drag(box, [10, 10], [110, 110]);
    fireEvent.click(regionsRunButton(1));

    expect(mocks.documentRedactRegions).toHaveBeenCalledWith(
      "v1",
      [{ pageIndex: 0, rect: [5, 5, 55, 55] }], // 10 * 72/144
      expect.any(String),
    );
  });

  it("belge değişince önceki belgenin sonucu, önizlemesi ve bölgeleri kalkar", async () => {
    await renderScreen();
    // v1 üzerinde sonuç + önizleme + bölge biriktir.
    fireEvent.change(termsArea(), { target: { value: "Ahmet Yılmaz" } });
    fireEvent.click(runButton());
    await resultCard();
    const { box } = await loadPreview(0);
    drag(box, [10, 10], [110, 110]);
    expect(regionsRunButton(1)).toBeTruthy();

    fireEvent.change(docSelect(), { target: { value: "v2" } });

    expect(docSelect().value).toBe("v2");
    expect(screen.queryByText(t("redaction.resultTitle"))).toBeNull();
    expect(screen.queryByAltText(t("redaction.previewAlt", { page: 1 }))).toBeNull();
    expect(screen.getByText(t("redaction.previewHint"))).toBeTruthy();
    expect(regionsRunButton(0).disabled).toBe(true);
    expect((screen.getByLabelText(t("redaction.pageLabel")) as HTMLInputElement).value).toBe("0");
  });

  it("dosya değişince önceki dosyanın sonucu, önizlemesi ve bölgeleri kalkar (INV-05)", async () => {
    await renderScreen();
    fireEvent.change(termsArea(), { target: { value: "Ahmet Yılmaz" } });
    fireEvent.click(runButton());
    await resultCard();
    const { box } = await loadPreview(0);
    drag(box, [10, 10], [110, 110]);

    fireEvent.change(caseSelect(), { target: { value: "p2" } });

    expect(mocks.documentList).toHaveBeenCalledWith("p2");
    await waitFor(() => expect(docSelect().value).toBe("v9"));
    expect(screen.queryByText(t("redaction.resultTitle"))).toBeNull();
    expect(screen.queryByAltText(t("redaction.previewAlt", { page: 1 }))).toBeNull();
    expect(regionsRunButton(0).disabled).toBe(true);
    expect((screen.getByLabelText(t("redaction.pageLabel")) as HTMLInputElement).value).toBe("0");
  });

  it("geç gelen eski dosya listesi yeni dosyanın belgelerini ve sürümünü ezmez", async () => {
    let resolveP2: ((rows: DocumentRow[]) => void) | null = null;
    mocks.documentList.mockImplementation((caseId: string) => {
      if (caseId === "p1") return Promise.resolve([D1]);
      return new Promise<DocumentRow[]>((resolve) => {
        resolveP2 = resolve;
      });
    });

    await renderScreen();
    fireEvent.change(caseSelect(), { target: { value: "p2" } });
    await waitFor(() => expect(mocks.documentList).toHaveBeenCalledWith("p2"));

    // p2 askıdayken p1'e dön: p1 yanıtı hemen gelir.
    fireEvent.change(caseSelect(), { target: { value: "p1" } });
    await waitFor(() => expect(docSelect().value).toBe("v1"));

    // Şimdi eski (p2) yanıtı geç gelir; seçili dosya p1, belgeler p1 kalmalı.
    await act(async () => {
      resolveP2!([D4]);
      await Promise.resolve();
    });

    expect(caseSelect().value).toBe("p1");
    expect(docSelect().value).toBe("v1");
    expect(screen.getByRole("option", { name: D1.displayName })).toBeTruthy();
    expect(screen.queryByRole("option", { name: D4.displayName })).toBeNull();
  });

  it("düşen belge listesi önceki dosyanın belgelerini bırakmaz", async () => {
    mocks.documentList.mockImplementation((caseId: string) => {
      if (caseId === "p1") return Promise.resolve([D1, D2]);
      return Promise.reject(apiError("E_DB", "Kasa okunamadı."));
    });

    await renderScreen();
    fireEvent.change(caseSelect(), { target: { value: "p2" } });

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DB");
    expect(within(docSelect()).getAllByRole("option")).toHaveLength(1); // yalnız “Seçin…”
    expect(docSelect().value).toBe("");
    expect(runButton().disabled).toBe(true);
  });

  it("sürümlü belgesi olmayan dosyada maskeleme düğmeleri kapalıdır", async () => {
    mocks.documentList.mockImplementation((caseId: string) =>
      Promise.resolve(caseId === "p1" ? [D1] : [D3]),
    );

    await renderScreen();
    fireEvent.change(caseSelect(), { target: { value: "p2" } });

    await waitFor(() => expect(docSelect().value).toBe(""));
    expect(runButton().disabled).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: t("redaction.modeRegions") }));
    const load = screen.getByRole("button", { name: t("redaction.loadPage") }) as HTMLButtonElement;
    expect(load.disabled).toBe(true);
    expect(regionsRunButton(0).disabled).toBe(true);
  });

  it("dosya listesi düşerse dürüst hata yazar, istek üretmez", async () => {
    mocks.caseList.mockRejectedValue(apiError("E_VAULT_LOCKED", "Kasa kilitli."));
    render(<Redaction />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_VAULT_LOCKED");
    expect(mocks.documentList).not.toHaveBeenCalled();
    expect(caseSelect().value).toBe("");
    expect(runButton().disabled).toBe(true);
  });

  it("ilerleme olayını yalnız kendi çalıştırması için yazar; olay yoksa sayaç uydurmaz", async () => {
    const pending = deferred<RedactionResult>();
    mocks.documentRedact.mockReturnValue(pending.promise);

    await renderScreen();
    fireEvent.change(termsArea(), { target: { value: "Ahmet Yılmaz" } });
    fireEvent.click(runButton());
    // Çalıştırma kimliği arayüzden gider (F13): kareler bununla ayrılır.
    const runId = mocks.documentRedact.mock.calls[0]![2] as string;
    expect(runId).toBeTruthy();

    // Henüz olay yok: sayaç yok, dürüst “hazırlanıyor…”.
    expect(screen.getByRole("status").textContent).toBe(t("redaction.preparing"));
    expect(screen.queryByRole("progressbar")).toBeNull();
    // Maskeleme sürerken hedef değiştirilemez (sonuç başka dosyaya yazılmasın).
    expect(caseSelect().disabled).toBe(true);
    expect(docSelect().disabled).toBe(true);
    expect(screen.getByRole("button", { name: t("redaction.masking") })).toBeTruthy();

    // Başka sürümün olayı yok sayılır (kimlik doğru olsa bile).
    act(() => {
      for (const handler of progressHandlers) {
        handler({
          payload: { versionId: "v2", runId, stage: "redact", done: 9, total: 9 },
        });
      }
    });
    expect(screen.getByRole("status").textContent).toBe(t("redaction.preparing"));

    const event: ParseProgressEvent = {
      versionId: "v1",
      runId,
      stage: "redact",
      done: 2,
      total: 4,
    };
    act(() => {
      for (const handler of progressHandlers) handler({ payload: event });
    });
    expect(screen.getByRole("status").textContent).toBe(
      parseProgressLine({ stage: "redact", done: 2, total: 4 }),
    );
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("50");

    await act(async () => {
      pending.resolve(redactionResult());
      await pending.promise;
    });
    await resultCard();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });

  it("aynı sürümde ikinci maskelemede birincinin gecikmiş karesi görünmez", async () => {
    const first = deferred<RedactionResult>();
    const second = deferred<RedactionResult>();
    mocks.documentRedact.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    await renderScreen();
    fireEvent.change(termsArea(), { target: { value: "Ahmet Yılmaz" } });
    fireEvent.click(runButton());
    await waitFor(() => expect(mocks.documentRedact).toHaveBeenCalledTimes(1));
    const firstRun = mocks.documentRedact.mock.calls[0]![2] as string;
    await act(async () => {
      first.resolve(redactionResult());
      await first.promise;
    });

    // Aynı sürümde ikinci maskeleme: kendi kimliğiyle gider.
    fireEvent.click(runButton());
    await waitFor(() => expect(mocks.documentRedact).toHaveBeenCalledTimes(2));
    const secondRun = mocks.documentRedact.mock.calls[1]![2] as string;
    expect(secondRun).not.toBe(firstRun);

    // Birincinin gecikmiş karesi: aynı sürüm, eski kimlik → yeni satıra yazılmaz.
    act(() => {
      for (const handler of progressHandlers) {
        handler({ payload: { versionId: "v1", runId: firstRun, stage: "redact", done: 3, total: 4 } });
      }
    });
    expect(screen.getByRole("status").textContent).toBe(t("redaction.preparing"));
    expect(screen.queryByRole("progressbar")).toBeNull();

    // Kendi karesi görünür (kapı her şeyi yutmuyor).
    act(() => {
      for (const handler of progressHandlers) {
        handler({ payload: { versionId: "v1", runId: secondRun, stage: "redact", done: 1, total: 4 } });
      }
    });
    expect(screen.getByRole("status").textContent).toBe(
      parseProgressLine({ stage: "redact", done: 1, total: 4 }),
    );

    await act(async () => {
      second.resolve(redactionResult());
      await second.promise;
    });
  });

  it("çalıştırma kimliği taşımayan (kuyruktan gelen) olay kendi satırına yazılmaz", async () => {
    const pending = deferred<RedactionResult>();
    mocks.documentRedact.mockReturnValue(pending.promise);

    await renderScreen();
    fireEvent.change(termsArea(), { target: { value: "Ahmet Yılmaz" } });
    fireEvent.click(runButton());

    act(() => {
      for (const handler of progressHandlers) {
        handler({ payload: { versionId: "v1", runId: null, stage: "redact", done: 5, total: 9 } });
      }
    });
    expect(screen.getByRole("status").textContent).toBe(t("redaction.preparing"));

    await act(async () => {
      pending.resolve(redactionResult());
      await pending.promise;
    });
  });

  it("terim yer tutucusu sözlükten gelir", async () => {
    await renderScreen();
    expect(termsArea().placeholder).toBe(t("redaction.termsPlaceholder"));
    expect(termsArea().placeholder).toContain("Ahmet Yılmaz");
  });
});

/* ------------------------------------------------- hedef değişimi / uçuş yolu */
//
// Bu blok, bir önceki bloğun kapsamadığı **uçuş** davranışını ölçer: sayfa
// önizlemesi askıdayken hedef (dosya/belge) değişimi, maskeleme uçuşunda
// çizilen yeni bölge ve yarım kalan sürükleme. Kusurlar düzeltme öncesi
// bileşende ölçüldü (F13 README tur kaydı).

describe("Maskeleme ekranı uçuş yolları", () => {
  function openRegions() {
    fireEvent.click(screen.getByRole("button", { name: t("redaction.modeRegions") }));
  }

  function loadButton(): HTMLButtonElement {
    return screen.getByRole("button", { name: t("redaction.loadPage") }) as HTMLButtonElement;
  }

  function loadingButton(): HTMLButtonElement {
    return screen.getByRole("button", { name: t("redaction.loadingPage") }) as HTMLButtonElement;
  }

  function clearButton(count: number): HTMLButtonElement {
    return screen.getByRole("button", {
      name: t("redaction.clearRegions", { count }),
    }) as HTMLButtonElement;
  }

  function dashedBox(box: HTMLElement): HTMLElement | undefined {
    return Array.from(box.querySelectorAll("span")).find((el) =>
      (el as HTMLElement).style.border.includes("dashed"),
    ) as HTMLElement | undefined;
  }

  it("sayfa getirilirken belge değişince yeni belgenin sayfa düğmesi kilitli kalmaz", async () => {
    const pending = deferred<ReturnType<typeof preview>>();
    mocks.documentPagePreview.mockReturnValueOnce(pending.promise);

    await renderScreen();
    openRegions();
    fireEvent.click(loadButton());
    expect(loadingButton().disabled).toBe(true);

    fireEvent.change(docSelect(), { target: { value: "v2" } });

    // Kilit devredilir: yeni belgenin kendi sayfası istenebilir.
    expect(loadButton().disabled).toBe(false);

    // Eski yanıt yeni belgeye önizleme yazmaz.
    await act(async () => {
      pending.resolve(preview());
      await pending.promise;
    });
    expect(screen.queryByAltText(t("redaction.previewAlt", { page: 1 }))).toBeNull();
    expect(screen.getByText(t("redaction.previewHint"))).toBeTruthy();
  });

  it("sayfa getirilirken dosya değişince yeni dosyanın sayfa düğmesi kilitli kalmaz", async () => {
    const pending = deferred<ReturnType<typeof preview>>();
    mocks.documentPagePreview.mockReturnValueOnce(pending.promise);

    await renderScreen();
    openRegions();
    fireEvent.click(loadButton());
    expect(loadingButton().disabled).toBe(true);

    fireEvent.change(caseSelect(), { target: { value: "p2" } });
    await waitFor(() => expect(docSelect().value).toBe("v9"));

    expect(loadButton().disabled).toBe(false);

    await act(async () => {
      pending.resolve(preview());
      await pending.promise;
    });
    expect(screen.queryByAltText(t("redaction.previewAlt", { page: 1 }))).toBeNull();
  });

  it("geç gelen sayfa yanıtı yeni isteğin kilidini erken açmaz", async () => {
    const first = deferred<ReturnType<typeof preview>>();
    const second = deferred<ReturnType<typeof preview>>();
    mocks.documentPagePreview
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);

    await renderScreen();
    openRegions();
    fireEvent.click(loadButton()); // v1 sayfası
    fireEvent.change(docSelect(), { target: { value: "v2" } });
    fireEvent.click(loadButton()); // v2 sayfası

    expect(mocks.documentPagePreview).toHaveBeenLastCalledWith("v2", 0);

    // İlk istek geç döner: ikinci istek hâlâ uçuşta, kilit açılmamalı.
    await act(async () => {
      first.resolve(preview());
      await first.promise;
    });
    expect(loadingButton().disabled).toBe(true);
    expect(screen.queryByAltText(t("redaction.previewAlt", { page: 1 }))).toBeNull();

    await act(async () => {
      second.resolve(preview());
      await second.promise;
    });
    await screen.findByAltText(t("redaction.previewAlt", { page: 1 }));
    expect(loadButton().disabled).toBe(false);
  });

  it("devredilen sayfa isteğinin hatası yeni belgede görünmez", async () => {
    const pending = deferred<ReturnType<typeof preview>>();
    mocks.documentPagePreview.mockReturnValueOnce(pending.promise);

    await renderScreen();
    openRegions();
    fireEvent.click(loadButton());
    fireEvent.change(docSelect(), { target: { value: "v2" } });

    await act(async () => {
      pending.reject(apiError("E_PDF", "Sayfa okunamadı."));
      await pending.promise.catch(() => undefined);
    });

    expect(screen.queryByRole("alert")).toBeNull();
    expect(loadButton().disabled).toBe(false);
  });

  it("maskeleme sürerken çizilen bölge başarı yanıtıyla silinmez", async () => {
    const pending = deferred<RedactionResult>();
    mocks.documentRedactRegions.mockReturnValueOnce(pending.promise);

    await renderScreen();
    const { box } = await loadPreview(0);
    drag(box, [10, 10], [110, 110]);
    fireEvent.click(regionsRunButton(1));

    // Uçuşta ikinci bölge çizilir; gönderim yükü ilk bölgedir.
    drag(box, [200, 200], [300, 300]);
    expect(clearButton(2)).toBeTruthy();

    await act(async () => {
      pending.resolve(redactionResult());
      await pending.promise;
    });

    expect(mocks.documentRedactRegions).toHaveBeenCalledWith(
      "v1",
      [{ pageIndex: 0, rect: [5, 5, 55, 55] }],
      expect.any(String),
    );
    // Yalnız **gönderilen** bölge temizlenir; uçuşta çizilen kalır.
    expect(clearButton(1)).toBeTruthy();
    expect(box.querySelectorAll("span")).toHaveLength(1);
  });

  it("maskeleme sürerken seçim planı kilitlidir", async () => {
    const pending = deferred<RedactionResult>();
    mocks.documentRedactRegions.mockReturnValueOnce(pending.promise);

    await renderScreen();
    const { box } = await loadPreview(0);
    drag(box, [10, 10], [110, 110]);
    expect(clearButton(1).disabled).toBe(false);

    fireEvent.click(regionsRunButton(1));
    expect(clearButton(1).disabled).toBe(true);

    await act(async () => {
      pending.resolve(redactionResult());
      await pending.promise;
    });
  });

  it("dosya değişince yarım kalan sürükleme yeni önizlemeye taşınmaz", async () => {
    await renderScreen();
    const { box } = await loadPreview(0);
    fireEvent(box, new MouseEvent("pointerdown", { bubbles: true, clientX: 10, clientY: 10 }));
    expect(dashedBox(box)).toBeTruthy();

    fireEvent.change(docSelect(), { target: { value: "v2" } });
    const { box: next } = await loadPreview(0);

    expect(dashedBox(next)).toBeUndefined();
    fireEvent(next, new MouseEvent("pointerup", { bubbles: true, clientX: 60, clientY: 60 }));
    expect(
      screen.queryByRole("button", { name: t("redaction.runRegions", { count: 1 }) }),
    ).toBeNull();
  });
});

/**
 * **Ölçülen kusur:** her maskelemede `redaction_plans` + `redaction_regions`
 * yazılıyordu ama `list_redaction_plans` yalnız `crates/storage` içinde
 * tanımlıydı — Tauri komutu, `api.ts` bağı ve ekran kartı yoktu; ekranda
 * yalnız son çalıştırmanın ham plan kimliği duruyordu. Bu blok, sicilin
 * gerçekten **okunduğunu** ve satırın belge/sürüm/sayfa bilgisiyle
 * yazıldığını ölçer.
 */
describe("Maskeleme sicili (plan okuma yolu)", () => {
  function plansCard(): HTMLElement {
    return screen.getByText(t("redaction.plansTitle")).closest(".card") as HTMLElement;
  }

  it("dosya seçilince planları çekirdekten ister; satır belge adı ve sürümle yazılır", async () => {
    mocks.redactionPlanList.mockResolvedValue([planRow()]);
    await renderScreen();
    const card = plansCard();
    await within(card).findByText(/Kira sözleşmesi\.pdf/);
    expect(mocks.redactionPlanList).toHaveBeenCalledWith("p1");
    // Sürüm numarası ve bölge sayısı **satırdan** yazılır. Sürüm numarası
    // kaynak satırında belge adıyla birlikte aranır: çıktı satırı aynı numarayı
    // taşıyabilir (iki belge de 1. sürüm), iki satır karıştırılmaz.
    expect(
      within(card).getByText(
        new RegExp(
          `Kira sözleşmesi\\.pdf · ${t("redaction.planVersionNo", { version: 1 })}`,
        ),
      ),
    ).toBeTruthy();
    expect(within(card).getByText(t("redaction.planRegions", { count: 3 }))).toBeTruthy();
    expect(within(card).getByText(t("redaction.planStateApplied"))).toBeTruthy();
  });

  it("plan satırı ürettiği çıktı sürümünü yazar; kaynak satırı değişmez", async () => {
    mocks.redactionPlanList.mockResolvedValue([planRow()]);
    await renderScreen();
    const card = plansCard();
    // Çıktı **ayrı satırda** yazılır: bağ, kaynak sürümün yerine geçmez.
    expect(
      within(card).getByText(
        t("redaction.planOutput", {
          title: "Kira sözleşmesi (maskeli).pdf",
          version: t("redaction.planVersionNo", { version: 1 }),
        }),
      ),
    ).toBeTruthy();
    expect(within(card).getByText(/Kira sözleşmesi\.pdf · /)).toBeTruthy();
  });

  it("çıktı bağı kayıtlı olmayan eski plan çıktı uydurmaz", async () => {
    mocks.redactionPlanList.mockResolvedValue([
      planRow({
        outputVersionId: null,
        outputVersionNo: null,
        outputDocumentTitle: null,
      }),
    ]);
    await renderScreen();
    const card = plansCard();
    await within(card).findByText(t("redaction.planOutputUnlinked"));
    // "Çıktı: …" satırı **yok**: bağ yokken çıktı varmış gibi gösterilmez.
    expect(within(card).queryByText(/^Çıktı: /)).toBeNull();
  });

  it("çıktı belgesinin adı okunamazsa kimlik yazılır, ad uydurulmaz", async () => {
    mocks.redactionPlanList.mockResolvedValue([planRow({ outputDocumentTitle: null })]);
    await renderScreen();
    const card = plansCard();
    expect(
      within(card).getByText(
        t("redaction.planOutput", {
          title: t("redaction.planOutputNoTitle", { id: "v9" }),
          version: t("redaction.planVersionNo", { version: 1 }),
        }),
      ),
    ).toBeTruthy();
  });

  it("plan yoksa “kayıt yok” yazar (uydurma satır üretmez)", async () => {
    await renderScreen();
    await within(plansCard()).findByText(t("redaction.plansEmpty"));
    expect(within(plansCard()).queryAllByRole("button", { name: t("redaction.planRegionsShow") })).toHaveLength(0);
  });

  it("sicil okunamazsa hata kendi satırında yazılır, “kayıt yok” denmez", async () => {
    mocks.redactionPlanList.mockRejectedValue(apiError("E_IO", "kasa okunamadı"));
    await renderScreen();
    const alert = await within(plansCard()).findByRole("alert");
    expect(alert.textContent).toContain("E_IO");
    expect(alert.textContent).toContain("kasa okunamadı");
    expect(within(plansCard()).queryByText(t("redaction.plansEmpty"))).toBeNull();
  });

  it("tanınmayan plan durumu gizlenmez, ham değeriyle yazılır", async () => {
    mocks.redactionPlanList.mockResolvedValue([planRow({ state: "arsivlendi" })]);
    await renderScreen();
    await within(plansCard()).findByText(t("redaction.planStateUnknown", { state: "arsivlendi" }));
  });

  it("bölgeleri plan kimliğiyle ister ve sayfa dağılımını sayfaya göre yazar", async () => {
    mocks.redactionPlanList.mockResolvedValue([planRow({ id: "plan-7", regionCount: 3 })]);
    mocks.redactionPlanRegions.mockResolvedValue([
      regionRow({ id: "r1", pageIndex: 2 }),
      regionRow({ id: "r2", pageIndex: 0 }),
      regionRow({ id: "r3", pageIndex: 2 }),
    ]);
    await renderScreen();
    const card = plansCard();
    const show = await within(card).findByRole("button", {
      name: t("redaction.planRegionsShow"),
    });
    fireEvent.click(show);
    await waitFor(() => expect(mocks.redactionPlanRegions).toHaveBeenCalledWith("plan-7"));
    // Sayfa numarası **1 tabanlı** yazılır (çekirdek 0 tabanlı saklar).
    await within(card).findByText(t("redaction.planPageLine", { page: 1, count: 1 }));
    await within(card).findByText(t("redaction.planPageLine", { page: 3, count: 2 }));
  });

  it("geometrisi okunamayan bölge gizlenmez", async () => {
    mocks.redactionPlanList.mockResolvedValue([planRow({ regionCount: 2 })]);
    mocks.redactionPlanRegions.mockResolvedValue([
      regionRow({ id: "r1" }),
      regionRow({ id: "r2", rects: null }),
    ]);
    await renderScreen();
    const card = plansCard();
    fireEvent.click(await within(card).findByRole("button", { name: t("redaction.planRegionsShow") }));
    await within(card).findByText(t("redaction.planGeometryUnreadable", { count: 1 }));
    // Satır yine listelenir: sayım geometriye bağlı değil.
    await within(card).findByText(t("redaction.planPageLine", { page: 1, count: 2 }));
  });

  it("bölge okuması düşerse hata kendi satırında yazılır, plan satırı kalır", async () => {
    mocks.redactionPlanList.mockResolvedValue([planRow()]);
    mocks.redactionPlanRegions.mockRejectedValue(apiError("E_IO", "bölgeler okunamadı"));
    await renderScreen();
    const card = plansCard();
    fireEvent.click(await within(card).findByRole("button", { name: t("redaction.planRegionsShow") }));
    const alert = await within(card).findByRole("alert");
    expect(alert.textContent).toContain("bölgeler okunamadı");
    expect(within(card).getByText(/Kira sözleşmesi\.pdf/)).toBeTruthy();
  });

  it("aynı plana ikinci tıklama bölge isteğini tekrarlamaz, listeyi kapatır", async () => {
    mocks.redactionPlanList.mockResolvedValue([planRow()]);
    mocks.redactionPlanRegions.mockResolvedValue([regionRow()]);
    await renderScreen();
    const card = plansCard();
    fireEvent.click(await within(card).findByRole("button", { name: t("redaction.planRegionsShow") }));
    await within(card).findByText(t("redaction.planPageLine", { page: 1, count: 1 }));
    fireEvent.click(within(card).getByRole("button", { name: t("redaction.planRegionsHide") }));
    // Kapanış **düğme adından** okunur: listeyi boşaltmak tek başına "kapandı"
    // demek değil (açık kalan plan ikinci tıkta kapalı görünürdü).
    expect(within(card).getByRole("button", { name: t("redaction.planRegionsShow") })).toBeTruthy();
    expect(within(card).queryByText(t("redaction.planPageLine", { page: 1, count: 1 }))).toBeNull();
    expect(mocks.redactionPlanRegions).toHaveBeenCalledTimes(1);
  });

  it("dosya değişince önceki dosyanın planları ve bölge detayı kalkar (INV-05)", async () => {
    const pending = deferred<RedactionPlanRow[]>();
    mocks.redactionPlanList
      .mockResolvedValueOnce([planRow()])
      .mockReturnValueOnce(pending.promise);
    mocks.redactionPlanRegions.mockResolvedValue([regionRow()]);
    await renderScreen();
    const card = plansCard();
    await within(card).findByText(/Kira sözleşmesi\.pdf/);
    fireEvent.click(within(card).getByRole("button", { name: t("redaction.planRegionsShow") }));
    await within(card).findByText(t("redaction.planPageLine", { page: 1, count: 1 }));

    fireEvent.change(caseSelect(), { target: { value: "p2" } });
    // Yeni dosyanın isteği askıdayken eski satır ve açık bölge detayı kalkar.
    await waitFor(() => expect(within(card).queryByText(/Kira sözleşmesi\.pdf/)).toBeNull());
    expect(within(card).queryByText(t("redaction.planPageLine", { page: 1, count: 1 }))).toBeNull();

    await act(async () => {
      pending.resolve([
        planRow({
          id: "plan-2",
          caseId: "p2",
          documentVersionId: "v9",
          documentTitle: "Ceza dosyası belgesi.pdf",
          versionNo: 2,
        }),
      ]);
      await pending.promise;
    });
    await within(card).findByText(/Ceza dosyası belgesi\.pdf/);
  });

  it("başarılı maskelemeden sonra sicil tazelenir", async () => {
    // Tazeleme **gönderim sonrası** ikinci okumadır; satır başka bir plan
    // kimliğiyle gelir → "bu çalıştırma" işareti çıkmaz (yanlış eşleşme yok).
    mocks.redactionPlanList.mockResolvedValueOnce([]).mockResolvedValue([planRow({ id: "plan-9" })]);
    await renderScreen();
    await within(plansCard()).findByText(t("redaction.plansEmpty"));

    fireEvent.change(termsArea(), { target: { value: "Ahmet Yılmaz" } });
    fireEvent.click(runButton());
    await resultCard();

    await waitFor(() => expect(mocks.redactionPlanList).toHaveBeenCalledTimes(2));
    await within(plansCard()).findByText(/Kira sözleşmesi\.pdf/);
    expect(within(plansCard()).queryByText(t("redaction.planThisRun"))).toBeNull();
  });

  it("bu çalıştırmanın planı sicilde işaretlenir", async () => {
    mocks.redactionPlanList.mockResolvedValueOnce([]).mockResolvedValue([planRow()]);
    await renderScreen();
    await within(plansCard()).findByText(t("redaction.plansEmpty"));

    fireEvent.change(termsArea(), { target: { value: "Ahmet Yılmaz" } });
    fireEvent.click(runButton());
    await resultCard();

    await within(plansCard()).findByText(t("redaction.planThisRun"));
  });
});
