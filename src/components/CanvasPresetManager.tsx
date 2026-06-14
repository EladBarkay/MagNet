import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CanvasPreset, MagnetEvent } from "../types";
import CanvasPresetForm from "./CanvasPresetForm";
import { Modal } from "./ui";

type Props = {
  event: MagnetEvent;
  onClose: () => void;
  onEventUpdate: (e: MagnetEvent) => void;
};

type Mode = { kind: "list" } | { kind: "new" } | { kind: "edit"; preset: CanvasPreset };

export default function CanvasPresetManager({ event, onClose, onEventUpdate }: Props) {
  const [mode, setMode] = useState<Mode>(
    event.canvas_presets.length === 0 ? { kind: "new" } : { kind: "list" }
  );
  const [error, setError] = useState("");

  async function remove(preset: CanvasPreset) {
    setError("");
    try {
      await invoke("delete_canvas_preset", { eventId: event.id, presetId: preset.id });
      onEventUpdate({
        ...event,
        canvas_presets: event.canvas_presets.filter((p) => p.id !== preset.id),
      });
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <Modal onClose={onClose} size="lg">
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-base font-semibold text-neutral-100">Canvas presets</h2>
          {mode.kind === "list" && (
            <button
              onClick={() => setMode({ kind: "new" })}
              className="text-xs text-blue-400 hover:text-blue-300 font-medium"
            >
              + New
            </button>
          )}
        </div>

        {error && <p className="text-xs text-red-400">{error}</p>}

        {mode.kind === "list" ? (
          <div className="space-y-1.5">
            {event.canvas_presets.length === 0 ? (
              <p className="text-xs text-neutral-500">No presets yet.</p>
            ) : (
              event.canvas_presets.map((p) => (
                <div
                  key={p.id}
                  className="flex items-center justify-between rounded bg-neutral-800 px-3 py-2"
                >
                  <div className="min-w-0">
                    <p className="text-sm text-neutral-200 truncate">{p.name}</p>
                    <p className="text-[10px] text-neutral-500">
                      {p.canvas_width_px}×{p.canvas_height_px} · {p.photos_per_canvas}-up ·{" "}
                      {p.cols}×{p.rows} grid · margin {p.margin_px}px
                    </p>
                  </div>
                  <div className="flex items-center gap-2 shrink-0 ml-2">
                    <button
                      onClick={() => setMode({ kind: "edit", preset: p })}
                      className="text-xs text-blue-400 hover:text-blue-300"
                    >
                      Edit
                    </button>
                    <button
                      onClick={() => remove(p)}
                      className="text-xs text-red-400 hover:text-red-300"
                    >
                      Delete
                    </button>
                  </div>
                </div>
              ))
            )}
            <div className="flex justify-end pt-2">
              <button
                onClick={onClose}
                className="px-3 py-1.5 text-sm text-neutral-400 hover:text-neutral-200"
              >
                Close
              </button>
            </div>
          </div>
        ) : (
          <CanvasPresetForm
            event={event}
            editing={mode.kind === "edit" ? mode.preset : undefined}
            onCreated={(_preset, updatedEvent) => {
              onEventUpdate(updatedEvent);
              setMode({ kind: "list" });
            }}
            onCancel={() => setMode({ kind: "list" })}
          />
        )}
      </div>
    </Modal>
  );
}
