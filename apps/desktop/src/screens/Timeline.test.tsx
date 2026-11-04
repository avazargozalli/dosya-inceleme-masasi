// @vitest-environment jsdom
//
// Kronoloji ekranı davranış testi. `i18n-bindings` testleri metin parçalarını
// doğrular; burada 504 satırlık ekran gerçekten render edilip F05 kararlarının
// arayüzde **uygulandığı** ölçülür. Sabitlenen davranışlar:
//
// 1. Dosya listesi sunucudan gelir, ilk dosya seçilir; olaylar tarihli/tarihsiz
//    olarak iki bölüme ayrılır, künye satırı (tür, kesinlik, ham tarih) sunucu
//    verisinden yazılır; bilinmeyen rol/kesinlik **uydurulmaz**, ham anahtar yazılır.
// 2. Boş başlıkla olay **gönderilmez** (E_TITLE); geçerli kayıt kırpılarak tam
//    sözleşmeyle gider, taslak temizlenir ve liste tazelenir.
// 3. "Kesinlik bilinmiyor" seçilince tarih alanı kapanır ve tarih gönderilmez.
// 4. "Onayla" yalnız taslakta çıkar; onay çekirdeğe gider ve liste tazelenir.
// 5. "Sil" çekirdeğe gider ve liste tazelenir.
// 6. Düzenleme, kaydın **okunduğu sürümü** taşır; kaydetme o sürümle gider.
// 7. Çakışan düzenleme kaydı ezmez: panel kapanır, güncel liste yüklenir, hata görünür.
// 8. Kaynak paneli: çapa yoksa dürüst boş durum ve kapalı "Bağla"; ilk çapa seçilir,
//    ilişki taşınır, bağlama sonrası liste tazelenir, "Kaldır" çekirdeğe gider.
// 9. **Ölçülen kusur (INV-05):** dosya değişince önceki dosyanın olayları, açık
//    düzenleme/kaynak paneli ekranda kalıyordu; geri dönüldüğünde panel yeniden
//    açılıyor ve eski olayın çapası yeni dosyaya bağlanabiliyordu.
// 10. **Ölçülen kusur:** düşen kaynak isteği önceki olayın bağlantılarını ve çapasını
//     bırakıyordu → yeni olayın panelinde eski kaynak görünüyor, "Bağla" eski çapayla açıktı.
// 11. **Ölçülen kusur:** başarılı işlem önceki hata bildirimini silmiyordu (kırmızı
//     hata + başarılı liste aynı ekranda).
// 12. **Ölçülen kusur (metin):** bağlantı rozeti ilişkiyi ham anahtarla ("contradicts")
//     yazıyordu; sözlükte karşılığı varken kullanıcıya Türkçe etiket gösterilir.
// 13. **Uçuş yolları (ek tur):** dosya değişirken geç gelen olay listesi/hatası yeni
//     dosyaya yazılmaz; yazma sonrası tazeleme eski dosyanın kimliğiyle çağrılsa bile
//     yeni dosyanın listesini ezmez; geç gelen kaynak/bağlama/kaldırma yanıtları
//     sonradan açılan olayın panelini ezmez; onay/silme/bağla/kaldır uçuşta kilitlidir;
//     yazma uçuşunda form alanları kilitlidir (sessiz kayıp yok); kaydetme yanıtı
//     başka satırın düzenleyicisini kapatmaz.

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../lib/i18n";
import type {
  AnchorRow,
  CaseRow,
  EventRow,
  EventSourceRow,
} from "../lib/api";

