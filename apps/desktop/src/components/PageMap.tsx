import type { PageRow } from "../lib/api";
import { t } from "../lib/i18n";
import {
  displayPageSize,
  mapRectForDisplay,
  normalizeRotation,
  pageLabelInfo,
} from "../lib/reader";

/** Sayfa haritası: ayrıştırıcının **gerçekten ürettiği** blok kutuları.
 *  Sayfanın görüntüsü değildir; kutu yoksa hiç çizilmez (konum uydurulmaz).
 *  `rotation` belgenin kendi döndürmesi ile görünüm döndürmesinin toplamıdır. */
function PageMap({
  page,
  highlight,
  regions,
  rotation,
}: {
  page: PageRow;
  highlight: [number, number, number, number][];
  /** Kalıcı bölge işaretleri (sayfa koordinat uzayında). */
  regions: [number, number, number, number][];
  rotation: number;
}) {
  const width = page.width ?? 0;
  const height = page.height ?? 0;
  if (width <= 0 || height <= 0) return null;
  const display = displayPageSize(width, height, rotation);
  const displayRotation = normalizeRotation(rotation);
  // Harita yalnız **kutusu olan** blokları çizer; künye/erişilebilir ad da
  // çizilen kutuları sayar (haritada görünmeyen blok "N blok" diye yazılırsa
  // künye haritayla çelişirdi).
  const drawnBlocks = page.blocks.filter((block) => block.bbox).length;
  const unit =
    page.coordinateSpace === "image_pixels" ? t("pageMap.unitPixel") : t("pageMap.unitPoint");
  const label = pageLabelInfo(page);
  return (
    <figure style={{ margin: 0 }}>
      <svg
        viewBox={`0 0 ${display.width} ${display.height}`}
        role="img"
        aria-label={t("pageMap.aria", {
          physical: label.physical,
          count: drawnBlocks,
        })}
        style={{
          display: "block",
          width: "100%",
          maxWidth: 300,
          maxHeight: 150,
          background: "var(--surface)",
          border: "1px solid var(--line)",
          borderRadius: "var(--radius)",
        }}
      >
        {page.blocks.map((block, index) => {
          if (!block.bbox) return null;
          const [x0, y0, x1, y1] = mapRectForDisplay(block.bbox, width, height, rotation);
          return (
            <rect
              key={`block-${index}`}
              x={x0}
              y={y0}
              width={Math.max(x1 - x0, 0.5)}
              height={Math.max(y1 - y0, 0.5)}
              fill="none"
              stroke="var(--line-strong)"
              strokeWidth={0.6}
              vectorEffect="non-scaling-stroke"
            />
          );
        })}
        {regions.map((rect, index) => {
          const [x0, y0, x1, y1] = mapRectForDisplay(rect, width, height, rotation);
          return (
            <rect
              key={`region-${index}`}
              x={x0}
              y={y0}
              width={Math.max(x1 - x0, 0.5)}
              height={Math.max(y1 - y0, 0.5)}
              fill="none"
              stroke="var(--warn)"
              strokeDasharray="3 2"
              strokeWidth={1}
              vectorEffect="non-scaling-stroke"
            />
          );
        })}
        {highlight.map((rect, index) => {
          const [x0, y0, x1, y1] = mapRectForDisplay(rect, width, height, rotation);
          return (
            <rect
              key={`hit-${index}`}
              x={x0}
              y={y0}
              width={Math.max(x1 - x0, 0.5)}
              height={Math.max(y1 - y0, 0.5)}
              fill="var(--accent-soft)"
              stroke="var(--accent)"
              strokeWidth={0.8}
              vectorEffect="non-scaling-stroke"
            />
          );
        })}
      </svg>
      <figcaption className="hint" style={{ marginTop: 4 }}>
        {t("pageMap.captionLead")} ·{" "}
        {t("pageMap.dimensions", {
          width: Math.round(width),
          height: Math.round(height),
          unit,
        })}
        {displayRotation ? ` · ${t("pageMap.rotated", { rotation: displayRotation })}` : ""} ·{" "}
        {t("pageMap.blocks", { count: drawnBlocks })}
      </figcaption>
    </figure>
  );
}

export { PageMap };
