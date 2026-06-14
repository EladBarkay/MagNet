import { Entitlement, MagnetEvent, PhotoBatch } from "../types";
import { PrintIcon, SettingsIcon, TrashIcon } from "./icons";
import { tierLabel, tierColor } from "../lib/tiers";
import { QtyButton } from "./ui";

type Props = {
  event: MagnetEvent | null;
  entitlement: Entitlement | null;
  status: string;
  totalPhotos: number;
  activeBatch: PhotoBatch | null;
  queuedTotal: number;
  allQty: number;
  cellSize: number;
  onOpenEvent: () => void;
  onDeleteEvent: () => void;
  onProcess: () => void;
  onSettings: () => void;
  onSetAllQty: (qty: number) => void;
  onCellSizeChange: (size: number) => void;
};

export default function Toolbar({
  event, entitlement, status, totalPhotos, activeBatch, queuedTotal, allQty, cellSize,
  onOpenEvent, onDeleteEvent, onProcess, onSettings, onSetAllQty, onCellSizeChange,
}: Props) {
  const tier = entitlement?.tier ?? "free";

  function changeAllQty(delta: number) {
    onSetAllQty(Math.max(0, allQty + delta));
  }

  return (
    <header className="flex items-center gap-3 px-4 py-2.5 bg-neutral-800 border-b border-neutral-700 shrink-0">
      <span className="font-bold text-base tracking-tight text-white">MagNet</span>
      <div className="w-px h-4 bg-neutral-600" />
      <button onClick={onOpenEvent}
        className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 active:bg-blue-700 rounded text-sm font-medium transition-colors">
        Open Event
      </button>

      {event && (
        <>
          <span className="text-neutral-300 text-sm font-medium">{event.name}</span>
          <span className="text-neutral-500 text-xs">{totalPhotos} photos</span>

          <button
            onClick={onDeleteEvent}
            title="Delete this event"
            className="text-neutral-600 hover:text-red-400 transition-colors"
          >
            <TrashIcon />
          </button>

          <div className="ml-auto flex items-center gap-3">
            {/* Gallery cell size */}
            <div className="flex items-center gap-1">
              <span className="text-xs text-neutral-500">Size:</span>
              <QtyButton size="sm" label="−" onClick={() => onCellSizeChange(Math.max(100, cellSize - 20))} disabled={cellSize <= 100} />
              <QtyButton size="sm" label="+" onClick={() => onCellSizeChange(Math.min(280, cellSize + 20))} disabled={cellSize >= 280} />
            </div>

            {/* Batch-wide print qty: set all photos in the current batch at once */}
            {activeBatch && activeBatch.photos.length > 0 && (
              <div className="flex items-center gap-1.5">
                <span className="text-xs text-neutral-500">All:</span>
                <div className="flex items-center gap-0.5 rounded-full bg-neutral-700 px-0.5 py-0.5">
                  <QtyButton size="sm" label="−" onClick={() => changeAllQty(-1)} disabled={allQty <= 0} />
                  <span className="min-w-[18px] text-center text-xs font-semibold text-neutral-200 tabular-nums">
                    {allQty}
                  </span>
                  <QtyButton size="sm" label="+" onClick={() => changeAllQty(+1)} />
                </div>
              </div>
            )}

            <button
              onClick={onProcess}
              disabled={queuedTotal === 0}
              className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 disabled:opacity-40 disabled:cursor-not-allowed rounded text-sm font-medium transition-colors"
              title={queuedTotal === 0 ? "Set quantities on gallery photos first" : ""}
            >
              <PrintIcon />
              Process{queuedTotal > 0 ? ` (${queuedTotal})` : ""}
            </button>
          </div>
        </>
      )}

      {status && (
        <span className={["text-xs", event ? "" : "ml-auto", "text-neutral-400"].join(" ")}>
          {status}
        </span>
      )}

      <button
        onClick={onSettings}
        title="Settings & license"
        className={[
          "flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs transition-colors",
          event ? "" : "ml-auto",
          "text-neutral-400 hover:text-neutral-100 hover:bg-neutral-700",
        ].join(" ")}
      >
        <SettingsIcon />
        <span className={`font-semibold px-1.5 py-0.5 rounded-full text-[10px] ${tierColor(tier)}`}>
          {tierLabel(tier)}
        </span>
      </button>
    </header>
  );
}
