// @vitest-environment jsdom
//
// Çıktılar ekranı davranış testi. `lib/export-coverage` testleri yalnız metin
// yardımcılarını doğrular; burada 424 satırlık ekran gerçekten render edilip
// F11-T06 (kasadan okunan kapsam, dışa aktarma, devir paketi) kararlarının
// arayüzde **uygulandığı** ölçülür. `api` kısmi mock; `t()` ve
// `lib/export-coverage` gerçektir.
//
// Sabitlenen davranışlar:
//
// 1. Dosya listesi `caseList(false)` ile gelir, ilk dosya seçilir; kapsam o
//    dosya için kasadan istenir ve rozet/başlık/satırlar/uyarılar **sunucudan**
//    yazılır (arayüz kendi sayısını üretmez).
// 2. Kapsam `complete=false` ise “Kapsam kısmi” + uyarı listesi; belgesiz
//    dosyada “Kapsam yok” + dürüst uyarı; sayılar uydurulmaz.
// 3. Dosya yokken dışa aktarma istekleri gitmez, altı düğme de kapalıdır.
// 4. Kaydet diyaloğu dosya **başlığından** ad üretir (`fileStem`), uzantı ve
//    filtre seçilen biçimdir; çekirdeğe `(caseId, hedef yol)` gider.
// 5. Sonuç künyesi yazılan türü, yolu, baytı, sha256'nın ilk 16 karakterini ve
//    **yanıttan gelen** kapsamın gömüldüğü yeri yazar.
// 6. İptal edilen diyalog istek üretmez.
// 7. Düşen dışa aktarma önceki yeşil sonucu bırakmaz (bayat başarı).
// 8. Yazarken düğme “Yazılıyor…” olur, öteki çıktı ve paket düğmeleri kilitlenir.
// 9. Kapsam tazeleme yeni değerleri yazar; **düşen tazeleme** eski kırmızı
//    hatayı bırakmaz ve sonraki başarılı tazelemede silinir.
// 10. Devir paketi klasör ister; parola **yazıldıysa** çekirdekle aynı ölçüyle
//     (kod noktası) en az 10 karakter olmalı — kısa parola paket **üretmez**
//     (sessiz açık metin yok). Boş alan açık metin klasördür ve künyesi bunu yazar.
// 11. Paket künyesi yol/dosya/manifest öneki + şifreli paket yolunu yazar.
// 12. Düşen paket isteği önceki paket satırını bırakmaz.
// 13. **Ölçülen kusur (A):** paket başarısından sonra parola alanı temizlenmiyordu
//     — kullanıcının yazdığı parola ekranda/bellekte kalıyordu.
// 14. **Ölçülen kusur (B, INV-05):** dosya değişince önceki dosyanın kapsamı,
//     çıktı satırı ve paket satırı ekranda kalıyordu; yeni dosyanın isteği
//     askıdayken **başka dosyanın çıktısı** güncel sanılabiliyordu.
// 15. **Ölçülen kusur (C):** düşen kapsam isteği önceki dosyanın kapsamını
//     “okunmuş” gibi bırakıyordu.
// 16. **Ölçülen kusur (D):** kapsam okunurken “Kapsam okunamadı” yazılıyordu
//     (henüz okunmadı ≠ okunamadı).
// 17. Geç gelen **eski** dosya yanıtı yeni dosyanın kapsamını ezmez.
// 18. Dosya listesi düşerse dürüst hata yazılır, sahte liste üretilmez.
//
// Uçuş yolları (ikinci `describe`; 19–25):
//
// 19. **Ölçülen kusur (E):** dışa aktarma uçuştayken dosya değişince `busy`
//     bırakılmıyordu → yeni dosyanın altı düğmesi eski isteğin yanıtına kadar
//     (yanıt hiç gelmezse süresiz) kapalı kalıyordu.
// 20. **Ölçülen kusur (F):** `run`/`runPackage` `finally`si **koşulsuz**
//     `setBusy(null)` çağırıyordu → devredilen eski isteğin kapanışı yeni
//     dosyada başlatılan yazmanın kilidini erken açıyordu.
// 21. **Ölçülen kusur (G):** paket başarısı parola alanını koşulsuz
//     temizliyordu → uçuşta yazılan yeni parola siliniyordu.
// 22. **Ölçülen kusur (H):** kaydet diyaloğu açıkken düğmeler açıktı → ikinci
//     tık ikinci diyalog + ikinci yazma üretiyordu.
// 23. **Ölçülen kusur (I):** klasör seçimi açıkken paket düğmesi açıktı →
//     ikinci seçim birincinin hedefini eziyordu.
// 24. **Ölçülen kusur (J):** seçici reddi (`save`/`open` promise rejection)
//     hiçbir yere yazılmıyordu — düğmeye basıp hiçbir şey olmaması.
// 25. Uçuştaki **paket** yanıtı da dosya kapısı taşır (sabitleme): yeni
//     dosyanın altına yazılmaz.
// 26. Devredilen **paketin** kapanışı da yeni dosyada başlatılan yazmanın
//     kilidini açmaz (iki yazma yolu ayrı ayrı ölçülür).
//
// Kapsam şeridi (üçüncü `describe`; 27–31):
//
// 27. **Ölçülen kusur (K, INV-05):** kaydet diyaloğu açıkken dosya değişince
//     yazma yine başlıyordu; istek **yeni** nesli aldığı için kapıdan geçiyor
//     ve eski dosyanın “yazıldı” satırı + kapsamı yeni dosyanın altına
//     düşüyordu.
// 28. **Ölçülen kusur (L, aynı sınıf):** klasör seçimi açıkken dosya değişince
//     devir paketi eski dosya için oluşturuluyordu.
// 29. **Ölçülen kusur (M):** tek nesil sayacı yüzünden **ilgisiz** bir dışa
//     aktarma, kullanıcının istediği kapsam tazelemesinin yanıtını düşürüyordu
//     (tazeleme sessizce yutuluyordu).
// 30. **Ölçülen kusur (N):** kapsam okuması uçuştayken “Yenile” açıktı → her
//     tık yeni tam sayım sorgusu.
// 31. Yazma başarısı kapsam şeridini **devralır**: uçuştaki eski okuma geç
//     dönse de yazılan bloğun kapsamını ezmez ve okuma kilidi asılı kalmaz.
//
// Paket ve kapsam dürüstlüğü (dördüncü `describe`; 32–40):
//
// 32. **Ölçülen kusur (O):** parola alanı doluyken 10 karakterden kısa parola
//     `undefined` gönderiliyordu → çekirdek **açık metin** klasör yazıyor, arayüz
//     yeşil “Paket hazır” diyordu; parolanın uygulanmadığı hiçbir yerde yazmıyordu.
//     Yeni kapı: parola yazıldıysa kısa parola paket **üretmez** (görünür hata).
// 33. **Ölçülen kusur (P):** eşik `trim().length` ile (UTF-16 birimi) ölçülüyordu;
//     `"  1234567  "` 11 birim görünüp 7 sayılıyor, parola sessizce düşüyordu.
//     Çekirdek ham dizeyi `chars().count()` ile sayar → kapı aynı ölçüyü kullanır
//     ve parolayı kırpmadan gönderir.
// 34. **Ölçülen kusur (Q, aynı kök):** 5 emoji = 10 UTF-16 birimi → parola
//     gönderiliyor, çekirdek 5 karakter sayıp paketi **sessizce** açık metin
//     yazıyordu.
// 35. **Ölçülen kusur (R):** çekirdek parolayı aldığı hâlde `sealedPath`
//     dönmezse (sözleşme ihlali) arayüz yine yalnız yeşil satırı gösteriyordu.
// 36. **Ölçülen kusur (S):** şifreli pakette birincil konum `pkg.path`ti; çekirdek
//     geçici klasörü sildiği için künye **var olmayan** bir yolu “paket hazır”
//     diye yazıyordu.
// 37. **Ölçülen kusur (T, açık kayıt a):** kapsam paneli gösterdiği sayıların
//     kaynağını (kasadan okundu ↔ çıktıya yazıldı) ve `generatedAt` zamanını
//     yazmıyordu; yazma anındaki blok “şu anki kapsam” sanılıyordu. Bu test ayrıca
//     **dosya değişiminde** kaynağın “kasadan”a döndüğünü ve zamanın tazelendiğini
//     ölçer.
// 38. Yazma sonrası “Kapsamı tazele” de kaynağı kasadan okumaya döndürür.
// 39. Paket başarısı da kapsam kaynağını “devir paketi” yapar.
// 40. Açık metin paket künyesi parolanın uygulanmadığını **yazar** (boş alan
//     bilinçli açık metindir; sessiz değil).

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { t } from "../lib/i18n";
import {
  coverageHeadline,
  coverageRows,
  coverageState,
  emptyCoverageWarning,
} from "../lib/export-coverage";
import { fileStem } from "../lib/filename";
import { formatDateTime } from "../lib/format";
import type { CaseRow, ExportCoverage, ExportResult, PackageResult } from "../lib/api";

