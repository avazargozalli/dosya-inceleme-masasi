// @vitest-environment jsdom
//
// Görevler ekranı davranış testi. `lib/reminders` testleri yalnız metin/saat
// yardımcılarını doğrular; burada 299 satırlık ekran gerçekten render edilip
// F08-T10 (kullanıcı tarihi, açık onay, yerel hatırlatıcı) kararlarının arayüzde
// **uygulandığı** ölçülür. `api` kısmi mock; `t()` ve `lib/reminders` gerçektir.
// Sabitlenen davranışlar:
//
// 1. Dosya listesi `caseList(false)` ile gelir, ilk dosya seçilir; görevler ve
//    hatırlatıcılar o dosya ve **bugünün yerel günü** için istenir. Satır künyesi
//    (tarih/onay rozeti/durum) ve düğme kapıları sunucu verisinden yazılır.
// 2. Dosya yokken dürüst boş durum yazılır, görev/hatırlatıcı isteği **gitmez**,
//    kuyruk düğmesi kapalıdır.
// 3. Boş başlık çekirdeğe **gitmez** (`E_TITLE`); geçerli başlık kırpılarak tam
//    sözleşmeyle gider (`dateTimezone: "Europe/Istanbul"`, `assigneeLabel: null`),
//    form temizlenir ve liste tazelenir. Enter da ekler.
// 4. Hatırlatma seçimi tarih ister: tarih yokken kapalıdır, tarih seçilince açılır
//    ve seçilen politika sözleşmeye taşınır.
// 5. **Ölçülen kusur (F):** tarih seçilip politika atandıktan sonra tarih
//    silinince seçim kapalı kalıyor ama **değeri korunuyordu** → görev tarihsiz
//    kaydediliyor, hatırlatıcı hiç kurulamayacağı hâlde `reminderPolicy` dolu
//    gidiyordu (sessiz etkisiz ayar). Tarih silinince politika "none"a döner.
// 6. "Tarihi onayla" yalnız açık + tarihli + onaysız görevde çıkar; çekirdeğe
//    gider ve liste tazelenir (tarih yazmak onay değildir).
// 7. "Bitti" yalnız açık görevde, "Sil" her görevde çıkar; ikisi de çekirdeğe
//    gider ve listeyi tazeler.
// 8. Hatırlatıcı kuyruğu sonucu (0 / N) ayrı cümlelerle yazılır ve liste tazelenir.
// 9. Hatırlatıcı satırı sözlükten etiket + tarih + görev başlığını yazar; listelenen
//    görevlerde bulunmayan hatırlatıcı "görev" der, bilinmeyen politika "kapalı" sayılır.
// 10. **Ölçülen kusur (D):** işlemler başında hata temizlenmiyordu → `E_TITLE`
//     kırmızı bildirimi **başarılı eklemeden sonra** da ekranda kalıyordu.
// 11. **Ölçülen kusur (E):** düşen kuyruk isteği önceki "N hatırlatıcı yazıldı"
//     satırını bırakıyordu (kırmızı hatayla birlikte yeşil başarı).
// 12. **Ölçülen kusur (A, INV-05):** dosya değişince önceki dosyanın görevleri,
//     hatırlatıcıları ve bildirimi yeni dosyanın isteği askıdayken ekranda kalıyordu.
// 13. **Ölçülen kusur (B):** düşen yükleme önceki dosyanın listesini bırakıyordu.
// 14. **Ölçülen kusur (C):** geç gelen **eski** dosya yanıtı yeni dosyanın listesini
//     eziyordu (istek sırası korunmuyordu).
// 15. **Ölçülen kusur (G):** dosya seçili değilken "Ekle" başlık gerekçesi
//     yazıyordu ("Görev başlığı zorunlu") — eksik olan başlık değil dosyadır.
// 16. "Tamamlananlar" kutusu sunucudan `includeDone:true` ile istenir.

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { t } from "../lib/i18n";
import { localIsoDay, REMINDER_TEXT, reminderLabel } from "../lib/reminders";
import type { CaseRow, ReminderRow, TaskRow } from "../lib/api";

