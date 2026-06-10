import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MagnetEvent, Photo, PhotoBatch } from "../types";
import { useThumbnail } from "../hooks/useThumbnail";
import CanvasPresetForm from "./CanvasPresetForm";

type Props = {
  event: MagnetEvent;
  batch: PhotoBatch;
  onClose: () => void;
  onEventUpdate: (e: MagnetEvent) => void;
  /** Pre-selected photo (from gallery selection) */
  initialPhotoId?: string;
};

type Quantities = Record<string, number>;

export default function PrintDialog({
  event, batch, onClose, onEventUpdate, initialPhotoId,
}: Props) {
  const [quantities, setQuantities] = useState<Quantities>(() =>
    Object.fromEntries(batch.photos.map((p) => [p.id, p.id === initialPhotoId ? 1 : 1]))
  );
  const [selectedPresetId, setSelectedPresetId] = useState<string>(
    event.canvas_presets[0]?.id ?? ""
  );
  const [showNewPreset, setShowNewPreset] = useState(event.canvas_presets.length === 0);
  const [printing, setPrinting] = useState(false);
  const [error, setError] = useState("");
  const [done, setDone] = useState(false);

  const selectedPreset = event.canvas_presets.find((p) => p.id === selectedPresetId);
  const totalPrints = Object.values(quantities).reduce((s, q) => s + q, 0);
  const canvasCount = selectedPreset
    ? Math.ceil(totalPrints / selectedPreset.photos_per_canvas)
    : 0;

  function setQty(photoId: string, delta: number) {
    setQuantities((prev) => ({
      ...prev,
      [photoId]: Math.max(0, (prev[photoId] ?? 1) + delta),
    }));
  }

  function setAll(qty: number) {
    setQuantities(Object.fromEntries(batch.photos.map((p) => [p.id, qty])));
  }

  async function startPrint() {
    const activeIds = batch.photos.filter((p) => (quantities[p.id] ?? 0) > 0).map((p) => p.id);
    if (activeIds.length === 0) { setError("No photos selected (all quantities are 0)"); return; }
    if (!selectedPresetId) { setError("Select a canvas preset"); return; }
    if (!event.active_frame_preset_id) { setError("No frame preset active"); return; }
    setError("");
    setPrinting(true);
    try {
      const qtMap: Record<string, number> = {};
      activeIds.forEach((id) => { qtMap[id] = quantities[id]; });
      await invoke("print_photos", {
        eventId: event.id,
        photoIds: activeIds,
        quantities: qtMap,
        canvasPresetId: selectedPresetId,
      });
      setDone(true);
    } catch (e) {
      setError(String(e));
      setPrinting(false);
    }
  }

  if (done) {
    return (
      <Modal onClose={onClose}>
        <div className="space-y-4 text-center py-2">
          <p className="text-2xl">🖨</p>
          <p className="font-medium text-neutral-100">
            {canvasCount} canvas{canvasCount !== 1 ? "es" : ""} opened for printing
          </p>
          <p className="text-xs text-neutral-400">
            Your default photo app has opened the files. Print from there.
          </p>
          <button onClick={onClose}
            className="px-4 py-1.5 bg-blue-600 hover:bg-blue-500 rounded text-sm font-medium">
            Done
          </button>
        </div>
      </Modal>
    );
  }

  return (
    <Modal onClose={onClose}>
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-base font-semibold text-neutral-100">Print photos</h2>
          <div className="flex gap-2">
            <button onClick={() => setAll(1)} className="text-xs text-neutral-500 hover:text-neutral-300">
              Set all ×1
            </button>
            <button onClick={() => setAll(0)} className="text-xs text-neutral-500 hover:text-neutral-300">
              Clear all
            </button>
          </div>
        </div>

        {/* Photo list */}
        <div className="max-h-64 overflow-y-auto space-y-1 pr-1">
          {batch.photos.map((photo) => (
            <PhotoRow
              key={photo.id}
              photo={photo}
              qty={quantities[photo.id] ?? 1}
              onDelta={(d) => setQty(photo.id, d)}
              onSet={(v) => setQuantities((prev) => ({ ...prev, [photo.id]: v }))}
            />
          ))}
        </div>

        {/* Canvas preset */}
        <div className="space-y-2">
          <label className="text-xs font-medium text-neutral-400 uppercase tracking-wide">
            Canvas preset
          </label>
          {event.canvas_presets.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {event.canvas_presets.map((p) => (
                <button
                  key={p.id}
                  onClick={() => { setSelectedPresetId(p.id); setShowNewPreset(false); }}
                  className={[
                    "px-2.5 py-1 text-xs rounded transition-colors",
                    p.id === selectedPresetId
                      ? "bg-blue-600 text-white"
                      : "bg-neutral-700 hover:bg-neutral-600 text-neutral-300",
                  ].join(" ")}
                >
                  {p.name}
                </button>
              ))}
            </div>
          )}
          {showNewPreset ? (
            <CanvasPresetForm
              event={event}
              onCreated={(preset, updatedEvent) => {
                onEventUpdate(updatedEvent);
                setSelectedPresetId(preset.id);
                setShowNewPreset(false);
              }}
              onCancel={() => setShowNewPreset(false)}
            />
          ) : (
            <button onClick={() => setShowNewPreset(true)}
              className="text-xs text-blue-400 hover:text-blue-300">
              + New preset
            </button>
          )}
        </div>

        {/* Summary */}
        {selectedPreset && totalPrints > 0 && (
          <p className="text-xs text-neutral-500">
            <strong className="text-neutral-300">{totalPrints} prints</strong> across{" "}
            <strong className="text-neutral-300">{canvasCount} canvas{canvasCount !== 1 ? "es" : ""}</strong>
            {" "}({selectedPreset.photos_per_canvas}-up, {selectedPreset.canvas_width_px}×{selectedPreset.canvas_height_px}px)
          </p>
        )}

        {error && <p className="text-xs text-red-400">{error}</p>}

        <div className="flex justify-end gap-2 pt-1">
          <button onClick={onClose}
            className="px-3 py-1.5 text-sm text-neutral-400 hover:text-neutral-200">
            Cancel
          </button>
          <button
            onClick={startPrint}
            disabled={printing || totalPrints === 0 || !selectedPresetId}
            className="px-4 py-1.5 text-sm bg-green-700 hover:bg-green-600 disabled:opacity-40 disabled:cursor-not-allowed rounded font-medium"
          >
            {printing ? "Composing…" : `Print ${totalPrints > 0 ? `(×${totalPrints})` : ""}`}
          </button>
        </div>
      </div>
    </Modal>
  );
}

