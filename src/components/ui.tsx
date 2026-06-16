import { CanvasPreset } from "../types";

/** Centered modal with a click-to-dismiss backdrop. `size` controls max width. */
export function Modal({
  children,
  onClose,
  size = "md",
}: {
  children: React.ReactNode;
  onClose: () => void;
  size?: "sm" | "md" | "lg";
}) {
  const maxW = size === "sm" ? "max-w-sm" : size === "lg" ? "max-w-lg" : "max-w-md";
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />
      <div
        className={`relative z-10 w-full ${maxW} mx-4 bg-neutral-900 border border-neutral-700 rounded-xl shadow-2xl p-5`}
      >
        {children}
      </div>
    </div>
  );
}

/** Labeled form field (label above content). */
export function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1">
      <label className="text-xs font-medium text-neutral-400">{label}</label>
      {children}
    </div>
  );
}

/** Small pill toggle used for picking among presets. */
export function Chip({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
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

/** Numeric input bound to a number state. */
export function NumInput({
  value,
  onChange,
  min,
  max,
}: {
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
}) {
  return (
    <input
      type="number"
      value={value}
      min={min}
      max={max}
      onChange={(e) => onChange(Number(e.target.value))}
      className="w-full bg-neutral-700 rounded px-2 py-1 text-neutral-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
    />
  );
}

/** Round +/− stepper button. `sm` is the compact toolbar variant; the default
 *  is the larger overlay variant used on gallery cards. Stops click propagation
 *  so it can sit on top of a clickable parent (e.g. a photo card). */
export function QtyButton({
  label,
  onClick,
  disabled,
  size = "md",
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  size?: "sm" | "md";
}) {
  const variant =
    size === "sm"
      ? "w-5 h-5 bg-white/10 hover:bg-white/20 text-neutral-200 text-sm"
      : "w-7 h-7 bg-white/20 hover:bg-white/40 text-white text-base";
  return (
    <button
      onClick={(e) => { e.stopPropagation(); onClick(); }}
      disabled={disabled}
      className={`flex items-center justify-center rounded-full disabled:opacity-30 leading-none font-medium ${variant}`}
    >
      {label}
    </button>
  );
}

/** File-path display + pick button (shows the basename). */
export function PathPicker({
  path,
  placeholder,
  onPick,
}: {
  path: string;
  placeholder: string;
  onPick: () => void;
}) {
  return (
    <div className="flex gap-2">
      <div className="flex-1 min-w-0 bg-neutral-800 rounded px-3 py-1.5 text-xs text-neutral-400 truncate">
        {path ? path.split(/[\\/]/).pop() : <span className="text-neutral-600">{placeholder}</span>}
      </div>
      <button
        onClick={onPick}
        className="shrink-0 px-2.5 py-1.5 text-xs bg-neutral-700 hover:bg-neutral-600 rounded"
      >
        {path ? "Change" : "Pick PNG"}
      </button>
    </div>
  );
}

/** Selectable canvas-preset row showing its dimensions and layout. */
export function PresetOption({
  preset,
  selected,
  onSelect,
}: {
  preset: CanvasPreset;
  selected: boolean;
  onSelect: () => void;
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
        {preset.canvas_width_px}×{preset.canvas_height_px} · {preset.photos_per_canvas}-up ·{" "}
        {preset.dpi} DPI
      </span>
    </button>
  );
}
