import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Gallery from "./components/Gallery";
import PreviewPanel from "./components/PreviewPanel";
import ProcessDialog from "./components/ProcessDialog";
import FramePresetDialog from "./components/FramePresetDialog";
import SettingsDialog from "./components/SettingsDialog";
import CanvasPresetManager from "./components/CanvasPresetManager";
import Toolbar from "./components/Toolbar";
import Sidebar from "./components/Sidebar";
import EmptyState from "./components/EmptyState";
import { useFsWatcher } from "./hooks/useFsWatcher";
import { MagnetEvent, Orientation, Photo, PhotoBatch, FramePreset, LicenseInfo } from "./types";

type Modal = "process" | "addFrame" | "settings" | "canvasPresets" | null;

export default function App() {
  const [event, setEvent] = useState<MagnetEvent | null>(null);
  const [activeBatch, setActiveBatch] = useState<PhotoBatch | null>(null);
  const [selected, setSelected] = useState<Photo | null>(null);
  const [modal, setModal] = useState<Modal>(null);
  const [status, setStatus] = useState("");
  const [draggedBatchId, setDraggedBatchId] = useState<string | null>(null);
  const [license, setLicense] = useState<LicenseInfo | null>(null);
  const [frameNonce, setFrameNonce] = useState(0);
  const [editingFrame, setEditingFrame] = useState<FramePreset | null>(null);
  // Unified per-photo queue: photoId → quantity (session-only).
  const [photoQueue, setPhotoQueue] = useState<Record<string, number>>({});
  const [cellSize, setCellSize] = useState(168);
  const [previewWidth, setPreviewWidth] = useState(288);
  const previewDragRef = useRef<{ startX: number; startWidth: number } | null>(null);

  useEffect(() => {
    invoke<LicenseInfo | null>("get_license_info")
      .then((info) => setLicense(info ?? null))
      .catch(() => {});
  }, []);

  // Background revalidation: update license when the background task resolves.
  useEffect(() => {
    const unsub = listen<void>("tier-changed", async () => {
      try {
        const info = await invoke<LicenseInfo | null>("get_license_info");
        setLicense(info ?? null);
      } catch {}
    });
    const unsub2 = listen<void>("license-expired", () => setLicense(null));
    return () => { unsub.then(fn => fn()); unsub2.then(fn => fn()); };
  }, []);

  useFsWatcher(event, activeBatch, {
    onEvent: setEvent,
    onActiveBatch: setActiveBatch,
    onFrameChanged: () => setFrameNonce((n) => n + 1),
  });

  function initQueueForBatch(batch: PhotoBatch | null | undefined): Record<string, number> {
    if (!batch) return {};
    const q: Record<string, number> = {};
    for (const p of batch.photos) q[p.id] = 1;
    return q;
  }

  async function openEvent() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const folder = await open({ directory: true, multiple: false });
      if (!folder) return;
      setStatus("Loading…");
      const evt = await invoke<MagnetEvent>("open_event", { path: folder });
      setEvent(evt);
      setActiveBatch(evt.batches[0] ?? null);
      setSelected(null);
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
    if (!event || !draggedBatchId || draggedBatchId === targetId) return;
    const batches = [...event.batches];
    const fromIdx = batches.findIndex((b) => b.id === draggedBatchId);
    const toIdx = batches.findIndex((b) => b.id === targetId);
    if (fromIdx < 0 || toIdx < 0) return;
    const [moved] = batches.splice(fromIdx, 1);
    batches.splice(toIdx, 0, moved);
    const updated = { ...event, batches };
    setEvent(updated);
    invoke("save_event", { event: updated }).catch(() => {});
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
      setSelected(null);
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
  const queuedTotal = Object.values(photoQueue).reduce((s, q) => s + q, 0);

  // Derive uniform qty from queue — 0 if empty or mixed values.
  const allQty = photos.length > 0 && photos.every(
    (p) => (photoQueue[p.id] ?? 0) === (photoQueue[photos[0].id] ?? 0)
  ) ? (photoQueue[photos[0].id] ?? 0) : 0;

  // Keyboard navigation through photos when preview is open.
  useEffect(() => {
    if (!selected) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
        e.preventDefault();
        const idx = photos.findIndex((p) => p.id === selected.id);
        if (idx < 0) return;
        const next = e.key === "ArrowRight"
          ? Math.min(photos.length - 1, idx + 1)
          : Math.max(0, idx - 1);
        setSelected(photos[next]);
      }
      if (e.key === "Escape") setSelected(null);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [selected, photos]);

  function adjustQty(photoId: string, delta: number) {
    setPhotoQueue((prev) => {
      const next = Math.max(0, (prev[photoId] ?? 0) + delta);
      const updated = { ...prev };
      if (next === 0) delete updated[photoId];
      else updated[photoId] = next;
      return updated;
    });
  }

  function handleSetAllQty(qty: number) {
    if (!activeBatch) return;
    if (qty <= 0) {
      setPhotoQueue({});
      return;
    }
    const q: Record<string, number> = {};
    for (const p of activeBatch.photos) q[p.id] = qty;
    setPhotoQueue(q);
  }

  async function handleOrientationOverride(photoId: string, orientation: Orientation) {
    if (!event) return;
    try {
      await invoke("set_orientation_override", { eventId: event.id, photoId, orientation });
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
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  }

  // After processing: optimistically bump counts for queued photos, clear queue.
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
    setPhotoQueue({});
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
        license={license}
        status={status}
        totalPhotos={totalPhotos}
        activeBatch={activeBatch}
        queuedTotal={queuedTotal}
        allQty={allQty}
        cellSize={cellSize}
        onOpenEvent={openEvent}
        onDeleteEvent={deleteEvent}
        onProcess={() => setModal("process")}
        onSettings={() => setModal("settings")}
        onSetAllQty={handleSetAllQty}
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
            onSelectBatch={(b) => { setActiveBatch(b); setSelected(null); setPhotoQueue(initQueueForBatch(b)); }}
            onDeleteBatch={deleteBatch}
            onReorderBatch={reorderBatch}
            onAddFrame={() => setModal("addFrame")}
            onEditFrame={setEditingFrame}
            onDeleteFrame={deleteFramePreset}
            onManageCanvas={() => setModal("canvasPresets")}
          />
        ) : (
          <aside className="w-52 shrink-0 flex flex-col bg-neutral-850 border-r border-neutral-700">
            <div className="flex-1 flex items-center justify-center text-neutral-600 text-xs p-4 text-center">
              Open an event to begin
            </div>
          </aside>
        )}

        {event ? (
          <div className="flex flex-1 overflow-hidden">
            <Gallery
              photos={photos}
              selectedId={selected?.id ?? null}
              onSelect={setSelected}
              photoQueue={photoQueue}
              onQtyDelta={adjustQty}
              cellSize={cellSize}
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
                  onOrientationOverride={handleOrientationOverride}
                  width={previewWidth}
                />
              </>
            )}
          </div>
        ) : (
          <EmptyState onOpen={openEvent} />
        )}
      </div>

      {/* Modals */}
      {modal === "process" && event && (
        <ProcessDialog
          event={event}
          photoQueue={photoQueue}
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
          license={license}
          onClose={() => setModal(null)}
          onLicenseChange={setLicense}
        />
      )}
      {modal === "canvasPresets" && event && (
        <CanvasPresetManager
          event={event}
          onClose={() => setModal(null)}
          onEventUpdate={updateEvent}
        />
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
