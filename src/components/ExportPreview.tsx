import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { OrenewEvent } from "../types";
import { useCanvasPage } from "../hooks/useCanvasPage";

type Props = {
  event: OrenewEvent;
  frameId: string;
  canvasId: string;
  /** Copy queue (path → qty), already scoped by App. */
  quantities: Record<string, number>;
};

const MIN_SIZE = 200;
const MAX_SIZE = 560;
const STEP = 60;

/**
 * Live print preview shown inside the Export dialog: the actual export canvases
 * (frame + tiled photos) the current settings would produce, rendered page-by-page
 * in Rust at thumbnail resolution via `get_canvas_preview_page`. Tile size is
 * adjustable via the −/+ buttons or Ctrl+mouse-wheel over the preview.
 *
 * // ponytail: not virtualized — page count is bounded by the user's queue; add
 * // react-window only if a huge queue ever makes this sluggish.
 */
export default function ExportPreview({ event, frameId, canvasId, quantities }: Props) {
  const { t } = useTranslation();
  const [size, setSize] = useState(260);
  const scrollRef = useRef<HTMLDivElement>(null);

  const bump = (dir: number) =>
    setSize((s) => Math.min(MAX_SIZE, Math.max(MIN_SIZE, s + dir * STEP)));

  // Ctrl/Cmd + wheel zooms the preview. stopPropagation keeps the gallery's global
  // Ctrl+wheel zoom (in App) from also firing while the dialog is open.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      e.preventDefault();
      e.stopPropagation();
      bump(e.deltaY < 0 ? 1 : -1);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  const filtered = useMemo(
    () => Object.fromEntries(Object.entries(quantities).filter(([, q]) => q > 0)),
    [quantities]
  );
  const sig = useMemo(
    () => Object.entries(filtered).map(([k, v]) => `${k}:${v}`).sort().join("|"),
    [filtered]
  );

  const canvasPreset = event.canvas_presets.find((p) => p.id === canvasId);
  const totalQty = Object.values(filtered).reduce((s, q) => s + q, 0);
  const pageCount =
    canvasPreset && totalQty > 0
      ? Math.ceil(totalQty / Math.max(1, canvasPreset.photos_per_canvas))
      : 0;
  const aspect =
    canvasPreset && canvasPreset.canvas_height_px > 0
      ? canvasPreset.canvas_width_px / canvasPreset.canvas_height_px
      : 1.5;

  let body: React.ReactNode;
  if (!frameId || !canvasId) {
    body = <Hint text={t("export.previewPickPresets")} />;
  } else if (pageCount === 0) {
    body = <Hint text={t("canvasView.queueToPreview")} />;
  } else {
    body = (
      <div
        className="grid gap-3 p-1"
        style={{ gridTemplateColumns: `repeat(auto-fill, minmax(${size}px, 1fr))` }}
      >
        {Array.from({ length: pageCount }, (_, i) => (
          <PageTile
            key={i}
            eventId={event.id}
            frameId={frameId}
            canvasId={canvasId}
            quantities={filtered}
            sig={sig}
            index={i}
            aspect={aspect}
          />
        ))}
      </div>
    );
  }

  // Panel widens with the tile size so a single bigger tile still fits.
  const panelW = Math.min(MAX_SIZE + 32, size + 32);

  return (
    <div className="shrink-0 flex flex-col min-h-0" style={{ width: panelW }}>
      <div className="flex items-center justify-between mb-1.5">
        <span className="text-xs font-medium text-neutral-400">{t("export.preview")}</span>
        <div className="flex items-center gap-2">
          {pageCount > 0 && (
            <span className="text-xs text-neutral-500">{t("canvasView.pageCount", { count: pageCount })}</span>
          )}
          <div className="flex items-center gap-0.5 rounded-full bg-neutral-800 p-0.5">
            <ZoomBtn label="−" onClick={() => bump(-1)} disabled={size <= MIN_SIZE} title={t("view.zoomOut")} />
            <ZoomBtn label="+" onClick={() => bump(1)} disabled={size >= MAX_SIZE} title={t("view.zoomIn")} />
          </div>
        </div>
      </div>
      <div ref={scrollRef} className="flex-1 min-h-0 overflow-y-auto rounded-lg bg-neutral-950 ring-1 ring-neutral-800 p-2">
        {body}
      </div>
    </div>
  );
}

function ZoomBtn({ label, onClick, disabled, title }: {
  label: string; onClick: () => void; disabled?: boolean; title: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={title}
      className="flex items-center justify-center w-5 h-5 rounded-full text-sm font-medium text-neutral-300 hover:bg-white/10 disabled:opacity-30 leading-none"
    >
      {label}
    </button>
  );
}

function Hint({ text }: { text: string }) {
  return (
    <div className="h-full flex items-center justify-center text-center text-xs text-neutral-600 px-4">
      {text}
    </div>
  );
}

function PageTile({
  eventId, frameId, canvasId, quantities, sig, index, aspect,
}: {
  eventId: string;
  frameId: string;
  canvasId: string;
  quantities: Record<string, number>;
  sig: string;
  index: number;
  aspect: number;
}) {
  const src = useCanvasPage(eventId, frameId, canvasId, quantities, index, sig);
  return (
    <div
      className="relative w-full overflow-hidden rounded ring-1 ring-neutral-700 bg-neutral-800"
      style={{ aspectRatio: String(aspect) }}
    >
      {src ? (
        <img src={src} className="w-full h-full object-contain" draggable={false} alt="" />
      ) : (
        <div className="absolute inset-0 flex items-center justify-center text-neutral-600 text-xs animate-pulse">…</div>
      )}
      <span className="absolute top-1 start-1 px-1.5 py-0.5 rounded bg-black/60 text-[10px] text-neutral-200">
        {index + 1}
      </span>
    </div>
  );
}
