import { describe, expect, it } from "vitest";

import type { DocumentInventory } from "./api";
import {
  attachmentRows,
  attachmentSummary,
  changeRows,
  changeSummary,
  inventoryGateReason,
  inventoryNotices,
  inventorySupported,
  structureRows,
  structureTotal,
} from "./inventory";

function inventory(overrides: Partial<DocumentInventory> = {}): DocumentInventory {
  return {
    kind: "docx",
    structure: { paragraphs: 9, tables: 1, textBoxes: 0, cells: 2 },
    attachments: [
      { name: "word/embeddings/oleObject1.bin", kind: "ole_object", size: 84, detail: "Excel.Sheet.12" },
      { name: "word/media/image1.png", kind: "image", size: 82 },
    ],
    attachmentCount: 2,
    attachmentBytes: 166,
    changes: {
      insertions: 2,
      deletions: 1,
      comments: 1,
      authors: ["Av. Deneme", "Karsi Taraf"],
      firstDate: "2026-10-01T09:00:00Z",
      lastDate: "2026-10-02T09:00:00Z",
      revision: "12",
      lastModifiedBy: "Av. Karsi",
      creator: "Av. Deneme",
      created: "2026-09-01T09:00:00Z",
      modified: "2026-10-02T10:00:00Z",
      trackRevisions: true,
      protection: "readOnly",
      signatures: 1,
    },
    externalLinks: 1,
    notes: ["1 metin kutusu: metin kutusu içeriği gövde metnine katılmıyor (python-docx); yalnız sayıldı"],
    truncated: false,
    ...overrides,
  };
}

describe("structure rows", () => {
  it("sıfırları gizler ve çoktan aza sıralar", () => {
    const rows = structureRows(inventory());
    expect(rows.map((row) => row.key)).toEqual(["paragraphs", "cells", "tables"]);
    expect(rows[0]?.label).toBe("Paragraf");
  });

  it("bilinmeyen anahtarı kendi adıyla gösterir (uydurma etiket yok)", () => {
    const rows = structureRows(inventory({ structure: { yeniOge: 3 } }));
    expect(rows).toEqual([{ key: "yeniOge", label: "yeniOge", count: 3 }]);
  });

  it("toplam yalnız sıfırdan büyük sayıları toplar", () => {
    expect(structureTotal(inventory())).toBe(12);
    expect(structureTotal(null)).toBe(0);
  });
});

describe("attachments", () => {
  it("tür etiketini ve boyutu Türkçe biçimde verir", () => {
    const rows = attachmentRows(inventory());
    expect(rows[0]).toEqual({
      name: "word/embeddings/oleObject1.bin",
      kindLabel: "Gömülü nesne (OLE)",
      sizeLabel: "84 B",
      detail: "Excel.Sheet.12",
    });
  });

  it("bilinmeyen türü olduğu gibi yazar", () => {
    const rows = attachmentRows(
      inventory({ attachments: [{ name: "x.bin", kind: "yeni_tur", size: 1 }], attachmentCount: 1 }),
    );
    expect(rows[0]?.kindLabel).toBe("yeni_tur");
  });

  it("özet ek yoksa bunu söyler", () => {
    expect(attachmentSummary(inventory({ attachmentCount: 0 }))).toBe("Ek yok");
    expect(attachmentSummary(inventory())).toContain("2 ek");
  });
});

describe("change trail", () => {
  it("özet yalnız sıfırdan büyük olanları yazar", () => {
    expect(changeSummary(inventory().changes)).toBe("2 ekleme · 1 silme · 1 yorum");
    expect(
      changeSummary({ ...inventory().changes, insertions: 0, deletions: 0, comments: 0 }),
    ).toBe("İzlenen değişiklik yok");
  });

  it("künye satırları dolu alanlardan üretilir", () => {
    const labels = changeRows(inventory()).map((row) => row.label);
    expect(labels).toContain("Yazarlar");
    expect(labels).toContain("Revizyon");
    expect(labels).toContain("Son kaydeden");
    expect(labels).toContain("Koruma");
    expect(labels).toContain("İmza parçası");
  });

  it("UDF'te boş alanlar satır üretmez", () => {
    const rows = changeRows(
      inventory({
        kind: "udf",
        changes: {
          insertions: 0,
          deletions: 0,
          comments: 0,
          authors: [],
          firstDate: null,
          lastDate: null,
          revision: null,
          lastModifiedBy: null,
          creator: null,
          created: null,
          modified: null,
          trackRevisions: null,
          protection: null,
          signatures: 0,
        },
      }),
    );
    expect(rows).toEqual([]);
  });
});

describe("notices", () => {
  it("sınırları görünür kılar", () => {
    const notices = inventoryNotices(inventory({ truncated: true }));
    expect(notices.some((note) => note.includes("Ek listesi kesildi"))).toBe(true);
    expect(notices.some((note) => note.includes("dış bağlantı"))).toBe(true);
    expect(notices.some((note) => note.includes("doğrulama yapılmadı"))).toBe(true);
    expect(notices.some((note) => note.includes("Metin kutusu"))).toBe(true);
  });

  it("UDF'te revizyon kavramının olmadığını yazar", () => {
    const notices = inventoryNotices(inventory({ kind: "udf", externalLinks: 0 }));
    expect(notices.some((note) => note.includes("revizyon"))).toBe(true);
  });

  it("envanter yoksa uyarı üretmez", () => {
    expect(inventoryNotices(null)).toEqual([]);
  });
});

describe("gate", () => {
  it("yalnız docx ve udf açar", () => {
    expect(inventorySupported("docx")).toBe(true);
    expect(inventorySupported("udf")).toBe(true);
    expect(inventorySupported("pdf")).toBe(false);
    expect(inventorySupported(null)).toBe(false);
  });

  it("kapalıysa gerekçe verir", () => {
    expect(inventoryGateReason("docx")).toBeNull();
    expect(inventoryGateReason("pdf")).toContain("yalnız DOCX/UDF");
    expect(inventoryGateReason(null)).toContain("türü bilinmiyor");
  });
});
