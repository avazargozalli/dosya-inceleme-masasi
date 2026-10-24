import { describe, expect, it } from "vitest";
import type { AnchorLocationRow, DocumentRow, PageRow, SearchHit } from "./api";
import {
  EMPTY_VIEW,
  ZOOM_STEPS,
  blocksForRange,
  clampZoom,
  displayPageSize,
  entrySlice,
  mapRectForDisplay,
  nextZoom,
  offsetSlice,
  pageForKey,
  pageLabelInfo,
  parseWarnings,
  pickInitialVersion,
  previewCaption,
  previewDataUrl,
  querySlice,
  recallView,
  rememberView,
  resolvePageIndex,
  rotateView,
  sliceForPage,
  structureEntries,
  targetFromAnchor,
  targetFromHit,
  validSelection,
} from "./reader";

const pages: PageRow[] = [
  {
    id: "p1",
    segmentId: "s1",
    pageIndex: 0,
    text: "Birinci sayfa",
    textOrigin: "native",
    qualityFlags: "[]",
    width: null,
    height: null,
    rotation: 0,
    coordinateSpace: "structure",
    pageLabel: null,
    blocks: [],
  },
  {
    id: "p2",
    segmentId: "s2",
    pageIndex: 1,
    text: "İkinci sayfa: İTİRAZIN reddine karar verildi.",
    textOrigin: "ocr",
    qualityFlags: "[]",
    width: 300,
    height: 400,
    rotation: 90,
    coordinateSpace: "unrotated_cropbox_points",
    pageLabel: "iv",
    blocks: [
      { start: 0, end: 15, bbox: [10, 10, 200, 24], lines: [] },
      { start: 15, end: 47, bbox: [10, 40, 180, 54], lines: [[10, 40, 180, 54]] },
    ],
  },
];

const hit: SearchHit = {
  segmentId: "s2",
  pageId: "p2",
  documentVersionId: "v-old",
  documentId: "d1",
  documentName: "Dilekçe.pdf",
  documentType: "petition",
  documentStatus: "active",
  textOrigin: "native",
  caseId: "c1",
  tags: [],
  rawText: "İkinci sayfa: İTİRAZIN reddine karar verildi.",
  pageIndex: 1,
  rank: 0.5,
};

const anchor: AnchorLocationRow = {
  anchorId: "a1",
  caseId: "c1",
  documentVersionId: "v-old",
  documentId: "d1",
  documentName: "Dilekçe.pdf",
  parseRunId: "r1",
  pageId: "p2",
  pageIndex: 1,
  segmentId: "s2",
  startOffset: 14,
  endOffset: 30,
  exactQuote: "İTİRAZIN reddine",
  anchorStatus: "active",
  quoteIntact: true,
  quads: [[10, 40, 180, 54]],
  coordinateSpace: "unrotated_cropbox_points",
  pageRotation: 90,
};

describe("targetFromHit", () => {
  it("sonucun kendi sürümünü taşır, en son sürümü tahmin etmez", () => {
    const target = targetFromHit(hit, "itiraz");
    expect(target?.versionId).toBe("v-old");
    expect(target?.pageId).toBe("p2");
    expect(target?.query).toBe("itiraz");
  });

  it("sürüm kimliği yoksa hedef üretmez", () => {
    expect(targetFromHit({ ...hit, documentVersionId: null }, "itiraz")).toBeNull();
  });
});

describe("targetFromAnchor", () => {
  it("sayfa, offset ve kopma durumunu taşır", () => {
    const target = targetFromAnchor(anchor);
    expect(target.versionId).toBe("v-old");
    expect(target.startOffset).toBe(14);
    expect(target.quoteIntact).toBe(true);
    expect(target.quads).toEqual([[10, 40, 180, 54]]);
    expect(target.anchorId).toBe("a1");
  });
});

