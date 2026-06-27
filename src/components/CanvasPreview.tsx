import { useRef, useState, useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Grid, type CellComponentProps } from "react-window";
import { OrenewEvent } from "../types";
import { useCanvasPage } from "../hooks/useCanvasPage";
import { Field, Chip, PresetOption } from "./ui";

const GAP = 16;
const TILE_W = 300;

type Props = {
  event: OrenewEvent;
  /** The shared copy queue (path → quantity), already scoped to the active folder
   *  / selection by App. Read-only here. */
  photoQueue: Record<string, number>;
  onAddFrame: () => void;
  onAddCanvas: () => void;
};

type CellData = {
  eventId: string;
  frameId: string;
  canvasId: string;
  quantities: Record<string, number>;
  sig: string;
  colCount: number;
  pageCount: number;
  tileW: number;
  tileH: number;
};

// Module-level so react-window re-renders cells without remounting them.
function Cell({
  columnIndex,
  rowIndex,
  style,
  eventId,
  frameId,
  canvasId,
  quantities,
  sig,
  colCount,
  pageCount,
  tileW,
  tileH,
}: CellComponentProps<CellData>) {
  const index = rowIndex * colCount + columnIndex;
  if (index >= pageCount) return <div style={style} />;
  return (
    <div
      style={{
        ...style,
        left: (style.left as number) + GAP,
        top: (style.top as number) + GAP,
        width: tileW,
        height: tileH,
      }}
    >
      <PageTile
        eventId={eventId}
        frameId={frameId}
        canvasId={canvasId}
        quantities={quantities}
        sig={sig}
        index={index}
      />
    </div>
  );
}

function PageTile({
  eventId,
  frameId,
  canvasId,
  quantities,
  sig,
  index,
}: {
  eventId: string;
  frameId: string;
  canvasId: string;
  quantities: Record<string, number>;
  sig: string;
  index: number;
}) {
  const src = useCanvasPage(eventId, frameId, canvasId, quantities, index, sig);
  return (
    <div className="relative w-full h-full shadow-lg ring-1 ring-neutral-700 bg-neutral-800 overflow-hidden rounded">
      {src ? (
        <img src={src} className="w-full h-full object-contain" draggable={false} alt="" />
      ) : (
        <div className="absolute inset-0 flex items-center justify-center text-neutral-600 text-xs animate-pulse">
          …
        </div>
      )}
      <span className="absolute top-1 start-1 px-1.5 py-0.5 rounded bg-black/60 text-[10px] text-neutral-200">
        {index + 1}
      </span>
    </div>
  );
}

/**
 * Read-only preview of the actual export canvases, shown as a thumbnail gallery.
 * Tiles the queued photos (copies-expanded, path order) onto the chosen canvas
 * preset exactly as export would — each page rendered in Rust at thumbnail
 * resolution. Page count is derived client-side (same chunk-by-photos_per_canvas
 * math the backend uses).
 */