const mocks = vi.hoisted(() => ({
  caseList: vi.fn(),
  caseCoverage: vi.fn(),
  exportJson: vi.fn(),
  exportCsv: vi.fn(),
  exportDocx: vi.fn(),
  exportPdf: vi.fn(),
  exportPackage: vi.fn(),
}));

const dialog = vi.hoisted(() => ({ save: vi.fn(), open: vi.fn() }));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

// Tauri olmadan ekranı ölçmek için dosya/klasör seçicileri sahtelenir.
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: dialog.save, open: dialog.open }));

const { Exports } = await import("./Exports");

/* ------------------------------------------------------------------ fixtures */

const ISO = "2026-11-02T09:00:00.000Z";

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

function coverage(overrides: Partial<ExportCoverage> = {}): ExportCoverage {
  return {
    documents: 3,
    versions: 4,
    parseRuns: 4,
    pages: 120,
    textPages: 110,
    ocrPages: 8,
    segments: 640,
    emptyPages: 2,
    unparsedDocuments: 0,
    failedParseRuns: 0,
    complete: true,
    warnings: [],
    statement: "Kapsam kasadan okundu; sayılar çekirdekten gelir.",
    generatedAt: ISO,
    ...overrides,
  };
}

const FULL = coverage();
const PARTIAL = coverage({
  complete: false,
  ocrPages: 12,
  emptyPages: 3,
  unparsedDocuments: 2,
  failedParseRuns: 1,
  warnings: ["2 belge ayrıştırılmadı", "3 boş sayfa"],
  statement: "Kapsam kısmi; incelenmedi ≠ temiz.",
});
const EMPTY = coverage({
  documents: 0,
  versions: 0,
  parseRuns: 0,
  pages: 0,
  textPages: 0,
  ocrPages: 0,
  segments: 0,
  emptyPages: 0,
  complete: false,
  warnings: [],
  statement: "Dosyada belge yok.",
});

function exportResult(overrides: Partial<ExportResult> = {}): ExportResult {
  return {
    path: "/Users/avukat/Cikti/kira.json",
    bytes: 20480,
    sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    record: {
      id: "ex-1",
      exportType: "json",
      status: "ok",
      outputHash: "0123456789abcdef",
      createdAt: ISO,
    },
    coverage: FULL,
    ...overrides,
  };
}

