import { useTranslation } from "react-i18next";
import type { SortKey, ViewMode } from "../App";
import { PictureIcon } from "./icons";

type Props = {
  viewMode: ViewMode;
  onSetViewMode: (mode: ViewMode) => void;
  hideEmpty: boolean;
  onToggleHideEmpty: () => void;
  cellSize: number;
  onZoom: (dir: 1 | -1) => void;
  sortKey: SortKey;
  sortDir: 1 | -1;
  onSortKey: (key: SortKey) => void;
  onToggleSortDir: () => void;
};

const MIN_CELL = 100;
const MAX_CELL = 280;

/**
 * Slim gallery view-controls bar (sort, grid zoom, hide-empty). Extracted from
 * the old tab strip; folder navigation now lives in the sidebar tree.
 * `hideEmpty` hides folders with no photos in the tree and cards with no copies
 * in the grid.
 */
export default function ViewControls({
  viewMode, onSetViewMode,
  hideEmpty, onToggleHideEmpty, cellSize, onZoom,
  sortKey, sortDir, onSortKey, onToggleSortDir,
}: Props) {
  const { t } = useTranslation();

  return (
    <div className="flex items-center gap-3 px-3 py-1.5 bg-neutral-900 border-b border-neutral-800 shrink-0">
      {/* View-mode toggle: plain gallery vs. export-canvas preview */}
      <div className="flex items-center gap-0.5 rounded-full bg-neutral-800 p-0.5">
        <ModeButton label={t("view.modeGallery")} active={viewMode === "gallery"} onClick={() => onSetViewMode("gallery")} />
        <ModeButton label={t("view.modeCanvas")} active={viewMode === "canvas"} onClick={() => onSetViewMode("canvas")} />
      </div>

      {viewMode === "gallery" && <>
      <div className="flex items-center gap-1">
        <select
          value={sortKey}
          onChange={(e) => onSortKey(e.target.value as SortKey)}
          title={t("view.sortBy")}
          className="text-xs bg-neutral-800 text-neutral-300 rounded px-1.5 py-1 border border-neutral-700 focus:outline-none focus:ring-1 focus:ring-accent"
        >
          <option value="name">{t("view.sortName")}</option>
          <option value="created">{t("view.sortCreated")}</option>
          <option value="modified">{t("view.sortModified")}</option>
          <option value="size">{t("view.sortSize")}</option>
        </select>
        <button
          type="button"
          onClick={onToggleSortDir}
          title={sortDir === 1 ? t("view.sortAsc") : t("view.sortDesc")}
          className="flex items-center justify-center w-6 h-6 rounded text-neutral-300 hover:bg-neutral-800 text-sm leading-none"
        >
          {sortDir === 1 ? "↑" : "↓"}
        </button>
      </div>

      <div className="ms-auto flex items-center gap-3">
        <div className="flex items-center gap-0.5 rounded-full bg-neutral-800 p-0.5">
          <ZoomButton label="−" onClick={() => onZoom(-1)} disabled={cellSize <= MIN_CELL} title={t("view.zoomOut")} />
          <PictureIcon className="w-3.5 h-3.5 text-neutral-500 mx-0.5" />
          <ZoomButton label="+" onClick={() => onZoom(1)} disabled={cellSize >= MAX_CELL} title={t("view.zoomIn")} />
        </div>

        <label className="flex items-center gap-2 text-xs text-neutral-400 cursor-pointer select-none whitespace-nowrap">
          {t("galleryToolbar.hideEmpty")}
          <button
            type="button"
            role="switch"
            aria-checked={hideEmpty}
            onClick={onToggleHideEmpty}
            className={[
              "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors",
              hideEmpty ? "bg-accent" : "bg-neutral-600",
            ].join(" ")}
          >
            <span
              className={[
                "inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform",
                hideEmpty ? "translate-x-4 rtl:-translate-x-4" : "translate-x-0.5 rtl:-translate-x-0.5",
              ].join(" ")}
            />
          </button>
        </label>
      </div>
      </>}
    </div>
  );
}

function ModeButton({ label, active, onClick }: {
  label: string; active: boolean; onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={[
        "px-2.5 py-0.5 text-xs rounded-full transition-colors",
        active ? "bg-accent text-accent-fg font-medium" : "text-neutral-300 hover:bg-white/10",
      ].join(" ")}
    >
      {label}
    </button>
  );
}

function ZoomButton({ label, onClick, disabled, title }: {
  label: string; onClick: () => void; disabled?: boolean; title: string;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      className="flex items-center justify-center w-5 h-5 rounded-full text-sm font-medium text-neutral-300 hover:bg-white/10 disabled:opacity-30 leading-none"
    >
      {label}
    </button>
  );
}