export default function CanvasPreview({ event, photoQueue, onAddFrame, onAddCanvas }: Props) {
  const { t } = useTranslation();
  const [frameId, setFrameId] = useState(
    event.active_frame_preset_id ?? event.frame_presets[0]?.id ?? ""
  );
  const [canvasId, setCanvasId] = useState(
    event.active_canvas_preset_id ?? event.canvas_presets[0]?.id ?? ""
  );

  // Keep the selected preset valid as presets are added/removed (e.g. the user
  // just added the first one via the empty-state buttons).
  useEffect(() => {
    if (!event.frame_presets.some((p) => p.id === frameId))
      setFrameId(event.active_frame_preset_id ?? event.frame_presets[0]?.id ?? "");
  }, [event.frame_presets]);
  useEffect(() => {
    if (!event.canvas_presets.some((p) => p.id === canvasId))
      setCanvasId(event.active_canvas_preset_id ?? event.canvas_presets[0]?.id ?? "");
  }, [event.canvas_presets]);

  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const { width, height } = entries[0].contentRect;
      setSize({ width, height });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Filtered queue + a cheap signature so pages re-render on copy changes.
  const quantities = useMemo(
    () => Object.fromEntries(Object.entries(photoQueue).filter(([, q]) => q > 0)),
    [photoQueue]
  );
  const sig = useMemo(
    () =>
      Object.entries(quantities)
        .map(([k, v]) => `${k}:${v}`)
        .sort()
        .join("|"),
    [quantities]
  );

  const canvasPreset = event.canvas_presets.find((p) => p.id === canvasId);
  const totalQty = Object.values(quantities).reduce((s, q) => s + q, 0);
  const pageCount =
    canvasPreset && totalQty > 0
      ? Math.ceil(totalQty / Math.max(1, canvasPreset.photos_per_canvas))
      : 0;

  const aspect =
    canvasPreset && canvasPreset.canvas_height_px > 0
      ? canvasPreset.canvas_width_px / canvasPreset.canvas_height_px
      : 1.5;
  const tileW = TILE_W;
  const tileH = Math.round(tileW / aspect);
  const colStride = tileW + GAP;
  const rowStride = tileH + GAP;
  const colCount = Math.max(1, Math.floor((size.width - GAP) / colStride));
  const rowCount = Math.ceil(pageCount / colCount);

  const cellData: CellData = {
    eventId: event.id,
    frameId,
    canvasId,
    quantities,
    sig,
    colCount,
    pageCount,
    tileW,
    tileH,
  };

  const missingPresets =
    event.frame_presets.length === 0 || event.canvas_presets.length === 0;
  const ready = frameId && canvasId && pageCount > 0;

  return (
    <div className="flex flex-col flex-1 overflow-hidden">
      {/* Preset pickers */}
      <div className="flex flex-wrap items-end gap-4 px-3 py-2 bg-neutral-900 border-b border-neutral-800 shrink-0">
        <Field label={t("export.framePreset")}>
          <div className="flex flex-wrap gap-1.5">
            {event.frame_presets.map((p) => (
              <Chip key={p.id} label={p.name} active={p.id === frameId} onClick={() => setFrameId(p.id)} />
            ))}
          </div>
        </Field>
        <Field label={t("export.canvasPreset")}>
          <div className="flex flex-wrap gap-1.5">
            {event.canvas_presets.map((p) => (
              <div key={p.id} className="min-w-44">
                <PresetOption preset={p} selected={p.id === canvasId} onSelect={() => setCanvasId(p.id)} />
              </div>
            ))}
          </div>
        </Field>
        {pageCount > 0 && (
          <span className="ms-auto text-xs text-neutral-500 pb-1">
            {t("canvasView.pageCount", { count: pageCount })}
          </span>
        )}
      </div>

      {/* Pages */}
      <div ref={containerRef} className="flex-1 overflow-hidden bg-neutral-950">
        {missingPresets ? (
          <div className="h-full flex flex-col items-center justify-center gap-4 px-6 text-center">
            <p className="text-neutral-500 text-sm">{t("canvasView.needPresets")}</p>
            <div className="flex gap-3">
              {event.frame_presets.length === 0 && (
                <button
                  type="button"
                  onClick={onAddFrame}
                  className="px-4 py-1.5 text-sm bg-accent hover:bg-accent-hover rounded font-medium text-accent-fg"
                >
                  {t("canvasView.addFramePreset")}
                </button>
              )}
              {event.canvas_presets.length === 0 && (
                <button
                  type="button"
                  onClick={onAddCanvas}
                  className="px-4 py-1.5 text-sm bg-accent hover:bg-accent-hover rounded font-medium text-accent-fg"
                >
                  {t("canvasView.addCanvasPreset")}
                </button>
              )}
            </div>
          </div>
        ) : !ready ? (
          <div className="h-full flex items-center justify-center text-neutral-600 text-sm px-6 text-center">
            {t("canvasView.queueToPreview")}
          </div>
        ) : (
          size.width > 0 && (
            <Grid<CellData>
              cellComponent={Cell}
              cellProps={cellData}
              columnCount={colCount}
              columnWidth={colStride}
              rowCount={rowCount}
              rowHeight={rowStride}
              overscanCount={2}
              style={{ width: size.width, height: size.height, overflowX: "hidden", paddingTop: GAP / 2 }}
            />
          )
        )}
      </div>
    </div>
  );
}
