import type React from "react";
import { useRef, useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Grid, type CellComponentProps } from "react-window";
import PhotoCard from "./PhotoCard";
import { Photo } from "../types";

const GAP = 10;

type Props = {
  photos: Photo[];
  /** The last-clicked photo (preview/anchor) — gets the stronger ring. */
  selectedId: string | null;
  /** Every photo in the multi-selection — all get a ring. */
  selectedIds: Set<string>;
  onPhotoClick: (photo: Photo, e: React.MouseEvent) => void;
  onPhotoDoubleClick: (photo: Photo) => void;
  /** Clicking empty space (not a card) clears the selection. */
  onBackgroundClick: () => void;
  photoQueue: Record<string, number>;
  onQtyDelta: (photoId: string, delta: number) => void;
  cellSize?: number;
  onColCountChange?: (n: number) => void;
};

// Per-render data handed to the cell via react-window v2's `cellProps`. Keeping the
// cell component identity stable (module-level) while passing data this way means
// Grid re-renders cells on change but never *remounts* them — which is what
// double-click needs (a remount between the two clicks eats the dblclick).
type CellData = {
  photos: Photo[];
  colCount: number;
  cellSize: number;
  selectedId: string | null;
  selectedIds: Set<string>;
  photoQueue: Record<string, number>;
  onPhotoClick: (photo: Photo, e: React.MouseEvent) => void;
  onPhotoDoubleClick: (photo: Photo) => void;
  onQtyDelta: (photoId: string, delta: number) => void;
};

function Cell({
  columnIndex,
  rowIndex,
  style,
  photos,
  colCount,
  cellSize,
  selectedId,
  selectedIds,
  photoQueue,
  onPhotoClick,
  onPhotoDoubleClick,
  onQtyDelta,
}: CellComponentProps<CellData>) {
  const idx = rowIndex * colCount + columnIndex;
  if (idx >= photos.length) return <div style={style} />;
  const photo = photos[idx];
  return (
    <div
      style={{
        ...style,
        left: (style.left as number) + GAP,
        top: (style.top as number) + GAP,
        width: cellSize,
        height: cellSize,
      }}
    >
      <PhotoCard
        photo={photo}
        selected={selectedIds.has(photo.id)}
        active={photo.id === selectedId}
        onClick={(e) => onPhotoClick(photo, e)}
        onDoubleClick={() => onPhotoDoubleClick(photo)}
        cellSize={cellSize}
        qty={photoQueue[photo.id] ?? 0}
        onQtyDelta={(delta) => onQtyDelta(photo.id, delta)}
      />
    </div>
  );
}

export default function Gallery({ photos, selectedId, selectedIds, onPhotoClick, onPhotoDoubleClick, onBackgroundClick, photoQueue, onQtyDelta, cellSize = 168, onColCountChange }: Props) {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const cellStride = cellSize + GAP;

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const { width, height } = entries[0].contentRect;
      setSize({ width, height });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const colCount = Math.max(1, Math.floor((size.width + GAP) / cellStride));

  useEffect(() => {
    onColCountChange?.(colCount);
  }, [colCount, onColCountChange]);
  const rowCount = Math.ceil(photos.length / colCount);

  // Clear the selection when the click misses every card (empty grid space).
  const handleBackground = (e: React.MouseEvent) => {
    if (!(e.target as HTMLElement).closest("[data-card]")) onBackgroundClick();
  };

  const itemData: CellData = {
    photos, colCount, cellSize, selectedId, selectedIds, photoQueue,
    onPhotoClick, onPhotoDoubleClick, onQtyDelta,
  };

  if (photos.length === 0) {
    return (
      <div
        ref={containerRef}
        onClick={handleBackground}
        className="flex-1 flex items-center justify-center text-neutral-600 text-sm"
      >
        {t("gallery.noPhotos")}
      </div>
    );
  }

  return (
    <div ref={containerRef} onClick={handleBackground} className="flex-1 overflow-hidden bg-neutral-900">
      {size.width > 0 && (
        <Grid<CellData>
          cellComponent={Cell}
          cellProps={itemData}
          columnCount={colCount}
          columnWidth={cellStride}
          rowCount={rowCount}
          rowHeight={cellStride}
          overscanCount={3}
          style={{ width: size.width, height: size.height, overflowX: "hidden" }}
        />
      )}
    </div>
  );
}
