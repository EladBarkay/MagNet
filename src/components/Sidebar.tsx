import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FramePreset, MagnetEvent, PhotoBatch } from "../types";
import { batchDisplayPath, parentDir } from "../lib/paths";
import { EditIcon, TrashIcon } from "./icons";

type Props = {
  event: MagnetEvent;
  activeBatch: PhotoBatch | null;
  draggedBatchId: string | null;
  setDraggedBatchId: (id: string | null) => void;
  onAddBatch: () => void;
  onSelectBatch: (b: PhotoBatch) => void;
  onDeleteBatch: (b: PhotoBatch) => void;
  onReorderBatch: (targetId: string) => void;
  draggedFrameId: string | null;
  setDraggedFrameId: (id: string | null) => void;
  onReorderFrame: (targetId: string) => void;
  onAddFrame: () => void;
  onEditFrame: (p: FramePreset) => void;
  onDeleteFrame: (p: FramePreset) => void;
  onManageCanvas: () => void;
};

export default function Sidebar({
  event, activeBatch, draggedBatchId, setDraggedBatchId,
  onAddBatch, onSelectBatch, onDeleteBatch, onReorderBatch,
  draggedFrameId, setDraggedFrameId, onReorderFrame,
  onAddFrame, onEditFrame, onDeleteFrame, onManageCanvas,
}: Props) {
  const [dragOverBatchId, setDragOverBatchId] = useState<string | null>(null);
  const [dragOverFrameId, setDragOverFrameId] = useState<string | null>(null);

  return (
    <aside className="w-52 shrink-0 flex flex-col bg-neutral-850 border-r border-neutral-700 overflow-y-auto">
      <Section label="Batches" action={
        <button onClick={onAddBatch} className="text-[10px] text-blue-400 hover:text-blue-300 font-medium">+ Add</button>
      }>
        {event.batches.length === 0 ? (
          <p className="px-3 py-1 text-xs text-neutral-600">
            No batches —{" "}
            <button onClick={onAddBatch} className="text-blue-400 hover:text-blue-300 underline">
              add a folder
            </button>
          </p>
        ) : (
          event.batches.map((b) => {
            const displayPath = batchDisplayPath(b.source_path, event.root_path ? parentDir(event.root_path) : null);
            const isOver = dragOverBatchId === b.id && draggedBatchId !== b.id;
            return (
              <div
                key={b.id}
                draggable
                onDragStart={(e) => { e.dataTransfer.setData("text/plain", b.id); setDraggedBatchId(b.id); }}
                onDragOver={(e) => { e.preventDefault(); setDragOverBatchId(b.id); }}
                onDragLeave={() => setDragOverBatchId(null)}
                onDrop={(e) => { e.preventDefault(); setDragOverBatchId(null); onReorderBatch(b.id); }}
                onDragEnd={() => { setDraggedBatchId(null); setDragOverBatchId(null); }}
                className={[
                  "group relative cursor-grab",
                  draggedBatchId === b.id ? "opacity-40" : "",
                  isOver ? "border-t-2 border-blue-500" : "",
                ].join(" ")}
              >
                <button
                  onClick={() => onSelectBatch(b)}
                  onDoubleClick={async () => {
                    try {
                      await invoke("open_in_explorer", { path: b.source_path });
                    } catch (e) {
                      alert(`Could not open folder: ${e}`);
                    }
                  }}
                  className={[
                    "w-full text-left px-3 py-1.5 pr-8 text-sm transition-colors cursor-grab",
                    b.id === activeBatch?.id
                      ? "bg-blue-600/20 text-blue-300"
                      : "text-neutral-300 hover:bg-neutral-700/60",
                  ].join(" ")}
                >
                  <span className="block truncate">{b.name}</span>
                  <span className="block text-[10px] text-neutral-500 truncate" title={b.source_path}>
                    path: {displayPath}
                  </span>
                  <span className="block text-[10px] text-neutral-600">{b.photos.length} photos</span>
                </button>
                <button
                  onClick={(e) => { e.stopPropagation(); onDeleteBatch(b); }}
                  title="Remove batch"
                  className="absolute right-1.5 top-1/2 -translate-y-1/2 p-1 text-neutral-600 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
                >
                  <TrashIcon className="w-3 h-3" />
                </button>
              </div>
            );
          })
        )}
      </Section>

      <Section label="Frames" action={
        <button onClick={onAddFrame} className="text-[10px] text-blue-400 hover:text-blue-300 font-medium">+ Add</button>
      }>
        {event.frame_presets.length === 0 ? (
          <p className="px-3 py-1 text-xs text-neutral-600">
            No frames —{" "}
            <button onClick={onAddFrame} className="text-blue-400 hover:text-blue-300 underline">
              add one
            </button>
          </p>
        ) : (
          event.frame_presets.map((p) => {
            const isOver = dragOverFrameId === p.id && draggedFrameId !== p.id;
            return (
              <div
                key={p.id}
                draggable
                onDragStart={(e) => { e.dataTransfer.setData("text/plain", p.id); setDraggedFrameId(p.id); }}
                onDragOver={(e) => { e.preventDefault(); setDragOverFrameId(p.id); }}
                onDragLeave={() => setDragOverFrameId(null)}
                onDrop={(e) => { e.preventDefault(); setDragOverFrameId(null); onReorderFrame(p.id); }}
                onDragEnd={() => { setDraggedFrameId(null); setDragOverFrameId(null); }}
                className={[
                  "group relative cursor-grab",
                  draggedFrameId === p.id ? "opacity-40" : "",
                  isOver ? "border-t-2 border-blue-500" : "",
                ].join(" ")}
              >
                <div className="w-full px-3 py-1.5 pr-16 text-sm text-neutral-300">
                  <span className="block truncate">{p.name}</span>
                  <span className="block text-[10px] text-neutral-500">
                    {`${p.target_ratio_w}:${p.target_ratio_h} · ${p.crop_method === "center" ? "center" : "rule of thirds"}`}
                  </span>
                </div>
                <div className="absolute right-1.5 top-1/2 -translate-y-1/2 flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button
                    onClick={(e) => { e.stopPropagation(); onEditFrame(p); }}
                    title="Edit frame preset"
                    className="p-1 text-neutral-500 hover:text-blue-400"
                  >
                    <EditIcon />
                  </button>
                  <button
                    onClick={(e) => { e.stopPropagation(); onDeleteFrame(p); }}
                    title="Delete frame preset"
                    className="p-1 text-neutral-500 hover:text-red-400"
                  >
                    <TrashIcon className="w-3 h-3" />
                  </button>
                </div>
              </div>
            );
          })
        )}
      </Section>

      <Section
        label="Canvas presets"
        action={
          <button onClick={onManageCanvas} className="text-[10px] text-blue-400 hover:text-blue-300 font-medium">
            Manage
          </button>
        }
      >
        {event.canvas_presets.length === 0 ? (
          <p className="px-3 py-1 text-xs text-neutral-600">
            No presets —{" "}
            <button onClick={onManageCanvas} className="text-blue-400 hover:text-blue-300 underline">
              add one
            </button>
          </p>
        ) : (
          event.canvas_presets.map((p) => (
            <SidebarItem
              key={p.id}
              label={p.name}
              sublabel={`${p.canvas_width_px}×${p.canvas_height_px} · ${p.photos_per_canvas}-up`}
              active={false}
              onClick={onManageCanvas}
            />
          ))
        )}
      </Section>
    </aside>
  );
}

function Section({
  label, children, action,
}: {
  label: string; children: React.ReactNode; action?: React.ReactNode;
}) {
  return (
    <div className="py-2">
      <div className="flex items-center justify-between px-3 pb-1">
        <p className="text-[10px] font-semibold uppercase tracking-widest text-neutral-500">{label}</p>
        {action}
      </div>
      {children}
    </div>
  );
}

function SidebarItem({
  label, sublabel, active, onClick,
}: {
  label: string; sublabel?: string; active?: boolean; onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={[
        "w-full text-left px-3 py-1.5 text-sm transition-colors hover:bg-neutral-700/60",
        active ? "bg-blue-600/20 text-blue-300" : "text-neutral-300",
      ].join(" ")}
    >
      <span className="block truncate">{label}</span>
      {sublabel && <span className="block text-[10px] text-neutral-500">{sublabel}</span>}
    </button>
  );
}
