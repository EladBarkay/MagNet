import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CanvasPreset, MagnetEvent } from "../types";
import { Field, NumInput } from "./ui";
import { useAsyncForm } from "../hooks/useAsyncForm";

type Props = {
  event: MagnetEvent;
  onCreated: (preset: CanvasPreset, updatedEvent: MagnetEvent) => void;
  onCancel: () => void;
  /** When provided, the form edits this preset instead of creating a new one. */
  editing?: CanvasPreset;
};

const PRESETS = [
  { label: "1-up  2400×1600", w: 2400, h: 1600, n: 1, cols: 1, rows: 1 },
  { label: "2-up  2400×1600", w: 2400, h: 1600, n: 2, cols: 2, rows: 1 },
  { label: "4-up  3600×2400", w: 3600, h: 2400, n: 4, cols: 2, rows: 2 },
  { label: "Custom", w: 0, h: 0, n: 0, cols: 0, rows: 0 },
];

export default function CanvasPresetForm({ event, onCreated, onCancel, editing }: Props) {
  const [name, setName] = useState(editing?.name ?? "");
  const [w, setW] = useState(editing?.canvas_width_px ?? 2400);
  const [h, setH] = useState(editing?.canvas_height_px ?? 1600);
  const [n, setN] = useState(editing?.photos_per_canvas ?? 2);
  const [cols, setCols] = useState(editing?.cols ?? 2);
  const [rows, setRows] = useState(editing?.rows ?? 1);
  const [dpi, setDpi] = useState(editing?.dpi ?? 300);
  const [margin, setMargin] = useState(editing?.margin_px ?? 0);
  const { error, setError, loading: saving, run } = useAsyncForm();

  function applyTemplate(idx: number) {
    const t = PRESETS[idx];
    if (t.n === 0) return; // custom — leave fields as-is
    setW(t.w); setH(t.h); setN(t.n); setCols(t.cols); setRows(t.rows);
    if (!name) setName(t.label.replace(/\s+/g, " ").trim());
  }

  async function save() {
    if (!name.trim()) { setError("Name is required"); return; }
    if (cols * rows < n) { setError(`Grid ${cols}×${rows} has ${cols*rows} slots but photos/canvas is ${n}`); return; }
    const input = { name: name.trim(), canvas_width_px: w, canvas_height_px: h,
      photos_per_canvas: n, dpi, margin_px: margin, cols, rows };
    await run(async () => {
      if (editing) {
        const preset = await invoke<CanvasPreset>("update_canvas_preset", {
          eventId: event.id, presetId: editing.id, preset: input,
        });
        onCreated(preset, {
          ...event,
          canvas_presets: event.canvas_presets.map((p) => (p.id === preset.id ? preset : p)),
        });
      } else {
        const preset = await invoke<CanvasPreset>("create_canvas_preset", {
          eventId: event.id, preset: input,
        });
        onCreated(preset, { ...event, canvas_presets: [...event.canvas_presets, preset] });
      }
    });
  }

  return (
    <div className="bg-neutral-800 rounded-lg p-4 space-y-3">
      <p className="text-sm font-medium text-neutral-200">
        {editing ? "Edit canvas preset" : "New canvas preset"}
      </p>

      {/* Quick-pick templates */}
      <div className="flex flex-wrap gap-1.5">
        {PRESETS.map((t, i) => (
          <button
            key={t.label}
            onClick={() => applyTemplate(i)}
            className="px-2 py-1 text-xs bg-neutral-700 hover:bg-neutral-600 rounded"
          >
            {t.label}
          </button>
        ))}
      </div>

      <div className="grid grid-cols-2 gap-2 text-xs">
        <Field label="Name" span={2}>
          <input value={name} onChange={e => setName(e.target.value)}
            className="w-full bg-neutral-700 rounded px-2 py-1 text-neutral-100 focus:outline-none focus:ring-1 focus:ring-blue-500" />
        </Field>
        <Field label="Width (px)">
          <NumInput value={w} onChange={setW} min={100} />
        </Field>
        <Field label="Height (px)">
          <NumInput value={h} onChange={setH} min={100} />
        </Field>
        <Field label="Photos / canvas">
          <NumInput value={n} onChange={setN} min={1} max={16} />
        </Field>
        <Field label="Grid cols × rows">
          <div className="flex items-center gap-1">
            <NumInput value={cols} onChange={setCols} min={1} max={8} />
            <span className="text-neutral-500">×</span>
            <NumInput value={rows} onChange={setRows} min={1} max={8} />
          </div>
        </Field>
        <Field label="DPI">
          <NumInput value={dpi} onChange={setDpi} min={72} max={600} />
        </Field>
        <Field label="Margin (px)">
          <NumInput value={margin} onChange={setMargin} min={0} max={200} />
        </Field>
      </div>

      {error && <p className="text-xs text-red-400">{error}</p>}

      <div className="flex justify-end gap-2 pt-1">
        <button onClick={onCancel}
          className="px-3 py-1.5 text-xs text-neutral-400 hover:text-neutral-200">
          Cancel
        </button>
        <button onClick={save} disabled={saving}
          className="px-3 py-1.5 text-xs bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded font-medium">
          {saving ? "Saving…" : editing ? "Save changes" : "Save preset"}
        </button>
      </div>
    </div>
  );
}
