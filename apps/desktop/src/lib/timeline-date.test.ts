import { describe, expect, it } from "vitest";

import {
  composeDateValue,
  dateDraftIssue,
  dateInputMode,
  editDateInputs,
  eventDateText,
  isRealDay,
  isRealMonth,
  isYear,
} from "./timeline-date";

describe("kronoloji tarih yardımcıları", () => {
  it("kesinliğe göre giriş türünü seçer", () => {
    expect(dateInputMode("day")).toBe("date");
    expect(dateInputMode("month")).toBe("month");
    expect(dateInputMode("year")).toBe("year");
    expect(dateInputMode("range")).toBe("range");
    expect(dateInputMode("approximate")).toBe("month");
    expect(dateInputMode("unknown")).toBe("none");
    // Tanınmayan kesinlik tam tarih girişine düşer (sessizce kaybolmaz).
    expect(dateInputMode("week")).toBe("date");
  });

  it("takvimde olmayan günü reddeder", () => {
    expect(isRealDay("2026-02-28")).toBe(true);
    expect(isRealDay("2024-02-29")).toBe(true);
    expect(isRealDay("2026-02-29")).toBe(false);
    expect(isRealDay("2026-02-31")).toBe(false);
    expect(isRealDay("2026-13-01")).toBe(false);
    expect(isRealDay("2026-4-1")).toBe(false);
    expect(isRealDay("10.04.2026")).toBe(false);
  });

  it("ay ve yıl biçimini denetler", () => {
    expect(isRealMonth("2026-04")).toBe(true);
    expect(isRealMonth("2026-13")).toBe(false);
    expect(isRealMonth("2026-4")).toBe(false);
    expect(isRealMonth("2026-04-10")).toBe(false);
    expect(isYear("2026")).toBe(true);
    expect(isYear("26")).toBe(false);
    expect(isYear("2026-04")).toBe(false);
  });

  it("kesinliğe uymayan taslağı reddeder, boş tarihi kabul etmez", () => {
    expect(dateDraftIssue("day", "2026-04-10", "")).toBeNull();
    expect(dateDraftIssue("day", "2026-04", "")).toBe("needDay");
    expect(dateDraftIssue("day", "", "")).toBe("needDay");
    expect(dateDraftIssue("month", "2026-04", "")).toBeNull();
    expect(dateDraftIssue("month", "2026-04-10", "")).toBe("needMonth");
    expect(dateDraftIssue("year", "2026", "")).toBeNull();
    expect(dateDraftIssue("year", "2026-04", "")).toBe("needYear");
    expect(dateDraftIssue("unknown", "", "")).toBeNull();
    // Aralık iki uç ister; ters sıra reddedilir.
    expect(dateDraftIssue("range", "2026-01-01", "2026-03-01")).toBeNull();
    expect(dateDraftIssue("range", "2026-01-01", "")).toBe("needRange");
    expect(dateDraftIssue("range", "2026-04-01", "2026-03-01")).toBe("rangeOrder");
  });

  it("çekirdeğe gönderilecek değeri kesinliğe göre kurar", () => {
    expect(composeDateValue("day", "2026-04-10", "")).toBe("2026-04-10");
    expect(composeDateValue("month", "2026-04", "")).toBe("2026-04");
    expect(composeDateValue("year", "2026", "")).toBe("2026");
    expect(composeDateValue("range", "2026-01-01", "2026-03-01")).toBe(
      "2026-01-01/2026-03-01",
    );
    expect(composeDateValue("unknown", "2026-04-10", "")).toBeNull();
  });

  it("ay/yıl kesinliğinde gün yazmaz (eski kayıtlar dahil)", () => {
    expect(
      eventDateText({
        datePrecision: "month",
        dateValue: "2026-04-10",
        sortableDate: "2026-04-10",
      }),
    ).toBe("2026-04");
    expect(
      eventDateText({
        datePrecision: "year",
        dateValue: "2026-04-10",
        sortableDate: "2026-04-10",
      }),
    ).toBe("2026");
    expect(
      eventDateText({
        datePrecision: "range",
        dateValue: "2026-01-05/2026-03-09",
        sortableDate: "2026-01-05",
      }),
    ).toBe("2026-01-05 – 2026-03-09");
    expect(
      eventDateText({
        datePrecision: "day",
        dateValue: "2026-04-10",
        sortableDate: "2026-04-10",
      }),
    ).toBe("2026-04-10");
    expect(
      eventDateText({ datePrecision: "unknown", dateValue: null, sortableDate: null }),
    ).toBeNull();
    // Değer yoksa sıralama değerine düşer (uydurma yok).
    expect(
      eventDateText({
        datePrecision: "day",
        dateValue: null,
        sortableDate: "2026-04-10",
      }),
    ).toBe("2026-04-10");
  });

  it("düzenleme girişlerini kesinliğe kırpar, ham metne dokunmaz", () => {
    expect(editDateInputs("range", "2026-01-05/2026-03-09")).toEqual({
      date: "2026-01-05",
      dateEnd: "2026-03-09",
    });
    expect(editDateInputs("month", "2026-04-10")).toEqual({
      date: "2026-04",
      dateEnd: "",
    });
    expect(editDateInputs("year", "2026-04-10")).toEqual({
      date: "2026",
      dateEnd: "",
    });
    expect(editDateInputs("day", "2026-04-10")).toEqual({
      date: "2026-04-10",
      dateEnd: "",
    });
  });
});
