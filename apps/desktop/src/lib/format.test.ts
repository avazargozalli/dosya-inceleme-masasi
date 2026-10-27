import { describe, expect, it } from "vitest";
import {
  AI_POLICY_LABEL,
  CASE_KIND_LABEL,
  SENSITIVITY_LABEL,
  formatDateTime,
  formatDuration,
  formatLegalDate,
  formatNumber,
  formatPercent,
  formatSize,
  formatTimestamp,
  formatUnixSeconds,
} from "./format";

/** Takvim tarihi referansı: aynı günü **yerel** olarak kurar (saat dilimi kaymaz). */
function localDateText(year: number, month: number, day: number): string {
  return new Intl.DateTimeFormat("tr-TR", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
  }).format(new Date(year, month - 1, day));
}

describe("formatTimestamp / formatDateTime", () => {
  it("boş değerde tire döner", () => {
    expect(formatTimestamp(null)).toBe("—");
    expect(formatTimestamp(undefined)).toBe("—");
    expect(formatDateTime(null)).toBe("—");
  });

  it("geçersiz ISO'da tire döner", () => {
    expect(formatTimestamp("bozuk")).toBe("—");
    expect(formatDateTime("2026-13-45T99:99:99Z")).toBe("—");
  });

  it("geçerli ISO'yu tarih+saat olarak biçimler", () => {
    const out = formatTimestamp("2026-10-23T10:00:00Z");
    expect(out).toMatch(/^\d{2}\.\d{2}\.\d{4} \d{2}:\d{2}$/);
    expect(out).not.toBe("—");
  });

  it("formatTimestamp ile formatDateTime aynı sonucu verir (tek uygulama)", () => {
    expect(formatTimestamp("2026-03-09T06:07:00Z")).toBe(formatDateTime("2026-03-09T06:07:00Z"));
  });
});

describe("formatUnixSeconds", () => {
  it("saniyeyi yerel damgaya çevirir", () => {
    const seconds = Math.floor(new Date("2026-05-04T08:30:00Z").getTime() / 1000);
    expect(formatUnixSeconds(seconds)).toBe(formatDateTime("2026-05-04T08:30:00Z"));
  });

  it("sıfır/negatif/eksik yoklamayı '—' yazar (1970 uydurulmaz)", () => {
    expect(formatUnixSeconds(0)).toBe("—");
    expect(formatUnixSeconds(-5)).toBe("—");
    expect(formatUnixSeconds(null)).toBe("—");
    expect(formatUnixSeconds(Number.NaN)).toBe("—");
  });
});

describe("formatLegalDate — hukuki takvim tarihi", () => {
  it("gün varsa gg.aa.yyyy yazar", () => {
    expect(formatLegalDate("2026-10-23")).toBe("23.10.2026");
    expect(formatLegalDate("2026-01-05")).toBe("05.01.2026");
  });

  it("takvim günü saat dilimine göre kaymaz", () => {
    // Naive `new Date("2026-01-01")` UTC gece yarısıdır; eksi saat diliminde
    // 31.12.2025'e düşer. Referans yerel kurulumla karşılaştırılıyor.
    expect(formatLegalDate("2026-01-01")).toBe(localDateText(2026, 1, 1));
    expect(formatLegalDate("2026-12-31")).toBe(localDateText(2026, 12, 31));
  });

  it("eksik gün uydurmaz: yalnız ay ve yalnız yıl", () => {
    expect(formatLegalDate("2026-10")).toBe("10.2026");
    expect(formatLegalDate("2026")).toBe("2026");
  });

  it("olmayan tarihi okunamadı diye işaretler", () => {
    expect(formatLegalDate("2026-02-30")).toBe("tarih okunamadı");
    expect(formatLegalDate("2026-13")).toBe("tarih okunamadı");
    expect(formatLegalDate("2026-00-10")).toBe("tarih okunamadı");
  });

  it("zaman bileşeni taşıyan değeri takvim tarihi saymaz", () => {
    expect(formatLegalDate("2026-10-23T10:00:00Z")).toBe("tarih biçimi desteklenmiyor");
  });

  it("boş değerde tire döner", () => {
    expect(formatLegalDate(null)).toBe("—");
    expect(formatLegalDate("")).toBe("—");
  });
});

describe("formatNumber / formatPercent", () => {
  it("binlik ayracı nokta, ondalık virgüldür", () => {
    expect(formatNumber(1234)).toBe("1.234");
    expect(formatNumber(1234567)).toBe("1.234.567");
    expect(formatNumber(12)).toBe("12");
    expect(formatNumber(0)).toBe("0");
  });

  it("geçersiz sayıda tire döner", () => {
    expect(formatNumber(null)).toBe("—");
    expect(formatNumber(undefined)).toBe("—");
    expect(formatNumber(Number.NaN)).toBe("—");
    expect(formatNumber(Number.POSITIVE_INFINITY)).toBe("—");
  });

  it("oranı yüzdeye çevirir", () => {
    expect(formatPercent(0.3)).toBe("%30");
    expect(formatPercent(1)).toBe("%100");
    expect(formatPercent(0)).toBe("%0");
    expect(formatPercent(Number.NaN)).toBe("—");
  });
});

describe("formatSize — tek boyut biçimi", () => {
  it("1024 altını bayt olarak yazar", () => {
    expect(formatSize(0)).toBe("0 bayt");
    expect(formatSize(1023)).toBe("1.023 bayt");
  });

  it("KiB/MiB kullanır (1024 tabanı KB değil)", () => {
    expect(formatSize(1024)).toBe("1 KiB");
    expect(formatSize(1536)).toBe("1,5 KiB");
    expect(formatSize(20 * 1024)).toBe("20 KiB");
    expect(formatSize(1048576)).toBe("1 MiB");
    expect(formatSize(2.5 * 1048576)).toBe("2,5 MiB");
  });

  it("Türkçe ondalık virgül kullanır, nokta değil", () => {
    expect(formatSize(1536)).toContain(",");
    expect(formatSize(1536)).not.toContain(".");
  });

  it("geçersiz/negatif değerde tire döner", () => {
    expect(formatSize(null)).toBe("—");
    expect(formatSize(-1)).toBe("—");
  });
});

describe("formatDuration", () => {
  it("saniye altını ms yazar", () => {
    expect(formatDuration(340)).toBe("340 ms");
    expect(formatDuration(0)).toBe("0 ms");
  });

  it("saniyeyi bir ondalıkla yazar", () => {
    expect(formatDuration(1234)).toBe("1,2 sn");
    expect(formatDuration(10000)).toBe("10 sn");
  });

  it("geçersizde tire döner", () => {
    expect(formatDuration(null)).toBe("—");
    expect(formatDuration(-3)).toBe("—");
  });
});

describe("etiketler", () => {
  it("bilinen türleri kapsar", () => {
    expect(CASE_KIND_LABEL.review).toBe("İnceleme dosyası");
    expect(SENSITIVITY_LABEL.high).toBe("Yüksek");
    expect(AI_POLICY_LABEL.api_allowed).toBe("API izinli");
  });
});
