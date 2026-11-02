// @vitest-environment jsdom
//
// Kontrol listesi ekranı davranış testi. `checklist` lib testleri kural/özet
// metinlerini doğrular; burada 517 satırlık ekran gerçekten render edilip
// F04/F07 kararlarının arayüzde **uygulandığı** ölçülür. Sabitlenen davranışlar:
//
// 1. Dosya listesi sunucudan gelir, ilk dosya seçilir; belgeler, şablon kataloğu
//    ve dosya maddeleri sunucudan yüklenir; özet/rozet sayıları sunucu listesinden
//    hesaplanır, katalog künyesi (lisans, sürüm, hash, kaynak) birebir yazılır.
// 2. Sözlükte olmayan durum **uydurulmaz**: bilinmeyen durum "Açık" olarak
//    gösterilir ve ham anahtar ekrana sızmaz.
// 3. Kural kapısı: "Bulundu" en az bir belge ister, "İncelenen kapsamda
//    bulunamadı"/"Uygulanamaz" gerekçe ister; kural varken "Kaydet" kapalıdır ve
//    kural metni düğme `title`ında da taşınır.
// 4. Durum değişince belge bağları temizlenir (bağ yalnız "Bulundu"da anlamlı).
// 5. Kaydetme **okunan satır sürümünü** taşır, tam sözleşmeyle gider, listeyi
//    tazeler ve bildirimi madde metniyle yazar.
// 6. Şablon kopyalama sonucu (created true/false) ayrı bildirimler üretir; işlem
//    sürerken düğmeler kilitlenir ve liste tazelenir.
// 7. Listede görünmeyen belge bağı dürüstçe gösterilir ve kaldırılabilir.
// 8. **Ölçülen kusur (INV-05):** dosya değişince önceki dosyanın maddeleri,
//    kataloğu, belgeleri ve bildirimi ekranda kalıyordu → askıdaki istekte yanlış
//    dosyanın verisi okunuyor, "Kaydet" önceki dosyanın maddesini yazabiliyordu.
// 9. **Ölçülen kusur:** düşen yükleme önceki dosyanın verisini bırakıyordu.
// 10. **Ölçülen kusur:** çakışma sonrası "Güncel kaydı yükle" hatayı silmiyordu →
//     başarılı tazelemeden sonra kırmızı `E_ROW_VERSION` ekranda kalıyordu.
// 11. **Ölçülen kusur:** yeni işlem başlarken önceki yeşil bildirim silinmiyordu →
//     başarısız kayıtla birlikte "«…» güncellendi." aynı ekranda görünüyordu.

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../lib/i18n";
import type {
  CaseChecklistItemRow,
  CaseRow,
  ChecklistTemplateRef,
  ChecklistTemplateRow,
  DocumentRow,
} from "../lib/api";

