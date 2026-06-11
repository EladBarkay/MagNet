import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MagnetEvent } from "../types";
import CanvasPresetForm from "./CanvasPresetForm";

type Props = {
  event: MagnetEvent;
  /** photoId -> quantity, from the gallery print queue. */
  printQueue: Record<string, number>;
  onClose: () => void;
  onEventUpdate: (e: MagnetEvent) => void;
  /** Called after a successful print so the caller can clear the queue. */
  onPrinted: () => void;
};

export default function PrintConfirmDialog({
  event, printQueue, onClose, onEventUpdate, onPrinted,
}: Props) {
  const [frameId, setFrameId] = useState<string>(
    event.active_frame_preset_id ?? event.frame_presets[0]?.id ?? ""
  );
  const [canvasId, setCanvasId] = useState<string>(event.canvas_presets[0]?.id ?? "");
  const [showNewPreset, setShowNewPreset] = useState(event.canvas_presets.length === 0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [sentCount, setSentCount] = useState<number | null>(null);

  const photoIds = Object.keys(printQueue).filter((id) => printQueue[id] > 0);
  const totalPrints = photoIds.reduce((s, id) => s + printQueue[id], 0);
  const canvasPreset = event.canvas_presets.find((p) => p.id === canvasId);
  const canvasCount = canvasPreset
    ? Math.ceil(totalPrints / canvasPreset.photos_per_canvas)
    : 0;

  async function startPrint() {
    if (photoIds.length === 0) { setError("No photos queued for printing"); return; }
    if (!frameId) { setError("Select a frame preset"); return; }
    if (!canvasId) { setError("Select a canvas preset"); return; }
    setError("");
    setBusy(true);
    try {
      const count = await invoke<number>("print_photos", {
        eventId: event.id,
        photoIds,
        quantities: printQueue,
        canvasPresetId: canvasId,
        framePresetId: frameId,
      });
      setSentCount(count);
      onPrinted();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  if (sentCount !== null) {
    return (
      <Modal onClose={onClose}>
        <div className="space-y-4 text-center py-2">
          <p className="text-2xl">🖨</p>
          <p className="font-medium text-neutral-100">
            Sent {sentCount} file{sentCount !== 1 ? "s" : ""} for printing
          </p>
          <button
            onClick={onClose}
            className="px-4 py-1.5 bg-blue-600 hover:bg-blue-500 rounded text-sm font-medium"
          >
            Done
          </button>
        </div>
      </Modal>
    );
  }

  return (
    <Modal onClose={onClose}>
      <div className="space-y-4">
        <h2 className="text-base font-semibold text-neutral-100">Print</h2>

        {/* Frame preset */}
        <Field label="Frame preset">
          {event.frame_presets.length === 0 ? (
            <p className="text-xs text-red-400">No frame presets — add one first.</p>
          ) : (
            <div className="flex flex-wrap gap-1.5">
              {event.frame_presets.map((p) => (
                <Chip
                  key={p.id}
                  label={p.name}
                  active={p.id === frameId}
                  onClick={() => setFrameId(p.id)}
                />
              ))}
            </div>
          )}
        </Field>

        {/* Canvas preset */}
        <Field label="Canvas preset">
          {event.canvas_presets.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {event.canvas_presets.map((p) => (
                <Chip
                  key={p.id}
                  label={p.name}
                  active={p.id === canvasId}
                  onClick={() => { setCanvasId(p.id); setShowNewPreset(false); }}
                />
              ))}
            </div>
          )}
          {showNewPreset ? (
            <CanvasPresetForm
              event={event}
              onCreated={(preset, updatedEvent) => {
                onEventUpdate(updatedEvent);
                setCanvasId(preset.id);
                setShowNewPreset(false);
              }}
              onCancel={() => setShowNewPreset(false)}
            />
          ) : (
            <button
              onClick={() => setShowNewPreset(true)}
              className="text-xs text-blue-400 hover:text-blue-300"
            >
              + New preset
            </button>
          )}
        </Field>

        {/* Summary */}
        {canvasPreset && totalPrints > 0 && (
          <p className="text-xs text-neutral-500">
            <strong className="text-neutral-300">{totalPrints} prints</strong> across{" "}
            <strong className="text-neutral-300">
              {canvasCount} canvas{canvasCount !== 1 ? "es" : ""}
            </strong>{" "}
            ({canvasPreset.photos_per_canvas}-up, {canvasPreset.canvas_width_px}×
            {canvasPreset.canvas_height_px}px)
          </p>
        )}

        {error && <p className="text-xs text-red-400">{error}</p>}

        <div className="flex justify-end gap-2 pt-1">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-sm text-neutral-400 hover:text-neutral-200"
          >
            Cancel
          </button>
          <button
            onClick={startPrint}
            disabled={busy || totalPrints === 0 || !frameId || !canvasId}
            className="px-4 py-1.5 text-sm bg-green-700 hover:bg-green-600 disabled:opacity-40 disabled:cursor-not-allowed rounded font-medium"
          >
            {busy ? "Composing…" : `Print (${totalPrints})`}
          </button>
        </div>
      </div>
    </Modal>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="space-y-1.5">
      <label className="text-xs font-medium text-neutral-400 uppercase tracking-wide">
        {label}
      </label>
      {children}
    </div>
  );
}

function Chip({
  label, active, onClick,
}: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={[
        "px-2.5 py-1 text-xs rounded transition-colors",
        active ? "bg-blue-600 text-white" : "bg-neutral-700 hover:bg-neutral-600 text-neutral-300",
      ].join(" ")}
    >
      {label}
    </button>
  );
}

function Modal({ children, onClose }: { children: React.ReactNode; onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />
      <div className="relative z-10 w-full max-w-md mx-4 bg-neutral-900 border border-neutral-700 rounded-xl shadow-2xl p-5">
        {children}
      </div>
    </div>
  );
}