describe("resolvePageIndex", () => {
  it("sayfa kimliğini fiziksel indekse tercih eder", () => {
    expect(resolvePageIndex(pages, { ...targetFromAnchor(anchor), pageIndex: 0 })).toBe(1);
  });

  it("segment kimliğiyle çözer", () => {
    expect(resolvePageIndex(pages, targetFromHit(hit, "itiraz"))).toBe(1);
  });

  it("aralık dışı indeksi kırpar", () => {
    const target = { ...targetFromAnchor(anchor), pageId: null, segmentId: null, pageIndex: 99 };
    expect(resolvePageIndex(pages, target)).toBe(1);
  });

  it("boş belgede -1 döner", () => {
    expect(resolvePageIndex([], targetFromAnchor(anchor))).toBe(-1);
  });
});

describe("offsetSlice", () => {
  it("Unicode scalar aralığını keser", () => {
    const text = "A😀BğC";
    expect(offsetSlice(text, 1, 3)).toEqual({ before: "A", hit: "😀B", after: "ğC" });
  });

  it("geçersiz aralıkta null döner", () => {
    expect(offsetSlice("kısa", 0, 99)).toBeNull();
    expect(offsetSlice("kısa", 3, 1)).toBeNull();
    expect(offsetSlice("kısa", null, 1)).toBeNull();
  });
});

describe("querySlice", () => {
  it("Türkçe büyük harfli metinde sorguyu işaretler", () => {
    const slice = querySlice("İkinci sayfa: İTİRAZIN reddine", "itiraz");
    expect(slice?.hit).toBe("İTİRAZ");
    expect(slice?.before).toBe("İkinci sayfa: ");
  });

  it("bulunmayan sorguda null döner", () => {
    expect(querySlice("metin", "kira")).toBeNull();
    expect(querySlice("metin", "   ")).toBeNull();
  });
});

describe("parseWarnings", () => {
  it("JSON uyarı dizisini çözer", () => {
    expect(parseWarnings('["UDF imza: tespit edildi — denetlenmedi"]')).toEqual([
      "UDF imza: tespit edildi — denetlenmedi",
    ]);
  });

  it("bozuk veya dizi olmayan kayıtta boş liste döner", () => {
    expect(parseWarnings("bozuk")).toEqual([]);
    expect(parseWarnings('{"a":1}')).toEqual([]);
    expect(parseWarnings(null)).toEqual([]);
  });
});

describe("sliceForPage", () => {
  it("anchor yalnız kendi sayfasında işaretlenir", () => {
    const target = targetFromAnchor(anchor);
    expect(sliceForPage(pages[1]!.text, target, pages[1]!)).not.toBeNull();
    expect(sliceForPage(pages[0]!.text, target, pages[0]!)).toBeNull();
  });

  it("offset yoksa sorgu geçişini işaretler", () => {
    const target = targetFromHit(hit, "reddine");
    expect(sliceForPage(pages[1]!.text, target, pages[1]!)?.hit).toBe("reddine");
  });
});

describe("displayPageSize", () => {
  it("90/270'te eksenleri değiştirir", () => {
    expect(displayPageSize(300, 400, 0)).toEqual({ width: 300, height: 400 });
    expect(displayPageSize(300, 400, 90)).toEqual({ width: 400, height: 300 });
    expect(displayPageSize(300, 400, 180)).toEqual({ width: 300, height: 400 });
    expect(displayPageSize(300, 400, 270)).toEqual({ width: 400, height: 300 });
    expect(displayPageSize(300, 400, -90)).toEqual({ width: 400, height: 300 });
  });
});

describe("mapRectForDisplay", () => {
  it("90 derece döndürmede kutuyu görünen uzaya taşır", () => {
    // PyMuPDF: (50,100,120,115) * rotation_matrix(90) = (285,50,300,120)
    expect(mapRectForDisplay([50, 100, 120, 115], 300, 400, 90)).toEqual([285, 50, 300, 120]);
  });

  it("180 ve 270 derecede kutuyu sayfa içinde tutar", () => {
    expect(mapRectForDisplay([10, 20, 30, 40], 300, 400, 180)).toEqual([270, 360, 290, 380]);
    expect(mapRectForDisplay([10, 20, 30, 40], 300, 400, 270)).toEqual([20, 270, 40, 290]);
    expect(mapRectForDisplay([10, 20, 30, 40], 300, 400, 0)).toEqual([10, 20, 30, 40]);
  });
});