const mocks = vi.hoisted(() => ({
  caseList: vi.fn(),
  eventList: vi.fn(),
  eventCreate: vi.fn(),
  eventUpdate: vi.fn(),
  eventConfirm: vi.fn(),
  eventDelete: vi.fn(),
  eventSources: vi.fn(),
  eventLinkSource: vi.fn(),
  eventUnlinkSource: vi.fn(),
  anchorList: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

const { Timeline } = await import("./Timeline");

/* ------------------------------------------------------------------ fixtures */

const ISO = "2026-10-01T00:00:00.000Z";

function caseRow(overrides: Partial<CaseRow> & { id: string; title: string }): CaseRow {
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
    ...overrides,
  };
}

const P1 = caseRow({ id: "p1", title: "Kira uyuşmazlığı" });
const P2 = caseRow({ id: "p2", title: "Ceza dosyası" });

function eventRow(overrides: Partial<EventRow> & { id: string; title: string }): EventRow {
  return {
    caseId: "p1",
    eventKind: "other",
    dateValue: null,
    datePrecision: "day",
    dateRole: "event",
    dateRaw: null,
    sortableDate: null,
    description: "",
    assertionKind: "user",
    status: "draft",
    createdAt: ISO,
    updatedAt: ISO,
    rowVersion: 1,
    ...overrides,
  };
}

/** Tarihli taslak olay: onay düğmesi çıkmalı, sürüm 3 taşınmalı. */
const E_DRAFT = eventRow({
  id: "e1",
  title: "Kira ödemesi yapılmadı",
  dateValue: "2026-03-01",
  dateRaw: "2026-03-01",
  sortableDate: "2026-03-01",
  status: "draft",
  rowVersion: 3,
});

/** Tarihi bilinmeyen, onaylanmış olay: ikinci bölümde, onay düğmesi yok. */
const E_UNKNOWN = eventRow({
  id: "e2",
  title: "Tebligat tarihi belirsiz",
  datePrecision: "unknown",
  dateRole: "notification",
  sortableDate: null,
  status: "confirmed",
  rowVersion: 2,
});

/** Ham tarih metni farklı: "ham: …" notu çıkmalı. */
const E_RAW = eventRow({
  id: "e3",
  title: "Keşif",
  dateValue: "2026-04-10",
  dateRaw: "10 Nisan 2026",
  sortableDate: "2026-04-10",
  datePrecision: "month",
  dateRole: "hearing",
  status: "confirmed",
  rowVersion: 1,
});

const A1: AnchorRow = {
  id: "anchor-1234567890",
  caseId: "p1",
  documentVersionId: "v1",
  parseRunId: "run-1",
  pageId: "pg1",
  segmentId: "seg-1",
  startOffset: 10,
  endOffset: 34,
  exactQuote: "kira bedeli ödenmedi",
  prefix: "",
  suffix: "",
  quoteHash: "hash-1",
  anchorStatus: "current",
  createdAt: ISO,
};

const A2: AnchorRow = {
  ...A1,
  id: "anchor-2",
  exactQuote:
    "kiracı, kira bedelini ödemediğini kabul etmiş ve tahliyeyi taahhüt etmiştir",
};

const LINK: EventSourceRow = {
  id: "link-1",
  eventId: "e2",
  sourceAnchorId: A1.id,
  relation: "contradicts",
  userConfirmed: true,
};

type SetupOptions = {
  cases?: CaseRow[];
  events?: (caseId: string) => Promise<EventRow[]>;
  anchors?: (caseId: string) => Promise<AnchorRow[]>;
  sources?: (eventId: string) => Promise<EventSourceRow[]>;
};

function setup(options: SetupOptions = {}) {
  mocks.caseList.mockResolvedValue(options.cases ?? [P1, P2]);
  mocks.eventList.mockImplementation(options.events ?? (async () => []));
  mocks.anchorList.mockImplementation(options.anchors ?? (async () => []));
  mocks.eventSources.mockImplementation(options.sources ?? (async () => []));
  mocks.eventCreate.mockResolvedValue(eventRow({ id: "e-new", title: "Yeni olay" }));
  mocks.eventUpdate.mockResolvedValue(eventRow({ id: "e1", title: "Güncel başlık" }));
  mocks.eventConfirm.mockResolvedValue({ ...E_DRAFT, status: "confirmed" });
  mocks.eventDelete.mockResolvedValue(undefined);
  mocks.eventLinkSource.mockResolvedValue({});
  mocks.eventUnlinkSource.mockResolvedValue(undefined);
  render(<Timeline />);
}

/** Listeden olay seçer (başlık taşıyan `.card.tight`). */
function rowOf(title: string): HTMLElement {
  const row = screen.getByText(title).closest(".card.tight");
  if (!row) throw new Error(`"${title}" satırı bulunamadı`);
  return row as HTMLElement;
}

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) throw new Error(`#${id} yok`);
  return element as T;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/**
 * Ekranın p1'i gerçekten yüklediğini bekler. "Henüz tarihli olay yok" boş durumu
 * **yükleme öncesinde de** çizilir; tek başına hazır olma işareti değildir.
 * (CI'da ölçüldü: boş durum metninden hemen sonra tıklanan "Olay ekle" hatası,
 * dosya yüklenince çalışan `[caseId]` temizliğiyle siliniyordu.)
 */
async function caseLoaded() {
  await waitFor(() => expect(mocks.eventList).toHaveBeenCalledWith("p1"));
}

describe("kronoloji ekranı davranışı", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("dosya ve olayları sunucudan yükler; tarihli/tarihsiz bölümleri ve künyeyi yazar", async () => {
    setup({ events: async () => [E_DRAFT, E_UNKNOWN, E_RAW] });

    expect(await screen.findByText(E_DRAFT.title)).toBeTruthy();
    expect(mocks.caseList).toHaveBeenCalledWith(false);
    expect(byId<HTMLSelectElement>("tl-case").value).toBe("p1");
    expect(mocks.eventList).toHaveBeenCalledWith("p1");

    // Bölüm sayıları sunucudan gelen listeden hesaplanır; uydurulmaz.
    expect(screen.getByText(t("timeline.knownSub", { count: 2 }))).toBeTruthy();
    expect(screen.getByText(t("timeline.unknownSub", { count: 1 }))).toBeTruthy();
    expect(screen.getByText(t("timeline.knownTitle"))).toBeTruthy();
    expect(screen.getByText(t("timeline.unknownTitle"))).toBeTruthy();

    const draftRow = rowOf(E_DRAFT.title);
    expect(within(draftRow).getByText(t("timeline.role.event"))).toBeTruthy();
    expect(within(draftRow).getByText(t("timeline.draft"))).toBeTruthy();
    expect(draftRow.textContent).toContain("2026-03-01");
    expect(draftRow.textContent).toContain(t("timeline.precision.day"));
    // Ham tarih sıralanabilir tarihle aynıysa ayrıca yazılmaz.
    expect(draftRow.textContent).not.toContain(t("timeline.rawNote", { raw: E_DRAFT.dateRaw! }));

    const rawRow = rowOf(E_RAW.title);
    expect(rawRow.textContent).toContain("10 Nisan 2026");
    expect(rawRow.textContent).toContain(t("timeline.rawNote", { raw: "10 Nisan 2026" }));

    const unknownRow = rowOf(E_UNKNOWN.title);
    expect(unknownRow.textContent).toContain(t("dates.missing"));
    expect(unknownRow.textContent).toContain(t("timeline.precision.unknown"));
    expect(within(unknownRow).getByText(t("timeline.confirmed"))).toBeTruthy();
  });

  it("bilinmeyen tarih türü/kesinliği ham anahtarla yazar; tarihsiz olay yoksa ikinci bölüm çizilmez", async () => {
    setup({
      events: async () => [
        eventRow({
          id: "e9",
          title: "Zamanaşımı",
          sortableDate: "2026-02-02",
          dateValue: "2026-02-02",
          dateRole: "deadline",
          datePrecision: "fortnight",
        }),
      ],
    });

    await screen.findByText("Zamanaşımı");
    const row = rowOf("Zamanaşımı");
    // Sözlükte karşılığı olmayan değer uydurulmaz; sunucunun anahtarı görünür.
    expect(row.textContent).toContain("deadline");
    expect(row.textContent).toContain("fortnight");
    expect(screen.queryByText(t("timeline.unknownTitle"))).toBeNull();
  });

  it("olay yoksa tarihli bölümde dürüst boş durum yazar", async () => {
    setup({ events: async () => [] });
    await caseLoaded();
    expect(await screen.findByText(t("timeline.knownEmpty"))).toBeTruthy();
  });

  it("boş başlıkla olay gönderilmez; geçerli kayıt kırpılarak gider ve taslak temizlenir", async () => {
    setup({ events: async () => [] });
    await screen.findByText(t("timeline.knownEmpty"));
    await caseLoaded();

    fireEvent.click(screen.getByRole("button", { name: t("timeline.addEvent") }));
    expect(await screen.findByText("E_TITLE")).toBeTruthy();
    expect(screen.getByText(t("timeline.titleRequired"), { exact: false })).toBeTruthy();
    expect(mocks.eventCreate).not.toHaveBeenCalled();

    fireEvent.change(byId<HTMLInputElement>("tl-title"), {
      target: { value: "  Duruşma ertelendi  " },
    });
    fireEvent.change(byId<HTMLInputElement>("tl-date"), { target: { value: "2026-05-12" } });
    fireEvent.change(byId<HTMLSelectElement>("tl-precision"), { target: { value: "month" } });
    fireEvent.change(byId<HTMLSelectElement>("tl-role"), { target: { value: "hearing" } });
    fireEvent.click(screen.getByRole("button", { name: t("timeline.addEvent") }));

    await waitFor(() => expect(mocks.eventCreate).toHaveBeenCalledTimes(1));
    expect(mocks.eventCreate).toHaveBeenCalledWith("p1", {
      eventKind: "other",
      dateValue: "2026-05-12",
      datePrecision: "month",
      dateRole: "hearing",
      dateRaw: "2026-05-12",
      title: "Duruşma ertelendi",
      description: "",
      assertionKind: "user",
    });
    expect(byId<HTMLInputElement>("tl-title").value).toBe("");
    await waitFor(() => expect(mocks.eventList).toHaveBeenCalledTimes(2));
  });

  it("kesinlik bilinmiyorken tarih alanı kapalı ve tarih gönderilmez", async () => {
    setup({ events: async () => [] });
    await screen.findByText(t("timeline.knownEmpty"));
    await caseLoaded();

    fireEvent.change(byId<HTMLInputElement>("tl-title"), {
      target: { value: "Tarihi belirsiz olay" },
    });
    fireEvent.change(byId<HTMLSelectElement>("tl-precision"), { target: { value: "unknown" } });

    expect(byId<HTMLInputElement>("tl-date").disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: t("timeline.addEvent") }));

    await waitFor(() => expect(mocks.eventCreate).toHaveBeenCalledTimes(1));
    expect(mocks.eventCreate).toHaveBeenCalledWith(
      "p1",
      expect.objectContaining({
        dateValue: null,
        dateRaw: null,
        datePrecision: "unknown",
        title: "Tarihi belirsiz olay",
      }),
    );
  });

  it("onayla düğmesi yalnız taslakta çıkar; onay çekirdeğe gider ve liste tazelenir", async () => {
    setup({ events: async () => [E_DRAFT, E_UNKNOWN] });
    await screen.findByText(E_DRAFT.title);

    const draftRow = rowOf(E_DRAFT.title);
    const confirmedRow = rowOf(E_UNKNOWN.title);
    expect(within(confirmedRow).queryByRole("button", { name: t("timeline.confirm") })).toBeNull();

    fireEvent.click(within(draftRow).getByRole("button", { name: t("timeline.confirm") }));
    await waitFor(() => expect(mocks.eventConfirm).toHaveBeenCalledWith("e1"));
    await waitFor(() => expect(mocks.eventList).toHaveBeenCalledTimes(2));
  });

  it("silme çekirdeğe gider ve listeyi tazeler", async () => {
    setup({ events: async () => [E_DRAFT] });
    await screen.findByText(E_DRAFT.title);

    fireEvent.click(within(rowOf(E_DRAFT.title)).getByRole("button", { name: t("timeline.remove") }));
    await waitFor(() => expect(mocks.eventDelete).toHaveBeenCalledWith("e1"));
    await waitFor(() => expect(mocks.eventList).toHaveBeenCalledTimes(2));
  });

  it("düzenleme okunan sürümü taşır; kaydetme o sürümle gider ve panel kapanır", async () => {
    setup({ events: async () => [E_DRAFT] });
    await screen.findByText(E_DRAFT.title);
    const row = rowOf(E_DRAFT.title);

    fireEvent.click(within(row).getByRole("button", { name: t("timeline.edit") }));
    expect(within(row).getByText(t("timeline.versionHint", { version: 3 }))).toBeTruthy();
    expect(byId<HTMLInputElement>("edit-title-e1").value).toBe(E_DRAFT.title);

    fireEvent.change(byId<HTMLInputElement>("edit-title-e1"), {
      target: { value: "  Güncel başlık  " },
    });
    fireEvent.change(byId<HTMLInputElement>("edit-desc-e1"), { target: { value: "  not  " } });
    fireEvent.click(within(row).getByRole("button", { name: t("common.save") }));

    await waitFor(() => expect(mocks.eventUpdate).toHaveBeenCalledTimes(1));
    expect(mocks.eventUpdate).toHaveBeenCalledWith("e1", 3, {
      eventKind: "other",
      dateValue: "2026-03-01",
      datePrecision: "day",
      dateRole: "event",
      dateRaw: "2026-03-01",
      title: "Güncel başlık",
      description: "not",
    });
    await waitFor(() => expect(document.getElementById("edit-title-e1")).toBeNull());
    await waitFor(() => expect(mocks.eventList).toHaveBeenCalledTimes(2));
  });

  it("düzenlemede boş başlık gönderilmez", async () => {
    setup({ events: async () => [E_DRAFT] });
    await screen.findByText(E_DRAFT.title);
    const row = rowOf(E_DRAFT.title);

    fireEvent.click(within(row).getByRole("button", { name: t("timeline.edit") }));
    fireEvent.change(byId<HTMLInputElement>("edit-title-e1"), { target: { value: "   " } });
    fireEvent.click(within(row).getByRole("button", { name: t("common.save") }));

    expect(await screen.findByText("E_TITLE")).toBeTruthy();
    expect(mocks.eventUpdate).not.toHaveBeenCalled();
    expect(document.getElementById("edit-title-e1")).not.toBeNull();
  });

  it("düzenleme çakışması kaydı ezmez: panel kapanır, güncel liste yüklenir, hata görünür", async () => {
    let calls = 0;
    setup({
      events: async () => {
        calls += 1;
        return calls === 1
          ? [E_DRAFT]
          : [{ ...E_DRAFT, title: "Sunucudaki başlık", rowVersion: 4 }];
      },
    });
    await screen.findByText(E_DRAFT.title);
    mocks.eventUpdate.mockRejectedValue({
      code: "E_CONFLICT",
      message: "Kayıt başka pencerede değişti.",
      retryable: false,
    });

    const row = rowOf(E_DRAFT.title);
    fireEvent.click(within(row).getByRole("button", { name: t("timeline.edit") }));
    fireEvent.change(byId<HTMLInputElement>("edit-title-e1"), {
      target: { value: "Benim başlığım" },
    });
    fireEvent.click(within(row).getByRole("button", { name: t("common.save") }));

    expect(await screen.findByText("E_CONFLICT")).toBeTruthy();
    // Eski sürümle ikinci kez yazılmaz; panel kapanır ve sunucunun taze kaydı görünür.
    expect(mocks.eventUpdate).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("Sunucudaki başlık")).toBeTruthy();
    expect(document.getElementById("edit-title-e1")).toBeNull();
  });

  it("kaynak paneli: çapa yoksa dürüst boş durum yazar ve bağlamayı kapatır", async () => {
    setup({ events: async () => [E_UNKNOWN] });
    await screen.findByText(E_UNKNOWN.title);
    const row = rowOf(E_UNKNOWN.title);

    fireEvent.click(within(row).getByRole("button", { name: t("timeline.sources") }));
    expect(await within(row).findByText(t("timeline.noSources"))).toBeTruthy();
    expect(mocks.anchorList).toHaveBeenCalledWith("p1");
    expect(mocks.eventSources).toHaveBeenCalledWith("e2");

    const select = byId<HTMLSelectElement>("src-e2");
    expect(within(select).getByText(t("timeline.noAnchors"))).toBeTruthy();
    expect(within(row).getByRole("button", { name: t("timeline.attach") })).toHaveProperty(
      "disabled",
      true,
    );

    // İkinci tıklama paneli kapatır.
    fireEvent.click(within(row).getByRole("button", { name: t("timeline.closeSources") }));
    expect(within(row).queryByText(t("timeline.noSources"))).toBeNull();
  });

  it("kaynak bağlama ilişkiyi taşır, rozet sözlükten yazılır, kaldırma çekirdeğe gider", async () => {
    let sourceCalls = 0;
    setup({
      events: async () => [E_UNKNOWN],
      anchors: async () => [A1, A2],
      sources: async () => {
        sourceCalls += 1;
        if (sourceCalls === 1) return [];
        if (sourceCalls === 2) return [LINK];
        return [];
      },
    });
    await screen.findByText(E_UNKNOWN.title);
    const row = rowOf(E_UNKNOWN.title);

    fireEvent.click(within(row).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(mocks.eventSources).toHaveBeenCalledWith("e2"));

    const select = byId<HTMLSelectElement>("src-e2");
    // İlk çapa seçilir; uzun alıntı 60 karakterde kırpılır.
    expect(select.value).toBe(A1.id);
    expect(within(select).getByText(A1.exactQuote.slice(0, 60))).toBeTruthy();
    expect(within(select).getByText(A2.exactQuote.slice(0, 60))).toBeTruthy();
    expect(within(select).queryByText(A2.exactQuote)).toBeNull();

    fireEvent.change(byId<HTMLSelectElement>("rel-e2"), { target: { value: "contradicts" } });
    fireEvent.click(within(row).getByRole("button", { name: t("timeline.attach") }));

    await waitFor(() =>
      expect(mocks.eventLinkSource).toHaveBeenCalledWith("e2", A1.id, "contradicts"),
    );
    await waitFor(() => expect(mocks.eventSources).toHaveBeenCalledTimes(2));

    // Bağlantı rozeti ham anahtar değil, sözlükteki Türkçe etiketi yazar.
    const linkItem = row.querySelector("li");
    expect(linkItem).not.toBeNull();
    expect(linkItem!.textContent).toContain(t("timeline.relationKind.contradicts"));
    expect(linkItem!.textContent).not.toContain("contradicts");

    fireEvent.click(within(linkItem as HTMLElement).getByRole("button", { name: t("timeline.removeLink") }));
    await waitFor(() => expect(mocks.eventUnlinkSource).toHaveBeenCalledWith("link-1"));
    await waitFor(() => expect(row.querySelector("li")).toBeNull());
  });

  it("düşen kaynak isteği önceki olayın bağlantılarını ve çapasını bırakmaz", async () => {
    let anchorCalls = 0;
    setup({
      events: async () => [E_DRAFT, E_UNKNOWN],
      anchors: async () => {
        anchorCalls += 1;
        if (anchorCalls === 1) return [A1];
        throw { code: "E_ANCHOR", message: "Çapalar yüklenemedi.", retryable: true };
      },
      sources: async (eventId) => (eventId === "e1" ? [LINK] : []),
    });
    await screen.findByText(E_DRAFT.title);

    const draftRow = rowOf(E_DRAFT.title);
    fireEvent.click(within(draftRow).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(draftRow.querySelector("li")).not.toBeNull());
    fireEvent.click(within(draftRow).getByRole("button", { name: t("timeline.closeSources") }));

    const unknownRow = rowOf(E_UNKNOWN.title);
    fireEvent.click(within(unknownRow).getByRole("button", { name: t("timeline.sources") }));

    expect(await screen.findByText("E_ANCHOR")).toBeTruthy();
    // Önceki olayın bağlantısı yeni olayın panelinde görünmez; çapa da taşınmaz.
    expect(unknownRow.querySelector("li")).toBeNull();
    expect(within(unknownRow).getByText(t("timeline.noSources"))).toBeTruthy();
    expect(within(unknownRow).getByRole("button", { name: t("timeline.attach") })).toHaveProperty(
      "disabled",
      true,
    );
  });

  it("dosya değişince önceki dosyanın olayları ve panelleri ekranda kalmaz (INV-05)", async () => {
    const p2Events = deferred<EventRow[]>();
    setup({
      events: async (caseId) => (caseId === "p1" ? [E_DRAFT] : p2Events.promise),
      anchors: async () => [A1],
      sources: async () => [LINK],
    });
    await screen.findByText(E_DRAFT.title);

    fireEvent.click(within(rowOf(E_DRAFT.title)).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(rowOf(E_DRAFT.title).querySelector("li")).not.toBeNull());

    fireEvent.change(byId<HTMLSelectElement>("tl-case"), { target: { value: "p2" } });
    await waitFor(() => expect(mocks.eventList).toHaveBeenCalledWith("p2"));

    // Yeni dosyanın verisi gelmeden önce eski dosyanın olayı ve açık paneli kalkar.
    expect(screen.queryByText(E_DRAFT.title)).toBeNull();
    expect(screen.queryByText(t("timeline.closeSources"))).toBeNull();

    p2Events.resolve([{ ...E_UNKNOWN, caseId: "p2", title: "Ceza olayı" }]);
    expect(await screen.findByText("Ceza olayı")).toBeTruthy();

    // Geri dönüldüğünde eski panel kendiliğinden açılmaz (taşınan durum yok).
    fireEvent.change(byId<HTMLSelectElement>("tl-case"), { target: { value: "p1" } });
    await waitFor(() => expect(mocks.eventList).toHaveBeenCalledWith("p1"));
    expect(await screen.findByText(E_DRAFT.title)).toBeTruthy();
    expect(screen.queryByText(t("timeline.closeSources"))).toBeNull();
    expect(screen.queryByText(t("timeline.noSources"))).toBeNull();
  });

  it("başarılı işlem önceki hata bildirimini siler", async () => {
    setup({ events: async () => [] });
    await screen.findByText(t("timeline.knownEmpty"));
    await caseLoaded();

    fireEvent.click(screen.getByRole("button", { name: t("timeline.addEvent") }));
    expect(await screen.findByText("E_TITLE")).toBeTruthy();

    fireEvent.change(byId<HTMLInputElement>("tl-title"), { target: { value: "Yeni olay" } });
    fireEvent.click(screen.getByRole("button", { name: t("timeline.addEvent") }));

    await waitFor(() => expect(mocks.eventCreate).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.queryByText("E_TITLE")).toBeNull());
  });
});

