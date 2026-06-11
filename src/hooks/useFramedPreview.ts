import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

export function useFramedPreview(
  eventId: string | null,
  photoId: string | null,
  presetId: string | null,
  nonce: number = 0
): string | null {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    if (!eventId || !photoId || !presetId) {
      setSrc(null);
      return;
    }

    let cancelled = false;

    invoke<number[]>("get_framed_preview", { eventId, photoId, presetId })
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
  }, [eventId, photoId, presetId, nonce]);

  return src;
}