describe("blocksForRange", () => {
  it("yalnız aralıkla kesişen blokların kutusunu döndürür", () => {
    expect(blocksForRange(pages[1]!.blocks, { start: 15, end: 20 })).toEqual([[10, 40, 180, 54]]);
    expect(blocksForRange(pages[1]!.blocks, { start: 0, end: 1 })).toEqual([[10, 10, 200, 24]]);
    expect(blocksForRange(pages[1]!.blocks, null)).toHaveLength(2);
  });

  it("kutusu olmayan blok için konum uydurmaz", () => {
    expect(blocksForRange([{ start: 0, end: 5 }], null)).toEqual([]);
  });
});

describe("yakınlaştırma", () => {
  it("kademeler arasında ilerler ve uçta taşmaz", () => {
    expect(nextZoom(1, 1)).toBe(1.25);
    expect(nextZoom(1, -1)).toBe(0.75);
    expect(nextZoom(2, 1)).toBe(2);
    expect(nextZoom(0.75, -1)).toBe(0.75);
  });

  it("aralık dışı değeri kırpar, sayı olmayanda %100 döner", () => {
    expect(clampZoom(9)).toBe(2);
    expect(clampZoom(0.1)).toBe(0.75);
    expect(clampZoom(Number.NaN)).toBe(1);
    expect(ZOOM_STEPS).toContain(1);
  });
});

describe("rotateView", () => {
  it("belge ve görünüm döndürmesini toplar, 360'ta sıfırlar", () => {
    expect(rotateView(90, 90)).toBe(180);
    expect(rotateView(270, 90)).toBe(0);
    expect(rotateView(0, -90)).toBe(270);
    expect(rotateView(0, 0)).toBe(0);
  });
});

describe("pageLabelInfo", () => {
  it("fiziksel numarayı her zaman verir, etiketi yalnız tanımlıysa gösterir", () => {
    expect(pageLabelInfo({ pageIndex: 3, pageLabel: "iv" })).toEqual({
      label: "iv",
      physical: "Fiziksel sayfa 4",
      showsLabel: true,
    });
    expect(pageLabelInfo({ pageIndex: 0, pageLabel: null })).toEqual({
      label: null,
      physical: "Fiziksel sayfa 1",
      showsLabel: false,
    });
    expect(pageLabelInfo({ pageIndex: 0, pageLabel: "   " }).label).toBeNull();
  });

  it("etiket fiziksel numarayla aynıysa yinelemez", () => {
    expect(pageLabelInfo({ pageIndex: 2, pageLabel: "3" }).showsLabel).toBe(false);
    expect(pageLabelInfo({ pageIndex: 2, pageLabel: "A-3" }).showsLabel).toBe(true);
  });
});

const structurePage: PageRow = {
  id: "p3",
  segmentId: "s3",
  pageIndex: 0,
  text: "Kira Sözleşmesi\n\nÖdeme · p/2",
  textOrigin: "native",
  qualityFlags: "[]",
  width: null,
  height: null,
  rotation: 0,
  coordinateSpace: "structure",
  pageLabel: null,
  blocks: [
    { start: 0, end: 15, path: "Kira Sözleşmesi", kind: "heading" },
    { start: 17, end: 28, path: "Kira Sözleşmesi · p/2", kind: "paragraph" },
  ],
};

