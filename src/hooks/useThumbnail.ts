import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

// Module-level cache survives re-renders; blob URLs are valid for the app lifetime.
// Keyed by path + content hash so an edited photo (new hash) refetches instead of
// serving a stale thumbnail.
const cache = new Map<string, string>();
// Track in-flight requests to avoid duplicate invocations for the same key
const inflight = new Map<string, Promise<string>>();

/**
 * @param photoPath  absolute path to the photo
 * @param contentHash  photo.content_hash — changes when the file changes, busting the cache
 */
export function useThumbnail(photoPath: string, contentHash?: string): string | null {
  const key = contentHash ? `${photoPath}@${contentHash}` : photoPath;
  const [src, setSrc] = useState<string | null>(cache.get(key) ?? null);

  useEffect(() => {
    if (cache.has(key)) {
      setSrc(cache.get(key)!);
      return;
    }

    let cancelled = false;

    const pending =
      inflight.get(key) ??
      invoke<number[]>("get_thumbnail", { photoPath, contentHash }).then((bytes) => {
        const url = URL.createObjectURL(
          new Blob([new Uint8Array(bytes)], { type: "image/jpeg" })
        );
        cache.set(key, url);
        inflight.delete(key);
        return url;
      });

    if (!inflight.has(key)) inflight.set(key, pending);

    pending.then((url) => {
      if (!cancelled) setSrc(url);
    });

    return () => {
      cancelled = true;
    };
  }, [key, photoPath]);

  return src;
}
