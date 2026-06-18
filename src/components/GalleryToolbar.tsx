import { QtyButton } from "./ui";

type Props = {
  selectedCount: number;
  allQty: number;
  hideEmpty: boolean;
  scanning: boolean;
  onSetAllQty: (qty: number) => void;
  onScanFaces: () => void;
  onToggleHideEmpty: () => void;
};

/**
 * Contextual sub-bar above the gallery. Holds the per-batch controls that act on
 * the current selection (or the whole batch) — kept out of the app-level top
 * toolbar so each bar reads as one coherent unit.
 */
export default function GalleryToolbar({
  selectedCount, allQty, hideEmpty, scanning,
  onSetAllQty, onScanFaces, onToggleHideEmpty,
}: Props) {
  return (
    <div className="flex items-center gap-4 px-4 py-1.5 bg-neutral-800 border-b border-neutral-700 shrink-0">
      {/* Copies for the selection (or whole batch when none selected) */}
      <div className="flex items-center gap-1.5">
        <span className="text-xs text-neutral-500">
          {selectedCount > 0 ? `Selected (${selectedCount})` : "All photos"}
        </span>
        <div className="flex items-center gap-0.5 rounded-full bg-neutral-700 px-0.5 py-0.5">
          <QtyButton size="sm" label="−" onClick={() => onSetAllQty(Math.max(0, allQty - 1))} disabled={allQty <= 0} />
          <span className="min-w-[18px] text-center text-xs font-semibold text-neutral-200 tabular-nums">
            {allQty}
          </span>
          <QtyButton size="sm" label="+" onClick={() => onSetAllQty(allQty + 1)} />
        </div>
        <span className="text-xs text-neutral-600">copies</span>
      </div>

      <div className="w-px h-4 bg-neutral-700" />

      <button
        onClick={onScanFaces}
        disabled={scanning}
        title="Set each photo's copies to the number of faces detected"
        className="px-2.5 py-1 bg-neutral-700 hover:bg-neutral-600 disabled:opacity-40 disabled:cursor-wait rounded text-xs font-medium transition-colors"
      >
        {scanning ? "Scanning…" : "Suggest copies (faces)"}
      </button>

      <label className="flex items-center gap-1.5 text-xs text-neutral-400 cursor-pointer select-none">
        <input
          type="checkbox"
          checked={hideEmpty}
          onChange={onToggleHideEmpty}
          className="accent-blue-500"
        />
        Hide empty
      </label>
    </div>
  );
}