describe("structureEntries", () => {
  it("paragraf aralıklarını ve gerçek yolları verir", () => {
    const entries = structureEntries(structurePage);
    expect(entries).toHaveLength(2);
    expect(entries[0]!.text).toBe("Kira Sözleşmesi");
    expect(entries[0]!.heading).toBe(true);
    expect(entries[1]!.path).toBe("Kira Sözleşmesi · p/2");
    expect(entries[1]!.heading).toBe(false);
    expect(entries[0]!.end).toBe(15);
  });

  it("PDF sayfasında yapı üretmez", () => {
    expect(structureEntries(pages[0]!)).toEqual([]);
  });

  it("aralık dışı bloğu kırpar, yol yoksa uydurmaz", () => {
    const page = { ...structurePage, blocks: [{ start: 0, end: 999 }] };
    const [entry] = structureEntries(page);
    expect(entry!.end).toBe(Array.from(structurePage.text).length);
    expect(entry!.path).toBeNull();
  });
});

describe("entrySlice", () => {
  it("vurgu aralığını paragraf içinde keser", () => {
    const entries = structureEntries(structurePage);
    const piece = entrySlice(entries[1]!, { start: 18, end: 23 });
    expect(piece.before).toBe("Ö");
    expect(piece.hit).toBe("deme ");
    expect(piece.after).toBe("· p/2");
  });

  it("aralık paragrafla kesişmezse tüm metni before verir", () => {
    const entries = structureEntries(structurePage);
    const piece = entrySlice(entries[0]!, { start: 17, end: 19 });
    expect(piece.before).toBe("Kira Sözleşmesi");
    expect(piece.hit).toBe("");
  });
});

describe("sayfa önizlemesi (F07-T11)", () => {
  it("base64 PNG'yi data URL'e çevirir", () => {
    expect(previewDataUrl("iVBORw0KGgo=")).toBe("data:image/png;base64,iVBORw0KGgo=");
  });

  it("önbellekten gelen görüntüyü kaynağıyla yazar", () => {
    expect(previewCaption({ cached: true, byteSize: 2048, dpi: 96 })).toBe(
      "şifreli önbellekten · 2 KB · 96 dpi",
    );
  });

  it("yeni üretilen görüntüyü önbellek diye sunmaz", () => {
    const caption = previewCaption({ cached: false, byteSize: 1536 * 1024, dpi: 200 });
    expect(caption).toContain("yeni üretildi");
    expect(caption).not.toContain("önbellek");
    expect(caption).toContain("1.5 MB");
  });

  it("küçük görüntüde 1 KB'dan aşağı yazmaz", () => {
    expect(previewCaption({ cached: true, byteSize: 20, dpi: 48 })).toContain("1 KB");
  });
});

describe("panel görünüm belleği (F07-T08)", () => {
  const view = { page: 3, zoom: 1.5, rotation: 90, selection: null };

  it("sürümün görünümünü hatırlar ve geri verir", () => {
    const memory = rememberView([], "v1", view);
    expect(recallView(memory, "v1")).toEqual(view);
    expect(recallView(memory, "v2")).toBeNull();
    expect(recallView(memory, null)).toBeNull();
  });

  it("aynı sürüm yeniden kaydedilince kaydı günceller, çoğaltmaz", () => {
    let memory = rememberView([], "v1", view);
    memory = rememberView(memory, "v1", { ...view, page: 7 });
    expect(memory).toHaveLength(1);
    expect(recallView(memory, "v1")?.page).toBe(7);
  });

  it("sınırı aşınca en eski kaydı düşürür", () => {
    let memory = rememberView([], "v1", view, 2);
    memory = rememberView(memory, "v2", view, 2);
    memory = rememberView(memory, "v3", view, 2);
    expect(memory.map((entry) => entry.versionId)).toEqual(["v2", "v3"]);
  });

  it("boş sürüm kimliğini kaydetmez", () => {
    expect(rememberView([], "", view)).toHaveLength(0);
  });

  it("kaydı panelin dışına taşırmaz: döndürülen kopya değiştirilebilir", () => {
    const selection = { quote: "kira", start: 0, end: 4 };
    const memory = rememberView([], "v1", { ...view, selection });
    selection.quote = "değişti";
    expect(recallView(memory, "v1")?.selection?.quote).toBe("kira");
  });
});

