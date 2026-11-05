// @vitest-environment jsdom
//
// Notlar bileşeni davranış testi (F07-T10).
//
// `lib/notes.ts` testleri yalnız **saf** yardımcıları doğruluyor: taslak durumu
// etiketi, satır farkı, çakışma özeti, "kirli" karşılaştırması. Bu dosya 782
// satırlık bileşenin kendisini çizip **bağlantıyı** ölçer: hangi düğme hangi
// çekirdek komutunu çağırıyor, besteci taslağı kasaya ne zaman yazılıyor,
// çakışmada kullanıcı ne görüyor ve **dosya değişince** önceki dosyanın notu,
// taslağı ya da düzenleme paneli ekranda kalıyor mu. `api` kısmi mock; `t()`,
// `lib/notes` ve `lib/format` gerçektir.
//
// Sabitlenen davranışlar:
//
//  1. Notlar sunucudan okunur; künye (zaman · sürüm), başlıksız not etiketi ve
//     alt satırdaki sayı **sunucunun listesinden** gelir.
//  2. Kaynak alıntı yoksa dürüst boş durum yazılır; varsa alıntı ve bağlamı
//     (önek/sonek) kırpılarak gösterilir.
//  3. Boş gövde çekirdeğe **gönderilmez** (`E_NOTE`).
//  4. Geçerli not kırpılarak tam sözleşmeyle gider, besteci temizlenir, kasadaki
//     taslak silinir ve liste tazelenir.
//  5. Besteci taslağı 900 ms sonra kasaya yazılır; içerik boşalınca silinir.
//  6. Açılışta kasadaki gerçek taslak geri yüklenir (metin + çapalar + durum).
//  7. Düzenleme paneli notu alanlara yükler; kasadaki **farklı** taslak
//     bildirilir, eski sürüme dayanıyorsa söylenir ve "Taslağı yükle" uygular.
//  8. "Kaydet" yalnız gerçek değişiklikte açılır ve **okunan `rowVersion`ı**
//     taşır; başarıdan sonra taslak silinir.
//  9. Çakışmada not sessizce ezilmez: fark, satır sayıları ve güncel sürüm
//     gösterilir; "Güncel metni koru" çekirdeğin metnine döner, **yazmaz**.
// 10. "Benim metnimi kaydet" **güncel** sürüm numarasıyla yazar (eskiyle değil).
// 11. Geri al yalnız kayıt yokken kapalıdır; `E_NOTE_HISTORY` kırmızı hata
//     değil, ayrı bir bilgi satırıdır.
// 12. Sürüm geçmişi satırları kaynaktan yazılır; boş geçmiş ve bilinmeyen
//     kaynak uydurulmaz.
// 13. "Sil" çekirdeğe gider ve listeyi tazeler; düzenlenen not silinirse panel
//     kapanır.
// 14. **Ölçülen kusur A (INV-05, iki yol):** dosya değişince liste
//     **temizlenmiyordu** — yeni dosyanın isteği askıdayken/düşerken önceki
//     dosyanın notları yeni dosyanın altında görünüyordu. Aynı sınıf düşen
//     okumada da vardı: kırmızı hata + önceki dosyanın notları.
// 15. **Ölçülen kusur A3 (yanıt sırası):** liste okumaları sıraya bağlı
//     değildi; geç gelen eski yanıt yeni dosyanın listesini eziyordu.
// 16. **Ölçülen kusur B (INV-05, yazma yolu):** dosya değişince düzenleme
//     paneli, çakışma ve bekleyen taslak **kapanmıyordu** → "Kaydet" önceki
//     dosyanın notunu yeni dosya açıkken yazabiliyor, düzenleme taslağı
//     `note_draft_save(yeni dosya, eski not)` olarak kasa içine gidebiliyordu.
// 17. **Ölçülen kusur B2:** dosya değişince besteci önceki dosyanın metnini
//     taşıyordu; yeni dosyanın taslağı gelene kadar o metin "bu dosyanın
//     taslağı" gibi okunuyor ve "Notu ekle" ile **yanlış dosyaya** yazılıyordu.
// 18. **Ölçülen kusur C (bayat hata):** başarılı tazeleme kırmızı hatayı
//     silmiyordu.
// 19. **Ölçülen kusur D (taslak okuma yarışı):** bir notun taslağı yavaş
//     dönerse, sonradan açılan **başka** notun panelinde görünüyor ve
//     "Taslağı yükle" onun metnini öteki nota taşıyordu.
// 20. **Ölçülen kusur E (uçuştaki taslak yazımı):** `note_draft_save` uçuşta
//     kalırken not eklendiğinde/silindiğinde, geç dönen yazma taslağı **geri
//     getiriyordu** (sonraki açılışta aynı metin yeniden belirir).

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { formatTimestamp } from "../lib/format";
import { t } from "../lib/i18n";
import type { AnchorRow, NoteDraftRow, NoteRevisionRow, NoteRow } from "../lib/api";

