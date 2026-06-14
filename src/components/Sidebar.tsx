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
  onAddFrame: () => void;
  onEditFrame: (p: FramePreset) => void;
  onDeleteFrame: (p: FramePreset) => void;
  onManageCanvas: () => void;
};

export default function Sidebar({
  event, activeBatch, draggedBatchId, setDraggedBatchId,
  onAddBatch, onSelectBatch, onDeleteBatch, onReorderBatch,
  onAddFrame, onEditFrame, onDeleteFrame, onManageCanvas,
}: Props) {
  return (
    <aside className="w-52 shrink-0 flex flex-col bg-neutral-850 border-r border-neutral-700 overflow-y-auto">
      <Section label="Batches" action={<AddButton onClick={onAddBatch} />}>
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
            return (
              <div
                key={b.id}
                draggable
                onDragStart={() => setDraggedBatchId(b.id)}
                onDragOver={(e) => e.preventDefault()}
                onDrop={(e) => { e.preventDefault(); onReorderBatch(b.id); }}
                onDragEnd={() => setDraggedBatchId(null)}
                className={`group relative ${draggedBatchId === b.id ? "opacity-40" : ""}`}
              >
                <button
                  onClick={() => onSelectBatch(b)}
                  onDoubleClick={async () => {
                    try {
                      const { openPath } = await import("@tauri-apps/plugin-opener");
                      await openPath(b.source_path);
                    } catch {}
                  }}
                  className={[
                    "w-full text-left px-3 py-1.5 pr-8 text-sm transition-colors",
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

      <Section label="Frames" action={<AddButton onClick={onAddFrame} />}>
        {event.frame_presets.length === 0 ? (
          <p className="px-3 py-1 text-xs text-neutral-600">
            No frames —{" "}
            <button onClick={onAddFrame} className="text-blue-400 hover:text-blue-300 underline">
              add one
            </button>
          </p>
        ) : (
          event.frame_presets.map((p) => (
            <div key={p.id} className="group relative">
              <SidebarItem
                label={p.name}
                sublabel={`${p.target_ratio_w}:${p.target_ratio_h} · ${p.crop_method === "center" ? "center" : "rule of thirds"}`}
              />
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
          ))
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

function AddButton({ onClick }: { onClick: () => void }) {
  return (
    <button onClick={onClick} className="text-[10px] text-blue-400 hover:text-blue-300 font-medium">
      + Add
    </button>
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
  label: string; sublabel?: string; active?: boolean; onClick?: () => void;
}) {
  const cls = [
    "w-full text-left px-3 py-1.5 text-sm transition-colors",
    active ? "bg-blue-600/20 text-blue-300" : "text-neutral-300",
    onClick ? "hover:bg-neutral-700/60" : "",
  ].join(" ");

  if (onClick) {
    return (
      <button onClick={onClick} className={cls}>
        <span className="block truncate">{label}</span>
        {sublabel && <span className="block text-[10px] text-neutral-500">{sublabel}</span>}
      </button>
    );
  }
  return (
    <div className={cls}>
      <span className="block truncate">{label}</span>
      {sublabel && <span className="block text-[10px] text-neutral-500">{sublabel}</span>}
    </div>
  );
}
