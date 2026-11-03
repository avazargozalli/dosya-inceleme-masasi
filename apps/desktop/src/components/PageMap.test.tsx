// @vitest-environment jsdom
//
// Sayfa haritası (`components/PageMap.tsx`) davranış testi.
//
// Bileşen 111 satırlık **saf** bir çizim yüzeyi: ayrıştırıcının gerçekten
// ürettiği blok kutularını, kalıcı bölge işaretlerini ve arama/kaynak
// vurgusunu sayfa uzayından görünen uzaya çevirip SVG olarak çizer. Bugüne dek
// yalnız `DualReader.test.tsx` içinde "harita var mı / yok mu" ölçülmüştü;
// **hangi kutunun nereye** çizildiği, döndürme eşlemesi, birim ve künye hiç
// ölçülmemişti.
//
// Sabitlenen davranışlar:
//
// 1. Ölçüsüz sayfada (genişlik/yükseklik yok veya 0) harita **hiç** çizilmez;
//    konum uydurulmaz.
// 2. Gerçek blok kutuları birebir çizilir (x/y/genişlik/yükseklik, ince çizgi,
//    dolgu yok); `bbox` taşımayan blok için kutu üretilmez.
// 3. Künye ve erişilebilir ad **çizilen** kutuları sayar: kutusu olmayan blok
//    sayıya girmez (haritada görünmeyen blok "N blok" diye yazılırsa künye
//    haritayla çelişir).
// 4. Kalıcı bölge işaretleri kesikli/uyarı rengiyle, vurgu kutusu aksan
//    dolgusuyla çizilir; katmanlar birbirine karışmaz.
// 5. Döndürme toplamı görünen uzayı belirler: 90/270'te viewBox ve kutu
//    koordinatları takas edilir, 180'de ters çevrilir, 450 → 90 normalize
//    edilir.
// 6. Künye ölçüyü **döndürme öncesi** sayfa boyutundan yuvarlayarak yazar,
//    birimi koordinat uzayından seçer (`image_pixels` → piksel, diğerleri
//    punto) ve döndürme yoksa "döndürülmüş" ibaresini yazmaz.
// 7. Sıfır/negatif alanlı kutu 0,5 birimden ince çizilmez (görünmez kutu
//    bırakılmaz).

import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { PageMap } from "./PageMap";
import type { PageBlock, PageRow } from "../lib/api";
import { mapRectForDisplay } from "../lib/reader";
import { t } from "../lib/i18n";

afterEach(cleanup);

function page(overrides: Partial<PageRow> = {}): PageRow {
  return {
    id: "p1",
    segmentId: "s1",
    pageIndex: 0,
    text: "Sayfa metni",
    textOrigin: "native",
    qualityFlags: "",
    width: 400,
    height: 600,
    rotation: 0,
    coordinateSpace: "unrotated_cropbox_points",
    pageLabel: null,
    blocks: [],
    ...overrides,
  };
}

function block(bbox: [number, number, number, number] | null, start = 0, end = 5): PageBlock {
  return { start, end, bbox };
}

const EMPTY: [number, number, number, number][] = [];

function draw(pageRow: PageRow, opts: {
  highlight?: [number, number, number, number][];
  regions?: [number, number, number, number][];
  rotation?: number;
} = {}) {
  const { highlight = EMPTY, regions = EMPTY, rotation = 0 } = opts;
  const view = render(
    <PageMap page={pageRow} highlight={highlight} regions={regions} rotation={rotation} />,
  );
  return view;
}

function rects(container: HTMLElement): SVGElement[] {
  return Array.from(container.querySelectorAll("rect")) as unknown as SVGElement[];
}

function geom(rect: SVGElement) {
  return {
    x: rect.getAttribute("x"),
    y: rect.getAttribute("y"),
    width: rect.getAttribute("width"),
    height: rect.getAttribute("height"),
    fill: rect.getAttribute("fill"),
    stroke: rect.getAttribute("stroke"),
    dash: rect.getAttribute("stroke-dasharray"),
  };
}

function caption(container: HTMLElement): string {
  return container.querySelector("figcaption")?.textContent ?? "";
}

