import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CanvasPreset, MagnetEvent } from "../types";
import CanvasPresetForm from "./CanvasPresetForm";
import { Modal } from "./ui";
import { reorderById } from "../lib/reorder";
import { useAsyncForm } from "../hooks/useAsyncForm";

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
  const { error, run } = useAsyncForm();
  const [draggedId, setDraggedId] = useState<string | null>(null);
  const [dragOverId, setDragOverId] = useState<string | null>(null);

  async function remove(preset: CanvasPreset) {
    await run(async () => {
      await invoke("delete_canvas_preset", { eventId: event.id, presetId: preset.id });
      onEventUpdate({
        ...event,
        canvas_presets: event.canvas_presets.filter((p) => p.id !== preset.id),
      });
    });
  }

  function reorder(targetId: string) {
    const canvas_presets = reorderById(event.canvas_presets, draggedId, targetId);
    if (!canvas_presets) return;
    const updated = { ...event, canvas_presets };
    onEventUpdate(updated);
    invoke("save_event", { event: updated }).catch(() => {});
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
              event.canvas_presets.map((p) => {
                const isOver = dragOverId === p.id && draggedId !== p.id;
                return (
                  <div
                    key={p.id}
                    draggable
                    onDragStart={(e) => { e.dataTransfer.setData("text/plain", p.id); setDraggedId(p.id); }}
                    onDragOver={(e) => { e.preventDefault(); setDragOverId(p.id); }}
                    onDragLeave={() => setDragOverId(null)}
                    onDrop={(e) => { e.preventDefault(); setDragOverId(null); reorder(p.id); }}
                    onDragEnd={() => { setDraggedId(null); setDragOverId(null); }}
                    className={[
                      "flex items-center justify-between rounded bg-neutral-800 px-3 py-2 cursor-grab transition-opacity",
                      draggedId === p.id ? "opacity-40" : "",
                      isOver ? "ring-2 ring-blue-500" : "",
                    ].join(" ")}
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
                );
              })
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