describe("seçim doğrulama (F07-T08)", () => {
  const text = "İTİRAZIN reddine karar verildi.";

  it("alıntı kaydedildiği offset'te duruyorsa seçimi geri verir", () => {
    const start = Array.from(text.slice(0, text.indexOf("reddine"))).length;
    expect(validSelection(text, { quote: "reddine", start, end: start + 7 })).toEqual({
      quote: "reddine",
      start,
      end: start + 7,
    });
  });

  it("offset kaydıysa seçimi geri yüklemez", () => {
    expect(validSelection(text, { quote: "reddine", start: 11, end: 18 })).toBeNull();
  });

  it("aralık dışı, ters ve tam sayı olmayan aralığı reddeder", () => {
    expect(validSelection(text, { quote: "x", start: 0, end: 999 })).toBeNull();
    expect(validSelection(text, { quote: "x", start: 5, end: 5 })).toBeNull();
    expect(validSelection(text, { quote: "x", start: 0.5, end: 2 })).toBeNull();
  });

  it("seçim yoksa null döner", () => {
    expect(validSelection(text, null)).toBeNull();
  });
});

describe("panel klavye gezinmesi (F07-T08)", () => {
  it("Alt ile sayfa değiştirir, uçlarda durur", () => {
    expect(pageForKey("PageDown", { alt: true }, 0, 3)).toBe(1);
    expect(pageForKey("ArrowRight", { alt: true }, 1, 3)).toBe(2);
    expect(pageForKey("PageDown", { alt: true }, 2, 3)).toBeNull();
    expect(pageForKey("ArrowLeft", { alt: true }, 0, 3)).toBeNull();
  });

  it("Alt+Home/End ilk ve son sayfaya gider", () => {
    expect(pageForKey("Home", { alt: true }, 2, 4)).toBe(0);
    expect(pageForKey("End", { alt: true }, 1, 4)).toBe(3);
    expect(pageForKey("Home", { alt: true }, 0, 4)).toBeNull();
    expect(pageForKey("End", { alt: true }, 3, 4)).toBeNull();
  });

  it("Alt yoksa tuşu yutmaz (metin kaydırması bozulmaz)", () => {
    expect(pageForKey("PageDown", {}, 0, 3)).toBeNull();
    expect(pageForKey("ArrowDown", { alt: false }, 0, 3)).toBeNull();
  });

  it("başka değiştiriciyle veya boş belgede gezinmez", () => {
    expect(pageForKey("PageDown", { alt: true, ctrl: true }, 0, 3)).toBeNull();
    expect(pageForKey("PageDown", { alt: true, meta: true }, 0, 3)).toBeNull();
    expect(pageForKey("PageDown", { alt: true, shift: true }, 0, 3)).toBeNull();
    expect(pageForKey("PageDown", { alt: true }, 0, 0)).toBeNull();
    expect(pageForKey("x", { alt: true }, 0, 3)).toBeNull();
  });
});

describe("boş panelde açılacak belge (F07-T08)", () => {
  const doc = (id: string, versionId: string | null): DocumentRow => ({
    id,
    caseId: "c1",
    displayName: `${id}.pdf`,
    documentType: "petition",
    status: "active",
    currentVersionId: versionId,
    declaredDocumentDate: null,
    createdAt: "2026-10-24T00:00:00Z",
    updatedAt: "2026-10-24T00:00:00Z",
    rowVersion: 1,
  });

  it("diğer panelde açık sürümü seçmez", () => {
    const documents = [doc("d1", "v1"), doc("d2", "v2")];
    expect(pickInitialVersion(documents, "v1")).toBe("v2");
  });

  it("başka belge yoksa tek belgeye döner (panel boş kalmaz)", () => {
    expect(pickInitialVersion([doc("d1", "v1")], "v1")).toBe("v1");
  });

  it("sürümü olmayan belgeyi ve boş listeyi atlar", () => {
    expect(pickInitialVersion([doc("d1", null)], null)).toBeNull();
    expect(pickInitialVersion([], null)).toBeNull();
    expect(pickInitialVersion([doc("d1", null), doc("d2", "v2")], null)).toBe("v2");
  });
});
