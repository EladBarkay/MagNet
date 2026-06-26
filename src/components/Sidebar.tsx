import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FolderNode } from "../types";

type Props = {
  tree: FolderNode | null;
  activePath: string | null;
  hideEmpty: boolean;
  onSelectFolder: (path: string) => void;
};

const norm = (s: string) => s.replace(/\\/g, "/");

// Total images in a folder and all its descendants — used for the hide-empty
// filter so a folder whose photos live only in subfolders stays visible.
function subtreeCount(node: FolderNode): number {
  return node.photo_count + node.children.reduce((n, c) => n + subtreeCount(c), 0);
}

// Paths of every ancestor folder of `target` within `tree` (so we can auto-open
// the tree down to the selected folder).
function ancestorsOf(tree: FolderNode, target: string): string[] {
  const out: string[] = [];
  const walk = (node: FolderNode): boolean => {
    if (norm(node.path) === norm(target)) return true;
    if (node.children.some(walk)) {
      out.push(node.path);
      return true;
    }
    return false;
  };
  walk(tree);
  return out;
}

/**
 * Lightroom-style filesystem sidebar. Renders the event's folder tree; clicking
 * a folder loads its photos into the gallery (lazy batch creation in Rust). No
 * manual add/remove — the tree mirrors the disk.
 */
export default function Sidebar({ tree, activePath, hideEmpty, onSelectFolder }: Props) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  // Open the root by default, and auto-open the path down to the active folder.
  useEffect(() => {
    if (!tree) return;
    setExpanded((prev) => {
      const next = new Set(prev);
      next.add(tree.path);
      if (activePath) for (const a of ancestorsOf(tree, activePath)) next.add(a);
      return next;
    });
  }, [tree, activePath]);

  if (!tree) return <aside className="w-60 shrink-0 border-e border-neutral-800 bg-neutral-900" />;

  return (
    <aside className="w-60 shrink-0 overflow-y-auto border-e border-neutral-800 bg-neutral-900 py-1.5">
      <Row
        node={tree}
        depth={0}
        activePath={activePath}
        hideEmpty={hideEmpty}
        expanded={expanded}
        onToggle={(p) =>
          setExpanded((prev) => {
            const next = new Set(prev);
            next.has(p) ? next.delete(p) : next.add(p);
            return next;
          })
        }
        onSelectFolder={onSelectFolder}
        revealTitle={t("sidebar.revealFolder")}
      />
    </aside>
  );
}

function Row({
  node, depth, activePath, hideEmpty, expanded, onToggle, onSelectFolder, revealTitle,
}: {
  node: FolderNode;
  depth: number;
  activePath: string | null;
  hideEmpty: boolean;
  expanded: Set<string>;
  onToggle: (path: string) => void;
  onSelectFolder: (path: string) => void;
  revealTitle: string;
}) {
  if (hideEmpty && subtreeCount(node) === 0) return null;

  const active = activePath != null && norm(node.path) === norm(activePath);
  const isOpen = expanded.has(node.path);
  const hasChildren = node.children.length > 0;

  return (
    <>
      <div
        className={[
          "group flex items-center gap-1 pe-2 py-1 cursor-pointer transition-colors",
          active ? "bg-accent/15 text-accent" : "text-neutral-300 hover:bg-neutral-800",
        ].join(" ")}
        style={{ paddingInlineStart: 8 + depth * 14 }}
        onClick={() => onSelectFolder(node.path)}
        onDoubleClick={async () => {
          try {
            const { openPath } = await import("@tauri-apps/plugin-opener");
            await openPath(node.path);
          } catch {}
        }}
        title={revealTitle}
      >
        <button
          type="button"
          onClick={(e) => { e.stopPropagation(); if (hasChildren) onToggle(node.path); }}
          className={[
            "w-4 shrink-0 text-[10px] leading-none text-neutral-500",
            hasChildren ? "hover:text-neutral-200" : "invisible",
          ].join(" ")}
        >
          {isOpen ? "▾" : "▸"}
        </button>
        <span className="text-sm truncate flex-1">{node.name}</span>
        {node.photo_count > 0 && (
          <span className={["text-[10px] tabular-nums", active ? "text-accent/70" : "text-neutral-600"].join(" ")}>
            {node.photo_count}
          </span>
        )}
      </div>
      {isOpen &&
        node.children.map((c) => (
          <Row
            key={c.path}
            node={c}
            depth={depth + 1}
            activePath={activePath}
            hideEmpty={hideEmpty}
            expanded={expanded}
            onToggle={onToggle}
            onSelectFolder={onSelectFolder}
            revealTitle={revealTitle}
          />
        ))}
    </>
  );
}