const mocks = vi.hoisted(() => ({
  caseList: vi.fn(),
  documentList: vi.fn(),
  checklistTemplates: vi.fn(),
  checklistItems: vi.fn(),
  checklistCopy: vi.fn(),
  checklistItemState: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

const { Checklists } = await import("./Checklists");

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

function documentRow(overrides: Partial<DocumentRow> & { id: string; displayName: string }): DocumentRow {
  return {
    caseId: "p1",
    documentType: "contract",
    status: "imported",
    currentVersionId: "v1",
    declaredDocumentDate: null,
    createdAt: ISO,
    updatedAt: ISO,
    rowVersion: 1,
    ...overrides,
  };
}

const D1 = documentRow({ id: "d1", displayName: "Kira sözleşmesi.pdf" });
const D2 = documentRow({ id: "d2", displayName: "Tahliye taahhüdü.pdf" });

const TPL1: ChecklistTemplateRow = {
  id: "tpl:kira:v2",
  title: "Kira kontrol listesi",
  practiceArea: "Kira hukuku",
  version: 2,
  author: "İçerik ekibi",
  reviewDate: "2026-09-01",
  licenseId: "CC-BY-4.0",
  licenseUrl: null,
  sourceUrl: "https://ornek.test/kira",
  contentHash: "a".repeat(64),
  isCommunity: false,
  itemCount: 3,
  copiedItemCount: 0,
};

/** Alanı, lisansı ve kaynağı olmayan, kopyası bulunan şablon. */
const TPL2: ChecklistTemplateRow = {
  id: "tpl:genel:v1",
  title: "Genel ön inceleme",
  practiceArea: null,
  version: 1,
  author: "Kütüphane",
  reviewDate: null,
  licenseId: null,
  licenseUrl: null,
  sourceUrl: null,
  contentHash: "abcdefghijklmnop",
  isCommunity: true,
  itemCount: 1,
  copiedItemCount: 1,
};

const TPL_REF: ChecklistTemplateRef = {
  id: TPL1.id,
  title: TPL1.title,
  version: TPL1.version,
  licenseId: TPL1.licenseId,
  licenseUrl: TPL1.licenseUrl,
  author: TPL1.author,
  reviewDate: TPL1.reviewDate,
  contentHash: TPL1.contentHash,
};

function itemRow(overrides: Partial<CaseChecklistItemRow> & { id: string; itemText: string }): CaseChecklistItemRow {
  return {
    caseId: "p1",
    templateVersionId: null,
    state: "open",
    linkedDocumentIds: [],
    linkedDocuments: [],
    note: "",
    ordinal: 0,
    category: "",
    createdAt: ISO,
    updatedAt: ISO,
    rowVersion: 1,
    template: null,
    ...overrides,
  };
}

/** Açık madde; künye "1. madde · genel". */
const I1 = itemRow({ id: "i1", itemText: "Kira sözleşmesi imzalı mı?" });

/** Bulundu; bağı listede olmayan bir belgeye işaret ediyor (bayat bağ). */
const I2 = itemRow({
  id: "i2",
  itemText: "Tahliye taahhüdü var mı?",
  state: "present",
  linkedDocumentIds: ["d9"],
  linkedDocuments: [{ id: "d9", displayName: "Silinmiş belge.pdf" }],
  ordinal: 1,
  category: "Belgeler",
  rowVersion: 2,
  template: TPL_REF,
});

/** İncelenen kapsamda bulunamadı; gerekçe dolu. */
const I3 = itemRow({
  id: "i3",
  itemText: "Tebligat belgesi var mı?",
  state: "missing",
  note: "2020-2024 arası tebligatlar incelendi",
  ordinal: 2,
  rowVersion: 1,
});

type SetupOptions = {
  cases?: CaseRow[];
  documents?: (caseId: string) => Promise<DocumentRow[]>;
  templates?: (caseId: string) => Promise<ChecklistTemplateRow[]>;
  items?: (caseId: string) => Promise<CaseChecklistItemRow[]>;
};

function setup(options: SetupOptions = {}) {
  mocks.caseList.mockResolvedValue(options.cases ?? [P1, P2]);
  mocks.documentList.mockImplementation(options.documents ?? (async () => [D1, D2]));
  mocks.checklistTemplates.mockImplementation(options.templates ?? (async () => [TPL1, TPL2]));
  mocks.checklistItems.mockImplementation(options.items ?? (async () => [I1, I2, I3]));
  mocks.checklistCopy.mockResolvedValue({
    created: true,
    itemCount: 3,
    templateId: TPL1.id,
    templateTitle: TPL1.title,
    templateVersion: 2,
    licenseId: "CC-BY-4.0",
    contentHash: TPL1.contentHash,
  });
  mocks.checklistItemState.mockResolvedValue(I1);
  render(<Checklists />);
}

/** Madde satırı: madde metninden üç seviye yukarıdaki çerçeveli kutu. */
function itemRowOf(text: string): HTMLElement {
  const node = screen.getByText(text);
  const row = node.parentElement?.parentElement?.parentElement;
  if (!row) throw new Error(`"${text}" maddesi bulunamadı`);
  return row as HTMLElement;
}

/** Katalog satırı: başlık `<strong>`inden üç seviye yukarıdaki çerçeveli kutu. */
function catalogRowOf(title: string): HTMLElement {
  const node = screen.getByText(title);
  const row = node.parentElement?.parentElement?.parentElement;
  if (!row) throw new Error(`"${title}" şablonu bulunamadı`);
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

describe("kontrol listesi ekranı davranışı", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("dosya, belge, şablon ve maddeleri sunucudan yükler; özet ve künyeyi yazar", async () => {
    setup();

    expect(await screen.findByText(I1.itemText)).toBeTruthy();
    expect(mocks.caseList).toHaveBeenCalledWith(false);
    expect(byId<HTMLSelectElement>("checklist-case").value).toBe("p1");
    expect(mocks.documentList).toHaveBeenCalledWith("p1");
    expect(mocks.checklistTemplates).toHaveBeenCalledWith("p1");
    expect(mocks.checklistItems).toHaveBeenCalledWith("p1");

    // Sayılar sunucu listesinden hesaplanır; uydurulmaz.
    expect(
      screen.getByText(
        t("checklists.summary", {
          total: 3,
          open: 1,
          present: 1,
          missing: 1,
          notApplicable: 0,
        }),
      ),
    ).toBeTruthy();
    expect(screen.getByText(t("checklists.itemsSub", { count: 3 }))).toBeTruthy();
    expect(screen.getByText(t("checklists.foundBadge", { count: 1 }))).toBeTruthy();
    expect(screen.getByText(t("checklists.missingBadge", { count: 1 }))).toBeTruthy();
    expect(screen.getByText(t("checklists.notApplicableBadge", { count: 0 }))).toBeTruthy();

    // Katalog künyesi: alan, lisans, sürüm, hash ve kaynak.
    const tpl1 = catalogRowOf(TPL1.title);
    expect(tpl1.textContent).toContain(TPL1.practiceArea!);
    expect(tpl1.textContent).toContain("sürüm 2");
    expect(tpl1.textContent).toContain("CC-BY-4.0");
    expect(tpl1.textContent).toContain("gözden geçirme 2026-09-01");
    expect(tpl1.textContent).toContain(TPL1.author);
    expect(tpl1.textContent).toContain(t("checklists.templateItems", { count: 3 }));
    expect(tpl1.textContent).toContain(`${"a".repeat(12)}…`);
    expect(tpl1.textContent).toContain(t("checklists.sourceLabel"));
    expect(tpl1.textContent).toContain(TPL1.sourceUrl!);
    expect(
      within(tpl1).getByRole("button", { name: t("checklists.copyToCase") }),
    ).toBeTruthy();

    // Eksik alan uydurulmaz; kopya rozeti ve "doğrula" etiketi sunucu sayısından.
    const tpl2 = catalogRowOf(TPL2.title);
    expect(tpl2.textContent).toContain(t("checklists.practiceUnknown"));
    expect(tpl2.textContent).toContain(t("checklists.licenseUnknown"));
    expect(tpl2.textContent).not.toContain(t("checklists.sourceLabel"));
    expect(tpl2.textContent).toContain(t("checklists.copiedCount", { count: 1 }));
    expect(
      within(tpl2).getByRole("button", { name: t("checklists.verifyCopy") }),
    ).toBeTruthy();

    // Madde künyeleri: sıra numarası, kategori, şablon sürümü ve hash.
    const i1Row = itemRowOf(I1.itemText);
    expect(i1Row.textContent).toContain(t("checklists.itemMeta", { ordinal: 1, category: "genel" }));
    expect(i1Row.textContent).toContain(t("checklists.noTemplateMeta"));
    expect(i1Row.querySelector(".badge")!.textContent).toBe(t("checklists.stateLabel.open"));

    const i2Row = itemRowOf(I2.itemText);
    expect(i2Row.textContent).toContain(
      t("checklists.itemMeta", { ordinal: 2, category: "Belgeler" }),
    );
    expect(i2Row.textContent).toContain(
      t("checklists.templateMeta", {
        title: TPL_REF.title,
        version: 2,
        license: "CC-BY-4.0",
      }),
    );
    expect(i2Row.textContent).toContain(`${"a".repeat(12)}…`);
    expect(i2Row.querySelector(".badge")!.textContent).toBe(t("checklists.stateLabel.present"));

    const i3Row = itemRowOf(I3.itemText);
    expect(i3Row.querySelector(".badge")!.textContent).toBe(t("checklists.stateLabel.missing"));
    expect(screen.getByText(t("checklists.missingScopeNote"))).toBeTruthy();

    // Belge listesi yalnız "Bulundu" maddesinde çizilir.
    expect(within(i2Row).getByRole("checkbox", { name: D1.displayName })).toBeTruthy();
    expect(within(i2Row).getByRole("checkbox", { name: D2.displayName })).toBeTruthy();
    expect(within(i1Row).queryByRole("checkbox")).toBeNull();
  });

  it("dosya yokken sessiz boş durum yazar; yükleme isteği atılmaz", async () => {
    setup({ cases: [] });

    expect(await screen.findByText(t("checklists.catalogEmpty"))).toBeTruthy();
    expect(screen.getByText(t("checklists.empty"))).toBeTruthy();
    expect(
      screen.getByText(
        t("checklists.summary", {
          total: 0,
          open: 0,
          present: 0,
          missing: 0,
          notApplicable: 0,
        }),
      ),
    ).toBeTruthy();
    expect(byId<HTMLSelectElement>("checklist-case").value).toBe("");
    expect(within(byId<HTMLSelectElement>("checklist-case")).getByText(t("common.noCase"))).toBeTruthy();
    expect(mocks.documentList).not.toHaveBeenCalled();
    expect(mocks.checklistItems).not.toHaveBeenCalled();
  });

  it("sözlükte olmayan durumu uydurmaz; ham anahtar ekrana sızmaz", async () => {
    setup({
      items: async () => [{ ...I1, state: "archived" }],
    });

    const row = await screen.findByText(I1.itemText);
    const itemRowElement = row.parentElement!.parentElement!.parentElement as HTMLElement;
    expect(itemRowElement.textContent).toContain(t("checklists.stateLabel.open"));
    expect(itemRowElement.textContent).not.toContain("archived");
    expect(byId<HTMLSelectElement>("state-i1").value).toBe("open");
    // Durum seçimi de yalnız bilinen dört değeri sunar; ham anahtar seçenek olmaz.
    const options = Array.from(byId<HTMLSelectElement>("state-i1").options).map((o) => o.value);
    expect(options).toEqual(["open", "present", "missing", "not_applicable"]);
  });

  it("bulundu kuralı belge seçmeden kaydetmeyi kapatır; seçim kuralı kaldırır", async () => {
    setup();
    await screen.findByText(I1.itemText);
    const row = itemRowOf(I1.itemText);

    fireEvent.change(byId<HTMLSelectElement>("state-i1"), { target: { value: "present" } });

    const save = within(row).getByRole("button", { name: t("common.save") });
    expect(save).toHaveProperty("disabled", true);
    expect(save.getAttribute("title")).toBe(t("checklists.rule.presentNeedsDocument"));
    expect(within(row).getByText(t("checklists.rule.presentNeedsDocument"))).toBeTruthy();
    expect(within(row).getByText(t("checklists.linkedDocuments", { count: 0 }))).toBeTruthy();

    fireEvent.click(within(row).getByRole("checkbox", { name: D1.displayName }));

    expect(within(row).queryByText(t("checklists.rule.presentNeedsDocument"))).toBeNull();
    expect(within(row).getByRole("button", { name: t("common.save") })).toHaveProperty(
      "disabled",
      false,
    );
    expect(within(row).getByText(t("checklists.linkedDocuments", { count: 1 }))).toBeTruthy();
  });

  it("kaydetme okunan satır sürümünü taşır, listeyi tazeler ve madde metniyle bildirir", async () => {
    let calls = 0;
    setup({
      items: async () => {
        calls += 1;
        if (calls === 1) return [I1, I3];
        return [
          {
            ...I1,
            state: "present",
            linkedDocumentIds: ["d1"],
            linkedDocuments: [{ id: "d1", displayName: D1.displayName }],
            rowVersion: 2,
          },
          I3,
        ];
      },
    });
    await screen.findByText(I1.itemText);
    const row = itemRowOf(I1.itemText);

    fireEvent.change(byId<HTMLSelectElement>("state-i1"), { target: { value: "present" } });
    fireEvent.click(within(row).getByRole("checkbox", { name: D1.displayName }));
    fireEvent.click(within(row).getByRole("button", { name: t("common.save") }));

    await waitFor(() =>
      expect(mocks.checklistItemState).toHaveBeenCalledWith("i1", "present", ["d1"], "", 1),
    );
    await waitFor(() => expect(mocks.checklistItems).toHaveBeenCalledTimes(2));
    expect(
      await screen.findByText(t("checklists.noticeUpdated", { item: I1.itemText })),
    ).toBeTruthy();
    // Tazelenen kayıt sunucudan gelir: rozet artık "Bulundu".
    await waitFor(() =>
      expect(itemRowOf(I1.itemText).querySelector(".badge")!.textContent).toBe(
        t("checklists.stateLabel.present"),
      ),
    );
  });

  it("durum 'bulundu'dan çıkınca belge bağlarını temizler; gerekçe ister", async () => {
    setup();
    await screen.findByText(I2.itemText);
    const row = itemRowOf(I2.itemText);

    expect(within(row).getByRole("checkbox", { name: D1.displayName })).toBeTruthy();
    fireEvent.change(byId<HTMLSelectElement>("state-i2"), { target: { value: "missing" } });

    expect(within(row).queryByRole("checkbox")).toBeNull();
    expect(within(row).getByText(t("checklists.rule.missingNeedsScope"))).toBeTruthy();
    expect(within(row).getByRole("button", { name: t("common.save") })).toHaveProperty(
      "disabled",
      true,
    );
    expect(byId<HTMLLabelElement>("note-i2").closest("div")!.textContent).toContain(
      t("checklists.noteMissing"),
    );

    fireEvent.change(byId<HTMLInputElement>("note-i2"), {
      target: { value: "2024 aralığı incelendi" },
    });
    fireEvent.click(within(row).getByRole("button", { name: t("common.save") }));

    await waitFor(() =>
      expect(mocks.checklistItemState).toHaveBeenCalledWith(
        "i2",
        "missing",
        [],
        "2024 aralığı incelendi",
        2,
      ),
    );
  });

  it("uygulanamaz durumu gerekçe ister; etiket ve yer tutucu duruma göre değişir", async () => {
    setup();
    await screen.findByText(I2.itemText);
    const row = itemRowOf(I2.itemText);

    fireEvent.change(byId<HTMLSelectElement>("state-i2"), { target: { value: "not_applicable" } });

    expect(within(row).getByText(t("checklists.rule.notApplicableNeedsReason"))).toBeTruthy();
    expect(byId<HTMLInputElement>("note-i2").placeholder).toBe(t("checklists.notePlaceholder"));
    expect(row.textContent).toContain(t("checklists.noteNotApplicable"));
    expect(within(row).getByRole("button", { name: t("common.save") })).toHaveProperty(
      "disabled",
      true,
    );

    fireEvent.change(byId<HTMLInputElement>("note-i2"), {
      target: { value: "Tahliye istenmiyor" },
    });
    expect(within(row).queryByText(t("checklists.rule.notApplicableNeedsReason"))).toBeNull();
    expect(within(row).getByRole("button", { name: t("common.save") })).toHaveProperty(
      "disabled",
      false,
    );
  });

  it("şablon kopyalama sözleşmesini taşır, düğmeleri kilitler ve bildirimi yazar", async () => {
    const copy = deferred<{
      created: boolean;
      itemCount: number;
      templateId: string;
      templateTitle: string;
      templateVersion: number;
      licenseId: string | null;
      contentHash: string;
    }>();
    let templateCalls = 0;
    setup({
      templates: async () => {
        templateCalls += 1;
        return templateCalls === 1 ? [TPL1, TPL2] : [{ ...TPL1, copiedItemCount: 3 }, TPL2];
      },
    });
    mocks.checklistCopy.mockReturnValue(copy.promise);
    await screen.findByText(TPL1.title);

    const tpl1 = catalogRowOf(TPL1.title);
    const tpl2 = catalogRowOf(TPL2.title);
    fireEvent.click(within(tpl1).getByRole("button", { name: t("checklists.copyToCase") }));

    await waitFor(() => expect(mocks.checklistCopy).toHaveBeenCalledWith("p1", TPL1.id));
    // İşlem sürerken kopyalama düğmeleri kilitli.
    expect(within(tpl1).getByRole("button", { name: t("checklists.copyToCase") })).toHaveProperty(
      "disabled",
      true,
    );
    expect(within(tpl2).getByRole("button", { name: t("checklists.verifyCopy") })).toHaveProperty(
      "disabled",
      true,
    );

    copy.resolve({
      created: true,
      itemCount: 3,
      templateId: TPL1.id,
      templateTitle: TPL1.title,
      templateVersion: 2,
      licenseId: "CC-BY-4.0",
      contentHash: TPL1.contentHash,
    });

    expect(
      await screen.findByText(
        t("checklists.noticeCopied", {
          title: TPL1.title,
          version: 2,
          count: 3,
          license: "CC-BY-4.0",
        }),
      ),
    ).toBeTruthy();
    await waitFor(() => expect(mocks.checklistItems).toHaveBeenCalledTimes(2));
    // Tazelenen katalog "kopyalandı" rozetini ve "doğrula" etiketini gösterir.
    await waitFor(() =>
      expect(
        within(catalogRowOf(TPL1.title)).getByRole("button", { name: t("checklists.verifyCopy") }),
      ).toBeTruthy(),
    );
    expect(catalogRowOf(TPL1.title).textContent).toContain(
      t("checklists.copiedCount", { count: 3 }),
    );
  });

  it("zaten kopyalanmış şablonda yinelenen öğe üretilmediğini söyler", async () => {
    setup();
    mocks.checklistCopy.mockResolvedValue({
      created: false,
      itemCount: 3,
      templateId: TPL1.id,
      templateTitle: TPL1.title,
      templateVersion: 2,
      licenseId: "CC-BY-4.0",
      contentHash: TPL1.contentHash,
    });
    await screen.findByText(TPL1.title);

    fireEvent.click(
      within(catalogRowOf(TPL1.title)).getByRole("button", { name: t("checklists.copyToCase") }),
    );

    expect(
      await screen.findByText(t("checklists.noticeAlready", { title: TPL1.title })),
    ).toBeTruthy();
    expect(mocks.checklistCopy).toHaveBeenCalledWith("p1", TPL1.id);
  });

  it("başarısız kopyalama önceki kopya bildirimini ekranda bırakmaz", async () => {
    setup();
    await screen.findByText(TPL1.title);

    const copiedNotice = t("checklists.noticeCopied", {
      title: TPL1.title,
      version: 2,
      count: 3,
      license: "CC-BY-4.0",
    });
    fireEvent.click(
      within(catalogRowOf(TPL1.title)).getByRole("button", { name: t("checklists.copyToCase") }),
    );
    expect(await screen.findByText(copiedNotice)).toBeTruthy();

    mocks.checklistCopy.mockRejectedValue({
      code: "E_COPY",
      message: "Şablon kopyalanamadı.",
      retryable: true,
    });
    fireEvent.click(
      within(catalogRowOf(TPL1.title)).getByRole("button", { name: t("checklists.copyToCase") }),
    );

    expect(await screen.findByText("E_COPY")).toBeTruthy();
    // Yeşil "kopyalandı" satırı kırmızı hatanın yanında kalmaz.
    expect(screen.queryByText(copiedNotice)).toBeNull();
  });

  it("listede görünmeyen belge bağını adıyla gösterir ve kaldırır", async () => {
    setup();
    await screen.findByText(I2.itemText);
    const row = itemRowOf(I2.itemText);

    expect(row.textContent).toContain(t("checklists.staleLinks"));
    expect(row.textContent).toContain("Silinmiş belge.pdf");
    fireEvent.click(within(row).getByRole("button", { name: t("checklists.removeLink") }));

    expect(within(row).queryByText(t("checklists.staleLinks"))).toBeNull();
    // Bağ kalmadığı için "Bulundu" kuralı devreye girer.
    expect(within(row).getByText(t("checklists.rule.presentNeedsDocument"))).toBeTruthy();
    expect(within(row).getByText(t("checklists.linkedDocuments", { count: 0 }))).toBeTruthy();

    fireEvent.click(within(row).getByRole("checkbox", { name: D1.displayName }));
    fireEvent.click(within(row).getByRole("button", { name: t("common.save") }));
    await waitFor(() =>
      expect(mocks.checklistItemState).toHaveBeenCalledWith("i2", "present", ["d1"], "", 2),
    );
  });

  it("çakışma sonrası 'Güncel kaydı yükle' hatayı siler ve taze kaydı gösterir", async () => {
    let calls = 0;
    setup({
      items: async () => {
        calls += 1;
        if (calls === 1) return [I1, I3];
        return [{ ...I1, note: "Sunucudaki not", rowVersion: 7 }, I3];
      },
    });
    await screen.findByText(I1.itemText);
    mocks.checklistItemState.mockRejectedValue({
      code: "E_ROW_VERSION",
      message: "Kayıt başka pencerede değişti.",
      retryable: false,
    });

    const row = itemRowOf(I1.itemText);
    fireEvent.change(byId<HTMLInputElement>("note-i1"), { target: { value: "çakışan not" } });
    fireEvent.click(within(row).getByRole("button", { name: t("common.save") }));

    expect(await screen.findByText("E_ROW_VERSION")).toBeTruthy();
    fireEvent.click(await screen.findByRole("button", { name: t("checklists.reload") }));

    await waitFor(() => expect(mocks.checklistItems).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.queryByText("E_ROW_VERSION")).toBeNull());
    expect(screen.queryByRole("button", { name: t("checklists.reload") })).toBeNull();
    expect(byId<HTMLInputElement>("note-i1").value).toBe("Sunucudaki not");
  });

  it("başarısız kayıt önceki yeşil bildirimi ekranda bırakmaz", async () => {
    setup({ items: async () => [I1, I3] });
    await screen.findByText(I1.itemText);

    mocks.checklistItemState.mockResolvedValue({ ...I1, note: "ilk not" });
    fireEvent.change(byId<HTMLInputElement>("note-i1"), { target: { value: "ilk not" } });
    fireEvent.click(within(itemRowOf(I1.itemText)).getByRole("button", { name: t("common.save") }));

    const notice = await screen.findByText(
      t("checklists.noticeUpdated", { item: I1.itemText }),
    );
    expect(notice).toBeTruthy();

    mocks.checklistItemState.mockRejectedValue({
      code: "E_ROW_VERSION",
      message: "Kayıt başka pencerede değişti.",
      retryable: false,
    });
    fireEvent.change(byId<HTMLInputElement>("note-i3"), { target: { value: "yeni kapsam" } });
    fireEvent.click(within(itemRowOf(I3.itemText)).getByRole("button", { name: t("common.save") }));

    expect(await screen.findByText("E_ROW_VERSION")).toBeTruthy();
    // Yeşil "güncellendi" bildirimi kırmızı hatanın yanında kalmaz.
    expect(
      screen.queryByText(t("checklists.noticeUpdated", { item: I1.itemText })),
    ).toBeNull();
  });

  it("dosya değişince önceki dosyanın maddeleri, kataloğu, belgeleri ve bildirimi kalkar (INV-05)", async () => {
    const p2Docs = deferred<DocumentRow[]>();
    const p2Templates = deferred<ChecklistTemplateRow[]>();
    const p2Items = deferred<CaseChecklistItemRow[]>();

    setup({
      documents: async (caseId) => (caseId === "p1" ? [D1] : p2Docs.promise),
      templates: async (caseId) => (caseId === "p1" ? [TPL1] : p2Templates.promise),
      items: async (caseId) => (caseId === "p1" ? [I1] : p2Items.promise),
    });

    await screen.findByText(I1.itemText);
    // p1'de bir bildirim bırak (kopyalama) — dosya değişince kalkmalı.
    fireEvent.click(
      within(catalogRowOf(TPL1.title)).getByRole("button", { name: t("checklists.copyToCase") }),
    );
    const notice = await screen.findByText(
      t("checklists.noticeCopied", {
        title: TPL1.title,
        version: 2,
        count: 3,
        license: "CC-BY-4.0",
      }),
    );
    expect(notice).toBeTruthy();

    fireEvent.change(byId<HTMLSelectElement>("checklist-case"), { target: { value: "p2" } });
    await waitFor(() => expect(mocks.checklistItems).toHaveBeenCalledWith("p2"));

    // Yeni dosyanın verisi gelmeden önce eski dosyanın verisi ve bildirimi kalkar.
    expect(screen.queryByText(I1.itemText)).toBeNull();
    expect(screen.queryByText(TPL1.title)).toBeNull();
    expect(screen.queryByText(D1.displayName)).toBeNull();
    expect(screen.queryByText(t("checklists.noticeCopied", {
      title: TPL1.title,
      version: 2,
      count: 3,
      license: "CC-BY-4.0",
    }))).toBeNull();
    expect(
      screen.getByText(
        t("checklists.summary", {
          total: 0,
          open: 0,
          present: 0,
          missing: 0,
          notApplicable: 0,
        }),
      ),
    ).toBeTruthy();

    p2Docs.resolve([documentRow({ id: "d20", caseId: "p2", displayName: "İddianame.pdf" })]);
    p2Templates.resolve([{ ...TPL2, id: "tpl:ceza:v1", title: "Ceza kontrol listesi" }]);
    p2Items.resolve([
      itemRow({ id: "i9", caseId: "p2", itemText: "İddianame tebliğ edildi mi?" }),
    ]);

    expect(await screen.findByText("İddianame tebliğ edildi mi?")).toBeTruthy();
    expect(screen.getByText("Ceza kontrol listesi")).toBeTruthy();

    // Geri dönüldüğünde p1 yeniden yüklenir; eski bildirim kendiliğinden geri gelmez.
    fireEvent.change(byId<HTMLSelectElement>("checklist-case"), { target: { value: "p1" } });
    await waitFor(() => expect(mocks.checklistItems).toHaveBeenCalledWith("p1"));
    expect(await screen.findByText(I1.itemText)).toBeTruthy();
    expect(
      screen.queryByText(
        t("checklists.noticeCopied", {
          title: TPL1.title,
          version: 2,
          count: 3,
          license: "CC-BY-4.0",
        }),
      ),
    ).toBeNull();
  });

  it("düşen yükleme önceki dosyanın maddelerini ve kataloğunu bırakmaz", async () => {
    setup({
      documents: async (caseId) => {
        if (caseId === "p1") return [D1];
        throw { code: "E_DOC", message: "Belgeler yüklenemedi.", retryable: true };
      },
      templates: async (caseId) => (caseId === "p1" ? [TPL1] : []),
      items: async (caseId) => (caseId === "p1" ? [I1] : []),
    });

    await screen.findByText(I1.itemText);
    fireEvent.change(byId<HTMLSelectElement>("checklist-case"), { target: { value: "p2" } });

    expect(await screen.findByText("E_DOC")).toBeTruthy();
    // Kısmi veri yok: Promise.all düştüğünde eski dosyanın maddesi/kataloğu kalmaz.
    expect(screen.queryByText(I1.itemText)).toBeNull();
    expect(screen.queryByText(TPL1.title)).toBeNull();
    expect(screen.getByText(t("checklists.catalogEmpty"))).toBeTruthy();
    expect(screen.getByText(t("checklists.empty"))).toBeTruthy();
  });
});
