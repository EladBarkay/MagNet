import { memo } from "react";
import { useThumbnail } from "../hooks/useThumbnail";
import { Photo } from "../types";
import { basename } from "../lib/paths";

type Props = {
  photo: Photo;
  selected: boolean;
  onClick: () => void;
  cellSize: number;
  /** Number queued for the next process run. */
  qty: number;
  /** Adjust the queued quantity by `delta` (clamped at 0). */
  onQtyDelta: (delta: number) => void;
};

function PhotoCard({ photo, selected, onClick, cellSize, qty, onQtyDelta }: Props) {
  const src = useThumbnail(photo.path, photo.content_hash);
  const filename = basename(photo.path);

  return (
    <div
      className={[
        "relative w-full h-full rounded overflow-hidden group",
        "transition-all duration-100",
        selected
          ? "ring-2 ring-blue-400 ring-offset-1 ring-offset-neutral-900"
          : "hover:ring-1 hover:ring-neutral-500 hover:ring-offset-1 hover:ring-offset-neutral-900",
      ].join(" ")}
      title={filename}
    >
      {/* Thumbnail (click selects) */}
      <button onClick={onClick} className="block w-full h-full focus:outline-none">
        {src ? (
          <img
            src={src}
            alt={filename}
            className="w-full h-full object-cover"
            draggable={false}
          />
        ) : (
          <div className="w-full h-full bg-neutral-800 animate-pulse" />
        )}
      </button>

      {/* Bottom overlay: filename */}
      <div
        className={[
          "absolute bottom-0 inset-x-0 flex items-end justify-between px-1.5 py-1 pointer-events-none",
          "bg-gradient-to-t from-black/70 to-transparent",
          "opacity-0 group-hover:opacity-100 transition-opacity",
          selected ? "opacity-100" : "",
        ].join(" ")}
      >
        {cellSize >= 140 && (
          <span className="text-[10px] text-neutral-300 truncate leading-tight max-w-[80%]">
            {filename}
          </span>
        )}
      </div>

      {/* Historical badges (top-right) */}
      <div className="absolute top-1 right-1 flex flex-col items-end gap-1 pointer-events-none">
        {photo.export_count > 0 && <ExportBadge count={photo.export_count} />}
        {photo.print_count > 0 && <PrintBadge count={photo.print_count} />}
      </div>

      {/* Quantity stepper (top-left). Always visible if queued, else on hover. */}
      <div
        className={[
          "absolute top-1 left-1 flex items-center gap-0.5 rounded-full bg-black/70 px-0.5 py-0.5",
          "transition-opacity",
          qty > 0 ? "opacity-100" : "opacity-0 group-hover:opacity-100",
        ].join(" ")}
      >
        <QtyBtn label="−" onClick={() => onQtyDelta(-1)} disabled={qty <= 0} />
        <span className="min-w-[14px] text-center text-[11px] font-semibold text-white tabular-nums">
          {qty}
        </span>
        <QtyBtn label="+" onClick={() => onQtyDelta(+1)} />
      </div>
    </div>
  );
}

function QtyBtn({
  label, onClick, disabled,
}: { label: string; onClick: () => void; disabled?: boolean }) {
  return (
    <button
      onClick={(e) => { e.stopPropagation(); onClick(); }}
      disabled={disabled}
      className="w-5 h-5 flex items-center justify-center rounded-full bg-white/15 hover:bg-white/30 disabled:opacity-30 text-white text-sm leading-none font-medium"
    >
      {label}
    </button>
  );
}

function PrintBadge({ count }: { count: number }) {
  return (
    <span className="inline-flex items-center gap-0.5 bg-green-700/90 text-white text-[10px] font-semibold px-1.5 py-0.5 rounded-full leading-none">
      ×{count}
    </span>
  );
}

function ExportBadge({ count }: { count: number }) {
  return (
    <span className="inline-flex items-center gap-0.5 bg-blue-700/90 text-white text-[10px] font-semibold px-1.5 py-0.5 rounded-full leading-none">
      ⬇{count}
    </span>
  );
}

export default memo(PhotoCard);