/* ------------------------------------------------------ uçuş yolları (ek tur) */

const LINK_B: EventSourceRow = {
  id: "link-b",
  eventId: "e1",
  sourceAnchorId: A2.id,
  relation: "supports",
  userConfirmed: true,
};

const E_P2: EventRow = { ...E_UNKNOWN, id: "e2-p2", caseId: "p2", title: "Ceza olayı" };

describe("kronoloji ekranı uçuş yolları", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("geç gelen olay listesi yeni dosyanın listesini ezmez (INV-05)", async () => {
    const p1Events = deferred<EventRow[]>();
    setup({
      events: async (caseId) => (caseId === "p1" ? p1Events.promise : [E_P2]),
    });
    await waitFor(() => expect(mocks.eventList).toHaveBeenCalledWith("p1"));

    fireEvent.change(byId<HTMLSelectElement>("tl-case"), { target: { value: "p2" } });
    expect(await screen.findByText("Ceza olayı")).toBeTruthy();
    expect(screen.getByText(t("timeline.unknownSub", { count: 1 }))).toBeTruthy();

    await act(async () => {
      p1Events.resolve([E_DRAFT]);
      await p1Events.promise;
    });

    // Eski dosyanın geç yanıtı yeni dosyanın bölümlerine yazılmaz.
    expect(screen.queryByText(E_DRAFT.title)).toBeNull();
    expect(screen.queryByText(t("timeline.knownSub", { count: 1 }))).toBeNull();
    expect(screen.getByText(t("timeline.unknownSub", { count: 1 }))).toBeTruthy();
  });

  it("geç gelen eski dosya hatası yeni dosyada görünmez", async () => {
    const p1Events = deferred<EventRow[]>();
    setup({
      events: async (caseId) => (caseId === "p1" ? p1Events.promise : [E_P2]),
    });
    await waitFor(() => expect(mocks.eventList).toHaveBeenCalledWith("p1"));

    fireEvent.change(byId<HTMLSelectElement>("tl-case"), { target: { value: "p2" } });
    expect(await screen.findByText("Ceza olayı")).toBeTruthy();

    await act(async () => {
      p1Events.reject({ code: "E_IO", message: "Olaylar okunamadı.", retryable: true });
      await p1Events.promise.catch(() => undefined);
    });

    expect(screen.queryByText("E_IO")).toBeNull();
  });

  it("yazma sonrası tazeleme eski dosyanın listesini yeni dosyaya yazmaz", async () => {
    const created = deferred<EventRow>();
    const refresh = deferred<EventRow[]>();
    let p1Calls = 0;
    setup({
      events: async (caseId) => {
        if (caseId !== "p1") return [E_P2];
        p1Calls += 1;
        return p1Calls === 1 ? [] : refresh.promise;
      },
    });
    await caseLoaded();

    mocks.eventCreate.mockReturnValue(created.promise);
    fireEvent.change(byId<HTMLInputElement>("tl-title"), { target: { value: "Yeni olay" } });
    fireEvent.click(screen.getByRole("button", { name: t("timeline.addEvent") }));
    await waitFor(() => expect(mocks.eventCreate).toHaveBeenCalledTimes(1));

    fireEvent.change(byId<HTMLSelectElement>("tl-case"), { target: { value: "p2" } });
    expect(await screen.findByText("Ceza olayı")).toBeTruthy();

    await act(async () => {
      created.resolve(eventRow({ id: "e-new", title: "Yeni olay" }));
      await created.promise;
    });
    // Taslak temizliği ekleme akışının bittiğini gösterir; tazeleme ondan sonra koşar.
    await waitFor(() => expect(byId<HTMLInputElement>("tl-title").value).toBe(""));

    await act(async () => {
      refresh.resolve([E_DRAFT]);
      await refresh.promise;
    });

    expect(screen.queryByText(E_DRAFT.title)).toBeNull();
    expect(screen.getByText("Ceza olayı")).toBeTruthy();
    // Eski dosya için ikinci bir liste isteği hiç gönderilmez (boşa çekirdek okuması yok).
    const p1Loads = mocks.eventList.mock.calls.filter((call: unknown[]) => call[0] === "p1");
    expect(p1Loads).toHaveLength(1);
  });

  it("geç gelen kaynak yanıtı sonra açılan olayın panelini ezmez", async () => {
    const firstSources = deferred<EventSourceRow[]>();
    setup({
      events: async () => [E_DRAFT, E_UNKNOWN],
      anchors: async () => [A1, A2],
      sources: async (eventId) => (eventId === "e1" ? firstSources.promise : [LINK]),
    });
    await screen.findByText(E_DRAFT.title);
    const row1 = rowOf(E_DRAFT.title);
    const row2 = rowOf(E_UNKNOWN.title);

    fireEvent.click(within(row1).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(mocks.eventSources).toHaveBeenCalledWith("e1"));

    fireEvent.click(within(row2).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(row2.querySelector("li")).not.toBeNull());
    expect(row2.textContent).toContain(t("timeline.relationKind.contradicts"));

    await act(async () => {
      firstSources.resolve([LINK_B]);
      await firstSources.promise;
    });

    // e2 paneli kendi bağlantısını korur; e1'in bağlantısı buraya yazılmaz.
    expect(row2.querySelectorAll("li")).toHaveLength(1);
    const item = row2.querySelector("li") as HTMLElement;
    expect(item.textContent).toContain(t("timeline.relationKind.contradicts"));
    expect(item.textContent).not.toContain(t("timeline.relationKind.supports"));
  });

  it("geç gelen kaynak hatası sonra açılan olayın panelinde görünmez", async () => {
    const failing = deferred<EventSourceRow[]>();
    setup({
      events: async () => [E_DRAFT, E_UNKNOWN],
      anchors: async () => [A1],
      sources: async (eventId) => (eventId === "e1" ? failing.promise : []),
    });
    await screen.findByText(E_DRAFT.title);
    const row1 = rowOf(E_DRAFT.title);
    const row2 = rowOf(E_UNKNOWN.title);

    fireEvent.click(within(row1).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(mocks.eventSources).toHaveBeenCalledWith("e1"));

    fireEvent.click(within(row2).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(row2.querySelector("p.hint")).not.toBeNull());

    await act(async () => {
      failing.reject({ code: "E_ANCHOR", message: "Çapalar yüklenemedi.", retryable: true });
      await failing.promise.catch(() => undefined);
    });

    expect(screen.queryByText("E_ANCHOR")).toBeNull();
  });

  it("geç gelen bağlama hatası panel değiştiyse yeni olayda görünmez", async () => {
    const failing = deferred<unknown>();
    setup({
      events: async () => [E_DRAFT, E_UNKNOWN],
      anchors: async () => [A1],
      sources: async () => [],
    });
    await screen.findByText(E_DRAFT.title);
    const row1 = rowOf(E_DRAFT.title);
    const row2 = rowOf(E_UNKNOWN.title);

    fireEvent.click(within(row1).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(byId<HTMLSelectElement>("src-e1").value).toBe(A1.id));

    mocks.eventLinkSource.mockReturnValue(failing.promise);
    fireEvent.click(within(row1).getByRole("button", { name: t("timeline.attach") }));
    await waitFor(() => expect(mocks.eventLinkSource).toHaveBeenCalledTimes(1));

    fireEvent.click(within(row2).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(row2.querySelector("p.hint")).not.toBeNull());

    await act(async () => {
      failing.reject({ code: "E_ANCHOR", message: "Bağ kurulamadı.", retryable: true });
      await failing.promise.catch(() => undefined);
    });

    expect(screen.queryByText("E_ANCHOR")).toBeNull();
  });

  it("bağlama sonrası tazeleme panel değiştiyse yeni olayın bağlantılarını yazmaz", async () => {
    const linking = deferred<unknown>();
    setup({
      events: async () => [E_DRAFT, E_UNKNOWN],
      anchors: async () => [A1],
      sources: async (eventId) => (eventId === "e1" ? [] : [LINK]),
    });
    await screen.findByText(E_DRAFT.title);
    const row1 = rowOf(E_DRAFT.title);
    const row2 = rowOf(E_UNKNOWN.title);

    fireEvent.click(within(row1).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(byId<HTMLSelectElement>("src-e1").value).toBe(A1.id));

    mocks.eventLinkSource.mockReturnValue(linking.promise);
    fireEvent.click(within(row1).getByRole("button", { name: t("timeline.attach") }));
    await waitFor(() => expect(mocks.eventLinkSource).toHaveBeenCalledTimes(1));

    fireEvent.click(within(row2).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(row2.querySelector("li")).not.toBeNull());

    await act(async () => {
      linking.resolve({});
      await linking.promise;
    });

    // e1 için yapılan tazeleme ([]) e2 panelini boşaltmaz.
    await waitFor(() => expect(mocks.eventSources).toHaveBeenCalledTimes(3));
    expect(row2.querySelectorAll("li")).toHaveLength(1);
    expect(row1.querySelector("li")).toBeNull();
  });

  it("kaldırma sonrası tazeleme panel değiştiyse yeni olayın bağlantılarını silmez", async () => {
    const unlinking = deferred<void>();
    setup({
      events: async () => [E_DRAFT, E_UNKNOWN],
      anchors: async () => [A1],
      sources: async (eventId) => (eventId === "e1" ? [LINK_B] : [LINK]),
    });
    await screen.findByText(E_DRAFT.title);
    const row1 = rowOf(E_DRAFT.title);
    const row2 = rowOf(E_UNKNOWN.title);

    fireEvent.click(within(row1).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(row1.querySelector("li")).not.toBeNull());

    mocks.eventUnlinkSource.mockReturnValue(unlinking.promise);
    const removeLink = within(row1.querySelector("li") as HTMLElement).getByRole("button", {
      name: t("timeline.removeLink"),
    });
    fireEvent.click(removeLink);
    await waitFor(() => expect(mocks.eventUnlinkSource).toHaveBeenCalledWith("link-b"));

    fireEvent.click(within(row2).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(row2.querySelector("li")).not.toBeNull());

    await act(async () => {
      unlinking.resolve();
      await unlinking.promise;
    });

    await waitFor(() => expect(mocks.eventSources).toHaveBeenCalledTimes(3));
    expect(row2.querySelectorAll("li")).toHaveLength(1);
    const item = row2.querySelector("li") as HTMLElement;
    expect(item.textContent).toContain(t("timeline.relationKind.contradicts"));
    expect(item.textContent).not.toContain(t("timeline.relationKind.supports"));
  });

  it("aynı olay paneli yeniden açılınca eski isteğin geç yanıtı yenisini ezmez", async () => {
    const firstOpen = deferred<EventSourceRow[]>();
    const secondOpen = deferred<EventSourceRow[]>();
    let calls = 0;
    setup({
      events: async () => [E_UNKNOWN],
      anchors: async () => [A1],
      sources: async () => {
        calls += 1;
        return calls === 1 ? firstOpen.promise : secondOpen.promise;
      },
    });
    await screen.findByText(E_UNKNOWN.title);
    const row = rowOf(E_UNKNOWN.title);

    fireEvent.click(within(row).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(mocks.eventSources).toHaveBeenCalledTimes(1));
    fireEvent.click(within(row).getByRole("button", { name: t("timeline.closeSources") }));
    fireEvent.click(within(row).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(mocks.eventSources).toHaveBeenCalledTimes(2));

    await act(async () => {
      secondOpen.resolve([LINK]);
      await secondOpen.promise;
    });
    expect((row.querySelector("li") as HTMLElement).textContent).toContain(
      t("timeline.relationKind.contradicts"),
    );

    await act(async () => {
      firstOpen.resolve([LINK_B]);
      await firstOpen.promise;
    });

    // Eski açılışın geç yanıtı yeni paneli "destekler" bağlantısıyla ezmez.
    const item = row.querySelector("li") as HTMLElement;
    expect(item.textContent).toContain(t("timeline.relationKind.contradicts"));
    expect(item.textContent).not.toContain(t("timeline.relationKind.supports"));
  });

  it("onay uçuşta kilitlidir: çift tık tek yazma", async () => {
    const confirming = deferred<EventRow>();
    setup({ events: async () => [E_DRAFT] });
    await screen.findByText(E_DRAFT.title);
    mocks.eventConfirm.mockReturnValue(confirming.promise);

    const row = rowOf(E_DRAFT.title);
    const button = within(row).getByRole("button", { name: t("timeline.confirm") });
    fireEvent.click(button);
    expect(button).toHaveProperty("disabled", true);
    fireEvent.click(button);
    expect(mocks.eventConfirm).toHaveBeenCalledTimes(1);

    await act(async () => {
      confirming.resolve({ ...E_DRAFT, status: "confirmed" });
      await confirming.promise;
    });
    await waitFor(() => expect(mocks.eventList).toHaveBeenCalledTimes(2));
  });

  it("silme uçuşta kilitlidir: çift tık tek yazma", async () => {
    const deleting = deferred<void>();
    setup({ events: async () => [E_DRAFT] });
    await screen.findByText(E_DRAFT.title);
    mocks.eventDelete.mockReturnValue(deleting.promise);

    const row = rowOf(E_DRAFT.title);
    const button = within(row).getByRole("button", { name: t("timeline.remove") });
    fireEvent.click(button);
    expect(button).toHaveProperty("disabled", true);
    fireEvent.click(button);
    expect(mocks.eventDelete).toHaveBeenCalledTimes(1);

    await act(async () => {
      deleting.resolve();
      await deleting.promise;
    });
    await waitFor(() => expect(mocks.eventList).toHaveBeenCalledTimes(2));
  });

  it("bağlama uçuşta kilitlidir: çift tık tek yazma", async () => {
    const linking = deferred<unknown>();
    setup({
      events: async () => [E_UNKNOWN],
      anchors: async () => [A1],
      sources: async () => [],
    });
    await screen.findByText(E_UNKNOWN.title);
    const row = rowOf(E_UNKNOWN.title);

    fireEvent.click(within(row).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(byId<HTMLSelectElement>("src-e2").value).toBe(A1.id));

    mocks.eventLinkSource.mockReturnValue(linking.promise);
    const button = within(row).getByRole("button", { name: t("timeline.attach") });
    fireEvent.click(button);
    expect(button).toHaveProperty("disabled", true);
    fireEvent.click(button);
    expect(mocks.eventLinkSource).toHaveBeenCalledTimes(1);

    await act(async () => {
      linking.resolve({});
      await linking.promise;
    });
    await waitFor(() => expect(mocks.eventSources).toHaveBeenCalledTimes(2));
  });

  it("kaldırma uçuşta kilitlidir: çift tık tek yazma", async () => {
    const unlinking = deferred<void>();
    let sourceCalls = 0;
    setup({
      events: async () => [E_UNKNOWN],
      anchors: async () => [A1],
      sources: async () => {
        sourceCalls += 1;
        return sourceCalls === 1 ? [LINK] : [];
      },
    });
    await screen.findByText(E_UNKNOWN.title);
    const row = rowOf(E_UNKNOWN.title);

    fireEvent.click(within(row).getByRole("button", { name: t("timeline.sources") }));
    await waitFor(() => expect(row.querySelector("li")).not.toBeNull());

    mocks.eventUnlinkSource.mockReturnValue(unlinking.promise);
    const button = within(row.querySelector("li") as HTMLElement).getByRole("button", {
      name: t("timeline.removeLink"),
    });
    fireEvent.click(button);
    expect(button).toHaveProperty("disabled", true);
    fireEvent.click(button);
    expect(mocks.eventUnlinkSource).toHaveBeenCalledTimes(1);

    await act(async () => {
      unlinking.resolve();
      await unlinking.promise;
    });
    await waitFor(() => expect(mocks.eventSources).toHaveBeenCalledTimes(2));
    expect(row.querySelector("li")).toBeNull();
  });

  it("ekleme uçuşunda form alanları kilitlidir (sessiz kayıp yok)", async () => {
    const created = deferred<EventRow>();
    setup({ events: async () => [] });
    await caseLoaded();
    mocks.eventCreate.mockReturnValue(created.promise);

    fireEvent.change(byId<HTMLInputElement>("tl-title"), { target: { value: "Yeni olay" } });
    fireEvent.click(screen.getByRole("button", { name: t("timeline.addEvent") }));
    await waitFor(() => expect(mocks.eventCreate).toHaveBeenCalledTimes(1));

    // Uçuşta yazılabilseydi yanıt geldiğinde taslak temizlenip metin sessizce silinirdi.
    expect(byId<HTMLInputElement>("tl-title")).toHaveProperty("disabled", true);
    expect(byId<HTMLInputElement>("tl-date")).toHaveProperty("disabled", true);
    expect(byId<HTMLSelectElement>("tl-precision")).toHaveProperty("disabled", true);
    expect(byId<HTMLSelectElement>("tl-role")).toHaveProperty("disabled", true);

    await act(async () => {
      created.resolve(eventRow({ id: "e-new", title: "Yeni olay" }));
      await created.promise;
    });
    expect(byId<HTMLInputElement>("tl-title").value).toBe("");
    await waitFor(() => expect(mocks.eventList).toHaveBeenCalledTimes(2));
  });

  it("kaydetme uçuşunda açılan başka satırın düzenleyicisi kapanmaz", async () => {
    const updating = deferred<EventRow>();
    setup({ events: async () => [E_DRAFT, E_UNKNOWN] });
    await screen.findByText(E_DRAFT.title);
    mocks.eventUpdate.mockReturnValue(updating.promise);

    const row1 = rowOf(E_DRAFT.title);
    fireEvent.click(within(row1).getByRole("button", { name: t("timeline.edit") }));
    fireEvent.change(byId<HTMLInputElement>("edit-title-e1"), { target: { value: "Güncel" } });
    fireEvent.click(within(row1).getByRole("button", { name: t("common.save") }));
    await waitFor(() => expect(mocks.eventUpdate).toHaveBeenCalledTimes(1));

    const row2 = rowOf(E_UNKNOWN.title);
    fireEvent.click(within(row2).getByRole("button", { name: t("timeline.edit") }));
    expect(document.getElementById("edit-title-e2")).not.toBeNull();
    expect(byId<HTMLInputElement>("edit-title-e2")).toHaveProperty("disabled", true);
    expect(byId<HTMLInputElement>("edit-desc-e2")).toHaveProperty("disabled", true);

    await act(async () => {
      updating.resolve({ ...E_DRAFT, title: "Güncel" });
      await updating.promise;
    });

    // Geç yanıt, kullanıcının açtığı başka satırın düzenleyicisini kapatmaz.
    expect(document.getElementById("edit-title-e2")).not.toBeNull();
    expect(document.getElementById("edit-title-e1")).toBeNull();
  });
});
