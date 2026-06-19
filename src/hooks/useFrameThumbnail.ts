import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

// Small PNG preview of a frame file (alpha preserved). Cached by path for the app
// lifetime; the frame-preset dialog is short-lived so a disk edit mid-dialog isn't
// worth busting. Returns null while loading or on error (bad path).
const cache = new Map<string, string>();
const inflight = new Map<string, Promise<string>>();

export function useFrameThumbnail(path: string | null | undefined): string | null {
  const [src, setSrc] = useState<string | null>(path ? cache.get(path) ?? null : null);

  useEffect(() => {
    if (!path) { setSrc(null); return; }
    if (cache.has(path)) { setSrc(cache.get(path)!); return; }

    let cancelled = false;
    const pending =
      inflight.get(path) ??
      invoke<number[]>("get_frame_thumbnail", { path }).then((bytes) => {
        const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: "image/png" }));
        cache.set(path, url);
        inflight.delete(path);
        return url;
      });
    if (!inflight.has(path)) inflight.set(path, pending);

    pending.then((url) => { if (!cancelled) setSrc(url); }).catch(() => { inflight.delete(path); });

    return () => { cancelled = true; };
  }, [path]);

  return src;
}
