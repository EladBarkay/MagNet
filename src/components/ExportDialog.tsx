import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { CanvasPreset, FramePreset, OrenewEvent } from "../types";
import { SaveProgress, useSaveProgress } from "../hooks/useSaveProgress";
import { Modal, Field, Chip, PresetOption } from "./ui";
import { EditIcon, TrashIcon } from "./icons";

type Destination = "print" | "save";

type PrintResult = { count: number; dialog_opened: boolean; output_dir: string };

type Props = {
  event: OrenewEvent;
  photoQueue: Record<string, number>;
  onClose: () => void;
  onEventUpdate: (e: OrenewEvent) => void;
  onExported: (destination: Destination, quantities: Record<string, number>) => void;
  onAddFrame: () => void;
  onEditFrame: (p: FramePreset) => void;
  onDeleteFrame: (p: FramePreset) => void;
  onAddCanvas: () => void;
  onEditCanvas: (p: CanvasPreset) => void;
  onDeleteCanvas: (p: CanvasPreset) => void;
};

export default function ExportDialog({
  event, photoQueue, onClose, onEventUpdate, onExported,
  onAddFrame, onEditFrame, onDeleteFrame, onAddCanvas, onEditCanvas, onDeleteCanvas,
}: Props) {
  const { t } = useTranslation();
  const [destination, setDestination] = useState<Destination>("save");
  const [frameId, setFrameId] = useState<string>(
    event.active_frame_preset_id ?? event.frame_presets[0]?.id ?? ""
  );
  const [canvasId, setCanvasId] = useState<string>(
    event.active_canvas_preset_id ?? event.canvas_presets[0]?.id ?? ""
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [printResult, setPrintResult] = useState<PrintResult | null>(null);
  const { progress, result: saveResult, clear: clearSave } = useSaveProgress();

  const saveProcessedRef = useRef(false);

  // Call onExported as soon as the save-complete event arrives so the queue
  // is cleared even if the component unmounts before the user clicks Done.
  useEffect(() => {
    if (saveResult && !saveProcessedRef.current) {
      saveProcessedRef.current = true;
      onExported("save", photoQueue);
    }
  }, [saveResult, onExported, photoQueue]);

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
    if (!frameId) { setError(t("export.selectFramePreset")); return; }
    if (!canvasId) { setError(t("export.selectCanvasPreset")); return; }
    if (totalQty === 0) { setError(t("export.noPhotosQueued")); return; }
    if (destination === "save" && !event.output_folder) {
      setError(t("export.setSavePathFirst"));
      return;
    }
    setError("");
    setBusy(true);

    // Remember the chosen presets as this event's defaults so the next session
    // (and the next Export) restores the last-used frame + canvas.
    if (event.active_frame_preset_id !== frameId || event.active_canvas_preset_id !== canvasId) {
      const withDefaults = { ...event, active_frame_preset_id: frameId, active_canvas_preset_id: canvasId };
      invoke("save_event", { event: withDefaults }).catch(() => {});
      onEventUpdate(withDefaults);
    }

    try {
      if (destination === "print") {
        const res = await invoke<PrintResult>("print_photos", {
          eventId: event.id,
          quantities,
          framePresetId: frameId,
          canvasPresetId: canvasId,
        });
        setPrintResult(res);
        onExported("print", photoQueue);
      } else {
        clearSave();
        await invoke("save_batch", {
          eventId: event.id,
          quantities,
          framePresetId: frameId,
          canvasPresetId: canvasId,
        });
        // save-complete handled by useSaveProgress
      }
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  // Save done
  if (saveResult) {
    return (
      <Modal onClose={() => { clearSave(); onClose(); }}>
        <div className="space-y-4">
          <div className="flex items-center gap-3">
            <span className="text-2xl">{saveResult.errors.length === 0 ? "✓" : "⚠"}</span>
            <div>
              <p className="font-medium text-neutral-100">
                {saveResult.errors.length === 0
                  ? t("export.saveComplete")
                  : t("export.saveFinishedErrors", { count: saveResult.errors.length })}
              </p>
              <p className="text-xs text-neutral-400 mt-0.5">
                {t("export.saveSummary", { count: totalQty, dir: saveResult.output_dir })}
              </p>
            </div>
          </div>
          {saveResult.errors.length > 0 && (
            <ul className="text-xs text-red-400 space-y-0.5 max-h-32 overflow-y-auto">
              {saveResult.errors.map((e, i) => <li key={i}>{e}</li>)}
            </ul>
          )}
          <div className="flex justify-end gap-2">
            <button
              onClick={async () => {
                try {
                  const { openPath } = await import("@tauri-apps/plugin-opener");
                  await openPath(saveResult.output_dir);
                } catch (e) {
                  alert(t("common.couldNotOpenFolder", { message: String(e) }));
                }
              }}
              className="px-3 py-1.5 text-sm bg-neutral-700 hover:bg-neutral-600 rounded"
            >
              {t("export.openFolder")}
            </button>
            <button
              onClick={() => { clearSave(); onClose(); }}
              className="px-3 py-1.5 text-sm bg-accent hover:bg-accent-hover rounded font-medium"
            >
              {t("common.done")}
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
            {printResult.dialog_opened
              ? t("export.printDialogOpened", { count: printResult.count })
              : t("export.printToFolder", { count: printResult.count })}
          </p>
          <div className="flex justify-center gap-2">
            {!printResult.dialog_opened && (
              <button
                onClick={async () => {
                  try {
                    const { openPath } = await import("@tauri-apps/plugin-opener");
                    await openPath(printResult.output_dir);
                  } catch (e) {
                    alert(t("common.couldNotOpenFolder", { message: String(e) }));
                  }
                }}
                className="px-3 py-1.5 text-sm bg-neutral-700 hover:bg-neutral-600 rounded"
              >
                {t("export.openFolder")}
              </button>
            )}
            <button
              onClick={onClose}
              className="px-4 py-1.5 bg-accent hover:bg-accent-hover rounded text-sm font-medium"
            >
              {t("common.done")}
            </button>
          </div>
        </div>
      </Modal>
    );
  }

  // Save in progress
  if (busy && destination === "save" && progress) {
    return (
      <Modal onClose={() => {}}>
        <SaveProgressView progress={progress} />
      </Modal>
    );
  }

  return (
    <Modal onClose={onClose}>
      <div className="space-y-4">
        <h2 className="text-base font-semibold text-neutral-100">{t("export.title")}</h2>

        {/* Destination toggle */}
        <Field label={t("export.sendTo")}>
          <div className="flex gap-1.5">
            <Chip label={t("export.print")} active={destination === "print"} onClick={() => setDestination("print")} />
            <Chip label={t("export.saveToPath")} active={destination === "save"} onClick={() => setDestination("save")} />
          </div>
        </Field>

        {/* Frame preset — pick + manage (add/edit/delete) */}
        <Field label={t("export.framePreset")}>
          <div className="flex flex-wrap items-center gap-1.5">
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

        {/* Canvas preset — pick + manage (add/edit/delete) */}
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

        {/* Save path (save only) */}
        {destination === "save" && (
          <Field label={t("export.savePath")}>
            <div className="flex items-center gap-2">
              {event.output_folder ? (
                <span className="flex-1 text-xs text-neutral-300 truncate bg-neutral-800 rounded px-2 py-1.5">
                  {event.output_folder}
                </span>
              ) : (
                <span className="flex-1 text-xs text-neutral-600 italic">{t("export.notSet")}</span>
              )}
              <button
                onClick={pickOutputFolder}
                className="px-2 py-1.5 text-xs bg-neutral-700 hover:bg-neutral-600 rounded whitespace-nowrap"
              >
                {event.output_folder ? t("common.change") : t("export.setPath")}
              </button>
            </div>
          </Field>
        )}

        {/* Summary */}
        {canvasPreset && totalQty > 0 && (
          <p className="text-xs text-neutral-500">
            <strong className="text-neutral-300">{t("common.photos", { count: totalQty })}</strong>
            {" → "}
            <strong className="text-neutral-300">
              {t("export.canvases", { count: canvasCount })}
            </strong>
            {" "}({t("export.specUp", { n: canvasPreset.photos_per_canvas, w: canvasPreset.canvas_width_px, h: canvasPreset.canvas_height_px })}
            {destination === "save" ? t("export.specDpi", { dpi: canvasPreset.dpi }) : ""})
          </p>
        )}

        {error && <p className="text-xs text-red-400">{error}</p>}

        <div className="flex justify-end gap-2 pt-1">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-sm text-neutral-400 hover:text-neutral-200"
          >
            {t("common.cancel")}
          </button>
          <button
            onClick={go}
            disabled={busy || totalQty === 0 || !frameId || !canvasId || (destination === "save" && !event.output_folder)}
            className="px-4 py-1.5 text-sm bg-accent hover:bg-accent-hover disabled:opacity-40 disabled:cursor-not-allowed rounded font-medium"
          >
            {busy
              ? destination === "print" ? t("export.composing") : t("export.starting")
              : destination === "print"
                ? t("export.printAction", { count: totalQty })
                : t("export.saveAction", { count: canvasCount })}
          </button>
        </div>
      </div>
    </Modal>
  );
}

function SaveProgressView({ progress }: { progress: SaveProgress }) {
  const { t } = useTranslation();
  const pct = progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0;
  return (
    <div className="space-y-4 py-2">
      <p className="text-sm font-medium text-neutral-200">{t("export.saving")}</p>
      <div className="space-y-2">
        <div className="flex justify-between text-xs text-neutral-400">
          <span className="truncate max-w-[70%]">{progress.current_file}</span>
          <span>{progress.done} / {progress.total}</span>
        </div>
        <div className="h-2 bg-neutral-700 rounded-full overflow-hidden">
          <div
            className="h-full bg-accent transition-all duration-200"
            style={{ width: `${pct}%` }}
          />
        </div>
      </div>
    </div>
  );
}