describe("PageMap — sayfa görüntüsü değil, gerçek blok kutuları", () => {
  it("ölçüsüz sayfada harita hiç çizilmez", () => {
    const missing = draw(page({ width: null, height: null, blocks: [block([10, 10, 50, 50])] }));
    expect(missing.container.querySelector("svg")).toBeNull();
    expect(missing.container.querySelector("figure")).toBeNull();
    expect(caption(missing.container)).toBe("");

    const zero = draw(page({ width: 0, height: 600, blocks: [block([10, 10, 50, 50])] }));
    expect(zero.container.querySelector("svg")).toBeNull();

    const negative = draw(page({ width: 400, height: -1, blocks: [block([10, 10, 50, 50])] }));
    expect(negative.container.querySelector("svg")).toBeNull();
  });

  it("gerçek blok kutuları birebir çizilir, kutusuz blok çizilmez", () => {
    const { container } = draw(
      page({
        blocks: [
          block([10, 20, 110, 40], 0, 5),
          { start: 5, end: 9 },
          block([0, 0, 400, 600], 9, 12),
        ],
      }),
    );

    const boxes = rects(container);
    expect(boxes).toHaveLength(2);
    expect(geom(boxes[0]!)).toMatchObject({
      x: "10",
      y: "20",
      width: "100",
      height: "20",
      fill: "none",
      stroke: "var(--line-strong)",
    });
    expect(geom(boxes[1]!)).toMatchObject({ x: "0", y: "0", width: "400", height: "600" });
  });

  it("künye ve erişilebilir ad yalnız çizilen kutuları sayar", () => {
    const { container } = draw(
      page({
        blocks: [
          block([10, 20, 110, 40], 0, 5),
          { start: 5, end: 9 }, // kutusu yok → haritada yok
          block([0, 0, 400, 600], 9, 12),
        ],
      }),
    );

    const svg = container.querySelector("svg")!;
    expect(svg.getAttribute("aria-label")).toBe(
      t("pageMap.aria", { physical: t("readerUi.physicalPage", { page: 1 }), count: 2 }),
    );
    expect(caption(container)).toContain(t("pageMap.blocks", { count: 2 }));
  });

  it("bölge işaretleri kesikli uyarı rengiyle, vurgu aksan dolgusuyla çizilir", () => {
    const { container } = draw(
      page({ blocks: [block([0, 0, 400, 600])] }),
      {
        regions: [[10, 10, 60, 40]],
        highlight: [[100, 120, 200, 160]],
      },
    );

    const [blockRect, region, hit] = rects(container);
    expect(geom(blockRect!)).toMatchObject({ fill: "none", stroke: "var(--line-strong)" });
    expect(geom(region!)).toMatchObject({
      x: "10",
      y: "10",
      width: "50",
      height: "30",
      fill: "none",
      stroke: "var(--warn)",
      dash: "3 2",
    });
    expect(geom(hit!)).toMatchObject({
      x: "100",
      y: "120",
      width: "100",
      height: "40",
      fill: "var(--accent-soft)",
      stroke: "var(--accent)",
    });
  });

  it("90° döndürmede viewBox takas edilir ve kutular görünen uzaya çevrilir", () => {
    const { container } = draw(page({ blocks: [block([10, 20, 110, 40])] }), { rotation: 90 });

    const svg = container.querySelector("svg")!;
    expect(svg.getAttribute("viewBox")).toBe("0 0 600 400");
    const [box] = rects(container);
    const [x0, y0, x1, y1] = mapRectForDisplay([10, 20, 110, 40], 400, 600, 90);
    expect(geom(box!)).toMatchObject({
      x: String(x0),
      y: String(y0),
      width: String(x1 - x0),
      height: String(y1 - y0),
    });
  });

  it("180° ve 270° döndürmede kutular ters çevrilir/takas edilir", () => {
    const bbox: [number, number, number, number] = [10, 20, 110, 40];

    const flat = draw(page({ blocks: [block(bbox)] }), { rotation: 180 });
    const [flatBox] = rects(flat.container);
    const [fx0, fy0, fx1, fy1] = mapRectForDisplay(bbox, 400, 600, 180);
    expect(geom(flatBox!)).toMatchObject({
      x: String(fx0),
      y: String(fy0),
      width: String(fx1 - fx0),
      height: String(fy1 - fy0),
    });
    expect(flat.container.querySelector("svg")!.getAttribute("viewBox")).toBe("0 0 400 600");

    const turn = draw(page({ blocks: [block(bbox)] }), { rotation: 270 });
    const [turnBox] = rects(turn.container);
    const [tx0, ty0, tx1, ty1] = mapRectForDisplay(bbox, 400, 600, 270);
    expect(geom(turnBox!)).toMatchObject({
      x: String(tx0),
      y: String(ty0),
      width: String(tx1 - tx0),
      height: String(ty1 - ty0),
    });
    expect(turn.container.querySelector("svg")!.getAttribute("viewBox")).toBe("0 0 600 400");
  });

  it("döndürme 450° → 90° normalize edilir (künye dahil)", () => {
    const { container } = draw(page({ blocks: [block([0, 0, 10, 10])] }), { rotation: 450 });
    expect(container.querySelector("svg")!.getAttribute("viewBox")).toBe("0 0 600 400");
    expect(caption(container)).toContain(t("pageMap.rotated", { rotation: 90 }));
  });

  it("künye döndürme öncesi boyutu yuvarlar, birimi koordinat uzayından seçer", () => {
    const points = draw(page({ width: 400.4, height: 599.6, blocks: [block([0, 0, 10, 10])] }));
    expect(caption(points.container)).toContain(
      t("pageMap.dimensions", { width: 400, height: 600, unit: t("pageMap.unitPoint") }),
    );

    const pixels = draw(
      page({
        width: 1200,
        height: 1600,
        coordinateSpace: "image_pixels",
        blocks: [block([0, 0, 10, 10])],
      }),
    );
    expect(caption(pixels.container)).toContain(
      t("pageMap.dimensions", { width: 1200, height: 1600, unit: t("pageMap.unitPixel") }),
    );
  });

  it("döndürme yoksa künyede döndürme ibaresi yazılmaz", () => {
    const { container } = draw(page({ blocks: [block([0, 0, 10, 10])] }));
    expect(caption(container)).not.toContain(t("pageMap.rotated", { rotation: 0 }));

    // 360° tam tur demek: görünür döndürme yok, künye "0° döndürülmüş" yazmamalı.
    const full = draw(page({ blocks: [block([0, 0, 10, 10])] }), { rotation: 360 });
    expect(caption(full.container)).not.toContain(t("pageMap.rotated", { rotation: 0 }));
  });

  it("sıfır alanlı kutu en az 0,5 birim kalınlıkla çizilir", () => {
    const { container } = draw(page({ blocks: [block([30, 40, 30, 40])] }));
    const [box] = rects(container);
    expect(geom(box!)).toMatchObject({ x: "30", y: "40", width: "0.5", height: "0.5" });
  });
});
