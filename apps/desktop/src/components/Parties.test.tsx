// @vitest-environment jsdom
//
// Dosya tarafları kartı davranış testi (şartname F08-T01 / REQ-010).
//
// Kart gerçek çekirdek komutlarını çağırır; burada `api` kısmi mock'lanır ve
// **bağlantı** ölçülür: hangi düğme hangi komutu hangi argümanla çağırıyor,
// kırpma yapılıyor mu, birleştirme onay kapısı çalışıyor mu, düşen okuma
// "taraf yok" diyor mu ve dosya değişince önceki dosyanın verisi ekranda
// kalıyor mu (INV-05).

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ActorMergeRow, CaseParties, CaseParticipantRow } from "../lib/api";

const mocks = vi.hoisted(() => ({
  casePartyList: vi.fn(),
  casePartyAdd: vi.fn(),
  casePartyRemove: vi.fn(),
  actorMerge: vi.fn(),
  actorMergeUndo: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      casePartyList: mocks.casePartyList,
      casePartyAdd: mocks.casePartyAdd,
      casePartyRemove: mocks.casePartyRemove,
      actorMerge: mocks.actorMerge,
      actorMergeUndo: mocks.actorMergeUndo,
    },
  };
});

const { Parties } = await import("./Parties");

const ISO = "2026-11-08T10:00:00Z";

function participant(overrides: Partial<CaseParticipantRow> = {}): CaseParticipantRow {
  return {
    id: "p1",
    caseId: "c1",
    actorId: "a1",
    displayName: "Ayşe Yılmaz",
    kind: "person",
    role: "davacı",
    roleNote: "vekili",
    createdAt: ISO,
    ...overrides,
  };
}

function mergeRow(overrides: Partial<ActorMergeRow> = {}): ActorMergeRow {
  return {
    id: "m1",
    sourceActorId: "a2",
    sourceDisplayName: "A. Yılmaz",
    targetActorId: "a1",
    targetDisplayName: "Ayşe Yılmaz",
    movedParticipantIds: ["p2"],
    caseIds: ["c1"],
    reviewerLabel: "local-user",
    createdAt: ISO,
    undoneAt: null,
    ...overrides,
  };
}

function parties(rows: CaseParticipantRow[], merges: ActorMergeRow[] = []): CaseParties {
  return { participants: rows, merges };
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

async function expectText(text: string) {
  await waitFor(() => expect(screen.queryAllByText(text).length).toBeGreaterThan(0));
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.casePartyList.mockResolvedValue(parties([participant()]));
});

afterEach(() => {
  cleanup();
});

