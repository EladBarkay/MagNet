import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Gallery from "./components/Gallery";
import PreviewPanel from "./components/PreviewPanel";
import ProcessDialog from "./components/ProcessDialog";
import FramePresetDialog from "./components/FramePresetDialog";
import SettingsDialog from "./components/SettingsDialog";
import CanvasPresetForm from "./components/CanvasPresetForm";
import { Modal } from "./components/ui";
import Toolbar from "./components/Toolbar";
import GalleryToolbar from "./components/GalleryToolbar";
import Sidebar from "./components/Sidebar";
import EmptyState from "./components/EmptyState";
import { useFsWatcher } from "./hooks/useFsWatcher";
import { useAuthDeepLink } from "./hooks/useAuthDeepLink";
import { reorderById } from "./lib/reorder";
import { rangeIds } from "./lib/selection";
import { EVENTS } from "./constants";
import { MagnetEvent, Orientation, Photo, PhotoBatch, FramePreset, CanvasPreset, Entitlement } from "./types";

type ModalKind = "process" | "addFrame" | "addCanvas" | "settings" | null;

export default function App() {
  const [event, setEvent] = useState<MagnetEvent | null>(null);
  const [activeBatch, setActiveBatch] = useState<PhotoBatch | null>(null);
  const [selected, setSelected] = useState<Photo | null>(null);
  const [modal, setModal] = useState<ModalKind>(null);
  const [status, setStatus] = useState("");
  const [draggedBatchId, setDraggedBatchId] = useState<string | null>(null);
  const [draggedFrameId, setDraggedFrameId] = useState<string | null>(null);
  const [draggedCanvasId, setDraggedCanvasId] = useState<string | null>(null);
  const [entitlement, setEntitlement] = useState<Entitlement | null>(null);
  const [frameNonce, setFrameNonce] = useState(0);
  const [editingFrame, setEditingFrame] = useState<FramePreset | null>(null);
  const [editingCanvas, setEditingCanvas] = useState<CanvasPreset | null>(null);
  // Unified per-photo queue: photoId → quantity (session-only).
  const [photoQueue, setPhotoQueue] = useState<Record<string, number>>({});
  const [cellSize, setCellSize] = useState(168);
  const [hideEmpty, setHideEmpty] = useState(false);
  const [scanning, setScanning] = useState(false);
  // Multi-selection in the grid; `selected` (above) is the last-clicked photo and
  // drives the preview + shift-range anchor.
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const anchorRef = useRef<string | null>(null);
  // Photo ids the queue has already seeded. New photos that appear later (file
  // watcher) get a default qty of 1; photos the user zeroed stay "seen" so they
  // aren't bumped back up.
  const seenIdsRef = useRef<Set<string>>(new Set());

  function clearSelection() {
    setSelected(null);
    setSelectedIds(new Set());
    anchorRef.current = null;
  }
  const [previewWidth, setPreviewWidth] = useState(288);
  const previewDragRef = useRef<{ startX: number; startWidth: number } | null>(null);
  const colCountRef = useRef(1);

  useEffect(() => {
    invoke<Entitlement | null>("get_entitlement")
      .then((info) => setEntitlement(info ?? null))
      .catch(() => {});
  }, []);

  // Background refresh: update entitlement when the background task resolves.
  useEffect(() => {
    const unsub = listen<void>(EVENTS.TIER_CHANGED, async () => {
      try {
        const info = await invoke<Entitlement | null>("get_entitlement");
        setEntitlement(info ?? null);
      } catch {}
    });
    const unsub2 = listen<void>(EVENTS.LICENSE_EXPIRED, () => setEntitlement(null));
    return () => { unsub.then(fn => fn()); unsub2.then(fn => fn()); };
  }, []);

  // Completes OAuth sign-in when the magnetapp://auth-callback deep link arrives.
  useAuthDeepLink(setEntitlement);

  useFsWatcher(event, activeBatch, {
    onEvent: setEvent,
    onActiveBatch: setActiveBatch,
    onFrameChanged: () => setFrameNonce((n) => n + 1),
  });

  function initQueueForBatch(batch: PhotoBatch | null | undefined): Record<string, number> {
    seenIdsRef.current = new Set(batch?.photos.map((p) => p.id) ?? []);
    if (!batch) return {};
    const q: Record<string, number> = {};
    for (const p of batch.photos) q[p.id] = 1;
    return q;
  }

  // Seed qty=1 for photos that appear after the batch was opened (added on disk
  // and picked up by the watcher). Runs after refresh updates activeBatch.
  useEffect(() => {
    if (!activeBatch) return;
    const newIds = activeBatch.photos.filter((p) => !seenIdsRef.current.has(p.id));
    if (newIds.length === 0) return;
    setPhotoQueue((prev) => {
      const next = { ...prev };
      for (const p of newIds) next[p.id] = 1;
      return next;
    });
    for (const p of newIds) seenIdsRef.current.add(p.id);
  }, [activeBatch]);

  async function openEvent() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const folder = await open({ directory: true, multiple: false });
      if (!folder) return;
      setStatus("Loading…");
      const evt = await invoke<MagnetEvent>("open_event", { path: folder });
      setEvent(evt);
      setActiveBatch(evt.batches[0] ?? null);
      clearSelection();
      setPhotoQueue(initQueueForBatch(evt.batches[0]));
      invoke("sync_watches", { eventId: evt.id }).catch(() => {});
      setStatus("");
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  }

  async function deleteEvent() {
    if (!event) return;
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    const yes = await confirm(
      `Delete event "${event.name}"? This removes all saved settings and cannot be undone.`,
      { title: "Delete event", kind: "warning" }
    );
    if (!yes) return;
    try {
      await invoke("delete_event", { eventId: event.id });
      setEvent(null);
      setActiveBatch(null);
      setSelected(null);
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  }

  async function deleteBatch(batch: PhotoBatch) {
    if (!event) return;
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    const yes = await confirm(
      `Remove batch "${batch.name}" from this event? Files are not deleted.`,
      { title: "Remove batch", kind: "warning" }
    );
    if (!yes) return;
    try {
      const updated = await invoke<MagnetEvent>("delete_batch", { eventId: event.id, batchId: batch.id });
      updateEvent(updated);
      if (activeBatch?.id === batch.id) {
        setActiveBatch(updated.batches[0] ?? null);
        setSelected(null);
      }
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  }

  function reorderBatch(targetId: string) {
    if (!event) return;
    const batches = reorderById(event.batches, draggedBatchId, targetId);
    if (!batches) return;
    const updated = { ...event, batches };
    setEvent(updated);
    invoke("save_event", { event: updated }).catch(() => {});
  }

  function reorderFramePreset(targetId: string) {
    if (!event) return;
    const frame_presets = reorderById(event.frame_presets, draggedFrameId, targetId);
    if (!frame_presets) return;
    const updated = { ...event, frame_presets };
    setEvent(updated);
    invoke("save_event", { event: updated }).catch(() => {});
  }

  function reorderCanvasPreset(targetId: string) {
    if (!event) return;
    const canvas_presets = reorderById(event.canvas_presets, draggedCanvasId, targetId);
    if (!canvas_presets) return;
    const updated = { ...event, canvas_presets };
    setEvent(updated);
    invoke("save_event", { event: updated }).catch(() => {});
  }

  async function deleteCanvasPreset(preset: CanvasPreset) {
    if (!event) return;
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    const yes = await confirm(
      `Delete canvas preset "${preset.name}"?`,
      { title: "Delete canvas preset", kind: "warning" }
    );
    if (!yes) return;
    try {
      await invoke("delete_canvas_preset", { eventId: event.id, presetId: preset.id });
      updateEvent({
        ...event,
        canvas_presets: event.canvas_presets.filter((p) => p.id !== preset.id),
      });
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  }

  async function deleteFramePreset(preset: FramePreset) {
    if (!event) return;
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    const yes = await confirm(
      `Delete frame preset "${preset.name}"? The PNG files are not deleted.`,
      { title: "Delete frame preset", kind: "warning" }
    );
    if (!yes) return;
    try {
      await invoke("delete_frame_preset", { eventId: event.id, presetId: preset.id });
      const remaining = event.frame_presets.filter((p) => p.id !== preset.id);
      updateEvent({
        ...event,
        frame_presets: remaining,
        active_frame_preset_id:
          event.active_frame_preset_id === preset.id
            ? remaining[0]?.id ?? null
            : event.active_frame_preset_id,
      });
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  }

  async function addBatch() {
    if (!event) return;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const folder = await open({
        directory: true,
        multiple: false,
        defaultPath: event.root_path ?? undefined,
      });
      if (!folder) return;
      setStatus("Loading batch…");
      const updated = await invoke<MagnetEvent>("add_batch", { eventId: event.id, folder });
      updateEvent(updated);
      const newBatch = updated.batches[updated.batches.length - 1];
      if (newBatch) { setActiveBatch(newBatch); setPhotoQueue(initQueueForBatch(newBatch)); }
      clearSelection();
      setStatus("");
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  }

  function updateEvent(updated: MagnetEvent) {
    setEvent(updated);
    if (activeBatch) {
      const refreshed = updated.batches.find((b) => b.id === activeBatch.id);
      if (refreshed) setActiveBatch(refreshed);
    }
  }

  const totalPhotos = event?.batches.reduce((n, b) => n + b.photos.length, 0) ?? 0;
  const photos = activeBatch?.photos ?? [];
  // When "hide empty" is on, drop photos queued for 0 copies from the grid.
  const visiblePhotos = hideEmpty ? photos.filter((p) => (photoQueue[p.id] ?? 0) > 0) : photos;

  // Batch actions act on the selection, or the whole batch when nothing is selected.
  const targetIds = selectedIds.size > 0 ? [...selectedIds] : photos.map((p) => p.id);
  // Process/totals only count the targeted photos.
  const effectiveQueue = selectedIds.size > 0
    ? Object.fromEntries(Object.entries(photoQueue).filter(([id]) => selectedIds.has(id)))
    : photoQueue;
  const queuedTotal = Object.values(effectiveQueue).reduce((s, q) => s + q, 0);

  // Suggest per-photo export quantities = number of faces detected. Heavy, so
  // it runs on user click (not automatically) with a live progress count. Scans
  // only the selected photos (or the whole batch if none selected) and merges the
  // result into the queue, leaving non-scanned photos' quantities untouched.
  async function scanFaces() {
    if (!event || !activeBatch) return;
    const ids = selectedIds.size > 0 ? [...selectedIds] : null;
    setScanning(true);
    setStatus("Scanning faces…");
    const unsub = await listen<{ done: number; total: number }>(
      EVENTS.FACE_SCAN_PROGRESS,
      (e) => setStatus(`Scanning faces… ${e.payload.done}/${e.payload.total}`)
    );
    try {
      const counts = await invoke<Record<string, number>>("count_faces_in_batch", {
        eventId: event.id,
        batchId: activeBatch.id,
        photoIds: ids,
      });
      // Positive counts only (matches adjustQty's "no zero entries" rule); a
      // scanned photo with 0 faces falls through to qty 0 → dimmed card.
      setPhotoQueue((prev) => {
        const next = { ...prev };
        for (const [id, n] of Object.entries(counts)) {
          if (n > 0) next[id] = n;
          else delete next[id];
        }
        return next;
      });
      setStatus("");
    } catch (e) {
      setStatus(`Error: ${e}`);
    } finally {
      unsub();
      setScanning(false);
    }
  }

  // Click selects (plain = replace, ctrl = toggle, shift = range from anchor);
  // `selected` tracks the last click for the preview + shift anchor.
  function handlePhotoClick(photo: Photo, e: React.MouseEvent) {
    const id = photo.id;
    setSelected(photo);
    if (e.shiftKey && anchorRef.current) {
      setSelectedIds(new Set(rangeIds(visiblePhotos, anchorRef.current, id)));
    } else if (e.ctrlKey || e.metaKey) {
      setSelectedIds((prev) => {
        const next = new Set(prev);
        next.has(id) ? next.delete(id) : next.add(id);
        return next;
      });
      anchorRef.current = id;
    } else {
      setSelectedIds(new Set([id]));
      anchorRef.current = id;
    }
  }

  // Ctrl/Cmd+A selects every photo currently shown in the grid.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && (e.key === "a" || e.key === "A")) {
        if (!activeBatch) return;
        // Don't hijack select-all inside text fields.
        const t = e.target as HTMLElement | null;
        if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
        e.preventDefault();
        setSelectedIds(new Set(visiblePhotos.map((p) => p.id)));
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [activeBatch, visiblePhotos]);

  // Derive uniform qty across the targeted photos — 0 if empty or mixed values.
  const targetQtys = targetIds.map((id) => photoQueue[id] ?? 0);
  const allQty = targetQtys.length > 0 && targetQtys.every((q) => q === targetQtys[0])
    ? targetQtys[0]
    : 0;

  // Keyboard navigation through photos when preview is open.
  useEffect(() => {
    if (!selected) return;
    const handler = (e: KeyboardEvent) => {
      if (["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown"].includes(e.key)) {
        e.preventDefault();
        const idx = visiblePhotos.findIndex((p) => p.id === selected.id);
        if (idx < 0) return;
        const cols = colCountRef.current;
        let next = idx;
        if (e.key === "ArrowRight") next = Math.min(visiblePhotos.length - 1, idx + 1);
        else if (e.key === "ArrowLeft") next = Math.max(0, idx - 1);
        else if (e.key === "ArrowDown") next = Math.min(visiblePhotos.length - 1, idx + cols);
        else if (e.key === "ArrowUp") next = Math.max(0, idx - cols);
        const nextPhoto = visiblePhotos[next];
        setSelected(nextPhoto);
        setSelectedIds(new Set([nextPhoto.id]));
        anchorRef.current = nextPhoto.id;
      }
      if (e.key === "Escape") setSelected(null);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [selected, visiblePhotos]);

  function adjustQty(photoId: string, delta: number) {
    setPhotoQueue((prev) => {
      const next = Math.max(0, (prev[photoId] ?? 0) + delta);
      const updated = { ...prev };
      if (next === 0) delete updated[photoId];
      else updated[photoId] = next;
      return updated;
    });
  }

  // Set the queued qty for the targeted photos (selection, or whole batch when
  // nothing is selected); other photos' quantities are left untouched.
  function handleSetAllQty(qty: number) {
    if (!activeBatch) return;
    setPhotoQueue((prev) => {
      const next = { ...prev };
      for (const id of targetIds) {
        if (qty <= 0) delete next[id];
        else next[id] = qty;
      }
      return next;
    });
  }

  // Set (or clear, with `null`) a photo's orientation override and mirror the
  // change into event/activeBatch/selected state.
  async function setOrientation(photoId: string, orientation: Orientation | null) {
    if (!event) return;
    try {
      if (orientation) {
        await invoke("set_orientation_override", { eventId: event.id, photoId, orientation });
      } else {
        await invoke("clear_orientation_override", { eventId: event.id, photoId });
      }
      const updatePhoto = (p: Photo): Photo =>
        p.id === photoId ? { ...p, orientation_override: orientation } : p;
      const updatedEvent = {
        ...event,
        batches: event.batches.map((b) => ({ ...b, photos: b.photos.map(updatePhoto) })),
      };
      setEvent(updatedEvent);
      if (activeBatch) {
        const refreshedBatch = updatedEvent.batches.find((b) => b.id === activeBatch.id);
        if (refreshedBatch) setActiveBatch(refreshedBatch);
      }
      setSelected((prev) => (prev?.id === photoId ? updatePhoto(prev) : prev));
      // Orientation changes the crop ratio/frame, so force the preview to refetch.
      setFrameNonce((n) => n + 1);
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  }

  // After processing: optimistically bump counts for processed photos and clear
  // only those from the queue (`queue` is the effective, possibly selection-scoped set).
  function handleProcessed(destination: "print" | "export", queue: Record<string, number>) {
    const bump = (p: Photo): Photo => {
      const qty = queue[p.id] ?? 0;
      if (!qty) return p;
      return destination === "print"
        ? { ...p, print_count: p.print_count + qty }
        : { ...p, export_count: p.export_count + qty };
    };
    setEvent((prev) =>
      prev
        ? { ...prev, batches: prev.batches.map((b) => ({ ...b, photos: b.photos.map(bump) })) }
        : prev
    );
    setActiveBatch((prev) => (prev ? { ...prev, photos: prev.photos.map(bump) } : prev));
    setPhotoQueue((prev) => {
      const next = { ...prev };
      for (const id of Object.keys(queue)) delete next[id];
      return next;
    });
  }

  function onDividerMouseDown(e: React.MouseEvent) {
    e.preventDefault();
    previewDragRef.current = { startX: e.clientX, startWidth: previewWidth };
    const onMove = (ev: MouseEvent) => {
      if (!previewDragRef.current) return;
      const delta = previewDragRef.current.startX - ev.clientX;
      setPreviewWidth(Math.max(240, Math.min(640, previewDragRef.current.startWidth + delta)));
    };
    const onUp = () => {
      previewDragRef.current = null;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  return (
    <div className="flex flex-col h-screen bg-neutral-900 text-neutral-100 select-none">
      <Toolbar
        event={event}
        entitlement={entitlement}
        status={status}
        totalPhotos={totalPhotos}
        queuedTotal={queuedTotal}
        cellSize={cellSize}
        onOpenEvent={openEvent}
        onDeleteEvent={deleteEvent}
        onProcess={() => setModal("process")}
        onSettings={() => setModal("settings")}
        onCellSizeChange={setCellSize}
      />

      <div className="flex flex-1 overflow-hidden">
        {event ? (
          <Sidebar
            event={event}
            activeBatch={activeBatch}
            draggedBatchId={draggedBatchId}
            setDraggedBatchId={setDraggedBatchId}
            onAddBatch={addBatch}
            onSelectBatch={(b) => { setActiveBatch(b); clearSelection(); setPhotoQueue(initQueueForBatch(b)); }}
            onDeleteBatch={deleteBatch}
            onReorderBatch={reorderBatch}
            draggedFrameId={draggedFrameId}
            setDraggedFrameId={setDraggedFrameId}
            onReorderFrame={reorderFramePreset}
            onAddFrame={() => setModal("addFrame")}
            onEditFrame={setEditingFrame}
            onDeleteFrame={deleteFramePreset}
            draggedCanvasId={draggedCanvasId}
            setDraggedCanvasId={setDraggedCanvasId}
            onReorderCanvas={reorderCanvasPreset}
            onAddCanvas={() => setModal("addCanvas")}
            onEditCanvas={setEditingCanvas}
            onDeleteCanvas={deleteCanvasPreset}
          />
        ) : (
          <aside className="w-52 shrink-0 flex flex-col bg-neutral-850 border-r border-neutral-700">
            <div className="flex-1 flex items-center justify-center text-neutral-600 text-xs p-4 text-center">
              Open an event to begin
            </div>
          </aside>
        )}

        {event ? (
          <div className="flex flex-1 flex-col overflow-hidden">
            {activeBatch && activeBatch.photos.length > 0 && (
              <GalleryToolbar
                selectedCount={selectedIds.size}
                allQty={allQty}
                hideEmpty={hideEmpty}
                scanning={scanning}
                onSetAllQty={handleSetAllQty}
                onScanFaces={scanFaces}
                onToggleHideEmpty={() => setHideEmpty((v) => !v)}
              />
            )}
            <div className="flex flex-1 overflow-hidden">
              <Gallery
                photos={visiblePhotos}
                selectedId={selected?.id ?? null}
                selectedIds={selectedIds}
                onPhotoClick={handlePhotoClick}
                photoQueue={photoQueue}
                onQtyDelta={adjustQty}
                cellSize={cellSize}
                onColCountChange={(n) => { colCountRef.current = n; }}
              />
              {selected && (
                <>
                  <div
                    onMouseDown={onDividerMouseDown}
                    className="w-1 cursor-col-resize bg-neutral-700 hover:bg-blue-500 transition-colors shrink-0"
                  />
                  <PreviewPanel
                    event={event}
                    photo={selected}
                    onClose={() => setSelected(null)}
                    frameNonce={frameNonce}
                    onOrientationOverride={(id, o) => setOrientation(id, o)}
                    onClearOrientationOverride={(id) => setOrientation(id, null)}
                    width={previewWidth}
                  />
                </>
              )}
            </div>
          </div>
        ) : (
          <EmptyState onOpen={openEvent} />
        )}
      </div>

      {/* Modals */}
      {modal === "process" && event && (
        <ProcessDialog
          event={event}
          photoQueue={effectiveQueue}
          onClose={() => setModal(null)}
          onEventUpdate={updateEvent}
          onProcessed={handleProcessed}
        />
      )}
      {modal === "addFrame" && event && (
        <FramePresetDialog
          event={event}
          onCreated={(updatedEvent) => {
            updateEvent(updatedEvent);
            invoke("sync_watches", { eventId: updatedEvent.id }).catch(() => {});
            setModal(null);
          }}
          onClose={() => setModal(null)}
        />
      )}
      {modal === "settings" && (
        <SettingsDialog
          entitlement={entitlement}
          onClose={() => setModal(null)}
          onEntitlementChange={setEntitlement}
        />
      )}
      {modal === "addCanvas" && event && (
        <Modal onClose={() => setModal(null)}>
          <CanvasPresetForm
            event={event}
            onCreated={(_preset, updatedEvent) => { updateEvent(updatedEvent); setModal(null); }}
            onCancel={() => setModal(null)}
          />
        </Modal>
      )}
      {editingCanvas && event && (
        <Modal onClose={() => setEditingCanvas(null)}>
          <CanvasPresetForm
            event={event}
            editing={editingCanvas}
            onCreated={(_preset, updatedEvent) => { updateEvent(updatedEvent); setEditingCanvas(null); }}
            onCancel={() => setEditingCanvas(null)}
          />
        </Modal>
      )}
      {editingFrame && event && (
        <FramePresetDialog
          event={event}
          editing={editingFrame}
          onCreated={(updatedEvent) => {
            updateEvent(updatedEvent);
            invoke("sync_watches", { eventId: updatedEvent.id }).catch(() => {});
            setFrameNonce((n) => n + 1);
            setEditingFrame(null);
          }}
          onClose={() => setEditingFrame(null)}
        />
      )}
    </div>
  );
}
