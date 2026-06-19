import { useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { open as openFilePicker } from "@tauri-apps/plugin-dialog";
import { FramePreset, MagnetEvent } from "../types";
import { Modal, Field, PathPicker } from "./ui";
import { useAsyncForm } from "../hooks/useAsyncForm";
import { useFrameThumbnail } from "../hooks/useFrameThumbnail";

type Props = {
  event: MagnetEvent;
  onCreated: (updatedEvent: MagnetEvent) => void;
  onClose: () => void;
  /** When provided, the dialog edits this preset instead of creating a new one. */
  editing?: FramePreset;
};

export default function FramePresetDialog({ event, onCreated, onClose, editing }: Props) {
  const { t } = useTranslation();
  const [name, setName] = useState(editing?.name ?? "");
  const [landscapePath, setLandscapePath] = useState(editing?.landscape_frame_path ?? "");
  const [portraitPath, setPortraitPath] = useState(editing?.portrait_frame_path ?? "");
  const [ratioW, setRatioW] = useState(editing?.target_ratio_w ?? 4);
  const [ratioH, setRatioH] = useState(editing?.target_ratio_h ?? 3);
  const { error, setError, loading: saving, run } = useAsyncForm();

  async function pickPng(setter: (p: string) => void) {
    const path = await openFilePicker({
      multiple: false,
      filters: [{ name: "PNG frame", extensions: ["png"] }],
      defaultPath: event.root_path ?? undefined,
    });
    if (path) setter(path as string);
  }

  async function save() {
    if (!name.trim()) { setError(t("framePreset.nameRequired")); return; }
    if (!landscapePath) { setError(t("framePreset.landscapeRequired")); return; }
    if (!portraitPath) { setError(t("framePreset.portraitRequired")); return; }
    if (ratioW <= 0 || ratioH <= 0) { setError(t("framePreset.ratioPositive")); return; }

    const input = {
      name: name.trim(),
      landscape_frame_path: landscapePath,
      portrait_frame_path: portraitPath,
      target_ratio_w: ratioW,
      target_ratio_h: ratioH,
    };
    await run(async () => {
      if (editing) {
        const preset = await invoke<FramePreset>("update_frame_preset", {
          eventId: event.id, presetId: editing.id, preset: input,
        });
        onCreated({
          ...event,
          frame_presets: event.frame_presets.map((p) => (p.id === preset.id ? preset : p)),
        });
      } else {
        const preset = await invoke<FramePreset>("create_frame_preset", {
          eventId: event.id, preset: input,
        });
        // Set as active and persist
        const updatedEvent: MagnetEvent = {
          ...event,
          frame_presets: [...event.frame_presets, preset],
          active_frame_preset_id: preset.id,
        };
        await invoke("save_event", { event: updatedEvent });
        onCreated(updatedEvent);
      }
    });
  }

  return (
    <Modal onClose={onClose} size="sm">
      <div className="space-y-4">
        <h2 className="text-base font-semibold text-neutral-100">
          {editing ? t("framePreset.editTitle") : t("framePreset.addTitle")}
        </h2>

        {/* Name */}
        <Field label={t("framePreset.name")}>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t("framePreset.namePlaceholder")}
            className="w-full bg-neutral-800 rounded px-3 py-1.5 text-sm text-neutral-100 focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-neutral-600"
          />
        </Field>

        {/* Landscape frame */}
        <Field label={t("framePreset.landscapeFrame")}>
          <div className="flex items-center gap-3">
            <div className="flex-1 min-w-0">
              <PathPicker
                path={landscapePath}
                placeholder={t("framePreset.pickLandscape")}
                onPick={() => pickPng(setLandscapePath)}
              />
            </div>
            <FramePreview path={landscapePath} className="w-24 aspect-[4/3]" />
          </div>
        </Field>

        {/* Portrait frame */}
        <Field label={t("framePreset.portraitFrame")}>
          <div className="flex items-center gap-3">
            <div className="flex-1 min-w-0">
              <PathPicker
                path={portraitPath}
                placeholder={t("framePreset.pickPortrait")}
                onPick={() => pickPng(setPortraitPath)}
              />
            </div>
            <FramePreview path={portraitPath} className="w-[4.5rem] aspect-[3/4]" />
          </div>
        </Field>

        {/* Ratio */}
        <Field label={t("framePreset.targetRatio")}>
          <div className="flex items-center gap-2">
            <input
              type="number" value={ratioW} min={1} max={100} step={0.1}
              onChange={(e) => setRatioW(Number(e.target.value))}
              className="w-20 bg-neutral-800 rounded px-2 py-1.5 text-sm text-center text-neutral-100 focus:outline-none focus:ring-1 focus:ring-accent"
            />
            <span className="text-neutral-500 font-medium">:</span>
            <input
              type="number" value={ratioH} min={1} max={100} step={0.1}
              onChange={(e) => setRatioH(Number(e.target.value))}
              className="w-20 bg-neutral-800 rounded px-2 py-1.5 text-sm text-center text-neutral-100 focus:outline-none focus:ring-1 focus:ring-accent"
            />
            <span className="text-xs text-neutral-500">
              = {(ratioW / ratioH).toFixed(2)}
            </span>
          </div>
        </Field>

        {error && <p className="text-xs text-red-400">{error}</p>}

        <div className="flex justify-end gap-2 pt-1">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-sm text-neutral-400 hover:text-neutral-200"
          >
            {t("common.cancel")}
          </button>
          <button
            onClick={save}
            disabled={saving}
            className="px-4 py-1.5 text-sm bg-accent hover:bg-accent-hover disabled:opacity-40 rounded font-medium"
          >
            {saving ? t("framePreset.saving") : editing ? t("framePreset.saveChanges") : t("framePreset.addFrame")}
          </button>
        </div>
      </div>
    </Modal>
  );
}

// Frame preview on a checkerboard so the transparent border is visible.
const CHECKER =
  "repeating-conic-gradient(#3a3a3d 0% 25%, #2a2a2d 0% 50%) 50% / 12px 12px";

function FramePreview({ path, className = "" }: { path: string; className?: string }) {
  const src = useFrameThumbnail(path || null);
  return (
    <div
      className={`shrink-0 rounded border border-neutral-700 overflow-hidden ${className}`}
      style={{ background: CHECKER }}
    >
      {src && <img src={src} alt="" className="w-full h-full object-contain" draggable={false} />}
    </div>
  );
}