const mocks = vi.hoisted(() => ({
  noteList: vi.fn(),
  noteGet: vi.fn(),
  noteCreate: vi.fn(),
  noteUpdate: vi.fn(),
  noteDelete: vi.fn(),
  noteDraftGet: vi.fn(),
  noteDraftSave: vi.fn(),
  noteDraftClear: vi.fn(),
  noteUndo: vi.fn(),
  noteRedo: vi.fn(),
  noteRevisions: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

const { Notes } = await import("./Notes");

/* ------------------------------------------------------------------ fixtures */

const ISO = "2026-11-02T09:00:00.000Z";
const STAMP = formatTimestamp(ISO);

/** Bileşenin kendi taslak gecikmesi; burada birebir yazılır. */
const AUTOSAVE = 900;

function note(over: Partial<NoteRow> = {}): NoteRow {
  return {
    id: "n1",
    caseId: "c1",
    title: "Tapu incelemesi",
    body: "Tapu kaydı eksik.",
    authorKind: "user",
    visibilityLabel: "Dosya ekibi",
    sourceAnchorIds: [],
    approvedAt: null,
    createdAt: ISO,
    updatedAt: ISO,
    rowVersion: 1,
    revisionNo: 1,
    canUndo: false,
    canRedo: false,
    ...over,
  };
}

function anchor(over: Partial<AnchorRow> = {}): AnchorRow {
  return {
    id: "a1",
    caseId: "c1",
    documentVersionId: "v1",
    parseRunId: "r1",
    pageId: null,
    segmentId: "s1",
    startOffset: 10,
    endOffset: 34,
    exactQuote: "Satıcı, taşınmazı ayıptan ari olarak devretmiştir.",
    prefix: "…bu sözleşmede ",
    suffix: " ve taraflar kabul eder.",
    quoteHash: "h1",
    anchorStatus: "verified",
    createdAt: ISO,
    ...over,
  };
}

function draft(over: Partial<NoteDraftRow> = {}): NoteDraftRow {
  return {
    caseId: "c1",
    noteId: null,
    title: "",
    body: "",
    sourceAnchorIds: [],
    baseRowVersion: null,
    updatedAt: ISO,
    ...over,
  };
}

function revision(over: Partial<NoteRevisionRow> = {}): NoteRevisionRow {
  return {
    noteId: "n1",
    revisionNo: 1,
    title: "Tapu incelemesi",
    body: "Tapu kaydı eksik.",
    sourceAnchorIds: [],
    origin: "create",
    createdAt: ISO,
    ...over,
  };
}

/* ------------------------------------------------------------------- harness */

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** Mikro görevleri boşalt (sahte zamanlayıcıda `findBy`/`waitFor` kullanılmaz). */
async function flush(): Promise<void> {
  await act(async () => {
    for (let i = 0; i < 8; i += 1) await Promise.resolve();
  });
}

/** Sahte saati ilerlet: 900 ms'lik taslak yazımı tetiklenir, yanıt boşaltılır. */
async function tick(ms = AUTOSAVE): Promise<void> {
  await act(async () => {
    vi.advanceTimersByTime(ms);
  });
  await flush();
}

type Props = { caseId?: string; anchors?: AnchorRow[] };

function element(props: Props) {
  return <Notes caseId={props.caseId ?? "c1"} anchors={props.anchors ?? [anchor()]} />;
}

async function mount(props: Props = {}) {
  const utils = render(element(props));
  await flush();
  return utils;
}

async function setCase(utils: { rerender: (ui: JSX.Element) => void }, props: Props) {
  await act(async () => {
    utils.rerender(element(props));
  });
  await flush();
}

function button(name: string): HTMLButtonElement {
  return screen.getByRole("button", { name }) as HTMLButtonElement;
}

function composerDiscard(): HTMLButtonElement {
  return screen.getAllByRole("button", { name: t("notes.discardDraft") })[0] as HTMLButtonElement;
}

function bodyInput(): HTMLTextAreaElement {
  return screen.getByLabelText(t("notes.body")) as HTMLTextAreaElement;
}

function editBodyInput(): HTMLTextAreaElement {
  return screen.getByLabelText(t("notes.editBodyLabel")) as HTMLTextAreaElement;
}

function titleInput(): HTMLInputElement {
  return screen.getByLabelText(t("notes.titleOptional")) as HTMLInputElement;
}

beforeEach(() => {
  vi.useFakeTimers();
  mocks.noteList.mockResolvedValue([]);
  mocks.noteDraftGet.mockResolvedValue(null);
  mocks.noteDraftSave.mockResolvedValue(draft({ updatedAt: ISO }));
  mocks.noteDraftClear.mockResolvedValue(true);
  mocks.noteDelete.mockResolvedValue(undefined);
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.useRealTimers();
});

/* -------------------------------------------------------------------- tests */

describe("notlar", () => {
  it("notları sunucudan okur; künye, sürüm ve başlıksız not etiketi kaynaktan gelir", async () => {
    mocks.noteList.mockResolvedValue([
      note({ id: "n1", title: "Tapu incelemesi", body: "Tapu kaydı eksik.", rowVersion: 3 }),
      note({ id: "n2", title: "", body: "Başlıksız gövde" }),
    ]);

    await mount();

    expect(mocks.noteList).toHaveBeenCalledWith("c1");
    expect(screen.getByText(t("notes.cardSub", { count: 2 }))).toBeTruthy();
    expect(screen.getByText("Tapu incelemesi")).toBeTruthy();
    expect(screen.getByText("Tapu kaydı eksik.")).toBeTruthy();
    expect(screen.getByText(`${STAMP} · ${t("notes.versionLabel", { version: 3 })}`)).toBeTruthy();
    // Başlık boşsa uydurulmaz: sözlükteki "Başlıksız not" yazılır, gövde korunur.
    expect(screen.getByText(t("notes.untitled"))).toBeTruthy();
    expect(screen.getByText("Başlıksız gövde")).toBeTruthy();
  });

  it("kaynak alıntı yoksa dürüst boş durum yazar, varsa alıntıyı ve bağlamı gösterir", async () => {
    const long = "A".repeat(200);
    const utils = await mount({ anchors: [] });
    expect(screen.getAllByText(t("notes.noAnchors")).length).toBe(1); // yalnız besteci
    expect(screen.queryByRole("checkbox")).toBeNull();

    await setCase(utils, { anchors: [anchor({ exactQuote: long })] });

    expect(screen.queryByText(t("notes.noAnchors"))).toBeNull();
    expect(screen.getAllByRole("checkbox").length).toBe(1);
    // Alıntı 140 karakterde kırpılır; ham 200 karakterlik metin basılmaz.
    expect(screen.getByText(`“${long.slice(0, 140)}”`)).toBeTruthy();
    expect(screen.queryByText(`“${long}”`)).toBeNull();
  });

  it("boş gövdeyi çekirdeğe göndermez", async () => {
    await mount();
    fireEvent.change(titleInput(), { target: { value: "Yalnız başlık" } });
    fireEvent.click(button(t("notes.add")));
    await flush();

    expect(mocks.noteCreate).not.toHaveBeenCalled();
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("E_NOTE");
    expect(alert.textContent).toContain(t("notes.emptyBody"));
  });

  it("geçerli notu kırparak gönderir, besteciyi temizler, taslağı siler ve listeyi tazeler", async () => {
    mocks.noteList.mockResolvedValueOnce([note()]).mockResolvedValueOnce([note(), note({ id: "n2" })]);
    mocks.noteCreate.mockResolvedValue(note({ id: "n2", title: "Yeni not", body: "Gövde" }));

    await mount();
    fireEvent.change(titleInput(), { target: { value: "  Yeni not  " } });
    fireEvent.change(bodyInput(), { target: { value: "  Gövde  " } });
    fireEvent.click(screen.getAllByRole("checkbox")[0]!);
    fireEvent.click(button(t("notes.add")));
    await flush();

    expect(mocks.noteCreate).toHaveBeenCalledWith("c1", {
      title: "Yeni not",
      body: "Gövde",
      sourceAnchorIds: ["a1"],
    });
    expect(mocks.noteDraftClear).toHaveBeenCalledWith("c1", null);
    expect(titleInput().value).toBe("");
    expect(bodyInput().value).toBe("");
    expect((screen.getAllByRole("checkbox")[0] as HTMLInputElement).checked).toBe(false);
    expect(mocks.noteList).toHaveBeenCalledTimes(2);
  });

  it("besteci taslağını 900 ms sonra kasaya yazar, içerik boşalınca siler", async () => {
    await mount();

    fireEvent.change(bodyInput(), { target: { value: "Gözlem" } });
    await tick();

    expect(mocks.noteDraftSave).toHaveBeenCalledWith("c1", null, {
      title: "",
      body: "Gözlem",
      sourceAnchorIds: [],
      baseRowVersion: null,
    });
    expect(screen.getByRole("status").textContent).toBe(
      `${t("notes.draftSaved")} · ${STAMP}`,
    );

    fireEvent.change(bodyInput(), { target: { value: "" } });
    await tick();

    expect(mocks.noteDraftClear).toHaveBeenCalledWith("c1", null);
    expect(screen.getByRole("status").textContent).toBe(t("notes.draftIdle"));
  });

  it("açılışta kasadaki gerçek taslağı geri yükler (metin, çapa, durum, saat)", async () => {
    mocks.noteDraftGet.mockResolvedValue(
      draft({ title: "Gözlem", body: "Eksik belge", sourceAnchorIds: ["a1"], updatedAt: ISO }),
    );

    await mount();

    expect(mocks.noteDraftGet).toHaveBeenCalledWith("c1", null);
    expect(screen.getByLabelText(t("notes.titleOptional"))).toHaveProperty("value", "Gözlem");
    expect(bodyInput().value).toBe("Eksik belge");
    expect((screen.getAllByRole("checkbox")[0] as HTMLInputElement).checked).toBe(true);
    expect(screen.getByRole("status").textContent).toBe(`${t("notes.draftSaved")} · ${STAMP}`);
  });

  it("düzenleme paneli notu yükler, eski sürüme dayanan farklı taslağı bildirir ve uygular", async () => {
    mocks.noteList.mockResolvedValue([note()]);
    mocks.noteDraftGet.mockImplementation(async (_caseId: string, noteId: string | null) =>
      noteId === "n1"
        ? draft({ noteId: "n1", title: "Tapu incelemesi", body: "Taslak metin", baseRowVersion: 0 })
        : null,
    );

    await mount();
    fireEvent.click(button(t("notes.edit")));
    await flush();

    expect(mocks.noteDraftGet).toHaveBeenCalledWith("c1", "n1");
    expect(editBodyInput().value).toBe("Tapu kaydı eksik.");
    expect(
      screen.getByText(
        `${t("notes.draftDiffers", { time: STAMP })} ${t("notes.draftStaleShort")}`,
      ),
    ).toBeTruthy();

    fireEvent.click(button(t("notes.loadDraft")));
    await flush();

    expect(editBodyInput().value).toBe("Taslak metin");
    // Eski sürüme dayanan taslak yüklendiğinde gerekçe ayrı satırda yazılır.
    expect(screen.getByText(t("notes.draftStaleLong"))).toBeTruthy();
    expect(screen.queryByText(t("notes.draftDiffers", { time: STAMP }))).toBeNull();
  });

  it("kaydetme yalnız değişiklikte açılır, okunan sürümü taşır ve taslağı temizler", async () => {
    mocks.noteList.mockResolvedValue([note({ rowVersion: 4 })]);
    mocks.noteUpdate.mockResolvedValue(note({ rowVersion: 5, body: "Yeni metin" }));

    await mount();
    fireEvent.click(button(t("notes.edit")));
    await flush();

    expect(button(t("common.save")).disabled).toBe(true);
    fireEvent.change(editBodyInput(), { target: { value: "  Yeni metin  " } });
    await flush();
    expect(button(t("common.save")).disabled).toBe(false);

    fireEvent.click(button(t("common.save")));
    await flush();

    expect(mocks.noteUpdate).toHaveBeenCalledWith("n1", 4, {
      title: "Tapu incelemesi",
      body: "Yeni metin",
      sourceAnchorIds: [],
    });
    expect(mocks.noteDraftClear).toHaveBeenCalledWith("c1", "n1");
    expect(mocks.noteList).toHaveBeenCalledTimes(2);
  });

  it("çakışmada notu ezmez: farkı gösterir, 'Güncel metni koru' çekirdeğin metnine döner", async () => {
    mocks.noteList.mockResolvedValue([note()]);
    mocks.noteUpdate.mockRejectedValue({ code: "E_ROW_VERSION", message: "sürüm eskidi", retryable: false });
    mocks.noteGet.mockResolvedValue(
      note({ rowVersion: 2, body: "Başka pencerede değişti", updatedAt: ISO }),
    );

    await mount();
    fireEvent.click(button(t("notes.edit")));
    await flush();
    fireEvent.change(editBodyInput(), { target: { value: "Benim metnim" } });
    await flush();
    fireEvent.click(button(t("common.save")));
    await flush();

    expect(screen.getByText(t("notes.conflictTitle"))).toBeTruthy();
    expect(
      screen.getByText(t("notes.conflictStats", { mine: 1, theirs: 1 })),
    ).toBeTruthy();
    expect(screen.getByText(t("notes.currentVersion", { version: 2, time: STAMP }))).toBeTruthy();
    // Fark satırları: "− benim" / "+ kasadaki" (metin gövdesi ayrı düğüm).
    expect(screen.getAllByText("Benim metnim").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Başka pencerede değişti").length).toBeGreaterThan(0);
    // Çakışma açıkken alanlar kilitlidir: sessiz yazma yok.
    expect(editBodyInput().disabled).toBe(true);

    fireEvent.click(button(t("notes.keepCurrent")));
    await flush();

    expect(mocks.noteUpdate).toHaveBeenCalledTimes(1); // yalnız düşen deneme
    expect(editBodyInput().value).toBe("Başka pencerede değişti");
    expect(editBodyInput().disabled).toBe(false);
    expect(screen.queryByText(t("notes.conflictTitle"))).toBeNull();
    expect(mocks.noteDraftClear).toHaveBeenCalledWith("c1", "n1");
  });

  it("'Benim metnimi kaydet' güncel sürüm numarasıyla yazar", async () => {
    mocks.noteList.mockResolvedValue([note()]);
    mocks.noteUpdate
      .mockRejectedValueOnce({ code: "E_ROW_VERSION", message: "sürüm eskidi", retryable: false })
      .mockResolvedValueOnce(note({ rowVersion: 3, body: "Benim metnim" }));
    mocks.noteGet.mockResolvedValue(note({ rowVersion: 2, body: "Başka pencerede değişti" }));

    await mount();
    fireEvent.click(button(t("notes.edit")));
    await flush();
    fireEvent.change(editBodyInput(), { target: { value: "Benim metnim" } });
    await flush();
    fireEvent.click(button(t("common.save")));
    await flush();
    fireEvent.click(button(t("notes.keepMine")));
    await flush();

    expect(mocks.noteUpdate).toHaveBeenLastCalledWith("n1", 2, {
      title: "Tapu incelemesi",
      body: "Benim metnim",
      sourceAnchorIds: [],
    });
    expect(screen.queryByText(t("notes.conflictTitle"))).toBeNull();
  });

  it("geri al yalnız kayıt yokken kapalıdır; geçmiş yokluğu kırmızı hata değildir", async () => {
    mocks.noteList.mockResolvedValue([note({ canUndo: true, rowVersion: 2 })]);
    mocks.noteUndo.mockRejectedValue({ code: "E_NOTE_HISTORY", message: "Geri alınacak kayıt yok.", retryable: false });

    await mount();
    fireEvent.click(button(t("notes.edit")));
    await flush();

    expect(button(t("notes.undo")).disabled).toBe(false);
    expect(button(t("notes.redo")).disabled).toBe(true);

    fireEvent.change(editBodyInput(), { target: { value: "kirli" } });
    await flush();
    expect(button(t("notes.undo")).disabled).toBe(true); // kaydedilmemiş değişiklik varken kapalı

    fireEvent.change(editBodyInput(), { target: { value: "Tapu kaydı eksik." } });
    await flush();
    fireEvent.click(button(t("notes.undo")));
    await flush();

    expect(mocks.noteUndo).toHaveBeenCalledWith("n1", 2);
    expect(screen.getByText("Geri alınacak kayıt yok.")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("sürüm geçmişini kaynaktan yazar; boş geçmiş ve bilinmeyen kaynak uydurulmaz", async () => {
    mocks.noteList.mockResolvedValue([note({ canUndo: true })]);
    mocks.noteRevisions.mockResolvedValue([
      revision({ revisionNo: 1, origin: "create" }),
      revision({ revisionNo: 2, origin: "edit" }),
      revision({ revisionNo: 3, origin: "gizemli" }),
    ]);

    await mount();
    fireEvent.click(button(t("notes.edit")));
    await flush();
    fireEvent.click(button(t("notes.historyTitle")));
    await flush();

    expect(mocks.noteRevisions).toHaveBeenCalledWith("n1");
    expect(screen.getByText("#1")).toBeTruthy();
    expect(screen.getByText(t("notes.originCreate"))).toBeTruthy();
    expect(screen.getByText(t("notes.originEdit"))).toBeTruthy();
    expect(screen.getByText(t("notes.originUnknown"))).toBeTruthy();

    mocks.noteRevisions.mockResolvedValue([]);
    fireEvent.click(button(t("notes.historyTitle")));
    await flush();
    expect(screen.getByText(t("notes.historyEmpty"))).toBeTruthy();
  });

  it("silme çekirdeğe gider, listeyi tazeler ve düzenlenen not silinirse panel kapanır", async () => {
    mocks.noteList.mockResolvedValueOnce([note()]).mockResolvedValueOnce([]);

    await mount();
    fireEvent.click(button(t("notes.edit")));
    await flush();
    expect(screen.getByText(t("notes.editTitle"))).toBeTruthy();

    fireEvent.click(button(t("notes.delete")));
    await flush();

    expect(mocks.noteDelete).toHaveBeenCalledWith("n1");
    expect(screen.queryByText(t("notes.editTitle"))).toBeNull();
    expect(mocks.noteList).toHaveBeenCalledTimes(2);
    expect(screen.getByText(t("notes.cardSub", { count: 0 }))).toBeTruthy();
  });

  /* -------------------------------------------------- A: dosya değişimi (INV-05) */

  it("dosya değişince önceki dosyanın notları hemen kalkar", async () => {
    mocks.noteList.mockResolvedValueOnce([note()]);
    const pending = deferred<NoteRow[]>();
    mocks.noteList.mockReturnValueOnce(pending.promise);

    const utils = await mount();
    expect(screen.getByText("Tapu incelemesi")).toBeTruthy();

    await setCase(utils, { caseId: "c2" });

    // Yeni dosyanın yanıtı henüz gelmedi: eski dosyanın notu ekranda kalmamalı.
    expect(screen.queryByText("Tapu incelemesi")).toBeNull();
    expect(screen.getByText(t("notes.cardSub", { count: 0 }))).toBeTruthy();
  });

  it("düşen liste önceki dosyanın notlarını bırakmaz", async () => {
    mocks.noteList.mockResolvedValueOnce([note()]);
    mocks.noteList.mockRejectedValueOnce({ code: "E_DB", message: "kasa okunamadı", retryable: true });

    const utils = await mount();
    await setCase(utils, { caseId: "c2" });

    expect(screen.queryByText("Tapu incelemesi")).toBeNull();
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("E_DB");
    expect(alert.textContent).toContain("kasa okunamadı");
  });

  it("işlem sonrası düşen tazeleme eski listeyi kırmızı hatayla bırakmaz", async () => {
    mocks.noteList.mockResolvedValueOnce([note()]);
    mocks.noteList.mockRejectedValueOnce({ code: "E_DB", message: "liste okunamadı", retryable: true });
    mocks.noteDelete.mockResolvedValue(undefined);

    await mount();
    expect(screen.getByText("Tapu incelemesi")).toBeTruthy();

    // Silme çekirdekte başarılı; tazeleme düşüyor → bayat satır ekranda kalmamalı.
    fireEvent.click(button(t("notes.delete")));
    await flush();

    expect(mocks.noteDelete).toHaveBeenCalledWith("n1");
    expect(screen.queryByText("Tapu incelemesi")).toBeNull();
    expect(screen.getByRole("alert").textContent).toContain("E_DB");
  });

  it("geç gelen eski liste yeni dosyanın notlarını ezmez", async () => {
    const first = deferred<NoteRow[]>();
    const second = deferred<NoteRow[]>();
    mocks.noteList.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    const utils = await mount();
    await setCase(utils, { caseId: "c2" });

    await act(async () => {
      second.resolve([note({ id: "n2", caseId: "c2", title: "Ceza dosyası notu" })]);
    });
    await flush();
    expect(screen.getByText("Ceza dosyası notu")).toBeTruthy();

    await act(async () => {
      first.resolve([note({ id: "n1", caseId: "c1", title: "Tapu incelemesi" })]);
    });
    await flush();

    expect(screen.getByText("Ceza dosyası notu")).toBeTruthy();
    expect(screen.queryByText("Tapu incelemesi")).toBeNull();
  });

  /* ------------------------------------------------- B: dosya değişiminde yazma yolu */

  it("dosya değişince düzenleme paneli kapanır ve eski dosyanın notuna yazmaz", async () => {
    mocks.noteList.mockResolvedValueOnce([note()]);
    mocks.noteList.mockResolvedValueOnce([]);
    const pendingDraft = deferred<NoteDraftRow | null>();
    mocks.noteDraftGet.mockImplementation(async (_caseId: string, noteId: string | null) =>
      noteId === "n1" ? pendingDraft.promise : null,
    );

    const utils = await mount();
    fireEvent.click(button(t("notes.edit")));
    await flush();
    expect(screen.getByText(t("notes.editTitle"))).toBeTruthy();

    await setCase(utils, { caseId: "c2" });

    expect(screen.queryByText(t("notes.editTitle"))).toBeNull();
    expect(screen.queryByRole("button", { name: t("common.save") })).toBeNull();

    // Geç dönen düzenleme taslağı da yeni dosyaya yazılmaz.
    await act(async () => {
      pendingDraft.resolve(draft({ noteId: "n1", body: "Eski dosyanın taslağı", baseRowVersion: 1 }));
    });
    await flush();
    await tick();

    expect(mocks.noteDraftSave).not.toHaveBeenCalled();
    expect(mocks.noteUpdate).not.toHaveBeenCalled();
  });

  it("dosya değişince besteci önceki dosyanın metnini taşımaz", async () => {
    mocks.noteList.mockResolvedValueOnce([]).mockResolvedValueOnce([]);
    const pendingDraft = deferred<NoteDraftRow | null>();
    mocks.noteDraftGet.mockImplementation(async (_caseId: string, noteId: string | null) =>
      noteId === null ? pendingDraft.promise : null,
    );

    const utils = await mount();
    fireEvent.change(bodyInput(), { target: { value: "Eski dosyanın gözlemi" } });
    await flush();
    expect(bodyInput().value).toBe("Eski dosyanın gözlemi");

    await setCase(utils, { caseId: "c2" });

    // Yeni dosyanın taslağı askıda: eski metin "bu dosyanın taslağı" gibi durmamalı.
    expect(bodyInput().value).toBe("");
    expect(screen.queryByText(t("notes.draftSaved"))).toBeNull();
  });

  /* --------------------------------------------------------------- C: bayat hata */

  it("başarılı tazeleme önceki hatayı siler", async () => {
    mocks.noteList.mockResolvedValueOnce([note()]).mockResolvedValueOnce([]);
    mocks.noteDelete
      .mockRejectedValueOnce({ code: "E_IO", message: "silinemedi", retryable: true })
      .mockResolvedValueOnce(undefined);

    await mount();
    fireEvent.click(button(t("notes.delete")));
    await flush();
    expect(screen.getByRole("alert").textContent).toContain("E_IO");

    fireEvent.click(button(t("notes.delete")));
    await flush();

    expect(mocks.noteDelete).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText(t("notes.cardSub", { count: 0 }))).toBeTruthy();
  });

  /* ------------------------------------------------------------- D: taslak yarışı */

  it("geç gelen taslak okuması sonradan açılan notun panelini ele geçirmez", async () => {
    mocks.noteList.mockResolvedValue([
      note({ id: "n1", title: "Tapu incelemesi", body: "Tapu kaydı eksik." }),
      note({ id: "n2", title: "Ceza notu", body: "Ceza dosyası gövdesi" }),
    ]);
    const slow = deferred<NoteDraftRow | null>();
    mocks.noteDraftGet.mockImplementation(async (_caseId: string, noteId: string | null) => {
      if (noteId === "n1") return slow.promise;
      if (noteId === "n2") return null;
      return null;
    });

    await mount();
    fireEvent.click(screen.getAllByRole("button", { name: t("notes.edit") })[0]!);
    await flush();
    fireEvent.click(screen.getAllByRole("button", { name: t("notes.edit") })[1]!);
    await flush();

    expect(editBodyInput().value).toBe("Ceza dosyası gövdesi");

    await act(async () => {
      slow.resolve(draft({ noteId: "n1", body: "Tapu taslağı", baseRowVersion: 1 }));
    });
    await flush();

    // n1'in taslağı n2 düzenlenirken gösterilmez.
    expect(screen.queryByText(t("notes.draftDiffers", { time: STAMP }))).toBeNull();
    expect(editBodyInput().value).toBe("Ceza dosyası gövdesi");
  });

  /* ------------------------------------------------------- E: uçuştaki taslak yazımı */

  it("not eklendikten sonra uçuştaki taslak yazımı taslağı geri getirmez", async () => {
    const save = deferred<NoteDraftRow>();
    let vault: NoteDraftRow | null = null;
    mocks.noteDraftSave.mockImplementation(async () => {
      const saved = await save.promise;
      vault = saved;
      return saved;
    });
    mocks.noteDraftClear.mockImplementation(async () => {
      vault = null;
      return true;
    });
    mocks.noteCreate.mockResolvedValue(note({ id: "n2" }));
    mocks.noteList.mockResolvedValue([note({ id: "n2" })]);

    await mount();
    fireEvent.change(bodyInput(), { target: { value: "Yeni gözlem" } });
    await tick();
    expect(mocks.noteDraftSave).toHaveBeenCalledTimes(1);

    fireEvent.click(button(t("notes.add")));
    await flush();
    await act(async () => {
      save.resolve(draft({ body: "Yeni gözlem" }));
    });
    await flush();

    // Not gerçekten yazıldı ve besteci temizlendi; kasadaki taslak da boş kalmalı.
    expect(mocks.noteCreate).toHaveBeenCalledTimes(1);
    expect(bodyInput().value).toBe("");
    expect(vault).toBeNull();
  });

  it("'Taslağı sil' de uçuştaki yazımı bekler; taslak geri gelmez", async () => {
    const save = deferred<NoteDraftRow>();
    let vault: NoteDraftRow | null = null;
    mocks.noteDraftSave.mockImplementation(async () => {
      const saved = await save.promise;
      vault = saved;
      return saved;
    });
    mocks.noteDraftClear.mockImplementation(async () => {
      vault = null;
      return true;
    });

    await mount();
    fireEvent.change(bodyInput(), { target: { value: "Silinecek taslak" } });
    await tick();
    expect(mocks.noteDraftSave).toHaveBeenCalledTimes(1);

    fireEvent.click(composerDiscard());
    await flush();
    await act(async () => {
      save.resolve(draft({ body: "Silinecek taslak" }));
    });
    await flush();

    expect(bodyInput().value).toBe("");
    expect(vault).toBeNull();
  });
});

/* ---------------------------------------------------------------------------
 * Yazma uçuşu (2026-11-05 turu)
 *
 * Yukarıdaki blok okuma yollarını, dosya kimliğini ve taslak yarışını kapatmıştı.
 * **Yazma** uçuşu hiç ölçülmemişti: `busy` kilidi, yanıt beklerken yazılan metin
 * ve geç gelen yanıtın hangi dosyaya/panoya yazıldığı. Ölçülen on kusur:
 *
 *  W1  `addNote` başarısı besteciyi **koşulsuz** temizliyordu → uçuşta yazılan
 *      yeni taslak yanıtla siliniyordu (sessiz kayıp).
 *  W2  `addNote` geç başarısı dosya değiştikten sonra **yeni** dosyanın
 *      bestecisini siliyordu (INV-05).
 *  W3  `addNote` geç hatası yeni dosyanın altında kırmızı bant oluyordu.
 *  W4  `saveEdit` başarısı panelde uçuşta yazılan metni sunucunun metniyle
 *      eziyordu (sessiz kayıp).
 *  W5  `saveEdit` geç başarısı **kapanan** paneli yeniden açıyordu.
 *  W6  `saveEdit` geç hatası yeni dosyada görünüyordu.
 *  W7  `remove` kilitsizdi → çift tık iki `note_delete` (ikincisi `E_NOT_FOUND`
 *      ile başarılı silmenin üstüne kırmızı yazıyordu).
 *  W8  Dört yazma yolunun `finally`si koşulsuz `setBusy(false)` → devredilen
 *      isteğin kapanışı yeni dosyada başlatılan yazmanın kilidini açıyordu.
 *  W9  `openConflict` çakışmayı **o anki** panelden okuyordu → başka notun
 *      metni "benim metnim" olarak çakışan nota yazılabiliyordu.
 *  W10 Panel kapatılınca uçuştaki kayıt damgası ilerlemiyordu → geç yanıt
 *      kapatılan paneli yeniden açıyordu.
 *  W11 Taslak karşılaştırması tek başına yetmez: iki dosyada **aynı** metin
 *      yazılıysa eski yanıt yeni dosyanın bestecisini yine temizlerdi.
 * ------------------------------------------------------------------------- */

describe("notlar yazma uçuşu", () => {
  it("not eklerken yazılan yeni taslak yanıtla silinmez", async () => {
    const create = deferred<NoteRow>();
    mocks.noteCreate.mockReturnValueOnce(create.promise);
    mocks.noteList.mockResolvedValue([]);

    await mount();
    fireEvent.change(bodyInput(), { target: { value: "İlk gözlem" } });
    fireEvent.click(button(t("notes.add")));
    await flush();
    expect(mocks.noteCreate).toHaveBeenCalledTimes(1);

    // Uçuşta yeni taslak yazılır.
    fireEvent.change(bodyInput(), { target: { value: "Uçuşta yazıldı" } });
    await act(async () => {
      create.resolve(note({ id: "n9" }));
    });
    await flush();

    // Gönderilen metin artık ekranda değil: yeni taslak korunur.
    expect(bodyInput().value).toBe("Uçuşta yazıldı");
  });

  it("not ekleme yanıtı dosya değiştikten sonra yeni dosyanın bestecisini silmez", async () => {
    const create = deferred<NoteRow>();
    mocks.noteCreate.mockReturnValueOnce(create.promise);
    mocks.noteList.mockResolvedValue([]);
    mocks.noteDraftGet.mockImplementation(async (caseId: string, noteId: string | null) =>
      noteId === null && caseId === "c2" ? draft({ caseId: "c2", body: "c2 taslağı" }) : null,
    );

    const utils = await mount();
    fireEvent.change(bodyInput(), { target: { value: "c1 notu" } });
    fireEvent.click(button(t("notes.add")));
    await flush();

    await setCase(utils, { caseId: "c2" });
    expect(bodyInput().value).toBe("c2 taslağı");

    await act(async () => {
      create.resolve(note({ id: "n1", caseId: "c1" }));
    });
    await flush();

    expect(bodyInput().value).toBe("c2 taslağı");
  });

  it("aynı metin iki dosyada yazılıysa eski yanıt yeni dosyanın bestecisini silmez", async () => {
    const create = deferred<NoteRow>();
    mocks.noteCreate.mockReturnValueOnce(create.promise);
    mocks.noteList.mockResolvedValue([]);
    mocks.noteDraftGet.mockImplementation(async (caseId: string, noteId: string | null) =>
      noteId === null && caseId === "c2" ? draft({ caseId: "c2", body: "Aynı gözlem" }) : null,
    );

    const utils = await mount();
    fireEvent.change(bodyInput(), { target: { value: "Aynı gözlem" } });
    fireEvent.click(button(t("notes.add")));
    await flush();

    await setCase(utils, { caseId: "c2" });
    expect(bodyInput().value).toBe("Aynı gözlem");

    await act(async () => {
      create.resolve(note({ id: "n1", caseId: "c1" }));
    });
    await flush();

    // Metin aynı olsa da **başka** dosyanın bestecisidir: eski yanıt onu temizlemez.
    expect(bodyInput().value).toBe("Aynı gözlem");
  });

  it("eski dosyanın ekleme hatası yeni dosyada görünmez", async () => {
    const create = deferred<NoteRow>();
    mocks.noteCreate.mockReturnValueOnce(create.promise);
    mocks.noteList.mockResolvedValue([]);

    const utils = await mount();
    fireEvent.change(bodyInput(), { target: { value: "c1 notu" } });
    fireEvent.click(button(t("notes.add")));
    await flush();

    await setCase(utils, { caseId: "c2" });
    expect(screen.queryByRole("alert")).toBeNull();

    await act(async () => {
      create.reject({ code: "E_IO", message: "yazılamadı", retryable: true });
    });
    await flush();

    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("düzenleme kaydı uçuşta yazılan metni ezmez", async () => {
    mocks.noteList.mockResolvedValue([note()]);
    const update = deferred<NoteRow>();
    mocks.noteUpdate.mockReturnValueOnce(update.promise);

    await mount();
    fireEvent.click(button(t("notes.edit")));
    await flush();

    fireEvent.change(editBodyInput(), { target: { value: "Kaydedilecek metin" } });
    fireEvent.click(button(t("common.save")));
    await flush();

    fireEvent.change(editBodyInput(), { target: { value: "Uçuşta yazıldı" } });
    await act(async () => {
      update.resolve(note({ body: "Kaydedilecek metin", rowVersion: 2 }));
    });
    await flush();

    expect(mocks.noteUpdate).toHaveBeenCalledTimes(1);
    expect(editBodyInput().value).toBe("Uçuşta yazıldı");
  });

  it("kaydetme yanıtı dosya değiştikten sonra paneli yeniden açmaz", async () => {
    mocks.noteList.mockResolvedValueOnce([note()]).mockResolvedValue([]);
    const update = deferred<NoteRow>();
    mocks.noteUpdate.mockReturnValueOnce(update.promise);

    const utils = await mount();
    fireEvent.click(button(t("notes.edit")));
    await flush();
    fireEvent.change(editBodyInput(), { target: { value: "Kaydedilecek" } });
    fireEvent.click(button(t("common.save")));
    await flush();

    await setCase(utils, { caseId: "c2" });
    expect(screen.queryByText(t("notes.editTitle"))).toBeNull();

    await act(async () => {
      update.resolve(note({ body: "Kaydedilecek", rowVersion: 2 }));
    });
    await flush();

    expect(screen.queryByText(t("notes.editTitle"))).toBeNull();
  });

  it("eski dosyanın kaydetme hatası yeni dosyada görünmez", async () => {
    mocks.noteList.mockResolvedValueOnce([note()]).mockResolvedValue([]);
    const update = deferred<NoteRow>();
    mocks.noteUpdate.mockReturnValueOnce(update.promise);

    const utils = await mount();
    fireEvent.click(button(t("notes.edit")));
    await flush();
    fireEvent.change(editBodyInput(), { target: { value: "Kaydedilecek" } });
    fireEvent.click(button(t("common.save")));
    await flush();

    await setCase(utils, { caseId: "c2" });

    await act(async () => {
      update.reject({ code: "E_IO", message: "yazılamadı", retryable: true });
    });
    await flush();

    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("silme uçuşunda satır düğmesi ikinci silmeyi engeller", async () => {
    mocks.noteList.mockResolvedValue([note()]);
    const del = deferred<void>();
    mocks.noteDelete.mockReturnValueOnce(del.promise);

    await mount();
    const removeButton = () => button(t("notes.delete"));
    expect(removeButton().disabled).toBe(false);

    fireEvent.click(removeButton());
    await flush();

    // Uçuşta iken ikinci tık yeni bir `note_delete` göndermemeli.
    expect(mocks.noteDelete).toHaveBeenCalledTimes(1);
    expect(removeButton().disabled).toBe(true);

    await act(async () => {
      del.resolve(undefined);
    });
    await flush();
    expect(removeButton().disabled).toBe(false);
  });

  it("dosya değişince devredilen kilit eski yanıtla erken açılmaz", async () => {
    const first = deferred<NoteRow>();
    const second = deferred<NoteRow>();
    mocks.noteCreate.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    mocks.noteList.mockResolvedValue([]);

    const utils = await mount();
    fireEvent.change(bodyInput(), { target: { value: "c1 notu" } });
    fireEvent.click(button(t("notes.add")));
    await flush();

    await setCase(utils, { caseId: "c2" });
    fireEvent.change(bodyInput(), { target: { value: "c2 notu" } });
    fireEvent.click(button(t("notes.add")));
    await flush();
    expect(mocks.noteCreate).toHaveBeenCalledTimes(2);
    expect(button(t("notes.add")).disabled).toBe(true);

    await act(async () => {
      first.resolve(note({ id: "n1", caseId: "c1" }));
    });
    await flush();

    // Devredilen isteğin kapanışı, yeni dosyada uçuşta olan yazmanın kilidini açmaz.
    expect(button(t("notes.add")).disabled).toBe(true);
  });

  it("çakışma okuması başka notun panelinde açılmaz", async () => {
    mocks.noteList.mockResolvedValue([
      note({ id: "n1", title: "Tapu incelemesi", body: "Tapu kaydı eksik." }),
      note({ id: "n2", title: "Ceza notu", body: "Ceza dosyası gövdesi" }),
    ]);
    const theirs = deferred<NoteRow | null>();
    mocks.noteUpdate.mockRejectedValueOnce({
      code: "E_ROW_VERSION",
      message: "sürüm çakıştı",
      retryable: false,
    });
    mocks.noteGet.mockReturnValueOnce(theirs.promise);

    await mount();
    fireEvent.click(screen.getAllByRole("button", { name: t("notes.edit") })[0]!);
    await flush();
    fireEvent.change(editBodyInput(), { target: { value: "Benim metnim" } });
    fireEvent.click(button(t("common.save")));
    await flush();

    // Çakışma okuması askıda: kullanıcı başka notun panelini açar.
    fireEvent.click(screen.getAllByRole("button", { name: t("notes.edit") })[1]!);
    await flush();
    expect(editBodyInput().value).toBe("Ceza dosyası gövdesi");

    await act(async () => {
      theirs.resolve(note({ id: "n1", body: "Sunucudaki metin", rowVersion: 3 }));
    });
    await flush();

    // n1'in çakışması n2 panelinde açılmamalı: "benim metnim" n2'nin metni olurdu.
    expect(screen.queryByText(t("notes.conflictTitle"))).toBeNull();
  });

  it("kapatılan panel geç kayıt yanıtıyla yeniden açılmaz", async () => {
    mocks.noteList.mockResolvedValue([note()]);
    const update = deferred<NoteRow>();
    mocks.noteUpdate.mockReturnValueOnce(update.promise);

    await mount();
    fireEvent.click(button(t("notes.edit")));
    await flush();
    fireEvent.change(editBodyInput(), { target: { value: "Kaydedilecek" } });
    fireEvent.click(button(t("common.save")));
    await flush();

    fireEvent.click(button(t("notes.close")));
    await flush();
    expect(screen.queryByText(t("notes.editTitle"))).toBeNull();

    await act(async () => {
      update.resolve(note({ body: "Kaydedilecek", rowVersion: 2 }));
    });
    await flush();

    expect(screen.queryByText(t("notes.editTitle"))).toBeNull();
  });
});
