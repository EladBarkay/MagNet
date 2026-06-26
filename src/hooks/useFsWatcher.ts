import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { OrenewEvent, PhotoBatch } from "../types";
import { EVENTS } from "../constants";
import { parentDir } from "../lib/paths";

type Handlers = {
  onEvent: (e: OrenewEvent) => void;
  onActiveBatch: (b: PhotoBatch) => void;
  onFrameChanged: () => void;
  onTreeChanged: () => void;
};

/**
 * Wires the backend `fs-changed` event to the UI. The backend emits the changed
 * file path; we decide whether it belongs to a batch folder (→ refresh that
 * batch) or is a frame PNG (→ signal a preview refetch).
 *
 * The listener is registered once and reads current state via refs to avoid
 * stale closures, so passing fresh `event`/`activeBatch` each render is fine.
 */
export function useFsWatcher(
  event: OrenewEvent | null,
  activeBatch: PhotoBatch | null,
  handlers: Handlers
) {
  const eventRef = useRef(event);
  const activeBatchRef = useRef(activeBatch);
  const handlersRef = useRef(handlers);
  useEffect(() => { eventRef.current = event; }, [event]);
  useEffect(() => { activeBatchRef.current = activeBatch; }, [activeBatch]);
  useEffect(() => { handlersRef.current = handlers; });

  // Debounce the folder-tree refetch: a single SD dump fires many file events.
  const treeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const norm = (s: string) => s.replace(/\\/g, "/");

    const unlistenPromise = listen<string>(EVENTS.FS_CHANGED, async (e) => {
      const cur = eventRef.current;
      if (!cur) return;
      const changedPath = norm(e.payload);

      // Frame PNG change → clear Rust preview cache for that preset + refresh UI.
      const changedPreset = cur.frame_presets.find(
        (fp) =>
          norm(fp.landscape_frame_path) === changedPath ||
          norm(fp.portrait_frame_path) === changedPath
      );
      if (changedPreset) {
        invoke("clear_framed_preview_cache", { presetId: changedPreset.id }).catch(() => {});
        handlersRef.current.onFrameChanged();
        return;
      }

      // Any other change under the watched root may add/remove a folder → refetch
      // the sidebar tree (debounced).
      if (treeTimer.current) clearTimeout(treeTimer.current);
      treeTimer.current = setTimeout(() => handlersRef.current.onTreeChanged(), 300);

      // Photo change → refresh the owning batch (if it's a folder we've opened).
      const folder = parentDir(changedPath);
      const batch = cur.batches.find((b) => norm(b.source_path) === folder);
      if (!batch) return;
      try {
        const updated = await invoke<OrenewEvent>("refresh_batch", {
          eventId: cur.id,
          batchId: batch.id,
        });
        handlersRef.current.onEvent(updated);
        const active = activeBatchRef.current;
        if (active) {
          const refreshed = updated.batches.find((b) => b.id === active.id);
          if (refreshed) handlersRef.current.onActiveBatch(refreshed);
        }
      } catch (err) {
        console.error("refresh_batch failed:", err);
      }
    });
    return () => { unlistenPromise.then((u) => u()); };
  }, []); // register once; reads current state via refs
}
