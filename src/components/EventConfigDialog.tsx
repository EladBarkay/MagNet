import { useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { CanvasPreset, FramePreset, MagnetEvent } from "../types";
import { Modal, Field, Chip, PresetOption } from "./ui";
import { EditIcon, TrashIcon } from "./icons";

type Props = {
  event: MagnetEvent;
  onClose: () => void;
  onEventUpdate: (e: MagnetEvent) => void;
  onAddFrame: () => void;
  onEditFrame: (p: FramePreset) => void;
  onDeleteFrame: (p: FramePreset) => void;
  onAddCanvas: () => void;
  onEditCanvas: (p: CanvasPreset) => void;
  onDeleteCanvas: (p: CanvasPreset) => void;
};

/**
 * Set the event's active frame + canvas presets (and manage them) without going
 * through Export. This makes the gallery/lightbox previews show the chosen frame
 * before the user ever opens the export flow.
 */
export default function EventConfigDialog({
  event, onClose, onEventUpdate,
  onAddFrame, onEditFrame, onDeleteFrame, onAddCanvas, onEditCanvas, onDeleteCanvas,
}: Props) {
  const { t } = useTranslation();
  const [frameId, setFrameId] = useState<string | null>(event.active_frame_preset_id);
  const [canvasId, setCanvasId] = useState<string | null>(
    event.active_canvas_preset_id ?? event.canvas_presets[0]?.id ?? null
  );

  function done() {
    if (event.active_frame_preset_id !== frameId || event.active_canvas_preset_id !== canvasId) {
      const updated = { ...event, active_frame_preset_id: frameId, active_canvas_preset_id: canvasId };
      invoke("save_event", { event: updated }).catch(() => {});
      onEventUpdate(updated);
    }
    onClose();
  }

  return (
    <Modal onClose={onClose}>
      <div className="space-y-4">
        <h2 className="text-base font-semibold text-neutral-100">{t("eventConfig.title")}</h2>

        {/* Frame preset — pick (incl. None) + manage */}
        <Field label={t("export.framePreset")}>
          <div className="flex flex-wrap items-center gap-1.5">
            <Chip label={t("preview.none")} active={frameId === null} onClick={() => setFrameId(null)} />
            {event.frame_presets.map((p) => (
              <div key={p.id} className="group relative flex items-center">
                <Chip label={p.name} active={p.id === frameId} onClick={() => setFrameId(p.id)} onDoubleClick={() => onEditFrame(p)} />
                <span className="ms-1 hidden group-hover:flex items-center gap-0.5">
                  <button onClick={() => onEditFrame(p)} title={t("sidebar.editFrame")} className="p-0.5 text-neutral-500 hover:text-accent"><EditIcon /></button>
                  <button onClick={() => onDeleteFrame(p)} title={t("sidebar.deleteFrame")} className="p-0.5 text-neutral-500 hover:text-red-400"><TrashIcon className="w-3 h-3" /></button>
                </span>
              </div>
            ))}
            <button
              onClick={onAddFrame}
              className="px-2.5 py-1 text-xs rounded border border-dashed border-neutral-600 text-neutral-400 hover:border-accent hover:text-accent transition-colors"
            >
              {t("sidebar.add")}
            </button>
          </div>
          {event.frame_presets.length === 0 && (
            <p className="text-xs text-neutral-500 mt-1">{t("export.noFramePresets")}</p>
          )}
        </Field>

        {/* Canvas preset — pick + manage */}
        <Field label={t("export.canvasPreset")}>
          <div className="space-y-1">
            {event.canvas_presets.map((p) => (
              <div key={p.id} className="group flex items-center gap-1">
                <div className="flex-1 min-w-0">
                  <PresetOption preset={p} selected={p.id === canvasId} onSelect={() => setCanvasId(p.id)} onDoubleClick={() => onEditCanvas(p)} />
                </div>
                <button onClick={() => onEditCanvas(p)} title={t("sidebar.editCanvas")} className="p-1 text-neutral-500 hover:text-accent opacity-0 group-hover:opacity-100 transition-opacity"><EditIcon /></button>
                <button onClick={() => onDeleteCanvas(p)} title={t("sidebar.deleteCanvas")} className="p-1 text-neutral-500 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"><TrashIcon className="w-3 h-3" /></button>
              </div>
            ))}
            <button
              onClick={onAddCanvas}
              className="w-full px-3 py-2 text-xs text-start rounded border border-dashed border-neutral-600 text-neutral-400 hover:border-accent hover:text-accent transition-colors"
            >
              {t("sidebar.add")}
            </button>
          </div>
          {event.canvas_presets.length === 0 && (
            <p className="text-xs text-neutral-500 mt-1">{t("export.noCanvasPresets")}</p>
          )}
        </Field>

        <div className="flex justify-end gap-2 pt-1">
          <button onClick={done} className="px-4 py-1.5 text-sm bg-accent hover:bg-accent-hover rounded font-medium">
            {t("common.done")}
          </button>
        </div>
      </div>
    </Modal>
  );
}
