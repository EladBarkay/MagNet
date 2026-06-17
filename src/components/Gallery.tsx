import { useRef, useState, useEffect, useCallback } from "react";
import { FixedSizeGrid, GridChildComponentProps } from "react-window";
import PhotoCard from "./PhotoCard";
import { Photo } from "../types";

const GAP = 6;

type Props = {
  photos: Photo[];
  /** The last-clicked photo (preview/anchor) — gets the stronger ring. */
  selectedId: string | null;
  /** Every photo in the multi-selection — all get a ring. */
  selectedIds: Set<string>;
  onPhotoClick: (photo: Photo, e: React.MouseEvent) => void;
  photoQueue: Record<string, number>;
  onQtyDelta: (photoId: string, delta: number) => void;
  cellSize?: number;
  onColCountChange?: (n: number) => void;
};

export default function Gallery({ photos, selectedId, selectedIds, onPhotoClick, photoQueue, onQtyDelta, cellSize = 168, onColCountChange }: Props) {
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

  const Cell = useCallback(
    ({ columnIndex, rowIndex, style }: GridChildComponentProps) => {
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
            cellSize={cellSize}
            qty={photoQueue[photo.id] ?? 0}
            onQtyDelta={(delta) => onQtyDelta(photo.id, delta)}
          />
        </div>
      );
    },
    [photos, selectedId, selectedIds, onPhotoClick, colCount, photoQueue, onQtyDelta, cellSize]
  );

  if (photos.length === 0) {
    return (
      <div
        ref={containerRef}
        className="flex-1 flex items-center justify-center text-neutral-600 text-sm"
      >
        No photos in this batch
      </div>
    );
  }

  return (
    <div ref={containerRef} className="flex-1 overflow-hidden bg-neutral-900">
      {size.width > 0 && (
        <FixedSizeGrid
          columnCount={colCount}
          columnWidth={cellStride}
          rowCount={rowCount}
          rowHeight={cellStride}
          width={size.width}
          height={size.height}
          overscanRowCount={3}
          style={{ overflowX: "hidden" }}
        >
          {Cell}
        </FixedSizeGrid>
      )}
    </div>
  );
}