const mocks = vi.hoisted(() => ({
  caseList: vi.fn(),
  taskList: vi.fn(),
  taskCreate: vi.fn(),
  taskConfirmDate: vi.fn(),
  taskComplete: vi.fn(),
  taskDelete: vi.fn(),
  taskReminders: vi.fn(),
  taskRemindersEnqueue: vi.fn(),
  notificationsDeliver: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

const { Tasks } = await import("./Tasks");

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

function taskRow(overrides: Partial<TaskRow> & { id: string; title: string }): TaskRow {
  return {
    caseId: "p1",
    status: "open",
    userDueDate: null,
    dateTimezone: null,
    dateOrigin: "user_entered",
    assigneeLabel: null,
    reminderPolicy: "none",
    dateConfirmedAt: null,
    createdAt: ISO,
    updatedAt: ISO,
    rowVersion: 1,
    ...overrides,
  };
}

const T1 = taskRow({
  id: "t1",
  title: "Tapu kaydı getir",
  userDueDate: "2026-11-10",
  dateTimezone: "Europe/Istanbul",
  reminderPolicy: "day_before",
});
const T2 = taskRow({ id: "t2", title: "Bilirkişi raporunu incele" });
const T3 = taskRow({
  id: "t3",
  title: "Dilekçe taslağını gönder",
  status: "done",
  userDueDate: "2026-10-01",
  dateConfirmedAt: "2026-10-01T09:00:00.000Z",
});

const R1: ReminderRow = {
  taskId: "t1",
  dueDate: "2026-11-05",
  policy: "day_before",
  localDay: "2026-11-02",
};

/* ------------------------------------------------------------------- helpers */

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) throw new Error(`#${id} yok`);
  return element as T;
}

/** Satır: başlık `<span>`inin ebeveyni (grid satırı). */
function rowOf(title: string): HTMLElement {
  const span = screen.getByText(title);
  const row = span.parentElement;
  if (!row) throw new Error(`"${title}" satırı bulunamadı`);
  return row as HTMLElement;
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

function setup(options: {
  cases?: CaseRow[];
  tasks?: TaskRow[];
  reminders?: ReminderRow[];
} = {}) {
  mocks.caseList.mockResolvedValue(options.cases ?? [P1, P2]);
  mocks.taskList.mockResolvedValue(options.tasks ?? [T1, T2, T3]);
  mocks.taskReminders.mockResolvedValue(options.reminders ?? [R1]);
  mocks.taskCreate.mockResolvedValue(T1);
  mocks.taskConfirmDate.mockResolvedValue(T1);
  mocks.taskComplete.mockResolvedValue(T1);
  mocks.taskDelete.mockResolvedValue(undefined);
  mocks.taskRemindersEnqueue.mockResolvedValue({ queued: 0, reminders: [] });
  mocks.notificationsDeliver.mockResolvedValue({ sent: 1, failed: 0, remaining: 0 });
  return render(<Tasks />);
}

/** İlk yükleme bitene kadar bekler (dosya + görev listesi yazıldı). */
async function loaded() {
  await screen.findByText(t("tasks.listTitle"));
  await waitFor(() => expect(mocks.taskList).toHaveBeenCalled());
}

describe("görevler ekranı davranışı", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("görevleri ve hatırlatıcıları sunucudan yükler; künye, rozet ve düğme kapılarını yazar", async () => {
    setup();
    await loaded();

    expect(mocks.caseList).toHaveBeenCalledWith(false);
    expect(byId<HTMLSelectElement>("task-case").value).toBe("p1");
    expect(mocks.taskList).toHaveBeenCalledWith("p1", false);
    expect(mocks.taskReminders).toHaveBeenCalledWith("p1", localIsoDay());
    expect(screen.getByText(t("tasks.recordCount", { count: 3 }))).toBeTruthy();

    // Tarihli + onaysız: tarih yazılır, "onay bekliyor" rozeti ve onay düğmesi çıkar.
    const t1 = rowOf("Tapu kaydı getir");
    expect(within(t1).getByText("2026-11-10")).toBeTruthy();
    expect(within(t1).getByText(t("tasks.pendingConfirm"))).toBeTruthy();
    expect(within(t1).getByText(t("tasks.confirmDate"))).toBeTruthy();
    expect(within(t1).getByText(t("tasks.complete"))).toBeTruthy();
    expect(within(t1).getByText(t("tasks.open"))).toBeTruthy();

    // Tarihsiz: "tarih yok" yazılır, onay rozeti/düğmesi **uydurulmaz**.
    const t2 = rowOf("Bilirkişi raporunu incele");
    expect(within(t2).getByText(t("tasks.noDate"))).toBeTruthy();
    expect(within(t2).queryByText(t("tasks.pendingConfirm"))).toBeNull();
    expect(within(t2).queryByText(t("tasks.confirmed"))).toBeNull();
    expect(within(t2).queryByText(t("tasks.confirmDate"))).toBeNull();

    // Tamamlanmış: "tamamlandı" rozeti, "Bitti" yok, "Sil" var; onaylı rozeti yazılır.
    const t3 = rowOf("Dilekçe taslağını gönder");
    expect(within(t3).getByText(t("tasks.done"))).toBeTruthy();
    expect(within(t3).getByText(t("tasks.confirmed"))).toBeTruthy();
    expect(within(t3).queryByText(t("tasks.complete"))).toBeNull();
    expect(within(t3).getByText(t("tasks.remove"))).toBeTruthy();

    // Hatırlatıcı kartı: sayı + gün yerel günden, satır etiketi sözlükten.
    expect(screen.getByText(t("tasks.remindersSub", { count: 1, day: localIsoDay() }))).toBeTruthy();
    expect(screen.getByText(REMINDER_TEXT)).toBeTruthy();
    expect(
      screen.getByText(`${reminderLabel("day_before")} · 2026-11-05 · Tapu kaydı getir`),
    ).toBeTruthy();
  });

  it("dosya yokken dürüst boş durum yazar, istek üretmez, kuyruk düğmesini kapatır", async () => {
    setup({ cases: [], tasks: [], reminders: [] });
    await screen.findByText(t("tasks.listTitle"));

    expect(screen.getByText(t("common.noCase"))).toBeTruthy();
    expect(byId<HTMLSelectElement>("task-case").value).toBe("");
    expect(mocks.taskList).not.toHaveBeenCalled();
    expect(mocks.taskReminders).not.toHaveBeenCalled();
    expect(screen.getByText(t("tasks.empty"))).toBeTruthy();
    expect(screen.getByText(t("tasks.noReminders"))).toBeTruthy();
    expect((screen.getByText(t("tasks.enqueue")) as HTMLButtonElement).disabled).toBe(true);
  });

  it("boş başlığı göndermez; geçerli görevi kırpıp tam sözleşmeyle yazar ve formu temizler", async () => {
    setup();
    await loaded();

    fireEvent.click(screen.getByText(t("tasks.add")));
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("E_TITLE");
    expect(screen.getByRole("alert").textContent).toContain(t("tasks.titleRequired"));
    expect(mocks.taskCreate).not.toHaveBeenCalled();

    // Başlık yalnız boşluktan oluşuyorsa da gönderilmez.
    const title = byId<HTMLInputElement>("task-title");
    fireEvent.change(title, { target: { value: "   " } });
    fireEvent.click(screen.getByText(t("tasks.add")));
    expect(mocks.taskCreate).not.toHaveBeenCalled();

    // Enter de ekler; başlık kırpılır, tarih yoksa `null`, politika "none".
    fireEvent.change(title, { target: { value: "  Tebliğ tarihini kontrol et  " } });
    fireEvent.keyDown(title, { key: "Enter" });

    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(1));
    expect(mocks.taskCreate).toHaveBeenCalledWith("p1", {
      title: "Tebliğ tarihini kontrol et",
      userDueDate: null,
      dateTimezone: "Europe/Istanbul",
      assigneeLabel: null,
      reminderPolicy: "none",
    });
    await waitFor(() => expect(byId<HTMLInputElement>("task-title").value).toBe(""));
    expect(byId<HTMLInputElement>("task-due").value).toBe("");
    expect(byId<HTMLSelectElement>("task-policy").value).toBe("none");
    // Liste tazelenir (ilk yükleme + ekleme sonrası).
    await waitFor(() => expect(mocks.taskList).toHaveBeenCalledTimes(2));
  });

  it("hatırlatma seçimi tarih ister; seçilen politika ve tarih sözleşmeye taşınır", async () => {
    setup();
    await loaded();

    const policy = byId<HTMLSelectElement>("task-policy");
    expect(policy.disabled).toBe(true);

    fireEvent.change(byId<HTMLInputElement>("task-due"), { target: { value: "2026-11-10" } });
    expect(byId<HTMLSelectElement>("task-policy").disabled).toBe(false);

    fireEvent.change(byId<HTMLSelectElement>("task-policy"), { target: { value: "day_before" } });
    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "Tebligatı kontrol et" } });
    fireEvent.click(screen.getByText(t("tasks.add")));

    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(1));
    expect(mocks.taskCreate).toHaveBeenCalledWith("p1", {
      title: "Tebligatı kontrol et",
      userDueDate: "2026-11-10",
      dateTimezone: "Europe/Istanbul",
      assigneeLabel: null,
      reminderPolicy: "day_before",
    });
  });

  it("tarih silinince hatırlatma politikası sessizce taşınmaz", async () => {
    setup();
    await loaded();

    fireEvent.change(byId<HTMLInputElement>("task-due"), { target: { value: "2026-11-10" } });
    fireEvent.change(byId<HTMLSelectElement>("task-policy"), { target: { value: "day_before" } });

    // Tarih temizlenir: seçim kapalı kalır **ve** "none"a döner.
    fireEvent.change(byId<HTMLInputElement>("task-due"), { target: { value: "" } });
    expect(byId<HTMLSelectElement>("task-policy").disabled).toBe(true);
    expect(byId<HTMLSelectElement>("task-policy").value).toBe("none");

    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "Tarihsiz görev" } });
    fireEvent.click(screen.getByText(t("tasks.add")));

    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(1));
    expect(mocks.taskCreate).toHaveBeenCalledWith("p1", {
      title: "Tarihsiz görev",
      userDueDate: null,
      dateTimezone: "Europe/Istanbul",
      assigneeLabel: null,
      reminderPolicy: "none",
    });
  });

  it("'Tarihi onayla' çekirdeğe gider ve listeyi tazeler", async () => {
    setup();
    await loaded();

    fireEvent.click(within(rowOf("Tapu kaydı getir")).getByText(t("tasks.confirmDate")));

    await waitFor(() => expect(mocks.taskConfirmDate).toHaveBeenCalledWith("t1"));
    await waitFor(() => expect(mocks.taskList).toHaveBeenCalledTimes(2));
  });

  it("'Bitti' ve 'Sil' çekirdeğe gider, listeyi tazeler", async () => {
    setup();
    await loaded();

    fireEvent.click(within(rowOf("Tapu kaydı getir")).getByText(t("tasks.complete")));
    await waitFor(() => expect(mocks.taskComplete).toHaveBeenCalledWith("t1"));

    fireEvent.click(within(rowOf("Bilirkişi raporunu incele")).getByText(t("tasks.remove")));
    await waitFor(() => expect(mocks.taskDelete).toHaveBeenCalledWith("t2"));

    await waitFor(() => expect(mocks.taskList).toHaveBeenCalledTimes(3));
  });

  it("hatırlatıcı kuyruğu sonucunu (0 / N) ayrı cümlelerle yazar", async () => {
    setup();
    await loaded();

    mocks.taskRemindersEnqueue.mockResolvedValue({ queued: 0, reminders: [] });
    fireEvent.click(screen.getByText(t("tasks.enqueue")));
    expect(await screen.findByText(t("tasks.noNewReminders"))).toBeTruthy();
    expect(mocks.taskRemindersEnqueue).toHaveBeenCalledWith("p1", localIsoDay());

    mocks.taskRemindersEnqueue.mockResolvedValue({ queued: 2, reminders: [R1] });
    fireEvent.click(screen.getByText(t("tasks.enqueue")));
    expect(await screen.findByText(t("tasks.remindersQueued", { queued: 2 }))).toBeTruthy();
    // Önceki (0) bildirimi yeni sonucun yerine geçer.
    expect(screen.queryByText(t("tasks.noNewReminders"))).toBeNull();
    await waitFor(() => expect(mocks.taskList).toHaveBeenCalledTimes(3));
  });

  it("bildirim gönderimi gerçek komutu çağırır ve sonucu (gönderilen/kalan) yazar", async () => {
    setup();
    await loaded();

    mocks.notificationsDeliver.mockResolvedValue({ sent: 2, failed: 0, remaining: 0 });
    fireEvent.click(screen.getByText(t("tasks.notify")));
    expect(
      await screen.findByText(t("tasks.notifySent", { sent: 2, remaining: 0 })),
    ).toBeTruthy();
    expect(mocks.notificationsDeliver).toHaveBeenCalledWith("p1");

    // OS bir satırı reddederse "gönderildi" denmez: başarısız ve kalan ayrı yazılır.
    mocks.notificationsDeliver.mockResolvedValue({ sent: 1, failed: 2, remaining: 2 });
    fireEvent.click(screen.getByText(t("tasks.notify")));
    expect(
      await screen.findByText(t("tasks.notifyPartial", { sent: 1, failed: 2, remaining: 2 })),
    ).toBeTruthy();
    expect(screen.queryByText(t("tasks.notifySent", { sent: 1, remaining: 2 }))).toBeNull();
  });

  it("bildirim izni yoksa hata kodu görünür ve \"gönderildi\" yazılmaz", async () => {
    setup();
    await loaded();

    mocks.notificationsDeliver.mockRejectedValue({
      code: "E_NOTIFY_PERMISSION",
      message: "Bildirim izni verilmedi. Sistem ayarlarından izin verin.",
    });
    fireEvent.click(screen.getByText(t("tasks.notify")));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("E_NOTIFY_PERMISSION");
    expect(alert.textContent).toContain("Bildirim izni verilmedi");
    // Hiçbir "gönderildi" cümlesi yazılmaz.
    expect(screen.queryByText(t("tasks.notifySent", { sent: 0, remaining: 0 }))).toBeNull();
    expect(screen.queryByText(t("tasks.notifyNone", { remaining: 0 }))).toBeNull();
  });

  it("hatırlatıcı satırında eşleşmeyen görev ve bilinmeyen politika uydurulmaz", async () => {
    setup({
      tasks: [T1],
      reminders: [
        { taskId: "t9", dueDate: "2026-11-06", policy: "hergun", localDay: "2026-11-02" },
      ],
    });
    await loaded();

    expect(
      screen.getByText(
        `${reminderLabel("hergun")} · 2026-11-06 · ${t("tasks.taskFallback")}`,
      ),
    ).toBeTruthy();
    expect(reminderLabel("hergun")).toBe(t("reminders.label.off"));
  });

  it("başarılı işlem önceki kırmızı hatayı ekranda bırakmaz", async () => {
    setup();
    await loaded();

    fireEvent.click(screen.getByText(t("tasks.add")));
    expect(await screen.findByRole("alert")).toBeTruthy();

    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "Yeni görev" } });
    fireEvent.click(screen.getByText(t("tasks.add")));

    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(mocks.taskCreate).toHaveBeenCalledTimes(1);
  });

  it("düşen kuyruk isteği önceki yeşil bildirimi bırakmaz", async () => {
    setup();
    await loaded();

    mocks.taskRemindersEnqueue.mockResolvedValue({ queued: 1, reminders: [R1] });
    fireEvent.click(screen.getByText(t("tasks.enqueue")));
    expect(await screen.findByText(t("tasks.remindersQueued", { queued: 1 }))).toBeTruthy();

    mocks.taskRemindersEnqueue.mockRejectedValue(new Error("kuyruk kapalı"));
    fireEvent.click(screen.getByText(t("tasks.enqueue")));

    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("E_UI_UNKNOWN");
    expect(screen.getByRole("alert").textContent).toContain(t("api.unexpectedError"));
    expect(screen.queryByText(t("tasks.remindersQueued", { queued: 1 }))).toBeNull();
  });

  it("dosya değişince önceki dosyanın görevleri, hatırlatıcıları ve bildirimi kalkar", async () => {
    setup();
    await loaded();

    // Önceki dosyadan yeşil bir kuyruk bildirimi bırak.
    mocks.taskRemindersEnqueue.mockResolvedValue({ queued: 3, reminders: [R1] });
    fireEvent.click(screen.getByText(t("tasks.enqueue")));
    expect(await screen.findByText(t("tasks.remindersQueued", { queued: 3 }))).toBeTruthy();

    // Yeni dosyanın isteği askıda kalsın.
    const pending = deferred<TaskRow[]>();
    mocks.taskList.mockReturnValue(pending.promise);
    mocks.taskReminders.mockResolvedValue([]);

    fireEvent.change(byId<HTMLSelectElement>("task-case"), { target: { value: "p2" } });

    // Askıdaki istek sırasında önceki dosyanın verisi görünmemeli.
    await waitFor(() => expect(screen.queryByText("Tapu kaydı getir")).toBeNull());
    expect(screen.queryByText(t("tasks.remindersQueued", { queued: 3 }))).toBeNull();
    expect(screen.queryByText(REMINDER_TEXT)).toBeNull();

    pending.resolve([taskRow({ id: "t5", caseId: "p2", title: "Savunma dilekçesi" })]);
    expect(await screen.findByText("Savunma dilekçesi")).toBeTruthy();
    expect(mocks.taskList).toHaveBeenLastCalledWith("p2", false);
  });

  it("düşen yükleme önceki dosyanın listesini bırakmaz", async () => {
    setup();
    await loaded();

    mocks.taskList.mockRejectedValue(new Error("veritabanı kapalı"));
    fireEvent.change(byId<HTMLSelectElement>("task-case"), { target: { value: "p2" } });

    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("E_UI_UNKNOWN");
    await waitFor(() => expect(screen.queryByText("Tapu kaydı getir")).toBeNull());
    expect(screen.getByText(t("tasks.empty"))).toBeTruthy();
  });

  it("başarılı işlemden sonra düşen tazeleme eski listeyi güncel gibi bırakmaz", async () => {
    setup();
    await loaded();

    // İşlem çekirdekte başarılı; liste tazelemesi düşer → eski liste gösterilmez.
    mocks.taskList.mockRejectedValueOnce(new Error("veritabanı kapalı"));
    fireEvent.click(within(rowOf("Tapu kaydı getir")).getByText(t("tasks.complete")));

    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(mocks.taskComplete).toHaveBeenCalledWith("t1");
    await waitFor(() => expect(screen.queryByText("Tapu kaydı getir")).toBeNull());
    expect(screen.getByText(t("tasks.empty"))).toBeTruthy();
  });

  it("geç gelen eski dosya yanıtı yeni dosyanın listesini ezmez", async () => {
    const first = deferred<TaskRow[]>();
    mocks.caseList.mockResolvedValue([P1, P2]);
    mocks.taskList.mockReturnValueOnce(first.promise);
    mocks.taskList.mockResolvedValue([
      taskRow({ id: "t7", caseId: "p2", title: "Ceza dosyası görevi" }),
    ]);
    mocks.taskReminders.mockResolvedValue([]);
    render(<Tasks />);

    await waitFor(() => expect(mocks.taskList).toHaveBeenCalledWith("p1", false));
    fireEvent.change(byId<HTMLSelectElement>("task-case"), { target: { value: "p2" } });
    expect(await screen.findByText("Ceza dosyası görevi")).toBeTruthy();

    // Eski dosyanın yanıtı **sonra** gelir: yeni listeyi ezmemeli.
    first.resolve([taskRow({ id: "t6", caseId: "p1", title: "Kira dosyası görevi" })]);
    await waitFor(() => expect(screen.queryByText("Kira dosyası görevi")).toBeNull());
    expect(screen.getByText("Ceza dosyası görevi")).toBeTruthy();
  });

  it("dosya seçili değilken 'Ekle' başlık değil dosya gerekçesi yazar", async () => {
    setup({ cases: [], tasks: [], reminders: [] });
    await screen.findByText(t("tasks.listTitle"));

    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "Görev başlığı" } });
    fireEvent.click(screen.getByText(t("tasks.add")));

    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("E_NO_CASE");
    expect(screen.getByRole("alert").textContent).toContain(t("tasks.caseRequired"));
    expect(screen.getByRole("alert").textContent).not.toContain(t("tasks.titleRequired"));
    expect(mocks.taskCreate).not.toHaveBeenCalled();
  });

  it("'Tamamlananlar' kutusu sunucudan includeDone=true ile istenir", async () => {
    setup();
    await loaded();

    const box = screen.getByRole("checkbox") as HTMLInputElement;
    expect(box.checked).toBe(false);
    fireEvent.click(box);

    await waitFor(() => expect(mocks.taskList).toHaveBeenLastCalledWith("p1", true));
    expect((screen.getByRole("checkbox") as HTMLInputElement).checked).toBe(true);
  });
});

