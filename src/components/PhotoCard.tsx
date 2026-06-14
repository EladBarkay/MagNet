import { memo } from "react";
import { useThumbnail } from "../hooks/useThumbnail";
import { Photo } from "../types";
import { basename } from "../lib/paths";
import { QtyButton } from "./ui";

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

      {/* Bottom overlay: filename + quantity stepper */}
      <div
        className={[
          "absolute bottom-0 inset-x-0 flex items-center justify-between px-1.5 py-1.5",
          "bg-gradient-to-t from-black/80 to-transparent",
          "transition-opacity",
          qty > 0 || selected ? "opacity-100" : "opacity-0 group-hover:opacity-100",
        ].join(" ")}
      >
        {cellSize >= 140 ? (
          <span className="text-[10px] text-neutral-300 truncate leading-tight max-w-[50%] pointer-events-none">
            {filename}
          </span>
        ) : (
          <span />
        )}
        <div className="flex items-center gap-1">
          <QtyButton label="−" onClick={() => onQtyDelta(-1)} disabled={qty <= 0} />
          <span className="min-w-[18px] text-center text-sm font-semibold text-white tabular-nums">
            {qty}
          </span>
          <QtyButton label="+" onClick={() => onQtyDelta(+1)} />
        </div>
      </div>
    </div>
  );
}

export default memo(PhotoCard);
