import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openFilePicker } from "@tauri-apps/plugin-dialog";
import { FramePreset, MagnetEvent } from "../types";
import { Modal, Field, PathPicker } from "./ui";

type Props = {
  event: MagnetEvent;
  onCreated: (updatedEvent: MagnetEvent) => void;
  onClose: () => void;
  /** When provided, the dialog edits this preset instead of creating a new one. */
  editing?: FramePreset;
};

export default function FramePresetDialog({ event, onCreated, onClose, editing }: Props) {
  const [name, setName] = useState(editing?.name ?? "");
  const [landscapePath, setLandscapePath] = useState(editing?.landscape_frame_path ?? "");
  const [portraitPath, setPortraitPath] = useState(editing?.portrait_frame_path ?? "");
  const [ratioW, setRatioW] = useState(editing?.target_ratio_w ?? 4);
  const [ratioH, setRatioH] = useState(editing?.target_ratio_h ?? 3);
  const [cropMethod, setCropMethod] = useState<"center" | "rule_of_thirds">(
    editing?.crop_method ?? "center"
  );
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  async function pickPng(setter: (p: string) => void) {
    const path = await openFilePicker({
      multiple: false,
      filters: [{ name: "PNG frame", extensions: ["png"] }],
      defaultPath: event.root_path ?? undefined,
    });
    if (path) setter(path as string);
  }

  async function save() {
    if (!name.trim()) { setError("Name is required"); return; }
    if (!landscapePath) { setError("Landscape frame PNG is required"); return; }
    if (!portraitPath) { setError("Portrait frame PNG is required"); return; }
    if (ratioW <= 0 || ratioH <= 0) { setError("Ratio must be positive"); return; }

    setSaving(true);
    setError("");
    const input = {
      name: name.trim(),
      landscape_frame_path: landscapePath,
      portrait_frame_path: portraitPath,
      target_ratio_w: ratioW,
      target_ratio_h: ratioH,
      crop_method: cropMethod,
    };
    try {
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
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  }

  return (
    <Modal onClose={onClose} size="sm">
      <div className="space-y-4">
        <h2 className="text-base font-semibold text-neutral-100">
          {editing ? "Edit frame preset" : "Add frame preset"}
        </h2>

        {/* Name */}
        <Field label="Name">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Classic 4:3"
            className="w-full bg-neutral-800 rounded px-3 py-1.5 text-sm text-neutral-100 focus:outline-none focus:ring-1 focus:ring-blue-500 placeholder:text-neutral-600"
          />
        </Field>

        {/* Landscape frame */}
        <Field label="Landscape frame (PNG)">
          <PathPicker
            path={landscapePath}
            placeholder="Pick landscape PNG…"
            onPick={() => pickPng(setLandscapePath)}
          />
        </Field>

        {/* Portrait frame */}
        <Field label="Portrait frame (PNG)">
          <PathPicker
            path={portraitPath}
            placeholder="Pick portrait PNG…"
            onPick={() => pickPng(setPortraitPath)}
          />
        </Field>

        {/* Ratio */}
        <Field label="Target ratio (W : H)">
          <div className="flex items-center gap-2">
            <input
              type="number" value={ratioW} min={1} max={100} step={0.1}
              onChange={(e) => setRatioW(Number(e.target.value))}
              className="w-20 bg-neutral-800 rounded px-2 py-1.5 text-sm text-center text-neutral-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
            />
            <span className="text-neutral-500 font-medium">:</span>
            <input
              type="number" value={ratioH} min={1} max={100} step={0.1}
              onChange={(e) => setRatioH(Number(e.target.value))}
              className="w-20 bg-neutral-800 rounded px-2 py-1.5 text-sm text-center text-neutral-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
            />
            <span className="text-xs text-neutral-500">
              = {(ratioW / ratioH).toFixed(2)}
            </span>
          </div>
        </Field>

        {/* Crop method */}
        <Field label="Crop method">
          <div className="flex gap-2">
            {(["center", "rule_of_thirds"] as const).map((m) => (
              <button
                key={m}
                onClick={() => setCropMethod(m)}
                className={[
                  "flex-1 py-1.5 text-xs rounded transition-colors",
                  cropMethod === m
                    ? "bg-blue-600 text-white"
                    : "bg-neutral-800 text-neutral-400 hover:bg-neutral-700",
                ].join(" ")}
              >
                {m === "center" ? "Center" : "Rule of thirds"}
              </button>
            ))}
          </div>
        </Field>

        {error && <p className="text-xs text-red-400">{error}</p>}

        <div className="flex justify-end gap-2 pt-1">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-sm text-neutral-400 hover:text-neutral-200"
          >
            Cancel
          </button>
          <button
            onClick={save}
            disabled={saving}
            className="px-4 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-40 rounded font-medium"
          >
            {saving ? "Saving…" : editing ? "Save changes" : "Add frame"}
          </button>
        </div>
      </div>
    </Modal>
  );
}
