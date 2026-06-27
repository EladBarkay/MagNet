import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * Render one export canvas page (`pageIndex`) to an object URL via Rust. Mirrors
 * `useFramedPreview`, but for a full tiled canvas. `sig` is a cheap signature of
 * the inputs (queue + presets) so the page re-fetches when copies or presets
 * change without diffing the `quantities` object each render.
 */
export function useCanvasPage(
  eventId: string | null,
  framePresetId: string | null,
  canvasPresetId: string | null,
  quantities: Record<string, number>,
  pageIndex: number,
  sig: string
): string | null {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    if (!eventId || !framePresetId || !canvasPresetId) {
      setSrc(null);
      return;
    }

    let cancelled = false;

    invoke<number[]>("get_canvas_preview_page", {
      eventId,
      framePresetId,
      canvasPresetId,
      quantities,
      pageIndex,
    })
      .then((bytes) => {
        if (cancelled) return;
        const url = URL.createObjectURL(
          new Blob([new Uint8Array(bytes)], { type: "image/jpeg" })
        );
        setSrc((prev) => {
          if (prev) URL.revokeObjectURL(prev);
          return url;
        });
      })
      .catch(() => {
        if (!cancelled) setSrc(null);
      });

    return () => {
      cancelled = true;
    };
    // `quantities` is intentionally excluded — `sig` captures its content.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [eventId, framePresetId, canvasPresetId, pageIndex, sig]);

  return src;
}