function packageResult(overrides: Partial<PackageResult> = {}): PackageResult {
  return {
    path: "/Users/avukat/Devir/kira-paket",
    files: 5,
    sha256: "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    sealedPath: null,
    record: { id: "pk-1", exportType: "package", status: "ok", createdAt: ISO },
    coverage: FULL,
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

function apiError(code: string, message: string) {
  return { code, message };
}

const CSV_RESULT = exportResult({
  path: "/Users/avukat/Cikti/kira.csv",
  record: { id: "ex-2", exportType: "csv", status: "ok", outputHash: null, createdAt: ISO },
});

/** Her testin varsayılan çekirdek/seçici yanıtları (reset sonrası yeniden kurulur). */
function setupDefaults() {
  mocks.caseList.mockResolvedValue([P1, P2]);
  mocks.caseCoverage.mockResolvedValue(FULL);
  mocks.exportJson.mockResolvedValue(exportResult());
  mocks.exportCsv.mockResolvedValue(CSV_RESULT);
  mocks.exportDocx.mockResolvedValue(exportResult());
  mocks.exportPdf.mockResolvedValue(exportResult());
  dialog.save.mockResolvedValue("/Users/avukat/Cikti/kira.json");
  dialog.open.mockResolvedValue("/Users/avukat/Devir");
}

beforeEach(() => {
  vi.clearAllMocks();
  setupDefaults();
});


afterEach(() => {
  cleanup();
});

async function renderScreen() {
  render(<Exports />);
  await screen.findByText(coverageHeadline(FULL));
}

/** Yeşil sonuç bildirimi (metni `<strong>` ile bölünür; düğüm olarak aranır). */
async function findOkNotice(): Promise<HTMLElement> {
  return waitFor(() => {
    const el = document.querySelector(".notice.ok");
    if (!el) throw new Error("yeşil bildirim yok");
    return el as HTMLElement;
  });
}

/* -------------------------------------------------------------------- tests */

describe("Çıktılar ekranı", () => {
  it("dosya listesini kasadan ister, ilk dosyayı seçer ve kapsamı o dosya için okur", async () => {
    await renderScreen();

    expect(mocks.caseList).toHaveBeenCalledWith(false);
    expect(mocks.caseCoverage).toHaveBeenCalledTimes(1);
    expect(mocks.caseCoverage).toHaveBeenCalledWith("p1");

    const select = screen.getByLabelText(t("common.case")) as HTMLSelectElement;
    expect(select.value).toBe("p1");
    expect(screen.getByRole("option", { name: "Kira uyuşmazlığı" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Ceza dosyası" })).toBeTruthy();

    // Kapsam künyesi sunucudan: rozet + tek satır özet + on satır + cümle.
    expect(screen.getByText(coverageState(FULL).label)).toBeTruthy();
    expect(screen.getByText(coverageState(FULL).detail)).toBeTruthy();
    expect(screen.getByText(coverageHeadline(FULL))).toBeTruthy();
    for (const row of coverageRows(FULL)) {
      expect(screen.getByText(row.label)).toBeTruthy();
      expect(screen.getAllByText(String(row.value)).length).toBeGreaterThan(0);
    }
    expect(screen.getByText(FULL.statement)).toBeTruthy();
    expect(screen.queryByText(t("exports.coverageWarnings"))).toBeNull();
  });

  it("kısmi kapsamda uyarıları ve “incelenmedi ≠ temiz” cümlesini yazar", async () => {
    mocks.caseCoverage.mockResolvedValue(PARTIAL);
    render(<Exports />);

    await screen.findByText(coverageHeadline(PARTIAL));
    expect(screen.getByText(coverageState(PARTIAL).label)).toBeTruthy();
    expect(screen.getByText(coverageState(PARTIAL).detail)).toBeTruthy();
    expect(screen.getByText(t("exports.coverageWarnings"))).toBeTruthy();
    for (const warning of PARTIAL.warnings) {
      expect(screen.getByText(warning)).toBeTruthy();
    }
    expect(screen.getByText(PARTIAL.statement)).toBeTruthy();
    expect(emptyCoverageWarning(PARTIAL)).toBeNull();
  });

  it("belgesiz dosyada “Kapsam yok” ve dürüst uyarı yazar", async () => {
    mocks.caseCoverage.mockResolvedValue(EMPTY);
    render(<Exports />);

    await screen.findByText(coverageHeadline(EMPTY));
    expect(screen.getByText(coverageState(EMPTY).label)).toBeTruthy();
    expect(screen.getByText(t("exportCoverage.emptyWarning"))).toBeTruthy();
    // Sıfır sayıları uydurulmaz: hepsi sunucudan gelir.
    expect(screen.getByText(EMPTY.statement)).toBeTruthy();
  });

  it("dosya yokken kapsam isteği gitmez ve altı düğme de kapalıdır", async () => {
    mocks.caseList.mockResolvedValue([]);
    render(<Exports />);

    await screen.findByRole("option", { name: t("common.noCase") });
    expect(mocks.caseCoverage).not.toHaveBeenCalled();

    const buttons = screen.getAllByRole("button");
    const enabled = buttons.filter((b) => !(b as HTMLButtonElement).disabled);
    expect(enabled).toHaveLength(0);
    expect(buttons).toHaveLength(6); // 4 çıktı + kapsam tazele + paket
  });

  it("JSON çıktısında kaydet diyaloğu dosya başlığından ad üretir ve sözleşmeyle yazar", async () => {
    await renderScreen();

    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[0]!);

    await waitFor(() => expect(mocks.exportJson).toHaveBeenCalledTimes(1));
    expect(dialog.save).toHaveBeenCalledWith({
      title: t("exports.saveDialog"),
      defaultPath: `${fileStem(P1.title)}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    expect(mocks.exportJson).toHaveBeenCalledWith("p1", "/Users/avukat/Cikti/kira.json");
    expect(mocks.exportCsv).not.toHaveBeenCalled();

    const notice = await findOkNotice();
    expect(notice.textContent).toContain("JSON");
    expect(notice.textContent).toContain("/Users/avukat/Cikti/kira.json");
    expect(notice.textContent).toContain(`20480 ${t("units.byte")}`);
    expect(notice.textContent).toContain("0123456789abcdef…");
    expect(notice.textContent).toContain(t("exportCoverage.embedWhere.json"));

    // Gösterilen kapsam yanıttan gelir.
    expect(screen.getByText(coverageHeadline(FULL))).toBeTruthy();
  });

  it("iptal edilen kaydet diyaloğu istek üretmez", async () => {
    await renderScreen();
    dialog.save.mockResolvedValue(null);

    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[1]!);
    await waitFor(() => expect(dialog.save).toHaveBeenCalledTimes(1));

    expect(mocks.exportCsv).not.toHaveBeenCalled();
    expect(screen.queryByText(t("exports.writing"))).toBeNull();
  });

  it("düşen dışa aktarma önceki yeşil sonucu bırakmaz", async () => {
    await renderScreen();
    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[0]!);
    await findOkNotice();

    mocks.exportCsv.mockRejectedValue(apiError("E_DISK_FULL", "Disk dolu."));
    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[1]!);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DISK_FULL");
    expect(document.querySelector(".notice.ok")).toBeNull();
  });

  it("yazarken düğme “Yazılıyor…” olur ve öteki çıktılar kilitlenir", async () => {
    await renderScreen();
    const pending = deferred<ExportResult>();
    mocks.exportJson.mockReturnValue(pending.promise);

    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[0]!);

    const writing = await screen.findByRole("button", { name: t("exports.writing") });
    expect((writing as HTMLButtonElement).disabled).toBe(true);
    for (const other of screen.getAllByRole("button", { name: t("common.save") })) {
      expect((other as HTMLButtonElement).disabled).toBe(true);
    }
    expect((screen.getByRole("button", { name: t("exports.packageCreate") }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: t("exports.refresh") }) as HTMLButtonElement).disabled).toBe(true);

    pending.resolve(exportResult());
    await findOkNotice();
    await waitFor(() =>
      expect((screen.getAllByRole("button", { name: t("common.save") })[0] as HTMLButtonElement).disabled).toBe(false),
    );
  });

  it("kapsam tazeleme yeni değerleri yazar ve düşen tazelemenin hatasını sonraki başarıda siler", async () => {
    await renderScreen();

    mocks.caseCoverage.mockResolvedValueOnce(PARTIAL);
    fireEvent.click(screen.getByRole("button", { name: t("exports.refresh") }));
    await screen.findByText(coverageHeadline(PARTIAL));

    mocks.caseCoverage.mockRejectedValueOnce(apiError("E_DB", "Kasa okunamadı."));
    fireEvent.click(screen.getByRole("button", { name: t("exports.refresh") }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DB");

    // Başarılı tazeleme eski kırmızıyı bırakmaz.
    mocks.caseCoverage.mockResolvedValueOnce(FULL);
    fireEvent.click(screen.getByRole("button", { name: t("exports.refresh") }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(screen.getByText(coverageHeadline(FULL))).toBeTruthy();
  });

  it("devir paketi tam 10 karakterlik parolayı olduğu gibi gönderir", async () => {
    await renderScreen();

    // Tam eşik: çekirdek `chars().count() >= 10` ister; 10 karakter gönderilir.
    fireEvent.change(screen.getByLabelText(t("exports.packagePassword")), {
      target: { value: "0123456789" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));

    await waitFor(() => expect(mocks.exportPackage).toHaveBeenCalledTimes(1));
    expect(dialog.open).toHaveBeenCalledWith({
      directory: true,
      multiple: false,
      title: t("exports.dirDialog"),
    });
    expect(mocks.exportPackage).toHaveBeenCalledWith("p1", "/Users/avukat/Devir", "0123456789");
  });

  it("paket künyesini ve şifreli paket yolunu yazar", async () => {
    await renderScreen();
    const created = packageResult({ sealedPath: "/Users/avukat/Devir/kira-paket.dimpak" });
    mocks.exportPackage.mockResolvedValue(created);

    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));

    const ready = await findOkNotice();
    expect(ready.textContent).toContain(t("exports.sealed"));
    expect(ready.textContent).toContain(created.sealedPath!);
    expect(ready.textContent).toContain(
      t("exports.packageFiles", { files: created.files, hash: created.sha256.slice(0, 16) }),
    );
  });

  it("iptal edilen klasör seçimi paket isteği üretmez", async () => {
    await renderScreen();
    dialog.open.mockResolvedValue(null);

    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));
    await waitFor(() => expect(dialog.open).toHaveBeenCalledTimes(1));
    expect(mocks.exportPackage).not.toHaveBeenCalled();
  });

  it("düşen paket isteği önceki paket satırını bırakmaz", async () => {
    await renderScreen();
    mocks.exportPackage.mockResolvedValueOnce(packageResult());
    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));
    await findOkNotice();

    mocks.exportPackage.mockRejectedValueOnce(apiError("E_IO", "Yazılamadı."));
    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_IO");
    expect(document.querySelector(".notice.ok")).toBeNull();
  });

  it("ÖLÇÜLEN KUSUR (A): paket başarısından sonra parola alanı temizlenir", async () => {
    await renderScreen();
    mocks.exportPackage.mockResolvedValue(packageResult());
    const input = screen.getByLabelText(t("exports.packagePassword")) as HTMLInputElement;

    fireEvent.change(input, { target: { value: "cok-guclu-parola" } });
    expect(input.value).toBe("cok-guclu-parola");
    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));

    await findOkNotice();
    await waitFor(() => expect((screen.getByLabelText(t("exports.packagePassword")) as HTMLInputElement).value).toBe(""));

  });

  it("ÖLÇÜLEN KUSUR (B): dosya değişince önceki kapsam, çıktı ve paket satırları kalkar", async () => {
    await renderScreen();
    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[0]!);
    const written = await screen.findByText(/Cikti\/kira\.json/);
    mocks.exportPackage.mockResolvedValue(packageResult());
    fireEvent.click(screen.getByRole("button", { name: t("exports.packageCreate") }));
    await waitFor(() => expect(document.querySelectorAll(".notice.ok")).toHaveLength(2));

    // Yeni dosyanın kapsamı askıda: eski veri ekranda kalmamalı.
    const pending = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValue(pending.promise);
    fireEvent.change(screen.getByLabelText(t("common.case")), { target: { value: "p2" } });

    await waitFor(() => expect(screen.queryByText(/Cikti\/kira\.json/)).toBeNull());
    expect(written.isConnected).toBe(false);
    expect(document.querySelector(".notice.ok")).toBeNull();
    expect(screen.queryByText(coverageHeadline(FULL))).toBeNull();
    expect(screen.queryByText(coverageState(FULL).label)).toBeNull();
    expect(screen.getByText(t("exports.coverageLoading"))).toBeTruthy();
  });

  it("dosya değişince önceki dosyanın hatası da kalkar", async () => {
    await renderScreen();
    mocks.exportJson.mockRejectedValueOnce(apiError("E_DISK_FULL", "Disk dolu."));
    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[0]!);
    expect((await screen.findByRole("alert")).textContent).toContain("E_DISK_FULL");

    mocks.caseCoverage.mockResolvedValueOnce(PARTIAL);
    fireEvent.change(screen.getByLabelText(t("common.case")), { target: { value: "p2" } });

    await screen.findByText(coverageHeadline(PARTIAL));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("uçuştaki dışa aktarma dosya değişince yeni dosyanın altına yazılmaz", async () => {
    await renderScreen();
    const pending = deferred<ExportResult>();
    mocks.exportJson.mockReturnValue(pending.promise);
    fireEvent.click(screen.getAllByRole("button", { name: t("common.save") })[0]!);
    await waitFor(() => expect(mocks.exportJson).toHaveBeenCalledTimes(1));

    mocks.caseCoverage.mockResolvedValueOnce(PARTIAL);
    fireEvent.change(screen.getByLabelText(t("common.case")), { target: { value: "p2" } });
    await screen.findByText(coverageHeadline(PARTIAL));

    pending.resolve(exportResult());
    // İstek p1 için koştu; p2 ekranında "yazıldı" satırı görünmemeli.
    await waitFor(() => expect(document.querySelector(".notice.ok")).toBeNull());
    expect(screen.queryByText(/Cikti\/kira\.json/)).toBeNull();
  });

  it("uçuştaki kapsam tazeleme dosya değişince yeni kapsamı ezmez", async () => {
    await renderScreen();
    const stale = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValueOnce(stale.promise);
    fireEvent.click(screen.getByRole("button", { name: t("exports.refresh") }));
    await waitFor(() => expect(mocks.caseCoverage).toHaveBeenCalledTimes(2));

    mocks.caseCoverage.mockResolvedValueOnce(PARTIAL);
    fireEvent.change(screen.getByLabelText(t("common.case")), { target: { value: "p2" } });
    await screen.findByText(coverageHeadline(PARTIAL));

    stale.resolve(FULL);
    await waitFor(() => expect(screen.queryByText(coverageHeadline(FULL))).toBeNull());
    expect(screen.getByText(coverageHeadline(PARTIAL))).toBeTruthy();
  });


  it("ÖLÇÜLEN KUSUR (C): düşen kapsam isteği önceki dosyanın kapsamını bırakmaz", async () => {
    await renderScreen();
    mocks.caseCoverage.mockRejectedValue(apiError("E_DB", "Kapsam okunamadı."));

    fireEvent.change(screen.getByLabelText(t("common.case")), { target: { value: "p2" } });

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DB");
    expect(screen.queryByText(coverageHeadline(FULL))).toBeNull();
    expect(screen.getByText(t("exports.coverageUnreadable"))).toBeTruthy();
  });

  it("ÖLÇÜLEN KUSUR (D): kapsam okunurken “okunamadı” yazılmaz", async () => {
    const pending = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValue(pending.promise);

    render(<Exports />);
    await waitFor(() => expect(mocks.caseCoverage).toHaveBeenCalledWith("p1"));
    expect(screen.queryByText(t("exports.coverageUnreadable"))).toBeNull();

    pending.resolve(FULL);
    await screen.findByText(coverageHeadline(FULL));
  });

  it("geç gelen eski kapsam yanıtı yeni dosyanın kapsamını ezmez", async () => {
    const first = deferred<ExportCoverage>();
    const second = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    render(<Exports />);
    await waitFor(() => expect(mocks.caseCoverage).toHaveBeenCalledWith("p1"));
    fireEvent.change(screen.getByLabelText(t("common.case")), { target: { value: "p2" } });
    await waitFor(() => expect(mocks.caseCoverage).toHaveBeenCalledWith("p2"));

    second.resolve(PARTIAL);
    await screen.findByText(coverageHeadline(PARTIAL));
    first.resolve(FULL);
    await waitFor(() => expect(screen.queryByText(coverageHeadline(FULL))).toBeNull());
    expect(screen.getByText(coverageHeadline(PARTIAL))).toBeTruthy();
  });

  it("dosya listesi düşerse dürüst hata yazar, sahte liste üretmez", async () => {
    mocks.caseList.mockRejectedValue(apiError("E_DB", "Kasa açılamadı."));
    render(<Exports />);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DB");
    expect(screen.getByRole("option", { name: t("common.noCase") })).toBeTruthy();
    expect(mocks.caseCoverage).not.toHaveBeenCalled();
  });
});

/* --------------------------------------------------- uçuş yolları (19–25) */

describe("Çıktılar ekranı uçuş yolları", () => {
  // Bir önceki blok `clearAllMocks` kullanır: bu **bir kezlik** (`...Once`)
  // kuyruğunu bırakır ve düşen bir testin artığı sonraki testin yanıtını
  // belirler. Burada sıfırdan kurulur.
  beforeEach(() => {
    vi.resetAllMocks();
    setupDefaults();
  });

  const saveButtons = () =>
    screen.getAllByRole("button", { name: t("common.save") }) as HTMLButtonElement[];
  const caseSelect = () => screen.getByLabelText(t("common.case")) as HTMLSelectElement;
  const packageButton = () =>
    screen.getByRole("button", { name: t("exports.packageCreate") }) as HTMLButtonElement;
  const refreshButton = () =>
    screen.getByRole("button", { name: t("exports.refresh") }) as HTMLButtonElement;
  const passwordInput = () =>
    screen.getByLabelText(t("exports.packagePassword")) as HTMLInputElement;

  /** Dosya değiştirir ve yeni dosyanın kapsamını bekler. */
  async function switchToP2() {
    mocks.caseCoverage.mockResolvedValueOnce(PARTIAL);
    fireEvent.change(caseSelect(), { target: { value: "p2" } });
    await screen.findByText(coverageHeadline(PARTIAL));
  }

  it("uçuştaki dışa aktarma dosya değişince yeni dosyanın düğmelerini kilitlemez", async () => {
    await renderScreen();
    const pending = deferred<ExportResult>();
    mocks.exportJson.mockReturnValue(pending.promise);

    fireEvent.click(saveButtons()[0]!);
    await waitFor(() => expect(mocks.exportJson).toHaveBeenCalledTimes(1));
    // Kilit gerçekten tutuluyor: JSON düğmesi "Yazılıyor…", ötekiler kapalı.
    // Durum güncellemesi istek kaydından sonra boyanır → bekle (CI'da yarıştı).
    await waitFor(() => expect(saveButtons()).toHaveLength(3));
    await waitFor(() => expect(refreshButton().disabled).toBe(true));

    await switchToP2();

    // Dosya değişti: kilit devredilir, yeni dosyanın altı düğmesi açılır.
    expect(saveButtons()).toHaveLength(4);
    for (const button of saveButtons()) expect(button.disabled).toBe(false);
    expect(packageButton().disabled).toBe(false);
    expect(refreshButton().disabled).toBe(false);

    // Devredilen isteğin yanıtı yeni dosyanın altına yazılmaz.
    await act(async () => {
      pending.resolve(exportResult());
      await pending.promise;
    });
    expect(document.querySelector(".notice.ok")).toBeNull();
    expect(saveButtons()).toHaveLength(4);
  });

  it("devredilen yazmanın kapanışı yeni dosyada başlatılan yazmanın kilidini açmaz", async () => {
    await renderScreen();
    const stale = deferred<ExportResult>();
    mocks.exportJson.mockReturnValue(stale.promise);
    fireEvent.click(saveButtons()[0]!);
    await waitFor(() => expect(mocks.exportJson).toHaveBeenCalledTimes(1));

    await switchToP2();

    const second = deferred<ExportResult>();
    mocks.exportCsv.mockReturnValue(second.promise);
    fireEvent.click(saveButtons()[1]!);
    await waitFor(() => expect(mocks.exportCsv).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("button", { name: t("exports.writing") })).toBeTruthy();

    // Eski istek döner: yeni yazmanın kilidi **açılmamalı**.
    await act(async () => {
      stale.resolve(exportResult());
      await stale.promise;
    });
    const writing = screen.getByRole("button", { name: t("exports.writing") }) as HTMLButtonElement;
    expect(writing.disabled).toBe(true);

    await act(async () => {
      second.resolve(CSV_RESULT);
      await second.promise;
    });
    const notice = await findOkNotice();
    expect(notice.textContent).toContain("CSV");
    expect(notice.textContent).toContain(`/Users/avukat/Cikti/kira.csv`);
  });

  it("paket başarısı uçuşta yazılan yeni parolayı silmez", async () => {
    await renderScreen();
    const pending = deferred<PackageResult>();
    mocks.exportPackage.mockReturnValue(pending.promise);

    fireEvent.change(passwordInput(), { target: { value: "cok-guclu-parola" } });
    fireEvent.click(packageButton());
    await waitFor(() =>
      expect(mocks.exportPackage).toHaveBeenCalledWith("p1", "/Users/avukat/Devir", "cok-guclu-parola"),
    );

    // Uçuşta yeni parola yazılır (bir sonraki paket için).
    fireEvent.change(passwordInput(), { target: { value: "yeni-parola-1234" } });
    await act(async () => {
      pending.resolve(packageResult());
      await pending.promise;
    });

    expect(passwordInput().value).toBe("yeni-parola-1234");
  });

  it("kaydet diyaloğu açıkken ikinci tıklama ikinci pencere ve ikinci yazma üretmez", async () => {
    await renderScreen();
    const picked = deferred<string | null>();
    dialog.save.mockReturnValue(picked.promise);

    fireEvent.click(saveButtons()[0]!);
    await waitFor(() => expect(dialog.save).toHaveBeenCalledTimes(1));
    fireEvent.click(saveButtons()[1]!);
    expect(dialog.save).toHaveBeenCalledTimes(1);

    await act(async () => {
      picked.resolve("/Users/avukat/Cikti/kira.json");
      await picked.promise;
    });
    await waitFor(() => expect(mocks.exportJson).toHaveBeenCalledTimes(1));
    expect(mocks.exportCsv).not.toHaveBeenCalled();
  });

  it("klasör seçimi açıkken ikinci tıklama ikinci paket penceresi açmaz", async () => {
    await renderScreen();
    const picked = deferred<string | null>();
    dialog.open.mockReturnValue(picked.promise);

    fireEvent.click(packageButton());
    await waitFor(() => expect(dialog.open).toHaveBeenCalledTimes(1));
    fireEvent.click(packageButton());
    expect(dialog.open).toHaveBeenCalledTimes(1);

    await act(async () => {
      picked.resolve("/Users/avukat/Devir");
      await picked.promise;
    });
    await waitFor(() => expect(mocks.exportPackage).toHaveBeenCalledTimes(1));
  });

  it("düşen kaydet diyaloğu sessiz kalmaz", async () => {
    await renderScreen();
    dialog.save.mockRejectedValue(new Error("seçici kapandı"));

    fireEvent.click(saveButtons()[0]!);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DIALOG");
    expect(alert.textContent).toContain(t("exports.savePickFailed"));
    expect(mocks.exportJson).not.toHaveBeenCalled();
  });

  it("düşen klasör seçici sessiz kalmaz", async () => {
    await renderScreen();
    dialog.open.mockRejectedValue(new Error("seçici kapandı"));

    fireEvent.click(packageButton());

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DIALOG");
    expect(alert.textContent).toContain(t("exports.dirPickFailed"));
    expect(mocks.exportPackage).not.toHaveBeenCalled();
  });

  it("uçuştaki paket yanıtı dosya değişince yeni dosyanın altına yazılmaz", async () => {
    await renderScreen();
    const pending = deferred<PackageResult>();
    mocks.exportPackage.mockReturnValue(pending.promise);
    fireEvent.click(packageButton());
    await waitFor(() => expect(mocks.exportPackage).toHaveBeenCalledTimes(1));

    await switchToP2();

    await act(async () => {
      pending.resolve(packageResult());
      await pending.promise;
    });
    expect(document.querySelector(".notice.ok")).toBeNull();
    expect(screen.queryByText(/Devir\/kira-paket/)).toBeNull();
  });

  it("devredilen paketin kapanışı yeni dosyada başlatılan yazmanın kilidini açmaz", async () => {
    await renderScreen();
    const stalePkg = deferred<PackageResult>();
    mocks.exportPackage.mockReturnValue(stalePkg.promise);
    fireEvent.click(packageButton());
    await waitFor(() => expect(mocks.exportPackage).toHaveBeenCalledTimes(1));

    await switchToP2();

    const second = deferred<ExportResult>();
    mocks.exportJson.mockReturnValue(second.promise);
    fireEvent.click(saveButtons()[0]!);
    await waitFor(() => expect(mocks.exportJson).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("button", { name: t("exports.writing") })).toBeTruthy();

    // Eski paket isteği döner: yeni yazmanın kilidi **açılmamalı**.
    await act(async () => {
      stalePkg.resolve(packageResult());
      await stalePkg.promise;
    });
    const writing = screen.getByRole("button", { name: t("exports.writing") }) as HTMLButtonElement;
    expect(writing.disabled).toBe(true);

    await act(async () => {
      second.resolve(exportResult());
      await second.promise;
    });
    expect(await findOkNotice()).toBeTruthy();
  });
});

/* ------------------------------------------- kapsam şeridi ve diyalog uçuşu */

describe("Çıktılar ekranı kapsam şeridi", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    setupDefaults();
  });

  const saveButtons = () =>
    screen.getAllByRole("button", { name: t("common.save") }) as HTMLButtonElement[];
  const caseSelect = () => screen.getByLabelText(t("common.case")) as HTMLSelectElement;
  const packageButton = () =>
    screen.getByRole("button", { name: t("exports.packageCreate") }) as HTMLButtonElement;
  const refreshButton = () =>
    screen.getByRole("button", { name: t("exports.refresh") }) as HTMLButtonElement;

  /** Dosya değiştirir ve yeni dosyanın kapsamını bekler. */
  async function switchToP2() {
    mocks.caseCoverage.mockResolvedValueOnce(PARTIAL);
    fireEvent.change(caseSelect(), { target: { value: "p2" } });
    await screen.findByText(coverageHeadline(PARTIAL));
  }

  it("kaydet diyaloğu açıkken dosya değişirse eski dosyanın dışa aktarması başlatılmaz", async () => {
    await renderScreen();
    const picked = deferred<string | null>();
    dialog.save.mockReturnValue(picked.promise);

    fireEvent.click(saveButtons()[0]!);
    await waitFor(() => expect(dialog.save).toHaveBeenCalledTimes(1));

    await switchToP2();

    await act(async () => {
      picked.resolve("/Users/avukat/Cikti/kira.json");
      await picked.promise;
    });

    // Seçim yanıtı geldiğinde hedef dosya artık ekranda değil: yazma başlamaz.
    expect(mocks.exportJson).not.toHaveBeenCalled();
    expect(document.querySelector(".notice.ok")).toBeNull();
    // Yeni dosyanın kapsamı da eski dosyanın çıktısıyla ezilmez.
    expect(screen.getByText(coverageHeadline(PARTIAL))).toBeTruthy();
    expect(screen.queryByText(coverageHeadline(FULL))).toBeNull();
  });

  it("klasör seçimi açıkken dosya değişirse eski dosyanın paketi oluşturulmaz", async () => {
    await renderScreen();
    const picked = deferred<string | null>();
    dialog.open.mockReturnValue(picked.promise);

    fireEvent.click(packageButton());
    await waitFor(() => expect(dialog.open).toHaveBeenCalledTimes(1));

    await switchToP2();

    await act(async () => {
      picked.resolve("/Users/avukat/Devir");
      await picked.promise;
    });

    expect(mocks.exportPackage).not.toHaveBeenCalled();
    expect(document.querySelector(".notice.ok")).toBeNull();
    expect(screen.getByText(coverageHeadline(PARTIAL))).toBeTruthy();
  });

  it("dışa aktarma başlarken uçuştaki kapsam tazelemesi düşmez", async () => {
    await renderScreen();
    const read = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValueOnce(read.promise);
    fireEvent.click(refreshButton());
    await waitFor(() => expect(mocks.caseCoverage).toHaveBeenCalledTimes(2));

    const write = deferred<ExportResult>();
    mocks.exportJson.mockReturnValue(write.promise);
    fireEvent.click(saveButtons()[0]!);
    await waitFor(() => expect(mocks.exportJson).toHaveBeenCalledTimes(1));

    // Yazma düşer; kullanıcının istediği tazeleme yine de yazılır.
    await act(async () => {
      write.reject(apiError("E_DISK_FULL", "Disk dolu."));
      await write.promise.catch(() => undefined);
    });
    expect((await screen.findByRole("alert")).textContent).toContain("E_DISK_FULL");

    await act(async () => {
      read.resolve(PARTIAL);
      await read.promise;
    });
    expect(screen.getByText(coverageHeadline(PARTIAL))).toBeTruthy();
  });

  it("kapsam okunurken “Yenile” ikinci okuma başlatmaz", async () => {
    const first = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValueOnce(first.promise);

    render(<Exports />);
    await waitFor(() => expect(mocks.caseCoverage).toHaveBeenCalledTimes(1));
    // Kilit durumu istek kaydından sonra boyanır: CI'da bir kez yarıştı.
    await waitFor(() => expect(refreshButton().disabled).toBe(true));

    fireEvent.click(refreshButton());
    expect(mocks.caseCoverage).toHaveBeenCalledTimes(1);

    await act(async () => {
      first.resolve(FULL);
      await first.promise;
    });
    await screen.findByText(coverageHeadline(FULL));
    await waitFor(() => expect(refreshButton().disabled).toBe(false));
  });

  it("yazma başarısı kapsam kilidini devralır ve eski okumayı geçersiz kılar", async () => {
    mocks.caseCoverage.mockRejectedValue(apiError("E_DB", "Kapsam okunamadı."));
    render(<Exports />);
    await screen.findByRole("alert");
    expect(screen.getByText(t("exports.coverageUnreadable"))).toBeTruthy();

    const stale = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValueOnce(stale.promise);
    fireEvent.click(refreshButton());
    await waitFor(() => expect(refreshButton().disabled).toBe(true));
    expect(screen.getByText(t("exports.coverageLoading"))).toBeTruthy();

    // Yazma başarısı kapsamı yanıttan yazar ve okuma kilidini devralır:
    // asılı “Kapsam okunuyor…” satırı kalmaz, düğme yeniden açılır.
    fireEvent.click(saveButtons()[0]!);
    await findOkNotice();
    await waitFor(() => expect(refreshButton().disabled).toBe(false));
    expect(screen.getByText(coverageHeadline(FULL))).toBeTruthy();
    expect(screen.queryByText(t("exports.coverageLoading"))).toBeNull();

    // Uçuştaki eski okuma geç döner: yazılan bloğun kapsamını ezmez.
    await act(async () => {
      stale.resolve(PARTIAL);
      await stale.promise;
    });
    expect(screen.queryByText(coverageHeadline(PARTIAL))).toBeNull();
    expect(screen.getByText(coverageHeadline(FULL))).toBeTruthy();
  });

  it("paket başarısı da kapsam şeridini devralır", async () => {
    mocks.caseCoverage.mockRejectedValue(apiError("E_DB", "Kapsam okunamadı."));
    mocks.exportPackage.mockResolvedValue(packageResult());
    render(<Exports />);
    await screen.findByRole("alert");

    const stale = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValueOnce(stale.promise);
    fireEvent.click(refreshButton());
    await waitFor(() => expect(refreshButton().disabled).toBe(true));

    fireEvent.click(packageButton());
    await findOkNotice();
    await waitFor(() => expect(refreshButton().disabled).toBe(false));

    await act(async () => {
      stale.resolve(PARTIAL);
      await stale.promise;
    });
    expect(screen.queryByText(coverageHeadline(PARTIAL))).toBeNull();
    expect(screen.getByText(coverageHeadline(FULL))).toBeTruthy();
  });

  it("dosya değişiminin kapsam okuması, ilgisiz bir yazma başlayınca düşmez", async () => {
    await renderScreen();
    const read = deferred<ExportCoverage>();
    mocks.caseCoverage.mockReturnValueOnce(read.promise);
    fireEvent.change(caseSelect(), { target: { value: "p2" } });
    await waitFor(() => expect(mocks.caseCoverage).toHaveBeenCalledTimes(2));

    const write = deferred<ExportResult>();
    mocks.exportJson.mockReturnValue(write.promise);
    fireEvent.click(saveButtons()[0]!);
    await waitFor(() => expect(mocks.exportJson).toHaveBeenCalledTimes(1));

    // Yazma düşer; yeni dosyanın kapsam okuması yine de yazılır.
    await act(async () => {
      write.reject(apiError("E_IO", "Yazılamadı."));
      await write.promise.catch(() => undefined);
    });
    await act(async () => {
      read.resolve(PARTIAL);
      await read.promise;
    });
    expect(screen.getByText(coverageHeadline(PARTIAL))).toBeTruthy();
    expect(screen.queryByText(t("exports.coverageLoading"))).toBeNull();
  });
});

/* ------------------------------- paket ve kapsam dürüstlüğü (32–39) */

describe("Çıktılar ekranı paket ve kapsam dürüstlüğü", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    setupDefaults();
  });

  const saveButtons = () =>
    screen.getAllByRole("button", { name: t("common.save") }) as HTMLButtonElement[];
  const packageButton = () =>
    screen.getByRole("button", { name: t("exports.packageCreate") }) as HTMLButtonElement;
  const refreshButton = () =>
    screen.getByRole("button", { name: t("exports.refresh") }) as HTMLButtonElement;
  const passwordInput = () =>
    screen.getByLabelText(t("exports.packagePassword")) as HTMLInputElement;
  const caseSelect = () => screen.getByLabelText(t("common.case")) as HTMLSelectElement;
  const vaultLine = (at: string) => t("exports.coverageFromVault", { at });

  it("parola yazıldıysa ama 10 karakterden kısaysa paket hiç yazılmaz", async () => {
    await renderScreen();
    fireEvent.change(passwordInput(), { target: { value: "kisa" } });
    fireEvent.click(packageButton());

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_PACKAGE_PASSWORD");
    expect(alert.textContent).toContain(t("exports.packagePasswordTooShort", { count: 4 }));
    // Ne seçici açılır ne istek gider: açık metin klasör sessizce yazılmaz.
    expect(dialog.open).not.toHaveBeenCalled();
    expect(mocks.exportPackage).not.toHaveBeenCalled();
    expect(passwordInput().value).toBe("kisa");
    expect(document.querySelector(".notice.ok")).toBeNull();
  });

  it("parola kırpılmadan, yazıldığı gibi gönderilir", async () => {
    await renderScreen();
    // Eski kapı `trim().length` sayıyordu: bu 11 birimlik parola 7 sayılıp
    // **sessizce düşüyor** ve açık metin klasör yazılıyordu. Çekirdek ham dizeyi
    // `chars().count()` ile sayar → kapı da ham dizeyi sayar ve aynen gönderir.
    const typed = "  1234567  ";
    mocks.exportPackage.mockResolvedValue(
      packageResult({ sealedPath: "/Users/avukat/Devir/kira-devir.dimpak" }),
    );
    fireEvent.change(passwordInput(), { target: { value: typed } });
    fireEvent.click(packageButton());

    await waitFor(() =>
      expect(mocks.exportPackage).toHaveBeenCalledWith("p1", "/Users/avukat/Devir", typed),
    );
    const notice = await findOkNotice();
    // Çekirdek şifreledi: "şifrelenmedi" uyarısı çıkmamalı.
    expect(notice.textContent).toContain(t("exports.sealedConverted"));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("parola ölçüsü çekirdekle aynı: kod noktası sayılır, UTF-16 birimi değil", async () => {
    await renderScreen();
    // 5 emoji = 10 UTF-16 birimi; çekirdek `chars().count()` ile 5 sayar ve
    // paketi **sessizce** açık metin yazardı.
    fireEvent.change(passwordInput(), { target: { value: "🔒🔒🔒🔒🔒" } });
    fireEvent.click(packageButton());

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_PACKAGE_PASSWORD");
    expect(mocks.exportPackage).not.toHaveBeenCalled();
  });

  it("parola gönderildiği hâlde şifrelenmemiş yanıt sessiz kalmaz", async () => {
    await renderScreen();
    // Çekirdek parolayı aldı ama `sealedPath` dönmedi: paket açık metindir.
    mocks.exportPackage.mockResolvedValue(packageResult({ sealedPath: null }));
    fireEvent.change(passwordInput(), { target: { value: "cok-guclu-parola" } });
    fireEvent.click(packageButton());

    const notice = await findOkNotice();
    expect(notice.textContent).toContain(t("exports.packagePlaintext"));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_PACKAGE_NOT_SEALED");
  });

  it("şifreli pakette birincil konum .dimpak dosyasıdır, silinmiş klasör değil", async () => {
    await renderScreen();
    const created = packageResult({
      path: "/Users/avukat/Devir/gecici-kira",
      sealedPath: "/Users/avukat/Devir/kira-devir.dimpak",
    });
    mocks.exportPackage.mockResolvedValue(created);
    fireEvent.click(packageButton());

    const notice = await findOkNotice();
    expect(notice.textContent).toContain(created.sealedPath!);
    // Çekirdek geçici klasörü siler: o yol artık yok, künyede de yazılmaz.
    expect(notice.textContent).not.toContain(created.path);
    expect(notice.textContent).toContain(t("exports.sealedConverted"));
  });

  it("kapsam paneli kaynağını ve okunduğu zamanı yazar", async () => {
    await renderScreen();
    expect(screen.getByText(vaultLine(formatDateTime(FULL.generatedAt)))).toBeTruthy();

    const exported = coverage({ generatedAt: "2026-11-02T10:30:00.000Z" });
    mocks.exportJson.mockResolvedValue(exportResult({ coverage: exported }));
    fireEvent.click(saveButtons()[0]!);
    await findOkNotice();
    expect(
      screen.getByText(t("exports.coverageFromExport", { at: formatDateTime(exported.generatedAt) })),
    ).toBeTruthy();
    expect(screen.queryByText(vaultLine(formatDateTime(FULL.generatedAt)))).toBeNull();

    // Dosya değişimi yeniden kasadan okur: kaynak "vault"a döner ve zaman tazelenir.
    const other = coverage({ generatedAt: "2026-11-02T12:30:00.000Z" });
    mocks.caseCoverage.mockResolvedValueOnce(other);
    fireEvent.change(caseSelect(), { target: { value: "p2" } });
    await screen.findByText(vaultLine(formatDateTime(other.generatedAt)));
    expect(
      screen.queryByText(t("exports.coverageFromExport", { at: formatDateTime(exported.generatedAt) })),
    ).toBeNull();
  });

  it("yazma sonrası kapsam tazelemesi kaynağı kasadan okumaya döndürür", async () => {
    await renderScreen();
    const exported = coverage({ generatedAt: "2026-11-02T10:30:00.000Z" });
    mocks.exportJson.mockResolvedValue(exportResult({ coverage: exported }));
    fireEvent.click(saveButtons()[0]!);
    await findOkNotice();

    const refreshed = coverage({ generatedAt: "2026-11-02T13:15:00.000Z" });
    mocks.caseCoverage.mockResolvedValueOnce(refreshed);
    fireEvent.click(refreshButton());
    await screen.findByText(vaultLine(formatDateTime(refreshed.generatedAt)));
    expect(
      screen.queryByText(t("exports.coverageFromExport", { at: formatDateTime(exported.generatedAt) })),
    ).toBeNull();
  });

  it("paket sonrası kapsam kaynağı devir paketidir", async () => {
    await renderScreen();
    const packed = coverage({ generatedAt: "2026-11-02T10:45:00.000Z" });
    mocks.exportPackage.mockResolvedValue(packageResult({ coverage: packed }));
    fireEvent.click(packageButton());
    await findOkNotice();

    expect(
      screen.getByText(t("exports.coverageFromPackage", { at: formatDateTime(packed.generatedAt) })),
    ).toBeTruthy();
    expect(screen.queryByText(vaultLine(formatDateTime(FULL.generatedAt)))).toBeNull();
  });

  it("açık metin paket künyesi parolanın uygulanmadığını yazar", async () => {
    await renderScreen();
    mocks.exportPackage.mockResolvedValue(packageResult());
    fireEvent.click(packageButton());

    const notice = await findOkNotice();
    expect(notice.textContent).toContain(t("exports.packagePlaintext"));
    expect(notice.textContent).not.toContain(t("exports.sealedConverted"));
    // Parola alanı boş: istek `undefined` parolayla gider (açık metin bilinçli).
    expect(mocks.exportPackage).toHaveBeenCalledWith("p1", "/Users/avukat/Devir", undefined);
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