/* ------------------------------------------------------------- uçuş yolları */

const P2_TASK = taskRow({ id: "t9", caseId: "p2", title: "Ceza dosyası görevi" });

/** İki dosyalı kurulum: her dosya kendi görev listesini döndürür. */
function setupTwoCases() {
  mocks.caseList.mockResolvedValue([P1, P2]);
  mocks.taskList.mockImplementation(async (id: string) =>
    id === "p1" ? [T1, T2] : [P2_TASK],
  );
  mocks.taskReminders.mockResolvedValue([]);
  mocks.taskCreate.mockResolvedValue(T1);
  mocks.taskConfirmDate.mockResolvedValue(T1);
  mocks.taskComplete.mockResolvedValue(T1);
  mocks.taskDelete.mockResolvedValue(undefined);
  mocks.taskRemindersEnqueue.mockResolvedValue({ queued: 0, reminders: [] });
  render(<Tasks />);
}

function addButton(): HTMLButtonElement {
  return screen.getByRole("button", { name: t("tasks.add") }) as HTMLButtonElement;
}

async function switchToP2() {
  fireEvent.change(byId<HTMLSelectElement>("task-case"), { target: { value: "p2" } });
  expect(await screen.findByText(P2_TASK.title)).toBeTruthy();
}

describe("görevler ekranı uçuş yolları", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("oluşturma yanıtı uçuşta yazılan yeni başlığı ve tarihi silmez", async () => {
    const creating = deferred<TaskRow>();
    setup();
    await loaded();
    mocks.taskCreate.mockReturnValue(creating.promise);

    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "İlk görev" } });
    fireEvent.click(screen.getByText(t("tasks.add")));
    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(1));

    // Yanıt beklerken **yeni** görev yazılır: yanıt bu metni silmemeli.
    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "uçuşta yazıldı" } });
    fireEvent.change(byId<HTMLInputElement>("task-due"), { target: { value: "2026-12-01" } });

    await act(async () => {
      creating.resolve(T1);
    });

    expect(byId<HTMLInputElement>("task-title").value).toBe("uçuşta yazıldı");
    expect(byId<HTMLInputElement>("task-due").value).toBe("2026-12-01");
  });

  it("oluşturma yanıtı dosya değiştiyse yeni dosyanın listesini ve formunu ezmez", async () => {
    const creating = deferred<TaskRow>();
    setupTwoCases();
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskCreate.mockReturnValue(creating.promise);

    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "p1 görevi" } });
    fireEvent.click(addButton());
    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(1));

    await switchToP2();
    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "p2 için yazıldı" } });

    await act(async () => {
      creating.resolve(T1);
    });

    // Eski dosyanın listesi yeni dosyaya yazılmaz; uçuşta yazılan form metni korunur.
    expect(screen.getByText(P2_TASK.title)).toBeTruthy();
    expect(screen.queryByText(T1.title)).toBeNull();
    expect(mocks.taskList).toHaveBeenLastCalledWith("p2", false);
    expect(byId<HTMLInputElement>("task-title").value).toBe("p2 için yazıldı");
  });

  it("oluşturma hatası uçuşta dosya değişirse yeni dosyada görünmez", async () => {
    const creating = deferred<TaskRow>();
    setupTwoCases();
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskCreate.mockReturnValue(creating.promise);

    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "p1 görevi" } });
    fireEvent.click(addButton());
    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(1));

    await switchToP2();

    await act(async () => {
      creating.reject({ code: "E_DB", message: "Görev yazılamadı.", retryable: true });
    });

    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText(P2_TASK.title)).toBeTruthy();
  });

  it("'Bitti' çift tıkta tek yazma gönderir", async () => {
    const finishing = deferred<TaskRow>();
    setup();
    await loaded();
    mocks.taskComplete.mockReturnValue(finishing.promise);

    const button = within(rowOf(T1.title)).getByRole("button", { name: t("tasks.complete") });
    fireEvent.click(button);
    fireEvent.click(button);

    expect(mocks.taskComplete).toHaveBeenCalledTimes(1);
    await act(async () => {
      finishing.resolve({ ...T1, status: "done" });
    });
  });

  it("'Sil' çift tıkta tek yazma gönderir", async () => {
    const removing = deferred<void>();
    setup();
    await loaded();
    mocks.taskDelete.mockReturnValue(removing.promise);

    const button = within(rowOf(T2.title)).getByRole("button", { name: t("tasks.remove") });
    fireEvent.click(button);
    fireEvent.click(button);

    expect(mocks.taskDelete).toHaveBeenCalledTimes(1);
    await act(async () => {
      removing.resolve();
    });
  });

  it("uçuşta Enter ikinci görev göndermez", async () => {
    const creating = deferred<TaskRow>();
    setup();
    await loaded();
    mocks.taskCreate.mockReturnValue(creating.promise);

    const title = byId<HTMLInputElement>("task-title");
    fireEvent.change(title, { target: { value: "ilk görev" } });
    fireEvent.click(addButton());
    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(1));

    fireEvent.change(title, { target: { value: "ikinci görev" } });
    fireEvent.keyDown(title, { key: "Enter" });

    expect(mocks.taskCreate).toHaveBeenCalledTimes(1);
    await act(async () => {
      creating.resolve(T1);
    });
  });

  it("oluşturma uçuşunda satır düğmeleri ikinci yazma göndermez", async () => {
    const creating = deferred<TaskRow>();
    setup();
    await loaded();
    mocks.taskCreate.mockReturnValue(creating.promise);

    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "yeni görev" } });
    fireEvent.click(addButton());
    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(1));

    fireEvent.click(within(rowOf(T1.title)).getByRole("button", { name: t("tasks.complete") }));
    fireEvent.click(within(rowOf(T2.title)).getByRole("button", { name: t("tasks.remove") }));

    expect(mocks.taskComplete).not.toHaveBeenCalled();
    expect(mocks.taskDelete).not.toHaveBeenCalled();
    await act(async () => {
      creating.resolve(T1);
    });
  });

  it("yazma sonrası tazeleme o anki süzgeci kullanır", async () => {
    const finishing = deferred<TaskRow>();
    mocks.caseList.mockResolvedValue([P1]);
    mocks.taskList.mockImplementation(async (_id: string, done: boolean) =>
      done ? [T1, T2, T3] : [T1, T2],
    );
    mocks.taskReminders.mockResolvedValue([]);
    render(<Tasks />);
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());

    mocks.taskComplete.mockReturnValue(finishing.promise);
    fireEvent.click(within(rowOf(T1.title)).getByRole("button", { name: t("tasks.complete") }));
    await waitFor(() => expect(mocks.taskComplete).toHaveBeenCalledTimes(1));

    // Süzgeç, yazma uçuşta iken değişir.
    fireEvent.click(screen.getByRole("checkbox"));
    await waitFor(() => expect(mocks.taskList).toHaveBeenLastCalledWith("p1", true));
    expect(await screen.findByText(T3.title)).toBeTruthy();

    await act(async () => {
      finishing.resolve({ ...T1, status: "done" });
    });

    // Tazeleme eski süzgeçle koşup "tamamlananlar" listesini ezmez.
    expect(screen.getByText(T3.title)).toBeTruthy();
    expect((screen.getByRole("checkbox") as HTMLInputElement).checked).toBe(true);
    expect(mocks.taskList).toHaveBeenLastCalledWith("p1", true);
  });

  it("dosya değişimi uçuştaki yazmanın kilidini devreder", async () => {
    const creating = deferred<TaskRow>();
    setupTwoCases();
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskCreate.mockReturnValue(creating.promise);

    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "p1 görevi" } });
    fireEvent.click(addButton());
    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(1));
    expect(addButton().disabled).toBe(true);

    await switchToP2();

    // Yeni dosyanın düğmesi eski isteğin dönmesini beklemez.
    expect(addButton().disabled).toBe(false);

    await act(async () => {
      creating.resolve(T1);
    });
  });

  it("eski yazmanın kapanışı yeni dosyadaki kilidi erken açmaz", async () => {
    const first = deferred<TaskRow>();
    const second = deferred<TaskRow>();
    setupTwoCases();
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskCreate.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "p1 görevi" } });
    fireEvent.click(addButton());
    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(1));

    await switchToP2();
    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "p2 görevi" } });
    fireEvent.click(addButton());
    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(2));
    expect(addButton().disabled).toBe(true);

    // Eski dosyanın yanıtı döner: yeni dosyanın kilidi **açılmamalı**.
    await act(async () => {
      first.resolve(T1);
    });
    expect(addButton().disabled).toBe(true);

    await act(async () => {
      second.resolve(P2_TASK);
    });
    await waitFor(() => expect(addButton().disabled).toBe(false));
  });

  it("kuyruk yanıtı dosya değiştiyse yeni dosyaya yazılmaz", async () => {
    const queueing = deferred<{ queued: number; reminders: ReminderRow[] }>();
    setupTwoCases();
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskRemindersEnqueue.mockReturnValue(queueing.promise);

    fireEvent.click(screen.getByText(t("tasks.enqueue")));
    await waitFor(() => expect(mocks.taskRemindersEnqueue).toHaveBeenCalledTimes(1));

    await switchToP2();

    await act(async () => {
      queueing.resolve({ queued: 2, reminders: [R1] });
    });

    expect(screen.queryByText(t("tasks.remindersQueued", { queued: 2 }))).toBeNull();
    expect(mocks.taskList).toHaveBeenLastCalledWith("p2", false);
  });

  it("kuyruk hatası dosya değiştiyse yeni dosyada görünmez", async () => {
    const queueing = deferred<{ queued: number; reminders: ReminderRow[] }>();
    setupTwoCases();
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskRemindersEnqueue.mockReturnValue(queueing.promise);

    fireEvent.click(screen.getByText(t("tasks.enqueue")));
    await waitFor(() => expect(mocks.taskRemindersEnqueue).toHaveBeenCalledTimes(1));

    await switchToP2();

    await act(async () => {
      queueing.reject({ code: "E_DB", message: "Kuyruk yazılamadı.", retryable: true });
    });

    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText(P2_TASK.title)).toBeTruthy();
  });

  it("onay yanıtı dosya değiştiyse yeni dosyanın listesini ezmez", async () => {
    const confirming = deferred<TaskRow>();
    setupTwoCases();
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskConfirmDate.mockReturnValue(confirming.promise);

    fireEvent.click(within(rowOf(T1.title)).getByRole("button", { name: t("tasks.confirmDate") }));
    await waitFor(() => expect(mocks.taskConfirmDate).toHaveBeenCalledTimes(1));

    await switchToP2();

    await act(async () => {
      confirming.resolve({ ...T1, dateConfirmedAt: ISO });
    });

    expect(screen.getByText(P2_TASK.title)).toBeTruthy();
    expect(screen.queryByText(T1.title)).toBeNull();
    expect(mocks.taskList).toHaveBeenLastCalledWith("p2", false);
  });

  it("onay hatası uçuşta dosya değişirse yeni dosyada görünmez", async () => {
    const confirming = deferred<TaskRow>();
    setupTwoCases();
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskConfirmDate.mockReturnValue(confirming.promise);

    fireEvent.click(within(rowOf(T1.title)).getByRole("button", { name: t("tasks.confirmDate") }));
    await waitFor(() => expect(mocks.taskConfirmDate).toHaveBeenCalledTimes(1));

    await switchToP2();

    await act(async () => {
      confirming.reject({ code: "E_DB", message: "Tarih onaylanamadı.", retryable: true });
    });

    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText(P2_TASK.title)).toBeTruthy();
  });

  it("silme yanıtı dosya değiştiyse yeni dosyanın listesini ezmez", async () => {
    const removing = deferred<void>();
    setupTwoCases();
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskDelete.mockReturnValue(removing.promise);

    fireEvent.click(within(rowOf(T2.title)).getByRole("button", { name: t("tasks.remove") }));
    await waitFor(() => expect(mocks.taskDelete).toHaveBeenCalledTimes(1));

    await switchToP2();

    await act(async () => {
      removing.resolve();
    });

    expect(screen.getByText(P2_TASK.title)).toBeTruthy();
    expect(screen.queryByText(T1.title)).toBeNull();
    expect(mocks.taskList).toHaveBeenLastCalledWith("p2", false);
  });

  it("silme hatası uçuşta dosya değişirse yeni dosyada görünmez", async () => {
    const removing = deferred<void>();
    setupTwoCases();
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskDelete.mockReturnValue(removing.promise);

    fireEvent.click(within(rowOf(T2.title)).getByRole("button", { name: t("tasks.remove") }));
    await waitFor(() => expect(mocks.taskDelete).toHaveBeenCalledTimes(1));

    await switchToP2();

    await act(async () => {
      removing.reject({ code: "E_DB", message: "Görev silinemedi.", retryable: true });
    });

    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText(P2_TASK.title)).toBeTruthy();
  });

  it("satır yazmasının yanıtı ve kapanışı yeni dosyaya taşınmaz", async () => {
    const finishing = deferred<TaskRow>();
    const creating = deferred<TaskRow>();
    setupTwoCases();
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskComplete.mockReturnValue(finishing.promise);

    fireEvent.click(within(rowOf(T1.title)).getByRole("button", { name: t("tasks.complete") }));
    await waitFor(() => expect(mocks.taskComplete).toHaveBeenCalledTimes(1));

    await switchToP2();
    mocks.taskCreate.mockReturnValue(creating.promise);
    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "p2 görevi" } });
    fireEvent.click(addButton());
    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(1));
    expect(addButton().disabled).toBe(true);

    // Eski dosyanın satır yazması döner: ne listesi ne kilidi yeni dosyaya taşınır.
    await act(async () => {
      finishing.resolve({ ...T1, status: "done" });
    });
    expect(screen.getByText(P2_TASK.title)).toBeTruthy();
    expect(screen.queryByText(T1.title)).toBeNull();
    expect(addButton().disabled).toBe(true);

    await act(async () => {
      creating.resolve(P2_TASK);
    });
    await waitFor(() => expect(addButton().disabled).toBe(false));
  });

  it("satır yazması hatası uçuşta dosya değişirse yeni dosyada görünmez", async () => {
    const finishing = deferred<TaskRow>();
    setupTwoCases();
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskComplete.mockReturnValue(finishing.promise);

    fireEvent.click(within(rowOf(T1.title)).getByRole("button", { name: t("tasks.complete") }));
    await waitFor(() => expect(mocks.taskComplete).toHaveBeenCalledTimes(1));

    await switchToP2();

    await act(async () => {
      finishing.reject({ code: "E_DB", message: "Görev tamamlanamadı.", retryable: true });
    });

    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText(P2_TASK.title)).toBeTruthy();
  });

  it("oluşturma tazelemesi de o anki süzgeci kullanır", async () => {
    const creating = deferred<TaskRow>();
    mocks.caseList.mockResolvedValue([P1]);
    mocks.taskList.mockImplementation(async (_id: string, done: boolean) =>
      done ? [T1, T2, T3] : [T1, T2],
    );
    mocks.taskReminders.mockResolvedValue([]);
    render(<Tasks />);
    await waitFor(() => expect(screen.getByText(T1.title)).toBeTruthy());
    mocks.taskCreate.mockReturnValue(creating.promise);

    fireEvent.change(byId<HTMLInputElement>("task-title"), { target: { value: "yeni görev" } });
    fireEvent.click(addButton());
    await waitFor(() => expect(mocks.taskCreate).toHaveBeenCalledTimes(1));

    // Süzgeç, oluşturma uçuşta iken değişir.
    fireEvent.click(screen.getByRole("checkbox"));
    await waitFor(() => expect(mocks.taskList).toHaveBeenLastCalledWith("p1", true));
    expect(await screen.findByText(T3.title)).toBeTruthy();

    await act(async () => {
      creating.resolve(T1);
    });

    expect(screen.getByText(T3.title)).toBeTruthy();
    expect(mocks.taskList).toHaveBeenLastCalledWith("p1", true);
  });
});
