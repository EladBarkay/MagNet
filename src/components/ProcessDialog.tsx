import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { MagnetEvent } from "../types";
import { ExportProgress, useExportProgress } from "../hooks/useExportProgress";
import CanvasPresetForm from "./CanvasPresetForm";
import { Modal, Field, Chip, PresetOption } from "./ui";

type Destination = "print" | "export";

type Props = {
  event: MagnetEvent;
  photoQueue: Record<string, number>;
  onClose: () => void;
  onEventUpdate: (e: MagnetEvent) => void;
  onProcessed: (destination: Destination, quantities: Record<string, number>) => void;
};

export default function ProcessDialog({ event, photoQueue, onClose, onEventUpdate, onProcessed }: Props) {
  const [destination, setDestination] = useState<Destination>("print");
  const [frameId, setFrameId] = useState<string>(
    event.active_frame_preset_id ?? event.frame_presets[0]?.id ?? ""
  );
  const [canvasId, setCanvasId] = useState<string>(event.canvas_presets[0]?.id ?? "");
  const [showNewPreset, setShowNewPreset] = useState(event.canvas_presets.length === 0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [printResult, setPrintResult] = useState<number | null>(null);
  const { progress, result: exportResult, clear: clearExport } = useExportProgress();

  const totalQty = Object.values(photoQueue).reduce((s, q) => s + q, 0);
  const canvasPreset = event.canvas_presets.find((p) => p.id === canvasId);
  const canvasCount = canvasPreset && totalQty > 0
    ? Math.ceil(totalQty / canvasPreset.photos_per_canvas)
    : 0;

  // Convert string-keyed queue to object Tauri will serialize as map<uuid, u32>
  const quantities = Object.fromEntries(
    Object.entries(photoQueue).filter(([, q]) => q > 0)
  );

  async function pickOutputFolder() {
    const folder = await openDialog({ directory: true, multiple: false });
    if (!folder) return;
    await invoke("set_output_folder", { eventId: event.id, folder });
    onEventUpdate({ ...event, output_folder: folder as string });
  }

  async function go() {
    if (!frameId) { setError("Select a frame preset"); return; }
    if (!canvasId) { setError("Select a canvas preset"); return; }
    if (totalQty === 0) { setError("No photos queued"); return; }
    if (destination === "export" && !event.output_folder) {
      setError("Set an output folder first");
      return;
    }
    setError("");
    setBusy(true);

    try {
      if (destination === "print") {
        const count = await invoke<number>("print_photos", {
          eventId: event.id,
          quantities,
          framePresetId: frameId,
          canvasPresetId: canvasId,
        });
        setPrintResult(count);
        onProcessed("print", photoQueue);
      } else {
        clearExport();
        await invoke("export_batch", {
          eventId: event.id,
          quantities,
          framePresetId: frameId,
          canvasPresetId: canvasId,
        });
        // export-complete handled by useExportProgress
      }
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  // Export done
  if (exportResult) {
    return (
      <Modal onClose={() => { clearExport(); onProcessed("export", photoQueue); onClose(); }}>
        <div className="space-y-4">
          <div className="flex items-center gap-3">
            <span className="text-2xl">{exportResult.errors.length === 0 ? "✓" : "⚠"}</span>
            <div>
              <p className="font-medium text-neutral-100">
                {exportResult.errors.length === 0
                  ? "Export complete"
                  : `Export finished with ${exportResult.errors.length} error(s)`}
              </p>
              <p className="text-xs text-neutral-400 mt-0.5">
                {totalQty} photos → {exportResult.output_dir}
              </p>
            </div>
          </div>
          {exportResult.errors.length > 0 && (
            <ul className="text-xs text-red-400 space-y-0.5 max-h-32 overflow-y-auto">
              {exportResult.errors.map((e, i) => <li key={i}>{e}</li>)}
            </ul>
          )}
          <div className="flex justify-end gap-2">
            <button
              onClick={async () => {
                try {
                  const { openPath } = await import("@tauri-apps/plugin-opener");
                  await openPath(exportResult.output_dir);
                } catch {}
              }}
              className="px-3 py-1.5 text-sm bg-neutral-700 hover:bg-neutral-600 rounded"
            >
              Open folder
            </button>
            <button
              onClick={() => { clearExport(); onProcessed("export", photoQueue); onClose(); }}
              className="px-3 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 rounded font-medium"
            >
              Done
            </button>
          </div>
        </div>
      </Modal>
    );
  }

  // Print done
  if (printResult !== null) {
    return (
      <Modal onClose={onClose}>
        <div className="space-y-4 text-center py-2">
          <p className="text-2xl">🖨</p>
          <p className="font-medium text-neutral-100">
            Sent {printResult} file{printResult !== 1 ? "s" : ""} for printing
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

  // Export in progress
  if (busy && destination === "export" && progress) {
    return (
      <Modal onClose={() => {}}>
        <ExportProgressView progress={progress} />
      </Modal>
    );
  }

  return (
    <Modal onClose={onClose}>
      <div className="space-y-4">
        <h2 className="text-base font-semibold text-neutral-100">Process photos</h2>

        {/* Destination toggle */}
        <Field label="Send to">
          <div className="flex gap-1.5">
            <Chip label="Print" active={destination === "print"} onClick={() => setDestination("print")} />
            <Chip label="Export to folder" active={destination === "export"} onClick={() => setDestination("export")} />
          </div>
        </Field>

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
            <div className="space-y-1">
              {event.canvas_presets.map((p) => (
                <PresetOption
                  key={p.id}
                  preset={p}
                  selected={p.id === canvasId}
                  onSelect={() => { setCanvasId(p.id); setShowNewPreset(false); }}
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
              disabled={busy}
              className={`text-xs ${busy ? "text-neutral-600 cursor-not-allowed" : "text-blue-400 hover:text-blue-300"}`}
            >
              + New preset
            </button>
          )}
        </Field>

        {/* Output folder (export only) */}
        {destination === "export" && (
          <Field label="Output folder">
            <div className="flex items-center gap-2">
              {event.output_folder ? (
                <span className="flex-1 text-xs text-neutral-300 truncate bg-neutral-800 rounded px-2 py-1.5">
                  {event.output_folder}
                </span>
              ) : (
                <span className="flex-1 text-xs text-neutral-600 italic">Not set</span>
              )}
              <button
                onClick={pickOutputFolder}
                className="px-2 py-1.5 text-xs bg-neutral-700 hover:bg-neutral-600 rounded whitespace-nowrap"
              >
                {event.output_folder ? "Change" : "Set folder"}
              </button>
            </div>
          </Field>
        )}

        {/* Summary */}
        {canvasPreset && totalQty > 0 && (
          <p className="text-xs text-neutral-500">
            <strong className="text-neutral-300">{totalQty} photo{totalQty !== 1 ? "s" : ""}</strong>
            {" → "}
            <strong className="text-neutral-300">
              {canvasCount} canvas{canvasCount !== 1 ? "es" : ""}
            </strong>
            {" "}({canvasPreset.photos_per_canvas}-up, {canvasPreset.canvas_width_px}×{canvasPreset.canvas_height_px}px
            {destination === "export" ? `, ${canvasPreset.dpi} DPI` : ""})
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
            onClick={go}
            disabled={busy || totalQty === 0 || !frameId || !canvasId || (destination === "export" && !event.output_folder)}
            className="px-4 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-40 disabled:cursor-not-allowed rounded font-medium"
          >
            {busy ? (destination === "print" ? "Composing…" : "Starting…") : destination === "print" ? `Print (${totalQty})` : `Export (${canvasCount} canvas${canvasCount !== 1 ? "es" : ""})`}
          </button>
        </div>
      </div>
    </Modal>
  );
}

function ExportProgressView({ progress }: { progress: ExportProgress }) {
  const pct = progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0;
  return (
    <div className="space-y-4 py-2">
      <p className="text-sm font-medium text-neutral-200">Exporting…</p>
      <div className="space-y-2">
        <div className="flex justify-between text-xs text-neutral-400">
          <span className="truncate max-w-[70%]">{progress.current_file}</span>
          <span>{progress.done} / {progress.total}</span>
        </div>
        <div className="h-2 bg-neutral-700 rounded-full overflow-hidden">
          <div
            className="h-full bg-blue-500 transition-all duration-200"
            style={{ width: `${pct}%` }}
          />
        </div>
      </div>
    </div>
  );
}