describe("dosya tarafları", () => {
  it("tarafları çekirdekten okur; tür etiketi ve rol notu görünür", async () => {
    mocks.casePartyList.mockResolvedValue(
      parties([
        participant(),
        participant({
          id: "p2",
          actorId: "a2",
          displayName: "Örnek A.Ş.",
          kind: "organization",
          role: "davalı",
          roleNote: null,
        }),
      ]),
    );
    render(<Parties caseId="c1" />);
    await expectText("Ayşe Yılmaz");
    await expectText("Örnek A.Ş.");
    expect(mocks.casePartyList).toHaveBeenCalledWith("c1");
    // Tür sözlükten gelir; bilinmeyen tür ham gösterilir (aşağıdaki testte).
    expect(screen.getAllByText("Kişi").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Kurum").length).toBeGreaterThan(0);
    expect(screen.getAllByText("vekili").length).toBeGreaterThan(0);
    expect(screen.getAllByText("davacı").length).toBeGreaterThan(0);
  });

  it("yeni taraf eklerken ad ve rol kırpılarak gönderilir", async () => {
    render(<Parties caseId="c1" />);
    await expectText("Ayşe Yılmaz");
    fireEvent.change(screen.getByLabelText("Ad veya unvan"), { target: { value: "  Mehmet Demir  " } });
    fireEvent.change(screen.getByLabelText("Rol"), { target: { value: "  tanık  " } });
    fireEvent.change(screen.getByLabelText("Rol notu (isteğe bağlı)"), { target: { value: "  komşu  " } });
    mocks.casePartyAdd.mockResolvedValue(
      participant({ id: "p9", actorId: "a9", displayName: "Mehmet Demir", role: "tanık", roleNote: "komşu" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Taraf ekle" }));
    await waitFor(() =>
      expect(mocks.casePartyAdd).toHaveBeenCalledWith("c1", {
        displayName: "Mehmet Demir",
        kind: "person",
        role: "tanık",
        roleNote: "komşu",
        actorId: null,
      }),
    );
    // Form temizlenir, liste tazelenir ve dürüst bildirim yazılır.
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Taraf eklendi."));
    expect((screen.getByLabelText("Rol") as HTMLInputElement).value).toBe("");
    expect(mocks.casePartyList).toHaveBeenCalledTimes(2);
  });

  it("mevcut tarafa ikinci rol eklerken yeni kişi açılmaz", async () => {
    mocks.casePartyList.mockResolvedValue(
      parties([
        participant(),
        participant({ id: "p2", actorId: "a2", displayName: "Örnek A.Ş.", kind: "organization", role: "davalı" }),
      ]),
    );
    render(<Parties caseId="c1" />);
    await expectText("Örnek A.Ş.");
    const existing = screen.getByLabelText("Mevcut taraf") as HTMLSelectElement;
    fireEvent.change(existing, { target: { value: "a2" } });
    // Ad ve tür alanları mevcut taraf seçilince kilitlenir (uydurma ad yok).
    expect((screen.getByLabelText("Ad veya unvan") as HTMLInputElement).disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Rol"), { target: { value: "tanık" } });
    mocks.casePartyAdd.mockResolvedValue(participant({ id: "p3", actorId: "a2", role: "tanık" }));
    fireEvent.click(screen.getByRole("button", { name: "Taraf ekle" }));
    await waitFor(() =>
      expect(mocks.casePartyAdd).toHaveBeenCalledWith("c1", {
        displayName: "Örnek A.Ş.",
        kind: "organization",
        role: "tanık",
        roleNote: null,
        actorId: "a2",
      }),
    );
  });

  it("rol boşken ekleme kapalıdır (çekirdeğe boş istek gitmez)", async () => {
    render(<Parties caseId="c1" />);
    await expectText("Ayşe Yılmaz");
    expect((screen.getByRole("button", { name: "Taraf ekle" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Rol"), { target: { value: "tanık" } });
    expect((screen.getByRole("button", { name: "Taraf ekle" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Ad veya unvan"), { target: { value: "Ali" } });
    expect((screen.getByRole("button", { name: "Taraf ekle" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("kaldırma çekirdeğe gider ve listeyi tazeler", async () => {
    render(<Parties caseId="c1" />);
    await expectText("Ayşe Yılmaz");
    mocks.casePartyRemove.mockResolvedValue(undefined);
    mocks.casePartyList.mockResolvedValue(parties([]));
    fireEvent.click(screen.getByRole("button", { name: "Kaldır" }));
    await waitFor(() => expect(mocks.casePartyRemove).toHaveBeenCalledWith("p1"));
    await expectText("Bu dosyada kayıtlı taraf yok.");
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Taraf kaldırıldı."));
  });

  it("birleştirme onay kutusu olmadan çağrılmaz; onayla çağrılır ve geri alınabilir", async () => {
    mocks.casePartyList.mockResolvedValue(
      parties([
        participant(),
        participant({ id: "p2", actorId: "a2", displayName: "A. Yılmaz", role: "tanık" }),
      ]),
    );
    render(<Parties caseId="c1" />);
    await expectText("A. Yılmaz");
    fireEvent.change(screen.getByLabelText("Kaynak"), { target: { value: "a2" } });
    fireEvent.change(screen.getByLabelText("Hedef"), { target: { value: "a1" } });
    const mergeButton = screen.getByRole("button", { name: "Birleştir" }) as HTMLButtonElement;
    expect(mergeButton.disabled).toBe(true);
    fireEvent.click(screen.getByLabelText("Birleştirmeyi onaylıyorum"));
    expect(mergeButton.disabled).toBe(false);
    mocks.actorMerge.mockResolvedValue(mergeRow());
    mocks.casePartyList.mockResolvedValue(
      parties([participant(), participant({ id: "p2", actorId: "a1", displayName: "Ayşe Yılmaz", role: "tanık" })], [
        mergeRow(),
      ]),
    );
    fireEvent.click(mergeButton);
    await waitFor(() => expect(mocks.actorMerge).toHaveBeenCalledWith("a2", "a1", "local-user"));
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Birleştirildi."));
    await expectText("A. Yılmaz → Ayşe Yılmaz");

    mocks.actorMergeUndo.mockResolvedValue(mergeRow({ undoneAt: ISO }));
    mocks.casePartyList.mockResolvedValue(
      parties([participant(), participant({ id: "p2", actorId: "a2", displayName: "A. Yılmaz", role: "tanık" })], [
        mergeRow({ undoneAt: ISO }),
      ]),
    );
    fireEvent.click(screen.getByRole("button", { name: "Geri al" }));
    await waitFor(() => expect(mocks.actorMergeUndo).toHaveBeenCalledWith("m1"));
    await expectText("geri alındı");
  });

  it("aynı taraf kaynak ve hedef seçilirse birleştirme kapalıdır", async () => {
    mocks.casePartyList.mockResolvedValue(
      parties([
        participant(),
        participant({ id: "p2", actorId: "a2", displayName: "A. Yılmaz", role: "tanık" }),
      ]),
    );
    render(<Parties caseId="c1" />);
    await expectText("A. Yılmaz");
    fireEvent.change(screen.getByLabelText("Kaynak"), { target: { value: "a1" } });
    fireEvent.change(screen.getByLabelText("Hedef"), { target: { value: "a1" } });
    fireEvent.click(screen.getByLabelText("Birleştirmeyi onaylıyorum"));
    expect((screen.getByRole("button", { name: "Birleştir" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("düşen okuma 'taraf yok' demez; hata ayrı satırda yazılır", async () => {
    mocks.casePartyList.mockRejectedValue({ code: "E_DB_ACTOR", message: "kayıt okunamadı", retryable: false });
    render(<Parties caseId="c1" />);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_DB_ACTOR");
    expect(screen.queryByText("Bu dosyada kayıtlı taraf yok.")).toBeNull();
  });

  it("dosya değişince önceki dosyanın tarafları ekranda kalmaz", async () => {
    const { rerender } = render(<Parties caseId="c1" />);
    await expectText("Ayşe Yılmaz");
    const pending = deferred<CaseParties>();
    mocks.casePartyList.mockReturnValue(pending.promise);
    rerender(<Parties caseId="c2" />);
    await waitFor(() => expect(screen.queryAllByText("Ayşe Yılmaz").length).toBe(0));
    pending.resolve(parties([participant({ id: "q1", caseId: "c2", actorId: "b1", displayName: "Yeni Taraf", role: "tanık" })]));
    await expectText("Yeni Taraf");
  });

  it("geç gelen eski dosya yanıtı yeni dosyanın listesini ezmez", async () => {
    const first = deferred<CaseParties>();
    const second = deferred<CaseParties>();
    mocks.casePartyList.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    const { rerender } = render(<Parties caseId="c1" />);
    rerender(<Parties caseId="c2" />);
    second.resolve(parties([participant({ id: "q1", caseId: "c2", actorId: "b1", displayName: "İkinci Dosya", role: "davalı" })]));
    await expectText("İkinci Dosya");
    first.resolve(parties([participant({ displayName: "Birinci Dosya" })]));
    await waitFor(() => expect(screen.queryAllByText("Birinci Dosya").length).toBe(0));
    expect(screen.getAllByText("İkinci Dosya").length).toBeGreaterThan(0);
  });

  it("bilinmeyen taraf türü ham gösterilir (etiket uydurulmaz)", async () => {
    mocks.casePartyList.mockResolvedValue(parties([participant({ kind: "robot" })]));
    render(<Parties caseId="c1" />);
    await expectText("robot");
  });
});
