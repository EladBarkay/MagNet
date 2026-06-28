import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { FolderEntry } from "../types";
import { basename } from "../lib/paths";

type Props = {
  rootPath: string | null;
  activePath: string | null;
  hideEmpty: boolean;
  onSelectFolder: (path: string) => void;
};

const norm = (s: string) => s.replace(/\\/g, "/").replace(/\/$/, "");

/**
 * Lightroom-style filesystem sidebar. Renders the event's folder tree starting at
 * the event root; each node lazily loads its immediate subfolders (one `list_folder`
 * call) only when expanded — the whole tree is never walked up front. Clicking a
 * folder loads its photos into the gallery.
 */
export default function Sidebar({ rootPath, activePath, hideEmpty, onSelectFolder }: Props) {
  if (!rootPath) return <aside className="w-60 shrink-0 border-e border-neutral-800 bg-neutral-900" />;

  const root: FolderEntry = {
    name: basename(rootPath),
    path: rootPath,
    photo_count: 0,
    has_subfolders: true,
  };

  return (
    <aside className="w-60 shrink-0 overflow-y-auto border-e border-neutral-800 bg-neutral-900 py-1.5">
      <Node node={root} depth={0} activePath={activePath} hideEmpty={hideEmpty} onSelectFolder={onSelectFolder} defaultOpen />
    </aside>
  );
}

function Node({
  node, depth, activePath, hideEmpty, onSelectFolder, defaultOpen = false,
}: {
  node: FolderEntry;
  depth: number;
  activePath: string | null;
  hideEmpty: boolean;
  onSelectFolder: (path: string) => void;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const [children, setChildren] = useState<FolderEntry[] | null>(null);
  // Right-click context menu position (viewport coords), or null when closed.
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

  // Lazy-load immediate subfolders the first time the node opens.
  useEffect(() => {
    if (!open || children !== null) return;
    invoke<FolderEntry[]>("list_folder", { folder: node.path })
      .then(setChildren)
      .catch(() => setChildren([]));
  }, [open, children, node.path]);

  const active = activePath != null && norm(node.path) === norm(activePath);
  const visibleChildren = (children ?? []).filter(
    (c) => !hideEmpty || c.photo_count > 0 || c.has_subfolders
  );

  return (
    <>
      <div
        className={[
          "group flex items-center gap-1 pe-2 py-1 cursor-pointer transition-colors",
          active ? "bg-accent/15 text-accent" : "text-neutral-300 hover:bg-neutral-800",
        ].join(" ")}
        style={{ paddingInlineStart: 8 + depth * 14 }}
        onClick={() => onSelectFolder(node.path)}
        onDoubleClick={() => { if (node.has_subfolders) setOpen((v) => !v); }}
        onContextMenu={(e) => { e.preventDefault(); setMenu({ x: e.clientX, y: e.clientY }); }}
      >
        <button
          type="button"
          onClick={(e) => { e.stopPropagation(); if (node.has_subfolders) setOpen((v) => !v); }}
          className={[
            "w-4 shrink-0 text-[10px] leading-none text-neutral-500",
            node.has_subfolders ? "hover:text-neutral-200" : "invisible",
          ].join(" ")}
        >
          {open ? "▾" : "▸"}
        </button>
        <span className="text-sm truncate flex-1">{node.name}</span>
        {node.photo_count > 0 && (
          <span className={["text-[10px] tabular-nums", active ? "text-accent/70" : "text-neutral-600"].join(" ")}>
            {node.photo_count}
          </span>
        )}
      </div>
      {menu && (
        <FolderContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          onOpenInExplorer={async () => {
            setMenu(null);
            try {
              const { openPath } = await import("@tauri-apps/plugin-opener");
              await openPath(node.path);
            } catch {}
          }}
        />
      )}
      {open &&
        visibleChildren.map((c) => (
          <Node
            key={c.path}
            node={c}
            depth={depth + 1}
            activePath={activePath}
            hideEmpty={hideEmpty}
            onSelectFolder={onSelectFolder}
          />
        ))}
    </>
  );
}

// Minimal right-click menu: one action, dismissed by Escape or any outside click
// (a full-screen transparent backdrop). Fixed-positioned at the cursor.
function FolderContextMenu({
  x, y, onClose, onOpenInExplorer,
}: {
  x: number;
  y: number;
  onClose: () => void;
  onOpenInExplorer: () => void;
}) {
  const { t } = useTranslation();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <>
      <div className="fixed inset-0 z-[100]" onClick={onClose} onContextMenu={(e) => { e.preventDefault(); onClose(); }} />
      <div
        className="fixed z-[101] min-w-44 rounded-md border border-neutral-700 bg-neutral-800 py-1 shadow-xl"
        style={{ top: y, left: x }}
      >
        <button
          onClick={onOpenInExplorer}
          className="w-full px-3 py-1.5 text-start text-sm text-neutral-200 hover:bg-neutral-700"
        >
          {t("sidebar.openInExplorer")}
        </button>
      </div>
    </>
  );
}
