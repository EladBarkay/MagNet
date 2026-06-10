import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { CanvasPreset, MagnetEvent, PhotoBatch } from "../types";
import { ExportProgress, useExportProgress } from "../hooks/useExportProgress";
import CanvasPresetForm from "./CanvasPresetForm";

type Props = {
  event: MagnetEvent;
  batch: PhotoBatch;
  onClose: () => void;
  onEventUpdate: (e: MagnetEvent) => void;
};

export default function ExportDialog({ event, batch, onClose, onEventUpdate }: Props) {
  const [selectedPresetId, setSelectedPresetId] = useState<string>(
    event.canvas_presets[0]?.id ?? ""
  );
  const [showNewPreset, setShowNewPreset] = useState(event.canvas_presets.length === 0);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState("");
  const { progress, result, clear } = useExportProgress();

  const selectedPreset = event.canvas_presets.find((p) => p.id === selectedPresetId);
  const canvasCount = selectedPreset
    ? Math.ceil(batch.photos.length / selectedPreset.photos_per_canvas)
    : 0;

  async function pickOutputFolder() {
    const folder = await openDialog({ directory: true, multiple: false });
    if (!folder) return;
    await invoke("set_output_folder", { eventId: event.id, folder });
    onEventUpdate({ ...event, output_folder: folder as string });
  }

  async function startExport() {
    if (!selectedPresetId) { setError("Select a canvas preset"); return; }
    if (!event.output_folder) { setError("Set an output folder first"); return; }
    if (!event.active_frame_preset_id) { setError("No frame preset active — set one in the sidebar"); return; }
    setError("");
    setExporting(true);
    clear();
    try {
      await invoke("export_batch", {
        eventId: event.id,
        batchId: batch.id,
        canvasPresetId: selectedPresetId,
      });
      // export-complete event is handled by useExportProgress
    } catch (e) {
      setError(String(e));
      setExporting(false);
    }
  }

  // Completed state
  if (result) {
    return (
      <Modal onClose={() => { clear(); onClose(); }}>
        <div className="space-y-4">
          <div className="flex items-center gap-3">
            <span className="text-2xl">{result.errors.length === 0 ? "✓" : "⚠"}</span>
            <div>
              <p className="font-medium text-neutral-100">
                {result.errors.length === 0 ? "Export complete" : `Export finished with ${result.errors.length} error(s)`}
              </p>
              <p className="text-xs text-neutral-400 mt-0.5 break-all">{result.output_dir}</p>
            </div>
          </div>
          {result.errors.length > 0 && (
            <ul className="text-xs text-red-400 space-y-0.5 max-h-32 overflow-y-auto">
              {result.errors.map((e, i) => <li key={i}>{e}</li>)}
            </ul>
          )}
          <div className="flex justify-end gap-2">
            <button
              onClick={() => invoke("open_path", { path: result.output_dir }).catch(() => {})}
              className="px-3 py-1.5 text-sm bg-neutral-700 hover:bg-neutral-600 rounded"
            >
              Open folder
            </button>
            <button onClick={() => { clear(); onClose(); }}
              className="px-3 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 rounded font-medium">
              Done
            </button>
          </div>
        </div>
      </Modal>
    );
  }

  // Exporting state
  if (exporting && progress) {
    return (
      <Modal onClose={() => {}}>
        <ExportProgressView progress={progress} />
      </Modal>
    );
  }

  // Config state
  return (
    <Modal onClose={onClose}>
      <div className="space-y-4">
        <h2 className="text-base font-semibold text-neutral-100">Export batch</h2>

        {/* Batch info */}
        <div className="text-sm text-neutral-400">
          <span className="font-medium text-neutral-200">{batch.name}</span>
          {" · "}{batch.photos.length} photos
        </div>

        {/* Canvas preset */}
        <div className="space-y-2">
          <label className="text-xs font-medium text-neutral-400 uppercase tracking-wide">
            Canvas preset
          </label>
          {event.canvas_presets.length > 0 && (
            <div className="space-y-1">
              {event.canvas_presets.map((p) => (
                <PresetOption
                  key={p.id}
                  preset={p}
                  selected={p.id === selectedPresetId}
                  onSelect={() => { setSelectedPresetId(p.id); setShowNewPreset(false); }}
                />
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
            <button
              onClick={() => setShowNewPreset(true)}
              className="text-xs text-blue-400 hover:text-blue-300"
            >
              + New preset
            </button>
          )}
        </div>

        {/* Output folder */}
        <div className="space-y-1">
          <label className="text-xs font-medium text-neutral-400 uppercase tracking-wide">
            Output folder
          </label>
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
        </div>

        {/* Summary */}
        {selectedPreset && (
          <p className="text-xs text-neutral-500">
            Will produce <strong className="text-neutral-300">{canvasCount} canvas{canvasCount !== 1 ? "es" : ""}</strong>
            {" "}({selectedPreset.canvas_width_px}×{selectedPreset.canvas_height_px}px,{" "}
            {selectedPreset.photos_per_canvas}-up, {selectedPreset.dpi} DPI)
          </p>
        )}

        {error && <p className="text-xs text-red-400">{error}</p>}

        <div className="flex justify-end gap-2 pt-1">
          <button onClick={onClose}
            className="px-3 py-1.5 text-sm text-neutral-400 hover:text-neutral-200">
            Cancel
          </button>
          <button
            onClick={startExport}
            disabled={!selectedPresetId || !event.output_folder || exporting}
            className="px-4 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-40 disabled:cursor-not-allowed rounded font-medium"
          >
            Export {canvasCount > 0 ? `${canvasCount} canvas${canvasCount !== 1 ? "es" : ""}` : ""}
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

function PresetOption({ preset, selected, onSelect }: {
  preset: CanvasPreset; selected: boolean; onSelect: () => void;
}) {
  return (
    <button
      onClick={onSelect}
      className={[
        "w-full text-left px-3 py-2 rounded text-sm transition-colors",
        selected
          ? "bg-blue-600/20 ring-1 ring-blue-500 text-neutral-100"
          : "bg-neutral-800 hover:bg-neutral-700 text-neutral-300",
      ].join(" ")}
    >
      <span className="font-medium">{preset.name}</span>
      <span className="ml-2 text-xs text-neutral-500">
        {preset.canvas_width_px}×{preset.canvas_height_px} · {preset.photos_per_canvas}-up · {preset.dpi} DPI
      </span>
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