function PhotoRow({
  photo, qty, onDelta, onSet,
}: {
  photo: Photo; qty: number; onDelta: (d: number) => void; onSet: (v: number) => void;
}) {
  const src = useThumbnail(photo.path);
  const filename = photo.path.split(/[\\/]/).pop() ?? photo.path;

  return (
    <div className={[
      "flex items-center gap-2 px-2 py-1.5 rounded",
      qty === 0 ? "opacity-40" : "bg-neutral-800",
    ].join(" ")}>
      {/* Thumbnail */}
      <div className="w-10 h-10 rounded overflow-hidden shrink-0 bg-neutral-700">
        {src && <img src={src} alt="" className="w-full h-full object-cover" draggable={false} />}
      </div>

      {/* Name + previous prints */}
      <div className="flex-1 min-w-0">
        <p className="text-xs text-neutral-200 truncate">{filename}</p>
        {photo.print_count > 0 && (
          <p className="text-[10px] text-neutral-500">Printed ×{photo.print_count}</p>
        )}
      </div>

      {/* Quantity stepper */}
      <div className="flex items-center gap-1 shrink-0">
        <StepBtn onClick={() => onDelta(-1)} label="−" />
        <input
          type="number"
          value={qty}
          min={0}
          onChange={(e) => onSet(Math.max(0, Number(e.target.value)))}
          className="w-9 text-center text-sm bg-neutral-700 rounded py-0.5 focus:outline-none focus:ring-1 focus:ring-blue-500 text-neutral-100"
        />
        <StepBtn onClick={() => onDelta(+1)} label="+" />
      </div>
    </div>
  );
}

function StepBtn({ onClick, label }: { onClick: () => void; label: string }) {
  return (
    <button
      onClick={onClick}
      className="w-6 h-6 flex items-center justify-center text-sm bg-neutral-700 hover:bg-neutral-600 rounded font-medium leading-none"
    >
      {label}
    </button>
  );
}

function Modal({ children, onClose }: { children: React.ReactNode; onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />
      <div className="relative z-10 w-full max-w-lg mx-4 bg-neutral-900 border border-neutral-700 rounded-xl shadow-2xl p-5">
        {children}
      </div>
    </div>
  );
}
